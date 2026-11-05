#!/usr/bin/env python3
"""Durable one-card implementation workers feeding the review queue."""
import argparse
import fcntl
import hashlib
import json
import os
import shutil
from pathlib import Path
import sys
import time
import traceback
from types import SimpleNamespace

import context_budget
import review_lane as review


def connect(root):
    db = review.connect(root)
    db.execute('''CREATE TABLE IF NOT EXISTS implementation_jobs (
        id TEXT PRIMARY KEY, name TEXT NOT NULL, revision TEXT NOT NULL,
        card TEXT NOT NULL, queue_meta TEXT NOT NULL, status TEXT NOT NULL,
        enqueued TEXT NOT NULL, worker TEXT, started TEXT, finished TEXT, result TEXT)''')
    db.commit()
    return db


def sync(root):
    cfg = review.config(root)
    mirror_outcomes(root)
    path = Path(cfg['implementation_queue'])
    with Path(str(path)+'.lock').open('a') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX)
        return sync_queue(root,review.read(path))


def sync_queue(root,queue):
    queue_revision = queue.get('generator_commit')
    if not queue_revision:
        raise ValueError('implementation queue must have generator_commit')
    meta = {k: v for k, v in queue.items() if k != 'cards'}
    pending = [c for c in queue['cards'] if c.get('status') == 'pending']
    keys = set()
    with connect(root) as db:
        db.execute('BEGIN IMMEDIATE')
        added = 0
        for card in pending:
            revision = card.get('implementation_generator_revision', queue_revision)
            job_meta = dict(meta, generator_commit=revision)
            retry = card.get('implementation_retry', 0)
            identity = card['name'] + '\0' + revision + (f'\0retry:{retry}' if retry else '')
            key = hashlib.sha256(identity.encode()).hexdigest()[:20]
            keys.add(key)
            added += db.execute('INSERT OR IGNORE INTO implementation_jobs(id,name,revision,card,queue_meta,status,enqueued) VALUES(?,?,?,?,?,?,?)',
                                (key, card['name'], revision, json.dumps(card), json.dumps(job_meta), 'queued', review.now())).rowcount
            db.execute("UPDATE implementation_jobs SET status='queued',card=?,queue_meta=? WHERE id=? AND status='unavailable'",
                       (json.dumps(card), json.dumps(job_meta), key))
        # A removed candidate cannot be claimed. Running and terminal leases are never reset.
        for row in db.execute("SELECT id FROM implementation_jobs WHERE status='queued'").fetchall():
            if row['id'] not in keys:
                db.execute("UPDATE implementation_jobs SET status='unavailable' WHERE id=?", (row['id'],))
    return added


def claim(db, slot):
    db.execute('BEGIN IMMEDIATE')
    row = db.execute("SELECT * FROM implementation_jobs WHERE status='queued' ORDER BY enqueued,rowid LIMIT 1").fetchone()
    if row:
        db.execute("UPDATE implementation_jobs SET status='running',worker=?,started=? WHERE id=?", (slot, review.now(), row['id']))
    db.commit()
    return dict(row) if row else None


def finish(root, job, status, result):
    with connect(root) as db:
        db.execute('UPDATE implementation_jobs SET status=?,finished=?,result=? WHERE id=? AND status=\'running\'',
                   (status, review.now(), json.dumps(result), job['id']))
    mirror_outcomes(root)


