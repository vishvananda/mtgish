#!/usr/bin/env python3
"""Reap retired Cargo caches without removing worktrees, source, or evidence."""
import argparse
from contextlib import ExitStack
import fcntl
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import cache_leases


def read(path, default=None):
    return json.loads(Path(path).read_text()) if Path(path).exists() else default


def save(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile('w', dir=path.parent, delete=False) as f:
        json.dump(value, f, indent=2)
        f.write('\n')
    os.replace(f.name, path)


def roots(cfg):
    policy = cfg.get('cache_cleanup', {})
    configured = policy.get('worktree_roots')
    if configured is None:
        configured = [cfg.get('publisher', {}).get('worktree_root')]
    if not isinstance(configured, list) or any(p is not None and not isinstance(p, str) for p in configured):
        raise ValueError('cache_cleanup.worktree_roots must be a list of directory paths')
    result = set()
    for value in configured:
        if not value:
            continue
        path = Path(value).expanduser()
        if not path.is_absolute() or any(p.is_symlink() for p in (path, *path.parents)):
            raise ValueError('cleanup roots must be absolute paths without symlink components')
        result.add(path)
    return sorted(result)


def protected(cfg, root=None):
    paths = [Path(p).expanduser().resolve() for p in cfg.get('cache_cleanup', {}).get('protected_paths', [])]
    for section in ('reviewers', 'implementers'):
        for worker in cfg.get(section, {}).values():
            if worker.get('tree'):
                paths.append((Path(worker['tree']).expanduser() / 'target').resolve())
            if worker.get('target_dir'):
                paths.append(Path(worker['target_dir']).expanduser().resolve())
    for section in ('integration', 'publisher'):
        if cfg.get(section, {}).get('tree'):
            paths.append((Path(cfg[section]['tree']).expanduser() / 'target').resolve())
    # Campaigns sharing a worktree pool must retain one another's prepared slots, even while idle.
    if root is not None and (Path(root) / 'config.json').is_file():
        for config in Path(root).parent.glob('*/config.json'):
            if config.resolve() == (Path(root) / 'config.json').resolve():
                continue
            try:
                other = read(config, {})
                if isinstance(other, dict) and other.get('enabled', True):
                    paths.extend(protected(other))
            except (OSError, ValueError, TypeError) as error:
                raise ValueError(f'cannot establish sibling campaign protections: {config}') from error
    return paths


def overlaps(a, b):
    return a == b or a.is_relative_to(b) or b.is_relative_to(a)


def cache_locks(path):
    return sorted(set(path.glob('*/.cargo-lock')) | set(path.glob('*/*/.cargo-lock')))


def is_cache(path):
    if path.is_symlink() or not path.is_dir():
        return False
    if any((path / name).exists() for name in ('.git', 'Cargo.toml', 'queue.sqlite', 'config.json')):
        return False
    marker = path / 'CACHEDIR.TAG'
    info = path / '.rustc_info.json'
    if not ((marker.is_file() and not marker.is_symlink()) or (info.is_file() and not info.is_symlink())):
        return False
    locks = cache_locks(path)
    return bool(locks) and all(p.is_file() and not p.is_symlink() for p in locks)


def candidates(cfg):
    result = set()
    for parent in roots(cfg):
        if not parent.is_dir():
            continue
        for child in parent.iterdir():
            if child.is_symlink() or not child.is_dir():
                continue
            for path in (child / 'target', child):
                if is_cache(path):
                    result.add(path)
    return sorted(result)


def observed_users(paths):
    busy = set()
    for proc in Path('/proc').iterdir():
        if not proc.name.isdigit():
            continue
        for link in [proc / 'cwd', proc / 'exe', *(proc / 'fd').glob('*')]:
            try:
                target = Path(os.readlink(link).removesuffix(' (deleted)'))
            except (FileNotFoundError, ProcessLookupError, PermissionError, NotADirectoryError):
                continue
            if int(proc.name) == os.getpid() and link.parent.name == 'fd' and target.name == '.cargo-lock':
                continue  # These are the locks this maintenance process holds during removal.
            if target.is_absolute():
                busy.update(p for p in paths if target == p or target.is_relative_to(p))
    return busy


def newest_write(path):
    # cp -a preserves mtime on a fresh CoW clone; ctime still records its recent creation.
    s = path.stat()
    latest = max(s.st_mtime, s.st_ctime)
    for folder, dirs, files in os.walk(path, followlinks=False):
        for name in dirs + files:
            s = (Path(folder) / name).stat(follow_symlinks=False)
            latest = max(latest, s.st_mtime, s.st_ctime)
    return latest


def retire(root, target):
    """A completed temporary publisher/validation cache may be reaped immediately."""
    root, target = Path(root), Path(target).absolute()
    folder = root / 'cache-cleanup'
    folder.mkdir(parents=True, exist_ok=True)
    # A different campaign may already be reusing this target. Retirement, like
    # removal, requires exclusive ownership; a later reuse clears this record.
    try:
        with cache_leases.removal(target):
            with (folder / 'registry.lock').open('a') as lock:
                fcntl.flock(lock, fcntl.LOCK_EX)
                registry = read(folder / 'retired.json', {})
                registry[str(target)] = time.time()
                save(folder / 'retired.json', registry)
        return True
    except BlockingIOError:
        return False


def tracked(path):
    """Check every candidate shape, including external caches inside a repository."""
    repo = subprocess.run(['git', 'rev-parse', '--show-toplevel'], cwd=path, capture_output=True, text=True)
    if repo.returncode:
        if 'not a git repository' in repo.stderr:
            return False
        raise OSError('cannot establish Git ownership of ' + str(path))
    top = Path(repo.stdout.strip())
    return bool(subprocess.run(['git', 'ls-files', '--', str(path.relative_to(top))], cwd=top,
                               check=True, capture_output=True).stdout)


def reuse(root, target):
    """Explicitly cancel retirement and record use even when Cargo only reads a warm cache."""
    cache_leases.acquire(target)
    record_reuse(root, target)


def record_reuse(root, target):
    """Update cache bookkeeping after the caller has established its required kernel lease."""
    root, target = Path(root), Path(target).absolute()
    folder = root / 'cache-cleanup'
    folder.mkdir(parents=True, exist_ok=True)
    with (folder / 'registry.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        registry = read(folder / 'retired.json', {})
        registry.pop(str(target), None)
        save(folder / 'retired.json', registry)
        if is_cache(target) and not tracked(target):
            (target / '.card-lane-last-used').touch()


def release(root, target):
    """End this process's temporary lease, including a failed publisher attempt."""
    cache_leases.release(target)


def sweep(root, cfg=None, *, dry_run=False, now=None):
    root = Path(root)
    cfg = cfg if cfg is not None else read(root / 'config.json', {})
    policy = cfg.get('cache_cleanup', {})
    if not policy.get('enabled', True):
        return {'status': 'disabled', 'removed': []}
    hours = float(policy.get('idle_hours', 6))
    if hours < 0:
        raise ValueError('cleanup idle_hours must be nonnegative')
    now = time.time() if now is None else now
    folder = root / 'cache-cleanup'
    folder.mkdir(parents=True, exist_ok=True)
    report = {'status': 'running', 'started': now, 'dry_run': dry_run, 'removed': [], 'eligible': [], 'skipped': {}, 'errors': {}}
    with (folder / 'run.lock').open('a') as running:
        try:
            fcntl.flock(running, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            return {'status': 'already_running', 'removed': []}
        try:
            paths = candidates(cfg)
            busy, keep = observed_users(paths), protected(cfg, root)
        except (OSError, ValueError, TypeError) as error:
            report.update(status='blocked', finished=time.time())
            report['errors']['configuration'] = str(error)
            save(folder / ('dry-run.json' if dry_run else 'latest.json'), report)
            return report
        space = {str(p): shutil.disk_usage(p).free for p in roots(cfg) if p.is_dir()}
        for path in paths:
            name, locks = str(path), []
            lease = ExitStack()
            try:
                if any(overlaps(path.resolve(), p) for p in keep):
                    report['skipped'][name] = 'configured worker or protected seed'
                    continue
                if path in busy:
                    report['skipped'][name] = 'process has cache open'
                    continue
                try:
                    lease.enter_context(cache_leases.removal(path))
                except BlockingIOError:
                    report['skipped'][name] = 'active cache lease'
                    continue
                # Hold both Linux Cargo lock conventions while inspecting and deleting.
                for lockpath in cache_locks(path):
                    lock = lockpath.open('r+b')
                    locks.append(lock)
                    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                    fcntl.lockf(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                # Reuse and the final deletion decision are serialized. A lease covers the gap
                # between reuse and Cargo opening its own lock/files, even for a read-only cache hit.
                with (folder / 'registry.lock').open('a') as registry_lock:
                    fcntl.flock(registry_lock, fcntl.LOCK_EX)
                    retired = read(folder / 'retired.json', {})
                    latest = newest_write(path)
                    explicitly_retired = name in retired and latest <= retired[name]
                    if not explicitly_retired and now - latest < hours * 3600:
                        report['skipped'][name] = 'recently used'
                        continue
                    if tracked(path):
                        report['skipped'][name] = 'contains tracked files'
                        continue
                    if path in observed_users([path]):
                        report['skipped'][name] = 'process started using cache'
                        continue
                    report['eligible'].append(name)
                    if not dry_run:
                        shutil.rmtree(path)
                        report['removed'].append(name)
            except BlockingIOError:
                report['skipped'][name] = 'Cargo cache is locked'
            except (OSError, ValueError, subprocess.CalledProcessError) as e:
                report['errors'][name] = str(e)
            finally:
                for lock in locks:
                    lock.close()
                lease.close()
        report.update(status='finished', finished=time.time(),
                      free_before=space,
                      free_after={str(p): shutil.disk_usage(p).free for p in roots(cfg) if p.is_dir()})
        save(folder / ('dry-run.json' if dry_run else 'latest.json'), report)
    return report


def maybe_start(root, cfg, *, force=False):
    """Launch a token-free maintenance job; never block the card supervisor on a scan."""
    if not cfg.get('cache_cleanup', {}).get('enabled', True) or not roots(cfg):
        return False
    root = Path(root)
    folder = root / 'cache-cleanup'
    folder.mkdir(parents=True, exist_ok=True)
    with (folder / 'launch.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        previous = read(folder / 'launch.json', {})
        interval = max(60, cfg.get('cache_cleanup', {}).get('interval_seconds', 3600))
        if not force and time.time() - previous.get('started', 0) < interval:
            return False
        with (folder / 'worker.log').open('ab') as output:
            proc = subprocess.Popen([sys.executable, str(Path(__file__).resolve()), '--root', str(root), 'sweep'],
                                    stdin=subprocess.DEVNULL, stdout=output, stderr=subprocess.STDOUT,
                                    close_fds=True, start_new_session=True)
        save(folder / 'launch.json', {'started': time.time(), 'pid': proc.pid})
    return True


def after_lane_work(root, cfg, *, retired_target=None):
    """Maintenance failure never changes a card's approval or a published PR's state."""
    try:
        if retired_target is not None:
            retire(root, retired_target)
        return maybe_start(root, cfg, force=retired_target is not None)
    except Exception as error:
        print(f'Card-lane cache cleanup could not start: {error}', file=sys.stderr)
        return False


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, required=True)
    commands = parser.add_subparsers(dest='command', required=True)
    p = commands.add_parser('sweep')
    p.add_argument('--dry-run', action='store_true')
    p = commands.add_parser('retire')
    p.add_argument('target', type=Path)
    p = commands.add_parser('reuse')
    p.add_argument('target', type=Path)
    args = parser.parse_args()
    if args.command == 'retire':
        retire(args.root, args.target)
    elif args.command == 'reuse':
        reuse(args.root, args.target)
    else:
        print(json.dumps(sweep(args.root, dry_run=args.dry_run), indent=2))


if __name__ == '__main__':
    main()
