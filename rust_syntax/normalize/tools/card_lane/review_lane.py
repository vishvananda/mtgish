#!/usr/bin/env python3
"""A durable consumer for committed card-lane cards. No pushes, PRs or merges.

Producers keep their existing queues. `sync` imports committed revisions exactly once;
review workers atomically claim one revision at a time in separate prepared worktrees.
Code fixes receive a fresh Terra confirmation. Original producer history is never edited.
"""
import argparse
import datetime as dt
import fcntl
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time
import traceback
import tomllib
import unicodedata

import context_budget
import review_evidence
import cache_leases
import queue_storage

HERE = Path(__file__).resolve().parent
DEFAULT_ROOT = Path.home() / "card-lane" / "pipeline"


class Database(sqlite3.Connection):
    def __enter__(self):
        self.depth = getattr(self, "depth", 0) + 1
        return super().__enter__()

    def __exit__(self, *args):
        try:
            return super().__exit__(*args)
        finally:
            self.depth -= 1
            if self.depth == 0:
                self.close()


def now():
    return dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds")


def read(path):
    return json.loads(Path(path).read_text())


def save(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(mode="w", dir=path.parent, delete=False) as f:
        json.dump(value, f, indent=2)
        f.write("\n")
    os.replace(f.name, path)


def maintain_caches(root, cfg, *, begin_target=None, retired_target=None, end_target=None):
    """Optional housekeeping cannot reclassify a completed card or publication."""
    # Cache-use protection is core coordination, established before optional maintenance imports.
    if begin_target is not None:
        cache_leases.acquire(begin_target)
    try:
        if end_target is not None:
            cache_leases.release(end_target)
        import cache_cleanup
        if begin_target is not None:
            cache_cleanup.record_reuse(root, begin_target)
        return cache_cleanup.after_lane_work(root, cfg, retired_target=retired_target)
    except Exception as error:
        print(f'Card-lane cache maintenance skipped: {error}', file=sys.stderr)
        return False


def connect(root):
    root = Path(root)
    root.mkdir(parents=True, exist_ok=True)
    return queue_storage.open_database(root / "queue.sqlite", Database, initialize_database)


def initialize_database(db):
    db.row_factory = sqlite3.Row
    db.execute("PRAGMA journal_mode=WAL")
    db.executescript("""
        CREATE TABLE IF NOT EXISTS jobs (
          id TEXT PRIMARY KEY, name TEXT NOT NULL, source_commit TEXT NOT NULL,
          source TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'queued',
          stage TEXT NOT NULL DEFAULT 'review', priority INTEGER NOT NULL DEFAULT 0,
          enqueued TEXT NOT NULL, worker TEXT, started TEXT, finished TEXT,
          candidate TEXT, branch TEXT, fixed_by TEXT, result TEXT);
        CREATE TABLE IF NOT EXISTS events (
          id INTEGER PRIMARY KEY, job TEXT, worker TEXT, at TEXT NOT NULL,
          kind TEXT NOT NULL, detail TEXT NOT NULL);
    """)


def preserve_running_assignment(db, slot, folder, table="jobs"):
    # A restarted process cannot know whether a failed commit was durable.
    # Keep that lease and its worktree until an explicit recovery decision.
    if table not in ("jobs", "implementation_jobs"):
        raise ValueError("unknown queue table")
    row = db.execute(f"SELECT id,name FROM {table} WHERE worker=? AND status='running' LIMIT 1", (slot,)).fetchone()
    if row is None:
        return False
    message = f"Unfinished job {row['id']} remains assigned to {slot}; preserve its worktree and recover explicitly."
    save(folder / "live.json", dict(state="infrastructure", worker=slot, card=row['name'],
                                   job=row['id'], at=now(), recovery_required=True, error=message))
    print(message, file=sys.stderr)
    return True


def event(db, job, worker, kind, detail):
    db.execute("INSERT INTO events(job,worker,at,kind,detail) VALUES(?,?,?,?,?)",
               (job, worker, now(), kind, json.dumps(detail)))


def enqueue(db, source, priority=0):
    key = hashlib.sha256((source["name"] + "\0" + source["commit"]).encode()).hexdigest()[:20]
    with db:
        added = db.execute("INSERT OR IGNORE INTO jobs(id,name,source_commit,source,priority,enqueued) VALUES(?,?,?,?,?,?)",
                           (key, source["name"], source["commit"], json.dumps(source), priority, now())).rowcount
    return added


def claim(db, worker, role="all"):
    # A lease is never expired automatically: a killed worker may have valuable dirty files.
    db.execute("BEGIN IMMEDIATE")
    if role not in ("all", "review", "confirm"):
        raise ValueError("reviewer role must be all, review, or confirm")
    row = db.execute("SELECT * FROM jobs WHERE status='queued' AND (?='all' OR stage=?) ORDER BY priority DESC,enqueued,id LIMIT 1", (role, role)).fetchone()
    if row:
        db.execute("UPDATE jobs SET status='running',worker=?,started=?,finished=NULL WHERE id=?",
                   (worker, now(), row["id"]))
        event(db, row["id"], worker, "claimed", {"stage": row["stage"]})
    db.commit()
    return dict(row) if row else None


def finish(db, job, status, result, **fields):
    values = dict(status=status, result=json.dumps(result), finished=now(), **fields)
    with db:
        db.execute("UPDATE jobs SET " + ",".join(k + "=?" for k in values) + " WHERE id=?",
                   [*values.values(), job["id"]])
        event(db, job["id"], job.get("worker"), status,
              dict(result, stage=job["stage"], review_round=json.loads(job["source"]).get("review_round", 0)))


def config(root):
    return read(Path(root) / "config.json")


def sync(root):
    cfg = config(root)
    added = 0
    with connect(root) as db:
        for manifest_path in cfg["producer_manifests"]:
            manifest_path = Path(manifest_path)
            manifest = read(manifest_path)
            for worker in manifest["workers"]:
                queue_path = Path(worker["state"]) / "queue.json"
                if not queue_path.exists():
                    continue
                queue = read(queue_path)
                for card in queue["cards"]:
                    if card.get("status") != "committed" or not card.get("commit"):
                        continue
                    source = dict(card, producer=worker["worker"], producer_tree=worker["tree"],
                                  packet=str(Path(worker["state"]) / "cards" / names(card["name"])[0]),
                                  campaign=str(manifest_path.parent),
                                  source_base=queue.get("cantrip_commit") or manifest.get("base"))
                    added += enqueue(db, source, cfg.get("priorities", {}).get(card["name"], 0))
    return added


def names(name):
    sys.path.insert(0, str(HERE.parent))
    from cantrip_candidates import file_stem, content_slug
    return file_stem(canonical_typography(name)), content_slug(canonical_typography(name))


def canonical_typography(name):
    return unicodedata.normalize('NFKC',name).translate(str.maketrans({'’':"'",'‘':"'",'“':'"','”':'"','–':'-','—':'-'}))


def run(tree, *args, env=None, check=True):
    p = subprocess.run([str(a) for a in args], cwd=tree, env=env, text=True,
                       stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    if check and p.returncode:
        raise RuntimeError(" ".join(map(str, args)) + "\n" + p.stdout[-5000:])
    return p.returncode, p.stdout


def git(tree, *args, **kwargs):
    return run(tree, "git", *args, **kwargs)[1].strip()


def clean(tree):
    if git(tree, "status", "--porcelain"):
        raise RuntimeError(f"{tree}: unfinished work preserved; inspect it before restarting this slot")


def driver_stamp():
    paths = [HERE / name for name in (
        'review_lane.py', 'implementation_lane.py', 'shared_base.py', 'review_confirmation.py', 'card_lane.py',
        'context_budget.py', 'review_evidence.py', 'cache_cleanup.py', 'cache_leases.py')]
    paths.append(HERE.parent / 'cantrip_tree.py')
    return tuple(path.stat().st_mtime_ns for path in paths)


def reload_at_boundary(root, slot, tree, script, stamp):
    if driver_stamp() != stamp:
        clean(tree)
        # The slot lock is close-on-exec. Drop an already-consumed --resume
        # argument and start fresh only after the previous lease has finished.
        print('Reloading updated driver at clean card boundary', flush=True)
        os.execv(sys.executable, [sys.executable, '-u', str(HERE / script),
                                 '--root', str(root), 'worker', slot])


def load_lane(root, tree, state):
    cfg = config(root)
    # Writable sandbox roots must exist before Codex constructs its bind mounts.
    Path(state).mkdir(parents=True, exist_ok=True)
    scratch = tree / 'target' / 'tmp'
    scratch.mkdir(parents=True, exist_ok=True)
    tempfile.tempdir = str(scratch)
    os.environ.update(CARD_LANE=cfg["lane_root"], CARD_LANE_CANTRIP=str(tree),
                      CARD_LANE_STATE=str(state), CARD_LANE_TARGET=str(tree / "target"),
                      CARD_LANE_BRANCH=git(tree, "branch", "--show-current"), CARGO_BUILD_JOBS="8",
                      CARD_LANE_FOCUSED="1" if cfg.get("focused_builds", True) else "0", TMPDIR=str(scratch))
    spec = importlib.util.spec_from_file_location("review_card_lane", HERE / "card_lane.py")
    lane = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(lane)
    # Re-imported per card; no stale in-process build memo can cross revisions.
    return lane


def prerequisite_commits(tree, source):
    base = source.get("source_base")
    if not base:
        raise RuntimeError("producer did not record its base; cannot determine prerequisites safely")
    rows = git(tree, "log", "--reverse", "--first-parent", "--format=%H %s", f"{base}..{source['commit']}").splitlines()
    if len(rows) > 100:
        raise RuntimeError("unexpected producer ancestry: refusing a broad cherry-pick")
    # The requested card is applied separately below. Its title is a convention,
    # not evidence that the tip is a shared prerequisite.
    return [line.split(" ", 1)[0] for line in rows
            if line.split(" ", 1)[0] != source['commit']
            and not line.split(" ", 1)[1].startswith("Card:")]


def job_folder(root, job, stage):
    source = json.loads(job["source"]) if isinstance(job["source"], str) else job["source"]
    folder = Path(root) / "jobs" / job["id"]
    round_number = source.get("review_round", 0)
    attempt = source.get('confirmation_attempt', 0) if stage == 'confirm' else 0
    return folder / (f"round-{round_number}" if round_number else "") / (f'confirm-attempt-{attempt}' if attempt else stage)


def handoff_verifier_findings(root):
    """Return substantive verifier blocks to review, preserving each round's evidence."""
    count = 0
    cfg = config(root)
    limit = cfg.get("max_verifier_rejections", cfg.get("max_verifier_rounds", 3))
    if not isinstance(limit, int) or limit < 1:
        raise ValueError("max_verifier_rejections must be a positive integer")
    with connect(root) as db:
        db.execute("BEGIN IMMEDIATE")
        for row in db.execute("SELECT * FROM jobs WHERE stage='confirm' AND status='blocked'").fetchall():
            job = dict(row)
            source = json.loads(job["source"])
            round_number = source.get("review_round", 0)
            confirm_dir = job_folder(root, job, "confirm")
            report_path = confirm_dir / "review.json"
            if not report_path.exists():
                continue
            report = read(report_path)
            findings = report.get("findings")
            if report.get("decision") != "blocked" or not isinstance(findings, list) or not any(findings):
                continue
            findings = [finding for finding in findings if finding]
            review_dir = job_folder(root, job, "review")
            checks_path, result_path = review_dir / "checks.json", review_dir / "result.json"
            if not checks_path.exists() or not result_path.exists() or not job["candidate"]:
                continue
            checks, previous = read(checks_path), read(result_path)
            if not checks.get("head") or not previous.get("prepared_parent"):
                continue
            detail = dict(round=round_number, findings=findings, count=len(findings),
                          report=str(report_path), candidate=job["candidate"], review=str(review_dir))
            # A capped job remains blocked; record its findings just once.
            recorded = db.execute("SELECT 1 FROM events WHERE job=? AND kind='verification_blocked' AND json_extract(detail, '$.round')=?", (job["id"], round_number)).fetchone()
            if not recorded:
                event(db, job["id"], job["worker"], "verification_blocked", detail)
            # Coordinator retries advance artifact rounds too. Only actual new
            # verifier rejections consume this independent limit.
            rejections = source.get("verifier_rejections", round_number + int(bool(recorded)))
            if not recorded:
                rejections += 1
            source["verifier_rejections"] = rejections
            if rejections >= limit:
                db.execute("UPDATE jobs SET source=? WHERE id=?", (json.dumps(source), job["id"]))
                continue
            source["review_round"] = round_number + 1
            source["retry_note"] = ("A fresh verifier blocked the previous candidate. Resolve every finding and prove the fixes.\n"
                + "Findings (verbatim JSON):\n" + json.dumps(findings, indent=2, ensure_ascii=False)
                + f"\nPrevious candidate: {job['candidate']}\nPrior review and proof: {review_dir}\nVerifier report: {report_path}\n")
            retry = dict(retry_requested=True, retry_base=checks["head"], retry_candidate=job["candidate"],
                         prepared_parent=previous["prepared_parent"])
            db.execute("UPDATE jobs SET stage='review',status='queued',source=?,result=?,priority=?,worker=NULL,started=NULL,finished=NULL WHERE id=?",
                       (json.dumps(source), json.dumps(retry), max(job["priority"], 1), job["id"]))
            event(db, job["id"], None, "returned_for_fixes", {**detail, "next_round": round_number + 1})
            count += 1
        db.commit()
    return count


def retry_stopped(root, job_id, reason, *, new_base=None):
    """Explicitly return preserved review work after its blocker changes."""
    if not reason.strip():
        raise ValueError("retry requires the concrete reason the blocker has changed")
    cfg = config(root)
    with connect(root) as db:
        db.execute("BEGIN IMMEDIATE")
        row = db.execute("SELECT * FROM jobs WHERE id=?", (job_id,)).fetchone()
        if not row or row["status"] not in ("blocked", "infrastructure"):
            raise ValueError("only stopped review jobs or reportless confirmations can be explicitly retried")
        job = dict(row)
        source = json.loads(job["source"])
        previous = json.loads(job["result"] or "{}")
        limit = cfg.get("max_verifier_rejections", cfg.get("max_verifier_rounds", 3))
        source.setdefault("verifier_rejections", source.get("review_round", 0))
        if source.get("verifier_rejections", 0) >= limit:
            raise ValueError("verifier rejection limit reached; retry cannot bypass it")
        if job['stage'] == 'confirm':
            prior_folder = job_folder(root, job, 'confirm')
            if previous.get('report') or (prior_folder/'review.json').exists() or not job['candidate']:
                raise ValueError('confirmation with a report requires its normal finding/fix process')
            attempt = source.get('confirmation_attempt', 0) + 1
            save(Path(root)/'jobs'/job_id/f'confirmation-retry-{attempt}.json', dict(at=now(),reason=reason,job=job))
            source['confirmation_attempt'] = attempt
            source['confirmation_retry_note'] = reason
            db.execute("UPDATE jobs SET status='queued',source=?,result=?,worker=NULL,started=NULL,finished=NULL WHERE id=?",
                       (json.dumps(source),json.dumps(dict(confirmation_retry=True,prior=str(prior_folder))),job_id))
            event(db,job_id,None,'confirmation_retry_requested',dict(reason=reason,attempt=attempt,prior=str(prior_folder)))
            db.commit()
            return job_id
        if job['stage'] != 'review':
            raise ValueError('only stopped review jobs or reportless confirmations can be explicitly retried')
        if not job["branch"] or not previous.get("prepared_parent"):
            raise ValueError("stopped job lacks its preserved branch or prepared parent")
        migration = {}
        pin = None
        if new_base is not None:
            import shared_base
            tree = cfg.get('integration', {}).get('tree')
            if not tree:
                raise ValueError('rebasing stopped work requires the local integration repository')
            pin = shared_base.read_work_base(cfg, Path(tree))
            if not pin or pin.get('kind') != 'local_integration' or pin['head'] != new_base:
                raise ValueError('new base must be the currently validated local snapshot')
            git(tree, 'merge-base', '--is-ancestor', previous['prepared_parent'], new_base)
            retained = git(tree, 'rev-parse', previous.get('wip_commit') or job['branch'])
            git(tree, 'merge-base', '--is-ancestor', previous['prepared_parent'], retained)
            migration = dict(rebase_base=previous['prepared_parent'], rebase_head=retained)
        round_number = source.get("review_round", 0) + 1
        prior_folder = job_folder(root, job, "review")
        source["review_round"] = round_number
        source["retry_note"] = (f"Coordinator returned this SAME card after its blocker changed:\n{reason}\n"
                                f"Prior immutable report and evidence: {prior_folder}\n"
                                f"Previous report:\n{json.dumps(previous.get('report', {}), ensure_ascii=False)}")
        retry = dict(retry_requested=True, prepared_parent=previous["prepared_parent"])
        if previous.get("wip_commit"):
            retry["wip_commit"] = previous["wip_commit"]
        if pin:
            source.update(shared_base.metadata(pin))
            retry.update(migration, prepared_parent=pin['head'])
        save(Path(root) / "jobs" / job_id / f"retry-{round_number}.json",
             dict(at=now(), reason=reason, job=job))
        db.execute("UPDATE jobs SET stage='review',status='queued',source=?,result=?,priority=?,"
                   "candidate=NULL,worker=NULL,started=NULL,finished=NULL WHERE id=?",
                   (json.dumps(source), json.dumps(retry), max(job["priority"], 1), job_id))
        event(db, job_id, None, "retry_requested", dict(reason=reason, round=round_number, prior=str(prior_folder)))
        db.commit()
    return job_id


def repair_confirmation(root, job_id, repair_folder, validated_tree, reason):
    """Explicit coordinator evidence repair, followed by fresh independent confirmation.

    This never resets the rejection counter or participates in automatic retries. A candidate can receive this
    intervention once; another rejection leaves it blocked. Prior reports and evidence remain untouched.
    """
    if not reason.strip():
        raise ValueError('evidence repair requires a concrete resolution')
    root, repair_folder, validated_tree = Path(root), Path(repair_folder).resolve(), Path(validated_tree)
    with connect(root) as db:
        db.execute('BEGIN IMMEDIATE')
        row = db.execute('SELECT * FROM jobs WHERE id=?', (job_id,)).fetchone()
        if not row or row['status'] != 'blocked' or row['stage'] != 'confirm' or not row['candidate']:
            raise ValueError('evidence repair requires a blocked confirmation candidate')
        job = dict(row)
        source = json.loads(job['source'])
        if job['candidate'] in source.get('evidence_repaired_candidates', []):
            raise ValueError('this candidate already received its evidence repair confirmation')
        old_review = job_folder(root, job, 'review')
        old_confirmation = job_folder(root, job, 'confirm') / 'review.json'
        rejection = read(old_confirmation)
        if rejection.get('decision') != 'blocked' or not rejection.get('findings'):
            raise ValueError('evidence repair must retain a substantive verifier rejection')
        clean(validated_tree)
        if git(validated_tree, 'rev-parse', 'HEAD') != job['candidate']:
            raise ValueError('repair must be checked on the exact unchanged candidate')
        checks = read(repair_folder / 'checks.json')
        if (checks.get('head') != job['candidate'] or checks.get('diff_digest') != work_digest(validated_tree)
                or checks.get('at', '') <= read(old_review / 'checks.json').get('at', '')):
            raise ValueError('repair needs fresh checks bound to the unchanged candidate')
        # build_bundle validates every named gate, report validation and required artifacts, and freezes the evidence.
        report = read(repair_folder / 'review.json')
        if not report_valid(report, job['name']):
            raise ValueError('repair report must resolve the findings and pass')
        if (repair_folder / 'fixes.diff').read_bytes() != (old_review / 'fixes.diff').read_bytes():
            raise ValueError('evidence repair cannot change the already reviewed code diff')
        source['review_round'] = source.get('review_round', 0) + 1
        source.pop('confirmation_attempt', None)
        source['evidence_repaired_candidates'] = [*source.get('evidence_repaired_candidates', []), job['candidate']]
        source['evidence_repair_note'] = reason
        updated = dict(job, source=json.dumps(source))
        destination = job_folder(root, updated, 'review')
        if destination.exists():
            raise ValueError('repair destination already exists; prior evidence cannot be overwritten')
        from review_confirmation import build_bundle
        build_bundle(repair_folder / 'validated-bundle', repair_folder, job['name'], job['candidate'],
                     git(validated_tree, 'rev-parse', 'HEAD^{tree}'), job['source_commit'])
        shutil.copytree(repair_folder, destination)
        archive = dict(at=now(), reason=reason, job=job, repair=str(repair_folder),
                       rejected_report=str(old_confirmation),
                       rejected_report_sha256=hashlib.sha256(old_confirmation.read_bytes()).hexdigest())
        save(destination / 'evidence-repair-origin.json', archive)
        previous = read(old_review / 'result.json')
        result = dict(prepared_parent=previous['prepared_parent'], evidence_repair=str(destination))
        db.execute("UPDATE jobs SET status='queued',source=?,result=?,worker=NULL,started=NULL,finished=NULL WHERE id=?",
                   (json.dumps(source), json.dumps(result), job_id))
        event(db, job_id, None, 'evidence_repair_requested', archive)
        db.commit()
    return job_id


def correct_approved(root, job_id, reason, new_base):
    """Invalidate an approval with a concrete new finding and review its complete candidate on a fixed engine."""
    if not reason.strip():
        raise ValueError('correction requires a concrete new finding and tested fix')
    import shared_base
    root = Path(root); cfg = config(root)
    tree = cfg.get('integration', {}).get('tree')
    if not tree:
        raise ValueError('correction requires the local integration repository')
    pin = shared_base.read_work_base(cfg, Path(tree))
    if not pin or pin.get('kind') != 'local_integration' or pin['head'] != new_base:
        raise ValueError('new base must be the currently validated local snapshot')
    folder = root/'integration'; folder.mkdir(exist_ok=True)
    with (folder/'integration.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        with connect(root) as db:
            db.execute('BEGIN IMMEDIATE')
            row = db.execute('SELECT * FROM jobs WHERE id=?', (job_id,)).fetchone()
            if not row:
                raise ValueError('correction requires a completed approval')
            job = dict(row)
            if job['status'] == 'superseded':
                old = db.execute("SELECT detail FROM events WHERE job=? AND kind='correction_requested' ORDER BY id DESC LIMIT 1", (job_id,)).fetchone()
                if old and json.loads(old['detail']).get('base') == new_base:
                    return json.loads(old['detail'])['correction_job']
            if job['status'] != 'passed' or not job['candidate']:
                raise ValueError('correction requires a completed approval')
            source = json.loads(job['source']); previous = json.loads(job['result'] or '{}')
            if not previous.get('prepared_parent') or source.get('publication_base') == new_base:
                raise ValueError('correction requires a changed validated engine base and preserved parent')
            git(tree, 'merge-base', '--is-ancestor', previous['prepared_parent'], new_base)
            retained = git(tree, 'rev-parse', job['candidate'])
            key = hashlib.sha256((job['name']+'\0'+retained+'\0correction:'+new_base).encode()).hexdigest()[:20]
            archive = root/'jobs'/job_id/f'correction-{new_base}.json'
            save(archive, dict(at=now(), reason=reason, job=job, correction_job=key, base=new_base))
            original_base = source.get('publication_base') or source.get('source_base') or previous['prepared_parent']
            source.update(shared_base.metadata(pin), commit=retained, source_base=original_base,
                          correction_of=job_id, correction_base=new_base, review_round=0, verifier_rejections=0,
                          retry_note=f'Correct this previously approved candidate after a shared engine fix.\n{reason}\nPrior approval and complete candidate: {archive}\nRetain its prior fixes and scenarios; run normal review and verification.')
            source.pop('confirmation_attempt', None)
            db.execute('INSERT INTO jobs(id,name,source_commit,source,priority,enqueued) VALUES(?,?,?,?,?,?)',
                       (key, job['name'], retained, json.dumps(source), max(job['priority'], 1), now()))
            db.execute("UPDATE jobs SET status='superseded' WHERE id=?", (job_id,))
            if db.execute("SELECT 1 FROM sqlite_master WHERE type='table' AND name='integration_jobs'").fetchone():
                db.execute("UPDATE integration_jobs SET status='superseded',detail=? WHERE status='pending' AND json_extract(source,'$.job')=?",
                           (f'Approval superseded by correction {key}', job_id))
            event(db, job_id, None, 'correction_requested', dict(reason=reason, correction_job=key, base=new_base, prior=str(archive)))
            event(db, key, None, 'correction_enqueued', dict(reason=reason, previous_job=job_id, base=new_base))
    return key


def prepare(root, tree, job):
    clean(tree)
    cfg, source = config(root), json.loads(job["source"])
    previous = json.loads(job["result"]) if job.get("result") else {}
    if previous.get("retry_requested") and job["stage"] == "review":
        if previous.get('rebase_base') and previous.get('rebase_head'):
            branch = job['branch'] + '-retry-' + str(time.time_ns())
            git(tree, 'checkout', '-b', branch, previous['prepared_parent'])
            diff = subprocess.run(['git', 'diff', '--binary', previous['rebase_base'], previous['rebase_head']],
                                  cwd=tree, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True).stdout
            # A conflicting migration stops before changing the new tree. The complete parked work and report remain.
            if diff:
                subprocess.run(['git', 'apply', '--index', '--check'], cwd=tree, input=diff, check=True)
                subprocess.run(['git', 'apply', '--index'], cwd=tree, input=diff, check=True)
            return branch, previous['prepared_parent'], source
        if previous.get("retry_base") and previous.get("retry_candidate"):
            branch = f"review/{Path(root).name}/{job['id']}/review-round-{source['review_round']}"
            git(tree, "checkout", "-b", branch, previous["retry_base"])
            diff = subprocess.run(["git", "diff", "--binary", previous["retry_base"], previous["retry_candidate"]],
                                  cwd=tree, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True).stdout
            subprocess.run(["git", "apply", "--index"], cwd=tree, input=diff, check=True)
            return branch, previous["prepared_parent"], source
        branch = job["branch"] + "-retry-" + str(time.time_ns())
        base = git(tree, "rev-parse", previous["wip_commit"] + "^") if previous.get("wip_commit") else job["branch"]
        git(tree, "checkout", "-b", branch, base)
        if previous.get("wip_commit"):
            git(tree, "cherry-pick", "--no-commit", previous["wip_commit"])
        return branch, previous["prepared_parent"], source
    branch = f"review/{Path(root).name}/{job['id']}/{job['stage']}"
    if source.get("review_round", 0):
        branch += f"-round-{source['review_round']}"
    if job["stage"] == "confirm":
        if source.get('confirmation_attempt'):
            branch += f"-attempt-{source['confirmation_attempt']}"
        git(tree, "checkout", "-b", branch, job["candidate"])
        return branch, git(tree, "rev-parse", "HEAD^"), source
    import shared_base
    if not source.get('publication_base'):
        # New producers already supply their frozen publication base. Older
        # candidates retain their reviewed API until an explicit migration.
        pin = shared_base.read_work_base(cfg, tree) if cfg.get('local_base_manifest') or cfg.get('review_use_shared_base') else None
        source.update(shared_base.metadata(pin) if pin else dict(publication_base=cfg['base']))
        with connect(root) as db:
            db.execute('UPDATE jobs SET source=? WHERE id=?', (json.dumps(source), job['id']))
        job['source'] = json.dumps(source)
    git(tree, "checkout", "-b", branch, source['publication_base'])
    for commit in prerequisite_commits(tree, source):
        if source.get('shared_dependencies') and not git(tree, 'cherry', 'HEAD', commit, commit+'^').startswith('+'):
            continue  # Already supplied by the validated helper pin or upstream.
        git(tree, "cherry-pick", commit)
    parent = git(tree, "rev-parse", "HEAD")
    # A validated local snapshot can already include the revision being independently reviewed.
    # Review its present code without attempting an empty cherry-pick of an ancestor.
    if subprocess.run(['git','merge-base','--is-ancestor',source['commit'],'HEAD'],cwd=tree,
                      stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode != 0:
        git(tree, "cherry-pick", source["commit"])
    return branch, parent, source


def packet(root, tree, job, source, lane):
    folder = job_folder(root, job, job["stage"])
    folder.mkdir(parents=True, exist_ok=True)
    author = Path(source["packet"]) if source.get("packet") else None
    for name in ("reference.md", "shared.md", "prior-art.md", "report.json", "check.json", "mutate.txt", "test-points.json"):
        if author is not None and (author / name).exists() and not (folder / ("author-" + name)).exists():
            (folder / ("author-" + name)).write_bytes((author / name).read_bytes())
    missing = [name for name in ("author-reference.md", "author-report.json") if not (folder / name).is_file()]
    if missing:
        raise ValueError("review input lacks producer evidence: " + ", ".join(missing) +
                         "; restore the producer packet or its candidate-bound cached evidence")
    source_path = lane.card_file(source["name"])
    current = source_path.read_text() if source_path.exists() else "(characteristics only)"
    (folder / "current-shared.md").write_text(lane.shared_code(current) if source_path.exists() else "")
    scenarios = lane.card_scenarios(source["name"])
    prior = (Path(source["campaign"]) / "publication" / (source["producer"] + "-review.json")
             if source.get("campaign") and source.get("producer") else None)
    hints = None
    if prior is not None and prior.exists():
        hints = next((c for c in read(prior).get("cards", []) if c.get("name") == source["name"]), None)
    save(folder / "prior-findings.json", hints)
    save(folder / "input.json", dict(job=job, source=source, tree=str(tree), model=config(root)["review_model"]))
    body = [f"# {source['name']} — independent {job['stage']} and fix\n",
            f"Source commit: {source['commit']}. Issue: #{source.get('issue')}.\n",
            "## Oracle / rulings / rules / test points\n" + (folder / "author-reference.md").read_text(),
            "## Current card code\n```rust\n" + current + "\n```",
            "## Content\n```json\n" + lane.content_file(source["name"]).read_text() + "\n```",
            "## Shared definitions in the CURRENT checkout\n" + (folder / "current-shared.md").read_text(),
            "## Author's line-by-line validation and evidence\n" + (folder / "author-report.json").read_text(),
            "## Scenarios\n" + "\n".join(f"### {p.relative_to(tree)}\n```\n{p.read_text()}\n```" for p in scenarios),
            "## Prior findings (leads to verify; not a verdict)\n" + json.dumps(hints, indent=2)]
    if source.get("retry_note"):
        body.append("## Retry instructions\n" + source["retry_note"])
    if job["stage"] == "confirm":
        original = job_folder(root, job, "review")
        body += ["## Fix report\n" + (original / "review.json").read_text(),
                 "## Exact fixes to confirm\n```diff\n" + (original / "fixes.diff").read_text() + "\n```"]
    (folder / "packet.md").write_text("\n\n".join(body))
    if job["stage"] == "confirm":
        # Candidate content must be exactly what passed the original review's machine gates.
        checked = read(original / "checks.json")
        if checked.get("diff_digest") != work_digest(tree):
            raise ValueError("confirmation candidate does not match checked source")
        from review_confirmation import build_bundle
        build_bundle(folder, original, source["name"], job["candidate"], git(tree, "rev-parse", "HEAD^{tree}"), source["commit"])
    return folder


def prompt(root, tree, folder, job):
    cfg = config(root) if (Path(root)/'config.json').exists() else {}
    consolidation = ""
    if cfg.get('workflow') == 'consolidate_then_publish' and cfg.get('integration', {}).get('dedup_owner') == 'consolidator':
        consolidation = """
LOCAL CONSOLIDATION OVERRIDE: the consolidator owns campaign-wide duplication and shared-recipe extraction.
Reuse a correct helper already available in this checkout. When sharing would require a new helper or a broad
existing-caller refactor, keep this card correct and record the concrete sharing proposal in report notes for
the consolidator. Do not independently refactor unrelated callers solely to satisfy sharing, and do not block
correct card code solely because that extraction is deferred to consolidation. Keywords still belong in shared
mechanics from their first use. Correctness, rules, safety and test sufficiency remain blocking requirements;
deferring sharing never excuses a broken helper or an engine gap. Final publication waits for consolidation.
"""
    if job["stage"] == "confirm":
        from review_confirmation import compact_prompt
        return compact_prompt(folder, json.loads(job["source"])["name"]) + consolidation
    command = f"python3 {HERE / 'review_lane.py'} --root {root}"
    return f"""You are a Terra worker in the card pipeline, reviewing code written by a DIFFERENT author.
Work on the current card and the existing callers its fixes require; do not pick another new card.
{consolidation}
Read {folder / 'packet.md'} first: full Oracle text, rulings,
current code/content/shared definitions, scenarios and author evidence. Derive expected behavior from Oracle first.
Once per context read .claude/skills/add-cards/review.md and the non-negotiables, Where code goes and Scenarios
sections of its SKILL.md, plus write-game-script/SKILL.md. On later cards reuse those standards; read only
card-specific rule/helper definitions needed. Keep other reads bounded and purposeful.

USER OVERRIDES the older review procedure: use Terra; YOU may fix card logic, scenarios, citations and shared
mechanics. A fresh Terra context will confirm code changes. Do not hand easy fixes back to another author.
Necessary fixes to affected existing callers and their scenarios are within this assignment: declare them all
in affected_cards and run their required gates. Do not block solely because a shared fix needs those caller fixes.
The mtgish generation lane's read-only-code restriction does not apply to this review/fix stage.
Existing author histories and publication branches are preserved; do not edit a commit stack.
No git mutations, pushes, GitHub, network, subagents, engine/core/client/harness/tool changes. Do not run the full
corpus or workspace suite. No --release. The driver owns commits. Local cache/build environment is set.
Use existing target/selection/keyword/effect helpers and the never list. New shared mechanics require a real
second printed card, checked existing callers, and declaration of all affected cards. An engine gap needs a
concrete reachable case, not a hypothetical concern. Never weaken an expectation to pass.

Client scope follows add-cards/SKILL.md step 8: existing prompts, actions and state need no browser test.
Correcting a caption or selecting an already-rendered choice kind does not by itself add a client capability.
Verify wording and structured choice metadata with focused engine probes and inspect the existing presenter;
do not block on a missing .NET SDK solely for those edits. A genuinely new client capability still needs its
client change and browser evidence, and stays blocked here. Do not claim an unrun browser check passed.

Review every printed line, target, trigger, amount, duration, ordering, LKI, ruling and citation; check LOOKs,
overlap, client implications and sufficiency. Fix what is needed. No new corner-case checklist without a reason.
Commands: `{command} tool cr.py 701.20e`, `{command} corpus --card "CARD" --brief`,
`{command} check` (driver gates, focused corpus/coverage/mutation/layers). `corpus` always builds current source.
Play the focused corpus and necessary manual meaning probes, then finish your report and return. The driver
automatically runs full coverage/mutation/layer gates AFTER you return, and resumes you only for failures.
Do not run an initial full check before review/fixes, and do not poll successful gate runs or rerun unchanged gates.
Use `check` yourself only to diagnose a gate failure returned by the driver. Address surviving mutants with
actual expectations or give a precise equivalence argument per survivor. Driver never treats a completed run
as all mutants caught. Never claim a pending machine check passed; report only evidence you actually ran.
Serialize source edits, probes and checks: never mutate source while a check is running.
Use {folder / 'scratch'} for temporary probes; restore every temporary break. For a manual break, save the
original bytes, change ONE decision, show the scenario fails, restore, and show it passes. Record its evidence.
During confirmation, inspect the exact fix diff against the full packet; make NO source edits. Confirm what the
fix decides and whether its probes catch it. Do not redo the original review or rerun unchanged machine gates.

Write {folder / 'review.json'} as JSON. `pass` means this review/fix is ready for the driver's next stage;
it does NOT certify your own code fix independently. The driver will queue fresh confirmation automatically.
Use `checkpoint` ONLY for unfinished work that needs another context, never merely to request confirmation.
{{"decision":"pass|blocked|checkpoint", "reason":"", "validation":[{{"oracle":"...","code":"...","agrees":true}}],
"findings":[{{"file":"...","line":1,"what":"...","resolution":"..."}}],
"fixes":[{{"file":"...","kind":"logic|scenario|citation|comment|sharing","what":"...","proof":"..."}}],
"affected_cards":["current card", "every other existing card changed or shared caller affected"],
"equivalent_mutants":[{{"mutant":"exact key", "why":"why no game can distinguish it"}}],
"rust_mutation_tests":{{"Card Name":["cantrip-cards::integration_test_target"]}},
"manual_breaks":[{{"change":"...","scenario":"...","fails_broken":true,"passes_restored":true,"evidence":"log"}}],
"generator_findings":["recipe improvements so future generation keeps these fixes"],
"client":"existing interactions / concrete gap", "notes":""}}
No pass with unresolved blockers or insufficient evidence. If context grows large, write checkpoint notes and
return; fresh context resumes these same files. Finish the current card and return, never pick the next yourself.
Optional rust_mutation_tests maps an affected card to changed card/mechanics/script integration test targets. Only
Rust integration tests in the script crate are allowed; its harness implementation remains out of scope. A new
card's tests should use its filename prefix; coordinator-declared card_test_paths can retain an existing fixture file.
The driver
can credit a surviving scenario mutant only if an actual Rust assertion fails under that exact source mutation
and the test passes after restoration. A compiler failure never counts. Tests must already pass their baseline.
"""


def current(root):
    path = os.environ.get("REVIEW_LANE_CURRENT")
    if not path:
        raise RuntimeError("this command is only for the assigned reviewer environment")
    return read(path)


def allowed_changes(tree, name):
    paths = git(tree, "diff", "HEAD", "--name-only").splitlines()
    paths += git(tree, "ls-files", "--others", "--exclude-standard").splitlines()
    own, slug = names(name)
    manifests = {}
    workspace = None
    for package in ('cantrip-cards','cantrip-mechanics'):
        path=f'crates/{package}/Cargo.toml'
        if path not in paths:
            continue
        before=tomllib.loads(git(tree,'show','HEAD:'+path))
        after=tomllib.loads((tree/path).read_text())
        old=before.pop('dev-dependencies',{})
        new=after.pop('dev-dependencies',{})
        if before != after or any(new.get(k) != v for k,v in old.items()):
            raise RuntimeError(f'only additive local test dependencies are allowed in {path}')
        if workspace is None:
            workspace=tomllib.loads((tree/'Cargo.toml').read_text())['workspace']['dependencies']
        added=set(new)-set(old)
        for dependency in added:
            definition=workspace.get(dependency,{})
            if new[dependency] != {'workspace':True} or not isinstance(definition,dict) or 'path' not in definition or not (tree/definition['path']).resolve().is_relative_to(tree.resolve()):
                raise RuntimeError(f'only existing local workspace test dependencies are allowed: {dependency}')
        manifests[package]=added
    if 'Cargo.lock' in paths:
        old=tomllib.loads(git(tree,'show','HEAD:Cargo.lock'))
        new=tomllib.loads((tree/'Cargo.lock').read_text())
        a=old.pop('package',[]);b=new.pop('package',[])
        if old!=new or len(a)!=len(b):
            raise RuntimeError('lockfile changes must only add the declared local test dependencies')
        for prior,current in zip(a,b):
            if prior==current:
                continue
            p=dict(prior);c=dict(current);left=p.pop('dependencies',[]);right=c.pop('dependencies',[])
            if p!=c or p.get('name') not in manifests or set(right)!=set(left)|manifests[p['name']]:
                raise RuntimeError('lockfile changes must only add the declared local test dependencies')
    for path in paths:
        if not (path == f"content/mtg/cards/{slug}.json" or
                path == 'Cargo.lock' and bool(manifests) or
                path in {f'crates/{p}/Cargo.toml' for p in manifests} or
                path.startswith("crates/cantrip-cards/src/cards/") and path.endswith(".rs") or
                path.startswith("crates/cantrip-cards/tests/") and path.endswith(".rs") or
                path.startswith("crates/cantrip-mechanics/src/") and path.endswith(".rs") or
                path.startswith("crates/cantrip-mechanics/tests/") and path.endswith(".rs") or
                path.startswith("crates/cantrip-script/tests/") and path.endswith(".rs") or
                path.startswith("scenarios/") and path.endswith(".cantrip")):
            raise RuntimeError(f"out-of-scope edit {path}; preserved for inspection")
    return sorted(set(paths))


def native_mutant_check(lane, card, mutant, tests, allowed_tests, folder):
    """Credit an assertion failure on the exact mutant, never a failed build."""
    path=lane.card_file(card)
    if not path.is_file() or not isinstance(mutant.get('line'),int):return None
    original=path.read_bytes()
    lines=original.decode().splitlines(keepends=True)
    index=mutant.get('line',0)-1
    was,changed=mutant.get('was'),mutant.get('now')
    if (not isinstance(was,str) or not isinstance(changed,str) or '\n' in changed or
        not 0<=index<len(lines) or lines[index].rstrip('\r\n')!=was):
        return None
    ending='\r\n' if lines[index].endswith('\r\n') else '\n' if lines[index].endswith('\n') else ''
    lines[index]=changed+ending
    for target in tests:
        if not isinstance(target,str):continue
        parts=tuple(target.split('::'))
        if parts not in allowed_tests:continue
        package,test=parts
        tag=hashlib.sha256((card+'\0'+mutant['key']+'\0'+target).encode()).hexdigest()[:16]
        broken_log=folder/f'native-mutant-{tag}-broken.log'
        restored_log=folder/f'native-mutant-{tag}-restored.log'
        command=('cargo','test','--profile','checked','-p',package,'--test',test)
        started=time.monotonic()
        try:
            path.write_text(''.join(lines))
            code,out=lane.shell(*command,env=lane.full_environment())
            broken_log.write_text(out)
        finally:
            path.write_bytes(original)
        if not (code!=0 and re.search(r'test result: FAILED\..*\b[1-9]\d* failed',out)):
            continue
        code,restored=lane.shell(*command,env=lane.full_environment())
        restored_log.write_text(restored)
        if code==0 and re.search(r'[1-9]\d* passed; 0 failed',restored):
            return dict(test=target,mutant=mutant['key'],was=was,now=changed,
                        source_sha256=hashlib.sha256(original).hexdigest(),
                        seconds=round(time.monotonic()-started,2),broken_log=str(broken_log),restored_log=str(restored_log))
    return None


def checks(root):
    assignment = current(root)
    tree, folder = Path(assignment["tree"]), Path(assignment["folder"])
    review_evidence.archive_previous_checks(folder)
    lane = load_lane(root, tree, folder / "state")
    name = assignment["name"]
    lane.card_dir(name).mkdir(parents=True, exist_ok=True)
    report = read(folder / "review.json") if (folder / "review.json").exists() else {}
    changed = allowed_changes(tree, name)
    changed_stems = {Path(p).stem for p in changed
                     if p.startswith("crates/cantrip-cards/src/cards/") and p.endswith(".rs")}
    affected = list(dict.fromkeys(canonical_typography(n) for n in [name, *report.get("affected_cards", [])]))
    lane.focus_cards(affected)
    declared = {names(n)[0] for n in affected}
    if changed_stems - declared:
        result=dict(at=now(),ok=False,seconds=0,diff_digest=work_digest(tree),gates=[dict(gate='affected cards',ok=False,seconds=0,
                    detail="declare every changed card in affected_cards before check: " + str(changed_stems - declared))])
        result, _ = review_evidence.freeze_checks(folder, result)
        save(folder/'checks.json',result)
        return result
    initial_digest = work_digest(tree)
    rows, started = [], time.monotonic()
    def edited(card):
        stem, slug = names(card)
        return card == name or stem in changed_stems or any(
            p.startswith(f"scenarios/{slug}-") or p == f"content/mtg/cards/{slug}.json" for p in changed)
    edited_cards = [card for card in affected if edited(card)]
    shared_callers = [card for card in affected if not edited(card)]
    ordered = edited_cards + shared_callers
    card_index = 0

    def progress(label, state='running'):
        save(folder/'checks-live.json',dict(state=state,at=now(),started_seconds=started,card=name,
             gate=label,card_index=card_index,total_cards=len(ordered),edited_cards=len(edited_cards),
             phase='edited checks' if card_index <= len(edited_cards) else 'caller regressions'))

    def record(label, operation, ok=None):
        progress(label)
        at = time.monotonic()
        code, out = operation()
        good = (code == 0) and (ok(out) if ok else True)
        log = folder / (re.sub(r"[^a-zA-Z0-9_-]", "_", label) + ".log")
        log.write_text(out)
        rows.append(dict(gate=label, ok=bool(good), seconds=round(time.monotonic()-at, 2), log=str(log), tail=out[-2000:] if not good else out[-300:]))
        print(label, "PASS" if good else "FAIL", f"{rows[-1]['seconds']}s", flush=True)
        return good

    integration_tests = {(p.split('/')[1], Path(p).stem) for p in changed
                         if p.startswith(('crates/cantrip-mechanics/tests/', 'crates/cantrip-cards/tests/', 'crates/cantrip-script/tests/')) and p.endswith('.rs')}
    for package, test in sorted(integration_tests):
        record(package.removeprefix('cantrip-')+' test '+test,
               lambda p=package,t=test: lane.shell('cargo','test','--profile','checked','-p',
               p,'--test',t,env=lane.full_environment()),
               lambda out: bool(re.search(r'[1-9]\d* passed; 0 failed',out)))
    for card_index, card in enumerate(ordered, 1):
        is_edited = card in edited_cards
        if not is_edited and any(not row['ok'] for row in rows):
            break  # Repair edited cards before the wider regression sweep.
        first_row = len(rows)
        # run_corpus uses build_corpus/source_state, unlike the generation-only CLI's prebuilt shortcut.
        if not record(card + " corpus", lambda c=card: lane.run_corpus(c),
                      lambda out: bool(re.search(r"[1-9]\d* pass, 0 fail, 0 known bugs?", out)) and bool(re.search(r"0 need cards", out))):
            if is_edited:
                continue
            break
        if is_edited:
            record(card + " oracle", lambda c=card: lane.tool("new-card.py", c, "--no-write"),
                   lambda out: "matches Scryfall" in out and "DIFFERS" not in out)
            record(card + " lint", lambda c=card: lane.tool("card-lint.py", c), lambda out: ": NEVER:" not in out)
            files = [*([lane.card_file(card)] if lane.card_file(card).exists() else []), *lane.card_scenarios(card)]
            record(card + " citations", lambda f=files: lane.tool("cr.py", "check", "--show", *f))
        record(card + " coverage", lambda c=card: lane.tool("card-coverage.py", c))
        if not is_edited:
            # Shared callers still play and measure their scenarios. Their
            # unchanged card decisions do not need another mutation review.
            continue
        if card != name and names(card)[0] not in changed_stems:
            # Scenario-only repairs keep their corpus/coverage/citation gates.
            # They do not require re-reviewing every unchanged card decision.
            continue
        if any(not row['ok'] for row in rows[first_row:]):
            continue  # A failed basic gate needs a repair before mutation work.
        if lane.card_file(card).is_file() and '#[cfg(test)]' in lane.card_file(card).read_text():
            record(card+' Rust tests', lambda s=names(card)[0]: lane.shell('cargo','test','--profile','checked',
                   '-p','cantrip-cards','--lib',s+'::tests',env=lane.full_environment()),
                   lambda out: bool(re.search(r'[1-9]\d* passed; 0 failed',out)))
            if not rows[-1]['ok']:
                continue
        lane.card_dir(card).mkdir(parents=True, exist_ok=True)
        gates = {card: lane.Gates(card, False)}
        at = time.monotonic()
        progress(card+' mutation')
        survivors = lane.mutation_gates([card], gates).get(card, [])
        declared_equiv = {e.get("mutant"): e.get("why") for e in report.get("equivalent_mutants", []) if isinstance(e, dict)}
        unresolved = [s for s in survivors if not lane.known_equivalent(s) and not declared_equiv.get(s["key"])]
        native_catches=[]
        mapping=report.get('rust_mutation_tests',{})
        tests=mapping.get(card,[]) if isinstance(mapping,dict) else []
        if isinstance(tests,list) and tests and all(row['ok'] for row in rows):
            for mutant in list(unresolved):
                proof=native_mutant_check(lane,card,mutant,tests,integration_tests,folder)
                if proof:
                    unresolved.remove(mutant);native_catches.append(proof)
                    rows.append(dict(gate=card+' native mutant '+mutant['key'],ok=True,seconds=proof['seconds'],
                                     log=proof['broken_log'],proof=proof))
                    rows.append(dict(gate=card+' native mutant restored '+mutant['key'],ok=True,seconds=0,
                                     log=proof['restored_log']))
        # A successful summary alone cannot prove which changes were played. Bind the raw run and per-change results
        # before a later check or a reused worker directory replaces them; freeze_checks snapshots nested log fields.
        artifacts = []
        missing = []
        for filename, kind in [('mutate.txt', 'raw mutation output'), ('mutants.json', 'per-mutant results')]:
            path = lane.card_dir(card) / filename
            if path.is_file():
                artifacts.append(dict(kind=kind, log=str(path), sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
            else:
                missing.append(str(path))
        rows.append(dict(gate=card+" mutation", ok=not unresolved and not gates[card].failures() and not missing,
                         seconds=round(time.monotonic()-at, 2), survivors=survivors, unresolved=unresolved,
                         native_catches=native_catches,details=gates[card].results,
                         artifacts=artifacts, missing_artifacts=missing))
    shared = [p for p in changed if p.startswith("crates/cantrip-mechanics/")]
    if shared:
        record("shared citations", lambda: lane.tool("cr.py", "check", "--show", *shared))
    source = lane.card_file(name).read_text() if lane.card_file(name).exists() else ""
    if ("statics:" in source or "granted_static" in source) and all(r["ok"] for r in rows):
        record("layers", lambda: lane.shell("cargo", "test", "--profile", "checked", "-q", "-p", "cantrip-script", "--test", "layer_pass_cases", env=lane.full_environment()))
    final_digest = work_digest(tree)
    rows.append(dict(gate="stable source", ok=initial_digest == final_digest, seconds=0))
    result = dict(at=now(), head=git(tree, "rev-parse", "HEAD"), diff_digest=final_digest,
                  initial_digest=initial_digest,
                  ok=bool(rows) and all(r["ok"] for r in rows), seconds=round(time.monotonic()-started, 2), gates=rows)
    result, receipt = review_evidence.freeze_checks(folder, result)
    save(folder / "checks.json", result)
    print('Immutable check receipt:', receipt, flush=True)
    progress('complete', 'passed' if result['ok'] else 'failed')
    return result


def work_digest(tree):
    # Content includes untracked scenarios, not just the tracked diff.
    paths = sorted(set(git(tree, "ls-files").splitlines() + git(tree, "ls-files", "--others", "--exclude-standard").splitlines()))
    h = hashlib.sha256()
    for path in paths:
        if path.startswith(("crates/", "content/", "scenarios/")) or path in ("Cargo.toml", "Cargo.lock"):
            file = tree / path
            h.update(path.encode() + b"\0" + (file.read_bytes() if file.exists() else b"<deleted>"))
    return h.hexdigest()


def completed_review_matches(tree, report_path, previous):
    evidence = previous.get("completed_review")
    return bool(isinstance(evidence, dict) and report_path.is_file() and
                evidence.get("report_sha256") == hashlib.sha256(report_path.read_bytes()).hexdigest() and
                evidence.get("source_digest") == work_digest(tree))


def commit_candidate(tree, source, parent, paths):
    """Build final prerequisite + Card commits once; never repeatedly rebase a stack."""
    original_paths = git(tree, "diff-tree", "--no-commit-id", "--name-only", "-r", source["commit"]).splitlines()
    own, slug = names(source["name"])
    owned = {f"crates/cantrip-cards/src/cards/{own}.rs", f"content/mtg/cards/{slug}.json"}
    owned.update(p for p in original_paths + paths if p.startswith(f"scenarios/{slug}-"))
    # A test executing the new card cannot compile on the helper-only branch.
    owned.update(p for p in original_paths + paths
                 if any(p.startswith(f'crates/{package}/tests/{own}_') or
                        p == f'crates/{package}/tests/{own}.rs'
                        for package in ('cantrip-mechanics', 'cantrip-cards', 'cantrip-script')))
    for path in source.get('card_test_paths', []):
        parts = Path(path).parts
        if len(parts) != 4 or parts[0] != 'crates' or parts[1] not in ('cantrip-cards','cantrip-mechanics','cantrip-script') or parts[2] != 'tests' or not path.endswith('.rs'):
            raise RuntimeError('card_test_paths may name only Rust integration test files')
        owned.add(path)
    shared = [p for p in paths if p not in owned]
    if paths:
        git(tree, "add", "--", *paths)
    final_tree = git(tree, "write-tree")
    if shared:
        with tempfile.TemporaryDirectory() as temporary:
            env = dict(os.environ, GIT_INDEX_FILE=str(Path(temporary) / "index"))
            git(tree, "read-tree", parent, env=env)
            git(tree, "add", "--", *shared, env=env)
            shared_tree = git(tree, "write-tree", env=env)
        parent = git(tree, "commit-tree", shared_tree, "-p", parent, "-m", f"Mechanics: prerequisites for {source['name']} review fixes")
    author = git(tree, "show", "-s", "--format=%an%n%ae%n%aI", source["commit"]).splitlines()
    message = git(tree, "show", "-s", "--format=%B", source["commit"])
    _, separator, body = message.partition("\n")
    message = f"Card: {source['name']}" + (separator + body if separator else "")
    if source.get("issue") and f"Closes #{source['issue']}" not in message:
        message += f"\n\nCloses #{source['issue']}"
    env = dict(os.environ, GIT_AUTHOR_NAME=author[0], GIT_AUTHOR_EMAIL=author[1], GIT_AUTHOR_DATE=author[2])
    candidate = git(tree, "commit-tree", final_tree, "-p", parent, "-m", message, env=env)
    git(tree, "reset", "--soft", candidate)
    clean(tree)
    return candidate


def report_valid(report, name):
    return (report.get("decision") == "pass" and bool(report.get("validation")) and
            all(v.get("agrees") is True for v in report["validation"]) and
            canonical_typography(name) in [canonical_typography(n) for n in report.get("affected_cards", [])] and bool(report.get("client")))


def park_blocked(tree, source, folder, paths, result):
    """Retain a blocked card on its own named branch, freeing the worker without losing edits."""
    if not paths:
        return
    result.setdefault("prepared_parent", git(tree, "rev-parse", "HEAD^"))
    git(tree, "add", "--", *paths)
    (folder / "blocked.patch").write_text(git(tree, "diff", "HEAD") + "\n")
    git(tree, "commit", "-m", f"Review WIP: {source['name']} (blocked; not publishable)")
    result["wip_commit"] = git(tree, "rev-parse", "HEAD")
    clean(tree)


def worker(root, slot, once=False, resume=None):
    cfg = config(root)
    tree = Path(cfg["reviewers"][slot]["tree"])
    folder = Path(root) / "workers" / slot
    folder.mkdir(parents=True, exist_ok=True)
    # Lock the slot across supervisors and invocations, independently of the DB job lease.
    with (folder / "slot.lock").open("w") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        if not cfg.get("enabled", True):
            return
        if not resume:
            with connect(root) as db:
                if preserve_running_assignment(db, slot, folder):
                    return
        maintain_caches(root, cfg, begin_target=tree / 'target')
        session, context_cards, previous_usage, session_role = None, 0, None, None
        last_finished = None
        stamp = driver_stamp()
        while True:
            cfg = config(root)
            if not cfg.get("enabled", True):
                return
            if cfg['reviewers'][slot].get('draining') and not resume:
                save(folder / 'live.json', dict(state='drained', worker=slot, at=now(), context_cards=context_cards))
                return
            role = cfg["reviewers"][slot].get("role", "all")
            if role != session_role:
                session, context_cards, previous_usage, session_role = None, 0, None, role
            if not resume:
                clean(tree)
                reload_at_boundary(root, slot, tree, 'review_lane.py', stamp)
            with connect(root) as db:
                if resume:
                    db.execute("BEGIN IMMEDIATE")
                    row = db.execute("SELECT * FROM jobs WHERE id=? AND worker=? AND status IN ('blocked','infrastructure')", (resume, slot)).fetchone()
                    if not row:
                        raise RuntimeError("only the owning slot may explicitly resume its stopped job")
                    job = dict(row)
                    if git(tree, "branch", "--show-current") != job["branch"]:
                        raise RuntimeError("resume branch does not match the preserved job")
                    db.execute("UPDATE jobs SET status='running',started=?,finished=NULL WHERE id=?", (now(), resume))
                    event(db, resume, slot, "resumed", {})
                    db.commit()
                else:
                    job = claim(db, slot, role)
            if not job:
                save(folder / "live.json", dict(state="waiting", worker=slot, at=now(), context_cards=context_cards))
                if once:
                    return
                time.sleep(cfg.get("poll_seconds", 5))
                continue
            job["worker"] = slot
            started = time.monotonic()
            with connect(root) as db:
                branch=parent=None
                report, attempts = {}, []
                try:
                    resume_ready=False
                    if resume:
                        previous = json.loads(job["result"]) if job.get("result") else {}
                        branch, parent, source = job["branch"], previous.get("prepared_parent") or git(tree, "rev-parse", "HEAD^"), json.loads(job["source"])
                        cached_report=job_folder(root,job,job['stage'])/'review.json'
                        resume_ready=(previous.get('error','').startswith('declare every changed card in affected_cards') or
                                      completed_review_matches(tree,cached_report,previous)) and cached_report.exists() and report_valid(read(cached_report),source['name'])
                        resume = None
                    else:
                        branch, parent, source = prepare(root, tree, job)
                    state = job_folder(root, job, job["stage"]) / "state"
                    lane = load_lane(root, tree, state)
                    packet_dir = packet(root, tree, job, source, lane)
                    assignment = dict(tree=str(tree), folder=str(packet_dir), name=source["name"])
                    save(folder / "current.json", assignment)
                    os.environ["REVIEW_LANE_CURRENT"] = str(folder / "current.json")
                    original_environment = lane.environment
                    lane.environment = lambda: dict(original_environment(), REVIEW_LANE_CURRENT=str(folder / "current.json"), CARGO_BUILD_JOBS="8")
                    dedicated_verifier = cfg["reviewers"][slot].get("role") == "confirm"
                    if job["stage"] == "confirm" and not dedicated_verifier:
                        session, context_cards, previous_usage = None, 0, None
                    if session and context_budget.idle_restart(last_finished):
                        event(db, job["id"], slot, "context_rotation",
                              dict(reason="idle", idle_seconds=time.monotonic() - last_finished,
                                   cards=context_cards))
                        db.commit()
                        session, context_cards, previous_usage = None, 0, None
                    cap = cfg.get("cards_per_context", 20)
                    try:
                        from context_budget import session_context, budget_restart
                        metrics = session_context(session) if session else {}
                    except ImportError:
                        metrics = {}
                        budget_restart = lambda m, limit: False
                    if session and ((cap and context_cards >= cap) or budget_restart(metrics, cfg.get("context_tokens", 320000))):
                        event(db, job["id"], slot, "context_rotation", dict(cards=context_cards, context=metrics))
                        db.commit()
                        session, context_cards, previous_usage = None, 0, None
                    backend = lane.Codex(cfg["review_model"], cfg.get("effort", "medium"))
                    text = prompt(root, tree, packet_dir, job)
                    save(folder / "live.json", dict(state="running", worker=slot, card=source["name"], stage=job["stage"], at=now(), context_cards=context_cards, model=backend.model))
                    (packet_dir / "prompt.md").write_text(text)
                    report, evidence, used = {}, {}, lane.Usage()
                    attempt, rotations = 0, 0
                    deadline = time.monotonic() + cfg.get("timeout_seconds", 3600)
                    worker_seconds = 0
                    attempts = []
                    recovered_completed_report=resume_ready
                    marker = packet_dir / "precompact-stop.json"
                    while attempt <= cfg.get("repairs", 3):
                        marker.unlink(missing_ok=True)
                        at = time.monotonic()
                        prior_session, prior_usage = session, previous_usage
                        if resume_ready:
                            code,used,resume_ready=0,lane.Usage(),False
                        else:
                            code, session, used = lane.spawn(backend, {"dirs": [packet_dir], "context_marker": str(marker)}, text, session,
                                                             max(1, deadline-time.monotonic()), packet_dir / "worker.jsonl")
                        worker_seconds += time.monotonic()-at
                        delta = used.record(backend.model)
                        delta.pop("usd_api_equivalent", None)  # measurement is tokens, not an inferred subscription bill
                        if session == prior_session and prior_usage:
                            for k in ("input", "cached", "output"):
                                delta[k] = max(0, delta[k] - getattr(prior_usage, k))
                        previous_usage = used
                        attempts.append(dict(session=session, seconds=round(time.monotonic()-at, 2), usage=delta,
                                             context=session_context(session)))
                        from context_budget import precompact_stopped
                        stopped = precompact_stopped(marker, session)
                        report = read(packet_dir / "review.json") if (packet_dir / "review.json").exists() else {}
                        if (stopped or report.get("decision") == "checkpoint") and rotations < 3 and time.monotonic() < deadline:
                            rotations += 1
                            event(db, job["id"], slot, "context_rotation", dict(same_card=True, precompact=stopped, session=session))
                            db.commit()
                            session, context_cards, previous_usage = None, 0, None
                            text = ("Continue this SAME card in fresh context. All partial fixes remain on disk. Read the packet, "
                                    "review.json checkpoint and any notes, validate existing work and finish it. Do not discard edits.\n\n" + prompt(root, tree, packet_dir, job))
                            continue
                        if code != 0 or not report or report.get("decision") in ("blocked", "checkpoint"):
                            break
                        if job["stage"] == "confirm":
                            if git(tree, "status", "--porcelain"):
                                raise RuntimeError("confirmation worker edited source; preserved for inspection")
                            break
                        evidence = read(packet_dir / "checks.json") if (packet_dir / "checks.json").exists() else {}
                        if report_valid(report, source["name"]):
                            if not evidence.get("ok") or evidence.get("diff_digest") != work_digest(tree):
                                evidence = checks(root)
                            if evidence.get("ok"):
                                break
                        text = "Finish this same card. Fix the report and checks; do not pick another. " + json.dumps(dict(report_valid=report_valid(report, source["name"]), checks=evidence))
                        if not session:text+='\n\n'+prompt(root,tree,packet_dir,job)
                        attempt += 1
                    usage = {k: sum(a["usage"].get(k, 0) for a in attempts) for k in ("input", "cached", "output", "turns")}
                    paths = allowed_changes(tree, source["name"])
                    result = dict(model=backend.model, session=session, seconds=round(time.monotonic()-started, 2), prepared_parent=parent,
                                  worker_seconds=round(worker_seconds, 2), validation_seconds=evidence.get("seconds"),
                                  rotations=rotations, attempts=attempts,
                                  recovered_completed_report=recovered_completed_report,
                                  usage=usage, report=report, checks=evidence, branch=branch, changed=paths)
                    try:
                        result["context"] = session_context(session)
                    except (NameError, OSError, ValueError):
                        pass
                    context_cards += 1
                    if not report:
                        result['error']='Worker produced no review report; inspect worker.jsonl for the infrastructure failure.'
                    if job["stage"] == "confirm":
                        verdict = ("infrastructure" if code != 0 or not report else
                                   "passed" if report_valid(report, source["name"]) else "blocked")
                        finish(db, job, verdict, result)
                        if verdict == 'infrastructure':
                            save(packet_dir/'result.json',result)
                            save(folder/'live.json',dict(state='infrastructure',worker=slot,card=source['name'],at=now(),error=result.get('error','Confirmation process failed')))
                            return
                        if not dedicated_verifier:
                            session, context_cards, previous_usage = None, 0, None
                    elif code == 0 and report_valid(report, source["name"]) and evidence.get("ok"):
                        # Stage all allowed new scenarios too so the confirmation sees their exact diff.
                        if paths:
                            git(tree, "add", "--", *paths)
                        (packet_dir / "fixes.diff").write_text(git(tree, "diff", "HEAD") + "\n")
                        candidate = commit_candidate(tree, source, parent, paths)
                        logic_changed = any(p.endswith(".rs") or p.startswith("content/") for p in paths)
                        finish(db, job, "queued" if logic_changed else "passed", result, candidate=candidate,
                               branch=branch, fixed_by=slot, stage="confirm" if logic_changed else "review")
                    else:
                        # A blocker is preserved, never discarded or called a failed card because of capacity.
                        status = "infrastructure" if code != 0 or not report else "blocked"
                        park_blocked(tree, source, packet_dir, paths, result)
                        finish(db, job, status, result, branch=branch)
                        if status == "infrastructure":
                            print("Infrastructure stop; card work retained on", branch, flush=True)
                            return
                    save(packet_dir / "result.json", result)
                    last_finished = time.monotonic()
                    print(source["name"], job["stage"], result["seconds"], "seconds", flush=True)
                except Exception as error:
                    branch=git(tree,'branch','--show-current')
                    detail=dict(error=str(error),traceback=traceback.format_exc(),tree=str(tree),branch=branch)
                    if parent is not None:detail['prepared_parent']=parent
                    if attempts:
                        detail.update(model=cfg['review_model'], attempts=attempts, report=report,
                                      seconds=round(time.monotonic()-started,2),
                                      usage={k:sum(a['usage'].get(k,0) for a in attempts) for k in ('input','cached','output','turns')})
                        report_path=job_folder(root,job,job['stage'])/'review.json'
                        if report_valid(report,job['name']) and report_path.exists():
                            detail['completed_review']=dict(report_sha256=hashlib.sha256(report_path.read_bytes()).hexdigest(),source_digest=work_digest(tree))
                    save(folder/'live.json',dict(state='infrastructure',worker=slot,card=job['name'],at=now(),error=str(error)))
                    finish(db, job, "infrastructure", detail,branch=branch)
                    raise
            if once:
                return


def pipeline_flow(db, cfg):
    flow = {name: dict(cards=0, workers=0, active_workers=0, rounds=[0,0,0,0])
            for name in ('intake','implementation','review','fix','verify','ready','publishing','published','blocked')}
    pool = sum(w.get('role','all') in ('review','all') for w in cfg.get('reviewers',{}).values())
    flow['implementation']['workers'] = len(cfg.get('implementers',{}))
    flow['review']['workers'] = flow['fix']['workers'] = pool
    flow['verify']['workers'] = sum(w.get('role','all') in ('confirm','all') for w in cfg.get('reviewers',{}).values())
    publishing = {}
    if db.execute("SELECT 1 FROM sqlite_master WHERE type='table' AND name='publish_cards'").fetchone():
        publishing = {r['job']:r['status'] for r in db.execute('SELECT p.job,b.status FROM publish_cards p JOIN publish_batches b ON b.number=p.batch')}
    active = {name:set() for name in flow}
    for row in db.execute("""SELECT id,source,status,stage,worker FROM jobs
            WHERE status!='superseded' AND rowid IN
            (SELECT MAX(rowid) FROM jobs GROUP BY name)"""):
        source=json.loads(row['source']);round_number=source.get('review_round',0)
        completed=round_number+(row['stage']=='confirm' or row['status'] in ('passed','blocked','infrastructure'))
        completed=min(3,max(completed,source.get('verifier_rejections',0)))
        state=row['status']
        if state=='passed':
            pub=publishing.get(row['id'])
            node='published' if pub=='published' else 'publishing' if pub else 'ready'
        elif state in ('blocked','infrastructure'):
            node='blocked'
        elif row['stage']=='confirm':
            node='verify'
        else:
            node='fix' if round_number else 'review'
            completed=min(3,round_number)
        flow[node]['cards']+=1;flow[node]['rounds'][completed]+=1
        if state=='running' and row['worker']:active[node].add(row['worker'])
    queued_names=set();implementation=[]
    if db.execute("SELECT 1 FROM sqlite_master WHERE type='table' AND name='implementation_jobs'").fetchone():
        implementation=[dict(r) for r in db.execute('SELECT name,revision,status,worker FROM implementation_jobs')]
        for row in implementation:
            if row['status']=='queued':queued_names.add(row['name'])
            elif row['status']=='running':
                flow['implementation']['cards']+=1;flow['implementation']['rounds'][0]+=1
                active['implementation'].add(row['worker'])
    intake_path=cfg.get('implementation_queue')
    held=0
    if intake_path and Path(intake_path).exists():
        queue=read(intake_path)
        indexed={r['name']:r for r in implementation if r['revision']==queue.get('generator_commit')}
        for card in queue.get('cards',[]):
            if card.get('status')=='pending' and card['name'] not in indexed:queued_names.add(card['name'])
            if card.get('status') in ('generator_gap','deferred_blocked','needs_generation','needs_reservation'):held+=1
    flow['intake'].update(cards=len(queued_names),queued=len(queued_names),held=held,rounds=[len(queued_names),0,0,0])
    for name, workers in active.items():flow[name]['active_workers']=len(workers)
    return flow


def status(root):
    from usage_costs import pipeline_summary
    from phase_rates import summary as phase_summary
    with connect(root) as db:
        cfg=config(root) if (Path(root)/'config.json').exists() else {}
        flow=pipeline_flow(db,cfg)
        costs=pipeline_summary(db,cfg)
        phase_rates=phase_summary(db,cfg,now())
        counts = [dict(r) for r in db.execute("SELECT stage,status,count(*) AS cards FROM jobs GROUP BY stage,status")]
        active = [dict(r) for r in db.execute("SELECT id,name,stage,worker,started FROM jobs WHERE status='running'")]
        results = [json.loads(r[0]) for r in db.execute("SELECT detail FROM events WHERE kind IN ('passed','queued','blocked','infrastructure')")]
        events = [dict(r) for r in db.execute("SELECT events.*,jobs.name FROM events LEFT JOIN jobs ON jobs.id=events.job ORDER BY events.id DESC LIMIT 40")]
        worker_progress = [dict(r) for r in db.execute("SELECT worker,count(DISTINCT job) AS handled FROM events WHERE kind IN ('passed','queued','blocked','infrastructure') GROUP BY worker")]
        first_claim = db.execute("SELECT min(at) FROM events WHERE kind='claimed'").fetchone()[0]
        last_finish = db.execute("SELECT max(finished) FROM jobs").fetchone()[0]
        producers = []
        implemented_names = []
        if db.execute("SELECT 1 FROM sqlite_master WHERE type='table' AND name='implementation_jobs'").fetchone():
            producers = [dict(r) for r in db.execute("SELECT status,count(*) AS cards FROM implementation_jobs GROUP BY status")]
            implemented_names = [r[0] for r in db.execute("SELECT DISTINCT name FROM implementation_jobs WHERE status='committed'")]
    recent = []
    for entry in events:
        detail = json.loads(entry.pop("detail"))
        entry["detail"] = {k: detail[k] for k in ("stage", "seconds", "worker_seconds", "validation_seconds", "rotations", "context", "error") if k in detail}
        if "stage" not in entry["detail"] and detail.get("branch"):
            leaf=detail["branch"].rsplit("/",1)[-1]
            phase=re.match(r'^(review|confirm)(?:-|$)',leaf)
            entry["detail"]["stage"] = phase[1] if phase else leaf
        recent.append(entry)
    secs = sorted(r["seconds"] for r in results if "seconds" in r)
    end = now() if any(c["status"] in ("queued", "running") for c in counts) else last_finish
    elapsed = max(0, (dt.datetime.fromisoformat(end) - dt.datetime.fromisoformat(first_claim)).total_seconds()) if end and first_claim else 0
    ready = sum(c["cards"] for c in counts if c["status"] == "passed")
    return dict(at=now(), counts=counts, active=active, implementation=producers,flow=flow,costs=costs,phase_rates=phase_rates,
                implementation_committed_names=implemented_names, completed_attempts=len(secs),
                worker_progress=worker_progress, review_elapsed_seconds=elapsed,
                review_rate_per_hour=3600*ready/elapsed if elapsed else None,
                mean_seconds=round(sum(secs)/len(secs), 1) if secs else None,
                median_seconds=secs[len(secs)//2] if secs else None, recent=recent)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=DEFAULT_ROOT)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("sync")
    sub.add_parser("status")
    p = sub.add_parser("worker")
    p.add_argument("slot")
    p.add_argument("--once", action="store_true")
    p.add_argument("--resume", help="explicitly resume this slot's preserved blocked/infrastructure job")
    p = sub.add_parser("retry")
    p.add_argument("job")
    p.add_argument("--reason", required=True, help="concrete change that resolves the previous blocker")
    p.add_argument("--new-base", help="explicitly migrate parked work to this currently validated local snapshot")
    p = sub.add_parser("repair-confirmation")
    p.add_argument("job")
    p.add_argument("--folder", required=True, type=Path, help="fresh candidate-bound repair report and evidence")
    p.add_argument("--validated-tree", required=True, type=Path)
    p.add_argument("--reason", required=True)
    p = sub.add_parser("correct")
    p.add_argument("job")
    p.add_argument("--reason", required=True, help="concrete new finding and tested shared engine fix")
    p.add_argument("--new-base", required=True, help="the currently validated local snapshot with that fix")
    sub.add_parser("check")
    p = sub.add_parser("tool")
    p.add_argument("tool")
    p.add_argument("args", nargs=argparse.REMAINDER)
    p = sub.add_parser("corpus")
    p.add_argument("args", nargs=argparse.REMAINDER)
    sub.add_parser("watch")
    args, extra = parser.parse_known_args()
    if args.command == "sync":
        print("enqueued", sync(args.root))
    elif args.command == "status":
        print(json.dumps(status(args.root), indent=2))
    elif args.command == "worker":
        worker(args.root, args.slot, args.once, args.resume)
    elif args.command == "retry":
        print("requeued", retry_stopped(args.root, args.job, args.reason, new_base=args.new_base))
    elif args.command == "repair-confirmation":
        print("confirmation repair", repair_confirmation(args.root, args.job, args.folder,
                                                       args.validated_tree, args.reason))
    elif args.command == "correct":
        print("correction", correct_approved(args.root, args.job, args.reason, args.new_base))
    elif args.command == "watch":
        stamp=driver_stamp()
        while True:
            if driver_stamp()!=stamp:
                os.execv(sys.executable,[sys.executable,'-u',str(HERE/'review_lane.py'),'--root',str(args.root),'watch'])
            sync(args.root)
            if config(args.root).get('handoff_enabled',True):
                handoff_verifier_findings(args.root)
            cfg = config(args.root)
            maintain_caches(args.root, cfg)
            if cfg.get('workflow') == 'consolidate_then_publish':
                import integration_lane
                integration_lane.snapshot(args.root)
            save(args.root / "status.json", status(args.root))
            time.sleep(config(args.root).get("poll_seconds", 5))
    elif args.command == "check":
        return 0 if checks(args.root)["ok"] else 1
    else:
        assignment = current(args.root)
        lane = load_lane(args.root, Path(assignment["tree"]), Path(assignment["folder"]) / "state")
        if args.command == "tool":
            code, out = lane.tool(args.tool, *args.args)
        else:
            # argparse's REMAINDER and flags are combined in source order below.
            flags = sys.argv[sys.argv.index("corpus") + 1:]
            # The common runner includes decks from --file scratch probes in the focused registry.
            return lane.cmd_corpus(flags)
        print(out)
        return code
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
