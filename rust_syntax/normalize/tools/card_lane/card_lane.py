#!/usr/bin/env python3
"""Turn the Cantrip cards the generator writes whole into card commits, one card at a time, whoever does the checking.

Only a card with no gap comes here: the generator (`ast cantrip-files`) wrote it from its closed table of recipes
(card_file.rs) or from the whole-card form (pr_form.rs), so it needs no mechanic and no engine work. Its code and content
are final. The worker
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
import hashlib
import importlib.util
import json
import multiprocessing.pool
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
TARGET = Path(os.environ.get("CARD_LANE_TARGET", LANE / "targets" / "cantrip"))
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
RATES = {"gpt-6-luna": (.10, .01, .50), "gpt-5.6-luna": (.20, .02, 1.20), "gpt-6-sol": (2, .20, 10), "gpt-6.1-sol": (2, .10, 10), "gpt-6.1-terra": (1, .05, 5),
         "gpt-6-astra": (10, 1, 50)}

MUTATION_LIMIT = 40  # about 16 s each; a card past this is reported as partly tested, not passed silently
PRIOR_ART, PRIOR_ART_CHARS = 3, 3000  # scenarios in a packet's prior-art.md, and the most of any one
STOPS = ("generator_bug", "engine_gap", "needs_review")  # what a worker can report instead of `done`; no gate can undo these
BATCH = []  # the cards open together in the one tree, if any. Paths then go by owner: see `owner`
BATCH_DIR = STATE / "batch"


def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds")


def environment():
    """What every tool runs under, the driver's and the worker's alike. CANTRIP_VERIFY checks every rollback."""
    env = {k: v for k, v in os.environ.items() if k in ("HOME", "USER", "LANG", "LC_ALL", "TMPDIR", "TERM", "RUSTUP_HOME", "CARGO_HOME")}
    env.update(PATH=os.pathsep.join([str(BIN), str(Path.home() / ".cargo" / "bin"), "/usr/local/bin", "/usr/bin", "/bin"]),
               CARGO_TARGET_DIR=str(TARGET), CARGO_NET_OFFLINE="true", LLVM_COV=shutil.which("llvm-cov") or "llvm-cov",
               LLVM_PROFDATA=shutil.which("llvm-profdata") or "llvm-profdata", PYTHONDONTWRITEBYTECODE="1", CANTRIP_VERIFY="1",
               CARD_LANE=str(LANE), CARD_LANE_CANTRIP=str(CANTRIP), CARD_LANE_STATE=str(STATE), CARD_LANE_BRANCH=BRANCH, CARD_LANE_TARGET=str(TARGET))
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


ROWS = {}  # the open batch's rows, so its many gates do not each read the whole file


def generated_row(name):
    """The card's row of `ast cantrip-files`: its file in `rust`, or none for a card of core keywords alone."""
    if name in ROWS:
        return ROWS[name]
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
    lines = [l for l in lines if l and not all(w in cs.KEYWORDS for w in re.split(r",\s*", l.lower()))] or [""]
    for number, line in enumerate(lines, 1):
        code, out = tool("new-card.py", card_name, "--scenario", f"todo-line-{number}")
        if code != 0:
            sys.exit(out[-2000:])
        path = CANTRIP / out.strip().splitlines()[-1].removeprefix("wrote ")
        path.write_text(f"# TODO placeholder for printed line {number}: {line}\n" + path.read_text(encoding="utf-8"), encoding="utf-8", newline="\n")
        written.append(path.name)
    return written


def core_keywords():
    """{"first strike": "FirstStrike", ...}: the core keywords, as new-card.py reads them from the oracle crate's list."""
    source = (CANTRIP / "crates" / "cantrip-oracle" / "build.rs").read_text(encoding="utf-8")
    names = re.findall(r'"([^"]+)"', re.search(r"const KEYWORDS[^=]*=\s*\[(.*?)\];", source, re.S)[1])
    return {re.sub(r"(?<!^)(?=[A-Z])", " ", n).lower(): n for n in names}


