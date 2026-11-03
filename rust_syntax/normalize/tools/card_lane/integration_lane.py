#!/usr/bin/env python3
"""Collect approved work on one local branch. This stage never pushes or opens PRs."""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import sys
import time

import publish_lane as publication
import review_lane as review


def connect(root):
    db = publication.connect(root)
    db.execute('''CREATE TABLE IF NOT EXISTS integration_jobs (
        id TEXT PRIMARY KEY, name TEXT NOT NULL, candidate TEXT NOT NULL, source TEXT NOT NULL,
        status TEXT NOT NULL, steps TEXT NOT NULL, next_step INTEGER NOT NULL DEFAULT 0,
        last_head TEXT, detail TEXT, created TEXT NOT NULL, finished TEXT)''')
    db.commit()
    return db


def settings(root):
    cfg = review.config(root)
    if cfg.get('workflow') != 'consolidate_then_publish':
        raise RuntimeError('Local integration requires consolidate_then_publish mode')
    return cfg, cfg['integration'], Path(cfg['integration']['tree'])


def sync(root):
    cfg, integration, tree = settings(root)
    with connect(root) as db:
        # Newer approved revisions supersede older suggestions; all receipts
        # remain immutable, including a version already integrated previously.
        latest = {}
        for row in db.execute("SELECT * FROM jobs WHERE status='passed' AND candidate IS NOT NULL ORDER BY rowid"):
            source = json.loads(row['source'])
            if not isinstance(source.get('issue'), int):
                raise RuntimeError(f"Approved card has no issue: {row['name']}")
            card = dict(job=row['id'], name=row['name'], issue=source['issue'], candidate=row['candidate'])
            if source.get('publication_base'):
                card.update(source_base=source['publication_base'], dependencies=source.get('shared_dependencies', []))
            latest[row['name']] = card
        existing = {r['id'] for r in db.execute('SELECT id FROM integration_jobs')}
    # Git history reads can be expensive on the initial campaign import. Never
    # hold SQLite's writer lock while computing plans; card workers finish here too.
    planned = []
    for card in sorted(latest.values(), key=lambda c: (c['issue'], c['name'])):
        key = hashlib.sha256((card['name']+'\0'+card['candidate']).encode()).hexdigest()[:20]
        if key in existing:
            continue
        prerequisites, finals = publication.composition(tree, cfg['base'], [card])
        planned.append((key, card['name'], card['candidate'], json.dumps(card), 'pending',
                        json.dumps(prerequisites+finals), review.now()))
    with connect(root) as db:
        before = db.total_changes
        db.executemany('''INSERT OR IGNORE INTO integration_jobs
            (id,name,candidate,source,status,steps,created) VALUES(?,?,?,?,?,?,?)''', planned)
        added = db.total_changes - before
        db.commit()
    return added


def clean_branch(tree, integration):
    review.clean(tree)
    if publication.git(tree, 'branch', '--show-current') != integration['branch']:
        raise RuntimeError('Integration worktree is on an unexpected branch')
    publication.git(tree, 'merge-base', '--is-ancestor', integration['base'], 'HEAD')


def upstream_card(tree, card, base):
    """Keep a card independently introduced upstream after its original authoring base."""
    for path in publication.git(tree, 'diff-tree', '--no-commit-id', '--name-only', '-r', card['candidate']).splitlines():
        if not path.startswith('content/') or not path.endswith('.json'):
            continue
        code, text = publication.command(tree, 'git', 'show', card['candidate']+':'+path, check=False)
        if code or json.loads(text).get('name') != card['name']:
            continue
        if publication.command(tree, 'git', 'cat-file', '-e', card['candidate']+'^:'+path, check=False)[0] == 0:
            continue  # This is a deliberate edit to an existing card, not a stale introduction.
        code, text = publication.command(tree, 'git', 'show', base+':'+path, check=False)
        if code == 0 and json.loads(text).get('name') == card['name']:
            return True
    return False


def equivalent(tree, sha):
    if publication.command(tree, 'git', 'merge-base', '--is-ancestor', sha, 'HEAD', check=False)[0] == 0:
        return True
    return publication.git(tree, 'cherry', 'HEAD', sha, sha+'^').startswith('-')


def snapshot(root):
    root = Path(root)
    with connect(root) as db:
        revisions = {r['status']: r['n'] for r in db.execute('SELECT status,COUNT(*) AS n FROM integration_jobs GROUP BY status')}
        counts = {r['status']: r['n'] for r in db.execute('''SELECT status,COUNT(*) AS n FROM integration_jobs
            WHERE rowid IN (SELECT MAX(rowid) FROM integration_jobs GROUP BY name) GROUP BY status''')}
        held = [dict(r) for r in db.execute("SELECT id,name,candidate,status,detail FROM integration_jobs WHERE status IN ('running','needs_resolution')")]
    result = dict(at=review.now(), counts=counts, revision_counts=revisions, held=held)
    review.save(root/'integration/status.json', result)
    return result


