#!/usr/bin/env python3
"""Validate locally exported foundation and sibling commits; never create or push publication refs.

This is deliberately an explicit, separately invoked gate. See README.md. Each branch is tested in
one private, sequential checkout with its own Cargo target cache; no existing directory is removed.
"""
import argparse
import datetime as dt
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import time

OID = re.compile(r'^[0-9a-f]{40}$')

def sha(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        for block in iter(lambda: f.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()

def digest(obj):
    return hashlib.sha256(json.dumps(obj, sort_keys=True, separators=(',', ':')).encode()).hexdigest()

def invoke(args, cwd, *, env=None):
    return subprocess.run(args, cwd=cwd, env=env, text=True, stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE, check=True).stdout.strip()

def git(repo, *args):
    return invoke(['git', *args], repo)

def check_oid(value, what):
    if not isinstance(value, str) or not OID.fullmatch(value):
        raise ValueError(f'{what} must be a full SHA-1 commit id')

def paths_between(repo, before, after):
    result = subprocess.run(['git', 'diff', '--no-renames', '--name-only', '-z', before, after],
                            cwd=repo, check=True, stdout=subprocess.PIPE).stdout
    return set(os.fsdecode(x) for x in result.split(b'\0') if x)

def entry(repo, commit, path):
    raw = subprocess.run(['git', 'ls-tree', '-z', commit, '--', path], cwd=repo,
                         check=True, stdout=subprocess.PIPE).stdout
    if not raw:
        return None
    records = [x for x in raw.split(b'\0') if x]
    if len(records) != 1:
        raise ValueError(f'unexpected tree entry for {path}')
    meta, actual = records[0].split(b'\t', 1)
    mode, kind, oid = meta.decode().split()
    if os.fsdecode(actual) != path:
        raise ValueError(f'tree lookup did not return exact path {path}')
    return {'mode': mode, 'kind': kind, 'oid': oid}

def tree_files(repo, commit):
    raw = subprocess.run(['git', 'ls-tree', '-r', '-z', commit], cwd=repo,
                         check=True, stdout=subprocess.PIPE).stdout
    result = {}
    for record in raw.split(b'\0'):
        if not record:
            continue
        meta, path = record.split(b'\t', 1)
        mode, kind, oid = meta.decode().split()
        result[os.fsdecode(path)] = {'mode': mode, 'kind': kind, 'oid': oid}
    return result

def verify_tree(repo, before, after, expected, changes):
    actual = paths_between(repo, before, after)
    if actual != expected:
        raise ValueError(f'{after}: paths differ: missing={sorted(expected-actual)}, extra={sorted(actual-expected)}')
    for path in sorted(expected):
        if entry(repo, after, path) != changes[path]:
            raise ValueError(f'{after}: wrong exported file object: {path}')

def verify_plan(repo, plan, export, expected_base):
    if plan.get('digest') != digest({k: v for k, v in plan.items() if k != 'digest'}):
        raise ValueError('plan digest mismatch')
    if plan.get('blockers'):
        raise ValueError('export plan has blockers')
    if plan.get('base') != expected_base:
        raise ValueError('plan base is not the explicitly requested publication base')
    if export.get('source') != plan.get('head') or export.get('plan_digest') != plan['digest']:
        raise ValueError('export receipt does not bind to this plan and source')
    foundation = export['foundation']; check_oid(foundation, 'foundation')
    check_oid(expected_base, 'expected base'); check_oid(plan['head'], 'source head')
    series = export.get('foundation_commits', [foundation])
    actual_series = git(repo, 'rev-list', '--reverse', expected_base + '..' + foundation).splitlines()
    if not series or series != actual_series or series[-1] != foundation:
        raise ValueError('foundation series differs from the exported ordered history')
    parent = expected_base
    for current in series:
        check_oid(current, 'foundation commit')
        if git(repo, 'show', '-s', '--format=%P', current).split() != [parent]:
            raise ValueError('foundation is not a linear series directly above the publication base')
        if paths_between(repo, parent, current) - set(plan['foundation_paths']):
            raise ValueError('foundation series contains unassigned paths')
        parent = current
    verify_tree(repo, expected_base, foundation, set(plan['foundation_paths']), plan['changes'])
    groups = export['groups']
    if len(groups) != len(plan['groups']):
        raise ValueError('receipt group count differs from plan')
    seen = set(); assigned = set(plan['foundation_paths']); assembled = tree_files(repo, foundation)
    for group, indices in zip(groups, plan['groups']):
        head = group['head']; check_oid(head, 'group head')
        if group['base'] != foundation or group['cards'] != [plan['cards'][i]['name'] for i in indices]:
            raise ValueError(f'wrong sibling base or card list: {head}')
        if not 1 <= len(indices) <= 20:
            raise ValueError(f'group size outside 1..20: {head}')
        if seen.intersection(indices):
            raise ValueError('card appears in multiple groups')
        seen.update(indices)
        current = head
        for index in reversed(indices):
            parent = git(repo, 'rev-parse', current + '^')
            if len(git(repo, 'show', '-s', '--format=%P', current).split()) != 1:
                raise ValueError('group contains a merge commit')
            verify_tree(repo, parent, current, set(plan['cards'][index]['paths']), plan['changes'])
            current = parent
        if current != foundation:
            raise ValueError(f'group {head} does not start at foundation')
        group_paths = {p for i in indices for p in plan['cards'][i]['paths']}
        if assigned.intersection(group_paths):
            raise ValueError(f'file assigned to more than one export group: {sorted(assigned.intersection(group_paths))}')
        assigned.update(group_paths)
        verify_tree(repo, foundation, head, group_paths, plan['changes'])
        for path in group_paths:
            value = entry(repo, head, path)
            if value is None:
                assembled.pop(path, None)
            else:
                assembled[path] = value
    if assembled != tree_files(repo, plan['head']):
        raise ValueError('union of exported file objects differs from frozen source tree')
    if seen != set(range(len(plan['cards']))):
        raise ValueError('groups do not account for every exported card')
    return foundation, groups

def write_json(path, obj):
    tmp = path.with_suffix(path.suffix + '.tmp')
    tmp.write_text(json.dumps(obj, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
    os.replace(tmp, path)

def seed_cache(seed, target, protected):
    """Copy a stopped checked-profile cache without sharing writable files or locks."""
    if seed.is_symlink():
        raise ValueError('seed must not be a symlink')
    seed = seed.resolve(strict=True)
    if not seed.is_dir() or seed.name != 'checked':
        raise ValueError('seed must be an existing checked-profile directory')
    if any(seed == p or seed.is_relative_to(p) or p.is_relative_to(seed) for p in protected):
        raise ValueError('seed must not overlap checkout, target, or results')
    lock_path = seed / '.cargo-lock'
    if not lock_path.is_file() or lock_path.is_symlink():
        raise ValueError('seed needs an existing regular Cargo profile lock')
    # The Cargo lock is held throughout the copy. Reject symlinks, special files and other
    # processes with open seed files/cwd; the caller must keep the source cache stopped.
    with lock_path.open('rb') as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as e:
            raise ValueError('seed cache is locked by an active build') from e
        for path in seed.rglob('*'):
            mode = path.lstat().st_mode
            if not (stat.S_ISREG(mode) or stat.S_ISDIR(mode)):
                raise ValueError(f'unsafe cache entry: {path}')
        for proc in Path('/proc').iterdir():
            if not proc.name.isdigit() or int(proc.name) == os.getpid():
                continue
            for link in [proc / 'cwd', *(proc / 'fd').glob('*')]:
                try:
                    value = Path(os.readlink(link))
                    if value.is_absolute() and (value == seed or value.is_relative_to(seed)):
                        raise ValueError(f'seed cache is in use by process {proc.name}')
                except (FileNotFoundError, ProcessLookupError, PermissionError, NotADirectoryError):
                    continue
        destination = target / 'checked'
        destination.mkdir()
        subprocess.run(['cp', '-a', '--reflink=auto', str(seed) + '/.', str(destination)], check=True)
    return {'path':str(seed), 'copy':'cp -a --reflink=auto into private target/checked',
            'qualification':'source lock held during copy; source must remain stopped; Cargo validates fingerprints'}

def native_inputs(repo, head):
    # Keep every tracked input except the managed client and prose documentation.
    # In this repository native build scripts/tests do not load either subtree.
    entries = {p: obj for p, obj in tree_files(repo, head).items()
               if not p.startswith(('dotnet/', 'docs/')) and p != 'scenarios/README.md'}
    return digest(entries)

def verify_native_evidence(repo, records):
    full = {}
    for record in records:
        fingerprint = native_inputs(repo, record['head'])
        if record.get('native_inputs_sha256') != fingerprint:
            raise ValueError('native input fingerprint differs from tested tree')
        if record.get('gate_kind') == 'compile_unchanged_native':
            parents = git(repo, 'show', '-s', '--format=%P', record['head']).split()
            if (len(parents) != 1 or record.get('unchanged_native_parent') != parents[0]
                or native_inputs(repo, parents[0]) != fingerprint or record['command'][1] != 'check'):
                raise ValueError('compile-only gate changed native inputs')
            continue
        reused = record.get('equivalent_native_tests')
        if reused is None:
            if record.get('gate_kind') != 'full_native_tests' or record['command'][1] != 'test':
                raise ValueError('missing full native test evidence')
            full[record['head']] = record
        else:
            previous = full.get(reused.get('head'))
            if (previous is None or previous.get('exit_code') != 0
                or previous.get('dirty_after') or previous.get('head_after') != previous['head']
                or previous.get('native_inputs_sha256') != fingerprint
                or reused.get('log_sha256') != previous['log_sha256']
                or sha(Path(previous['log'])) != previous['log_sha256']
                or record.get('gate_kind') != 'compile_with_equivalent_native_tests'
                or record['command'][1] != 'check'):
                raise ValueError('native tests were reused across different inputs or without valid evidence')

def run_gate(checkout, target, results, label, head, equivalent=None, *, foundation_step=False):
    if git(checkout, 'rev-parse', 'HEAD') != head:
        raise RuntimeError('checkout head changed')
    if git(checkout, 'status', '--porcelain', '--untracked-files=normal'):
        raise RuntimeError('private checkout is not clean before validation')
    # A full workspace test per independent tree catches runtime scenario/card dependencies as
    # well as compile-time oracle and test references. Cargo uses one private target cache,
    # sequentially, with its normal fingerprint checking; no shared live build directory.
    fingerprint = native_inputs(checkout, head)
    if equivalent is not None and equivalent['native_inputs_sha256'] != fingerprint:
        equivalent = None
    parent = git(checkout, 'rev-parse', head + '^')
    if equivalent is not None and not foundation_step:
        raise ValueError('sibling groups require their own complete native test run')
    unchanged = foundation_step and fingerprint == native_inputs(checkout, parent)
    command = (['cargo', 'check', '--locked', '--offline', '--profile', 'checked', '--workspace', '--all-targets', '--jobs', '4']
               if equivalent or unchanged else ['cargo', 'test', '--locked', '--offline', '--profile', 'checked', '--workspace', '--no-fail-fast', '--jobs', '4'])
    env = os.environ.copy(); env.pop('CANTRIP_CARDS_ONLY', None); env.update({'CARGO_TARGET_DIR':str(target), 'CANTRIP_VERIFY':'1', 'RUST_MIN_STACK':'33554432', 'CARGO_BUILD_JOBS':'4'})
    log = results / (label + '.log')
    started = dt.datetime.now(dt.timezone.utc).isoformat(); start = time.monotonic()
    with log.open('xb') as f:
        proc = subprocess.run(command, cwd=checkout, env=env, stdout=f, stderr=subprocess.STDOUT)
    record = {'label': label, 'head': head, 'command': command, 'target_dir': str(target),
              'gate_kind': ('compile_with_equivalent_native_tests' if equivalent else 'compile_unchanged_native' if unchanged else 'full_native_tests'),
              'native_inputs_sha256': fingerprint,
              'environment':{k:env[k] for k in ('CARGO_TARGET_DIR','CANTRIP_VERIFY','RUST_MIN_STACK','CARGO_BUILD_JOBS')}, 'CANTRIP_CARDS_ONLY':'unset',
              'started_utc': started, 'elapsed_seconds': round(time.monotonic() - start, 3),
              'exit_code': proc.returncode, 'log': str(log), 'log_sha256': sha(log),
              'head_after': git(checkout, 'rev-parse', 'HEAD'),
              'dirty_after': git(checkout, 'status', '--porcelain', '--untracked-files=normal')}
    if equivalent:
        record['equivalent_native_tests'] = {'head': equivalent['head'], 'log_sha256': equivalent['log_sha256']}
    elif unchanged:
        record['unchanged_native_parent'] = parent
    write_json(results / (label + '.json'), record)
    if proc.returncode != 0 or record['head_after'] != head or record['dirty_after']:
        raise RuntimeError(f'{label} failed validation; see {log}')
    return record

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True, help='native Git repository containing local exported commits')
    parser.add_argument('--plan', type=Path, required=True)
    parser.add_argument('--export', type=Path, required=True, help='export_lane build receipt')
    parser.add_argument('--expected-base', required=True, help='full frozen publication base SHA; no branch names')
    parser.add_argument('--worktree', type=Path, required=True, help='new private checkout path; must not exist')
    parser.add_argument('--target-dir', type=Path, required=True, help='new private Cargo cache path; must not exist')
    parser.add_argument('--results', type=Path, required=True, help='new receipt directory; must not exist')
    parser.add_argument('--seed-checked', type=Path, help='stopped checked-profile cache to reflink/copy into new target')
    parser.add_argument('--foundation-only', action='store_true', help='validate foundation once and issue reusable foundation receipt')
    parser.add_argument('--foundation-receipt', type=Path, help='successful foundation-only summary.json')
    parser.add_argument('--shard', help='zero-based shard index/count, for example 0/3')
    args = parser.parse_args(); repo = args.repo.resolve(); plan = json.loads(args.plan.read_text()); export = json.loads(args.export.read_text())
    foundation, groups = verify_plan(repo, plan, export, args.expected_base)
    shard_index = None; shard_count = None; foundation_receipt_hash = None
    if args.shard:
        match = re.fullmatch(r'(\d+)/(\d+)', args.shard)
        if not match or not args.foundation_receipt or args.foundation_only:
            raise ValueError('shard requires index/count and a foundation receipt, and cannot be foundation-only')
        shard_index, shard_count = map(int, match.groups())
        if not 0 <= shard_index < shard_count or shard_count > len(groups):
            raise ValueError('shard must select an existing nonempty subset')
        foundation_receipt_hash = sha(args.foundation_receipt)
        previous = json.loads(args.foundation_receipt.read_text())
        expected = {'status':'passed', 'mode':'foundation', 'plan_sha256':sha(args.plan),
                    'export_sha256':sha(args.export), 'expected_base':args.expected_base,
                    'foundation':foundation, 'validator_sha256':sha(Path(__file__))}
        if any(previous.get(k) != v for k,v in expected.items()) or len(previous.get('records',[])) != len(export.get('foundation_commits', [foundation])):
            raise ValueError('foundation receipt is stale or does not bind this exact plan, exporter, base and validator')
        for current, record in zip(export.get('foundation_commits', [foundation]), previous['records']):
            log = Path(record['log'])
            if (record['head'] != current or record['head_after'] != current
                or record['exit_code'] != 0 or record['dirty_after'] or sha(log) != record['log_sha256']):
                raise ValueError('foundation gate receipt or log failed integrity check')
        verify_native_evidence(repo, previous['records'])
    elif args.foundation_receipt:
        raise ValueError('foundation receipt requires --shard')
    worktree = args.worktree.absolute(); target = args.target_dir.absolute(); results = args.results.absolute()
    if len({worktree, target, results}) != 3 or any(p.exists() for p in (worktree, target, results)):
        raise ValueError('worktree, target and results must be distinct new paths')
    paths = [p.resolve() for p in (worktree, target, results)]
    if any(a.is_relative_to(b) or b.is_relative_to(a) for i,a in enumerate(paths) for b in paths[i+1:]):
        raise ValueError('private checkout, target and receipts must not contain one another')
    results.mkdir(parents=True); target.mkdir(parents=True)
    with (results / 'run.lock').open('x') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        summary = {'status':'running','mode': ('shard' if args.shard else 'foundation' if args.foundation_only else 'full'),
                   'validator_sha256':sha(Path(__file__)), 'shard_index':shard_index, 'shard_count':shard_count,
                   'foundation_receipt_sha256':foundation_receipt_hash,'plan_sha256':sha(args.plan),'export_sha256':sha(args.export),
                   'plan_digest':plan['digest'],'source':plan['head'],'expected_base':args.expected_base,
                   'foundation':foundation,'group_count':len(groups),'worktree':str(worktree),
                   'target_dir':str(target),'requested_seed':str(args.seed_checked.absolute()) if args.seed_checked else None,
                   'gate_environment':{'CANTRIP_VERIFY':'1','RUST_MIN_STACK':'33554432','CARGO_BUILD_JOBS':'4','CANTRIP_CARDS_ONLY':'unset'},'records':[]}
        write_json(results / 'summary.json', summary)
        try:
            if args.seed_checked:
                summary['cache_seed'] = seed_cache(args.seed_checked, target, [worktree.resolve(), target.resolve(), results.resolve()])
                write_json(results / 'summary.json', summary)
            invoke(['git', 'worktree', 'add', '--detach', str(worktree), foundation], repo)
            if not args.shard:
                series = export.get('foundation_commits', [foundation])
                for number, current in enumerate(series, 1):
                    invoke(['git', 'switch', '--detach', current], worktree)
                    label = 'foundation' if current == foundation else f'foundation-{number:02d}'
                    fingerprint = native_inputs(worktree, current)
                    equivalent = next((r for r in summary['records'] if r['gate_kind'] == 'full_native_tests'
                                       and r['native_inputs_sha256'] == fingerprint), None)
                    summary['records'].append(run_gate(worktree, target, results, label, current, equivalent, foundation_step=True))
                    write_json(results / 'summary.json', summary)
            selected = [] if args.foundation_only else [(i, g) for i,g in enumerate(groups, 1) if not args.shard or (i-1) % shard_count == shard_index]
            for i, group in selected:
                if git(worktree, 'status', '--porcelain', '--untracked-files=normal'):
                    raise RuntimeError('private checkout has uncommitted or untracked files')
                invoke(['git', 'switch', '--detach', group['head']], worktree)
                summary['records'].append(run_gate(worktree, target, results, f'group-{i:03d}', group['head']))
                write_json(results / 'summary.json', summary)
            summary['status'] = 'passed'
        except BaseException as exc:
            summary['status'] = 'failed'; summary['error'] = f'{type(exc).__name__}: {exc}'
            write_json(results / 'summary.json', summary)
            raise
        write_json(results / 'summary.json', summary)
    print(f"Validated {len(summary['records'])} heads in {summary['mode']} mode; receipts: {results}")

if __name__ == '__main__':
    try:
        main()
    except Exception as exc:
        print(f'validation stopped: {exc}', file=sys.stderr)
        sys.exit(1)