def repair_keywords(name):
    """new-card.py reads a keyword line split by commas. Oracle splits it by semicolons where a keyword in it takes
    something ("Trample; banding", CR 702.1), and those keywords were lost: the card played as a vanilla creature, and with no
    code to break no gate could tell. A keyword line ends in no period."""
    path = content_file(name)
    card = json.loads(path.read_text(encoding="utf-8-sig"))
    core, have = core_keywords(), list(card.get("keywords") or [])
    found = [core[part.strip().lower()] for line in printed_lines(card.get("oracleText", "")) if ";" in line and not line.endswith(".")
             for part in line.split(";") if part.strip().lower() in core]
    missing = [k for k in dict.fromkeys(found) if k not in have]
    if missing:
        card["keywords"] = have + missing
        path.write_text(json.dumps(card, indent=2, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")


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


def prepare(card, build=True):
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
    repair_keywords(name)
    shutil.copyfile(content_file(name), packet / "content.json")
    source = row["rust"] or ""
    if source:
        card_file(name).write_text(source, encoding="utf-8", newline="\n")
    else:
        card_file(name).unlink(missing_ok=True)
    problems = (style_problems(name, source) + shared_copies(source)) if source else []
    if problems:
        raise GeneratorBug("; ".join(problems))
    (packet / "shared.md").write_text(shared_code(source), encoding="utf-8")
    (packet / "prior-art.md").write_text(prior_art(source, out), encoding="utf-8")
    points = parse_test_points(out)
    write_test_points(name, points)
    oracle_text = json.loads(content_file(name).read_text(encoding="utf-8-sig")).get("oracleText", "")
    (packet / "report.json").write_text(json.dumps(report_skeleton(points, parse_rulings(out), printed_lines(oracle_text)), indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    scenarios = placeholders(name)
    if build:
        code, out = run_corpus(name)
        (packet / "first-build.txt").write_text(out, encoding="utf-8")
        if re.search(r"^error", out, re.M) or "could not compile" in out:
            raise GeneratorBug(f"the generated file does not compile: {out[-500:]}")
    (packet / "prompt.md").write_text(brief(card), encoding="utf-8")
    return {"scenarios": scenarios}


def run_corpus(name):
    if BATCH:  # built once for them all
        return shell(TARGET / "cards" / "corpus", "--card", name, "--brief")
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
    allowed, notes = set(), set()
    for each in BATCH or [name]:
        stem, slug, _ = names_of(each)
        allowed |= {f"{CONTENT_DIR}/{slug}.json", f"{CARDS_DIR}/{stem}.rs"}
        notes.add(f"docs/card-notes/{stem}.md")
    problems = []
    for status, path in changed_paths():
        scenario = path.startswith(SCENARIOS_DIR + "/") and path.endswith(".cantrip") and status == "??"
        note = path in notes and "D" in status
        if not (path in allowed or scenario or note):
            problems.append(f"{status} {path}")
        elif scenario and BATCH and owner(Path(path).name) is None:
            problems.append(f"{path}: its name starts with no card of this batch")
    return problems


def owner(scenario):
    """Which open card a scenario file belongs to, by the longest card name it starts with."""
    best, length = None, 0
    for name in BATCH:
        stem, slug, _ = names_of(name)
        for start in (stem.replace("_", "-"), slug):
            if scenario.startswith(start + "-") and len(start) > length:
                best, length = name, len(start)
    return best


def owned_paths(name):
    """The changed paths of one open card, of the batch or of a tree holding only it."""
    if not BATCH:
        return [p for _, p in changed_paths()]
    stem, slug, _ = names_of(name)
    own = {f"{CONTENT_DIR}/{slug}.json", f"{CARDS_DIR}/{stem}.rs", f"docs/card-notes/{stem}.md"}
    return [p for _, p in changed_paths() if p in own or (p.startswith(SCENARIOS_DIR + "/") and owner(Path(p).name) == name)]


def card_scenarios(name=None):
    paths = sorted(CANTRIP / p for s, p in changed_paths() if p.startswith(SCENARIOS_DIR + "/") and p.endswith(".cantrip"))
    return [p for p in paths if owner(p.name) == name] if BATCH and name else paths


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


def shared_copies(source):
    """The card's functions that are a shared effect over again, which a review blocks. A recipe that writes one should
    name the shared effect instead, so this is the generator's to fix and no worker's."""
    def key(body):
        return re.sub(r"\s+", "", "".join(body)).replace("crate::", "")
    shared = (CANTRIP / "crates" / "cantrip-mechanics" / "src" / "effects.rs").read_text(encoding="utf-8")
    known = {key(body): fn for fn, _, body in functions(shared)}
    return [f"fn {fn} is effects::{known[key(body)]}" for fn, _, body in functions(source) if key(body) in known]


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
    if pair.get("expect") and not pair.get("line"):
        return f"{label}: {path.name} has no line `{pair['expect']}`"
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
    hits = {}
    for scenario in card_scenarios(name):
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
            lines = line_hits(lcov, path)
            for number, count in lines.items():
                hits[number] = hits.get(number, 0) + count
            seen[scenario.name] = sum(1 for count in lines.values() if count)
            if seen[scenario.name] == 0:
                dead.append(f"{scenario.name}: executed no line of {path.name}")
    return dead, hits


def env_tool(name):
    return environment()[name]


def costs_mana(name):
    cost = json.loads(content_file(name).read_text(encoding="utf-8-sig")).get("manaCost", "")
    return bool(cost) and cost != "{0}"


def check(name, fast=False):
    """Every gate that can be run on the tree, cheapest first. `fast` leaves out what takes minutes, so a worker can loop
    on it; a commit always takes the whole of it. The report is checked either way, so that a worker sees what would
    otherwise cost it a repair round."""
    gates = Gates(name, fast)
    context = early_gates(gates, name)
    if context is None:
        return finish(gates)
    path, source = context["path"], context["source"]

    started = time.time()
    code, out = tool("card-coverage.py", name)
    gates.add("coverage", code == 0, out[-1500:] if code else "", started)
    if code != 0:
        return finish(gates)

    started = time.time()
    dead, _ = scenario_liveness(name)
    gates.add("liveness", not dead, "\n".join(dead), started)

    survivors = mutation_gates([name], {name: gates}).get(name, []) if not fast else []
    if "statics:" in source and not fast:
        started = time.time()
        code, out = shell("cargo", "test", "--profile", "checked", "-q", "-p", "cantrip-script", "--test", "layer_pass_cases")
        gates.add("layers", code == 0, out[-1500:] if code else "", started)
    report_gate(gates, name, context, survivors)
    return finish(gates)


def early_gates(gates, name):
    """The gates up to the corpus, which need no build beyond the corpus's own. What the later gates need, or None if
    the scenarios do not pass, as nothing after that can say anything."""
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
    scenarios = card_scenarios(name)
    bad = [p.name for p in scenarios if "-todo" in p.name or "TODO" in p.read_text(encoding="utf-8")]
    bad += [f"{p.name}: says Known bug, so it asserts an engine failure" for p in scenarios if "Known bug" in p.read_text(encoding="utf-8")]
    bad += [f"{p.name}: does not open with what it shows (a # comment)" for p in scenarios if not p.read_text(encoding="utf-8").lstrip().startswith("#")]
    bad += [f"{p.name}: no [Expect]" for p in scenarios if "[Expect]" not in p.read_text(encoding="utf-8")]
    if scenarios and costs_mana(name) and not any(re.search(r"\.Mana =", p.read_text(encoding="utf-8")) for p in scenarios):
        bad.append("no scenario shows the cost paid exactly: `? Bot1.Mana =` right after a cast, so a cost too low fails")
    bad += [f"{p.name}: uses {other}, which may not land with this card" for p in scenarios for other in BATCH
            if other != name and re.search(rf"^(\d+ )?{re.escape(other)}( x\d+)?\s*$", p.read_text(encoding="utf-8"), re.M | re.I)]
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
        return None
    warned = re.findall(r"^warning: (.*)\n\s*--> (.*" + re.escape(path.name) + r".*)$", out + BUILT.get("out", ""), re.M) if source else []
    gates.add("warnings", not warned, "\n".join(f"{where}: {what}" for what, where in warned))
    return {"path": path, "source": source, "points": points, "looks": looks}


def report_looks(name, context):
    """What the report has to answer: lint's LOOKs, and any function repeated from another card."""
    if "overlaps" not in context:
        context["overlaps"] = overlaps(name, context["source"])
    return context["looks"] + [(o.split(" repeats ")[1].split(":")[0], o) for o in context["overlaps"]]


def report_gate(gates, name, context, survivors):
    started = time.time()
    problems = report_problems(name, context["points"], report_looks(name, context), survivors)
    report = report_of(name) or {}
    if report.get("decision") not in ("done", None):
        problems.insert(0, f"the worker reported {report.get('decision')}: {report.get('reason', '')}")
    gates.add("report", not problems, "\n".join(problems), started)


BUILT = {}  # what the batch's one build printed: its warnings are the warnings gate's


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
    git("add", "--", *owned_paths(name))
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
    paths = owned_paths(card["name"])
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
        roots = json.dumps([str(TARGET), *map(str, card.get("dirs") or [card_dir(card["name"])])])
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
                   "--allowedTools", "Bash(python3:*) Bash(cargo:*) Bash(grep:*) Bash(ls:*) Bash(git status:*) Bash(git diff:*)", "--add-dir", *map(str, card.get("dirs") or [card_dir(card["name"])])]
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





# ---- Batches --------------------------------------------------------------------------------------------------------
#
# Several cards in one tree and one worker session. What a card costs is the worker reading, the context it pays for on
# every step, and the builds: so the prompt carries all the worker needs, one build serves every card, and a round of
# mutation breaks one line of each card at once. Each card still gets its own gates and its own commit.

def open_batch(names):
    BATCH[:] = names
    BATCH_DIR.mkdir(parents=True, exist_ok=True)
    wanted = set(names)
    for line in (GEN / "rows.jsonl").open(encoding="utf-8"):
        if len(ROWS) == len(wanted):
            break
        row = json.loads(line)
        if row["name"] in wanted:
            ROWS[row["name"]] = row


def close_batch():
    BATCH.clear()
    ROWS.clear()
    (BATCH_DIR / "open.json").unlink(missing_ok=True)


def reopen_batch():
    path = BATCH_DIR / "open.json"
    if not path.exists():
        sys.exit("no batch is open")
    open_batch(json.loads(path.read_text(encoding="utf-8")))


def build_corpus():
    """The one build every card's gates share. (exit status, what it printed)."""
    code, out = shell("cargo", "build", "--profile", "cards", "-p", "cantrip-script", "--bin", "corpus")
    BUILT["out"] = out
    return code, out


def broken_cards(out):
    """The open cards a compiler error names."""
    stems = set(re.findall(r"--> \S*?cantrip-cards/src/cards/(\w+)\.rs", out))
    return [name for name in BATCH if names_of(name)[0] in stems]


def prepare_batch(cards, queue):
    """Prepares every card, then builds once. A card that cannot stand leaves the batch and is recorded; the others go on."""
    standing = []
    for card in cards:
        try:
            prepare(card, build=False)
            standing.append(card)
        except GeneratorBug as error:
            card["failure"] = str(error)[:600]
            discard(card, queue, "generator_bug")
    while standing:
        code, out = build_corpus()
        if code == 0:
            break
        broken = broken_cards(out)
        if not broken:
            raise RuntimeError(f"the batch does not build: {out[-800:]}")
        for card in [c for c in standing if c["name"] in broken]:
            card["failure"] = f"the generated file does not compile: {out[-500:]}"
            discard(card, queue, "generator_bug")
            standing.remove(card)
    with multiprocessing.pool.ThreadPool(16) as pool:
        for card, (_, out) in zip(standing, pool.map(lambda c: run_corpus(c["name"]), standing)):
            (card_dir(card["name"]) / "first-build.txt").write_text(out, encoding="utf-8")
    return standing


def discard(card, queue, outcome):
    """A card out of the batch: its work kept as a patch, its files gone, the rest of the tree left alone."""
    save_patch(card)
    for path in owned_paths(card["name"]):
        (CANTRIP / path).unlink(missing_ok=True)
    if card["name"] in BATCH:
        BATCH.remove(card["name"])
    conclude(card, queue, outcome)


ACTION_LINES = []  # every scenario's action lines, read once for all of a batch's cards


def keyword_words(text):
    """The first word of each keyword on a card's keyword lines: "Crew 2", "Trample; banding", "Flashback—{1}{G}"."""
    words = []
    for line in printed_lines(text):
        if not line.endswith(".") or re.match(r"\S+—", line):
            words += [re.split(r"[\s—]", part.strip())[0] for part in re.split(r"[,;]", line) if part.strip()]
    return list(dict.fromkeys(w for w in words if w[:1].isalpha() and len(w) > 3))


def keyword_lines(text, wanted=2):
    """Lines of existing scenarios that say a card's keywords, shortest first: how a worker learns that a foretold card is
    `Activate(Ability: Foretell)`, which otherwise it searches the corpus for, at the price of everything the search prints."""
    if not ACTION_LINES:
        for path in (CANTRIP / SCENARIOS_DIR).glob("*.cantrip"):
            body = path.read_text(encoding="utf-8").split("[Actions]", 1)[-1].split("[Expect]", 1)[0]
            ACTION_LINES.extend(line.strip() for line in body.splitlines() if line.startswith(("Bot", "! Bot")))
    out = []
    for word in keyword_words(text):
        pattern = re.compile(rf"(Ability: *|\.){re.escape(word)}(\b(?=[^.]*$)|\()", re.I)  # the action, not a card named Crew
        found = sorted({line for line in ACTION_LINES if pattern.search(line)}, key=lambda line: (line.startswith("!"), len(line)))
        out += [f"- {word}: `{line}`" for line in found[:wanted]]
    return out


def card_section(number, card):
    """What a worker needs of one card, so it reads nothing to get it."""
    name = card["name"]
    packet = card_dir(name)
    content = json.loads(content_file(name).read_text(encoding="utf-8-sig"))
    skeleton = json.loads((packet / "report.json").read_text(encoding="utf-8"))
    (packet / "report.skeleton.json").write_text(json.dumps(skeleton, indent=1, ensure_ascii=False), encoding="utf-8")
    path = card_file(name)
    source = path.read_text(encoding="utf-8") if path.exists() else ""
    _, lint = tool("card-lint.py", name)
    looks = [line.split("/")[-1] for line in lint.splitlines() if ": LOOK:" in line]
    facts = {k: v for k, v in content.items() if k not in ("oracleText", "name")}
    out = [f"## Card {number}: {name}", "", json.dumps(facts, ensure_ascii=False), "", "Printed lines:"]
    out += [f"{i}. {v['line']}" for i, v in enumerate(skeleton["validation"], 1)]
    if skeleton["test_points"]:
        out += ["", "Test points:"] + [f"{p['id']}. {p['point']}" for p in skeleton["test_points"]]
    if skeleton["rulings"]:
        out += ["", "Rulings:"] + [f"{i}. {r['ruling']}" for i, r in enumerate(skeleton["rulings"], 1)]
    if looks:
        out += ["", "Lint asks you to look at these; answer each in `looks`, naming its line as `line 12`:"] + looks
    said = keyword_lines(content.get("oracleText", ""))
    if said:
        out += ["", "How other scenarios say its keywords, the card's name standing in another's:"] + said
    out += ["", f"Its file, `{CARDS_DIR}/{path.name}`:" if source else "It has no file: the content's keywords are the whole card.", ""]
    out += ["```rust", source.rstrip(), "```", ""] if source else []
    written = card_scenarios(name)
    seeded = [p for p in written if "TODO" not in p.read_text(encoding="utf-8")]
    if seeded:
        out += ["Scenarios the tool wrote from the card's words. Keep one that proves what it says; fix or replace the rest:", ""]
        for scenario in seeded:
            out += [f"`{SCENARIOS_DIR}/{scenario.name}`", "```", scenario.read_text(encoding="utf-8").rstrip(), "```", ""]
    todo = [p.name for p in written if p not in seeded]
    if todo:
        out += ["Placeholders to delete once replaced: " + ", ".join(f"`{n}`" for n in todo), ""]
    out.append(f"Name its scenarios `{SCENARIOS_DIR}/{names_of(name)[0].replace('_', '-')}-<what-it-proves>.cantrip`.")
    return "\n".join(out)


def shared_section(cards):
    """The shared code the cards call, each definition once."""
    seen, out = set(), []
    for card in cards:
        path = card_file(card["name"])
        text = shared_code(path.read_text(encoding="utf-8")) if path.exists() else ""
        for block in re.split(r"(?m)^(?=## )", text):
            if block.strip() and block not in seen:
                seen.add(block)
                out.append(block.rstrip())
    return "\n\n".join(out)


REPORT_FORMAT = """Write the reports to `{path}`, one JSON object for all the cards, keyed by card name:

```json
{{"Card Name": {{"decision": "done", "lines": ["what implements printed line 1", "..."],
  "points": [["scenario-file-name", "? Bot2.Life = 17"], ["..."]],
  "rulings": [["scenario-file-name", "Bot1.Graveyard.Count = 2"], "why the card's own code decides nothing here"],
  "looks": ["line 12: why it is right"], "equivalent": [{{"mutant": "line 6: 4 to 5", "why": "..."}}], "notes": ""}}}}
```

`lines` has one entry per printed line, `points` one per test point, `rulings` one per ruling, in order. A `points` or `rulings`
pair is a scenario and the exact text of the expectation in it that catches the thing: a `?`, a `!` or a line under `[Expect]`.

Validation comes first: before writing a card's scenarios work out from its words what must happen, then read its file and the
shared code and say in `lines` what implements each printed line. Read bodies, not names: amounts and who gets them, what is
legal to target, the trigger's event and whose, the duration and the order. A line with no code is a disagreement.
A card that is wrong is worse than a card that is missing. If a line disagrees, or a scenario shows the card does other than its
text says, the card's entry is `{{"decision": "generator_bug", "reason": "the line, what the code does, what the text requires"}}`;
if the card is right and the engine plays the rules wrongly, `engine_gap`; if a tool is broken, `needs_review`. These are normal
results and cost little. Do not work around them.

A stop must say the game state in which the card does the wrong thing, and a scenario must show it if one can. That you have no
scenario for something yet is not a gap: write one. That code looks unusual is not a bug: `cast.from == ZoneKind::Hand` with the
caster as owner is what "your hand" means, since a spell can be cast from another player's hand. A card in a lane has been
compiled and its shape checked, so most cards are right."""


WAYS = {
    "work": """Tools, from the working directory:
- `{lane} batch-check --fast`: plays every card's scenarios and judges each card and its report. Prints what failed.
- `{lane} corpus --file PATH --brief`: one scenario. Add `--dump`, `--diff N` or `--next` to see what it can see.
- `{lane} batch-check --card "NAME"`: the same, and it breaks that card one change at a time: minutes the first time, seconds after.
- `{lane} tool find-cards.py ...`, `{lane} tool cr.py 603.6a`.

Build out the scenarios for every card: what each owes is every printed line, test point, ruling and keyword. Play them, fix them
and keep going until `batch-check --fast` passes for every card, or a card has a stop. Do not finish with a failure you know of.
Only then does a driver break each card and send back what no scenario caught. One command at a time, in the foreground.""",
    "draft": """Do not run anything now: no corpus, no check, no search. You have all you need above. Write every card's scenarios and the
reports, and stop. Put as many files as you like in an action. A driver plays them and sends back what failed, for every card at
once, and you fix them then, with the tools it names.

Do each card in full now. For each, list what it owes: every printed line, test point, ruling and keyword, and write an
expectation for each. Most cards need several scenarios, and a card with one is sent straight back. Nothing has been played, so
every card's decision is `done` unless reading its code shows a `generator_bug`. Unproved is the state of everything at this
point; it is no reason to stop.""",
}


def batch_prompt(cards, way):
    lane = f"python3 {Path(__file__).resolve()}"
    head = [f"# {len(cards)} generated card{'s' if len(cards) > 1 else ''}", "",
            f"The mtgish generator wrote each card below. Validate each against its Oracle text and write the scenarios that prove it. "
            f"You write new files under `{SCENARIOS_DIR}/` and the reports, nothing else: the cards' files and content are read-only, and no `git`. "
            f"Your working directory is the Cantrip tree `{CANTRIP}`. Delete a placeholder when you replace it. Never `--release`. A driver makes the commits.", "",
            (HERE / "handbook.md").read_text(encoding="utf-8").rstrip(), "", "# The reports", "",
            REPORT_FORMAT.format(path=BATCH_DIR / "reports.json"), "", "# How to work", "", WAYS[way].format(lane=lane), "", "# The cards", ""]
    shared = shared_section(cards)
    tail = ["", "# Shared code the cards call", "", shared] if shared else []
    return "\n".join(head + ["\n\n".join(card_section(i, c) for i, c in enumerate(cards, 1))] + tail) + "\n"


def expected_line(scenario, text):
    """The line of a scenario holding this expectation, counted from 1, or 0."""
    path = CANTRIP / SCENARIOS_DIR / f"{str(scenario).removesuffix('.cantrip')}.cantrip"
    if not path.is_file():
        return 0
    lines = [line.strip() for line in path.read_text(encoding="utf-8").splitlines()]
    return lines.index(text.strip()) + 1 if text.strip() in lines else 0


def clear_blind_stops(path):
    """A worker that has played nothing has seen no engine or tool fail, so an `engine_gap` or `needs_review` from a blind
    pass says only that nothing is proved yet, which is what the gates are about to find out. A `generator_bug` stands:
    reading is how that is found."""
    try:
        reports = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return
    changed = False
    for report in reports.values() if isinstance(reports, dict) else ():
        if isinstance(report, dict) and report.get("decision") in ("engine_gap", "needs_review"):
            report.update(decision="done", notes=str(report.pop("reason", "")))
            changed = True
    if changed:
        path.write_text(json.dumps(reports, ensure_ascii=False, indent=1), encoding="utf-8")


def expand_reports(names):
    """The one reports file into each card's `report.json`, which is what the gates read. A worker names an expectation by
    its text, so that editing a scenario cannot leave a report pointing at the wrong line."""
    path = BATCH_DIR / "reports.json"
    try:
        reports = json.loads(path.read_text(encoding="utf-8")) if path.exists() else {}
    except ValueError as error:
        reports = {name: {"decision": "invalid", "reason": f"reports.json is not JSON: {error}"} for name in names}
    for name in names:
        entry = next((v for k, v in reports.items() if k.lower() == name.lower()), None)
        if not isinstance(entry, dict):
            continue
        report = json.loads((card_dir(name) / "report.skeleton.json").read_text(encoding="utf-8"))
        report.update(decision=entry.get("decision", "todo"), reason=entry.get("reason", ""), notes=entry.get("notes", ""),
                      look_answers=entry.get("looks") or [], equivalent_mutants=entry.get("equivalent") or [])
        for line, text in zip(report["validation"], entry.get("lines") or []):
            line.update(code=str(text), agrees=report["decision"] == "done" or None)
        for point, pair in zip(report["test_points"], entry.get("points") or []):
            if isinstance(pair, list) and len(pair) == 2:
                point.update(scenario=str(pair[0]).removesuffix(".cantrip"), expect=pair[1], line=expected_line(pair[0], str(pair[1])))
        for ruling, answer in zip(report["rulings"], entry.get("rulings") or []):
            if isinstance(answer, list) and len(answer) == 2:
                ruling.update(scenario=str(answer[0]).removesuffix(".cantrip"), expect=answer[1], line=expected_line(answer[0], str(answer[1])))
            elif answer:
                ruling.update(no_decision=str(answer))
        (card_dir(name) / "report.json").write_text(json.dumps(report, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")


def tool_module(script):
    """A Cantrip maintenance script as a module."""
    sys.path.insert(0, str(CANTRIP / "tools" / "maintenance"))
    spec = importlib.util.spec_from_file_location(script.removesuffix(".py").replace("-", "_"), CANTRIP / "tools" / "maintenance" / script)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def coverage_gates(names, gates, contexts):
    """Coverage and liveness of every card from one instrumented build. They are one measurement: what each of a card's
    scenarios ran, added up, is what card-coverage.py measures of them together. The cards that pass both."""
    started = time.time()
    coded = [n for n in names if contexts[n]["source"]]
    if coded:
        code, out = shell("cargo", "llvm-cov", "run", "--profile", "cards", "--dep-coverage", "cantrip-cards", "-p", "cantrip-script", "--bin", "corpus",
                          "--no-report", "--", "--card", coded[0])
        if code != 0:
            for name in names:
                gates[name].add("coverage", False, out[-1500:], started)
            return []
    spent = (time.time() - started) / max(1, len(names))
    measure = tool_module("card-coverage.py")

    def one(name):
        started = time.time()
        dead, hits = scenario_liveness(name)
        source = contexts[name]["source"]
        exempt = measure.const_fn_lines(source.splitlines())
        missed = sorted(n for n, count in hits.items() if not count and n not in exempt)
        empty = bool(source) and not hits and re.search(r"\bfn\s+\w+\s*\(", source)
        lines = source.splitlines()
        detail = "no instrumented line ran" if empty else "\n".join(f"line {n} never ran: {lines[n - 1].strip()}" for n in missed)
        gates[name].add("coverage", not missed and not empty, detail)
        gates[name].results[-1]["seconds"] = round(spent, 1)
        gates[name].add("liveness", not dead, "\n".join(dead), started)
    with multiprocessing.pool.ThreadPool(16) as pool:
        pool.map(one, names)
    return [n for n in names if gates[n].ok]


def corpus_verdict(name):
    """The card's scenarios on the built corpus: (exit status, the first scenario that failed, or None). A change that makes
    the game run on is caught, by nothing that can be named."""
    try:
        code, out = shell(TARGET / "cards" / "corpus", "--card", name, "--brief", timeout=300)
    except subprocess.TimeoutExpired:
        return 1, None
    failed = re.search(r"^\s*FAIL (\S+)", out, re.M)
    return code, failed.group(1) if failed else None


def digest(data):
    return hashlib.sha256(data if isinstance(data, bytes) else data.encode("utf-8")).hexdigest()


def scenario_digest(stem):
    """What a scenario said, or None if it is gone."""
    path = CANTRIP / SCENARIOS_DIR / f"{stem}.cantrip"
    return digest(path.read_bytes()) if path.is_file() else None


def scenarios_digest(name):
    """What all of a card's scenarios say together."""
    return digest("".join(sorted(digest(path.read_bytes()) for path in card_scenarios(name))))


def verdict_of(change, known, together):
    """What an earlier round's verdict on this change still proves, or None if it has to be played. The card's code does not
    move, so only scenarios can: a change caught stays caught while the scenario that caught it says the same, one that
    survived stays alive until any of the card's scenarios changes, and one that does not compile never will."""
    if not known or known.get("change") != digest(change["text"]):
        return None
    if known["verdict"] == "no build":
        return "no build"
    if known["verdict"] == "SURVIVED":
        return "SURVIVED" if known.get("together") == together else None
    return "caught" if known.get("by") and scenario_digest(known["by"]) == known.get("said") else None


def changes_of(name, mutate):
    """What breaking a card tries: card-mutate.py's changes to its file, then each keyword out of its content, which
    nothing else can test: `..oracle::X` is no line of code, and a card of keywords alone has no file. (changes, how many
    there were before the limit)."""
    out, total = [], 0
    path = card_file(name)
    if path.exists():
        lines = path.read_text(encoding="utf-8").split("\n")
        found = mutate.mutants(lines, set(range(1, len(lines) + 1)))
        total = len(found)
        out += [{"path": path, "key": f"line {n}: {what}", "text": "\n".join(lines[:n - 1] + [new] + lines[n:]),
                 "detail": f"{path.name}:{n}: {what}, was: {lines[n - 1].strip()} now: {new.strip()}"} for n, what, new in found[:MUTATION_LIMIT]]
    content = content_file(name)
    card = json.loads(content.read_text(encoding="utf-8-sig"))
    for keyword in card.get("keywords") or []:
        rest = {k: v for k, v in card.items() if k != "keywords"}
        kept = [k for k in card["keywords"] if k != keyword]
        out.append({"path": content, "key": f"keyword {keyword}", "text": json.dumps(dict(rest, keywords=kept) if kept else rest, indent=2, ensure_ascii=False) + "\n",
                    "detail": f"{keyword} taken off the card, and every scenario still passes: nothing shows the card has it"})
    return out, total


def mutation_gates(names, gates):
    """Breaks each card one change at a time and plays its scenarios. A round breaks one thing of every card still to go, so
    one build serves them all, and each card's scenarios judge its own change: cards do not read each other, since no
    scenario holds another card of the batch. A change that does not compile is set aside and the round built again, as
    card-mutate.py skips it. The number of builds is the most changes any one card has, not the sum. {card: its survivors}."""
    mutate = tool_module("card-mutate.py")
    plans, started, kept = {}, time.time(), 0
    for name in names:
        changes, total = changes_of(name, mutate)
        originals = {path: path.read_bytes() for path in (card_file(name), content_file(name)) if path.exists()}
        memo = card_dir(name) / "mutants.json"
        known = json.loads(memo.read_text(encoding="utf-8")) if memo.exists() else {}
        plan = {"changes": [], "any": bool(changes), "total": total, "originals": originals, "log": [], "survivors": [], "next": 0,
                "memo": memo, "known": known, "together": scenarios_digest(name)}
        for change in changes:
            verdict = verdict_of(change, known.get(change["key"]), plan["together"])
            if verdict is None:
                plan["changes"].append(change)
                continue
            kept += 1
            plan["log"].append(f"{change['key']}: {verdict}, as before")
            if verdict == "SURVIVED":
                plan["survivors"].append((change["key"], change["detail"]))
        plans[name] = plan
    builds = 0

    def remember(plan, change, verdict, by=None):
        plan["known"][change["key"]] = {"change": digest(change["text"]), "verdict": verdict, "by": by, "said": scenario_digest(by) if by else None,
                                        "together": plan["together"]}

    def restore(plan):
        for path, text in plan["originals"].items():
            path.write_bytes(text)
    try:
        while True:
            live = [n for n, p in plans.items() if p["next"] < len(p["changes"])]
            if not live:
                break
            for name in live:
                change = plans[name]["changes"][plans[name]["next"]]
                change["path"].write_text(change["text"], encoding="utf-8", newline="\n")
            while live:
                builds += 1
                code, out = build_corpus()
                if code == 0:
                    break
                for name in [n for n in live if n in broken_cards(out)] or list(live):
                    plan = plans[name]
                    plan["log"].append(f"{plan['changes'][plan['next']]['key']}: no build")
                    remember(plan, plan["changes"][plan["next"]], "no build")
                    restore(plan)
                    plan["next"] += 1
                    live.remove(name)
            with multiprocessing.pool.ThreadPool(16) as pool:
                verdicts = pool.map(corpus_verdict, live)
            for name, (code, by) in zip(live, verdicts):
                plan = plans[name]
                change = plan["changes"][plan["next"]]
                plan["log"].append(f"{change['key']}: {'caught' if code else 'SURVIVED'}")
                remember(plan, change, "caught" if code else "SURVIVED", by)
                if code == 0:
                    plan["survivors"].append((change["key"], change["detail"]))
                restore(plan)
                plan["next"] += 1
    finally:
        for plan in plans.values():
            restore(plan)
            plan["memo"].write_text(json.dumps(plan["known"], ensure_ascii=False), encoding="utf-8")
    if builds:
        build_corpus()
    print(f"   mutation: {sum(len(p['changes']) for p in plans.values())} played, {kept} kept from before, {builds} builds, {time.time() - started:.0f}s", flush=True)
    spent = (time.time() - started) / max(1, len(plans))
    for name, plan in plans.items():
        (card_dir(name) / "mutate.txt").write_text("\n".join(plan["log"]) + "\n", encoding="utf-8")
        if plan["any"]:
            capped = plan["total"] > MUTATION_LIMIT
            gates[name].add("mutation", not capped, f"{plan['total']} possible changes, only {MUTATION_LIMIT} tried" if capped else "")
            gates[name].results[-1]["seconds"] = round(spent, 1)
    return {name: plan["survivors"] for name, plan in plans.items()}


def batch_gates(names, fast=False):
    """`check` over the open cards, in the same order and by the same gates, with the builds shared. {card: Gates}."""
    gates = {name: Gates(name, fast) for name in names}
    code, out = build_corpus()
    if code != 0:
        for name in names:
            gates[name].add("build", False, out[-1500:])
            finish(gates[name])
        return gates
    with multiprocessing.pool.ThreadPool(8) as pool:
        contexts = dict(zip(names, pool.map(lambda name: early_gates(gates[name], name), names)))
    passing = [n for n in names if contexts[n] is not None and gates[n].ok]
    covered = coverage_gates(passing, gates, contexts)
    survivors = {}
    # Breaking a card is the dearest gate, so only a card nothing else stops pays for it.
    ready = [n for n in covered if not report_problems(n, contexts[n]["points"], report_looks(n, contexts[n]), [])]
    if not fast and ready:
        survivors = mutation_gates(ready, gates)
        statics = [n for n in ready if "statics:" in contexts[n]["source"]]
        if statics:
            started = time.time()
            code, out = shell("cargo", "test", "--profile", "checked", "-q", "-p", "cantrip-script", "--test", "layer_pass_cases")
            for name in statics:
                gates[name].add("layers", code == 0, out[-1500:] if code else "", started)
    for name in names:
        if contexts[name] is not None:
            report_gate(gates[name], name, contexts[name], survivors.get(name, []))
        finish(gates[name])
    return gates


def batch_text(gates, limit=3000):
    """What failed, card by card: the only thing a worker needs to read back."""
    out = []
    for name, card in gates.items():
        failed = card.failures()
        out.append(f"{'PASS' if not failed else 'FAIL'} {name}" + (": " + ", ".join(r["gate"] for r in failed) if failed else ""))
    for name, card in gates.items():
        for failure in card.failures():
            out += ["", f"### {name}: {failure['gate']}", failure["detail"][:limit]]
    return "\n".join(out)


def cmd_batch_check(args):
    reopen_batch()
    expand_reports(BATCH)
    gates = batch_gates([args.card] if args.card else list(BATCH), args.fast)
    print(batch_text(gates))
    return 0 if all(g.ok for g in gates.values()) else 1


def batch_repair(gates, way, left):
    lane = f"python3 {Path(__file__).resolve()}"
    tools = (f"You can use these now. `{lane} corpus --file PATH --brief` plays one scenario; add `--dump`, `--diff N` or `--next` to see what it can see. "
             f"`{lane} batch-check --fast` plays every card still open and judges their reports. `{lane} batch-check --card \"NAME\"` also breaks that card one change "
             "at a time: minutes the first time, seconds after, since only what your edits touched is played again. ") if way == "draft" else ""
    return (f"The gates failed as below. {tools}\n\n"
            "Work through every failure, card by card, and keep going until each card passes or has a stop. Play a scenario after you change it. "
            "Before you finish, run the fast check and fix what it says. Do not finish with a failure you know of, with a change you have not played, or "
            f"with a note that something remains: there is nobody to read it, and after {left} more round{'s' if left != 1 else ''} a card still failing is dropped.\n\n"
            f"Change only your scenarios and `{BATCH_DIR / 'reports.json'}`. A scenario must not be weakened to pass, and a mutant is only declared equivalent if it "
            "truly changes nothing the rules can see. If they show a card is wrong, its decision is `generator_bug`.\n\n" + batch_text({n: g for n, g in gates.items() if not g.ok}))


def run_batch(cards, backend, args, queue):
    """Some cards, start to finish, in one session: prepared together, written together, judged together, each committed
    alone. A card that passes leaves the batch. Returns the batch's record."""
    started, phases, usage, session, rounds = time.time(), {}, Usage(), None, 0
    names = [c["name"] for c in cards]
    open_batch(names)
    (BATCH_DIR / "open.json").write_text(json.dumps(names, ensure_ascii=False), encoding="utf-8")
    (BATCH_DIR / "reports.json").unlink(missing_ok=True)
    log = BATCH_DIR / f"worker-{int(started)}.jsonl"
    who = f"{backend.name}:{backend.model}"
    for card in cards:
        begin(card, queue, who)
        card_dir(card["name"]).mkdir(parents=True, exist_ok=True)
    remaining = list(cards)
    try:
        t = time.time()
        remaining = prepare_batch(cards, queue)
        phases["prepare"] = round(time.time() - t, 1)
        prompt = batch_prompt(remaining, args.way)
        (BATCH_DIR / "prompt.md").write_text(prompt, encoding="utf-8")
        phases["prompt_chars"] = len(prompt)
        place = {"name": names[0], "dirs": [STATE]}
        deadline = started + args.timeout
        while remaining:
            t = time.time()
            code, session, used = spawn(backend, place, prompt, session, max(1, deadline - time.time()), log)
            phases["worker"] = round(phases.get("worker", 0) + time.time() - t, 1)
            usage.add(used, backend.cumulative)
            rounds += 1
            print(f"   round {rounds}: worker {time.time() - t:.0f}s, {usage.input} in, {usage.output} out", flush=True)
            if code is None or (code != 0 and used.turns == 0):
                for card in remaining:
                    card["failure"] = "timeout" if code is None else "the worker did not run"
                break
            if args.way == "draft" and rounds == 1:
                clear_blind_stops(BATCH_DIR / "reports.json")
            expand_reports(BATCH)
            for card in list(remaining):
                report = report_of(card["name"]) or {}
                if report.get("decision") in STOPS:
                    card["failure"] = str(report.get("reason", ""))[:400]
                    discard(card, queue, report["decision"])
                    remaining.remove(card)
            if not remaining:
                break
            t = time.time()
            gates = batch_gates([c["name"] for c in remaining])
            phases["check"] = round(phases.get("check", 0) + time.time() - t, 1)
            print(f"   gates {time.time() - t:.0f}s: " + ", ".join(f"{n}: {'ok' if g.ok else '/'.join(r['gate'] for r in g.failures())}" for n, g in gates.items()), flush=True)
            for card in list(remaining):
                if gates[card["name"]].ok:
                    card["commit"] = commit_card(card)
                    card.update(rounds=rounds)
                    conclude(card, queue, "committed")
                    remaining.remove(card)
                    BATCH.remove(card["name"])
            if not remaining:
                break
            if rounds > args.repairs or time.time() >= deadline:
                for card in remaining:
                    failures = gates[card["name"]].failures()
                    card["failed_gates"] = [r["gate"] for r in failures]
                    card["failure"] = "; ".join(f"{r['gate']}: {r['detail'][:160]}" for r in failures)[:600]
                break
            prompt = batch_repair(gates, args.way, args.repairs + 1 - rounds)
            (BATCH_DIR / f"repair-{rounds}.md").write_text(prompt, encoding="utf-8")
            (BATCH_DIR / "open.json").write_text(json.dumps([c["name"] for c in remaining], ensure_ascii=False), encoding="utf-8")
    except Exception as error:  # a batch's trouble is recorded on its cards, and the tree goes back clean
        for card in remaining:
            card["failure"] = f"{type(error).__name__}: {error}"[:600]
        raise
    finally:
        for card in list(remaining):
            discard(card, queue, "failed_gates")
        close_batch()
        clean_tree()
    tally = collections.Counter(c["status"] for c in cards)
    record = {"cards": names, "way": args.way, "who": who, "effort": args.effort, "rounds": rounds, "seconds": round(time.time() - started, 1), "phases": phases,
              "usage": usage.record(backend.model), "outcomes": dict(tally), "log": str(log),
              "by_card": {c["name"]: {k: c.get(k) for k in ("status", "commit", "failure", "failed_gates")} for c in cards}}
    with (STATE / "batches.jsonl").open("a", encoding="utf-8") as out:
        out.write(json.dumps(record, ensure_ascii=False) + "\n")
    share = {k: (v / max(1, len(cards)) if isinstance(v, (int, float)) else v) for k, v in record["usage"].items()}
    for card in cards:
        card.update(usage={k: round(v, 4) if isinstance(v, float) else v for k, v in share.items()}, phases=phases)
    save_queue(queue)
    return record


def cmd_batch_run(args):
    queue = load_queue()
    recover(queue)
    save_queue(queue)
    backend = BACKENDS[args.backend](args.command if args.backend == "command" else args.model, args.effort)
    names = args.only or []
    if args.only_file:
        names += [line.strip() for line in Path(args.only_file).read_text(encoding="utf-8").splitlines() if line.strip()]
    pending = [c for c in queue["cards"] if c["status"] == "pending" and c.get("attempts", 0) < args.attempts]
    if names:
        order = {n: i for i, n in enumerate(names)}
        pending = sorted((c for c in pending if c["name"] in order), key=lambda c: order[c["name"]])
    pending = pending[:args.limit]
    for start in range(0, len(pending), args.size):
        cards = pending[start:start + args.size]
        print(f"== batch of {len(cards)}: {', '.join(c['name'] for c in cards)}", flush=True)
        record = run_batch(cards, backend, args, queue)
        u = record["usage"]
        print(f"   {record['outcomes']} in {record['seconds']:.0f}s, {record['rounds']} round(s), {u['input']} in/{u['cached']} cached/{u['output']} out, ${u['usd_api_equivalent']}", flush=True)
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
    p = sub.add_parser("batch-run", help="cards together in one worker session")
    p.add_argument("--backend", choices=BACKENDS, default="codex")
    p.add_argument("--model")
    p.add_argument("--effort")
    p.add_argument("--command")
    p.add_argument("--size", type=int, default=10, help="cards in a session")
    p.add_argument("--limit", type=int, default=10, help="cards in all")
    p.add_argument("--only", nargs="*", default=[])
    p.add_argument("--only-file", help="a file of card names, one to a line, in order")
    p.add_argument("--way", choices=WAYS, default="work", help="`work` lets the worker run the tools; `draft` writes everything blind, then a driver sends back what failed")
    p.add_argument("--timeout", type=int, default=3600, help="seconds a batch may take")
    p.add_argument("--repairs", type=int, default=4, help="rounds a worker is sent back with failed gates")
    p.add_argument("--attempts", type=int, default=2)
    p.set_defaults(func=cmd_batch_run)
    p = sub.add_parser("batch-check", help="the gates over the open batch, or one of its cards")
    p.add_argument("--fast", action="store_true")
    p.add_argument("--card")
    p.set_defaults(func=cmd_batch_check)
    if sys.argv[1:2] == ["corpus"]:  # before argparse, which would take a leading `--card` for an option of its own
        return cmd_corpus(sys.argv[2:])
    args = parser.parse_args()
    return args.func(args) or 0


if __name__ == "__main__":
    sys.exit(main())
