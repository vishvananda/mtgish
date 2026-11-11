"""Retry only idempotent queue initialization, never a card transaction."""
import sqlite3
import sys
import time


def retryable(error):
    code = getattr(error, 'sqlite_errorcode', None)
    if code is not None:
        return code & 255 in {sqlite3.SQLITE_IOERR, sqlite3.SQLITE_BUSY, sqlite3.SQLITE_LOCKED,
                              sqlite3.SQLITE_FULL, sqlite3.SQLITE_CANTOPEN}
    return str(error) in {'disk I/O error', 'database is locked', 'database table is locked',
                          'database or disk is full', 'unable to open database file'}


def open_database(path, factory, initialize, attempts=3):
    for attempt in range(attempts):
        db = None
        try:
            db = sqlite3.connect(path, timeout=60, factory=factory)
            initialize(db)
            return db
        except sqlite3.Error as error:
            if db is not None:
                db.close()
            if not isinstance(error, sqlite3.OperationalError) or not retryable(error) or attempt+1 == attempts:
                raise
            delay = 2**attempt
            print(f'Queue storage unavailable; retrying initialization in {delay}s: {error}', file=sys.stderr)
            time.sleep(delay)
        except BaseException:
            if db is not None:
                db.close()
            raise