def integrate_one(root, defer_prerequisites=False):
    root = Path(root); cfg, integration, tree = settings(root)
    folder = root/'integration'; folder.mkdir(exist_ok=True)
    with (folder/'integration.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        with connect(root) as db:
            held = db.execute("SELECT name FROM integration_jobs WHERE status IN ('running','needs_resolution') LIMIT 1").fetchone()
            if held:
                raise RuntimeError('Resolve retained integration work first: '+held['name'])
            clean_branch(tree, integration)
            row = db.execute("SELECT * FROM integration_jobs WHERE status='pending' ORDER BY rowid LIMIT 1").fetchone()
            if not row:
                return None
            job = dict(row); card = json.loads(job['source']); steps = json.loads(job['steps'])
            proof = folder/'jobs'/job['id']; proof.mkdir(parents=True, exist_ok=True)
            review.save(proof/'source.json', card)
            publication.check_approvals(db, [card])
            approval=db.execute('SELECT id,name,status,candidate,source,result,finished FROM jobs WHERE id=?',(card['job'],)).fetchone()
            review.save(proof/'approval.json',dict(approval))
            if job['next_step'] == 0 and upstream_card(tree, card, integration['base']):
                head = publication.git(tree, 'rev-parse', 'HEAD')
                db.execute("UPDATE integration_jobs SET status='upstream',last_head=?,finished=?,detail=? WHERE id=?",
                           (head,review.now(),'Stale introduction: card already in frozen upstream base',job['id']))
                db.commit(); snapshot(root)
                return card['name']+' already upstream'
            for index in range(job['next_step'], len(steps)):
                sha = steps[index]; before = publication.git(tree, 'rev-parse', 'HEAD')
                db.execute("UPDATE integration_jobs SET status='running',last_head=?,next_step=? WHERE id=?", (before,index,job['id'])); db.commit()
                try:
                    if defer_prerequisites and index < len(steps)-1:
                        # Materialize every caller first, retaining the original
                        # helper proposals for one campaign-wide refactor.
                        review.save(proof/f'step-{index:03d}.json',dict(source=sha,before=before,
                                    head=before,deferred_to_common_refactor=True))
                        db.execute('UPDATE integration_jobs SET next_step=? WHERE id=?',(index+1,job['id']));db.commit()
                        continue
                    # Card revisions have durable receipts; cherry-pick's
                    # --empty=drop handles an identical card already present.
                    # Searching every previous card's patch ID here is quadratic.
                    skipped = index < len(steps)-1 and equivalent(tree, sha)
                    if not skipped:
                        if index < len(steps)-1:
                            publication.pick_prerequisite(tree, sha)
                        else:
                            # A fresh independent review of an integrated card can produce an
                            # initially empty Card attestation. Retain it; redundant nonempty
                            # revisions still drop normally.
                            publication.git(tree, 'cherry-pick', '--allow-empty', '--empty=drop', sha)
                    head = publication.git(tree, 'rev-parse', 'HEAD')
                    review.save(proof/f'step-{index:03d}.json', dict(source=sha,before=before,head=head,equivalent=skipped))
                    db.execute('UPDATE integration_jobs SET next_step=?,last_head=? WHERE id=?', (index+1,head,job['id'])); db.commit()
                except Exception as error:
                    db.execute("UPDATE integration_jobs SET status='needs_resolution',detail=? WHERE id=?", (str(error),job['id'])); db.commit()
                    review.save(proof/'conflict.json', dict(step=index,source=sha,before=before,error=str(error)))
                    snapshot(root)
                    raise
            clean_branch(tree, integration)
            outcome='assembled' if defer_prerequisites else 'integrated'
            db.execute("UPDATE integration_jobs SET status=?,finished=?,detail=NULL WHERE id=?", (outcome,review.now(),job['id'])); db.commit()
            snapshot(root)
            return outcome.capitalize()+' '+card['name']


def dependency_inventory(root):
    root=Path(root);cfg,integration,tree=settings(root);proposals={};seen_commits={}
    with connect(root) as db:
        jobs=[dict(row) for row in db.execute('SELECT name,steps FROM integration_jobs')]
    for job in jobs:
        for sha in json.loads(job['steps'])[:-1]:
            if sha not in seen_commits:
                pid=publication.patch_id(tree,sha);seen_commits[sha]=pid
                if pid not in proposals:
                    proposals[pid]=dict(patch_id=pid,commits=[],cards=[],
                        subject=publication.git(tree,'show','-s','--format=%s',sha),
                        paths=publication.git(tree,'diff-tree','--no-commit-id','--name-only','-r',sha).splitlines())
            item=proposals[seen_commits[sha]]
            if sha not in item['commits']:item['commits'].append(sha)
            if job['name'] not in item['cards']:item['cards'].append(job['name'])
    review.save(root/'integration/prerequisite-proposals.json',list(proposals.values()))
    return len(proposals)


def resume(root, key, reason):
    if not reason.strip():
        raise ValueError('A coordinator resolution needs a reason')
    root = Path(root); cfg, integration, tree = settings(root)
    with (root/'integration/integration.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        clean_branch(tree, integration)
        if publication.command(tree, 'git', 'rev-parse', '--verify', 'CHERRY_PICK_HEAD', check=False)[0] == 0:
            raise RuntimeError('Complete or explicitly skip the retained cherry-pick first')
        with connect(root) as db:
            job = db.execute('SELECT * FROM integration_jobs WHERE id=?', (key,)).fetchone()
            if not job or job['status'] not in ('needs_resolution','running'):
                raise ValueError('No retained integration step to resolve')
            head = publication.git(tree, 'rev-parse', 'HEAD')
            publication.git(tree, 'merge-base', '--is-ancestor', job['last_head'], head)
            proof = root/'integration/jobs'/key/f"resolution-{job['next_step']:03d}.json"
            review.save(proof, dict(at=review.now(),before=job['last_head'],head=head,reason=reason,
                                    changed=publication.git(tree,'diff','--name-only',job['last_head'],head).splitlines(),
                                    validation_required=True))
            db.execute("UPDATE integration_jobs SET status='pending',next_step=next_step+1,last_head=?,detail=NULL WHERE id=?", (head,key)); db.commit()
    snapshot(root)


def promote_snapshot(root, head, validation):
    """Promote tested committed state; no PR or remote is involved."""
    root = Path(root); cfg, integration, tree = settings(root)
    clean_branch(tree, integration)
    if publication.git(tree,'rev-parse','HEAD') != head or validation.get('ok') is not True or validation.get('head') != head:
        raise ValueError('Snapshot validation must match the clean integration head')
    path = root/'integration/snapshots'/f'{head}.json'
    data = dict(kind='local_integration',base=integration['base'],head=head,validation=validation)
    if path.exists() and review.read(path) != data:
        raise ValueError('Integration snapshots are immutable')
    review.save(path,data)
    with (root/'config.json.lock').open('a') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX);cfg=review.config(root)
        cfg['local_base_manifest']=str(path);review.save(root/'config.json',cfg)
    return path


def checkpoint(root):
    root=Path(root);cfg,integration,tree=settings(root)
    with (root/'integration/integration.lock').open('a') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX | fcntl.LOCK_NB);clean_branch(tree,integration)
        head=publication.git(tree,'rev-parse','HEAD');scratch=tree/'target/tmp';scratch.mkdir(parents=True,exist_ok=True)
        env=dict(os.environ,CANTRIP_VERIFY='1',CARGO_BUILD_JOBS='8',CARGO_TARGET_DIR=str(tree/'target'),TMPDIR=str(scratch))
        env.pop('CANTRIP_CARDS_ONLY',None);env.pop('CARD_LANE_FOCUSED',None)
        code,out=publication.command(tree,'cargo','test','--profile','checked','--workspace',env=env,check=False)
        log=root/'integration'/f'checkpoint-{head}.log';log.write_text(out+'\n')
        if code:
            raise RuntimeError('Integration checkpoint failed; see '+str(log))
        return promote_snapshot(root,head,dict(ok=True,head=head,at=review.now(),log=str(log),
                                command='CANTRIP_VERIFY=1 cargo test --profile checked --workspace'))


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--root',required=True,type=Path)
    sub=parser.add_subparsers(dest='action',required=True)
    for name in ('sync','status','run','assemble','dependencies','checkpoint','watch'):sub.add_parser(name)
    fix=sub.add_parser('resume');fix.add_argument('job');fix.add_argument('--reason',required=True)
    args=parser.parse_args();root=args.root
    (root/'integration').mkdir(parents=True,exist_ok=True)
    if args.action=='sync':print(sync(root));return
    if args.action=='status':print(json.dumps(snapshot(root),indent=2));return
    if args.action=='resume':resume(root,args.job,args.reason);return
    if args.action=='dependencies':print(dependency_inventory(root));return
    if args.action=='checkpoint':print(checkpoint(root));return
    stamp=Path(__file__).stat().st_mtime_ns
    while True:
        if args.action=='watch' and Path(__file__).stat().st_mtime_ns != stamp:
            os.execv(sys.executable,[sys.executable,'-u',str(Path(__file__).resolve()),'--root',str(root),'watch'])
        cfg,integration,tree=settings(root)
        if not integration.get('enabled',False):return
        sync(root)
        try:
            result=integrate_one(root,defer_prerequisites=args.action=='assemble' or integration.get('strategy')=='assemble')
        except BlockingIOError:
            if args.action!='watch':raise
            time.sleep(cfg.get('poll_seconds',5));continue
        print(result or 'No approved cards waiting',flush=True)
        if args.action in ('run','assemble'):return
        if result is None:
            time.sleep(cfg.get('poll_seconds',5))


if __name__=='__main__':main()
