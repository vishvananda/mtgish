#!/usr/bin/env python3
"""Catalog positional wording and typed IR, using only checked-in local data.

Run examples/export_unified_jsonl first; pass its output with --ir.
The normal export's ignore list is applied to scope classification. Excluded
products remain visible with their documented reason. Missing in-scope entries
are reported separately. A wording match may describe a destination.
"""
import argparse
import collections
import hashlib
import json
import re
from pathlib import Path

PATTERNS = [
    re.compile(r"\b(?P<end>top|bottom)\s+(?P<qualifier>(?:[^\s.;,:]+\s+){0,7})cards?\s+(?:of|from)\s+(?P<owner>[^.;:\n]{1,90}?)\s+(?P<zone>library|graveyard)\b", re.I),
    re.compile(r"\b(?:cards?\s+from\s+the\s+)?(?P<end>top|bottom)\s+of\s+(?P<owner>[^.;:\n]{1,90}?)\s+(?P<zone>library|graveyard)\b", re.I),
]
ACTIONS = {"LookAt", "Reveal", "RevealUntil", "Move", "Mill", "Draw", "Manifest", "Scry", "Surveil", "PlayCards", "ReorderLibrary", "ChooseObjects"}

def walk(value, path=""):
    if isinstance(value, dict):
        yield path, value
        for key, child in value.items():
            yield from walk(child, path + "/" + key)
    elif isinstance(value, list):
        for index, child in enumerate(value):
            yield from walk(child, path + "/" + str(index))

def mentions(text):
    result = []
    for pattern in PATTERNS:
        for match in pattern.finditer(text):
            start = max(text.rfind(".", 0, match.start()), text.rfind("\n", 0, match.start())) + 1
            end = re.search(r"[.\n]", text[match.end():])
            end = match.end() + end.start() if end else len(text)
            clause = text[start:end].strip()
            result.append({"text": match.group(), "offset": match.start(), "end": match["end"].lower(), "zone": match["zone"].lower(), "owner": match["owner"], "qualifier": match.groupdict().get("qualifier", "").strip(), "actions": re.findall(r"\b(?:look at|reveal|exile|manifest dread|manifest|cloak|mill|draw|scry|surveil|put|return|cast|play)\b", clause, re.I), "clause": clause})
    return sorted(result, key=lambda m: (m["offset"], m["text"]))

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[2])
    args = parser.parse_args()
    ignored = {}
    reason = "unspecified"
    for number, line in enumerate((args.repo / "grammars/ignore.json5").read_text().splitlines(), 1):
        comment = re.match(r"\s*//\s*(.*)", line)
        if comment and comment[1].strip() and not set(comment[1].strip()) <= {"-"}:
            reason = comment[1].strip()
        name = re.match(r'\s*("(?:[^"\\]|\\.)*")\s*,?\s*(?://.*)?$', line)
        if name:
            ignored[json.loads(name[1])] = {"category": reason, "line": number}
    by_name = collections.defaultdict(list)
    for index, line in enumerate(args.ir.read_text().splitlines()):
        card = json.loads(line)
        for face in card["faces"]:
            nodes = list(walk(face["abilities"]))
            actions = collections.Counter(v.get("kind") for _, v in nodes if isinstance(v.get("kind"), str) and v["kind"] in ACTIONS)
            selectors = [{"path": p, "kind": v["kind"], "args": v.get("args")} for p, v in nodes if v.get("kind") in ["LibraryTops", "Take"]]
            manifest = any("TopOfLibrary" in v or "ManifestDread" in v for _, v in nodes)
            by_name[face["name"]].append({"record_index": index, "actions": dict(actions), "selectors": selectors, "manifest_family": manifest})
    faces = []
    for dataset in ["oracle", "additional_cards", "tokens", "dungeons"]:
        for record in json.loads((args.repo / f"data/{dataset}.json").read_text()):
            for face in record.get("cards", []):
                text = face.get("text", "")
                hits = mentions(text)
                ir = by_name.get(face["name"], [])
                if not hits and not any(i["manifest_family"] for i in ir):
                    continue
                faces.append({"name": face["name"], "record": record["name"], "dataset": dataset, "export_status": "excluded" if record["name"] in ignored else ("cached" if ir else "unexpected_missing"), "ignore": ignored.get(record["name"]), "oracle_sha256": hashlib.sha256(text.encode()).hexdigest(), "mentions": hits, "followup_test": bool(re.search(r"\bif (?:it(?:'s| is)|that card|the (?:exiled|revealed|milled) card)", text, re.I)), "cached_ir": ir})
    report = {"scope": "Local wording discovery plus manifest/cloak/dread controls. Matches include read selectors and destinations. Export exclusions use grammars/ignore.json5; same-named IR projections do not override record-level scope.", "ir_sha256": hashlib.sha256(args.ir.read_bytes()).hexdigest(), "summary": {"face_occurrences": len(faces), "unique_names": len({f["name"] for f in faces}), "with_cached_ir": sum(bool(f["cached_ir"]) for f in faces), "excluded": sum(f["export_status"] == "excluded" for f in faces), "unexpected_missing": sum(f["export_status"] == "unexpected_missing" for f in faces), "followup_tests": sum(f["followup_test"] for f in faces)}, "faces": faces}
    if args.output.suffix == ".jsonl":
        header={key:value for key,value in report.items() if key!="faces"}
        rows=[json.dumps({"summary":header},ensure_ascii=False,separators=(",",":"))]
        rows += [json.dumps(face,ensure_ascii=False,separators=(",",":")) for face in faces]
        args.output.write_text("\n".join(rows)+"\n")
    else:
        args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(report["summary"], indent=2))

if __name__ == "__main__":
    main()
