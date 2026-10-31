#!/usr/bin/env python3
"""Play generated cards in real games under Cantrip's own verification.

    cantrip_smoke.py <cantrip checkout> <rows.jsonl> <ast binary> <corpus> <crate dir>
                     [--seed N] [--cards N] [--per-seat N] [--games N] [--decisions N]
                     [--workers N] [--minutes N] [--only NAME ...]

Compiling a generated module says nothing about what it does when it runs, and
Cantrip keeps checks that only fire in a verified game: a card all players saw,
moved into a hand by a move not marked revealed, compiles and then panics. So
this deals a seeded sample of generated cards into decks and has Cantrip's own
random seat play them against each other, recording each panic and refused
answer with the card on top of the stack.

A clean run shows only that these cards did not crash on the boards random play
happened to reach. It asserts nothing about what a card does, so it never counts
as a scenario and never closes a gap. It finds bugs; it cannot pass a card. The
report says how many sampled cards really resolved, since a card never cast was
not tested at all.
"""
import argparse
import collections
import json
import os
import random
import re
import subprocess
import sys
from pathlib import Path

BASICS = ["Plains", "Island", "Swamp", "Mountain", "Forest"]
LANDS_OF_EACH = 7

MAIN = r"""
use cantrip_bench::{Random, decide};
use cantrip_engine::{Answer, CardDef, CardTable, Event, Game, ObjectId, Outside, Rejected, RequestKind, Target, deal_on};
use std::cell::RefCell;
use std::io::Write;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicUsize, Ordering};

thread_local! { static PANIC: RefCell<String> = const { RefCell::new(String::new()) }; }

fn table() -> &'static CardTable {
    let mut definitions: Vec<&'static CardDef> = Vec::new();
    for (card, extra) in MODULES {
        definitions.push(*card);
        definitions.extend_from_slice(extra);
    }
    Box::leak(Box::new(CardTable::new(definitions)))
}

fn deck(names: &[&str]) -> Vec<String> {
    let mut deck: Vec<String> = Vec::new();
    for name in names {
        for _ in 0..COPIES {
            deck.push(name.to_string());
        }
    }
    for land in LANDS {
        for _ in 0..LANDS_OF_EACH {
            deck.push(land.to_string());
        }
    }
    deck
}

fn line(text: String) -> String {
    text.replace(['\n', '\t'], " ")
}

fn name_of(g: &Game, id: ObjectId) -> String {
    g.cards.get(g.state.obj(id).card).name.to_string()
}

/// The object a panic names: which card it is, and where the game has it.
fn describe(g: &Game, message: &str) -> String {
    let Some(id) = message.split("ObjectId(").nth(1).and_then(|rest| rest.split(')').next()?.parse().ok()) else {
        return String::new();
    };
    let id = ObjectId(id);
    let o = g.state.obj(id);
    line(format!(
        "{} owner {:?} controller {:?} zone {:?} attached to {:?}; attached here {:?}",
        name_of(g, id),
        o.owner,
        o.controller,
        o.zone,
        o.attached_to.map(|host| name_of(g, host)),
        g.state.battlefield().iter().filter(|&&a| g.state.obj(a).attached_to == Some(id)).map(|&a| name_of(g, a)).collect::<Vec<_>>(),
    ))
}

fn play(table: &'static CardTable, group: usize, seed: u64, out: &mut Vec<String>) {
    let (mine, theirs) = GROUPS[group];
    let decks = vec![deck(mine), deck(theirs)];
    out.push(format!("DECK\t{group}\t{seed}\t{}", [mine, theirs].concat().join("|")));
    let dealt = catch_unwind(AssertUnwindSafe(|| {
        let mut g = deal_on(table, &decks, &Outside::default(), seed, true, None).expect("decks of known cards");
        g.verify = true;
        g
    }));
    let Ok(mut g) = dealt else {
        out.push(format!("PANIC\t{group}\t{seed}\t(the deal)\t{}", PANIC.with(|p| p.borrow().clone())));
        return;
    };
    let mut random = Random::new(seed);
    let mut decisions = 0u32;
    let run = catch_unwind(AssertUnwindSafe(|| {
        while let Some(request) = g.pending().cloned() {
            if decisions >= DECISIONS {
                break;
            }
            let answer = match &request.kind {
                // The random seat picks distinct modes, which is not an answer when
                // fewer modes than it must choose have legal targets and the card
                // lets a mode repeat (CR 700.2d).
                RequestKind::ChooseModes { eligible, count, repeat: true, .. }
                    if eligible.len() < *count as usize && !eligible.is_empty() =>
                {
                    Answer::Modes(eligible.iter().cycle().take(*count as usize).copied().collect())
                }
                _ => decide(&g, &mut random, &request),
            };
            if let Err(Rejected(why)) = g.answer(request.id, request.player, answer.clone()) {
                panic!("REFUSED {why} to {:?}", request.kind);
            }
            decisions += 1;
        }
    }));
    if run.is_err() {
        let on_top = catch_unwind(AssertUnwindSafe(|| {
            g.state.stack.last().map(|item| name_of(&g, item.object)).unwrap_or_else(|| "(empty stack)".to_string())
        }))
        .unwrap_or_else(|_| "(unknown)".to_string());
        let message = PANIC.with(|p| p.borrow().clone());
        let about = catch_unwind(AssertUnwindSafe(|| describe(&g, &message))).unwrap_or_default();
        out.push(format!("PANIC\t{group}\t{seed}\t{}\t{message}\t{about}", line(on_top)));
    }
    let names = catch_unwind(AssertUnwindSafe(|| {
        let mut seen: Vec<String> = Vec::new();
        for record in &g.log {
            match record.event {
                Event::SpellCast { spell, .. } => seen.push(format!("CAST\t{}", name_of(&g, spell))),
                Event::Resolved { source, .. } => seen.push(format!("RESOLVED\t{}", name_of(&g, source))),
                _ => {}
            }
        }
        seen.sort();
        seen.dedup();
        seen
    }))
    .unwrap_or_default();
    out.extend(names);
    out.push(format!("GAME\t{group}\t{seed}\t{decisions}\t{}\t{}", g.state.turn.number, g.is_over()));
}

fn main() {
    std::panic::set_hook(Box::new(|info| PANIC.with(|p| *p.borrow_mut() = line(info.to_string()))));
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..WORKERS {
            std::thread::Builder::new()
                .stack_size(256 << 20)
                .spawn_scoped(scope, || {
                    let table = table();
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        if i >= GROUPS.len() * GAMES {
                            return;
                        }
                        let mut out = Vec::new();
                        play(table, i % GROUPS.len(), SEED + i as u64, &mut out);
                        let stdout = std::io::stdout();
                        let mut lock = stdout.lock();
                        for row in out {
                            writeln!(lock, "{row}").unwrap();
                        }
                        lock.flush().unwrap();
                    }
                })
                .unwrap();
        }
    });
}
"""


