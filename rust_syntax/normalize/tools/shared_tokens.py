#!/usr/bin/env python3
"""Cantrip's plain shared tokens, for the PR form: shared_tokens.py CANTRIP writes src/cantrip/shared_tokens.tsv.

A generated card defines each token it makes. Where crates/cantrip-mechanics/src/tokens.rs has that token already, the card
names it instead, as a hand-written card does. A row is `colors|types|power|toughness|keywords|name<TAB>STATIC`, and pr_form.rs
builds the same key from a generated token. Only tokens of plain characteristics are listed: one with abilities or any other
field is the same token only if its code is, which a key cannot say. Rerun when Cantrip's tokens change.
"""
import re
import sys
from pathlib import Path

COLORS = {"White": 1, "Blue": 2, "Black": 4, "Red": 8, "Green": 16}
PLAIN = {"colors", "types", "power", "toughness", "keywords", "text"}


def key(body):
    fields = dict(re.findall(r"^    (\w+): (.*),$", body, re.M))
    rest = re.search(r"^    \.\.token!\((.*)\)$", body, re.M)
    # Every field must be one we can compare, and each on one line: anything else is a token with more to it.
    lines = [line for line in body.splitlines() if line.strip()]
    if not rest or set(fields) - PLAIN or len(lines) != len(fields) + 1:
        return None
    colors = sum(COLORS[c] for c in re.findall(r"Color::(\w+)", fields.get("colors", "")))
    types = ",".join(sorted(re.findall(r"CardType::(\w+)", fields.get("types", ""))))
    keywords = ",".join(sorted(re.findall(r"Keyword::(\w+)", fields.get("keywords", ""))))
    number = lambda field: re.sub(r"\s", "", fields.get(field, ""))
    return f"{colors}|{types}|{number('power')}|{number('toughness')}|{keywords}|{rest[1]}"


def main():
    source = (Path(sys.argv[1]) / "crates/cantrip-mechanics/src/tokens.rs").read_text(encoding="utf-8")
    rows = {}
    for name, body in re.findall(r"^pub static (\w+): CardDef = CardDef \{\n(.*?)^\};", source, re.S | re.M):
        row = key(body)
        if row:
            rows.setdefault(row, name)  # two statics of one token: the first is as good as the other
    out = Path(__file__).resolve().parent.parent / "src/cantrip/shared_tokens.tsv"
    out.write_text("".join(f"{k}\t{v}\n" for k, v in sorted(rows.items())), encoding="utf-8", newline="\n")
    print(f"{len(rows)} plain shared tokens")


if __name__ == "__main__":
    main()
