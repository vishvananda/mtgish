#!/usr/bin/env python3
"""Configure a campaign and start or drain its prepared systemd worker slots."""
import argparse
import fcntl
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

import review_lane as review

HERE = Path(__file__).resolve().parent
MTGISH = HERE.parents[3]


def unit_prefix(root, cfg):
    value = cfg.get('unit_prefix')
    if value is None:
        name = re.sub(r'[^A-Za-z0-9_.-]+', '-', root.name)
        value = f'mtgish-{name}-{hashlib.sha256(str(root).encode()).hexdigest()[:8]}'
    if not re.fullmatch(r'[A-Za-z0-9_.-]+', value):
        raise ValueError('unit_prefix must contain only letters, digits, dots, underscores or hyphens')
    return value


def initialize(root, lane_root, cantrip, worktrees, base, implementers, reviewers, verifiers, model):
    if (root/'config.json').exists():
        raise ValueError('Campaign already exists; existing configuration is preserved')
    if (root/'implementation-queue.json').exists():
        raise ValueError('Existing implementation queue is preserved; choose a fresh campaign root')
    if min(implementers, reviewers, verifiers) < 0:
        raise ValueError('Worker counts cannot be negative')
    base = review.git(cantrip, 'rev-parse', base+'^{commit}')
    generator = review.git(MTGISH, 'rev-parse', 'HEAD')
    cfg = dict(enabled=False, display_name=root.name, lane_root=str(lane_root), producer_manifests=[],
               base=base, producer_model=model, producer_effort='medium', review_model=model, effort='medium',
               implementation_cards_per_context=20, cards_per_context=20, context_tokens=300000,
               cargo_build_jobs=8, rust_min_stack=16777216,
               poll_seconds=5, repairs=3, timeout_seconds=3600, max_verifier_rejections=3,
               keep_verifier=True, focused_builds=True, implementation_queue=str(root/'implementation-queue.json'),
               provisional_handoff=dict(enabled=False),
               implementers={f'i{i}':dict(tree=str(worktrees/f'i{i}')) for i in range(1,implementers+1)},
               reviewers={**{f'r{i}':dict(tree=str(worktrees/f'r{i}'),role='review') for i in range(1,reviewers+1)},
                          **{f'v{i}':dict(tree=str(worktrees/f'v{i}'),role='confirm') for i in range(1,verifiers+1)}},
               workflow='consolidate_then_publish',
               integration=dict(enabled=True,tree=str(worktrees/'integration'),branch='integration/'+root.name,
                                base=base,strategy='assemble',card_group_size=20,dedup_owner='consolidator',
                                publication_shape='shared_foundation_with_independent_card_groups'),
               publisher=dict(enabled=False,batch_size=20,repo=str(cantrip),upstream_repo='cantrip-rs/cantrip-rs',
                              fork_owner='YOUR_GITHUB_LOGIN',fork_remote='fork',upstream_remote='origin',
                              base_ref='origin/main',worktree_root=str(worktrees),flush_final=True,
                              require_shared_pin=True,check_pending_prs=True),
               resource_monitor=dict(enabled=True),
               cache_cleanup=dict(enabled=True,idle_hours=6,interval_seconds=3600,
                                  worktree_roots=[str(worktrees)],protected_paths=[]))
    cfg['unit_prefix'] = unit_prefix(root,cfg)
    cfg['integration']['branch'] = 'integration/'+cfg['unit_prefix']
    root.mkdir(parents=True,exist_ok=True)
    review.save(root/'config.json',cfg)
    queue = root/'implementation-queue.json'
    if not queue.exists():
        review.save(queue,dict(generator_commit=generator,cantrip_commit=base,cards=[]))
    return cfg


