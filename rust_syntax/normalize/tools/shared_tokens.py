#!/usr/bin/env python3
"""The tokens several cards make, for the PR form: shared_tokens.py CANTRIP ROWS.

Cantrip defines a token once, in crates/cantrip-mechanics/src/tokens.rs, if another printed card makes it exactly: its review
says so, and a worker cannot move one. ROWS are `ast cantrip-tracked` run with MTGISH_LOCAL_TOKENS set, so every card defines
each token it makes. A token Cantrip has is named by the cards that make it. A token two cards make and Cantrip does not have
is added by a commit before the first of them, as a counter is: its source goes in data/cantrip_new_tokens.json.

Writes src/cantrip/shared_tokens.tsv, `card<TAB>the token's static in the card<TAB>its static in Cantrip`, and that file.
Rerun when Cantrip's tokens change, or when the generator writes a token differently.
"""
import collections
import itertools
import json
import re
import sys
import textwrap
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
TOKENS = "crates/cantrip-mechanics/src/tokens.rs"
BITS = {"White": 1, "Blue": 2, "Black": 4, "Red": 8, "Green": 16}
COLOR_WORD = {1: "white", 2: "blue", 4: "black", 8: "red", 16: "green"}
PAIRS = {3: "white and blue", 6: "blue and black", 12: "black and red", 24: "red and green", 17: "green and white",
         5: "white and black", 10: "blue and red", 20: "black and green", 9: "red and white", 18: "green and blue"}
STRING = re.compile(r'"(?:[^"\\]|\\.)*"')
ITEM = re.compile(r"^(?:pub(?:\([a-z]+\))? )?(?:static|const|fn) (\w+)")


def outside_strings(text, change):
    """`change` over the code of `text`, its strings left alone."""
    out, at = [], 0
    for string in STRING.finditer(text):
        out += [change(text[at:string.start()]), string[0]]
        at = string.end()
    return "".join(out + [change(text[at:])])


def items_of(source):
    """{name: its text, doc comment included} of a file's statics, consts and functions."""
    items, lines, i, doc = {}, source.split("\n"), 0, []
    while i < len(lines):
        line = lines[i]
        if not line.strip():
            doc = []
        elif line.startswith(("///", "#[")):
            doc.append(line)
        elif not line.startswith((" ", "//")):
            start = i
            while not (lines[i].rstrip().endswith((";", "}")) and (i == start or lines[i][0] in "})]")):
                i += 1
            name = ITEM.match(line)
            if name:
                items[name[1]] = "\n".join(doc + lines[start:i + 1])
            doc = []
        i += 1
    return items


def mechanics_imports(source):
    """{a name in the file: its path in cantrip_mechanics}."""
    names = {}
    for clause in re.findall(r"^use cantrip_mechanics::(.*?);", source, re.M | re.S):
        braces = re.fullmatch(r"(.*?)\{(.*)\}", clause.strip(), re.S)
        for leaf in (f"{braces[1]}{part.strip()}" for part in braces[2].split(",") if part.strip()) if braces else [clause.strip()]:
            names[leaf.rsplit("::", 1)[-1]] = leaf
    return names


def words_of(code):
    return set(re.findall(r"\b[A-Za-z_]\w*\b", STRING.sub('""', code)))


def bare(text):
    """Without what does not change what a token is: comments, spaces, trailing commas."""
    return re.sub(r"[\s,]", "", "\n".join(line for line in text.split("\n") if not line.lstrip().startswith("///")))


