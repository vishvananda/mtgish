#!/usr/bin/env python3
"""Reserve approved cards, integrate and validate them, then open a fork PR.

A batch is intentionally never repaired or recreated automatically after a failure.
The database and the retained worktree are the recovery record.
"""
import argparse
import fcntl
import json
import os
from pathlib import Path
import re
import sqlite3
import subprocess
import sys
import time

import review_lane as review


def command(cwd, *args, env=None, check=True, input=None):
    p = subprocess.run([str(x) for x in args], cwd=cwd, env=env, input=input,
                       text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    if check and p.returncode:
        raise RuntimeError(f"{' '.join(map(str, args))}:\n{p.stdout[-6000:]}")
    return p.returncode, p.stdout.strip()


def git(cwd, *args, **kw):
    return command(cwd, 'git', *args, **kw)[1]


def connect(root):
    db = review.connect(root)
    db.executescript('''
    CREATE TABLE IF NOT EXISTS publish_batches (
      number INTEGER PRIMARY KEY, branch TEXT UNIQUE NOT NULL, status TEXT NOT NULL,
      base TEXT, head TEXT, pr_url TEXT, detail TEXT, created TEXT NOT NULL, source_base TEXT);
    CREATE TABLE IF NOT EXISTS publish_cards (
      batch INTEGER NOT NULL REFERENCES publish_batches(number), position INTEGER NOT NULL,
      job TEXT NOT NULL UNIQUE, name TEXT NOT NULL UNIQUE, issue INTEGER NOT NULL UNIQUE,
      candidate TEXT NOT NULL, PRIMARY KEY(batch,position));
    ''')
    if 'source_base' not in {r['name'] for r in db.execute('PRAGMA table_info(publish_batches)')}:
        db.execute('ALTER TABLE publish_batches ADD COLUMN source_base TEXT')
        db.commit()
    for column in ('source_base', 'dependencies'):
        if column not in {r['name'] for r in db.execute('PRAGMA table_info(publish_cards)')}:
            db.execute(f'ALTER TABLE publish_cards ADD COLUMN {column} TEXT')
    if 'prerequisites' not in {r['name'] for r in db.execute('PRAGMA table_info(publish_batches)')}:
        db.execute('ALTER TABLE publish_batches ADD COLUMN prerequisites TEXT')
    db.commit()
    return db


def eligible(db):
    rows = []
    for r in db.execute("SELECT j.* FROM jobs j WHERE j.status='passed' AND j.candidate IS NOT NULL AND NOT EXISTS (SELECT 1 FROM publish_cards p WHERE p.job=j.id OR p.name=j.name)"):
        source = json.loads(r['source'])
        issue = source.get('issue')
        if not isinstance(issue, int) or isinstance(issue, bool) or issue <= 0:
            raise ValueError(f"passed job {r['id']} has no numeric issue")
        card = dict(job=r['id'], name=r['name'], issue=issue, candidate=r['candidate'])
        if source.get('publication_base'):
            card.update(source_base=source['publication_base'], dependencies=source.get('shared_dependencies', []))
        rows.append(card)
    return sorted(rows, key=lambda r: (r['issue'], r['name'], r['job']))


def terminal(db, cfg):
    if db.execute("SELECT 1 FROM jobs WHERE status IN ('queued','running') LIMIT 1").fetchone():
        return False
    if db.execute("SELECT 1 FROM sqlite_master WHERE type='table' AND name='implementation_jobs'").fetchone():
        if db.execute("SELECT 1 FROM implementation_jobs WHERE status IN ('queued','running') LIMIT 1").fetchone():
            return False
    path = cfg.get('implementation_queue')
    if not path or not Path(path).exists():
        return False
    return not any(c.get('status') == 'pending' for c in json.loads(Path(path).read_text())['cards'])


def selection(db, cfg):
    p = cfg.get('publisher', {})
    cards = eligible(db)
    size = int(p.get('batch_size', 20))
    if size < 1:
        raise ValueError('batch_size must be positive')
    if len(cards) < size and not (cards and p.get('flush_final') and terminal(db, cfg)):
        return []
    return cards[:size]


def reserve(db, cfg):
    if cfg.get('workflow') == 'consolidate_then_publish':
        return None
    db.execute('BEGIN IMMEDIATE')
    try:
        cards = selection(db, cfg)
        if not cards:
            db.commit()
            return None
        number = db.execute('SELECT COALESCE(MAX(number),0)+1 FROM publish_batches').fetchone()[0]
        prefix = cfg['publisher'].get('branch_prefix', 'card/unlimited-20261103-batch')
        branch = f'{prefix}-{number:03d}'
        db.execute('INSERT INTO publish_batches(number,branch,status,created,source_base,prerequisites) VALUES(?,?,?,?,?,?)',
                   (number, branch, 'reserved', review.now(), cfg['base'],
                    json.dumps(cfg['publisher'].get('prerequisites', []))))
        for i, c in enumerate(cards):
            db.execute('INSERT INTO publish_cards(batch,position,job,name,issue,candidate,source_base,dependencies) VALUES(?,?,?,?,?,?,?,?)',
                       (number, i, c['job'], c['name'], c['issue'], c['candidate'], c.get('source_base'),
                        json.dumps(c['dependencies']) if 'dependencies' in c else None))
        db.commit()
        return number
    except BaseException:
        db.rollback()
        raise


def batch(db, number):
    row = db.execute('SELECT * FROM publish_batches WHERE number=?', (number,)).fetchone()
    cards = []
    for r in db.execute('SELECT job,name,issue,candidate,source_base,dependencies FROM publish_cards WHERE batch=? ORDER BY position', (number,)):
        card = {k: r[k] for k in ('job','name','issue','candidate')}
        if r['source_base']:
            card.update(source_base=r['source_base'], dependencies=json.loads(r['dependencies'] or '[]'))
        cards.append(card)
    return dict(row), cards


def update(db, number, **values):
    db.execute('UPDATE publish_batches SET ' + ','.join(k+'=?' for k in values) + ' WHERE number=?', (*values.values(), number))
    db.commit()


def manifest(root, b, cards, cfg):
    folder = Path(root) / 'publication' / 'batches' / f"{b['number']:03d}"
    folder.mkdir(parents=True, exist_ok=True)
    path = folder / 'manifest.json'
    contents = dict(number=b['number'], branch=b['branch'], source_base=b['source_base'], cards=cards)
    prerequisites = json.loads(b.get('prerequisites') or '[]')
    if prerequisites:
        contents['prerequisites'] = prerequisites
    data = json.dumps(contents, indent=2) + '\n'
    try:
        with path.open('x') as f:
            f.write(data)
            f.flush()
            os.fsync(f.fileno())
    except FileExistsError:
        if path.read_text() != data:
            raise RuntimeError('batch manifest differs from reserved membership')
    return folder


def check_approvals(db, cards):
    for card in cards:
        row = db.execute('SELECT status,candidate FROM jobs WHERE id=?', (card['job'],)).fetchone()
        if not row or row['status'] != 'passed' or row['candidate'] != card['candidate']:
            raise RuntimeError(f"reserved approval changed for {card['name']} ({card['job']})")


class ExternalOperationError(RuntimeError):
    pass


def external_read(function, *args, **kwargs):
    try:
        return function(*args, **kwargs)
    except (RuntimeError, OSError) as e:
        raise ExternalOperationError(str(e)) from e


def patch_id(repo, commit):
    patch = git(repo, 'show', '--format=', '--binary', commit)
    _, out = command(repo, 'git', 'patch-id', '--stable', input=patch)
    if not out:
        raise RuntimeError(f'empty prerequisite {commit}')
    return out.split()[0]


def composition(repo, source_base, cards, dependencies=()):
    prerequisites, finals, seen = [], [], set()
    # Coordinator-pinned integration repairs precede all card prerequisites.
    # They are frozen with the reservation, independent of the workers' bases.
    for dependency in dependencies:
        sha = dependency['commit']
        if not re.fullmatch(r'[0-9a-f]{40}', sha):
            raise RuntimeError('publication prerequisite must be an immutable commit')
        subject = git(repo, 'show', '-s', '--format=%s', sha)
        if not subject.startswith(('Engine: ', 'Mechanics: ')):
            raise RuntimeError(f'non-helper publication prerequisite {sha}: {subject}')
        if len(git(repo, 'rev-list', '--parents', '-n', '1', sha).split()) != 2:
            raise RuntimeError(f'merge or root publication prerequisite: {sha}')
        pid = patch_id(repo, sha)
        if pid not in seen:
            seen.add(pid)
            prerequisites.append(sha)
    for card in cards:
        candidate = card['candidate']
        baseline = card.get('source_base', source_base)
        for dependency in card.get('dependencies', []):
            sha = dependency['commit']
            git(repo, 'merge-base', '--is-ancestor', sha, baseline)
            subject = git(repo, 'show', '-s', '--format=%s', sha)
            if not subject.startswith(('Engine: ', 'Mechanics: ')):
                raise RuntimeError(f'non-helper dependency {sha}: {subject}')
            pid = patch_id(repo, sha)
            if pid not in seen:
                seen.add(pid)
                prerequisites.append(sha)
        git(repo, 'merge-base', '--is-ancestor', baseline, candidate)
        commits = git(repo, 'rev-list', '--reverse', '--first-parent', f'{baseline}..{candidate}').splitlines()
        if not commits or commits[-1] != candidate:
            raise RuntimeError(f"invalid candidate ancestry: {card['name']}")
        for index, sha in enumerate(commits):
            parents = git(repo, 'rev-list', '--parents', '-n', '1', sha).split()
            if len(parents) != 2:
                raise RuntimeError(f'merge or root commit in candidate ancestry: {sha}')
            subject = git(repo, 'show', '-s', '--format=%s', sha)
            if index == len(commits)-1:
                if subject != 'Card: ' + card['name']:
                    raise RuntimeError(f'unexpected final Card commit {sha}: {subject}')
                message = git(repo, 'show', '-s', '--format=%B', sha)
                if not re.search(rf'(?im)^Closes #{card["issue"]}\s*$', message):
                    raise RuntimeError(f'missing issue closure in {sha}')
                finals.append(sha)
            else:
                if not (subject.startswith('Engine: ') or subject.startswith('Mechanics: ')):
                    raise RuntimeError(f'unexpected prerequisite {sha}: {subject}')
                pid = patch_id(repo, sha)
                if pid not in seen:
                    seen.add(pid)
                    prerequisites.append(sha)
    return prerequisites, finals


def validate_remote(repo, remote, expected):
    url = git(repo, 'remote', 'get-url', '--push', remote)
    pattern = r'(?:github\.com[:/])' + re.escape(expected) + r'(?:\.git)?/?$'
    if not re.search(pattern, url, re.I):
        raise RuntimeError(f'remote {remote} does not point to {expected}: {url}')


def merge_counter_additions(base, ours, theirs):
    """Merge only appended counter vocabulary; retain every existing numeric ID."""
    sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
    from cantrip_tree import counter_block
    entry = re.compile(r'^\s*(\w+) = (\d+), "([^"]*)", "([^"]*)";\s*$', re.M)

    def table(text):
        start, end = counter_block(text)
        body = text[start:end]
        values = entry.findall(body)
        if [int(v[1]) for v in values] != list(range(len(values))):
            raise ValueError('counter IDs are not contiguous')
        return start, end, body, values

    bs, be, body, before = table(base)
    ts, te, added_body, incoming = table(theirs)
    _, _, _, current = table(ours)
    if theirs[:ts] != base[:bs] or theirs[te:] != base[be:] or not added_body.startswith(body):
        raise ValueError('prerequisite changes more than appended counter vocabulary')
    added = incoming[len(before):]
    suffix = added_body[len(body):]
    if incoming[:len(before)] != before or not added or entry.sub('',suffix).strip():
        raise ValueError('counter prerequisite is not a plain append')
    known = {v[0]:v for v in current}
    if any(v[0] not in known or known[v[0]][2:] != v[2:] for v in before):
        raise ValueError('existing counter vocabulary changed')
    additions = []
    for name, _, printed, token in added:
        if name in known:
            if known[name][2:] != (printed,token):
                raise ValueError('counter spelling changed')
            continue
        additions.append(f'\n    {name} = {len(current)+len(additions)}, "{printed}", "{token}";')
        known[name] = (name,'',printed,token)
    _, end = counter_block(ours)
    return ours[:end]+''.join(additions)+ours[end:]


def resolve_counter_conflict(tree, sha):
    path = 'crates/cantrip-state/src/vocab.rs'
    if git(tree,'diff','--name-only','--diff-filter=U').splitlines() != [path]:
        return False
    # Only a vocabulary-only prerequisite is eligible for automatic resolution.
    if git(tree,'diff-tree','--no-commit-id','--name-only','-r',sha).splitlines() != [path]:
        return False
    try:
        merged = merge_counter_additions(git(tree,'show',sha+'^:'+path),
                                         git(tree,'show','HEAD:'+path),git(tree,'show',sha+':'+path))
    except ValueError:
        return False
    (tree/path).write_text(merged)
    git(tree,'add','--',path)
    if not git(tree,'diff','--cached','--name-only'):
        git(tree,'cherry-pick','--skip')
    else:
        git(tree,'-c','core.editor=true','cherry-pick','--continue')
    return True


def pick_prerequisite(tree, sha):
    try:
        git(tree,'cherry-pick','--empty=drop',sha)
    except RuntimeError:
        if not resolve_counter_conflict(tree,sha):
            raise


def integration_checks(root, cfg, b, cards, tree, folder):
    if cfg['publisher'].get('check_pending_prs', True):
        pending_pr_checks(cfg, b, tree, folder)
    scratch = tree / 'target' / 'tmp'
    scratch.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, CANTRIP_VERIFY='1', CARGO_BUILD_JOBS='8', CARGO_TARGET_DIR=str(tree / 'target'),
               TMPDIR=str(scratch),
               PATH=str(Path(cfg['lane_root']) / 'bin') + os.pathsep + os.environ.get('PATH', ''))
    env.pop('CANTRIP_CARDS_ONLY', None)
    env.pop('CARD_LANE_FOCUSED', None)
    changed = git(tree, 'diff', '--name-only', b['base'], 'HEAD').splitlines()
    # Resolve each modified card module by the actual content registry, and fail closed if ambiguous.
    sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
    from cantrip_candidates import file_stem, content_slug
    names = {c['name'] for c in cards}
    stems = {Path(p).stem for p in changed if p.startswith('crates/cantrip-cards/src/cards/') and p.endswith('.rs') and Path(p).name != 'mod.rs'}
    contents = {Path(p).stem for p in changed if p.startswith('content/mtg/cards/') and p.endswith('.json')}
    for path in (tree / 'content/mtg/cards').glob('*.json'):
        if path.stem not in contents and not stems:
            continue
        try:
            data = json.loads(path.read_text())
            name = data.get('name')
            if name and (file_stem(name) in stems or path.stem in contents):
                names.add(name)
        except (ValueError, AttributeError):
            continue
    unresolved = stems - {file_stem(n) for n in names}
    unresolved_content = contents - {content_slug(n) for n in names}
    if unresolved or unresolved_content:
        raise RuntimeError(f'cannot identify changed cards: {sorted(unresolved | unresolved_content)}')
    checks = [('build-corpus', ['cargo', 'build', '--profile', 'cards', '-p', 'cantrip-script', '--bin', 'corpus'])]
    for i, name in enumerate(sorted(names)):
        checks.append((f'corpus-{i:03d}', [str(tree / 'target/cards/corpus'), '--card', name, '--brief']))
    for i, path in enumerate(changed):
        if path.startswith('scenarios/') and path.endswith('.cantrip') and (tree / path).is_file():
            checks.append((f'scenario-{i:03d}', [str(tree / 'target/cards/corpus'), '--file', path, '--brief']))
    # Full registry is used for the layer contract and for the workspace suite.
    checks += [('layers', ['cargo', 'test', '--profile', 'checked', '-q', '-p', 'cantrip-script', '--test', 'layer_pass_cases']),
               ('workspace', ['cargo', 'test', '--profile', 'checked', '--workspace'])]
    for label, args in checks:
        code, out = command(tree, *args, env=env, check=False)
        (folder / (label + '.log')).write_text(out + '\n')
        if code or ((label.startswith('corpus-') or label.startswith('scenario-')) and (not re.search(r'[1-9]\d* pass, 0 fail, 0 known bugs?', out) or not re.search(r'0 need cards', out))):
            raise RuntimeError(f'validation failed: {label}; see {folder / (label + ".log")}')
    if git(tree, 'status', '--porcelain'):
        raise RuntimeError('validation left uncommitted changes')
    return sorted(names)