def services(root, cfg):
    prefix = unit_prefix(root,cfg)
    for role, specs, script in [('implement',cfg.get('implementers',{}),'implementation_lane.py'),
                                ('review',cfg.get('reviewers',{}),'review_lane.py')]:
        for slot, spec in specs.items():
            if spec.get('draining'):
                continue
            if not re.fullmatch(r'[A-Za-z0-9_-]+',slot):
                raise ValueError('Invalid worker slot: '+slot)
            folder = root/('implementation' if role=='implement' else 'workers')/slot
            yield dict(unit=f'{prefix}-{role}-{slot}',folder=folder,tree=Path(spec['tree']),
                       command=[sys.executable,'-u',str(HERE/script),'--root',str(root),'worker',slot])
    watchers = [('pipeline','review_lane.py',[])]
    if cfg.get('workflow')=='consolidate_then_publish' and cfg.get('integration',{}).get('enabled'):
        watchers.append(('integration','integration_lane.py',[]))
    if cfg.get('publisher',{}).get('enabled'):
        watchers.append(('publish','publish_lane.py',[]))
    if cfg.get('resource_monitor',{}).get('enabled',True):
        watchers.append(('resources','resource_monitor.py',['--unit-prefix',prefix]))
    for role, script, extra in watchers:
        yield dict(unit=f'{prefix}-{role}',folder=root/role,
                   tree=Path(cfg['integration']['tree']) if role=='integration' else None,
                   command=[sys.executable,'-u',str(HERE/script),'--root',str(root),*extra,'watch'])


def launch_command(service):
    log = service['folder']/'driver.log'
    return ['systemd-run','--user','--collect','--unit',service['unit'],
            '--property=Type=exec','--property=WorkingDirectory='+str(MTGISH),
            '--property=StandardOutput=append:'+str(log),'--property=StandardError=append:'+str(log),
            '--property=Nice=5','--property=Restart=on-failure','--property=RestartSec=5',*service['command']]


def set_enabled(root, enabled):
    with (root/'config.json.lock').open('a') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX)
        cfg=review.config(root);cfg['enabled']=enabled;review.save(root/'config.json',cfg)


def start(root, cfg, dry_run=False):
    planned = list(services(root,cfg))
    if dry_run:
        return [launch_command(s) for s in planned]
    pending=[]
    for service in planned:
        active=subprocess.run(['systemctl','--user','is-active',service['unit']+'.service'],
                              capture_output=True,text=True).stdout.strip()
        if active in ('active','activating','deactivating','reloading'):
            continue
        if service['tree'] is not None:
            review.clean(service['tree'])
        pending.append(service)
    set_enabled(root,True)
    for service in pending:
        service['folder'].mkdir(parents=True,exist_ok=True)
        subprocess.run(launch_command(service),check=True)
    return [s['unit'] for s in pending]


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--root',required=True,type=Path)
    p.add_argument('action',choices=['init','start','drain','status'])
    p.add_argument('--lane-root',type=Path);p.add_argument('--cantrip',type=Path);p.add_argument('--worktrees',type=Path)
    p.add_argument('--base',default='HEAD');p.add_argument('--implementers',type=int,default=6)
    p.add_argument('--reviewers',type=int,default=5);p.add_argument('--verifiers',type=int,default=2)
    p.add_argument('--model',default='gpt-6.1-terra');p.add_argument('--dry-run',action='store_true')
    a=p.parse_args();root=a.root.expanduser().resolve()
    if a.dry_run and a.action!='start':p.error('--dry-run applies to start')
    if a.action=='init':
        if not all([a.lane_root,a.cantrip,a.worktrees]):p.error('init requires --lane-root, --cantrip and --worktrees')
        cfg=initialize(root,a.lane_root.expanduser().resolve(),a.cantrip.expanduser().resolve(),
                       a.worktrees.expanduser().resolve(),a.base,a.implementers,a.reviewers,a.verifiers,a.model)
        print(json.dumps(cfg,indent=2));return
    cfg=review.config(root)
    if a.action=='status':print(json.dumps(review.status(root),indent=2))
    elif a.action=='drain':
        set_enabled(root,False);print('Draining: current cards finish; no new cards are claimed.')
    else:print(json.dumps(start(root,cfg,a.dry_run),indent=2))


if __name__=='__main__':
    main()
