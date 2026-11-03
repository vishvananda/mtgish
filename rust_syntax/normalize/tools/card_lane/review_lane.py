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
import sqlite3
import subprocess
import sys
import tempfile
import time
import traceback

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


def connect(root):
    root = Path(root)
    root.mkdir(parents=True, exist_ok=True)
    db = sqlite3.connect(root / "queue.sqlite", timeout=60, factory=Database)
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
    return db


def event(db, job, worker, kind, detail):
    db.execute("INSERT INTO events(job,worker,at,kind,detail) VALUES(?,?,?,?,?)",
               (job, worker, now(), kind, json.dumps(detail)))


def enqueue(db, source, priority=0):
    key = hashlib.sha256((source["name"] + "\0" + source["commit"]).encode()).hexdigest()[:20]
    with db:
        added = db.execute("INSERT OR IGNORE INTO jobs(id,name,source_commit,source,priority,enqueued) VALUES(?,?,?,?,?,?)",
                           (key, source["name"], source["commit"], json.dumps(source), priority, now())).rowcount
    return added


def claim(db, worker):
    # A lease is never expired automatically: a killed worker may have valuable dirty files.
    db.execute("BEGIN IMMEDIATE")
    row = db.execute("SELECT * FROM jobs WHERE status='queued' ORDER BY priority DESC,enqueued,id LIMIT 1").fetchone()
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
        event(db, job["id"], job.get("worker"), status, result)


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
    return file_stem(name), content_slug(name)


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


def load_lane(root, tree, state):
    cfg = config(root)
    os.environ.update(CARD_LANE=cfg["lane_root"], CARD_LANE_CANTRIP=str(tree),
                      CARD_LANE_STATE=str(state), CARD_LANE_TARGET=str(tree / "target"),
                      CARD_LANE_BRANCH=git(tree, "branch", "--show-current"), CARGO_BUILD_JOBS="8",
                      CARD_LANE_FOCUSED="1" if cfg.get("focused_builds", True) else "0")
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
    return [line.split(" ", 1)[0] for line in rows if not line.split(" ", 1)[1].startswith("Card:")]


def prepare(root, tree, job):
    clean(tree)
    cfg, source = config(root), json.loads(job["source"])
    branch = f"review/{Path(root).name}/{job['id']}/{job['stage']}"
    if job["stage"] == "confirm":
        git(tree, "checkout", "-b", branch, job["candidate"])
        return branch, git(tree, "rev-parse", "HEAD^"), source
    git(tree, "checkout", "-b", branch, cfg["base"])
    for commit in prerequisite_commits(tree, source):
        git(tree, "cherry-pick", commit)
    parent = git(tree, "rev-parse", "HEAD")
    git(tree, "cherry-pick", source["commit"])
    return branch, parent, source


def packet(root, tree, job, source, lane):
    folder = Path(root) / "jobs" / job["id"] / job["stage"]
    folder.mkdir(parents=True, exist_ok=True)
    author = Path(source["packet"])
    for name in ("reference.md", "shared.md", "prior-art.md", "report.json", "check.json", "mutate.txt", "test-points.json"):
        if (author / name).exists() and not (folder / ("author-" + name)).exists():
            (folder / ("author-" + name)).write_bytes((author / name).read_bytes())
    source_path = lane.card_file(source["name"])
    current = source_path.read_text() if source_path.exists() else "(characteristics only)"
    (folder / "current-shared.md").write_text(lane.shared_code(current) if source_path.exists() else "")
    scenarios = lane.card_scenarios(source["name"])
    prior = Path(source["campaign"]) / "publication" / (source["producer"] + "-review.json")
    hints = None
    if prior.exists():
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
    if job["stage"] == "confirm":
        original = Path(root) / "jobs" / job["id"] / "review"
        body += ["## Fix report\n" + (original / "review.json").read_text(),
                 "## Exact fixes to confirm\n```diff\n" + (original / "fixes.diff").read_text() + "\n```"]
    (folder / "packet.md").write_text("\n\n".join(body))
    return folder


