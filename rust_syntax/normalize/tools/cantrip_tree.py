#!/usr/bin/env python3
"""Lay every generated card out in a Cantrip crate of its own, and check it quickly.

`ast cantrip-tracked` writes one JSON row a card. From those rows:

    cantrip_tree.py write ROWS CANTRIP [--sample N]   write crates/cantrip-generated and its sample list
    cantrip_tree.py check CANTRIP [--all]             cargo check the sample, or every card
    cantrip_tree.py tracker ROWS CANTRIP              write data/cantrip_tracker.json and CANTRIP_TRACKER.md

The crate is never a dependency of cantrip-cards, so nothing in it is a card the
game plays. A card with a gap has `planned!` holes: each names its requirement in
data/cantrip_tracker.json, and the code around it is the card's own.

A whole-corpus compile takes twenty minutes. The sample covers every requirement
and every form of hole with the smallest files that do, so a change to the
generator is checked in about a minute.
"""
import argparse
import json
import os
import random
import re
import subprocess
import sys
import unicodedata
from pathlib import Path

CRATE = "crates/cantrip-generated"
TARGET = Path.home() / ".cache/mtgish-ast-dev/build-cantrip-generated"

MANIFEST = """[package]
name = "cantrip-generated"
description = "Every card as mtgish generates it. Nothing here is registered as a playable card."
version.workspace = true
edition.workspace = true
publish.workspace = true
license.workspace = true

[features]
# Compile every card, not the sample: twenty minutes.
all = []

[dependencies]
cantrip-api.workspace = true
cantrip-mechanics.workspace = true
"""

LIB = """//! Every card as mtgish generates it, tracked by data/cantrip_tracker.json there.
//!
//! `planned!` stands where a card needs something the generator or Cantrip does
//! not have yet. `planned!("id" => code)` holds the code the card will have once
//! that exists, and `planned!("id", Form, "field" = value)` holds what the
//! generator knows so far, its values still type-checked.
//! `planned_value!("feature", placeholder => code)` compiles as the placeholder and
//! will say `code`. The rest of the file is the card's own code. A card with a hole
//! never runs.
#![allow(dead_code, unused_imports, unused_mut, unused_variables, unused_macros)]
#![allow(unused_assignments, unused_must_use, unreachable_code)]

macro_rules! planned {
    ($id:literal => $($code:tt)*) => {};
    ($id:literal, $form:ident $(, $field:literal = $value:expr)* $(,)?) => {
        $(let _ = &$value;)*
    };
}
macro_rules! planned_value {
    ($feature:literal, $placeholder:expr => $($code:tt)*) => {
        $placeholder
    };
}
macro_rules! planned_item {
    ($id:literal => $($code:tt)*) => {};
    ($id:literal, $source:literal) => {
        const _: &str = $source;
    };
}

include!(concat!(env!("OUT_DIR"), "/modules.rs"));
"""

BUILD = """//! The modules to compile: the sample, or with the `all` feature every card.
use std::fmt::Write;

fn main() {
    println!("cargo:rerun-if-changed=sample.txt");
    println!("cargo:rerun-if-changed=build.rs");
    let root = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let stems: Vec<String> = if std::env::var_os("CARGO_FEATURE_ALL").is_some() {
        let mut all: Vec<String> = std::fs::read_dir(root.join("cards"))
            .unwrap()
            .map(|e| e.unwrap().path().file_stem().unwrap().to_string_lossy().to_string())
            .collect();
        all.sort();
        all
    } else {
        std::fs::read_to_string(root.join("sample.txt")).unwrap().lines().map(String::from).collect()
    };
    let mut out = String::new();
    for (i, stem) in stems.iter().enumerate() {
        let path = root.join("cards").join(format!("{stem}.rs"));
        writeln!(out, "#[path = {:?}]\\nmod card_{i};", path.to_string_lossy()).unwrap();
    }
    let out_dir = std::env::var_os("OUT_DIR").unwrap();
    std::fs::write(std::path::Path::new(&out_dir).join("modules.rs"), out).unwrap();
}
"""