def pending_pr_checks(cfg, b, tree, folder):
    import pending_publications
    p = cfg['publisher']
    _, raw = external_read(command, tree, 'gh', 'pr', 'list', '--repo', p['upstream_repo'],
                           '--author', p['fork_owner'], '--state', 'open', '--limit', '1000',
                           '--json', 'number,headRefName,headRefOid,headRepositoryOwner')
    pending = [pr for pr in json.loads(raw)
               if pr['headRefName'].startswith('card/')
               and pr.get('headRepositoryOwner', {}).get('login', '').lower() == p['fork_owner'].lower()]
    for pr in pending:
        if command(tree, 'git', 'cat-file', '-e', pr['headRefOid']+'^{commit}', check=False)[0]:
            external_read(git, tree, 'fetch', p.get('fork_remote', 'fork'), pr['headRefOid'])
    head = git(tree, 'rev-parse', 'HEAD')
    result = pending_publications.check_heads(tree, b['branch'], head, pending)
    pending_publications.save(folder/'pending-pr-compatibility.json', result)
    if not result['ok']:
        conflicts = ', '.join('#'+str(pr['pr']) for pr in result['checks'] if not pr['ok'])
        raise RuntimeError('Integrated batch conflicts with pending PRs '+conflicts+'; reconcile the shared foundation first')