def prompt(root, tree, folder, job):
    command = f"python3 {HERE / 'review_lane.py'} --root {root}"
    return f"""You are a Terra worker in the card pipeline, reviewing code written by a DIFFERENT author.
Work only on the current card. Read {folder / 'packet.md'} first: full Oracle text, rulings,
current code/content/shared definitions, scenarios and author evidence. Derive expected behavior from Oracle first.
Then read .claude/skills/add-cards/review.md and the non-negotiables, Where code goes and Scenarios sections
of its SKILL.md, plus write-game-script/SKILL.md once per context. Keep other reads bounded and purposeful.

USER OVERRIDES the older review procedure: use Terra; YOU may fix card logic, scenarios, citations and shared
mechanics. A fresh Terra context will confirm code changes. Do not hand easy fixes back to another author.
The mtgish generation lane's read-only-code restriction does not apply to this review/fix stage.
Existing author histories and publication branches are preserved; do not edit a commit stack.
No git mutations, pushes, GitHub, network, subagents, engine/core/client/harness/tool changes. Do not run the full
corpus or workspace suite. No --release. The driver owns commits. Local cache/build environment is set.
Use existing target/selection/keyword/effect helpers and the never list. New shared mechanics require a real
second printed card, checked existing callers, and declaration of all affected cards. An engine gap needs a
concrete reachable case, not a hypothetical concern. Never weaken an expectation to pass.

Review every printed line, target, trigger, amount, duration, ordering, LKI, ruling and citation; check LOOKs,
overlap, client implications and sufficiency. Fix what is needed. No new corner-case checklist without a reason.
Commands: `{command} tool cr.py 701.20e`, `{command} corpus --card "CARD" --brief`,
`{command} check` (driver gates, focused corpus/coverage/mutation/layers). `corpus` always builds current source.
Run check after edits; read {folder / 'checks.json'}. Address surviving mutants with actual expectations or
give a precise equivalence argument per survivor. Driver never treats a mutation run finishing as all caught.
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
"manual_breaks":[{{"change":"...","scenario":"...","fails_broken":true,"passes_restored":true,"evidence":"log"}}],
"generator_findings":["recipe improvements so future generation keeps these fixes"],
"client":"existing interactions / concrete gap", "notes":""}}
No pass with unresolved blockers or insufficient evidence. If context grows large, write checkpoint notes and
return; fresh context resumes these same files. Finish the current card and return, never pick the next yourself.
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
    for path in paths:
        if not (path == f"content/mtg/cards/{slug}.json" or
                path.startswith("crates/cantrip-cards/src/cards/") and path.endswith(".rs") or
                path.startswith("crates/cantrip-mechanics/src/") and path.endswith(".rs") or
                path.startswith("scenarios/") and path.endswith(".cantrip")):
            raise RuntimeError(f"out-of-scope edit {path}; preserved for inspection")
    return sorted(set(paths))


def checks(root):
    assignment = current(root)
    tree, folder = Path(assignment["tree"]), Path(assignment["folder"])
    lane = load_lane(root, tree, folder / "state")
    name = assignment["name"]
    lane.card_dir(name).mkdir(parents=True, exist_ok=True)
    report = read(folder / "review.json") if (folder / "review.json").exists() else {}
    changed = allowed_changes(tree, name)
    changed_stems = {Path(p).stem for p in changed if p.startswith("crates/cantrip-cards/")}
    affected = list(dict.fromkeys([name, *report.get("affected_cards", [])]))
    lane.focus_cards(affected)
    declared = {names(n)[0] for n in affected}
    if changed_stems - declared:
        raise RuntimeError("declare every changed card in affected_cards before check: " + str(changed_stems - declared))
    rows, started = [], time.monotonic()

    def record(label, operation, ok=None):
        at = time.monotonic()
        code, out = operation()
        good = (code == 0) and (ok(out) if ok else True)
        log = folder / (re.sub(r"[^a-zA-Z0-9_-]", "_", label) + ".log")
        log.write_text(out)
        rows.append(dict(gate=label, ok=bool(good), seconds=round(time.monotonic()-at, 2), log=str(log), tail=out[-2000:] if not good else out[-300:]))
        print(label, "PASS" if good else "FAIL", f"{rows[-1]['seconds']}s", flush=True)
        return good

    for card in affected:
        # run_corpus uses build_corpus/source_state, unlike the generation-only CLI's prebuilt shortcut.
        if not record(card + " corpus", lambda c=card: lane.run_corpus(c),
                      lambda out: bool(re.search(r"[1-9]\d* pass, 0 fail, 0 known bugs?", out)) and bool(re.search(r"0 need cards", out))):
            break
        record(card + " oracle", lambda c=card: lane.tool("new-card.py", c, "--no-write"),
               lambda out: "matches Scryfall" in out and "DIFFERS" not in out)
        record(card + " lint", lambda c=card: lane.tool("card-lint.py", c), lambda out: ": NEVER:" not in out)
        files = [*([lane.card_file(card)] if lane.card_file(card).exists() else []), *lane.card_scenarios(card)]
        record(card + " citations", lambda f=files: lane.tool("cr.py", "check", "--show", *f))
        record(card + " coverage", lambda c=card: lane.tool("card-coverage.py", c))
        lane.card_dir(card).mkdir(parents=True, exist_ok=True)
        gates = {card: lane.Gates(card, False)}
        at = time.monotonic()
        survivors = lane.mutation_gates([card], gates).get(card, [])
        declared_equiv = {e.get("mutant"): e.get("why") for e in report.get("equivalent_mutants", []) if isinstance(e, dict)}
        unresolved = [s for s in survivors if not lane.known_equivalent(s) and not declared_equiv.get(s["key"])]
        rows.append(dict(gate=card+" mutation", ok=not unresolved and not gates[card].failures(),
                         seconds=round(time.monotonic()-at, 2), survivors=survivors, unresolved=unresolved,
                         details=gates[card].results))
    source = lane.card_file(name).read_text() if lane.card_file(name).exists() else ""
    if ("statics:" in source or "granted_static" in source) and all(r["ok"] for r in rows):
        record("layers", lambda: lane.shell("cargo", "test", "--profile", "checked", "-q", "-p", "cantrip-script", "--test", "layer_pass_cases", env=lane.full_environment()))
    result = dict(at=now(), head=git(tree, "rev-parse", "HEAD"), diff_digest=work_digest(tree),
                  ok=bool(rows) and all(r["ok"] for r in rows), seconds=round(time.monotonic()-started, 2), gates=rows)
    save(folder / "checks.json", result)
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


def commit_candidate(tree, source, parent, paths):
    """Build final prerequisite + Card commits once; never repeatedly rebase a stack."""
    original_paths = git(tree, "diff-tree", "--no-commit-id", "--name-only", "-r", source["commit"]).splitlines()
    own, slug = names(source["name"])
    owned = {f"crates/cantrip-cards/src/cards/{own}.rs", f"content/mtg/cards/{slug}.json"}
    owned.update(p for p in original_paths + paths if p.startswith(f"scenarios/{slug}-"))
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
            name in report.get("affected_cards", []) and bool(report.get("client")))


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
        session, context_cards, previous_usage = None, 0, None
        while True:
            cfg = config(root)
            if not cfg.get("enabled", True):
                return
            if not resume:
                clean(tree)
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
                    job = claim(db, slot)
            if not job:
                save(folder / "live.json", dict(state="waiting", worker=slot, at=now(), context_cards=context_cards))
                if once:
                    return
                time.sleep(cfg.get("poll_seconds", 5))
                continue
            job["worker"] = slot
            started = time.monotonic()
            with connect(root) as db:
                try:
                    if resume:
                        previous = json.loads(job["result"]) if job.get("result") else {}
                        branch, parent, source = job["branch"], previous.get("prepared_parent") or git(tree, "rev-parse", "HEAD^"), json.loads(job["source"])
                        resume = None
                    else:
                        branch, parent, source = prepare(root, tree, job)
                    state = Path(root) / "jobs" / job["id"] / job["stage"] / "state"
                    lane = load_lane(root, tree, state)
                    packet_dir = packet(root, tree, job, source, lane)
                    assignment = dict(tree=str(tree), folder=str(packet_dir), name=source["name"])
                    save(folder / "current.json", assignment)
                    os.environ["REVIEW_LANE_CURRENT"] = str(folder / "current.json")
                    original_environment = lane.environment
                    lane.environment = lambda: dict(original_environment(), REVIEW_LANE_CURRENT=str(folder / "current.json"), CARGO_BUILD_JOBS="8")
                    if job["stage"] == "confirm":
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
                    marker = packet_dir / "precompact-stop.json"
                    while attempt <= cfg.get("repairs", 3):
                        marker.unlink(missing_ok=True)
                        at = time.monotonic()
                        prior_session, prior_usage = session, previous_usage
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
                        if code != 0 or report.get("decision") in ("blocked", "checkpoint"):
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
                        attempt += 1
                    usage = {k: sum(a["usage"].get(k, 0) for a in attempts) for k in ("input", "cached", "output", "turns")}
                    paths = allowed_changes(tree, source["name"])
                    result = dict(model=backend.model, session=session, seconds=round(time.monotonic()-started, 2), prepared_parent=parent,
                                  worker_seconds=round(worker_seconds, 2), validation_seconds=evidence.get("seconds"),
                                  rotations=rotations, attempts=attempts,
                                  usage=usage, report=report, checks=evidence, branch=branch, changed=paths)
                    try:
                        result["context"] = session_context(session)
                    except (NameError, OSError, ValueError):
                        pass
                    context_cards += 1
                    if job["stage"] == "confirm":
                        verdict = "passed" if code == 0 and report_valid(report, source["name"]) else "blocked"
                        finish(db, job, verdict, result)
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
                        status = "infrastructure" if code != 0 else "blocked"
                        park_blocked(tree, source, packet_dir, paths, result)
                        finish(db, job, status, result, branch=branch)
                        if status == "infrastructure":
                            print("Infrastructure stop; card work retained on", branch, flush=True)
                            return
                    save(packet_dir / "result.json", result)
                    print(source["name"], job["stage"], result["seconds"], "seconds", flush=True)
                except Exception as error:
                    finish(db, job, "infrastructure", dict(error=str(error), traceback=traceback.format_exc(), tree=str(tree)))
                    raise
            if once:
                return


def status(root):
    with connect(root) as db:
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
            entry["detail"]["stage"] = detail["branch"].rsplit("/", 1)[-1]
        recent.append(entry)
    secs = sorted(r["seconds"] for r in results if "seconds" in r)
    end = now() if any(c["status"] in ("queued", "running") for c in counts) else last_finish
    elapsed = max(0, (dt.datetime.fromisoformat(end) - dt.datetime.fromisoformat(first_claim)).total_seconds()) if end and first_claim else 0
    ready = sum(c["cards"] for c in counts if c["status"] == "passed")
    return dict(at=now(), counts=counts, active=active, implementation=producers,
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
    elif args.command == "watch":
        while True:
            sync(args.root)
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
            report = Path(assignment["folder"]) / "review.json"
            affected = read(report).get("affected_cards", []) if report.exists() else []
            selected = [flags[flags.index("--card") + 1]] if "--card" in flags else [assignment["name"]]
            lane.focus_cards(list(dict.fromkeys([*selected, *affected])))
            # A historical memo says a source state built successfully, not that the current binary
            # still contains it after a temporary break. Let Cargo select/rebuild the exact source.
            code, out = lane.shell("cargo", "run", "--profile", "cards", "-q", "-p", "cantrip-script", "--bin", "corpus", "--", *flags)
        print(out)
        return code
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
