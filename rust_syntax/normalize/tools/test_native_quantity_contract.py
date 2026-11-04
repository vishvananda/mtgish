"""The profile is a complete API contract, not a single i64 substring."""
from pathlib import Path
import tempfile
import unittest
from native_quantity_contract import present, witnesses, ceiling_present

CONTRACT = Path(__file__).resolve().parents[1].joinpath('src/cantrip/quantity.rs').read_text()

class NativeQuantityContract(unittest.TestCase):
    def test_contract_contains_every_reviewed_boundary(self):
        self.assertEqual(len(witnesses(CONTRACT)), 26)

    def tree(self, root, rows):
        files = {}
        for path, signature in rows:
            files.setdefault(path, []).append(signature)
        # All referenced input files exist on both actual API versions.
        for path, _ in witnesses(CONTRACT):
            p = root / 'crates' / path
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text('\n'.join(files.get(path, [])))

    def test_old_profile_stays_old(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.tree(root, [])
            self.assertFalse(present(root, CONTRACT))

    def test_whitespace_does_not_change_the_contract(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.tree(root, [(path, text.replace(', ', ',\n    ')) for path, text in witnesses(CONTRACT)])
            self.assertTrue(present(root, CONTRACT))

    def test_every_missing_boundary_fails_closed(self):
        rows = witnesses(CONTRACT)
        for missing in rows:
            with self.subTest(signature=missing), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                self.tree(root, [row for row in rows if row != missing])
                with self.assertRaisesRegex(ValueError, 'incomplete wide effect quantity API contract'):
                    present(root, CONTRACT)

    def test_ceiling_follows_each_placement_profile(self):
        for wide, kind in [(False, 'i16'), (True, 'i64')]:
            with self.subTest(wide=wide), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                self.tree(root, witnesses(CONTRACT) if wide else [])
                game = root / 'crates/cantrip-api/src/game.rs'
                with game.open('a') as out:
                    out.write(f'fn add_counters_with_limit(&mut self, object: ObjectId, kind: CounterKind, amount: {kind}, maximum_total: {kind});')
                self.assertTrue(ceiling_present(root, wide=wide))

    def test_mixed_ceiling_width_fails_closed(self):
        for wide, wrong in [(False, 'i64'), (True, 'i16')]:
            with self.subTest(wide=wide), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                self.tree(root, witnesses(CONTRACT) if wide else [])
                game = root / 'crates/cantrip-api/src/game.rs'
                with game.open('a') as out:
                    out.write(f'fn add_counters_with_limit(&mut self, object: ObjectId, kind: CounterKind, amount: {wrong}, maximum_total: {wrong});')
                with self.assertRaisesRegex(ValueError, 'counter ceiling does not match'):
                    ceiling_present(root, wide=wide)

if __name__ == '__main__':
    unittest.main()
