#!/usr/bin/env python3
"""Hand a stopped, incomplete implementation to review without approving it."""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

import review_lane as review


def run(tree, *args, **kwargs):
    return subprocess.run([str(a) for a in args], cwd=tree, check=True, **kwargs)


def compile_candidate(root, tree, folder, name, cfg):
    lane = review.load_lane(root, tree, folder/'state')
    lane.focus_cards([name])
    log = folder/'compile.log'
    with log.open('w') as out:
        compiled = subprocess.run(['cargo','build','--profile','cards','-p','cantrip-script','--bin','corpus'], cwd=tree,
                                  env=lane.environment(), stdout=out, stderr=subprocess.STDOUT,
                                  timeout=cfg.get('provisional_compile_timeout_seconds', 1800))
    if compiled.returncode:
        raise ValueError('provisional focused build failed; see '+str(log))


def freeze_rows(root, revision, expected):
    root = Path(root).resolve(); cfg = review.config(root)
    if not re.fullmatch(r'[0-9a-f]{40}', revision) or not re.fullmatch(r'[0-9a-f]{64}', expected):
        raise ValueError('full revision and rows SHA256 are required')
    if review.read(cfg['implementation_queue']).get('generator_commit') != revision:
        raise ValueError('queue does not name the requested generator revision')
    rows = Path(cfg['lane_root'])/'gen/rows.jsonl'
    data = rows.read_bytes()
    if hashlib.sha256(data).hexdigest() != expected:
        raise ValueError('rows do not match the independently recorded SHA256')
    folder = root/'provisional'; folder.mkdir(parents=True, exist_ok=True)
    snapshot = folder/('rows-'+revision+'.jsonl')
    if snapshot.exists() and hashlib.sha256(snapshot.read_bytes()).hexdigest() != expected:
        raise ValueError('different frozen rows already exist')
    if not snapshot.exists():
        with snapshot.open('xb') as f:
            f.write(data)
    manifest = folder/'legacy-rows.json'
    record = dict(revision=revision, sha256=expected, path=str(snapshot))
    if manifest.exists() and review.read(manifest) != record:
        raise ValueError('different legacy rows manifest already exists')
    if not manifest.exists():
        review.save(manifest, record)
    return record


