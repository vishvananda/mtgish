#!/usr/bin/env python3
"""Plan and reconstruct independent card groups from a finished local tree.

Never replay isolated prerequisite patches. Never push or open a PR. The source
commit, approval receipts, exact file objects and authorship are frozen in a
manifest; building requires the final validated snapshot and an empty work queue.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import sqlite3
import tempfile

import integration_lane as integration
import publish_lane as publication
import review_lane as review


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def files(tree, head):
    result = {}
    # Paths are parsed at NUL boundaries; no shell interpolation or checkout.
    _, output = publication.command(tree, 'git', 'ls-tree', '-rz', head)
    for record in output.split('\0'):
        if not record:
            continue
        metadata, path = record.split('\t', 1)
        mode, kind, oid = metadata.split()
        result[path] = dict(mode=mode, kind=kind, oid=oid)
    return result


def latest(db, table):
    return [dict(row) for row in db.execute(f'''SELECT * FROM {table}
        WHERE rowid IN (SELECT MAX(rowid) FROM {table} GROUP BY name) ORDER BY name''')]


def scenario_cards(text):
    """The concrete deck/sideboard names, not prose or action captions."""
    reading = False
    for line in text.splitlines():
        line = line.strip()
        if line.startswith('['):
            reading = bool(re.fullmatch(r'\[[^]]+ (?:Deck|Sideboard)\]', line))
        elif reading and line and not line.startswith('#'):
            yield review.canonical_typography(re.sub(r'\s+x\d+$', '', line))


def owns(card, path):
    own, slug = card['stem'], card['slug']
    return (path in (f'content/mtg/cards/{slug}.json', f'crates/cantrip-cards/src/cards/{own}.rs')
            or path == f'scenarios/{slug}.cantrip' or path.startswith(f'scenarios/{slug}-')
            or path in card['extra_tests']
            or any(path == f'crates/{package}/tests/{own}.rs'
                   or path.startswith(f'crates/{package}/tests/{own}_')
                   for package in ('cantrip-cards', 'cantrip-mechanics', 'cantrip-script'))
            or path == f'crates/cantrip-cards/src/cards/tests/{own}.rs')


def plan(root, head, *, defer_implementations=()):
    root = Path(root).resolve()
    cfg, settings, tree = integration.settings(root)
    head = publication.git(tree, 'rev-parse', head + '^{commit}')
    base = publication.git(tree, 'rev-parse', settings['base'] + '^{commit}')
    publication.git(tree, 'merge-base', '--is-ancestor', base, head)
    publication.git(tree, 'merge-base', '--is-ancestor', head, settings['branch'])
    before, after = files(tree, base), files(tree, head)
    changed = sorted(path for path in before.keys() | after.keys() if before.get(path) != after.get(path))
    blockers, cards, foundation_cards = [], [], []
    with sqlite3.connect((root/'queue.sqlite').as_uri() + '?mode=ro', uri=True) as db:
        db.row_factory = sqlite3.Row
        jobs = latest(db, 'jobs')
        imports = {row['name']: row for row in latest(db, 'integration_jobs')}
        tables = {row[0] for row in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
        implementations = latest(db, 'implementation_jobs') if 'implementation_jobs' in tables else []
        if db.execute("SELECT 1 FROM jobs WHERE status IN ('queued','running') LIMIT 1").fetchone():
            blockers.append('Review or verification work is still active')
    approved = {row['name'] for row in jobs if row['status'] == 'passed'}
    deferred_names = set(defer_implementations)
    unfinished = {row['name']: row for row in implementations
                  if row['name'] not in approved and imports.get(row['name'], {}).get('status') != 'upstream'}
    unknown = deferred_names - unfinished.keys()
    if unknown:
        raise ValueError('Only unfinished implementations may be deferred: ' + ', '.join(sorted(unknown)))
    deferred = [dict(name=name, id=unfinished[name]['id'], status=unfinished[name]['status'])
                for name in sorted(deferred_names)]
    for row in implementations:
        if row['name'] in unfinished and row['name'] not in deferred_names:
            blockers.append(f"Unfinished implementation: {row['name']} ({row['status']})")
    for row in jobs:
        if row['status'] != 'passed':
            if row['status'] != 'superseded':
                blockers.append(f"Unfinished review: {row['name']} ({row['status']})")
            continue
        imported = imports.get(row['name'])
        if imported and imported['status'] == 'upstream':
            continue
        if not imported or imported['status'] not in ('assembled', 'integrated') or imported['candidate'] != row['candidate']:
            blockers.append(f"Approval has not been collected: {row['name']}")
            continue
        if publication.command(tree, 'git', 'merge-base', '--is-ancestor', imported['last_head'], head, check=False)[0]:
            blockers.append(f"Approval was collected after the source checkpoint: {row['name']}")
            continue
        source = json.loads(row['source'])
        issue = source.get('issue')
        if not isinstance(issue, int) or isinstance(issue, bool) or issue < 1:
            raise ValueError('Invalid issue for ' + row['name'])
        author = publication.git(tree, 'show', '-s', '--format=%an%n%ae%n%aI', row['candidate']).splitlines()
        stem, slug = review.names(row['name'])
        card = dict(name=row['name'], issue=issue, job=row['id'], candidate=row['candidate'],
                    integration_job=imported['id'], author=author,
                    stem=stem, slug=slug, extra_tests=source.get('card_test_paths', []), paths=[])
        # Already published cards receive their approved reconciliation changes
        # in the common foundation. Every sibling then gets their final behavior
        # and API compatibility, rather than depending on a separate card PR.
        destination = foundation_cards if f'content/mtg/cards/{slug}.json' in before else cards
        destination.append(card)
    cards.sort(key=lambda card: (card['issue'], card['name']))
    foundation_cards.sort(key=lambda card: (card['issue'], card['name']))
    by_name = {review.canonical_typography(card['name']): index for index, card in enumerate(cards)}
    parents = list(range(len(cards)))

    def find(index):
        while parents[index] != index:
            index = parents[index]
        return index

    def join(indices):
        if indices:
            first = find(min(indices))
            for index in indices:
                parents[find(index)] = first

    foundation = []
    all_cards = cards + foundation_cards
    deferred_cards = []
    for name in sorted(deferred_names):
        stem, slug = review.names(name)
        extra_tests = json.loads(unfinished[name].get('card', '{}')).get('card_test_paths', [])
        deferred_cards.append(dict(name=name, stem=stem, slug=slug, extra_tests=extra_tests))
    for path in changed:
        # An explicit backlog deferral never permits its unreviewed file objects
        # into the foundation. Compare longest owners to avoid prefix collisions
        # such as Desert and Desert Twister.
        deferred_owners = [owner for owner in deferred_cards if owns(owner, path)]
        owners = [index for index, card in enumerate(all_cards) if owns(card, path)]
        if deferred_owners:
            key = 'slug' if path.startswith('scenarios/') else 'stem'
            longest_approved = max((len(all_cards[index][key]) for index in owners), default=0)
            for owner in deferred_owners:
                if path in owner['extra_tests'] or len(owner[key]) >= longest_approved:
                    blockers.append(f"Deferred implementation has changes in finished source: {owner['name']}: {path}")
        if len(owners) > 1:
            # Desert and Desert Twister, for example, have overlapping scenario
            # prefixes. The complete longest name owns its own scenarios/tests.
            key = 'slug' if path.startswith('scenarios/') else 'stem'
            longest = max(len(all_cards[index][key]) for index in owners)
            owners = [index for index in owners if len(all_cards[index][key]) == longest]
        if len(owners) > 1:
            raise ValueError('Ambiguous file ownership: ' + path)
        required = set()
        if path.startswith('scenarios/') and path.endswith('.cantrip') and path in after:
            text = publication.git(tree, 'show', head + ':' + path)
            required = {by_name[name] for name in scenario_cards(text) if name in by_name}
        reconciled = all_cards[owners[0]] if owners and owners[0] >= len(cards) else None
        if owners and owners[0] < len(cards):
            owner = owners[0]
        elif required:
            owner = min(required)
        else:
            owner = None
        if owner is None:
            foundation.append(path)
            if reconciled is not None:
                reconciled['paths'].append(path)
            if path not in before and (path.startswith('content/') and path.endswith('.json')
                                      or path.startswith('crates/cantrip-cards/src/cards/') and path.endswith('.rs')):
                if reconciled is None and path not in settings.get('foundation_new_card_paths', []):
                    blockers.append('Unassigned new card or token needs explicit foundation ownership: ' + path)
        else:
            cards[owner]['paths'].append(path)
            join(required | {owner})
    components = {}
    for index in range(len(cards)):
        components.setdefault(find(index), []).append(index)
    size = int(settings.get('card_group_size', 20))
    if size < 1:
        raise ValueError('card_group_size must be positive')
    groups = []
    for component in sorted(components.values(), key=lambda indices: (-len(indices), min(indices))):
        if len(component) > size:
            blockers.append('Cross-card scenarios exceed group size: ' + ', '.join(cards[i]['name'] for i in component))
        group = next((group for group in groups if len(group) + len(component) <= size), None)
        if group is None:
            group = []
            groups.append(group)
        group.extend(component)
    groups = sorted((sorted(group) for group in groups), key=min)
    snapshot_path = cfg.get('local_base_manifest')
    snapshot = review.read(snapshot_path) if snapshot_path else {}
    if snapshot.get('head') != head or snapshot.get('validation', {}).get('ok') is not True:
        blockers.append('Finished source head is not the validated local checkpoint')
    queue_path = cfg.get('implementation_queue')
    if queue_path and any(card.get('status') == 'pending' for card in review.read(queue_path)['cards']):
        blockers.append('Implementation intake queue is not empty')
    committer = publication.git(tree, 'show', '-s', '--format=%cn%n%ce', head).splitlines()
    result = dict(version=2, root=str(root.resolve()), tree=str(tree), base=base, head=head, committer=committer,
                  snapshot=snapshot, cards=cards, foundation_cards=foundation_cards, foundation_paths=foundation, groups=groups,
                  changes={path: after.get(path) for path in changed}, blockers=sorted(set(blockers)))
    if deferred:
        result['deferred_implementations'] = deferred
    result['digest'] = digest(result)
    return result


def commit(tree, parent, paths, objects, message, author, timestamp, committer):
    with tempfile.TemporaryDirectory(prefix='cantrip-export-') as folder:
        env = dict(os.environ, GIT_INDEX_FILE=str(Path(folder)/'index'), GIT_COMMITTER_DATE=timestamp,
                   GIT_COMMITTER_NAME=committer[0], GIT_COMMITTER_EMAIL=committer[1],
                   GIT_AUTHOR_NAME=author[0], GIT_AUTHOR_EMAIL=author[1], GIT_AUTHOR_DATE=author[2])
        publication.git(tree, 'read-tree', parent, env=env)
        for path in paths:
            obj = objects[path]
            if obj is None:
                publication.git(tree, 'update-index', '--force-remove', '--', path, env=env)
            else:
                if obj['kind'] != 'blob':
                    raise ValueError('Only file objects may be exported: ' + path)
                publication.git(tree, 'update-index', '--add', '--cacheinfo', obj['mode'], obj['oid'], path, env=env)
        output = publication.git(tree, 'write-tree', env=env)
        return publication.git(tree, 'commit-tree', output, '-p', parent, env=env, input=message+'\n')


def build(root, manifest, prefix):
    if manifest.get('digest') != digest({key: value for key, value in manifest.items() if key != 'digest'}):
        raise ValueError('Export manifest digest does not match')
    fresh = plan(root, manifest['head'],
                 defer_implementations=[row['name'] for row in manifest.get('deferred_implementations', [])])
    if fresh != manifest:
        raise ValueError('Source or queue receipts changed; create a fresh export plan')
    if manifest['blockers']:
        raise ValueError('Export is held: ' + '; '.join(manifest['blockers']))
    tree = Path(manifest['tree'])
    publication.git(tree, 'check-ref-format', '--branch', prefix+'/foundation')
    timestamp = publication.git(tree, 'show', '-s', '--format=%cI', manifest['head'])
    author = publication.git(tree, 'show', '-s', '--format=%an%n%ae%n%aI', manifest['head']).splitlines()
    # An API change and its existing callers must remain atomic. Keep each
    # original review/candidate/author in foundation_cards and credit authors
    # in the combined commit instead of creating unbuildable intermediate
    # commits or closing already-published cards' issues again.
    coauthors = sorted({tuple(card['author'][:2]) for card in manifest['foundation_cards'] if card['paths']}
                       - {tuple(author[:2])})
    message = 'Engine and mechanics: shared early-set foundation\n\nConsolidated source: '+manifest['head']
    if coauthors:
        message += '\n\n' + '\n'.join(f'Co-authored-by: {name} <{email}>' for name, email in coauthors)
    foundation = commit(tree, manifest['base'], manifest['foundation_paths'], manifest['changes'],
                        message, author, timestamp, manifest['committer'])
    refs = {prefix+'/foundation': foundation}
    branches = []
    for number, indices in enumerate(manifest['groups'], 1):
        parent = foundation
        for index in indices:
            card = manifest['cards'][index]
            message = (f"Card: {card['name']}\n\nCloses #{card['issue']}\n\n"
                       f"Reviewed candidate: {card['candidate']}\nConsolidated source: {manifest['head']}")
            parent = commit(tree, parent, card['paths'], manifest['changes'], message, card['author'], timestamp, manifest['committer'])
        branch = f'{prefix}/cards-{number:03d}'
        refs[branch] = parent
        branches.append(dict(branch=branch, head=parent, base=foundation,
                             cards=[manifest['cards'][index]['name'] for index in indices]))
    # Verify the union before creating any refs. Sibling groups must change only
    # their assigned paths relative to the same foundation.
    foundation_files = files(tree, foundation)
    assembled = dict(foundation_files)
    for group, indices in zip(branches, manifest['groups']):
        expected = {path for index in indices for path in manifest['cards'][index]['paths']}
        actual = files(tree, group['head'])
        difference = {path for path in foundation_files.keys() | actual.keys()
                      if foundation_files.get(path) != actual.get(path)}
        if difference != expected:
            raise ValueError('Sibling export contains unexpected paths: '+group['branch'])
        for path in expected:
            if path in actual:
                assembled[path] = actual[path]
            else:
                assembled.pop(path, None)
    if assembled != files(tree, manifest['head']):
        raise ValueError('Exported foundation and card union differ from finished source')
    # Atomic local-only ref creation; an existing different branch is retained.
    commands = ['start']
    for branch, sha in refs.items():
        code, old = publication.command(tree, 'git', 'rev-parse', '--verify', 'refs/heads/'+branch, check=False)
        if code == 0:
            if old != sha:
                raise ValueError('Existing export branch differs: '+branch)
        else:
            commands.append(f'create refs/heads/{branch} {sha}')
    commands.extend(['prepare', 'commit'])
    publication.git(tree, 'update-ref', '--stdin', input='\n'.join(commands)+'\n')
    return dict(source=manifest['head'], plan_digest=manifest['digest'], foundation=foundation,
                foundation_branch=prefix+'/foundation', groups=branches, remote_writes=False,
                validation_required=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', required=True, type=Path)
    commands = parser.add_subparsers(dest='action', required=True)
    draft = commands.add_parser('plan')
    draft.add_argument('--head', required=True)
    draft.add_argument('--out', required=True, type=Path)
    draft.add_argument('--defer-implementations', type=Path,
                       help='JSON list of unfinished card names to retain in the backlog; their changed files block export')
    export = commands.add_parser('build')
    export.add_argument('--plan', required=True, type=Path)
    export.add_argument('--prefix', required=True)
    export.add_argument('--out', required=True, type=Path)
    args = parser.parse_args()
    if args.action == 'plan':
        deferred = review.read(args.defer_implementations) if args.defer_implementations else []
        if not isinstance(deferred, list) or any(not isinstance(name, str) for name in deferred):
            raise ValueError('Deferred implementations must be a JSON list of card names')
        result = plan(args.root, args.head, defer_implementations=deferred)
    else:
        result = build(args.root, review.read(args.plan), args.prefix)
    review.save(args.out, result)
    print(json.dumps({key: result[key] for key in ('head', 'digest', 'blockers') if key in result}, indent=2))


if __name__ == '__main__':
    main()