def pr_body(cards, prereqs, tested, set_name='Unlimited', dependencies=()):
    lines = [f'Adds {len(cards)} {set_name} cards.', '', '## Cards', '', '| Card | Issue |', '| --- | --- |']
    lines += [f"| {c['name'].replace('|', '&#124;')} | #{c['issue']} |" for c in cards]
    lines += ['', '## Integration', '', f'{len(prereqs)} shared prerequisite commits, followed by {len(cards)} card commits.',
              '', '## Validation', '', f'Focused corpus for {len(tested)} new and changed cards; full-registry layer contract; checked Rust workspace with CANTRIP_VERIFY=1.', '']
    links = sorted({d['pr_url'] for c in cards for d in c.get('dependencies', [])} | {d['pr_url'] for d in dependencies})
    if links:
        lines += ['Prerequisite helper PRs: '+', '.join(links)+'. These commits are included until they land upstream.', '']
    lines += [f"Closes #{c['issue']}" for c in cards]
    return '\n'.join(lines) + '\n'


def publish(root, cfg, db, number):
    if cfg.get('workflow') == 'consolidate_then_publish':
        raise RuntimeError('Publication deferred: export coherent sets from the finished local integration branch')
    p = cfg['publisher']; repo = Path(p['repo']); b, cards = batch(db, number)
    tree = Path(p['worktree_root']) / b['branch'].replace('/', '-')
    try:
        if not b.get('source_base'):
            raise RuntimeError('batch has no frozen source base; inspect legacy reservation')
        folder = manifest(root, b, cards, cfg)
        if b['status'] == 'reserved':
            check_approvals(db, cards)
            validate_remote(repo, p.get('upstream_remote', 'origin'), p['upstream_repo'])
            validate_remote(repo, p.get('fork_remote', 'fork'), p['fork_owner'] + '/' + p['upstream_repo'].split('/')[-1])
            dependencies = json.loads(b.get('prerequisites') or '[]')
            prereqs, finals = composition(repo, b['source_base'], cards, dependencies)
            git(repo, 'fetch', p.get('upstream_remote', 'origin'), 'main')
            base = git(repo, 'rev-parse', p.get('base_ref', 'origin/main') + '^{commit}')
            if p.get('require_shared_pin', False):
                import pending_publications
                import shared_base
                pin = shared_base.read_pin(cfg, repo)
                if not pin:
                    raise RuntimeError('Shared prerequisite promotion requires a validated foundation pin')
                pending_publications.require_promoted(repo, base, prereqs, pin, dependencies)
            update(db, number, status='composing', base=base)
            if tree.exists() or command(repo, 'git', 'show-ref', '--verify', '--quiet', 'refs/heads/' + b['branch'], check=False)[0] == 0:
                raise RuntimeError('publisher worktree or branch already exists')
            env = dict(os.environ, COW_WORKTREE_CONFIG=p.get('cow_config', '/mnt/vizier/cow-cantrip.conf'))
            command(repo, p.get('cow_git', '/home/vishvananda/.local/bin/cow-git'), '-C', p.get('cow_base', '/mnt/vizier/base/cantrip'), 'worktree', 'add', '-b', b['branch'], str(tree), base, env=env)
            for sha in prereqs:
                pick_prerequisite(tree, sha)
            for sha in finals:
                git(tree, 'cherry-pick', '--empty=drop', sha)
            head = git(tree, 'rev-parse', 'HEAD')
            (folder / 'composition.json').write_text(json.dumps(dict(prerequisites=prereqs, finals=finals), indent=2) + '\n')
            update(db, number, status='validating', head=head)
            b, _ = batch(db, number)
            tested = integration_checks(root, cfg, b, cards, tree, folder)
            (folder / 'pr.md').write_text(pr_body(cards, prereqs, tested, p.get('set_name', 'Unlimited'), dependencies))
            update(db, number, status='validated')
            b, _ = batch(db, number)
        if b['status'] in ('composing', 'validating'):
            raise RuntimeError('interrupted integration or validation; worktree retained for inspection')
        if b['status'] in ('validated', 'pushing', 'pushed', 'creating'):
            validate_remote(repo, p.get('fork_remote', 'fork'), p['fork_owner'] + '/' + p['upstream_repo'].split('/')[-1])
            if git(tree, 'status', '--porcelain') or git(tree, 'rev-parse', 'HEAD') != b['head']:
                raise RuntimeError('validated worktree changed')
            check_approvals(db, cards)
            if p.get('check_pending_prs', True):
                pending_pr_checks(cfg, b, tree, folder)
            ref = 'refs/heads/' + b['branch']
            remote = p.get('fork_remote', 'fork')
            remote_line = external_read(git, repo, 'ls-remote', '--heads', remote, ref)
            remote_head = remote_line.split()[0] if remote_line else None
            if remote_head and remote_head != b['head']:
                raise RuntimeError('fork branch exists with a different head')
            if not remote_head:
                check_approvals(db, cards)
                update(db, number, status='pushing')
                external_read(git, repo, 'push', remote, f"{b['head']}:{ref}")
            update(db, number, status='pushed')
            _, out = external_read(command, repo, 'gh', 'pr', 'list', '--repo', p['upstream_repo'], '--state', 'all', '--head', b['branch'], '--json', 'url,headRefOid,headRepositoryOwner', '--limit', '100')
            prs = [pr for pr in json.loads(out) if pr.get('headRepositoryOwner', {}).get('login', '').lower() == p['fork_owner'].lower()]
            if len(prs) > 1 or (prs and prs[0]['headRefOid'] != b['head']):
                raise RuntimeError('existing PR has unexpected head or duplicate PRs')
            check_approvals(db, cards)
            if prs:
                url = prs[0]['url']
            else:
                update(db, number, status='creating')
                _, url = external_read(command, repo, 'gh', 'pr', 'create', '--repo', p['upstream_repo'], '--base', 'main', '--head', p['fork_owner'] + ':' + b['branch'], '--title', f"Adds {len(cards)} {p.get('set_name', 'Unlimited')} cards (batch {number:03d})", '--body-file', str(folder / 'pr.md'))
            update(db, number, status='published', pr_url=url, detail=None)
    except Exception as e:
        # Push/create can have succeeded even if their client failed. Retain their state for retry.
        state, _ = batch(db, number)
        if isinstance(e, ExternalOperationError):
            update(db, number, detail=str(e))
        else:
            update(db, number, status='blocked', detail=str(e))
        raise



