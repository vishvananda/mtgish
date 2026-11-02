"""CANTRIP_TRACKER.md from the tracker's JSON: what a Cantrip developer reads first is where things stand and what to
build next, by the cards each thing lands; the full tables come after, folded.

`cantrip_tree.py tracker` calls `render`; nothing here reads the rows or the trees.
"""

import re

BRANCH = "https://github.com/vishvananda/cantrip-rs/blob/card-lane/generated/crates/cantrip-generated/cards"
PR = re.compile(r"cantrip-rs/cantrip-rs#(\d+)")
PR_URL = "https://github.com/cantrip-rs/cantrip-rs/pull/{}"

MEANING = {
    "generator": "Cantrip can do it today; mtgish's generator has only to write it",
    "mechanics module": "a keyword's module in `cantrip-mechanics`, no change to the core",
    "unsure": "the audit could not settle whether Cantrip can do it",
    "unaudited": "a form that appeared after the last audit",
    "mtgish data": "mtgish encodes the card wrongly; fixed in mtgish, not Cantrip",
    "Cantrip": "a change to Cantrip's engine or API",
    "not played": "a game mode or digital-only rule Cantrip does not play",
}

# What a Cantrip developer would have to decide for each kind of proposal the check rejects.
DECIDE = [
    (r"isn't in scope", "The hole reads a value that lives in another function or in a static table (a resolution's "
                        "result, `item`, an announced X). Decide where Cantrip keeps it: a `fn` in a card table that "
                        "is asked for it, or data the game state holds."),
    (r"^E0308", "The proposed value has the wrong type for its place, mostly a closure in a list that holds `fn` "
                "pointers or data."),
    (r"without what it derives", "A variant of stored data (`Copy + Eq + Hash`) is proposed with a list or a closure "
                                 "in it. It needs a `Copy` shape: an index into a card table, or one entry for each "
                                 "object."),
    (r"not Copy \+ Sync \+ 'static", "A `CardDef` table entry must be constant data; the proposed payload is not."),
    (r"dyn but not data", "A proposed method on a `dyn` trait takes a closure; Cantrip's traits take `fn` pointers "
                          "or data."),
    (r"^E0373|^E0521", "A closure captures a value of the resolution. Carry the value as data instead."),
    (r"^E0599", "A method is called on what Cantrip returns as a `bool` or a builder."),
]


def link(name, stems):
    stem = stems.get(name)
    return f"[{name}]({BRANCH}/{stem}.rs)" if stem else name


def cards_of(names, stems, most=3):
    return ", ".join(link(n, stems) for n in names[:most])


def status(needs):
    found = PR.search(needs)
    return f"[PR #{found.group(1)}]({PR_URL.format(found.group(1))})" if found else ""


def table(head, rows):
    return ["| " + " | ".join(head) + " |", "|" + "---|" * len(head)] + ["| " + " | ".join(str(c) for c in r) + " |" for r in rows]


def folded(summary, body):
    return ["<details>", f"<summary>{summary}</summary>", ""] + body + ["", "</details>"]


def unlocks_by_feature(tracked):
    """For each feature (a `needs` text), the remaining cards it alone stands between and their file: cards whose every
    requirement is a form of that feature."""
    needs_of = {row["id"]: row["needs"] for row in tracked["requirements"]}
    alone = {}
    for name, ids in tracked["cards"].items():
        wanted = {needs_of[i] for i in ids if i in needs_of}
        if len(wanted) == 1:
            alone.setdefault(wanted.pop(), []).append(name)
    return {k: sorted(v) for k, v in alone.items()}


# Needs texts that name a family rather than a feature: their forms are listed one by one, each with what the audit
# found it needs.
GENERIC = {"support in Cantrip's core", "a generator arm, or a Cantrip feature if none exists"}


def first_sentence(text, most=260):
    text = " ".join(text.split())
    cut = re.search(r"(?<=[.:;])\s", text[40:most])
    return text[: 40 + cut.start()] if cut else (text if len(text) <= most else text[:most].rsplit(" ", 1)[0] + " ...")