def slug(feature):
    """The id `requirement` in planned.rs gives a gap feature."""
    out = ""
    for c in feature:
        if c.isascii() and c.isalnum():
            if c.isupper() and out[-1:].islower():
                out += "-"
            out += c.lower()
        elif out and not out.endswith("-"):
            out += "-"
    return out.rstrip("-")


def stem_of(name):
    ascii_name = "".join(c for c in unicodedata.normalize("NFKD", name) if not unicodedata.combining(c))
    stem = re.sub(r"[^a-z0-9]+", "_", ascii_name.replace("'", "").lower()).strip("_")
    return stem or "unnamed"


def status_of(card):
    if not card["faces"]:
        return "data"
    return "planned" if card["requirements"] else "ready"


def read_rows(path):
    with open(path, encoding="utf-8") as rows:
        for row in rows:
            card = json.loads(row)
            card["requirements"] = sorted(
                {gap["feature"] for face in card["faces"] for gap in face["gaps"]}
            )
            yield card


def header(card):
    lines = [f"// {card['name']}", f"// status: {status_of(card)}"]
    lines += [f"// requires: {feature}" for feature in card["requirements"]]
    for face in card["faces"]:
        if face.get("oracle"):
            lines += [f"// {line}" for line in face["oracle"].split("\n")]
    return "\n".join(lines) + "\n"


def body(card):
    faces = card["faces"]
    if not faces:
        return ""
    if len(faces) == 1:
        return faces[0]["rust"]
    return "".join(f"pub mod face_{i} {{\n{face['rust']}\n}}\n" for i, face in enumerate(faces))


def sample_of(sized, size):
    """The fewest small files that hold every requirement and every hole form, plus a random spread."""
    covers = {}
    for stem, length, keys in sized:
        for key in keys:
            if key not in covers or length < covers[key][0]:
                covers[key] = (length, stem)
    chosen = {stem for _, stem in covers.values()}
    rest = sorted(stem for stem, _, _ in sized if stem not in chosen)
    random.Random(164).shuffle(rest)
    return sorted(chosen | set(rest[: max(0, size - len(chosen))]))


def write(args):
    crate = Path(args.cantrip) / CRATE
    cards = crate / "cards"
    if not args.standalone:
        cards.mkdir(parents=True, exist_ok=True)
        (crate / "src").mkdir(exist_ok=True)
        (crate / "Cargo.toml").write_text(MANIFEST)
        (crate / "src/lib.rs").write_text(LIB)
        (crate / "build.rs").write_text(BUILD)
    if args.standalone:
        # A lane's own copy: outside the Cantrip tree, so lanes share its sources.
        source = Path(args.standalone).resolve()
        crate = source
        cards = crate / "cards"
        cards.mkdir(parents=True, exist_ok=True)
        (crate / "src").mkdir(exist_ok=True)
        (crate / "src/lib.rs").write_text(LIB)
        (crate / "build.rs").write_text(BUILD)
        path = Path(args.cantrip).resolve() / "crates"
        (crate / "Cargo.toml").write_text(
            MANIFEST.replace("version.workspace = true", 'version = "0.0.0"')
            .replace("edition.workspace = true", 'edition = "2024"')
            .replace("publish.workspace = true\n", "")
            .replace("license.workspace = true\n", "")
            .replace("cantrip-api.workspace = true", f'cantrip-api = {{ path = "{path}/cantrip-api" }}')
            .replace("cantrip-mechanics.workspace = true", f'cantrip-mechanics = {{ path = "{path}/cantrip-mechanics" }}')
            + "\n[workspace]\n"
        )
    else:
        root = Path(args.cantrip) / "Cargo.toml"
        manifest = root.read_text()
        if f'"{CRATE}"' not in manifest:
            root.write_text(manifest.replace('members = [\n', f'members = [\n    "{CRATE}",\n', 1))
    wanted, sized, taken = set(), [], {}
    for card in read_rows(args.rows):
        stem = stem_of(card["name"])
        taken[stem] = taken.get(stem, 0) + 1
        if taken[stem] > 1:
            stem = f"{stem}_{card['line']}"
        wanted.add(stem)
        text = header(card) + body(card)
        path = cards / f"{stem}.rs"
        if not path.exists() or path.read_text(encoding="utf-8") != text:
            path.write_text(text, encoding="utf-8")
        forms = set(re.findall(r'planned!\("[^"]*", (\w+)', text))
        fields = set(re.findall(r'" = \{ let q', text))
        sized.append((stem, len(text), set(card["requirements"]) | forms | fields | {card["layout"]}))
    for stale in cards.glob("*.rs"):
        if stale.stem not in wanted:
            stale.unlink()
    sample = sample_of(sized, args.sample)
    (crate / "sample.txt").write_text("".join(s + "\n" for s in sample))
    print(f"{len(wanted)} cards, sample {len(sample)}")


