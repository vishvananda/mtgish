#!/usr/bin/env python3
"""Lay every generated card out in a Cantrip crate of its own, and check it quickly.

`ast cantrip-tracked` writes one JSON row a card. From those rows:

    cantrip_tree.py write ROWS CANTRIP [--sample N]   write crates/cantrip-generated and its sample list
    cantrip_tree.py check CANTRIP [--all]             cargo check the sample, or every card
    cantrip_tree.py tracker ROWS CANTRIP              write data/cantrip_tracker.json and CANTRIP_TRACKER.md
    cantrip_tree.py swap ROWS CANTRIP                 put whole generated cards over hand-written ones, to run scenarios

The crate is never a dependency of cantrip-cards, so nothing in it is a card the
game plays. A card with a gap has `planned!` holes: each names its requirement in
data/cantrip_tracker.json, and the code around it is the card's own.

A whole-corpus compile takes twenty minutes. The sample covers every requirement,
every form of hole and every Cantrip name the files use, with the smallest files
that do, so a change to the generator is checked in about a minute. It is not a
proof: run `--all` before calling a tree clean.
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


COUNTER_TABLE = "crates/cantrip-state/src/vocab.rs"
HELD_VALUE = re.compile(r'planned_value!\("(?:[^"\\]|\\.)*", [\w:]+ => [\w:]+\)')


def counter_block(table):
    """Where the lines of `counter_kinds!` are in the text of vocab.rs."""
    start = table.index("\ncounter_kinds! {\n")
    return start, table.index("\n}\n", start)


def counter_kinds(table):
    start, end = counter_block(table)
    return re.findall(r'^\s+(\w+) = \d+, "', table[start:end], re.M)


def counter_name(word):
    """As Oracle text writes it (CR 122.1)."""
    return re.sub(r"(?<=[a-z])(?=[A-Z])", " ", word).lower()


def counters_wanted(code, table):
    """The counter kinds `code` uses that `table`, the text of vocab.rs, does not list, first used first. A value still
    held back is not used: the generator holds back a counter that needs more than its name."""
    known = {*counter_kinds(table), "ALL", "COUNT"}
    used = re.findall(r"\bCounterKind::([A-Z]\w*)", HELD_VALUE.sub("", code))
    return [word for word in dict.fromkeys(used) if word not in known]


def with_counters(table, words):
    """`table` with `words` appended. Numbers run on from the last: a counter's number indexes an array of `COUNT`, so
    there can be no gap, and appending leaves every existing number alone."""
    start, end = counter_block(table)
    number = len(counter_kinds(table))
    lines = "".join(f'\n    {word} = {number + i}, "{counter_name(word)}", "{word}";' for i, word in enumerate(words))
    return table[:end] + lines + table[end:]


TOKEN_TABLE = "crates/cantrip-mechanics/src/tokens.rs"
NEW_TOKENS = {}


def new_tokens():
    """The tokens several cards make that Cantrip has no static for: {static: {title, cards, source}}, from shared_tokens.py."""
    if not NEW_TOKENS:
        path = Path(__file__).resolve().parents[3] / "data/cantrip_new_tokens.json"
        NEW_TOKENS.update(json.loads(path.read_text(encoding="utf-8")))
    return NEW_TOKENS


def tokens_wanted(code, table):
    """The shared tokens `code` names that `table`, the text of tokens.rs, does not define, first named first."""
    used = re.findall(r"\btokens::([A-Z][A-Z0-9_]*)\b", code)
    for names in re.findall(r"\btokens::\{([^}]*)\}", code):
        used += [name.strip() for name in names.split(",")]
    return [name for name in dict.fromkeys(used) if name in new_tokens() and f"pub static {name}:" not in table]


def with_tokens(table, names):
    """`table` with the definitions of `names`, after the last of its own and before its tests."""
    at = table.rindex("#[cfg(test)]")
    return table[:at] + "".join(new_tokens()[name]["source"] + "\n" for name in names) + table[at:]


def status_of(card):
    # A sticker sheet is no card: it has no name or face, and Cantrip leaves stickers out of the game it plays
    # (docs/rules-exceptions.md, "sub-games (Shahrazad), stickers and the like").
    if not card["faces"]:
        return "excluded"
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
    return re.sub(r"^// (Generated by mtgish-ast, schema \d+, n|Generator SHA256 ).*\n", "", whole(card), flags=re.M)


def whole(card):
    """The card's code. What names the generator is left out above: it would rewrite every file on each change."""
    faces = card["faces"]
    if not faces:
        return ""
    if card.get("rust"):
        return card["rust"]
    if len(faces) == 1:
        return faces[0]["rust"]
    return "".join(f"pub mod face_{i} {{\n{face['rust']}\n}}\n" for i, face in enumerate(faces))


