import hashlib
import json
from pathlib import Path
import tempfile
import unittest

import review_evidence as evidence


class EvidenceTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.folder = Path(self.tmp.name)
        self.log = self.folder / 'corpus.log'
        self.log.write_text('original passing run')
        self.result = dict(ok=True, head='candidate', diff_digest='exact-tree',
                           gates=[dict(gate='corpus', ok=True, log=str(self.log))])

    def test_rerunning_checks_cannot_change_a_hash_bound_old_receipt_or_log(self):
        original, path = evidence.freeze_checks(self.folder, self.result)
        before = path.read_bytes()
        original_log = Path(original['gates'][0]['log'])
        self.log.write_text('later independent run')
        later, other = evidence.freeze_checks(self.folder, self.result)
        self.assertNotEqual(path, other)
        self.assertEqual(path.read_bytes(), before)
        self.assertEqual(path.stem, hashlib.sha256(before).hexdigest())
        self.assertEqual(original_log.read_text(), 'original passing run')
        self.assertEqual(Path(later['gates'][0]['log']).read_text(), 'later independent run')

    def test_old_mutable_receipt_is_recoverable_by_its_original_manifest_hash(self):
        raw = (json.dumps(self.result, indent=2) + '\n').encode()
        (self.folder / 'checks.json').write_bytes(raw)
        expected = hashlib.sha256(raw).hexdigest()
        evidence.archive_previous_checks(self.folder)
        (self.folder / 'checks.json').write_text('overwritten')
        self.log.write_text('overwritten too')
        self.assertEqual((self.folder / 'evidence/checks' / (expected + '.json')).read_bytes(), raw)
        maps = list((self.folder / 'evidence/maps').glob('*.json'))
        mapping = json.loads(maps[0].read_text())
        self.assertEqual(mapping['checks_sha256'], expected)
        self.assertEqual(Path(mapping['logs'][0]['path']).read_text(), 'original passing run')

    def test_corrupted_existing_hash_address_is_rejected(self):
        _, path = evidence.freeze_checks(self.folder, self.result)
        path.write_text('corrupt')
        with self.assertRaisesRegex(ValueError, 'corrupt immutable evidence'):
            evidence.freeze_checks(self.folder, self.result)