def check(args):
    command = ["cargo", "check", "-q", "--message-format=short"]
    if not args.standalone:
        command += ["-p", "cantrip-generated"]
    if args.all:
        command += ["--features", "all"]
    env = dict(os.environ, CARGO_INCREMENTAL="0", CARGO_PROFILE_DEV_DEBUG="0")
    env["CARGO_TARGET_DIR"] = args.target or str(TARGET)
    cwd = args.standalone or args.cantrip
    return subprocess.run(command, cwd=cwd, env=env).returncode


def load_rules(root):
    """A file of requirements found on one family comes before the catch-alls of the base file: first match decides."""
    paths = sorted(root.glob("data/cantrip_requirements_*.json")) + [root / "data/cantrip_requirements.json"]
    return [rule for path in paths for rule in json.loads(path.read_text())]


def rule_for(feature, rules):
    for rule in rules:
        if re.search(rule["match"], feature):
            return rule
    raise SystemExit(f"no rule for {feature}")


def family(feature):
    words = feature.split()
    return " ".join(words[:2]) if len(words) > 1 else feature


def tracker(args):
    root = Path(__file__).resolve().parents[3]
    rules = load_rules(root)
    needs = {}
    totals = {"cards": 0, "ready": 0, "planned": 0, "data": 0}
    layouts = {}
    blocked = {}
    for card in read_rows(args.rows):
        status = status_of(card)
        totals["cards"] += 1
        totals[status] += 1
        layouts.setdefault(card["layout"], [0, 0])
        layouts[card["layout"]][status == "planned"] += 1
        if status == "planned":
            blocked[card["name"]] = [slug(f) for f in card["requirements"]]
            for feature in card["requirements"]:
                row = needs.setdefault(feature, {"cards": 0, "alone": 0, "examples": []})
                row["cards"] += 1
                row["alone"] += len(card["requirements"]) == 1
                if len(row["examples"]) < 5:
                    row["examples"].append(card["name"])
    rows = []
    for feature, row in sorted(needs.items(), key=lambda kv: (-kv[1]["cards"], kv[0])):
        rule = rule_for(feature, rules)
        rows.append({"id": slug(feature), "feature": feature, "class": rule["class"], "needs": rule["needs"],
                     "family": family(feature), **row})
    base = subprocess.run(["git", "merge-base", "HEAD", "origin/main"], cwd=args.cantrip, text=True,
                          capture_output=True, check=True).stdout.strip()[:10]
    classes = {}
    for row in rows:
        entry = classes.setdefault(row["class"], {"requirements": 0, "cards": set()})
        entry["requirements"] += 1
        entry["cards"].update(name for name, ids in blocked.items() if row["id"] in ids)
    summary = {name: {"requirements": e["requirements"], "cards": len(e["cards"])} for name, e in classes.items()}
    tracked = {"cantrip": base, "totals": totals, "layouts": {k: {"ready": v[0], "planned": v[1]} for k, v in sorted(layouts.items())},
               "classes": summary, "requirements": rows, "cards": blocked}
    out = Path(args.out) if args.out else root
    (out / "data").mkdir(exist_ok=True)
    (out / "data/cantrip_tracker.json").write_text(json.dumps(tracked, indent=1, ensure_ascii=False) + "\n")
    lines = ["# Cantrip tracker", "",
             "Generated by `rust_syntax/normalize/tools/cantrip_tree.py tracker`. Do not edit: the classes come from",
             "`data/cantrip_requirements.json`, the rest from what the generator emits.", "",
             f"Against Cantrip {base}: {totals['cards']} records, {totals['ready']} whole, "
             f"{totals['planned']} with holes, {totals['data']} with no card face.", "",
             "A card with holes has its file all the same. Each `planned!` in it names a requirement below.", "",
             "| class | requirements | cards |", "|---|---|---|"]
    lines += [f"| {k} | {v['requirements']} | {v['cards']} |" for k, v in sorted(summary.items())]
    lines += ["", "| requirement | class | cards | alone | needs | examples |", "|---|---|---|---|---|---|"]
    for row in rows:
        lines.append(f"| `{row['id']}` | {row['class']} | {row['cards']} | {row['alone']} | {row['needs']} | "
                     + ", ".join(row["examples"][:3]) + " |")
    (out / "CANTRIP_TRACKER.md").write_text("\n".join(lines) + "\n")
    print(json.dumps({"totals": totals, "classes": summary}))


