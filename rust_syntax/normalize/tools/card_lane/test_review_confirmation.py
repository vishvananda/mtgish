import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from review_confirmation import build_bundle, compact_prompt


class ConfirmationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.original = self.root / "review"
        self.folder = self.root / "confirm"
        self.original.mkdir()
        self.report = {"decision": "pass", "validation": [{"oracle": "text", "agrees": True}],
                       "affected_cards": ["Card"], "fixes": [{"proof": "see log"}]}
        self.checks = {"ok": True, "head": "head", "diff_digest": "digest", "gates": [
            {"gate": "oracle", "ok": True, "log": "oracle.log"}]}
        self.write_json("review.json", self.report)
        self.write_json("checks.json", self.checks)
        self.diff = "diff --git a/a b/a\n" + "+line\n" * 2000 + "+LAST-LINE\n"
        (self.original / "fixes.diff").write_text(self.diff)
        (self.original / "oracle.log").write_text("evidence")
        self.write_json("result.json", {"changed": ["a"]})

    def write_json(self, name, data):
        (self.original / name).write_text(json.dumps(data))

    def build(self):
        return build_bundle(self.folder, self.original, "Card", "candidate", "tree", "source")

    def test_full_diff_fallback_and_hashes(self):
        bundle = self.build().read_text()
        self.assertIn(self.diff, bundle)
        self.assertIn(str(self.folder / "packet.md"), bundle)
        manifest = json.loads((self.folder / "confirmation-manifest.json").read_text())
        self.assertEqual(manifest["files"]["diff"]["sha256"], hashlib.sha256(self.diff.encode()).hexdigest())
        self.assertEqual(manifest["candidate_tree"], "tree")
        self.assertIn("a", bundle)
        self.assertIn("client", compact_prompt(self.folder, "Card"))

    def test_modified_evidence_changes_hash(self):
        self.build()
        before = json.loads((self.folder / "confirmation-manifest.json").read_text())
        (self.original / "oracle.log").write_text("different evidence")
        self.report["reason"] = "different"
        self.write_json("review.json", self.report)
        self.build()
        after = json.loads((self.folder / "confirmation-manifest.json").read_text())
        self.assertNotEqual(before["evidence"][0]["sha256"], after["evidence"][0]["sha256"])
        self.assertNotEqual(before["files"]["report"]["sha256"], after["files"]["report"]["sha256"])

    def test_missing_or_failed_checks_block(self):
        for checks in ({}, {**self.checks, "ok": False},
                       {**self.checks, "gates": [{"gate": "oracle", "ok": False}]},
                       {**self.checks, "head": ""}):
            with self.subTest(checks=checks):
                self.write_json("checks.json", checks)
                with self.assertRaises(ValueError):
                    self.build()
        (self.original / "checks.json").unlink()
        with self.assertRaisesRegex(ValueError, "missing required evidence"):
            self.build()

    def test_report_validation_blocks(self):
        self.report["validation"][0]["agrees"] = False
        self.write_json("review.json", self.report)
        with self.assertRaisesRegex(ValueError, "validation"):
            self.build()

    def test_raw_mutation_artifacts_are_individually_bound_and_checked(self):
        output=self.original/'mutate.txt';output.write_text('line 1: caught\n')
        results=self.original/'mutants.json';results.write_text('{"one":"caught"}')
        self.checks['gates'].append(dict(gate='mutation',ok=True,artifacts=[
            dict(kind='raw mutation output',log=str(output),sha256=hashlib.sha256(output.read_bytes()).hexdigest()),
            dict(kind='per-mutant results',log=str(results),sha256=hashlib.sha256(results.read_bytes()).hexdigest())]))
        self.write_json('checks.json',self.checks)
        self.build()
        manifest=json.loads((self.folder/'confirmation-manifest.json').read_text())
        bound=[x for x in manifest['evidence'] if x['gate']=='mutation']
        self.assertEqual([x['kind'] for x in bound],['raw mutation output','per-mutant results'])
        self.assertEqual(Path(bound[1]['path']).read_bytes(),results.read_bytes())
        results.write_text('overwritten')
        self.assertEqual(Path(bound[1]['path']).read_text(),'{"one":"caught"}')
        with self.assertRaisesRegex(ValueError,'mutation artifact hash mismatch'):
            self.build()

    def test_confirmation_inputs_survive_later_live_receipt_and_log_overwrites(self):
        self.build()
        manifest = json.loads((self.folder / 'confirmation-manifest.json').read_text())
        (self.original / 'checks.json').write_text('a later check')
        (self.original / 'review.json').write_text('a later report')
        (self.original / 'oracle.log').write_text('a later log')
        for item in manifest['files'].values():
            self.assertEqual(hashlib.sha256(Path(item['path']).read_bytes()).hexdigest(), item['sha256'])
        self.assertEqual(Path(manifest['evidence'][0]['path']).read_text(), 'evidence')


if __name__ == "__main__":
    unittest.main()
