#!/usr/bin/env python3
"""Which lines of the generated cards Cantrip's own scenarios never run.

Every whole generated card is put over Cantrip's hand-written one (`cantrip_tree.py swap`), and Cantrip's whole scenario
corpus is run once under coverage. Cantrip's add-cards rule asks that every line of a card be covered, so the scenarios
covered every line of the hand-written cards: a line of a generated card they never reach is either a path the generator
added that no game can take (a shape to fix in the generator) or one no scenario takes yet (a scenario for the lane).

    coverage_scan.py prepare ROWS TREE [--ref REF]   a copy-on-write Cantrip tree at REF, with the cards swapped in
    coverage_scan.py run TREE [--out LCOV]           the corpus under coverage: TREE.lcov and TREE.fails beside TREE
    coverage_scan.py shapes LCOV TREE [--show SHAPE] the missed lines by shape, the generator's first
    coverage_scan.py cards LCOV TREE [--json]        each swapped card's missed lines, the cards with a shape first
    coverage_scan.py scan ROWS TREE                  all of it: prepare (or refresh the swap), run, shapes

`ROWS` is what `ast cantrip-tracked` writes. A tree made once is refreshed in place: its cards are put back as Cantrip has
them and swapped again, so only cantrip-cards and what depends on it rebuild. One scan is about 3 minutes of build and
90 seconds of corpus. Nothing here pushes, commits or touches another tree.

A scenario that fails with the generated cards but passes with Cantrip's own is listed in TREE.fails: the generated card
plays differently, or the scenario leans on how the hand-written card asks its questions.
"""
import argparse
import collections
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.dont_write_bytecode = True
import cantrip_tree  # noqa: E402

BASE = Path("/mnt/vizier/base/cantrip")
COW = dict(COW_WORKTREE_CONFIG="/mnt/vizier/cow-cantrip.conf")
COW_GIT = Path.home() / ".local/bin/cow-git"
LANE_BIN = Path.home() / "card-lane/bin"  # a rustc that reports the system's LLVM tools as its sysroot's
CARDS = "crates/cantrip-cards/src/cards"
# What `swap` writes beside the cards: put back before each swap, so a card no longer whole is Cantrip's again.
SWAPPED_PATHS = [CARDS, cantrip_tree.COUNTER_TABLE, cantrip_tree.TOKEN_TABLE]


def timed(label, started):
    print(f"[{label}: {time.monotonic() - started:.1f}s]", flush=True)


def git(tree, *args, check=True):
    return subprocess.run(["git", *args], cwd=tree, check=check, capture_output=True, text=True).stdout


def prepare(args):
    started = time.monotonic()
    tree = Path(args.tree)
    if not tree.exists():
        name = f"card-lane/{tree.name}"
        env = dict(os.environ, **COW)
        subprocess.run([str(COW_GIT), "-C", str(BASE), "worktree", "add", "-b", name, str(tree), args.ref], env=env, check=True)
    else:
        git(tree, "checkout", "--", *SWAPPED_PATHS)
    timed("tree", started)
    started = time.monotonic()
    cantrip_tree.swap(argparse.Namespace(rows=args.rows, cantrip=str(tree), faces=args.faces))
    timed("swap", started)


def environment(tree):
    path = os.pathsep.join([str(LANE_BIN), str(Path.home() / ".cargo/bin"), "/usr/local/bin", "/usr/bin", "/bin"])
    env = dict(os.environ, PATH=path, CARGO_TARGET_DIR=str(Path(tree) / "target"), CARGO_NET_OFFLINE="true", CANTRIP_VERIFY="1")
    for tool in ("llvm-cov", "llvm-profdata"):
        found = subprocess.run(["bash", "-c", f"command -v {tool}"], env=env, capture_output=True, text=True).stdout.strip()
        env[tool.upper().replace("-", "_")] = found
    return env