def todo(args):
    """What is left: requirement, cards it is on, cards only it stops, examples."""
    root = Path(__file__).resolve().parents[3]
    rules = load_rules(root)
    needs = {}
    for card in read_rows(args.rows):
        for feature in card["requirements"]:
            if re.search(args.match, feature) and not (args.exclude and re.search(args.exclude, feature)):
                row = needs.setdefault(feature, [0, 0, []])
                row[0] += 1
                row[1] += len(card["requirements"]) == 1
                if len(row[2]) < 3:
                    row[2].append(card["name"])
    shown = 0
    for feature, (cards, alone, examples) in sorted(needs.items(), key=lambda kv: -kv[1][0]):
        kind = rule_for(feature, rules)["class"]
        if args.all or kind == "generator":
            print(f"{cards:5} {alone:5} {kind[:4]} {feature} | {'; '.join(examples)}")
            shown += 1
    print(f"{shown} requirements")


def show(args):
    """A card's file and what it waits on."""
    for card in read_rows(args.rows):
        if args.name in (card["name"], *[face["name"] for face in card["faces"]]):
            print(header(card) + body(card))
            for face in card["faces"]:
                for gap in face["gaps"]:
                    print(f"// gap {gap['path']}: {gap['feature']}")
            return
    raise SystemExit(f"no card {args.name}")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)
    command = commands.add_parser("write")
    command.add_argument("rows")
    command.add_argument("cantrip")
    command.add_argument("--sample", type=int, default=400)
    command.add_argument("--standalone", help="write a crate of its own here instead of into the Cantrip tree")
    command.set_defaults(run=write)
    command = commands.add_parser("check")
    command.add_argument("cantrip")
    command.add_argument("--all", action="store_true")
    command.add_argument("--standalone", help="the crate `write --standalone` made")
    command.add_argument("--target", help="cargo's target directory")
    command.set_defaults(run=check)
    command = commands.add_parser("tracker")
    command.add_argument("rows")
    command.add_argument("cantrip")
    command.add_argument("--out", help="where to write, instead of the repository")
    command.set_defaults(run=tracker)
    command = commands.add_parser("todo")
    command.add_argument("rows")
    command.add_argument("--match", default=".", help="only features matching this")
    command.add_argument("--not", dest="exclude", help="no features matching this")
    command.add_argument("--all", action="store_true", help="include requirements already classed outside the generator")
    command.set_defaults(run=todo)
    command = commands.add_parser("show")
    command.add_argument("rows")
    command.add_argument("name")
    command.set_defaults(run=show)
    args = parser.parse_args()
    sys.exit(args.run(args))


if __name__ == "__main__":
    main()
