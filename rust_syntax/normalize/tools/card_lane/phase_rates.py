"""Observed phase throughput over a common rolling wall-clock window."""
import datetime as dt
import json
import re


def timestamp(value):
    try:
        parsed = dt.datetime.fromisoformat(value)
        return parsed.replace(tzinfo=dt.timezone.utc).timestamp() if parsed.tzinfo is None else parsed.timestamp()
    except (TypeError, ValueError):
        return None


def phase(detail, fallback='review'):
    """Historical branches preserve the phase even after a job changes stages."""
    leaf = detail.get('branch', '').rsplit('/', 1)[-1]
    match = re.match(r'^(review|confirm)(?:-round-(\d+))?$', leaf)
    stage = detail.get('stage') or (match[1] if match else fallback)
    if stage == 'confirm':
        return 'verify'
    number = detail.get('review_round', int(match[2] or 0) if match else 0)
    return 'fix' if number else 'review'


def summary(db, cfg, at, window_seconds=1800):
    end = timestamp(at)
    if end is None or window_seconds <= 0:
        raise ValueError('phase rates require a valid time and positive window')
    start = end - window_seconds
    records = {name: [] for name in ('implementation', 'review', 'fix', 'verify', 'review_pool')}
    active = {name: [] for name in records}
    queued = dict.fromkeys(records, 0)
    seen = set()

    for row in db.execute('SELECT * FROM jobs'):
        source = json.loads(row['source'])
        current = phase(dict(stage=row['stage'], review_round=source.get('review_round', 0)))
        if row['status'] == 'queued':
            queued[current] += 1
        elif row['status'] == 'running':
            active[current].append((row['worker'], timestamp(row['started']), end))
    for row in db.execute("SELECT * FROM events WHERE kind IN ('passed','queued','blocked','infrastructure') ORDER BY id"):
        finished = timestamp(row['at'])
        detail = json.loads(row['detail'])
        current = phase(detail)
        # Resuming gates can replay the original model result. The attempt
        # records identify that replay; never deduplicate on session alone.
        identity = (row['job'], current, json.dumps({k: detail.get(k) for k in
                    ('branch', 'session', 'attempts', 'seconds', 'usage')}, sort_keys=True))
        if identity in seen:
            continue
        seen.add(identity)
        if finished is None or not start < finished <= end:
            continue
        records[current].append(dict(job=row['job'], worker=row['worker'], end=finished,
                                     seconds=detail.get('seconds'), success=row['kind'] in ('passed', 'queued')))
    if db.execute("SELECT 1 FROM sqlite_master WHERE type='table' AND name='implementation_jobs'").fetchone():
        for row in db.execute('SELECT * FROM implementation_jobs'):
            if row['status'] == 'queued':
                queued['implementation'] += 1
            elif row['status'] == 'running':
                active['implementation'].append((row['worker'], timestamp(row['started']), end))
            finished = timestamp(row['finished'])
            if finished is not None and start < finished <= end:
                result = json.loads(row['result'] or '{}')
                seconds = result.get('elapsed_seconds')
                began = timestamp(row['started'])
                if seconds is None and began is not None:
                    seconds = max(0, finished - began)
                records['implementation'].append(dict(job=row['id'], worker=row['worker'], end=finished,
                                                     seconds=seconds, success=row['status'] == 'committed'))
    records['review_pool'] = records['review'] + records['fix']
    active['review_pool'] = active['review'] + active['fix']
    queued['review_pool'] = queued['review'] + queued['fix']
    reviewers = cfg.get('reviewers', {})
    pool = sum(w.get('role', 'all') in ('review', 'all') for w in reviewers.values())
    workers = dict(implementation=len(cfg.get('implementers', {})), review=pool, fix=pool, review_pool=pool,
                   verify=sum(w.get('role', 'all') in ('confirm', 'all') for w in reviewers.values()))
    phases = {}
    for name, completed in records.items():
        succeeded = {r['job'] for r in completed if r['success']}
        durations = [r['seconds'] for r in completed if isinstance(r['seconds'], (int, float)) and r['seconds'] >= 0]
        intervals = list(active[name]) + [(r['worker'], r['end'] - r['seconds'], r['end']) for r in completed
                                         if isinstance(r['seconds'], (int, float)) and r['seconds'] >= 0]
        # Union intervals per slot, so a replay or overlapping timestamps cannot
        # inflate utilization. Include work still running across the window.
        by_worker = {}
        for worker, began, finished in intervals:
            if worker and began is not None and max(start, began) < min(end, finished):
                by_worker.setdefault(worker, []).append((max(start, began), min(end, finished)))
        busy = 0
        for spans in by_worker.values():
            previous = start
            for began, finished in sorted(spans):
                busy += max(0, finished - max(began, previous))
                previous = max(previous, finished)
        phases[name] = dict(advanced_cards=len(succeeded), advanced_per_hour=3600*len(succeeded)/window_seconds,
                            completed_attempts=len(completed), attempts_per_hour=3600*len(completed)/window_seconds,
                            failed_attempts=sum(not r['success'] for r in completed),
                            mean_seconds=round(sum(durations)/len(durations), 1) if durations else None,
                            workers=workers[name], active_workers=len({w for w, _, _ in active[name]}),
                            queued=queued[name], busy_seconds=round(busy, 1),
                            utilization=min(1, busy/(window_seconds*workers[name])) if workers[name] else None)
    return dict(window_seconds=window_seconds, since=dt.datetime.fromtimestamp(start, dt.timezone.utc).isoformat(),
                at=at, basis='unique cards advanced per phase over the rolling window; attempts include failures and retries',
                phases=phases)