def sample_of(sized, size):
    """Small files that between them use every requirement, hole form and name of Cantrip's API, plus a random spread."""
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
        api = set(re.findall(r"\b[A-Z]\w+::[A-Z]\w+|cantrip_mechanics::\w+::\w+|\bg\.\w+\(|\bq\.\w+\(", text))
        sized.append((stem, len(text), set(card["requirements"]) | forms | fields | api | {card["layout"]}))
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


def rewrites_of(root):
    """What the source rewrites in card text before parsing, read from its rules, and our verdict on each.

    Cantrip plays Oracle text, so a rule with no verdict may be a card played as no one printed it."""
    source = re.sub(r"^\s*//.*$", "", (root / "grammars/find_replace.json5").read_text(), flags=re.M)
    finds = [json.loads(f'"{found}"') for found in re.findall(r'"find":\s*"((?:[^"\\]|\\.)*)"', source)]
    verdicts = json.loads((root / "data/cantrip_rewrites.json").read_text())
    known = {row["find"] for row in verdicts}
    return [row for row in verdicts if row["find"] in finds], [find for find in finds if find not in known]


def rule_for(feature, rules):
    for rule in rules:
        if re.search(rule["match"], feature):
            return rule
    raise SystemExit(f"no rule for {feature}")


def family(feature):
    words = feature.split()
    return " ".join(words[:2]) if len(words) > 1 else feature


# What stands between a card and its file, the hardest last. Each form's verdict is `data/cantrip_audit.json`: the API read
# against the hand-written cards, not a guess from the form's family. `card-code` of a keyword is a module for Cantrip's
# mechanics crate, since a keyword is not written out in every card that has it; the core need not change.
FIXES = ("generator", "mechanics module", "unsure", "unaudited", "mtgish data", "Cantrip", "not played")
NEW_MODULE = re.compile(r"\b(new|a plain function in|function in|module)\b.{0,40}cantrip[-_]mechanics|cantrip[-_]mechanics.{0,30}\b(new|module)\b|in the mechanics crate|new mechanics", re.I)
NOT_PLAYED = re.compile(r"game mode Cantrip does not play|casual variant Cantrip does not play|Alchemy|^digital:|casual-format", re.I)


def fix_of(row, audit):
    if NOT_PLAYED.search(row["needs"]):
        return "not played"
    verdict = audit.get(row["id"], {}).get("verdict")
    if verdict is None:
        return "unaudited"
    module = verdict == "card-code" and (row["class"] == "mechanic" or NEW_MODULE.search(audit[row["id"]].get("how", "")))
    if verdict == "exists" or (verdict == "card-code" and not module):
        return "generator"
    return {"card-code": "mechanics module", "unsure": "unsure", "data": "mtgish data"}.get(verdict, "Cantrip")


def hardest(fixes):
    return max(fixes, key=FIXES.index) if fixes else "generator"


def features_of(rows, blocked, written):
    """Rows by what they need. A row is one form the generator could not write, so one fix can be many rows: a new counter
    kind is a row for each name. {needs, class, forms, cards, written, examples}, the most cards first."""
    features = {}
    for row in rows:
        entry = features.setdefault(row["needs"], {"needs": row["needs"], "class": row["class"], "forms": [], "cards": set(), "written": set(), "fixes": set()})
        entry["forms"].append(row["id"])
        entry["fixes"].add(row["fix"])
        entry["cards"].update(name for name, ids in blocked.items() if row["id"] in ids)
        entry["written"].update(name for name, ids in written.items() if row["id"] in ids)
    out = [{**e, "fixes": sorted(e["fixes"]), "fix": hardest(e["fixes"]), "examples": sorted(e["cards"])[:3], "cards": len(e["cards"]), "written": len(e["written"])} for e in features.values()]
    return sorted(out, key=lambda e: (-e["cards"], -e["written"], e["needs"]))