def run(args):
    tree = Path(args.tree).resolve()
    out = Path(args.out) if args.out else tree.with_suffix(".lcov")
    env = environment(tree)
    coverage = tree / "target/llvm-cov-target"
    # Only this run's profiles: a `cargo llvm-cov clean` would rebuild every crate.
    for stale in coverage.glob("*.profraw"):
        stale.unlink()
    started = time.monotonic()
    command = ["nice", "cargo", "llvm-cov", "run", "--profile", "cards", "--dep-coverage", "cantrip-cards", "-p", "cantrip-script",
               "--bin", "corpus", "--no-report", "--", "--failing"]
    corpus = subprocess.run(command, cwd=tree, env=env, capture_output=True, text=True)
    timed("build and corpus", started)
    log = out.with_suffix(".run")
    log.write_text(corpus.stdout + corpus.stderr, encoding="utf-8")
    summary = [line for line in corpus.stdout.splitlines() if " scenarios: " in line]
    failing = [line.strip()[len("FAIL "):] for line in corpus.stdout.splitlines() if line.strip().startswith("FAIL ")]
    out.with_suffix(".fails").write_text("".join(f + "\n" for f in failing), encoding="utf-8")
    print(summary[-1] if summary else f"the corpus did not finish (exit {corpus.returncode}); see {log}")
    if not summary:
        return 1
    started = time.monotonic()
    profdata = coverage / "all.profdata"
    subprocess.run([env["LLVM_PROFDATA"], "merge", "-sparse", *map(str, coverage.glob("*.profraw")), "-o", str(profdata)], check=True)
    with out.open("w", encoding="utf-8") as lcov:
        subprocess.run([env["LLVM_COV"], "export", "-format=lcov", f"-instr-profile={profdata}", str(coverage / "cards/corpus")],
                       stdout=lcov, check=True)
    timed("lcov", started)
    print(f"{out}, {len(failing)} failing scenarios in {out.with_suffix('.fails')}")
    return 0


# ---- Shapes -------------------------------------------------------------------------------------------------------------

FILLER = re.compile(r"\}[;,)]*|None|false|true|Ok\(\(\)\)|\}\s*else\s*\{|_ => .*|else \{|else \{ return.*\};?")


def swapped_files(tree):
    """The card files `swap` wrote: every card file the tree has changed."""
    status = git(tree, "status", "--porcelain", "--", CARDS)
    return {line[3:].strip() for line in status.splitlines()}


def read_lcov(lcov, tree, wanted):
    hits, current = collections.defaultdict(dict), None
    root = str(Path(tree).resolve())
    for line in open(lcov, encoding="utf-8"):
        if line.startswith("SF:"):
            rel = os.path.relpath(line[3:].strip(), root)
            current = rel if rel in wanted else None
        elif current and line.startswith("DA:"):
            n, count = line[3:].split(",")[:2]
            hits[current][int(n)] = hits[current].get(int(n), 0) + int(count)
    return hits


def code_of(line):
    return re.sub(r'"(\\.|[^"\\])*"', '""', line.split("//")[0])


def opener(lines, number):
    """The line that opens the block a closing brace at `number` closes."""
    depth = 0
    for i in range(number - 1, -1, -1):
        code = code_of(lines[i])
        for ch in reversed(code if i < number - 1 else code[: code.rfind("}") + 1]):
            if ch == "}":
                depth += 1
            elif ch == "{":
                depth -= 1
                if depth == 0:
                    return i + 1
    return None


def signature(lines, number):
    """The signature of the function a line is in, joined from its lines."""
    for i in range(number - 1, -1, -1):
        if re.match(r"\s*(pub(\([^)]*\))? )?fn ", lines[i]):
            parts = []
            for line in lines[i:number]:
                parts.append(line.strip())
                if line.rstrip().endswith("{"):
                    break
            return " ".join(parts)
    return ""


def role(sig):
    if "proposal: &mut Proposal" in sig:
        return "apply"
    if "proposal: &Proposal" in sig or "before: &Proposal" in sig:
        return "applies"
    if "-> Option<Fired>" in sig:
        return "trigger"
    if "-> Result<(), Suspend>" in sig:
        return "resolve"
    if "-> bool" in sig:
        return "test"
    if "-> CostDelta" in sig or "-> Cost" in sig:
        return "cost"
    return "other"


def shape(lines, n):
    """What kind of line `n` (counted from 1) is: `behaviour` for a line that does something, else what says nothing alone
    (a closing brace, a tail, a fallback arm) with the role of its function and what opened its block."""
    text = lines[n - 1].strip()
    if not FILLER.fullmatch(text):
        return "behaviour"
    o = opener(lines, n) if text.startswith("}") else None
    head = re.sub(r"\b[a-z_]+_\d+\b", "V", lines[o - 1].strip()) if o else text
    kind = (
        "if let Proposal::X" if re.match(r"(\} else )?if let Proposal::", head)
        else "if let Event::X" if re.match(r"(\} else )?if let Event::", head)
        else "if let (.., Some(<always some>))" if re.search(r"if let \(.*Some\((item\.object|asker\.source|asker\.controller)\)", head)
        else "if let Some(..)" if re.match(r"(\} else )?if let Some\(", head)
        else "if let <other>" if re.match(r"(\} else )?if let ", head)
        else "if <condition>" if re.match(r"(\} else )?if ", head)
        else "let-else" if text.startswith("else {")
        else "for" if re.match(r"for ", head)
        else "match" if re.match(r"match ", head)
        else "fn end" if re.match(r"(pub )?fn |\) ->", head)
        else f"tail `{text}`" if text in ("None", "false", "true", "Ok(())")
        else "block" if head in ("{", "} {")
        else "closure" if re.search(r"\|\s*\{$", head)
        else head[:40]
    )
    return f"{role(signature(lines, n))}: {kind} -> `{text}`"


