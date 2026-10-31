#!/usr/bin/env python3
"""Turn the Cantrip cards the generator writes whole into card commits, one card at a time, whoever does the checking.

Only a card with no gap comes here: the generator's closed table of recipes (`ast cantrip-files`, card_file.rs) wrote it
from shared mechanics alone, so it needs no mechanic and no engine work. Its code and content are final. The worker
validates them against the Oracle text and writes the scenarios, and a card that is wrong is a generator bug, not an edit.

What a person or an agent uses, inside a session (the `card-lane` skill says how):

    card_lane.py next                                    open the next card: its code, placeholders and packet
    card_lane.py corpus --card "Card Name" --brief       play its scenarios (any flag of Cantrip's corpus)
    card_lane.py check "Card Name" [--fast]              the gates a commit must pass, the same for everyone
    card_lane.py commit "Card Name"                      every gate, then exactly one `Card:` commit
    card_lane.py abandon "Card Name" --outcome X --reason WHY    keep the work as a patch, clean the tree
    card_lane.py tool cr.py check --show FILE            a Cantrip maintenance tool, in the lane's environment
    card_lane.py status [--json]                         where the queue stands, and what each card cost

What sets it up, and what runs it unattended:

    card_lane.py setup                                   the worktree, the generator, the tools; safe to run again
    card_lane.py queue [--format pioneer]                the cards with no gap, the format's list first
    card_lane.py run --backend codex --model gpt-6-luna  a loop that starts a worker per card

Nothing above `run` knows about models. `run` starts a worker and then calls the same steps, and `check` decides, never
a worker's report. A subagent or an interactive session needs only the skill, at .claude/skills/card-lane/SKILL.md.

Nothing here pushes or posts. Commits stay on the lane's local Cantrip branch. See README.md beside this file.
"""

import argparse
import collections
import datetime
import json
import os
import re
import runpy
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
TOOLS = HERE.parent
sys.path.insert(0, str(TOOLS))
sys.dont_write_bytecode = True

MTGISH = HERE.parents[3]
LANE = Path(os.environ.get("CARD_LANE", Path.home() / "card-lane"))
CANTRIP = Path(os.environ.get("CARD_LANE_CANTRIP", LANE / "cantrip"))
STATE = Path(os.environ.get("CARD_LANE_STATE", LANE / "state"))
BRANCH = os.environ.get("CARD_LANE_BRANCH", "card-lane/pioneer-pilot")
TARGET = LANE / "targets" / "cantrip"
AST = LANE / "targets" / "ast" / "debug" / "ast"
GEN = LANE / "gen"
SCRYFALL = LANE / "scryfall"
BIN = LANE / "bin"
SKILL = MTGISH / ".claude" / "skills" / "card-lane"
QUEUE = STATE / "queue.json"

# What a card may add. Anything else is out of this lane: a card that needs it is a gap, not a bigger commit. The card's
# file and content come from tools, so of these a worker writes only scenarios; the `generated` gate holds the rest.
CONTENT_DIR = "content/mtg/cards"
CARDS_DIR = "crates/cantrip-cards/src/cards"
SCENARIOS_DIR = "scenarios"

# USD per million tokens: ordinary input, cached input, output. API-equivalent list prices as vizier's
# scripts/card_agent_usage.py records them; a ChatGPT login is not billed by the token.
RATES = {"gpt-6-luna": (.10, .01, .50), "gpt-5.6-luna": (.20, .02, 1.20), "gpt-6-sol": (2, .20, 10), "gpt-6.1-sol": (2, .10, 10),
         "gpt-6-astra": (10, 1, 50)}

MUTATION_LIMIT = 40  # about 16 s each; a card past this is reported as partly tested, not passed silently
PRIOR_ART, PRIOR_ART_CHARS = 3, 3000  # scenarios in a packet's prior-art.md, and the most of any one
STOPS = ("generator_bug", "engine_gap", "needs_review")  # what a worker can report instead of `done`; no gate can undo these


def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds")


def environment():
    """What every tool runs under, the driver's and the worker's alike. CANTRIP_VERIFY checks every rollback."""
    env = {k: v for k, v in os.environ.items() if k in ("HOME", "USER", "LANG", "LC_ALL", "TMPDIR", "TERM", "RUSTUP_HOME", "CARGO_HOME")}
    env.update(PATH=os.pathsep.join([str(BIN), str(Path.home() / ".cargo" / "bin"), "/usr/local/bin", "/usr/bin", "/bin"]),
               CARGO_TARGET_DIR=str(TARGET), CARGO_NET_OFFLINE="true", LLVM_COV=shutil.which("llvm-cov") or "llvm-cov",
               LLVM_PROFDATA=shutil.which("llvm-profdata") or "llvm-profdata", PYTHONDONTWRITEBYTECODE="1", CANTRIP_VERIFY="1",
               CARD_LANE=str(LANE), CARD_LANE_CANTRIP=str(CANTRIP), CARD_LANE_STATE=str(STATE), CARD_LANE_BRANCH=BRANCH)
    return env


def shell(*command, cwd=CANTRIP, check=False, timeout=None, env=None):
    """A command's (exit status, stdout and stderr together), in the lane's environment."""
    done = subprocess.run([str(c) for c in command], cwd=cwd, env=env or environment(), stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                          text=True, encoding="utf-8", errors="replace", timeout=timeout)
    if check and done.returncode != 0:
        sys.exit(f"{' '.join(str(c) for c in command)}\n{done.stdout[-4000:]}")
    return done.returncode, done.stdout


def git(*args, check=True):
    return shell("git", "-C", CANTRIP, *args, check=check)[1]


def tool(name, *args):
    """A tools/maintenance script through this file's `tool` command, so it reads the lane's Scryfall cache."""
    return shell(sys.executable, Path(__file__).resolve(), "tool", name, *args)


def cmd_tool(args):
    """Runs a maintenance script in this process. scryfall.py keeps its cache in the main checkout's target/, which the
    lane leaves alone, so the one thing changed is where the module looks."""
    os.environ.update(environment())
    os.chdir(CANTRIP)
    sys.path.insert(0, str(CANTRIP / "tools" / "maintenance"))
    import scryfall
    scryfall.CACHE = str(SCRYFALL)
    sys.argv = [args.name, *args.rest]
    if args.name == "scryfall.py":
        return scryfall.main()  # run as a script it would be a second copy of the module, with the cache unchanged
    runpy.run_path(str(CANTRIP / "tools" / "maintenance" / args.name), run_name="__main__")


# ---- Names ----------------------------------------------------------------------------------------------------------

def names_of(name):
    """(the card's file stem, its content slug, the generated oracle constant), by Cantrip's own rules."""
    import cantrip_candidates as cc
    constant = re.sub(r"[^A-Z0-9]+", "_", _ascii(name).replace("'", "").upper()).strip("_")
    return cc.file_stem(name), cc.content_slug(name), constant


def _ascii(name):
    import unicodedata
    return "".join(c for c in unicodedata.normalize("NFKD", name) if not unicodedata.combining(c))


def card_dir(name):
    return STATE / "cards" / names_of(name)[0]


def card_file(name):
    return CANTRIP / CARDS_DIR / (names_of(name)[0] + ".rs")


def content_file(name):
    return CANTRIP / CONTENT_DIR / (names_of(name)[1] + ".json")


# ---- The queue ------------------------------------------------------------------------------------------------------

def load_queue():
    if not QUEUE.exists():
        sys.exit(f"no queue at {QUEUE}: run `card_lane.py queue` first")
    return json.loads(QUEUE.read_text(encoding="utf-8"))