def characteristics(body, reached=""):
    """What Cantrip's definition and a generated one can be compared by, `reached` being the code the token hangs on. None for
    a token that is not one of `token!`."""
    body = "\n".join(line for line in body.split("\n") if not line.lstrip().startswith("///"))
    name = re.search(r"\.\.token!\((.*?)\)", body)
    if not name:
        return None
    fields = {m[1]: m[2] for m in re.finditer(r"^    (\w+): (.*?)(?=^    \S)", body + "    }", re.M | re.S)}
    colors = fields.get("colors", "")
    number = re.search(r"ColorSet\((\d+)\)", colors)
    number_of = lambda word: re.sub(r"[\s,]", "", fields.get(word, ""))
    wording = "\n".join(line for line in re.sub(r"^    text: .*$", "", body, flags=re.M).split("\n"))
    return {"name": name[1], "bits": int(number[1]) if number else sum(BITS[c] for c in re.findall(r"Color::(\w+)", colors)),
            "types": sorted(re.findall(r"CardType::(\w+)", fields.get("types", ""))), "power": number_of("power"), "toughness": number_of("toughness"),
            "keywords": sorted(set(re.findall(r"Keyword::(\w+)", wording + reached))), "rules": sorted({t for t in STRING.findall(wording) if re.search("[a-z]{3}", t)}),
            "named": sorted(set(re.findall(r"triggers::([A-Z_]+)", wording))), "other": sorted(set(fields) - {"colors", "types", "power", "toughness", "keywords", "abilities", "statics", "text"}),
            "text": fields.get("text", "").strip().rstrip(",")}


def key_of(c, rules=None):
    return json.dumps({"rules": c["rules"] if rules is None else rules, **{k: c[k] for k in ("name", "bits", "types", "power", "toughness", "keywords", "named", "other")}}, sort_keys=True)


def cantrip_statics(cantrip):
    """{a token's key: its static}, and every name tokens.rs has. A definition can put its rules in `text` and in its code,
    or only in `text`, with the keyword that does the work: both keys find it."""
    source = (Path(cantrip) / TOKENS).read_text(encoding="utf-8")
    found, names = {}, set(re.findall(r"^pub (?:static|const|fn) (\w+)", source, re.M))
    for name, body in re.findall(r"^pub static (\w+): CardDef = CardDef \{\n(.*?)^\};", source, re.S | re.M):
        c = characteristics(body)
        if c:
            found.setdefault(key_of(c), name)
            if c["text"] and not c["rules"]:
                found.setdefault(key_of(c, [c["text"]]), name)
    return found, names


def color_words(bits):
    if bits in PAIRS:
        return PAIRS[bits]
    words = [word for bit, word in COLOR_WORD.items() if bits & bit]
    return ", ".join(words[:-1]) + f", and {words[-1]}" if len(words) > 1 else (words[0] if words else "colorless")


def described(c):
    """The token's words, as Cantrip's doc comments have them: the commit's, and the first line of the comment."""
    size = f"{c['power'].removeprefix('Some(').removesuffix(')')}/{c['toughness'].removeprefix('Some(').removesuffix(')')} " if c["power"] else ""
    keywords = [re.sub(r"(?<=[a-z])(?=[A-Z])", " ", k).lower() for k in c["keywords"] + c["named"]]
    having = " with " + " and ".join(keywords) if keywords else ""
    return size, having


def static_name(c, taken):
    base = re.sub(r"\W+", "_", c["name"]).upper()
    color = "COLORLESS" if not c["bits"] else color_words(c["bits"]).replace(",", "").replace(" and ", " ").replace(" ", "_").upper()
    size = "_".join(re.sub(r"\D", "", c[f]) or "0" for f in ("power", "toughness") if c[f])
    sized = "_".join(x for x in (base, size, color) if x)
    full = "_".join(x for x in (sized, *[k.upper() for k in c["keywords"] + c["named"]]) if x)
    for choice in itertools.chain((base, sized, full), (f"{full}_{n}" for n in itertools.count(2))):
        if choice not in taken:
            taken.add(choice)
            return choice


def wrapped(line):
    """A doc comment within Cantrip's 120 columns."""
    if not line.startswith("/// ") or len(line) <= 120:
        return line
    return "\n".join("/// " + part for part in textwrap.wrap(line[4:], 116, break_long_words=False, break_on_hyphens=False))