def mirror_outcomes(root):
    """Reconcile durable results after completion or a crash between DB and JSON writes."""
    with connect(root) as db:
        jobs = db.execute("SELECT * FROM implementation_jobs WHERE status NOT IN ('queued','running','unavailable')").fetchall()
    if not jobs:
        return
    queue_path = Path(review.config(root)['implementation_queue'])
    with Path(str(queue_path) + '.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        queue = review.read(queue_path)
        by_name = {card['name']:card for card in queue['cards']}
        changed = False
        for job in jobs:
            if job['name'] not in by_name:
                continue
            card = by_name[job['name']]
            if card.get('implementation_generator_revision',queue.get('generator_commit')) != job['revision']:
                continue
            if card.get('implementation_retry',0) != json.loads(job['card']).get('implementation_retry',0):
                continue
            before = dict(card)
            card.update(json.loads(job['result'] or '{}').get('card', {}))
            card.update(status=job['status'], finished=job['finished'], worker=job['worker'])
            changed |= card != before
        if changed:
            review.save(queue_path, queue)


def retry_stopped(root, job_id, reason):
    """Queue a new attempt after a concrete fix, retaining the original terminal job."""
    if not reason.strip():
        raise ValueError('retry requires a concrete changed blocker')
    path = Path(review.config(root)['implementation_queue'])
    with Path(str(path)+'.lock').open('a') as lock:
        fcntl.flock(lock,fcntl.LOCK_EX)
        with connect(root) as db:
            db.execute('BEGIN IMMEDIATE')
            row = db.execute('SELECT * FROM implementation_jobs WHERE id=?',(job_id,)).fetchone()
            if not row or row['status'] in ('committed','running','queued','unavailable'):
                raise ValueError('only stopped unsuccessful implementations can be retried')
            queue=review.read(path)
            card=next(c for c in queue['cards'] if c['name']==row['name'])
            if card['status'] in ('pending','working','committed') or db.execute(
                "SELECT 1 FROM implementation_jobs WHERE name=? AND status IN ('queued','running')",(row['name'],)).fetchone():
                raise ValueError('card already active or completed')
            attempt=card.get('implementation_retry',0)+1
            review.save(Path(root)/'implementation-retries'/f'{job_id}-{attempt}.json',
                        dict(at=review.now(),reason=reason,job=dict(row),card=card))
            packet=Path(root)/'implementation'/str(row['worker'])/'state/cards'/review.names(row['name'])[0]
            if packet.is_dir():
                archive=Path(root)/'implementation-retries'/f'{job_id}-{attempt}'/'packet'
                shutil.copytree(packet,archive)
                card['implementation_previous_packet']=str(archive)
            for key in ('failure','failed_gates','finished','started','context_marker'):
                card.pop(key,None)
            card.update(status='pending',implementation_retry=attempt,implementation_retry_note=reason)
            review.save(path,queue)
    return row['name']


class Session:
    """Adapt run_card's per-card spawn to a retained session and per-card usage."""
    def __init__(self, lane, cfg, folder):
        self.lane, self.cfg, self.folder = lane, cfg, folder
        self.session = None
        self.cards = 0
        self.previous = None
        self.last_finished = None
        self.original = lane.spawn
        self.total = None
        self.attempts = []

    def start_card(self):
        metrics = context_budget.session_context(self.session) if self.session else {}
        cap = self.cfg.get('implementation_cards_per_context', 20)
        if not cap and metrics.get('last_tokens') is None:
            cap = 20  # Keep a bounded fallback when the rollout cannot be measured.
        if self.session and (context_budget.idle_restart(self.last_finished) or
                             (cap and self.cards >= cap) or
                             context_budget.budget_restart(metrics, self.cfg.get('context_tokens', 300000))):
            self.session, self.cards, self.previous = None, 0, None
        self.total = self.lane.Usage()
        self.attempts = []

    def finish_card(self):
        self.cards += 1
        self.last_finished = time.monotonic()

    def spawn(self, backend, card, prompt, session, timeout, log):
        deadline = time.monotonic() + timeout
        marker = self.folder / 'precompact-stop.json'
        rotations = 0
        invocation_turns = 0
        while True:
            marker.unlink(missing_ok=True)
            card['context_marker'] = str(marker)
            old_session, previous = self.session, self.previous
            started = time.monotonic()
            code, self.session, used = self.original(backend, card, prompt, self.session,
                                                       max(1, deadline - time.monotonic()), log)
            delta = self.lane.Usage()
            for key in ('input', 'cached', 'output'):
                value = getattr(used, key)
                if backend.cumulative and old_session == self.session and previous:
                    value = max(0, value - getattr(previous, key))
                setattr(delta, key, value)
            delta.turns = used.turns
            delta.reported_cost = used.reported_cost
            self.total.add(delta, False)
            invocation_turns += delta.turns
            self.previous = used
            self.attempts.append(dict(session=self.session, seconds=round(time.monotonic()-started, 2),
                                      usage=delta.record(backend.model), context=context_budget.session_context(self.session)))
            if not context_budget.precompact_stopped(marker, self.session):
                result = self.lane.Usage()
                result.add(self.total, False)
                result.turns = invocation_turns
                result.infrastructure_error = used.infrastructure_error
                return code, self.session, result
            if time.monotonic() >= deadline or rotations >= 3:
                # Return an infrastructure failure, never an apparent gate failure.
                empty = self.lane.Usage()
                empty.add(self.total, False)
                empty.turns = 0
                empty.infrastructure_error = 'precompact restart limit reached'
                return 1, None, empty
            rotations += 1
            self.session, self.cards, self.previous = None, 0, None
            prompt = ('Continue this SAME card in a fresh context. Preserve and inspect all existing work; '
                      'finish the card and update its report.\n\n' + prompt)


def worker(root, slot, once=False):
    root = Path(root)
    cfg = review.config(root)
    tree = Path(cfg['implementers'][slot]['tree'])
    folder = root / 'implementation' / slot
    state = folder / 'state'
    state.mkdir(parents=True, exist_ok=True)
    with (folder / 'slot.lock').open('w') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        review.maintain_caches(root, cfg, begin_target=tree / 'target')
        review.clean(tree)
        base = cfg['base']
        review.git(tree, 'merge-base', '--is-ancestor', base, 'HEAD')
        lane = review.load_lane(root, tree, state)
        backend = lane.Codex(cfg.get('producer_model', 'gpt-6.1-terra'), cfg.get('producer_effort', 'medium'))
        session = Session(lane, cfg, folder)
        lane.spawn = session.spawn
        stamp = review.driver_stamp()
        while True:
            cfg = review.config(root)
            if not cfg.get('enabled', True):
                return
            review.clean(tree)
            review.reload_at_boundary(root, slot, tree, 'implementation_lane.py', stamp)
            sync(root)
            with connect(root) as db:
                job = claim(db, slot)
            if not job:
                review.save(folder / 'live.json', dict(state='waiting', at=review.now(), context_cards=session.cards))
                if once:
                    return
                time.sleep(cfg.get('poll_seconds', 5))
                continue
            started = time.monotonic()
            card = json.loads(job['card'])
            queue = json.loads(job['queue_meta'])
            import shared_base
            pin = shared_base.read_work_base(cfg, tree)
            job_base = pin['head'] if pin else base
            if pin:
                # Each new card has a durable branch; completed candidates keep
                # their references when the shared helper pin advances.
                review.git(tree, 'checkout', '-b', 'implement/'+job['id'], job_base)
            queue['cantrip_commit'] = job_base
            queue['cards'] = [card]
            lane.save_queue(queue)
            session.start_card()
            review.save(folder / 'live.json', dict(state='running', at=review.now(), card=card['name'], context_cards=session.cards))
            try:
                args = SimpleNamespace(timeout=cfg.get('timeout_seconds', 3600), repairs=cfg.get('repairs', 3))
                outcome = lane.run_card(card, backend, args, queue)
                result = dict(card=card, elapsed_seconds=round(time.monotonic()-started, 2), attempts=session.attempts)
                if outcome == 'committed':
                    source = dict(card, producer=slot, producer_tree=str(tree),
                                  packet=str(lane.card_dir(card['name'])), campaign=str(cfg['lane_root']), source_base=job_base)
                    if pin:
                        source.update(shared_base.metadata(pin))
                    # Enqueue before releasing the producer lease. Insertion is idempotent.
                    db = review.connect(root)
                    review.enqueue(db, source, cfg.get('priorities', {}).get(card['name'], 0))
                finish(root, job, outcome, result)
                review.maintain_caches(root, cfg)
                session.finish_card()
                if outcome in ('infrastructure', 'error', 'timeout'):
                    return
            except Exception as error:
                finish(root, job, 'infrastructure', dict(error=str(error), traceback=traceback.format_exc(),
                                                          elapsed_seconds=round(time.monotonic()-started, 2)))
                raise
            if once:
                return


def status(root):
    with connect(root) as db:
        rows = db.execute('SELECT status, COUNT(*) n FROM implementation_jobs GROUP BY status ORDER BY status').fetchall()
        results = [json.loads(r['result']) for r in db.execute('SELECT result FROM implementation_jobs WHERE result IS NOT NULL')]
    seconds = [r['elapsed_seconds'] for r in results if 'elapsed_seconds' in r]
    return dict(counts={r['status']: r['n'] for r in rows}, completed=len(seconds),
                mean_seconds=round(sum(seconds)/len(seconds), 2) if seconds else None)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=review.DEFAULT_ROOT)
    sub = parser.add_subparsers(dest='command', required=True)
    sub.add_parser('sync')
    sub.add_parser('status')
    retry=sub.add_parser('retry');retry.add_argument('job');retry.add_argument('--reason',required=True)
    w = sub.add_parser('worker')
    w.add_argument('slot')
    w.add_argument('--once', action='store_true')
    args = parser.parse_args()
    if args.command == 'sync':
        print(sync(args.root))
    elif args.command == 'status':
        print(json.dumps(status(args.root), indent=2))
    elif args.command == 'retry':
        print(retry_stopped(args.root,args.job,args.reason))
    else:
        worker(args.root, args.slot, args.once)


if __name__ == '__main__':
    main()