def save_queue(queue):
    """Whole file, atomically: a killed run leaves the last state, never half of one."""
    partial = QUEUE.with_suffix(f".{os.getpid()}.part")
    partial.write_text(json.dumps(queue, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    os.replace(partial, QUEUE)


def implemented():
    """Every card name content/ already has, faces included."""
    have = set()
    for path in (CANTRIP / "content").glob("*/cards/*.json"):
        card = json.loads(path.read_text(encoding="utf-8-sig"))
        have.add(card["name"])
        for face in ("backFace", "adventure", "omen"):
            if isinstance(card.get(face), dict) and card[face].get("name"):
                have.add(card[face]["name"])
        for half in card.get("halves") or []:
            have.add(half.get("name"))
    return have


def scryfall_cards():
    code, out = shell(sys.executable, "-c", """
import gzip, glob, json, sys
path = sorted(glob.glob(sys.argv[1] + '/oracle-cards-*.jsonl.gz'))[-1]
out = {}
with gzip.open(path) as f:
    for line in f:
        if line.strip():
            d = json.loads(line)
            out[d['name']] = {'layout': d.get('layout'), 'oracle_text': d.get('oracle_text', ''), 'type_line': d.get('type_line', '')}
print(json.dumps(out))
""", SCRYFALL)
    return json.loads(out) if code == 0 else {}


def plain(text):
    """Text compared loosely: curly quotes, dashes and spacing are not what a comparison is after."""
    text = text.replace("’", "'").replace("‘", "'").replace("“", '"').replace("”", '"').replace("−", "-").replace("—", "-")
    return re.sub(r"\s+", " ", text).strip()


def oracle_differs(printed, ours):
    """Whether two Oracle texts say different things. oracle.json brackets a loyalty cost, `[+1]:`, and Scryfall does not."""
    return plain(printed) != plain(re.sub(r"(^|\n)\[([^\]]+)\]:", r"\1\2:", ours))


def cmd_queue(args):
    """Every card with no gap that Cantrip lacks: the format's list first, in its own order (add-cards: cards go in list
    order, never sorted by difficulty), then the rest as the corpus has them. A gap is `ast cantrip-files`'s `gap`: an
    ability, keyword or target with no shared recipe. `backend_capable` is no test here, since that scan inlines whatever
    a card needs. Cards already worked keep their records; a card opened and not finished goes back first."""
    STATE.mkdir(parents=True, exist_ok=True)
    GEN.mkdir(parents=True, exist_ok=True)
    rows_path = GEN / "rows.jsonl"
    data = MTGISH / "data"
    shell(AST, "cantrip-files", data / "mtgish.lines.ron", data / "oracle.json", rows_path, cwd=MTGISH, check=True)
    rows = [json.loads(line) for line in rows_path.open(encoding="utf-8")]
    listed = [c["name"] for c in json.loads((CANTRIP / "docs" / "formats" / f"{args.format}.json").read_text(encoding="utf-8"))["cards"]]
    previous = load_queue() if QUEUE.exists() else {"cards": []}
    recover(previous)
    kept = [c for c in previous["cards"] if c["status"] != "pending"]
    done = {c["name"] for c in kept}
    have, scryfall = implemented(), scryfall_cards()
    oracle = {}
    for entry in json.loads((data / "oracle.json").read_text(encoding="utf-8")):
        for face in entry["cards"]:
            oracle.setdefault(face["name"], face)
    issues = open_issues(args.format)
    old = {c["name"]: c for c in previous["cards"]}
    rows_by_name = {row["name"]: row for row in rows}
    order = [rows_by_name[n] for n in listed if n in rows_by_name]
    if not args.listed_only:
        order += [row for row in rows if row["name"] not in set(listed)]
    cards, skipped, details = [], collections.Counter(), []
    stopped = collections.Counter(rows_by_name[n]["gap"] for n in listed if n in rows_by_name and n not in have and rows_by_name[n]["gap"])
    for row in order:
        name = row["name"]
        if row["gap"] or name in have or name in done:
            continue
        reason = None
        if name not in scryfall:
            reason = "not on Scryfall"
        elif scryfall[name]["layout"] not in args.layouts:
            reason = "layout " + scryfall[name]["layout"]
        elif name in oracle and oracle_differs(scryfall[name]["oracle_text"], oracle[name].get("text", "")):
            reason = "Oracle text differs from mtgish's oracle.json"
            details.append({"name": name, "scryfall": scryfall[name]["oracle_text"], "mtgish": oracle[name].get("text", "")})
        if reason:
            skipped[reason] += 1
            continue
        entry = old.get(name) or {"name": name, "status": "pending", "attempts": 0}
        entry.update(listed=name in listed, issue=issues.get(name))
        cards.append(entry)
    missing = [n for n in listed if n not in have]
    queue = {"format": args.format, "built": now(), "generator_commit": git_head(MTGISH), "cantrip_commit": git("rev-parse", "HEAD").strip(),
             "missing": len(missing), "skipped": dict(skipped), "stopped_by": dict(stopped), "oracle_differences": details, "cards": kept + cards}
    save_queue(queue)
    print(f"{args.format}: {len(listed)} listed, {len(missing)} missing, {sum(c['listed'] for c in cards)} of those have no gap")
    print(f"queued {len(cards)} with no gap: {sum(c['listed'] for c in cards)} listed, {sum(not c['listed'] for c in cards)} others; "
          f"{sum(1 for c in cards if not rows_by_name[c['name']]['rust'])} need no code; {len(kept)} earlier cards kept")
    for reason, n in skipped.most_common(12):
        print(f"  skipped {n}: {reason}")
    for gap, n in stopped.most_common(12):
        print(f"  {args.format} stopped {n}: {gap}")


def git_head(path):
    return shell("git", "-C", path, "rev-parse", "HEAD", cwd=path)[1].strip()


def open_issues(fmt):
    """{card name: issue number} of the format's open card issues, read only. No GitHub, no numbers: a commit can go
    without its `Closes #N` and gain it when a person claims the card."""
    try:
        code, out = shell("gh", "issue", "list", "-R", "cantrip-rs/cantrip-rs", "--label", f"format:{fmt}", "--state", "open", "--limit", "1000",
                          "--json", "number,title", timeout=60, env=dict(environment(), **_gh_env()))
    except subprocess.TimeoutExpired:
        return {}
    if code != 0:
        return {}
    return {i["title"].removeprefix("Card: "): i["number"] for i in json.loads(out)}


def _gh_env():
    return {k: v for k, v in os.environ.items() if k.startswith("GH_") or k in ("XDG_CONFIG_HOME",)}


# ---- Preparing a card -----------------------------------------------------------------------------------------------

class GeneratorBug(Exception):
    """The generator's output cannot go to a worker. No worker may change it, so none is spent on it."""


def generated_row(name):
    """The card's row of `ast cantrip-files`: its file in `rust`, or none for a card of core keywords alone."""
    for line in (GEN / "rows.jsonl").open(encoding="utf-8"):
        row = json.loads(line)
        if row["name"] == name:
            return row
    raise GeneratorBug(f"{name} has no generator row")


def printed_lines(oracle_text):
    """A card's Oracle lines without reminder text, which has no game function (CR 207.2): what validating it covers."""
    lines = (re.sub(r"\s+", " ", re.sub(r"\([^)]*\)", "", line)).strip() for line in oracle_text.split("\n"))
    return [line for line in lines if line]


def definition(path, name):
    """The doc comment and text of a top-level `fn`, `const` or `static` in a file, or None."""
    lines = path.read_text(encoding="utf-8").splitlines()
    for i, line in enumerate(lines):
        if not re.match(rf"\s*(?:pub(?:\([^)]*\))? )?(?:fn|const|static) {re.escape(name)}\b", line):
            continue
        start = i
        while start and lines[start - 1].lstrip().startswith(("///", "#[")):
            start -= 1
        end, depth = i, 0
        while end < len(lines):
            depth += lines[end].count("{") + lines[end].count("(") - lines[end].count("}") - lines[end].count(")")
            if depth <= 0 and lines[end].rstrip().endswith(("}", ";")):
                break
            end += 1
        return "\n".join(lines[start:end + 1])
    return None


def shared_code(source):
    """What a card's file calls, as the shared code defines it. The file's words say what it means to do; these say what it
    does. Without them a worker searches `cantrip-mechanics` for each. A `const` brings the private function it names."""
    mechanics = CANTRIP / "crates" / "cantrip-mechanics" / "src"
    names = re.findall(r"\beffects::(\w+)", source) + re.findall(r"\btargets::(\w+)", source)
    names += re.findall(r"\b(?:damage_target|destroy_target|THIS_ENTERS|THIS_DIES)\b", source)
    out = []
    for name in dict.fromkeys(names):
        for module in ("effects.rs", "targets.rs", "triggers.rs", "lib.rs"):
            text = definition(mechanics / module, name)
            if text is None:
                continue
            out += [f"## {name} ({module})", "", "```rust", text]
            helpers = re.findall(r"\b([a-z]\w+_filter|this_\w+)\b", text) if re.search(r"\bconst [A-Z]", text) else []
            out += [f"\n{more}" for more in map(lambda helper: definition(mechanics / module, helper), helpers) if more]
            out += ["```", ""]
            break
    return "\n".join(out)


def brief(card, driven=False):
    """What a worker needs to know that the skill cannot: which card, and where its things are."""
    name = card["name"]
    stem, slug, _ = names_of(name)
    code = (f"`{CARDS_DIR}/{stem}.rs`, the generator's, read-only" if card_file(name).exists()
            else "none: the content file's core keywords are the whole card")
    lines = [f"# Card: {name}", "",
             f"Follow the card-lane skill, section \"Doing one card\": {SKILL / 'SKILL.md'}", "",
             f"Goal: the generated card is validated against its Oracle text, line by line, and its scenarios prove it: `check \"{name}\" --fast` passes "
             "and `report.json` says `done`. Or `report.json` says why it is wrong. You write scenarios and the report, no code.", "",
             f"- Tools: `python3 {Path(__file__).resolve()} <command>`",
             f"- Cantrip tree, your working directory: `{CANTRIP}`",
             f"- Packet: `{card_dir(name)}`",
             f"- Card file: {code}",
             f"- Content file: `{CONTENT_DIR}/{slug}.json`, from Scryfall, read-only",
             f"- Placeholder scenarios: `{SCENARIOS_DIR}/{stem.replace('_', '-')}-*.cantrip`"]
    if driven:
        lines += ["", "A driver runs the gates and makes the commit. Stop after writing `report.json`; do not run `commit` or `abandon`."]
    return "\n".join(lines) + "\n"


def placeholders(card_name):
    """Scenarios to start from. Those cantrip_scenarios can write from the card's words, or else new-card.py's start,
    one per printed line, each with a failing TODO: a placeholder left in fails `check`."""
    import cantrip_scenarios as cs
    card = json.loads(content_file(card_name).read_text(encoding="utf-8-sig"))
    try:
        scripts = cs.scenarios(card, cs.Pieces(str(CANTRIP / CONTENT_DIR)))
    except cs.Unwritable:
        scripts = []
    written = []
    for script in scripts or []:
        path = CANTRIP / SCENARIOS_DIR / f"{script.slug}.cantrip"
        if not path.exists():
            path.write_text(script.text(), encoding="utf-8", newline="\n")
            written.append(path.name)
    if written:
        return written
    lines = [re.sub(r"\s*\([^)]*\)", "", l).strip() for l in card.get("oracleText", "").split("\n")]
    lines = [l for l in lines if l] or [""]
    for number, line in enumerate(lines[:3], 1):
        code, out = tool("new-card.py", card_name, "--scenario", f"todo-line-{number}")
        if code != 0:
            sys.exit(out[-2000:])
        path = CANTRIP / out.strip().splitlines()[-1].removeprefix("wrote ")
        path.write_text(f"# TODO placeholder for printed line {number}: {line}\n" + path.read_text(encoding="utf-8"), encoding="utf-8", newline="\n")
        written.append(path.name)
    return written


def recipe_of(source):
    """The shared call a card's file is built on, or None."""
    match = re.search(r"\b(effects::\w+|damage_target|destroy_target)\b", source)
    return match[1] if match else None


def prior_art(source, reference):
    """Short scenarios of existing cards, those built on this card's own shared call first, then the cards `new-card.py`
    names. A scenario of the same shape answers most of what a worker would otherwise read the harness for."""
    stems = []
    recipe = recipe_of(source)
    if recipe:
        users = [p for p in (CANTRIP / CARDS_DIR).glob("*.rs") if recipe in p.read_text(encoding="utf-8")]
        stems += [p.stem for p in sorted(users, key=lambda path: path.stat().st_size)[:40]]
    stems += [m[1] for m in re.finditer(r"^\s+.+?\s{2,}crates/cantrip-cards/src/cards/(\w+)\.rs", reference, re.M)]
    out, seen = [], set()
    for stem in stems:
        if len(seen) == PRIOR_ART or stem in seen:
            continue
        scenarios = sorted((CANTRIP / SCENARIOS_DIR).glob(stem.replace("_", "-") + "*.cantrip"), key=lambda path: path.stat().st_size)
        if scenarios:
            seen.add(stem)
            text = scenarios[0].read_text(encoding="utf-8")[:PRIOR_ART_CHARS].rstrip()
            out += [f"`{SCENARIOS_DIR}/{scenarios[0].name}`", "", "```", text, "```", ""]
    return "\n".join(out)


def prepare(card):
    """Everything a worker starts from, in the worktree and the packet. The generator's file goes in as it is, its fields
    on their own lines, which `card-mutate.py` needs: it passes over any line naming `oracle::`. If it cannot stand, no
    worker is started."""
    name = card["name"]
    row = generated_row(name)
    if row["gap"]:
        raise GeneratorBug(f"{name} has a gap: {row['gap']}")
    packet = card_dir(name)
    packet.mkdir(parents=True, exist_ok=True)
    for stale in ("report.json", "check.json", "work.patch"):
        (packet / stale).unlink(missing_ok=True)
    code, out = tool("new-card.py", name)
    (packet / "reference.md").write_text(out, encoding="utf-8")
    if code != 0 or not content_file(name).exists():
        raise RuntimeError(f"new-card.py wrote no content file for {name}:\n{out[-1500:]}")
    shutil.copyfile(content_file(name), packet / "content.json")
    source = row["rust"] or ""
    if source:
        card_file(name).write_text(source, encoding="utf-8", newline="\n")
    else:
        card_file(name).unlink(missing_ok=True)
    problems = style_problems(name, source) if source else []
    if problems:
        raise GeneratorBug("; ".join(problems))
    (packet / "shared.md").write_text(shared_code(source), encoding="utf-8")
    (packet / "prior-art.md").write_text(prior_art(source, out), encoding="utf-8")
    points = parse_test_points(out)
    write_test_points(name, points)
    oracle_text = json.loads(content_file(name).read_text(encoding="utf-8-sig")).get("oracleText", "")
    (packet / "report.json").write_text(json.dumps(report_skeleton(points, parse_rulings(out), printed_lines(oracle_text)), indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    scenarios = placeholders(name)
    code, out = run_corpus(name)
    (packet / "first-build.txt").write_text(out, encoding="utf-8")
    if re.search(r"^error", out, re.M) or "could not compile" in out:
        raise GeneratorBug(f"the generated file does not compile: {out[-500:]}")
    (packet / "prompt.md").write_text(brief(card), encoding="utf-8")
    return {"scenarios": scenarios}


def run_corpus(name):
    return shell("cargo", "run", "--profile", "cards", "-q", "-p", "cantrip-script", "--bin", "corpus", "--", "--card", name, "--brief")


def begin(card, queue, who):
    card.update(status="working", attempts=card.get("attempts", 0) + 1, started=now(), began=time.time(), worker=who)
    for key in ("failure", "commit", "failed_gates"):
        card.pop(key, None)
    save_queue(queue)


def conclude(card, queue, outcome):
    if "began" in card:
        card["seconds"] = round(time.time() - card.pop("began"), 1)
    card.update(status=outcome, finished=now())
    (card_dir(card["name"]) / "result.json").write_text(json.dumps(card, indent=1, ensure_ascii=False), encoding="utf-8")
    save_queue(queue)


def cmd_next(args):
    """Opens a card. One card is open at a time, and asking again repeats its brief, so a session that lost its place
    finds it."""
    queue = load_queue()
    open_card = next((c for c in queue["cards"] if c["status"] == "working"), None)
    if open_card:
        print(brief(open_card))
        return 0
    if args.name:
        card = find(queue, args.name)
    else:
        card = next((c for c in queue["cards"] if c["status"] == "pending"), None)
    if card is None:
        print("the queue has no pending card")
        return 1
    if changed_paths():
        sys.exit(f"the worktree {CANTRIP} has changes no card owns: {changed_paths()[:5]}")
    begin(card, queue, args.who)
    try:
        card_dir(card["name"]).mkdir(parents=True, exist_ok=True)
        prepare(card)
    except Exception as error:
        save_patch(card)
        clean_tree()
        card["failure"] = f"{type(error).__name__}: {error}"[:600]
        conclude(card, queue, "generator_bug" if isinstance(error, GeneratorBug) else "error")
        if isinstance(error, GeneratorBug):
            print(f"{card['name']}: generator_bug, no worker needed: {error}")
            return 1
        raise
    save_queue(queue)
    print(brief(card))
    return 0


def find(queue, name):
    for card in queue["cards"]:
        if card["name"].lower() == name.lower():
            return card
    sys.exit(f"{name} is not in the queue")


# ---- The gates ------------------------------------------------------------------------------------------------------

class Gates:
    """Each gate's verdict and time, in the order run. A failed gate's detail is what a worker is sent back with."""

    def __init__(self, name, fast):
        self.name, self.fast, self.results = name, fast, []

    def add(self, gate, ok, detail="", started=None):
        self.results.append({"gate": gate, "ok": bool(ok), "seconds": round(time.time() - started, 1) if started else 0, "detail": detail[-3000:]})
        return ok

    @property
    def ok(self):
        return all(r["ok"] for r in self.results)

    def failures(self):
        return [r for r in self.results if not r["ok"]]

    def text(self):
        out = [f"{'PASS' if r['ok'] else 'FAIL'}  {r['gate']:<12} {r['seconds']:>6.1f}s  {r['detail'].splitlines()[0][:120] if r['detail'] else ''}"
               for r in self.results]
        for r in self.failures():
            out += ["", f"--- {r['gate']}", r["detail"]]
        return "\n".join(out)


def changed_paths():
    """Every path differing from HEAD, untracked included: (status, path)."""
    out = git("status", "--porcelain", "--untracked-files=all")
    return [(line[:2], line[3:].strip('"')) for line in out.splitlines() if line.strip()]


def scope_problems(name):
    """Paths a card may not touch. One new content file, one new card file, new scenarios, an owner's note deleted."""
    stem, slug, _ = names_of(name)
    allowed = {f"{CONTENT_DIR}/{slug}.json", f"{CARDS_DIR}/{stem}.rs"}
    problems = []
    for status, path in changed_paths():
        scenario = path.startswith(SCENARIOS_DIR + "/") and path.endswith(".cantrip") and status == "??"
        note = path == f"docs/card-notes/{stem}.md" and "D" in status
        if not (path in allowed or scenario or note):
            problems.append(f"{status} {path}")
    return problems


def card_scenarios():
    return sorted(CANTRIP / p for s, p in changed_paths() if p.startswith(SCENARIOS_DIR + "/") and p.endswith(".cantrip"))


def generated_problems(name):
    """How the tree's card differs from what the tools wrote. The code and the content are final in this lane."""
    row = generated_row(name)
    path = card_file(name)
    problems = []
    if row["rust"]:
        if not path.exists() or path.read_text(encoding="utf-8") != row["rust"]:
            problems.append(f"{path.name} is not the generator's")
    elif path.exists():
        problems.append(f"{path.name} exists, and this card needs no code")
    copy = card_dir(name) / "content.json"
    if not copy.exists() or copy.read_bytes() != content_file(name).read_bytes():
        problems.append(f"{content_file(name).name} is not what new-card.py wrote")
    if problems:
        problems.append("Put it back. If the card is wrong, the report's decision is `generator_bug`.")
    return problems


def called_texts(source):
    """The printed-text argument of each ability constructor: the last string literal at the call's own depth."""
    code = "\n".join(l for l in source.splitlines() if not l.lstrip().startswith("//"))
    for match in re.finditer(r"\b(ability|triggered|mana_ability|static_ability|activated|loyalty_ability|replacement|prevention)\(", code):
        i, depth, last, start = match.end(), 1, None, match.end()
        while i < len(code) and depth:
            c = code[i]
            if c == '"':
                end = i + 1
                while end < len(code) and code[end] != '"':
                    end += 2 if code[end] == "\\" else 1
                if depth == 1:
                    last = code[i + 1:end]
                i = end
            elif c in "([{":
                depth += 1
            elif c in ")]}":
                depth -= 1
            i += 1
        if last is not None:
            yield last, code.count("\n", 0, start) + 1


def functions(source):
    """(name, doc comment, body lines) of each top-level `fn` of a card file."""
    lines = source.splitlines()
    for i, line in enumerate(lines):
        match = re.match(r"(?:pub(?:\([^)]*\))? )?fn (\w+)", line)
        if not match:
            continue
        j, doc = i - 1, []
        while j >= 0 and lines[j].lstrip().startswith(("///", "#[")):
            if lines[j].lstrip().startswith("///"):
                doc.insert(0, lines[j].lstrip()[3:].strip())
            j -= 1
        end = i
        while end < len(lines) and lines[end].rstrip() != "}" and not (end == i and lines[end].rstrip().endswith("}")):
            end += 1
        yield match[1], " ".join(doc), lines[i + 1:end]


def style_problems(name, source):
    """What the review reads for mechanically: printed text in every ability, doc comments quoting the Oracle sentence
    and citing the rule, and none of the generator's scaffolding left."""
    card = json.loads(content_file(name).read_text(encoding="utf-8-sig"))
    oracle = plain(card.get("oracleText", "")).lower()

    def printed(text):
        """Whether these words are on the card. Oracle's reminder text counts: `Station (...)` is printed."""
        words = plain(re.sub(r"\\u\{([0-9a-fA-F]+)\}", lambda m: chr(int(m[1], 16)), text.replace('\\"', '"'))).lower()
        return words.rstrip(".,;") in oracle

    problems = [] if re.search(r"^\s*///", source, re.M) else ["no doc comment quotes the Oracle text and cites the CR"]
    for text, line in called_texts(source):
        if not printed(text):
            problems.append(f"line {line}: ability text {text!r} is not the card's printed text")
    for pattern, why in ((r"#\[allow\(", "an #[allow] left from the generator"), (r"pub mod token_", "a token defined by hand: use cantrip_mechanics::tokens or a named token"),
                         (r"fn ability_\d+", "generator names (ability_0): name each function for what it does"),
                         (r"\bKeywords::of\(&\[\]\)", "generated characteristics left in"), (r"\.clamp\(", "a clamp: rules values are never cut down (review.md)")):
        if re.search(pattern, source):
            problems.append(why)
    for fn, doc, _ in functions(source):
        if not doc:
            problems.append(f"fn {fn}: no doc comment (quote the Oracle sentence it implements, cite the CR)")
        elif not re.search(r"\bCR \d", doc):
            problems.append(f"fn {fn}: its doc comment cites no rule (CR ...)")
        elif not any(printed(q) for q in re.findall(r'"([^"]+)"', doc)):
            problems.append(f"fn {fn}: its doc comment quotes no sentence of this card's Oracle text")
    return problems


def overlaps(name, source):
    """Other cards with a function body equal to one of this card's, whitespace aside: the mechanical part of the
    review's overlap finding. An exact copy of two statements or more should have been shared."""
    def key(body):
        return re.sub(r"\s+", "", "".join(body))
    mine = {key(b): fn for fn, _, b in functions(source) if len([l for l in b if l.strip().endswith(";")]) >= 2}
    found = []
    if not mine:
        return found
    this = card_file(name)
    for path in (CANTRIP / CARDS_DIR).glob("*.rs"):
        if path == this:
            continue
        for fn, _, body in functions(path.read_text(encoding="utf-8")):
            if key(body) in mine:
                found.append(f"fn {mine[key(body)]} repeats {path.name}: fn {fn}")
    return found


def parse_test_points(out):
    if "=== Test points" not in out:
        return []
    section = out.split("=== Test points", 1)[1].split("\n=== ", 1)[0]
    return [line.strip() for line in section.splitlines() if re.match(r"\s+\[\d+\]", line)]


def parse_rulings(out):
    return re.findall(r"^\s+- \d{4}-\d{2}-\d{2}\s+(.+)$", out, re.M)


def report_skeleton(points, rulings=(), lines=()):
    """The report a worker fills in, every printed line, test point and ruling listed, so its shape is never a thing to
    work out."""
    return {"decision": "todo", "reason": "", "validation": [{"line": line, "code": "", "agrees": None, "why": ""} for line in lines],
            "test_points": [{"id": i, "point": p, "scenario": "", "line": 0} for i, p in enumerate(points, 1)],
            "rulings": [{"ruling": r, "scenario": "", "line": 0, "no_decision": ""} for r in rulings],
            "look_answers": [], "equivalent_mutants": [], "generator_disagreements": [], "notes": ""}


def write_test_points(name, points):
    card_dir(name).mkdir(parents=True, exist_ok=True)
    (card_dir(name) / "test-points.json").write_text(json.dumps([{"id": i, "point": p} for i, p in enumerate(points, 1)], indent=1, ensure_ascii=False), encoding="utf-8")


def report_of(name):
    path = card_dir(name) / "report.json"
    if not path.exists():
        return None
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except ValueError:
        return {"decision": "invalid", "reason": "report.json is not JSON"}


def expectation_problem(pair, label):
    """Why a report's (scenario, line) is no expectation, or None. `line` is a line of the file, counted from 1."""
    name = str(pair.get("scenario", "")).removesuffix(".cantrip")
    path = CANTRIP / SCENARIOS_DIR / f"{name}.cantrip"
    if not name or not path.is_file():
        return f"{label}: no scenario {SCENARIOS_DIR}/{name}.cantrip"
    lines = path.read_text(encoding="utf-8").splitlines()
    try:
        number = int(pair.get("line", 0))
    except (TypeError, ValueError):
        number = 0
    if not 1 <= number <= len(lines):
        return f"{label}: line {pair.get('line')} is not a line of {path.name}, which has {len(lines)}"
    line = lines[number - 1].strip()
    expect = next((i for i, text in enumerate(lines, 1) if text.strip() == "[Expect]"), len(lines))
    if line.startswith(("?", "!")) or (number > expect and line and not line.startswith("#")):
        return None
    return f"{label}: {path.name}:{number} is no expectation: {line[:80]}"


def report_problems(name, points, looks, survivors):
    """The worker's report against what the tools found: every test point paired with a line that can catch it, and every
    ruling with one, or with why the card's own code decides nothing there."""
    report = report_of(name)
    if report is None:
        return [f"no report at {card_dir(name) / 'report.json'}"]
    problems = []
    oracle_text = json.loads(content_file(name).read_text(encoding="utf-8-sig")).get("oracleText", "")
    validated = {plain(str(v.get("line"))): v for v in report.get("validation") or [] if isinstance(v, dict)}
    for line in printed_lines(oracle_text):
        entry = validated.get(plain(line))
        if entry is None or not str(entry.get("code", "")).strip():
            problems.append(f"validation: say what implements this line: {line}")
        elif entry.get("agrees") is not True:
            problems.append(f"validation: this line does not agree, so the decision is `generator_bug`: {line}")
    paired = {p.get("id"): p for p in report.get("test_points") or [] if isinstance(p, dict)}
    for number, point in enumerate(points, 1):
        pair = paired.get(number)
        if pair is None:
            problems.append(f"test point {number} unpaired: {point[:100]}")
            continue
        problem = expectation_problem(pair, f"test point {number}")
        if problem:
            problems.append(problem)
    for entry in report.get("rulings") or []:
        if not isinstance(entry, dict) or len(str(entry.get("no_decision", ""))) >= 20:
            continue
        problem = expectation_problem(entry, f"ruling \"{str(entry.get('ruling'))[:60]}\"")
        if problem:
            problems.append(problem + " (or say in no_decision why the card's own code decides nothing here)")
    answers = " ".join(str(a) for a in report.get("look_answers") or [])
    for key, look in looks:
        if key not in answers:
            problems.append(f"unanswered in look_answers (name `{key}`): {look[:140]}")
    declared = " ".join(str(m) for m in report.get("equivalent_mutants") or [])
    for key, change in survivors:
        if key not in declared:
            problems.append(f"mutant survived, so an expectation is missing, or name `{key}` in equivalent_mutants: {change}")
    return problems


def line_hits(lcov, source_path):
    hits, current = {}, False
    target = os.path.abspath(source_path)
    for line in lcov.splitlines():
        if line.startswith("SF:"):
            current = os.path.abspath(line[3:].strip()) == target
        elif current and line.startswith("DA:"):
            number, count = line[3:].split(",")[:2]
            hits[int(number)] = hits.get(int(number), 0) + int(count)
    return hits


def scenario_liveness(name):
    """Scenarios that execute none of the card's own lines. card-coverage.py judges the card's scenarios together, so one
    that plays nothing of the card hides among them. Run after card-coverage.py, on the instrumented corpus it built."""
    path = card_file(name)
    source = path.read_text(encoding="utf-8") if path.exists() else ""
    has_code = bool(re.search(r"\bfn\s+\w+\s*\(|\|g, item\|", source))
    dead, seen = [], {}
    corpus = TARGET / "llvm-cov-target" / "cards" / "corpus"
    for scenario in card_scenarios():
        text = scenario.read_text(encoding="utf-8")
        actions = text.split("[Actions]", 1)[-1].split("[Expect]", 1)[0]
        if not has_code:
            if name.lower() not in actions.lower():
                dead.append(f"{scenario.name}: no action names {name}")
            continue
        with tempfile.TemporaryDirectory(prefix="card-lane-") as scratch:
            profile = Path(scratch) / "s.profraw"
            env = dict(environment(), LLVM_PROFILE_FILE=str(profile))
            shell(corpus, "--file", scenario, "--brief", env=env)
            merged = Path(scratch) / "s.profdata"
            code, out = shell(env_tool("LLVM_PROFDATA"), "merge", "-sparse", profile, "-o", merged)
            if code != 0:
                dead.append(f"{scenario.name}: no profile ({out[-200:]})")
                continue
            code, lcov = shell(env_tool("LLVM_COV"), "export", "-format=lcov", f"-instr-profile={merged}", corpus, "-sources", path)
            hit = sum(1 for count in line_hits(lcov, path).values() if count)
            seen[scenario.name] = hit
            if hit == 0:
                dead.append(f"{scenario.name}: executed no line of {path.name}")
    return dead, seen


def env_tool(name):
    return environment()[name]


def check(name, fast=False):
    """Every gate that can be run on the tree, cheapest first. `fast` leaves out what takes minutes, so a worker can loop
    on it; a commit always takes the whole of it. The report is checked either way, so that a worker sees what would
    otherwise cost it a repair round."""
    gates = Gates(name, fast)
    started = time.time()
    problems = scope_problems(name)
    gates.add("scope", not problems, "\n".join(problems), started)

    started = time.time()
    problems = generated_problems(name)
    gates.add("generated", not problems, "\n".join(problems), started)

    started = time.time()
    code, out = tool("new-card.py", name, "--no-write")
    differs = [l.strip() for l in out.splitlines() if "DIFFERS" in l]
    gates.add("oracle", code == 0 and not differs and "matches Scryfall" in out, "\n".join(differs) or out[-400:] if code or differs or "matches Scryfall" not in out else "", started)
    points = parse_test_points(out)
    write_test_points(name, points)

    started = time.time()
    scenarios = card_scenarios()
    bad = [p.name for p in scenarios if "-todo" in p.name or "TODO" in p.read_text(encoding="utf-8")]
    bad += [f"{p.name}: says Known bug, so it asserts an engine failure" for p in scenarios if "Known bug" in p.read_text(encoding="utf-8")]
    bad += [f"{p.name}: does not open with what it shows (a # comment)" for p in scenarios if not p.read_text(encoding="utf-8").lstrip().startswith("#")]
    bad += [f"{p.name}: no [Expect]" for p in scenarios if "[Expect]" not in p.read_text(encoding="utf-8")]
    gates.add("scenarios", scenarios and not bad, "\n".join(bad) or ("" if scenarios else "the card has no new scenario"), started)

    path = card_file(name)
    source = path.read_text(encoding="utf-8") if path.exists() else ""
    started = time.time()
    problems = style_problems(name, source) if source else []
    gates.add("style", not problems, "\n".join(problems), started)

    started = time.time()
    code, out = tool("card-lint.py", name)
    never = [l for l in out.splitlines() if ": NEVER:" in l]
    looks = [(f"line {m[1]}", l) for l in out.splitlines() if (m := re.search(r"\.rs:(\d+): LOOK:", l))]
    gates.add("lint", code == 0 and not never, "\n".join(never) or (out[-800:] if code else ""), started)

    started = time.time()
    code, out = tool("cr.py", "check", *([path] if source else []), *scenarios)
    gates.add("citations", code == 0, out[-1200:] if code else "", started)

    started = time.time()
    code, out = run_corpus(name)
    tally = re.findall(r"(\d+) pass, (\d+) fail, (\d+) known bugs?", out)
    ok = code == 0 and tally and int(tally[-1][0]) >= len(scenarios) and tally[-1][1] == "0" and tally[-1][2] == "0"
    gates.add("corpus", ok, ("" if ok else out[max(out.find("  FAIL "), len(out) - 2500):][:2500]), started)
    if not ok:
        return finish(gates)
    warned = re.findall(r"^warning: (.*)\n\s*--> (.*" + re.escape(path.name) + r".*)$", out, re.M) if source else []
    gates.add("warnings", not warned, "\n".join(f"{where}: {what}" for what, where in warned))

    started = time.time()
    code, out = tool("card-coverage.py", name)
    gates.add("coverage", code == 0, out[-1500:] if code else "", started)
    if code != 0:
        return finish(gates)

    started = time.time()
    dead, seen = scenario_liveness(name)
    gates.add("liveness", not dead, "\n".join(dead), started)

    survivors = []
    if source and not fast:
        started = time.time()
        _, listing = tool("card-mutate.py", name, "--list")
        total = re.search(r"(\d+) changes", listing)
        if total and int(total[1]):
            code, out = tool("card-mutate.py", name, "--max", MUTATION_LIMIT)
            (card_dir(name) / "mutate.txt").write_text(out, encoding="utf-8")
            survivors = [(f"line {m[1]}: {m[2]}", f"{m[0].removeprefix('SURVIVED ')} {m[3].strip()} {m[4].strip()}")
                         for m in re.finditer(r"^SURVIVED \S+?:(\d+): (.*)\n(\s+was: .*)\n(\s+now: .*)$", out, re.M)]
            capped = int(total[1]) > MUTATION_LIMIT
            gates.add("mutation", "restored" in out and not capped,
                      (f"{total[1]} possible changes, only {MUTATION_LIMIT} tried\n" if capped else "") + ("" if "restored" in out else out[-800:]), started)
    if "statics:" in source and not fast:
        started = time.time()
        code, out = shell("cargo", "test", "--profile", "checked", "-q", "-p", "cantrip-script", "--test", "layer_pass_cases")
        gates.add("layers", code == 0, out[-1500:] if code else "", started)

    started = time.time()
    found = overlaps(name, source)
    problems = report_problems(name, points, looks + [(o.split(" repeats ")[1].split(":")[0], o) for o in found], survivors)
    report = report_of(name) or {}
    if report.get("decision") not in ("done", None):
        problems.insert(0, f"the worker reported {report.get('decision')}: {report.get('reason', '')}")
    gates.add("report", not problems, "\n".join(problems), started)
    return finish(gates)


def finish(gates):
    path = card_dir(gates.name) / "check.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps({"card": gates.name, "fast": gates.fast, "ok": gates.ok, "at": now(), "gates": gates.results}, indent=1, ensure_ascii=False), encoding="utf-8")
    return gates


def cmd_check(args):
    gates = check(args.name, args.fast)
    print(gates.text())
    print(("fast gates pass" if args.fast else "every gate passes") if gates.ok else f"{len(gates.failures())} gate(s) failed")
    return 0 if gates.ok else 1


# ---- Landing a card, or setting it aside ------------------------------------------------------------------------------

def commit_card(card):
    """One commit, `Card: <Name>`, holding the card's own paths. Refuses anything else in the tree. It carries no
    Co-Authored-By: the lane's commits say who ran them, and the report says which model wrote them."""
    name = card["name"]
    problems = scope_problems(name)
    if problems:
        raise RuntimeError("out of scope: " + "; ".join(problems))
    before = git("rev-parse", "HEAD").strip()
    git("add", "--", *[p for _, p in changed_paths()])
    message = f"Card: {name}\n" + (f"\nCloses #{card['issue']}\n" if card.get("issue") else "")
    code, out = shell("git", "-C", CANTRIP, "commit", "-q", "-m", message)
    if code != 0:
        raise RuntimeError(out[-1000:])
    after = git("rev-parse", "HEAD").strip()
    if git("rev-list", "--count", f"{before}..{after}").strip() != "1" or "co-authored-by" in git("log", "-1", "--format=%B").lower():
        raise RuntimeError("the commit is not one clean card commit")
    return after


def clean_tree():
    """The worktree back to HEAD. Only the lane works here, and a card's work is saved as a patch first."""
    git("reset", "-q", "--hard", "HEAD")
    git("clean", "-fdq", "--", CONTENT_DIR, CARDS_DIR, SCENARIOS_DIR, "docs/card-notes")
    leftover = changed_paths()
    if leftover:
        raise RuntimeError(f"the tree is not clean: {leftover[:5]}")


def save_patch(card):
    """The card's work as a patch, new files included, before the tree is cleaned."""
    paths = [p for _, p in changed_paths()]
    if not paths:
        return
    git("add", "-A", "--", *paths)
    (card_dir(card["name"]) / "work.patch").write_text(git("diff", "--cached", "HEAD"), encoding="utf-8")
    git("reset", "-q", "HEAD")


def cmd_commit(args):
    queue = load_queue()
    card = find(queue, args.name)
    gates = check(card["name"])
    print(gates.text())
    if not gates.ok:
        return 1
    card["commit"] = commit_card(card)
    conclude(card, queue, "committed")
    print(f"committed {card['commit']}")
    return 0


def cmd_abandon(args):
    queue = load_queue()
    card = find(queue, args.name)
    save_patch(card)
    clean_tree()
    card["failure"] = args.reason
    conclude(card, queue, args.outcome)
    print(f"{card['name']}: {args.outcome}; work kept in {card_dir(card['name']) / 'work.patch'}")


# ---- Workers --------------------------------------------------------------------------------------------------------

class Usage:
    """Tokens a run reported, added up over its turns."""

    def __init__(self):
        self.input = self.cached = self.output = self.turns = 0
        self.reported_cost = None

    def add(self, other, cumulative=False):
        """Another run's usage. A `cumulative` backend reports its session's totals so far, so the latest replaces the rest."""
        if not cumulative:
            self.input += other.input
            self.cached += other.cached
            self.output += other.output
        elif other.turns:
            self.input, self.cached, self.output = other.input, other.cached, other.output
        self.turns += other.turns
        if other.reported_cost is not None:
            self.reported_cost = (self.reported_cost or 0) + other.reported_cost

    def cost(self, model):
        if self.reported_cost is not None:
            return round(self.reported_cost, 4)
        rate = RATES.get(model)
        if rate is None:
            return None
        return round(((self.input - self.cached) * rate[0] + self.cached * rate[1] + self.output * rate[2]) / 1e6, 4)

    def record(self, model):
        return {"input": self.input, "cached": self.cached, "output": self.output, "turns": self.turns, "usd_api_equivalent": self.cost(model)}


class Codex:
    """`codex exec`, the way vizier runs it: no user config, no network, one writable tree plus the build and the
    packet, and a long wait so polling a build does not re-read the context. `turn.completed` on a resumed session gives
    the session's totals, not the turn's: its rollout's last `total_token_usage` is the same figure."""
    name = "codex"
    cumulative = True

    def __init__(self, model, effort):
        self.model, self.effort = model or "gpt-6-luna", effort or "max"

    def command(self, card, session):
        env = environment()
        settings = ",".join(f"{k}={json.dumps(v)}" for k, v in env.items())
        roots = json.dumps([str(TARGET), str(card_dir(card["name"]))])
        command = ["codex", "exec", "--model", self.model, "--sandbox", "workspace-write", "--skip-git-repo-check", "--ignore-user-config", "--json",
                   "--color", "never", "--config", f"model_reasoning_effort={self.effort}", "--config", "sandbox_workspace_write.network_access=false",
                   "--config", f"sandbox_workspace_write.writable_roots={roots}", "--config", "shell_environment_policy.inherit=none",
                   "--config", "shell_environment_policy.set={" + settings + "}", "--config", "features.code_mode.default_exec_yield_time_ms=1800000",
                   "--cd", str(CANTRIP)]
        return command + (["resume", session, "-"] if session else ["-"])

    def read(self, line, usage):
        """The session id, if this event names one."""
        event = json.loads(line)
        if event.get("type") == "thread.started":
            return event.get("thread_id")
        if event.get("type") == "turn.completed":
            u = event.get("usage") or {}
            usage.input += u.get("input_tokens", 0)
            usage.cached += u.get("cached_input_tokens", 0)
            usage.output += u.get("output_tokens", 0)
            usage.turns += 1
        return None


class Claude:
    """`claude -p`. It has no sandbox of its own here: writes are limited by the tool list, so run it on a machine
    where that is acceptable. The gates and the commit are the same as any other worker's."""
    name = "claude"
    cumulative = False

    def __init__(self, model, effort):
        self.model, self.effort = model or "sonnet", effort

    def command(self, card, session):
        command = ["claude", "-p", "--model", self.model, "--output-format", "stream-json", "--verbose", "--permission-mode", "acceptEdits",
                   "--allowedTools", "Bash(python3:*) Bash(cargo:*) Bash(grep:*) Bash(ls:*) Bash(git status:*) Bash(git diff:*)", "--add-dir", str(card_dir(card["name"]))]
        if self.effort:
            command += ["--effort", self.effort]
        return command + (["--resume", session] if session else [])

    def read(self, line, usage):
        event = json.loads(line)
        if event.get("type") == "result":
            u = event.get("usage") or {}
            usage.input += u.get("input_tokens", 0) + u.get("cache_read_input_tokens", 0) + u.get("cache_creation_input_tokens", 0)
            usage.cached += u.get("cache_read_input_tokens", 0)
            usage.output += u.get("output_tokens", 0)
            usage.turns += event.get("num_turns", 1)
            if event.get("total_cost_usd") is not None:
                usage.reported_cost = event["total_cost_usd"]
            return event.get("session_id")
        return None


class Command:
    """Any other model: a command that reads the prompt on stdin and works in the Cantrip tree, `{session}` replaced on
    a repair round. It reports nothing back; `check` is what counts."""
    name = "command"
    cumulative = False

    def __init__(self, template, effort=None):
        self.model, self.template = template, template

    def command(self, card, session):
        return self.template.replace("{session}", session or "").split()

    def read(self, line, usage):
        return None


BACKENDS = {"codex": Codex, "claude": Claude, "command": Command}


def spawn(backend, card, prompt, session, timeout, log):
    """One worker run, killed as a process group at the timeout. (exit status or None, session id, usage)."""
    usage = Usage()
    with open(log, "a", encoding="utf-8") as out:
        process = subprocess.Popen(backend.command(card, session), cwd=CANTRIP, env=dict(os.environ, **environment()), stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                   stderr=subprocess.STDOUT, text=True, encoding="utf-8", errors="replace", start_new_session=True)
        process.stdin.write(prompt)
        process.stdin.close()
        deadline = time.time() + timeout
        timer = _Timer(process, deadline)
        for line in process.stdout:
            out.write(line)
            out.flush()
            try:
                session = backend.read(line, usage) or session
            except ValueError:
                pass
        code = process.wait()
        timer.cancel()
    return (None if timer.fired else code), session, usage


class _Timer:
    def __init__(self, process, deadline):
        import threading
        self.fired = False
        self.process = process
        self.timer = threading.Timer(max(1, deadline - time.time()), self.fire)
        self.timer.daemon = True
        self.timer.start()

    def fire(self):
        self.fired = True
        try:
            os.killpg(self.process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass

    def cancel(self):
        self.timer.cancel()


def repair_prompt(gates):
    lines = ["The lane's gates failed on the tree as it stands. Fix what they name in your scenarios and your report, nothing else, re-run the fast "
             "check, update report.json, and stop. If they show the generated card is wrong, say so with the decision `generator_bug`.", ""]
    for failure in gates.failures():
        lines += [f"### {failure['gate']}", failure["detail"], ""]
    lines.append("A scenario must not be weakened to pass, and a mutant is only declared equivalent if it truly changes nothing the rules can see.")
    return "\n".join(lines)


def run_card(card, backend, args, queue):
    """One card, start to finish: the steps of `next`, `check` and `commit`, with a worker in the middle. Returns its
    outcome; the queue records every step, so a killed run can restart."""
    name = card["name"]
    started = time.time()
    begin(card, queue, f"{backend.name}:{backend.model}")
    phases, usage, session, outcome, rounds = {}, Usage(), None, None, 0
    log = card_dir(name) / "worker.jsonl"
    card_dir(name).mkdir(parents=True, exist_ok=True)
    log.unlink(missing_ok=True)
    try:
        t = time.time()
        prepare(card)
        phases["prepare"] = round(time.time() - t, 1)
        prompt = brief(card, driven=True)
        deadline = started + args.timeout
        while True:
            t = time.time()
            code, session, used = spawn(backend, card, prompt, session, max(1, deadline - time.time()), log)
            phases["worker"] = round(phases.get("worker", 0) + time.time() - t, 1)
            usage.add(used, backend.cumulative)
            rounds += 1
            if code is None:
                outcome = "timeout"
                break
            if code != 0 and used.turns == 0:
                outcome = "infrastructure"
                break
            report = report_of(name) or {}
            if report.get("decision") in STOPS:
                outcome = report["decision"]
                card["failure"] = str(report.get("reason", ""))[:400]
                break
            t = time.time()
            gates = check(name)
            phases["check"] = round(phases.get("check", 0) + time.time() - t, 1)
            phases["gates"] = {r["gate"]: r["seconds"] for r in gates.results}
            if gates.ok:
                card["commit"] = commit_card(card)
                outcome = "committed"
                break
            card["failed_gates"] = [r["gate"] for r in gates.failures()]
            if rounds > args.repairs or time.time() >= deadline:
                outcome = "failed_gates"
                card["failure"] = "; ".join(f"{r['gate']}: {r['detail'][:160]}" for r in gates.failures())[:600]
                break
            prompt = repair_prompt(gates)
    except GeneratorBug as error:
        outcome = "generator_bug"
        card["failure"] = str(error)[:600]
    except Exception as error:  # a card's trouble is the card's: record it, keep the tree clean, go on
        outcome = outcome or "error"
        card["failure"] = f"{type(error).__name__}: {error}"[:600]
    finally:
        if outcome != "committed":
            save_patch(card)
            clean_tree()
        card.update(phases=phases, rounds=rounds, usage=usage.record(backend.model))
        conclude(card, queue, outcome)
    return outcome


def recover(queue):
    """A card a killed run left `working` goes back to pending, its tree cleaned and its work kept as a patch. Attempts
    are kept, so a card that kills the run cannot loop for ever."""
    for card in queue["cards"]:
        if card["status"] == "working":
            print(f"recovering {card['name']}: back to pending")
            save_patch(card)
            clean_tree()
            card["status"] = "pending"
    if changed_paths():
        sys.exit(f"the worktree {CANTRIP} has changes no card owns: {changed_paths()[:5]}")


def cmd_run(args):
    queue = load_queue()
    recover(queue)
    save_queue(queue)
    backend = BACKENDS[args.backend](args.command if args.backend == "command" else args.model, args.effort)
    results = []
    pending = [c for c in queue["cards"] if c["status"] == "pending" and c.get("attempts", 0) < args.attempts]
    if args.only:
        pending = [c for c in pending if c["name"] in args.only]
    for card in pending[:args.limit]:
        print(f"== {card['name']}", flush=True)
        outcome = run_card(card, backend, args, queue)
        results.append(outcome)
        u = card.get("usage", {})
        print(f"   {outcome} in {card['seconds']:.0f}s, {card.get('rounds')} round(s), {u.get('input', 0)} in/{u.get('cached', 0)} cached/{u.get('output', 0)} out, "
              f"${u.get('usd_api_equivalent')}  {card.get('commit', '')[:10]} {card.get('failure', '')[:140]}", flush=True)
        if outcome == "infrastructure" and results[-2:] == ["infrastructure"] * 2:
            print("stopping: two infrastructure failures in a row, which no card caused")
            break
        if len(results) == args.stop_after and "committed" not in results:
            print(f"stopping: the first {args.stop_after} cards all failed; fix the lane before spending more")
            break
    print_status(queue)


def print_status(queue):
    tally = collections.Counter(c["status"] for c in queue["cards"])
    print(f"{queue['format']}: {len(queue['cards'])} queued; " + ", ".join(f"{k} {v}" for k, v in tally.most_common()))
    worked = [c for c in queue["cards"] if c.get("seconds")]
    for c in worked:
        u = c.get("usage", {})
        print(f"  {c['status']:<14} {c['seconds']:>6.0f}s {u.get('input', 0):>9} in {u.get('output', 0):>7} out ${u.get('usd_api_equivalent') or 0:<7} "
              f"{c.get('worker', ''):<18} {c['name']}  {c.get('commit', '')[:10]}")


def cmd_status(args):
    queue = load_queue()
    if args.json:
        print(json.dumps(queue, indent=1, ensure_ascii=False))
    else:
        print_status(queue)


# ---- Setup ----------------------------------------------------------------------------------------------------------

def cmd_setup(args):
    """Idempotent. The worktree comes from the Cantrip checkout on a new local branch; the checkout's own tree and branch
    are not touched. Nothing here goes to /tmp, and no release build is made."""
    for path in (LANE, STATE, GEN, BIN, SCRYFALL, TARGET):
        path.mkdir(parents=True, exist_ok=True)
    if not CANTRIP.exists():
        shell("git", "-C", args.checkout, "fetch", "origin", "main", cwd=args.checkout, check=True)
        shell("git", "-C", args.checkout, "worktree", "add", "-b", BRANCH, CANTRIP, "FETCH_HEAD", cwd=args.checkout, check=True)
    sysroot = LANE / "sysroot" / "lib" / "rustlib" / "x86_64-unknown-linux-gnu" / "bin"
    sysroot.mkdir(parents=True, exist_ok=True)
    for name in ("llvm-cov", "llvm-profdata"):
        link = sysroot / name
        if not link.exists() and shutil.which(name):
            link.symlink_to(shutil.which(name))
    shim = BIN / "rustc"
    real = Path.home() / ".cargo" / "bin" / "rustc"
    shim.write_text(f"""#!/bin/bash
# Reports a sysroot holding the system's LLVM tools, so an unmodified card-coverage.py finds them. Everything else is rustc.
if [ "$1" = "--print" ] && [ "$2" = "sysroot" ] && [ $# -eq 2 ]; then
    echo {LANE / 'sysroot'}
    exit 0
fi
exec {real} "$@"
""", encoding="utf-8")
    shim.chmod(0o755)
    if not (BIN / "cargo-llvm-cov").exists() and shutil.which("cargo-llvm-cov") is None:
        print("cargo-llvm-cov is missing: cargo install cargo-llvm-cov --locked --root", LANE)
    if not sorted(SCRYFALL.glob("oracle-cards-*.jsonl.gz")) or args.refresh:
        fetch_scryfall()
    index = SCRYFALL / "by-name.json"
    if not index.exists():
        code, out = shell(sys.executable, "-c", f"""
import sys
sys.path.insert(0, {str(CANTRIP / 'tools' / 'maintenance')!r})
import scryfall
scryfall.CACHE = {str(SCRYFALL)!r}
scryfall._load(write=True)
""")
        if code:
            print(out[-800:])
    env = dict(environment(), MTGISH_CANTRIP_DIR=str(CANTRIP), CARGO_TARGET_DIR=str(LANE / "targets" / "ast"), CARGO_INCREMENTAL="0", CARGO_PROFILE_DEV_DEBUG="0")
    env.pop("CARGO_NET_OFFLINE")
    shell("cargo", "build", "--locked", "--features", "cantrip", "-p", "mtgish-ast", "--bin", "ast", cwd=MTGISH / "rust_syntax", env=env, check=True)
    shell("cargo", "build", "--profile", "cards", "-p", "cantrip-script", "--bin", "corpus", check=True)
    print(f"lane ready: {CANTRIP} on {git('rev-parse', '--abbrev-ref', 'HEAD').strip()} at {git('rev-parse', '--short', 'HEAD').strip()}")


def cmd_corpus(flags):
    """Cantrip's corpus runner on this tree and this target directory, with any of its flags."""
    flags = flags[1:] if flags[:1] == ["--"] else flags
    return subprocess.run(["cargo", "run", "--profile", "cards", "-q", "-p", "cantrip-script", "--bin", "corpus", "--", *flags], cwd=CANTRIP, env=environment()).returncode


def fetch_scryfall():
    """The two bulk files scryfall.py --refresh would fetch, into the lane instead of the main checkout."""
    import urllib.request
    headers = {"User-Agent": "cantrip-maintenance/1.0", "Accept": "application/json"}
    stamp = datetime.date.today().strftime("%Y%m%d")
    for kind in ("oracle-cards", "rulings"):
        request = urllib.request.Request(f"https://api.scryfall.com/bulk-data/{kind}", headers=headers)
        meta = json.load(urllib.request.urlopen(request, timeout=30))
        with urllib.request.urlopen(urllib.request.Request(meta["jsonl_download_uri"], headers=headers), timeout=280) as r:
            (SCRYFALL / f"{kind}-{stamp}.jsonl.gz").write_bytes(r.read())


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command_name", required=True)
    p = sub.add_parser("setup")
    p.add_argument("--checkout", default=str(Path.home() / "cantrip"))
    p.add_argument("--refresh", action="store_true", help="fetch today's Scryfall bulk files")
    p.set_defaults(func=cmd_setup)
    p = sub.add_parser("queue")
    p.add_argument("--format", default="pioneer")
    p.add_argument("--layouts", nargs="*", default=["normal"], help="Scryfall layouts the card files cover")
    p.add_argument("--listed-only", action="store_true", help="only cards on the format's list")
    p.set_defaults(func=cmd_queue)
    p = sub.add_parser("status")
    p.add_argument("--json", action="store_true")
    p.set_defaults(func=cmd_status)
    p = sub.add_parser("next")
    p.add_argument("name", nargs="?", help="this card rather than the next pending one")
    p.add_argument("--who", default="session", help="what to record as the worker")
    p.set_defaults(func=cmd_next)
    sub.add_parser("corpus", help="plays scenarios; every flag is the corpus runner's")
    p = sub.add_parser("check")
    p.add_argument("name")
    p.add_argument("--fast", action="store_true")
    p.set_defaults(func=cmd_check)
    p = sub.add_parser("commit")
    p.add_argument("name")
    p.set_defaults(func=cmd_commit)
    p = sub.add_parser("abandon")
    p.add_argument("name")
    p.add_argument("--outcome", choices=(*STOPS, "failed_gates"), default="failed_gates")
    p.add_argument("--reason", required=True)
    p.set_defaults(func=cmd_abandon)
    p = sub.add_parser("tool")
    p.add_argument("name")
    p.add_argument("rest", nargs=argparse.REMAINDER)
    p.set_defaults(func=cmd_tool)
    p = sub.add_parser("run")
    p.add_argument("--backend", choices=BACKENDS, default="codex")
    p.add_argument("--model", help="gpt-6-luna for codex, sonnet for claude, unless named")
    p.add_argument("--effort", help="max for codex, unless named")
    p.add_argument("--command", help="with --backend command: the command line; `{session}` is the session on a repair")
    p.add_argument("--limit", type=int, default=10)
    p.add_argument("--only", nargs="*", default=[], help="only these cards")
    p.add_argument("--timeout", type=int, default=1800, help="seconds a card may take, worker and repairs together")
    p.add_argument("--repairs", type=int, default=2, help="rounds a worker is sent back with failed gates")
    p.add_argument("--attempts", type=int, default=2, help="runs a card may take across restarts")
    p.add_argument("--stop-after", type=int, default=3, help="stop if this many first cards all fail")
    p.set_defaults(func=cmd_run)
    if sys.argv[1:2] == ["corpus"]:  # before argparse, which would take a leading `--card` for an option of its own
        return cmd_corpus(sys.argv[2:])
    args = parser.parse_args()
    return args.func(args) or 0


if __name__ == "__main__":
    sys.exit(main())