def missed_by_file(lcov, tree):
    tree = Path(tree)
    hits = read_lcov(lcov, tree, swapped_files(tree))
    out = {}
    for rel, counts in sorted(hits.items()):
        lines = (tree / rel).read_text(encoding="utf-8").splitlines()
        missed = [n for n, c in sorted(counts.items()) if c == 0]
        out[rel] = {"lines": len(counts), "missed": [{"line": n, "text": lines[n - 1].strip(), "shape": shape(lines, n)} for n in missed]}
    return out


def shapes(args):
    files = missed_by_file(args.lcov, args.tree)
    counts, by_file, examples = collections.Counter(), collections.defaultdict(set), {}
    for rel, entry in files.items():
        for miss in entry["missed"]:
            counts[miss["shape"]] += 1
            by_file[miss["shape"]].add(rel)
            examples.setdefault(miss["shape"], []).append(f"{os.path.basename(rel)}:{miss['line']}")
    covered = sum(not entry["missed"] for entry in files.values())
    print(f"{len(files)} swapped files instrumented, {covered} fully covered by Cantrip's own scenarios")
    for k, v in counts.most_common(args.top):
        print(f"{len(by_file[k]):5} files {v:5} lines  {k}   e.g. {', '.join(examples[k][:3])}")
    if args.show:
        for k in counts:
            if args.show in k:
                print(f"{k}:\n  " + "\n  ".join(examples[k][:60]))
    if args.json:
        Path(args.json).write_text(json.dumps({"files": len(files), "covered": covered, "shapes": {k: {"lines": v, "files": len(by_file[k])} for k, v in counts.items()}}, indent=1) + "\n")


def cards(args):
    """For the lane: each swapped card that some line of was never run, with its lines; a card whose misses include a shape
    the generator writes comes first, as that is the generator's to fix and not a scenario's."""
    files = missed_by_file(args.lcov, args.tree)
    rows = []
    for rel, entry in files.items():
        if not entry["missed"]:
            continue
        generator = [m for m in entry["missed"] if m["shape"] != "behaviour"]
        rows.append({"file": rel, "stem": Path(rel).stem, "lines": entry["lines"], "missed": len(entry["missed"]),
                     "generator_shapes": len(generator), "lines_missed": entry["missed"]})
    rows.sort(key=lambda r: (-r["generator_shapes"], -r["missed"], r["stem"]))
    if args.json:
        Path(args.json).write_text("".join(json.dumps(r, ensure_ascii=False) + "\n" for r in rows), encoding="utf-8")
        print(f"{len(rows)} cards with lines no scenario ran, {sum(r['generator_shapes'] > 0 for r in rows)} with a generator shape: {args.json}")
        return
    for r in rows:
        print(f"{r['stem']}: {r['missed']} of {r['lines']} lines missed, {r['generator_shapes']} of a generator shape")
        for m in r["lines_missed"]:
            print(f"    {m['line']:4} {m['text'][:90]}   [{m['shape']}]")


def scan(args):
    prepare(args)
    if run(argparse.Namespace(tree=args.tree, out=None)):
        return 1
    shapes(argparse.Namespace(lcov=str(Path(args.tree).resolve().with_suffix(".lcov")), tree=args.tree, show=None, top=args.top, json=None))
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)
    for name, function in (("prepare", prepare), ("scan", scan)):
        command = commands.add_parser(name)
        command.add_argument("rows")
        command.add_argument("tree")
        command.add_argument("--ref", default="b12ea1c1e", help="the Cantrip commit a new tree starts from")
        command.add_argument("--faces", action="store_true", help="swap cards of several faces too")
        command.add_argument("--top", type=int, default=40)
        command.set_defaults(run=function)
    command = commands.add_parser("run")
    command.add_argument("tree")
    command.add_argument("--out", help="the lcov to write, TREE.lcov by default")
    command.set_defaults(run=run)
    command = commands.add_parser("shapes")
    command.add_argument("lcov")
    command.add_argument("tree")
    command.add_argument("--show", help="list the lines of the shapes this names")
    command.add_argument("--top", type=int, default=40)
    command.add_argument("--json", help="also write the counts here")
    command.set_defaults(run=shapes)
    command = commands.add_parser("cards")
    command.add_argument("lcov")
    command.add_argument("tree")
    command.add_argument("--json", help="write one JSON line a card here instead")
    command.set_defaults(run=cards)
    args = parser.parse_args()
    sys.exit(args.run(args) or 0)


if __name__ == "__main__":
    main()