def written_by_hand(cantrip):
    """The file stems of the cards Cantrip already has, so that what they ask of it is not reported as missing."""
    return {path.stem.removeprefix("r#") for path in (Path(cantrip) / "crates/cantrip-cards/src/cards").glob("*.rs")}


def tracker(args):
    root = Path(__file__).resolve().parents[3]
    rules = load_rules(root)
    needs = {}
    totals = {"cards": 0, "ready": 0, "planned": 0, "excluded": 0}
    layouts = {}
    blocked = {}
    written = {}
    hand = written_by_hand(args.cantrip)
    table = (Path(args.cantrip) / COUNTER_TABLE).read_text(encoding="utf-8")
    counters = {}
    token_table = (Path(args.cantrip) / TOKEN_TABLE).read_text(encoding="utf-8")
    tokens = {}
    for card in read_rows(args.rows):
        status = status_of(card)
        for word in counters_wanted(whole(card), table):
            counters.setdefault(word, []).append(card["name"])
        for token in tokens_wanted(card.get("file") or "", token_table):  # the PR form names shared tokens, the module does not
            tokens.setdefault(token, []).append(card["name"])
        totals["cards"] += 1
        totals[status] += 1
        layouts.setdefault(card["layout"], [0, 0])
        layouts[card["layout"]][status == "planned"] += 1
        if status == "planned":
            done = stem_of(card["name"].split(" // ")[0]) in hand
            (written if done else blocked)[card["name"]] = [slug(f) for f in card["requirements"]]
            for feature in card["requirements"]:
                row = needs.setdefault(feature, {"cards": 0, "written": 0, "alone": 0, "examples": []})
                if done:
                    row["written"] += 1
                    continue
                row["cards"] += 1
                row["alone"] += len(card["requirements"]) == 1
                if len(row["examples"]) < 5:
                    row["examples"].append(card["name"])
    totals["written"] = len(written)
    totals["new_tokens"] = len(tokens)
    totals["cards_with_new_tokens"] = len({name for names in tokens.values() for name in names})
    totals["counter_lines"] = len(counters)
    totals["cards_with_counter_lines"] = len({name for names in counters.values() for name in names})
    audit = json.loads((root / "data/cantrip_audit.json").read_text(encoding="utf-8"))
    rows = []
    for feature, row in sorted(needs.items(), key=lambda kv: (-kv[1]["cards"], -kv[1]["written"], kv[0])):
        rule = rule_for(feature, rules)
        rows.append({"id": slug(feature), "feature": feature, "class": rule["class"], "needs": rule["needs"],
                     "family": family(feature), **row})
        rows[-1]["fix"] = fix_of(rows[-1], audit)
        rows[-1]["how"] = audit.get(rows[-1]["id"], {}).get("how", "")
    base = subprocess.run(["git", "merge-base", "HEAD", "origin/main"], cwd=args.cantrip, text=True,
                          capture_output=True, check=True).stdout.strip()[:10]
    classes = {}
    for row in rows:
        entry = classes.setdefault(row["class"], {"requirements": 0, "cards": set()})
        entry["requirements"] += 1
        entry["cards"].update(name for name, ids in blocked.items() if row["id"] in ids)
    summary = {name: {"requirements": e["requirements"], "cards": len(e["cards"])} for name, e in classes.items()}
    features = features_of(rows, blocked, written)
    fix_by_id = {row["id"]: row["fix"] for row in rows}
    by_fix = {}
    for name, ids in blocked.items():
        fix = hardest([fix_by_id[i] for i in ids])
        by_fix[fix] = by_fix.get(fix, 0) + 1
    tracked = {"cantrip": base, "totals": totals, "layouts": {k: {"ready": v[0], "planned": v[1]} for k, v in sorted(layouts.items())},
               "classes": summary, "fixes": by_fix, "features": features, "new_tokens": {token: sorted(names) for token, names in sorted(tokens.items())}, "counters": {word: sorted(names) for word, names in sorted(counters.items())},
               "requirements": rows, "cards": blocked, "written": written}
    out = Path(args.out) if args.out else root
    (out / "data").mkdir(exist_ok=True)
    (out / "data/cantrip_tracker.json").write_text(json.dumps(tracked, indent=1, ensure_ascii=False) + "\n")
    lines = ["# Cantrip tracker", "",
             "Generated by `rust_syntax/normalize/tools/cantrip_tree.py tracker`. Do not edit: the classes come from",
             "`data/cantrip_requirements.json`, the rest from what the generator emits.", "",
             f"Against Cantrip {base}: {totals['cards']} records, {totals['ready']} whole, "
             f"{totals['planned']} with holes, {totals['excluded']} sticker sheets, which Cantrip leaves out of its game "
             "(its docs/rules-exceptions.md: stickers) and which have no card face to write.",
             f"Of those with holes Cantrip has written {len(written)} by hand, which leaves {len(blocked)}.", "",
             "A card with holes has its file all the same. Each `planned!` in it names a requirement below.",
             "`cards` counts what Cantrip does not have yet. `written` counts the cards Cantrip has written by hand:",
             "at least some of what that row asks can be done there, so it may be a gap in the generator, not in the engine.", "",
             "A requirement is one form the generator could not write, not one piece of work, and `class` comes from the form's",
             "family, not from anyone having tried. `fix` is from `data/cantrip_audit.json`, where each form's verdict cites the API",
             "or a hand-written card. It comes from reading the code: nothing there has been compiled or played.", "",
             "What stands between each remaining card and its file, counting the hardest thing a card needs:", "",
             "| fix | cards | meaning |", "|---|---|---|"]
    meaning = {"generator": "Cantrip can do it today: the generator has only to write it", "mechanics module": "a keyword's module for the mechanics crate, no change to the core",
               "unsure": "the audit could not settle it", "unaudited": "a form that appeared after the audit",
               "mtgish data": "the mtgish data encodes the card wrongly: fixed upstream, not in Cantrip", "Cantrip": "needs a change to Cantrip's engine or API",
               "not played": "a game mode or a digital-only rule Cantrip does not play"}
    lines += [f"| {k} | {by_fix.get(k, 0)} | {meaning[k]} |" for k in FIXES]
    lines += ["", "| class | requirements | cards |", "|---|---|---|"]
    lines += [f"| {k} | {v['requirements']} | {v['cards']} |" for k, v in sorted(summary.items())]
    lines += ["", f"{totals['new_tokens']} tokens are made by several cards and Cantrip has no static for them, and "
              f"{totals['cards_with_new_tokens']} cards name them. The lane commits each to `tokens.rs` before the first card that needs it, "
              "as it does a counter: `tools/shared_tokens.py` finds them."]
    shared = [f for f in features if f["cards"] > 1 or len(f["forms"]) > 1]
    single = [f for f in features if f["cards"] == 1 and len(f["forms"]) == 1]
    lines += ["", "## Features", "", f"What the forms below come to: {len(shared)} features more than one card or form shares, most cards first.", "",
              "| cards | written | forms | fix | needs | examples |", "|---|---|---|---|---|---|"]
    lines += [f"| {f['cards']} | {f['written']} | {len(f['forms'])} | {f['fix']} | {f['needs']} | {', '.join(f['examples'])} |" for f in shared]
    lines += ["", "## Asked by one card", "", f"{len(single)} forms that one remaining card has and nothing shares. Most are likely to be code in that card's file.", "",
              "| card | written | fix | needs |", "|---|---|---|---|"]
    lines += [f"| {f['examples'][0] if f['examples'] else ''} | {f['written']} | {f['fix']} | {f['needs']} |" for f in single]
    lines += ["", "## Counters a card brings with it", "",
              f"{len(counters)} counters are a name and nothing more (CR 122.1), and {totals['cards_with_counter_lines']} cards use them. They are not gaps: "
              "the card's file uses the kind, and a commit just before the card adds the line to `counter_kinds!`.", "",
              "| counter | cards | examples |", "|---|---|---|"]
    lines += [f"| {counter_name(word)} | {len(names)} | {', '.join(sorted(names)[:3])} |" for word, names in sorted(counters.items(), key=lambda kv: (-len(kv[1]), kv[0]))]
    rewrites, unread = rewrites_of(root)
    lines += ["", "## Text the source rewrites", "",
              f"The source changes {len(rewrites) + len(unread)} passages of card text on purpose before parsing (`grammars/find_replace.json5`), "
              "and Cantrip plays Oracle text, so each has a verdict in `data/cantrip_rewrites.json`. "
              f"{len(unread)} have none yet. Those that change nothing or sit on cards not played are not listed.", ""]
    lines += [f"- not read yet: `{find}`" for find in unread]
    lines += ["| cards | verdict | note |", "|---|---|---|"]
    lines += [f"| {', '.join(row['cards'])} | {row['verdict']} | {row.get('note', '')} |" for row in rewrites if row["verdict"] not in ("same", "not played")]
    lines += ["", "## Every requirement", "", "| requirement | class | fix | cards | written | alone | needs | examples |", "|---|---|---|---|---|---|---|---|"]
    for row in rows:
        if row["cards"]:
            lines.append(f"| `{row['id']}` | {row['class']} | {row['fix']} | {row['cards']} | {row['written']} | {row['alone']} | {row['needs']} | "
                         + ", ".join(row["examples"][:3]) + " |")
    done = [row for row in rows if not row["cards"]]
    lines += ["", "## Only on cards Cantrip has written", "",
              f"Cantrip has every card on these {len(done)} rows, so none of them is a missing feature. They remain gaps in the generator.", "",
              "| requirement | class | written |", "|---|---|---|"]
    lines += [f"| `{row['id']}` | {row['class']} | {row['written']} |" for row in done]
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