def items_for(fix, features, alone, requirements):
    """(lands, touches, written, needs, forms, examples) for each thing to build under `fix`, a family listed form by form."""
    items = []
    for f in features:
        if f["fix"] != fix:
            continue
        if f["needs"] in GENERIC:
            for form in f["forms"]:
                r = requirements[form]
                if r["cards"]:
                    how = r.get("how") or ""
                    items.append((r["alone"], r["cards"], r["written"], first_sentence(how) if how and how != r["needs"] else r["needs"],
                                  [form], r["examples"]))
            continue
        if f["cards"]:
            items.append((len(alone.get(f["needs"], [])), f["cards"], f["written"], f["needs"], f["forms"],
                          alone.get(f["needs"]) or f["examples"]))
    if fix == "unaudited":
        # A feature takes the hardest fix of its forms, so a form the audit hasn't seen is listed by itself.
        items = [(r["alone"], r["cards"], r["written"], r["needs"], [r["id"]], r["examples"])
                 for r in requirements.values() if r["fix"] == fix and r["cards"]]
    return sorted(items, key=lambda i: (-i[0], -i[1], i[3]))


def priorities(features, alone, fix, stems, most, requirements):
    items = items_for(fix, features, alone, requirements)
    rows = [[lands, touches, wrote, status(needs) or " ", needs.replace("|", "\\|"),
             " ".join(f"`{form}`" for form in forms[:3]) + (f" +{len(forms) - 3}" if len(forms) > 3 else ""),
             cards_of(examples, stems)] for lands, touches, wrote, needs, forms, examples in items[:most]]
    return items, table(["lands", "touches", "written", "status", "needs", "forms", "examples"], rows)


