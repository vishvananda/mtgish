"""Candidate card pull requests for Cantrip: content JSON, card file and scenarios.

    cantrip_candidates.py <cantrip checkout> <rows.jsonl> <oracle.json> <out dir> [--limit N] [--only NAME ...]

`rows.jsonl` is `ast cantrip-files`. Reads what the checkout has, so a card already
in it is left out, and writes under <out dir> the tree a card's commit adds:

    content/mtg/cards/<slug>.json
    crates/cantrip-cards/src/cards/<stem>.rs   (none for a card of core keywords)
    scenarios/<what-it-proves>.cantrip

plus `candidates.json` with what became of every card. Only a card with both a
file from the AST and scenarios from its words is a candidate: neither side alone
says anything about the other.
"""

import argparse
import json
import os
import re
import sys
import unicodedata

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import cantrip_scenarios as cs  # noqa: E402


def file_stem(name):
    """new-card.py's rule: accents folded, other non-alphanumerics to `_`."""
    folded = unicodedata.normalize("NFKD", name)
    folded = "".join(c for c in folded if not unicodedata.combining(c))
    return re.sub(r"[^a-z0-9]+", "_", folded.replace("'", "").lower()).strip("_")


def content_slug(name):
    plain = unicodedata.normalize("NFKD", name).encode("ascii", "ignore").decode().lower().replace("'", "")
    return re.sub(r"-+", "-", re.sub(r"[^a-z0-9]+", "-", plain)).strip("-")


def number(value):
    return int(value) if re.fullmatch(r"-?\d+", str(value or "")) else value


def content_of(face, core_keywords):
    """A content file's characteristics, as new-card.py writes them."""
    card = {"name": face["name"]}
    if face.get("manaCost"):
        card["manaCost"] = face["manaCost"]
    card["type"] = face["type"]
    for key in ("power", "toughness", "loyalty", "defense"):
        if face.get(key) not in (None, ""):
            card[key] = number(face[key])
    card["oracleText"] = face.get("text", "")
    keywords = []
    for line in face.get("text", "").split("\n"):
        line = re.sub(r"\s*\([^)]*\)", "", line).strip()
        words = [w.strip().lower() for w in line.split(",")]
        if line and all(w in cs.KEYWORDS for w in words):
            keywords += [cs.KEYWORDS[w] for w in words]
    keywords = [k for k in dict.fromkeys(keywords) if k in core_keywords]
    if keywords:
        card["keywords"] = keywords
    return card


def core_keywords(checkout):
    source = open(os.path.join(checkout, "crates/cantrip-oracle/build.rs"), encoding="utf-8").read()
    return set(re.findall(r'"([^"]+)"', re.search(r"const KEYWORDS[^=]*=\s*\[(.*?)\];", source, re.S).group(1)))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("checkout")
    ap.add_argument("rows")
    ap.add_argument("oracle")
    ap.add_argument("out")
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--only", nargs="*", default=[])
    args = ap.parse_args()

    content = os.path.join(args.checkout, "content/mtg/cards")
    have = {json.load(open(os.path.join(content, f), encoding="utf-8-sig"))["name"] for f in os.listdir(content)}
    pieces = cs.Pieces(content)
    core = core_keywords(args.checkout)
    faces = {}
    for card in json.load(open(args.oracle, encoding="utf-8")):
        for face in card["cards"]:
            faces.setdefault(face["name"], (card, face))
    used_slugs = set(os.listdir(os.path.join(args.checkout, "scenarios")))
    outcome = {"candidates": [], "no_recipe": {}, "unread": 0, "unwritable": {}, "already_in_cantrip": 0}
    written = 0
    for line in open(args.rows, encoding="utf-8"):
        row = json.loads(line)
        name = row["name"]
        if args.only and name not in args.only:
            continue
        if name in have:
            outcome["already_in_cantrip"] += 1
            continue
        if row["gap"]:
            outcome["no_recipe"][row["gap"]] = outcome["no_recipe"].get(row["gap"], 0) + 1
            continue
        card_entry, face = faces[name]
        if card_entry.get("layout") != "single" or "." in name or "(" in name or "/" in name or ";" in name:
            outcome["unread"] += 1
            continue
        card = content_of(face, core)
        try:
            scripts = cs.scenarios(card, pieces)
        except cs.Unwritable as why:
            outcome["unwritable"][str(why)] = outcome["unwritable"].get(str(why), 0) + 1
            continue
        if not scripts:
            outcome["unread"] += 1
            continue
        if any(s.slug + ".cantrip" in used_slugs for s in scripts):
            outcome["unread"] += 1
            continue
        for path, text in [
            (f"content/mtg/cards/{content_slug(name)}.json", json.dumps(card, indent=2, ensure_ascii=False) + "\n"),
            *([(f"crates/cantrip-cards/src/cards/{file_stem(name)}.rs", row["rust"])] if row["rust"] else []),
            *[(f"scenarios/{s.slug}.cantrip", s.text()) for s in scripts],
        ]:
            full = os.path.join(args.out, path)
            os.makedirs(os.path.dirname(full), exist_ok=True)
            with open(full, "w", encoding="utf-8", newline="\n") as f:
                f.write(text)
        outcome["candidates"].append({"name": name, "scenarios": [s.slug for s in scripts], "file": bool(row["rust"])})
        written += 1
        if args.limit and written >= args.limit:
            break
    with open(os.path.join(args.out, "candidates.json"), "w", encoding="utf-8") as f:
        json.dump(outcome, f, indent=1)
    print(f"{written} candidates; {outcome['already_in_cantrip']} already in the checkout; {outcome['unread']} texts not read whole;"
          f" {sum(outcome['no_recipe'].values())} without a recipe; {sum(outcome['unwritable'].values())} with no board")
    print(outcome["unwritable"])


if __name__ == "__main__":
    main()
