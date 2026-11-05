"""The profile is a complete API contract, not a single i64 substring."""
from pathlib import Path
import tempfile
import unittest
from native_quantity_contract import present, witnesses, ceiling_present, typed_prevention_present

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

    def test_bounded_32_contract_and_incomplete_profile_fail_closed(self):
        rows = witnesses(CONTRACT, 'BOUNDED_32')
        self.assertGreaterEqual(len(rows), 20)
        for missing in [None, *rows]:
            with self.subTest(missing=missing), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                self.tree(root, [])
                files = {}
                for path, signature in rows:
                    if missing != (path, signature):
                        files.setdefault(path, []).append(signature)
                for path, content in files.items():
                    p = root / 'crates' / path
                    p.parent.mkdir(parents=True, exist_ok=True)
                    p.write_text('\n'.join(content))
                chars = root / 'crates/cantrip-api/src/chars.rs'
                obj = root / 'crates/cantrip-state/src/object.rs'
                with chars.open('a') as out: out.write('\npub struct Chars {\n' + (' pub power: Option<i32>,\n' if missing != ('cantrip-api/src/chars.rs', 'pub power: Option<i32>,') else '') + (' pub toughness: Option<i32>,\n' if missing != ('cantrip-api/src/chars.rs', 'pub toughness: Option<i32>,') else '') + '}\n')
                with obj.open('a') as out: out.write('\npub struct Obj {\n' + (' pub x: u32,\n' if missing != ('cantrip-state/src/object.rs', 'pub x: u32,') else '') + '}\npub struct StackItem {\n' + (' pub x: u32,\n' if missing != ('cantrip-state/src/object.rs', 'pub x: u32,') else '') + '}\n')
                if missing is None:
                    self.assertTrue(present(root, CONTRACT, 'BOUNDED_32'))
                else:
                    with self.assertRaisesRegex(ValueError, 'incomplete 32-bit effect quantity API contract'):
                        present(root, CONTRACT, 'BOUNDED_32')

    def test_ceiling_matches_bounded_32_profile(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.tree(root, [])
            game = root / 'crates/cantrip-api/src/game.rs'
            with game.open('a') as out:
                out.write('fn add_counters_with_limit(&mut self, object: ObjectId, kind: CounterKind, amount: i32, maximum_total: i32);')
            self.assertTrue(ceiling_present(root, bounded_32=True))
            with self.assertRaisesRegex(ValueError, 'counter ceiling does not match'):
                ceiling_present(root, wide=False)

    def test_prevention_shape_is_independent_of_bounded_quantity_width(self):
        callback = 'fn(&dyn Query, &Asker, &DamageEvent) -> u32;'
        option = f'pub type PreventionAmount = {callback}\npub amount: Option<PreventionAmount>,'
        typed = f'pub type PreventionAmountFn = {callback}\npub enum PreventionAmount {{ All, Specific(PreventionAmountFn), Relative(PreventionAmountFn), }}\npub amount: PreventionAmount,'
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self.tree(root, [])
            replace = root / 'crates/cantrip-api/src/replace.rs'
            replace.write_text(option)
            self.assertFalse(typed_prevention_present(root, bounded_32=True))
            replace.write_text(typed)
            self.assertTrue(typed_prevention_present(root, bounded_32=True))
            for broken in [typed.replace('-> u32', '-> i64'), typed.replace('Relative(PreventionAmountFn),', ''), typed.replace('All,', '') + '\npub enum Unrelated { All, }']:
                replace.write_text(broken)
                with self.assertRaisesRegex(ValueError, 'incomplete typed prevention'):
                    typed_prevention_present(root, bounded_32=True)
            replace.write_text(option.replace('-> u32', '-> u16'))
            with self.assertRaisesRegex(ValueError, 'incomplete option prevention'):
                typed_prevention_present(root, bounded_32=True)

    def test_32_contract_rejects_each_mismatched_struct_field(self):
        rows = witnesses(CONTRACT, 'BOUNDED_32')
        for mismatch in ['power', 'toughness', 'Obj', 'StackItem']:
            with self.subTest(mismatch=mismatch), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                self.tree(root, [])
                grouped = {}
                for path, signature in rows: grouped.setdefault(path, []).append(signature)
                for path, signatures in grouped.items():
                    p = root / 'crates' / path
                    p.parent.mkdir(parents=True, exist_ok=True)
                    p.write_text('\n'.join(signatures))
                chars = root / 'crates/cantrip-api/src/chars.rs'
                with chars.open('a') as out:
                    out.write('\npub struct Chars {\n pub power: Option<' + ('i16' if mismatch == 'power' else 'i32') + '>,\n pub toughness: Option<' + ('i16' if mismatch == 'toughness' else 'i32') + '>,\n}\n')
                obj = root / 'crates/cantrip-state/src/object.rs'
                with obj.open('a') as out:
                    for name in ['Obj', 'StackItem']:
                        out.write(f'\npub struct {name} {{\n pub x: ' + ('u16' if mismatch == name else 'u32') + ',\n}\n')
                with self.assertRaisesRegex(ValueError, 'incomplete 32-bit effect quantity API contract'):
                    present(root, CONTRACT, 'BOUNDED_32')

if __name__ == '__main__':
    unittest.main()