def render(tracked, rewrites, unread, counter_name):
    t = tracked["totals"]
    blocked, written = tracked["cards"], tracked["written"]
    stems = tracked.get("stems", {})
    features = tracked["features"]
    alone = unlocks_by_feature(tracked)
    requirements = {r["id"]: r for r in tracked["requirements"]}
    by_fix = tracked["fixes"]
    base = tracked["cantrip"]
    lines = ["# Cantrip tracker", "",
             "mtgish writes a Cantrip card file for every Magic card it parses. This page says how many of those files "
             "are complete, what the rest wait on, and which piece of Cantrip work lands the most cards. It is generated "
             "by `rust_syntax/normalize/tools/cantrip_tree.py tracker` (the layout by `tools/tracker_md.py`); do not edit "
             "it by hand. The data is in `data/cantrip_tracker.json`.", "",
             f"Every file is on the branch [`card-lane/generated`]({BRANCH}) of vishvananda/cantrip-rs, in the crate "
             f"`cantrip-generated`, written against Cantrip {base}. The whole crate type-checks there.", "",
             "Contents: [Where things stand](#where-things-stand) · [How to read a card's file](#how-to-read-a-cards-file) · "
             "[What to build in Cantrip](#what-to-build-in-cantrip) · [Mechanics modules](#mechanics-modules) · "
             "[Proposed APIs that do not fit](#proposed-apis-that-do-not-fit) · [mtgish's own work](#mtgishs-own-work) · "
             "[Not played](#not-played) · [Reference](#reference)", "",
             "## Where things stand", ""]
    remaining = len(blocked)
    lines += table(["", "cards", "what it means"], [
        ["Magic cards", t["cards"], "every record mtgish parses, all faces of a card counted once"],
        ["**whole**", f"**{t['ready']}**", "a complete file with no hole: compiles, and needs nothing Cantrip lacks"
         + (f" ({t['ready_written']} of them Cantrip already has written by hand)" if "ready_written" in t else "")],
        ["with holes", t["planned"], "a complete file, but some lines wait on a named requirement"],
        ["... written by hand in Cantrip", t["written"], "Cantrip already has the card; only mtgish's generator is behind"],
        ["... **waiting**", f"**{remaining}**", "what the rest of this page is about"],
    ])
    lines += ["", "What each waiting card needs, counted once by the hardest thing it needs:", ""]
    lines += table(["fix", "cards", "meaning"], [[k, by_fix.get(k, 0), MEANING[k]] for k in MEANING if by_fix.get(k, 0)])
    proposals = tracked.get("proposals")
    if proposals:
        lines += ["", f"The code inside the holes is mtgish's proposal for the API it needs. `tools/proposal_check.py` "
                  f"compiles it against stubs of that API under Cantrip's rules for stored data (`Copy`, `Eq`, `Hash`, "
                  f"no closures): of {proposals['cards']} cards with holes, {proposals['compile']} compile, "
                  f"{proposals['compile_but_for_stubs']} fail only where Rust can't stub the API, and "
                  f"**{proposals['fail']} propose a shape Cantrip couldn't take** ([listed below](#proposed-apis-that-do-not-fit))."]
    lines += ["", "## How to read a card's file", "",
              "A whole file is the card as a Cantrip PR would add it. A file with holes is the same file with each part "
              "Cantrip can't express yet wrapped in a macro (defined in `crates/cantrip-generated/src/lib.rs`):", "",
              "- `planned!(\"id\" => code)`: statements the card would run.",
              "- `planned_value!(\"id\", placeholder => code)`: a value; the placeholder compiles today, the code is what it will be.",
              "- `planned_item!(\"id\" => item)`: a whole item, such as a table or a `fn`.", "",
              "The `id` is a row of [Every requirement](#every-requirement). The code inside is written as it will be once "
              "Cantrip has the feature, so landing a feature means removing the macros, not rewriting the card. When the "
              "pinned Cantrip has a feature (`src/cantrip_vocab.rs` `FEATURES`), the generator writes those cards bare.", "",
              "In the tables below, **lands** is the number of waiting cards this alone stands between and a whole file; "
              "**touches** is every waiting card that needs it among other things; **written** counts cards Cantrip has "
              "already written by hand that need it, which shows a hand-written way exists. Rows are sorted by **lands**.", ""]
    engine, rows = priorities(features, alone, "Cantrip", stems, 60, requirements)
    lines += ["## What to build in Cantrip", "",
              f"{len(engine)} pieces of Cantrip's engine or API, {by_fix.get('Cantrip', 0)} cards. The top 60:", ""] + rows
    modules, rows = priorities(features, alone, "mechanics module", stems, 200, requirements)
    lines += ["", "## Mechanics modules", "",
              f"{len(modules)} keywords and mechanics that need a module in `cantrip-mechanics` and no change to the core, "
              f"{by_fix.get('mechanics module', 0)} cards. A good first contribution.", ""] + rows
    lines += ["", "## Proposed APIs that do not fit", ""]
    if proposals:
        names = {stems[n]: n for n in stems}
        lines += [f"{proposals['fail']} cards hold hole code Cantrip couldn't take in that shape. Each needs a decision "
                  "in Cantrip's API before mtgish can propose it well. By what is wrong:", ""]
        for cls in proposals["classes"]:
            what = next((text for pattern, text in DECIDE if re.search(pattern, cls["class"])), "")
            members = [names.get(m["card"], m["card"]) for m in cls["members"]]
            lines += [f"**{cls['class']}** ({cls['cards']} cards). {what}", ""]
            lines += [f"- {link(n, stems)}: line {m['line']}, {m['message']}" for n, m in zip(members, cls["members"])]
            lines += [""]
    else:
        lines += ["The proposal check was not given to this run: `cantrip_tree.py tracker --proposals REPORT.json`.", ""]
    lines += ["## mtgish's own work", ""]
    for fix in ("generator", "mtgish data", "unsure", "unaudited"):
        chosen, rows = priorities(features, alone, fix, stems, 100, requirements)
        if chosen:
            lines += [f"### {fix}: {by_fix.get(fix, 0)} cards", "", MEANING[fix] + ".", ""] + rows + [""]
    played_not = [f for f in features if f["fix"] == "not played"]
    lines += ["## Not played", "",
              f"{by_fix.get('not played', 0)} cards need a game mode or a digital-only rule Cantrip does not play "
              "(Planechase, Vanguard, Archenemy, Alchemy's perpetual effects and conjuring, the commander designation, "
              "stickers). Nothing to do unless Cantrip takes those on.", ""]
    lines += folded(f"{len(played_not)} features", table(["touches", "needs", "examples"],
                    [[f["cards"], f["needs"].replace("|", "\\|"), cards_of(f["examples"], stems)] for f in played_not]))
    lines += ["", "## Reference", ""]
    shared = [f for f in features if f["cards"] > 1 or len(f["forms"]) > 1]
    single = [f for f in features if f["cards"] == 1 and len(f["forms"]) == 1]
    lines += ["### Every feature", ""] + folded(
        f"{len(shared)} features more than one card or form shares, most cards first",
        table(["cards", "written", "forms", "fix", "needs", "examples"],
              [[f["cards"], f["written"], len(f["forms"]), f["fix"], f["needs"].replace("|", "\\|"), ", ".join(f["examples"])] for f in shared]))
    lines += ["", "### Asked by one card", ""] + folded(
        f"{len(single)} forms one waiting card has and nothing shares; most are likely code in that card's own file",
        table(["card", "written", "fix", "needs"],
              [[link(f["examples"][0], stems) if f["examples"] else "", f["written"], f["fix"], f["needs"].replace("|", "\\|")] for f in single]))
    counters = tracked["counters"]
    lines += ["", "### Counters a card brings with it", "",
              f"{len(counters)} counter kinds are a name and nothing more (CR 122.1), used by {t['cards_with_counter_lines']} cards. "
              "They are not gaps: the card's file uses the kind, and a commit just before the card adds its line to "
              "`counter_kinds!`. The generated branch has them all in one commit.", ""]
    lines += folded(f"{len(counters)} counters", table(["counter", "cards", "examples"],
                    [[counter_name(w), len(n), ", ".join(sorted(n)[:3])] for w, n in sorted(counters.items(), key=lambda kv: (-len(kv[1]), kv[0]))]))
    lines += ["", f"{t['new_tokens']} tokens are made by several cards and Cantrip has no static for them; "
              f"{t['cards_with_new_tokens']} cards name them. Each is committed to `tokens.rs` before the first card that "
              "needs it, as a counter is (`tools/shared_tokens.py`)."]
    lines += ["", "### Text the source rewrites", "",
              f"mtgish changes {len(rewrites) + len(unread)} passages of card text on purpose before parsing "
              "(`grammars/find_replace.json5`). Cantrip plays Oracle text, so each has a verdict in "
              f"`data/cantrip_rewrites.json`; {len(unread)} have none yet. Those that change nothing or sit on cards not "
              "played are left out.", ""]
    lines += [f"- not read yet: `{find}`" for find in unread]
    lines += folded("rewrites and verdicts", table(["cards", "verdict", "note"],
                    [[", ".join(r["cards"]), r["verdict"], r.get("note", "")] for r in rewrites if r["verdict"] not in ("same", "not played")]))
    reqs = tracked["requirements"]
    lines += ["", "### Every requirement", "",
              "A requirement is one form the generator could not write, not one piece of work: one feature can be many "
              "forms. `class` comes from the form's family; `fix` from `data/cantrip_audit.json`, where each form's verdict "
              "cites the API or a hand-written card. **alone** counts the cards this form alone stands between and a whole file.", ""]
    lines += folded(f"{sum(1 for r in reqs if r['cards'])} requirements on waiting cards", table(
        ["requirement", "class", "fix", "cards", "written", "alone", "needs", "examples"],
        [[f"`{r['id']}`", r["class"], r["fix"], r["cards"], r["written"], r["alone"], r["needs"].replace("|", "\\|"), ", ".join(r["examples"][:3])]
         for r in reqs if r["cards"]]))
    done = [r for r in reqs if not r["cards"]]
    lines += ["", "### Only on cards Cantrip has written", "",
              f"Cantrip has every card on these {len(done)} rows, so none is a missing feature; each is a gap in mtgish's generator.", ""]
    lines += folded(f"{len(done)} requirements", table(["requirement", "class", "written"], [[f"`{r['id']}`", r["class"], r["written"]] for r in done]))
    return "\n".join(lines) + "\n"