def snapshot(root, db, cfg):
    batches = [dict(row) for row in db.execute('SELECT * FROM publish_batches ORDER BY number')]
    counts = {r['status']: r['n'] for r in db.execute('SELECT status, COUNT(*) AS n FROM publish_batches GROUP BY status')}
    blocked = []
    for row in db.execute("SELECT name,result FROM jobs WHERE status='blocked' ORDER BY name"):
        try:
            result = json.loads(row['result'] or '{}')
            reason = result.get('report', {}).get('reason') or result.get('reason') or result.get('error')
        except (ValueError, AttributeError, TypeError):
            reason = None
        blocked.append(dict(name=row['name'], reason=reason or 'Review blocked; inspect job result.'))
    errors = [b for b in batches if b.get('detail')]
    review.save(Path(root) / 'publication' / 'status.json', dict(
        updated=review.now(), enabled=cfg.get('publisher', {}).get('enabled', False),
        batch_counts=counts, batches=batches, blocked_cards=blocked,
        last_error=errors[-1]['detail'] if errors else None))

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', required=True)
    sub = parser.add_subparsers(dest='cmd', required=True)
    sub.add_parser('plan'); sub.add_parser('status')
    run = sub.add_parser('run'); run.add_argument('--once', action='store_true', required=True)
    sub.add_parser('watch')
    args = parser.parse_args(); root = Path(args.root); cfg = review.config(root)
    if args.cmd in ('plan', 'status'):
        # Read-only mode does not create tables, files or WAL state.
        db = sqlite3.connect(f'file:{root / "queue.sqlite"}?mode=ro', uri=True); db.row_factory = sqlite3.Row
        exists = db.execute("SELECT 1 FROM sqlite_master WHERE type='table' AND name='publish_cards'").fetchone()
        if not exists:
            db.execute('CREATE TEMP TABLE publish_cards(job TEXT,name TEXT)')
        if args.cmd == 'plan':
            print(json.dumps(dict(enabled=cfg.get('publisher', {}).get('enabled', False), eligible=eligible(db), next_batch=selection(db, cfg)), indent=2))
        else:
            batches = [dict(r) for r in db.execute('SELECT * FROM publish_batches ORDER BY number')] if exists else []
            print(json.dumps(dict(enabled=cfg.get('publisher', {}).get('enabled', False), batches=batches, eligible=len(eligible(db))), indent=2))
        return
    if not cfg.get('publisher', {}).get('enabled', False):
        print('publisher disabled'); return
    folder = root / 'publication'; folder.mkdir(parents=True, exist_ok=True)
    with (folder / 'publisher.lock').open('a') as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            print('publisher already running'); return
        while True:
            cfg = review.config(root)
            if not cfg.get('publisher', {}).get('enabled', False):
                with connect(root) as db:
                    snapshot(root, db, cfg)
                return
            with connect(root) as db:
                snapshot(root, db, cfg)
                row = db.execute("SELECT number FROM publish_batches WHERE status NOT IN ('published','blocked') ORDER BY number LIMIT 1").fetchone()
                number = row['number'] if row else reserve(db, cfg)
                if number:
                    try:
                        publish(root, cfg, db, number)
                    except Exception as e:
                        print(f'batch {number:03d}: {e}', file=sys.stderr, flush=True)
                snapshot(root, db, cfg)
            if args.cmd == 'run':
                return
            time.sleep(cfg.get('poll_seconds', 5))


if __name__ == '__main__':
    main()
