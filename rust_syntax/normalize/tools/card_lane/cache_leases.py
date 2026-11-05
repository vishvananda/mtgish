"""Per-user kernel leases shared by every campaign that uses a Cargo cache."""
from contextlib import contextmanager
import atexit
import fcntl
import hashlib
from pathlib import Path

LEASE_DIR = Path.home() / '.cache' / 'card-lane' / 'cache-leases'
_held = {}


def close_all():
    for lock, _ in _held.values():
        lock.close()
    _held.clear()


atexit.register(close_all)


def lease_file(target):
    LEASE_DIR.mkdir(parents=True, exist_ok=True)
    key = hashlib.sha256(str(Path(target).resolve()).encode()).hexdigest()
    return (LEASE_DIR / (key + '.lock')).open('a+b')


def acquire(target):
    """Hold a shared lease before cache use, including the gap before Cargo starts."""
    target = Path(target).resolve()
    if target in _held:
        _held[target][1] += 1
        return
    lock = lease_file(target)
    try:
        fcntl.flock(lock, fcntl.LOCK_SH)
    except BaseException:
        lock.close()
        raise
    _held[target] = [lock, 1]


def release(target):
    target = Path(target).resolve()
    if target not in _held:
        return
    lock, count = _held[target]
    if count > 1:
        _held[target][1] -= 1
    else:
        del _held[target]
        lock.close()


@contextmanager
def removal(target):
    """Exclusive removal fails while any campaign or concurrent owner holds a shared lease."""
    lock = lease_file(target)
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        yield
    finally:
        lock.close()