def rust_str(text):
    return json.dumps(text, ensure_ascii=False)


def own_module(name, text):
    """`name`'s module alone: a name that is also an adventure's spell prints that card's module too."""
    card = f"pub static CARD: CardDef = CardDef {{ name: {rust_str(name)},"
    # A token's module opens with the same line, inside `mod token_1 {`.
    blocks = re.split(r"^(?<!\{\n)(?=// Generated by mtgish-ast)", text, flags=re.M)
    return next((block for block in blocks if card in block), text)


def modules(ast, corpus, root, names):
    """Each named card's module, by name, from the generator itself."""
    env = dict(os.environ)
    found = {}
    for start in range(0, len(names), 2000):
        run = subprocess.run([ast, "cantrip-module", corpus, *names[start:start + 2000]], cwd=root, env=env,
                             capture_output=True, text=True)
        parts = re.split(r"^// ===== (.+)$", run.stdout, flags=re.M)
        found.update((name, own_module(name, text)) for name, text in zip(parts[1::2], parts[2::2]))
    return found


def crate(args, picked, found, groups):
    root = Path(args.crate)
    (root / "src").mkdir(parents=True, exist_ok=True)
    checkout = Path(args.checkout).resolve()
    deps = "".join(f'{d}={{path={rust_str(str(checkout / "crates" / d))}}}\n'
                   for d in ("cantrip-api", "cantrip-bench", "cantrip-engine", "cantrip-mechanics", "cantrip-rules", "cantrip-state"))
    (root / "Cargo.toml").write_text(
        '[package]\nname="mtgish-cantrip-smoke"\nversion="0.0.0"\nedition="2024"\n[workspace]\n[dependencies]\n' + deps
        + '[profile.dev]\nopt-level=1\ndebug=0\nincremental=false\ndebug-assertions=true\n')
    code = ["#![allow(unused, clippy::all)]\n"]
    table = []
    for i, name in enumerate(BASICS + picked):
        code.append(f"mod m{i} {{\n{found[name]}\n}}\n")
        table.append(f"(&m{i}::CARD, m{i}::EXTRA)")
    code.append("static MODULES: &[(&cantrip_engine::CardDef, &[&cantrip_engine::CardDef])] = &[" + ", ".join(table) + "];\n")
    code.append(f"const LANDS: &[&str] = &[{', '.join(rust_str(b) for b in BASICS)}];\n")
    code.append(f"const LANDS_OF_EACH: usize = {LANDS_OF_EACH};\nconst COPIES: usize = {args.copies};\n")
    code.append(f"const DECISIONS: u32 = {args.decisions};\nconst GAMES: usize = {args.games};\n")
    code.append(f"const WORKERS: usize = {args.workers};\nconst SEED: u64 = {args.seed};\n")
    rows = ", ".join(f"(&[{', '.join(rust_str(n) for n in a)}], &[{', '.join(rust_str(n) for n in b)}])" for a, b in groups)
    code.append(f"static GROUPS: &[(&[&str], &[&str])] = &[{rows}];\n")
    code.append(MAIN)
    (root / "src/main.rs").write_text("".join(code))