def definition(card_source, token, static, deps, cards, c):
    """The token's source as tokens.rs will have it: its functions named for it, and paths from inside the crate."""
    items, imports = items_of(card_source), mechanics_imports(card_source)
    renamed = {dep: f"{static.lower()}_{dep}" if dep[0].islower() else f"{static}_{dep}" for dep in deps}

    def in_crate(code):
        code = code.replace("cantrip_mechanics::", "crate::")
        for name, path in imports.items():
            code = re.sub(rf"(?<![\w:.]){name}\b(?!\s*:[^:])", f"crate::{path}", code)
        for old, new in renamed.items():
            code = re.sub(rf"(?<![\w:.]){old}\b", new, code)
        return code

    size, having = described(c)
    types = " ".join(t.lower() for t in c["types"])
    rules = [r for r in c["rules"] if r != '""']
    quoted = (' with ' + " and ".join(r for r in rules)) if rules else ""
    who = sorted(cards)[:3]
    who = (", ".join(who[:-1]) + " and " + who[-1]) if len(who) > 1 else who[0]
    doc = f"A {size}{color_words(c['bits'])} {c['name']} {types} token{having}{quoted} (CR 111). {who} create this exact token."
    comment = "\n".join("/// " + line for line in textwrap.wrap(doc, 116, break_long_words=False, break_on_hyphens=False))
    body = re.sub(r"^(?:///.*\n)*(?:#\[.*\n)*(?:pub )?static \w+:", f"pub static {static}:", items[token])
    parts = [comment + "\n" + outside_strings(body, in_crate)]
    parts += [outside_strings(items[dep].replace("pub fn ", "fn "), in_crate) for dep in deps]
    return "\n".join(wrapped(line) for line in "\n\n".join(parts).split("\n")) + "\n"


def main():
    cantrip, rows = sys.argv[1:3]
    known, names = cantrip_statics(cantrip)
    made = collections.defaultdict(dict)  # the exact token: {card: (static in the card, what it hangs on, source)}
    about, fixed, stuck = {}, [], collections.Counter()
    for line in open(rows, encoding="utf-8"):
        row = json.loads(line)
        source = row.get("file")
        if not source or "..token!(" not in source:
            continue
        items = items_of(source)
        for name, text in items.items():
            if not text.count("CardDef = CardDef") or "..token!(" not in text:
                continue
            deps, queue = [], [w for w in words_of(text) if w in items and w != name]
            while queue:
                dep = queue.pop()
                if dep not in deps:
                    deps.append(dep)
                    queue += [w for w in words_of(items[dep]) if w in items and w != dep]
            deps.sort()
            c = characteristics(text, "\n".join(items[d] for d in deps))
            if key_of(c) in known:
                fixed.append((row["name"], name, known[key_of(c)]))
                continue
            texts = [text] + [items[d] for d in deps]
            if any("oracle::" in t for t in texts) or any("..token!(" in items[d] or d == "CARD" for d in deps):
                stuck[c["name"]] += 1
                continue
            exact = "\n".join(texts)
            for number, dep in enumerate(deps):
                exact = re.sub(rf"\b{dep}\b", f"D{number}", exact)
            exact = bare(exact.replace(f"static {name}:", "static TOKEN:"))
            made[exact][row["name"]] = (name, deps, source)
            about[exact] = c
    new, taken = {}, set(names)
    shared = sorted(((k, v) for k, v in made.items() if len(v) > 1), key=lambda kv: (-len(kv[1]), kv[0]))
    for exact, cards in shared:
        c = about[exact]
        static = static_name(c, taken)
        first = min(cards)
        name, deps, source = cards[first]
        size, having = described(c)
        new[static] = {"title": f"the {size}{'' if not c['bits'] else color_words(c['bits']) + ' '}{c['name']} token{having}", "cards": len(cards),
                       "source": definition(source, name, static, deps, list(cards), c)}
        fixed += [(card, local[0], static) for card, local in cards.items()]
    out = ROOT / "rust_syntax/normalize/src/cantrip/shared_tokens.tsv"
    out.write_text("".join(f"{a}\t{b}\t{c}\n" for a, b, c in sorted(fixed)), encoding="utf-8", newline="\n")
    (ROOT / "data/cantrip_new_tokens.json").write_text(json.dumps(new, ensure_ascii=False, indent=0, sort_keys=True) + "\n", encoding="utf-8", newline="\n")
    print(f"{len(fixed)} card tokens shared; {len(new)} new statics for {sum(n['cards'] for n in new.values())} cards; "
          f"{sum(1 for k, v in made.items() if len(v) == 1)} tokens of one card stay in it; not movable {sum(stuck.values())} {stuck.most_common(5)}")


if __name__ == "__main__":
    main()