def handoff(root, job_id):
    root = Path(root).resolve()
    if not re.fullmatch(r'[0-9a-f]{20}', job_id):
        raise ValueError('invalid implementation job id')
    cfg = review.config(root)
    folder = root/'provisional'/job_id
    folder.mkdir(parents=True, exist_ok=True)
    with (folder/'handoff.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        with review.connect(root) as db:
            row = db.execute('SELECT * FROM implementation_jobs WHERE id=?', (job_id,)).fetchone()
            if not row or row['status'] != 'needs_review':
                raise ValueError('only stopped needs_review implementation jobs qualify')
            job = dict(row)
        result = json.loads(job['result'] or '{}')
        card = result.get('card', {})
        if card.get('name') != job['name'] or not job.get('worker'):
            raise ValueError('missing stopped producer identity')
        receipt = folder/'source.json'
        if receipt.exists():
            source = review.read(receipt)
            repo = Path(cfg['publisher']['repo'])
            ref = 'refs/card-lane/provisional/'+job_id
            if (source.get('implementation_job') != job_id or source.get('implementation_revision') != job['revision']
                    or review.git(repo, 'rev-parse', ref) != source.get('commit')
                    or hashlib.sha256((folder/'packet/work.patch').read_bytes()).hexdigest() != source.get('patch_sha256')):
                raise ValueError('existing receipt belongs to another implementation')
            with review.connect(root) as db:
                review.enqueue(db, source, cfg.get('priorities', {}).get(job['name'], 0))
            # A crash after saving the receipt may leave our private tree behind.
            tree = Path(cfg['publisher']['worktree_root'])/('provisional-'+job_id)
            if tree.exists() and review.git(tree, 'write-tree') == review.git(repo, 'rev-parse', source['commit']+'^{tree}'):
                run(repo, 'git', 'worktree', 'remove', '--force', tree, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            return source
        producer = root/'implementation'/job['worker']/'state/cards'/review.names(job['name'])[0]
        packet = folder/'packet'
        if not packet.exists():
            shutil.copytree(producer, packet)
        for name in ('work.patch', 'content.json', 'reference.md', 'report.json'):
            if not (packet/name).is_file():
                raise ValueError('missing producer evidence: '+name)
        report = review.read(packet/'report.json')
        if report.get('decision') != 'needs_review':
            raise ValueError('producer report is not needs_review')
        if any(isinstance(v, dict) and v.get('agrees') is False for v in report.get('validation', [])):
            raise ValueError('producer reported a code disagreement; do not hand off as incomplete work')
        rowfile = packet/'generator-row.json'
        if not rowfile.exists():
            # Legacy packets require an independently pinned, frozen copy of their original rows.
            manifest = review.read(root/'provisional/legacy-rows.json')
            frozen = Path(manifest['path'])
            if (manifest.get('revision') != job['revision'] or
                    hashlib.sha256(frozen.read_bytes()).hexdigest() != manifest.get('sha256')):
                raise ValueError('legacy generator rows do not match their frozen revision')
            import intake_lane
            rowdata = intake_lane.rows_at(frozen).get(job['name'])
            if rowdata is None:
                raise ValueError('missing original generator row')
            review.save(rowfile, dict(revision=job['revision'], row=rowdata))
        bound = review.read(rowfile)
        if bound.get('revision') != job['revision'] or bound.get('row', {}).get('name') != job['name'] or bound['row'].get('gap'):
            raise ValueError('generator row does not bind to this ready revision')
        repo = Path(cfg['publisher']['repo'])
        base = card.get('source_base') or json.loads(job['queue_meta']).get('cantrip_commit')
        if not base or not re.fullmatch(r'[0-9a-f]{40}', base):
            raise ValueError('missing frozen producer base')
        tree = Path(cfg['publisher']['worktree_root'])/('provisional-'+job_id)
        pending = folder/'prepared-source.json'
        if pending.exists():
            source = review.read(pending)
            patch_hash = hashlib.sha256((packet/'work.patch').read_bytes()).hexdigest()
            log_hash = hashlib.sha256((folder/'compile.log').read_bytes()).hexdigest()
            if (source.get('implementation_job') != job_id or source.get('implementation_revision') != job['revision']
                    or source.get('source_base') != base or source.get('patch_sha256') != patch_hash
                    or source.get('compile_sha256') != log_hash or not tree.exists()
                    or review.git(tree, 'write-tree') != review.git(repo, 'rev-parse', source['commit']+'^{tree}')
                    or review.git(repo, 'rev-parse', source['commit']+'^') != base):
                raise ValueError('interrupted handoff evidence does not match; inspect retained worktree')
            ref = 'refs/card-lane/provisional/'+job_id
            current = subprocess.run(['git','rev-parse','--verify',ref],cwd=repo,text=True,capture_output=True)
            if current.returncode == 0 and current.stdout.strip() != source['commit']:
                raise ValueError('provisional ref already points to different work')
            if current.returncode:
                run(repo, 'git', 'update-ref', ref, source['commit'], '0'*40)
            review.save(receipt, source)
            with review.connect(root) as db:
                review.enqueue(db, source, cfg.get('priorities', {}).get(job['name'], 0))
            run(repo, 'git', 'worktree', 'remove', '--force', tree, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            return source
        if tree.exists():
            raise ValueError('prior provisional worktree retained; inspect before recovery')
        run(repo, 'git', 'worktree', 'add', '--detach', tree, base, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        success = False
        try:
            patch = (packet/'work.patch').read_bytes()
            if not patch:
                raise ValueError('empty producer patch')
            run(tree, 'git', 'apply', '--index', '--check', input=patch, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            run(tree, 'git', 'apply', '--index', input=patch, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            paths = review.git(tree, 'diff', '--cached', '--name-only').splitlines()
            stem, slug = review.names(job['name'])
            own = {f'crates/cantrip-cards/src/cards/{stem}.rs', f'content/mtg/cards/{slug}.json', f'docs/card-notes/{stem}.md'}
            if not paths or any(p not in own and not (p.startswith(f'scenarios/{slug}-') and p.endswith('.cantrip')) for p in paths):
                raise ValueError('producer patch has unexpected paths')
            source_path = tree/f'crates/cantrip-cards/src/cards/{stem}.rs'
            content_path = tree/f'content/mtg/cards/{slug}.json'
            generated = bound['row'].get('rust') or ''
            if (source_path.read_text() if source_path.exists() else '') != generated or content_path.read_bytes() != (packet/'content.json').read_bytes():
                raise ValueError('producer code or content differs from pinned generator evidence')
            # The focused list and build use this private tree/target, never a live worker cache.
            compile_candidate(root, tree, folder, job['name'], cfg)
            digest = hashlib.sha256(patch).hexdigest()
            ref = 'refs/card-lane/provisional/'+job_id
            message = f'Review WIP: {job["name"]}\n\nImplementation job: {job_id}\nPatch-SHA256: {digest}\n'
            env = dict(os.environ, GIT_AUTHOR_DATE=job['finished'], GIT_COMMITTER_DATE=job['finished'])
            commit = review.git(tree, 'commit-tree', review.git(tree, 'write-tree'), '-p', base, '-m', message, env=env)
            source = dict(card, commit=commit, source_base=base, publication_base=base, producer=job['worker'],
                          producer_tree=str(tree), packet=str(packet), campaign=str(cfg['lane_root']), provisional=True,
                          implementation_job=job_id, implementation_revision=job['revision'], patch_sha256=digest,
                          compile_sha256=hashlib.sha256((folder/'compile.log').read_bytes()).hexdigest())
            review.save(pending, source)
            # A crash after the ref was written can be retried only with the identical commit.
            current = subprocess.run(['git','rev-parse','--verify',ref],cwd=tree,text=True,capture_output=True)
            if current.returncode == 0 and current.stdout.strip() != commit:
                raise ValueError('provisional ref already points to different work')
            if current.returncode:
                run(tree, 'git', 'update-ref', ref, commit, '0'*40)
            review.save(receipt, source)
            with review.connect(root) as db:
                review.enqueue(db, source, cfg.get('priorities', {}).get(job['name'], 0))
            success = True
            return source
        finally:
            if success:
                run(repo, 'git', 'worktree', 'remove', '--force', tree, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--root', required=True, type=Path)
    p.add_argument('job', nargs='?')
    p.add_argument('--freeze-rows', action='store_true')
    p.add_argument('--revision')
    p.add_argument('--sha256')
    a = p.parse_args()
    if a.freeze_rows:
        if a.job or not a.revision or not a.sha256: p.error('--freeze-rows requires --revision and --sha256, without a job')
        result = freeze_rows(a.root, a.revision, a.sha256)
    else:
        if not a.job or a.revision or a.sha256: p.error('provide exactly one job')
        result = handoff(a.root, a.job)
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