def file_stem_of(card):
    """The file a card is: its name's, or for a card of several faces the one its `CARD` is named for (`..oracle::DELVER_OF_SECRETS`
    is delver_of_secrets.rs, a front face's name). None for a card of several faces with no PR form."""
    if card["layout"] == "Card":
        return stem_of(card["name"])
    base = re.search(r"pub static CARD: CardDef = CardDef \{.*?\.\.oracle::(\w+)", card.get("file") or "", re.S)
    return base.group(1).lower() if base else None


def swap(args):
    """Replace each hand-written plain card that has whole generated code, so Cantrip's own scenarios judge it; with
    --faces, cards of several faces too."""
    cards = Path(args.cantrip) / "crates/cantrip-cards/src/cards"
    counter_path, token_path = Path(args.cantrip) / COUNTER_TABLE, Path(args.cantrip) / TOKEN_TABLE
    counter_table, token_table = counter_path.read_text(encoding="utf-8"), token_path.read_text(encoding="utf-8")
    swapped, counters, tokens = [], {}, {}
    for card in read_rows(args.rows):
        if card["layout"] != "Card" and not args.faces:
            continue
        stem = file_stem_of(card)
        path = cards / f"{stem}.rs"
        if stem and status_of(card) == "ready" and path.exists():
            code = card.get("file") or (header(card) + body(card))
            path.write_text(code, encoding="utf-8")
            swapped.append(card["name"])
            counters.update(dict.fromkeys(counters_wanted(code, counter_table)))
            tokens.update(dict.fromkeys(tokens_wanted(code, token_table)))
    if counters:
        counter_path.write_text(with_counters(counter_table, list(counters)), encoding="utf-8", newline="\n")
    if tokens:
        token_path.write_text(with_tokens(token_table, list(tokens)), encoding="utf-8", newline="\n")
    (Path(args.cantrip) / "swapped.txt").write_text("".join(n + "\n" for n in swapped))
    print(f"{len(swapped)} hand-written cards replaced, {len(counters)} counters and {len(tokens)} tokens added")


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
    command = commands.add_parser("swap")
    command.add_argument("rows")
    command.add_argument("cantrip")
    command.add_argument("--faces", action="store_true", help="also cards of several faces that have a PR form")
    command.set_defaults(run=swap)
    command = commands.add_parser("show")
    command.add_argument("rows")
    command.add_argument("name")
    command.set_defaults(run=show)
    args = parser.parse_args()
    sys.exit(args.run(args))


if __name__ == "__main__":
    main()