def summary(rows, picked):
    games = [r for r in rows if r[0] == "GAME"]
    cast = {r[1] for r in rows if r[0] == "CAST"} & set(picked)
    resolved = {r[1] for r in rows if r[0] == "RESOLVED"} & set(picked)
    panics = [r for r in rows if r[0] == "PANIC"]
    print(f"{len(games)} games, {sum(g[5] == 'true' for g in games)} finished, "
          f"{sum(int(g[3]) for g in games)} decisions")
    print(f"{len(picked)} cards sampled: {len(cast)} cast, {len(resolved | cast)} cast or resolved; "
          f"{len(picked) - len(resolved | cast)} never ran, so were not tested")
    print(f"{len(panics)} panics")
    groups = collections.defaultdict(list)
    for r in panics:
        place = re.match(r"panicked at ([^:]+:\d+):\d+:", r[4])
        message = r[4].split(": ", 1)[-1] if ": " in r[4] else r[4]
        # A message is often the only stable part of a panic in generated code.
        key = re.sub(r"\d+", "N", place[1] if place else "?") if place and "src/main.rs" not in place[1] else "generated: " + re.sub(r"\d+", "N", message[:70])
        groups[key].append((r[5] if len(r) > 5 and r[5] else r[3], message))
    for key, found in sorted(groups.items(), key=lambda kv: -len(kv[1])):
        cards = collections.Counter(c for c, _ in found)
        print(f"{len(found):5} {key}")
        print(f"      on the stack: {', '.join(f'{c} x{n}' for c, n in cards.most_common(6))}")
        print(f"      {found[0][1][:230]}")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("checkout")
    parser.add_argument("rows")
    parser.add_argument("ast")
    parser.add_argument("corpus")
    parser.add_argument("crate")
    parser.add_argument("--seed", type=int, default=1)
    parser.add_argument("--cards", type=int, default=600)
    parser.add_argument("--per-seat", type=int, default=6)
    parser.add_argument("--copies", type=int, default=4)
    parser.add_argument("--games", type=int, default=2)
    parser.add_argument("--decisions", type=int, default=3000)
    parser.add_argument("--workers", type=int, default=6)
    parser.add_argument("--minutes", type=float, default=8)
    parser.add_argument("--only", nargs="*", default=[])
    args = parser.parse_args()
    names = sorted({json.loads(line)["name"] for line in open(args.rows, encoding="utf-8")} - set(BASICS))
    if args.only:
        names = [n for n in args.only if n in names]
    else:
        random.Random(args.seed).shuffle(names)
    corpus = str(Path(args.corpus).resolve())
    root = str(Path(args.corpus).resolve().parent.parent)
    found = modules(args.ast, corpus, root, BASICS + names[:args.cards])
    picked = [n for n in names[:args.cards] if n in found]
    size = args.per_seat
    tables = [picked[i:i + size * 2] for i in range(0, len(picked), size * 2)]
    groups = [(t[:size], t[size:]) for t in tables if t[size:]]
    crate(args, picked, found, groups)
    env = dict(os.environ, CANTRIP_VERIFY="1", CARGO_INCREMENTAL="0")
    built = subprocess.run(["cargo", "build", "--offline", "--manifest-path", str(Path(args.crate) / "Cargo.toml")],
                           env=env, capture_output=True, text=True)
    if built.returncode:
        print(built.stderr[-3000:])
        sys.exit(built.returncode)
    target = os.environ.get("CARGO_TARGET_DIR") or str(Path(args.crate) / "target")
    out = Path(args.crate) / "smoke.tsv"
    with open(out, "w", encoding="utf-8") as sink:
        try:
            subprocess.run([f"{target}/debug/mtgish-cantrip-smoke"], env=env, stdout=sink, stderr=subprocess.DEVNULL,
                           timeout=args.minutes * 60)
        except subprocess.TimeoutExpired:
            print(f"stopped after {args.minutes} minutes; what follows is what had finished")
    rows = [line.rstrip("\n").split("\t") for line in open(out, encoding="utf-8", errors="replace")]
    summary(rows, picked)


if __name__ == "__main__":
    main()
