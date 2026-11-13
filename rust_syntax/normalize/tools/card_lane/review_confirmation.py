"""Build an evidence-addressed, diff-centered independent confirmation packet."""
import hashlib
import json
from pathlib import Path
from review_evidence import freeze


def _sha(data):
    return hashlib.sha256(data).hexdigest()


def _read(path):
    try:
        return path.read_bytes()
    except OSError as exc:
        raise ValueError(f"missing required evidence {path}: {exc}") from exc


def _json(path):
    raw = _read(path)
    try:
        return raw, json.loads(raw)
    except (ValueError, UnicodeDecodeError) as exc:
        raise ValueError(f"invalid JSON evidence {path}: {exc}") from exc


def build_bundle(folder, original, card_name, candidate, candidate_tree, source_commit, *, content_identity=None):
    """Write confirmation.md and its manifest; return the Markdown path.

    `original` is the prior review directory. The caller is responsible for verifying
    that checks' work digest and tree match the candidate before dispatch.
    """
    folder, original = Path(folder).resolve(), Path(original).resolve()
    for name, value in (("card_name", card_name), ("candidate", candidate),
                        ("candidate_tree", candidate_tree), ("source_commit", source_commit)):
        if not isinstance(value, str) or not value.strip():
            raise ValueError(f"missing {name}")
    report_bytes, report = _json(original / "review.json")
    checks_bytes, checks = _json(original / "checks.json")
    diff = _read(original / "fixes.diff")
    try:
        diff_text = diff.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise ValueError("fixes.diff is not UTF-8") from exc
    if not diff_text:
        raise ValueError("fixes.diff is empty")
    if not isinstance(report, dict) or report.get("decision") != "pass":
        raise ValueError("prior review must pass")
    validation = report.get("validation")
    if not isinstance(validation, list) or not validation or any(
        not isinstance(v, dict) or v.get("agrees") is not True for v in validation
    ):
        raise ValueError("prior review validation must be present and agree")
    if card_name not in report.get("affected_cards", []):
        raise ValueError("prior review affected_cards must include the current card")
    gates = checks.get("gates") if isinstance(checks, dict) else None
    if not isinstance(checks, dict) or checks.get("ok") is not True or not isinstance(gates, list) or not gates or any(
        not isinstance(g, dict) or g.get("ok") is not True or not g.get("gate") for g in gates
    ):
        raise ValueError("checks and every named gate must pass")
    if not checks.get("head") or not checks.get("diff_digest"):
        raise ValueError("checks must include head and diff_digest")
    result_path = original / "result.json"
    result_raw, result = _json(result_path) if result_path.exists() else (None, {})
    if not isinstance(result, dict):
        raise ValueError("invalid result.json object")
    folder.mkdir(parents=True, exist_ok=True)
    files = {"diff": original / "fixes.diff", "report": original / "review.json",
             "checks": original / "checks.json"}
    hashes = {"diff": _sha(diff), "report": _sha(report_bytes), "checks": _sha(checks_bytes)}
    if result_raw is not None:
        files["result"] = result_path
        hashes["result"] = _sha(result_raw)
    for name, path in list(files.items()):
        files[name] = freeze(folder / 'evidence/inputs', _read(path), path.suffix)
    evidence = []
    for gate in gates:
        for artifact in [gate, *gate.get('artifacts', [])]:
            log = artifact.get("log")
            if not log:
                continue
            path = Path(log)
            if not path.is_absolute():
                path = original / path
            path = path.resolve()
            item = {"gate": gate["gate"], "path": str(path)}
            if artifact.get('kind'):
                item['kind'] = artifact['kind']
            if path.is_file():
                raw = path.read_bytes()
                if artifact.get('sha256') and artifact['sha256'] != _sha(raw):
                    raise ValueError(f'mutation artifact hash mismatch: {path}')
                item.update(original=str(path), path=str(freeze(folder / 'evidence/logs', raw, path.suffix)), sha256=_sha(raw))
            else:
                item["missing"] = True
            evidence.append(item)
    for path in sorted(original.rglob("*")):
        if path.is_file() and ("manual" in path.name.lower() or "probe" in path.name.lower() or "manifest" in path.name.lower() or path.is_relative_to(original / 'evidence') or
                               path.is_relative_to(original / "scratch")):
            raw = path.read_bytes()
            evidence.append({"original": str(path), "path": str(freeze(folder / 'evidence/attachments', raw, path.suffix)), "sha256": _sha(raw)})
    manifest = {"card": card_name, "candidate": candidate, "candidate_tree": candidate_tree,
                "source_commit": source_commit, "checks_head": checks["head"],
                "checks_diff_digest": checks["diff_digest"],
                "files": {k: {"path": str(files[k]), "sha256": hashes[k]} for k in files},
                "evidence": evidence}
    if content_identity is not None:
        manifest['checked_content_identity'] = content_identity
    (folder / "confirmation-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    summaries = [{k: v for k, v in gate.items() if k != "tail"} for gate in gates]
    sections = [
        f"# Independent confirmation: {card_name}",
        f"Candidate commit: `{candidate}`\nCandidate tree: `{candidate_tree}`\nSource commit: `{source_commit}`",
        "The driver must verify the candidate and checks digest/tree before dispatch. "
        "These identities and evidence hashes bind the review to this candidate; do not assume a semantic change is mechanical.",
        "## Evidence manifest\n```json\n" + json.dumps(manifest, indent=2) + "\n```",
        "## Exact complete fix diff\n```diff\n" + diff_text + ("" if diff_text.endswith("\n") else "\n") + "```",
        "## Fix report and Oracle validation\n```json\n" + json.dumps(report, indent=2, ensure_ascii=False) + "\n```",
        "## Gate summary and log references\n```json\n" + json.dumps(summaries, indent=2, ensure_ascii=False) + "\n```",
        "## Changed files and callers\n" + json.dumps(result.get("changed", []), indent=2) +
        "\nInspect changed shared mechanics and their existing callers as needed; this list does not establish caller safety.",
        f"## Expansion and fallback\nFull current packet: {folder / 'packet.md'}\n"
        f"Original packet: {original / 'packet.md'}\n"
        f"Manual-break evidence and probes: see manifest evidence and {original}; "
        "inspect the actual referenced evidence and logs for proof claims. Missing evidence or unresolved uncertainty blocks confirmation.",
    ]
    path = folder / "confirmation.md"
    path.write_text("\n\n".join(sections) + "\n", encoding="utf-8")
    return path


def compact_prompt(folder, card_name):
    """Return the independent verifier prompt for the generated bundle."""
    folder = Path(folder).resolve()
    return f'''Independently confirm the exact fix for {card_name}. Read {folder / 'confirmation.md'} and its manifest first. Read .claude/skills/add-cards/review.md and the non-negotiables and never list in .claude/skills/add-cards/SKILL.md once per context. The full packet at {folder / 'packet.md'} remains available for targeted expansion.

Verify the exact fix's semantics against Oracle, the relevant code, shared mechanics and changed existing callers. Expand to source, scenarios, evidence and the full packet wherever needed to resolve uncertainty; do not redo the original review wholesale. Check the manifest hashes against evidence you rely on; read the actual referenced logs and manual-break/probe evidence before relying on proof claims. Do not infer semantic equivalence or safety from the diff alone. Do not change source, mutate git, use network, or spawn agents. Do not rerun unchanged gates unless stale, missing, or a specific concern requires it. Block on missing evidence, unresolved uncertainty, or any unverified semantic change.

The manifest points to immutable snapshots. If a historical manifest names a live checks.json or log that has since been overwritten, look for its exact hash in the archived evidence/checks and evidence/maps artifacts included in this bundle. Accept only hash-matching original bytes and their bound logs; a similar candidate, digest or later successful run is not a substitute for missing bytes.

Write {folder / 'review.json'} compatible with report_valid: decision pass or blocked, validation entries with agrees, affected_cards including {card_name}, fixes as an empty list, and a nonempty client field. Include findings and evidence for the decision. Do not make fixes.

Apply add-cards/SKILL.md step 8 accurately: existing prompts, actions and state need no browser test. Caption corrections and already-rendered choice kinds can be verified through focused engine metadata and the existing presenter; a missing .NET SDK alone does not block those changes. New client capabilities still require client changes and browser proof. Never claim an unrun browser check passed.
'''
