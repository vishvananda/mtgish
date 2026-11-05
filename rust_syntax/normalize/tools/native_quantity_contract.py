"""Validate the complete generator quantity API profile; inert signatures only."""
import json
from pathlib import Path
import re

FEATURE = 'wide-effect-quantities'
BOUNDED_32 = '32-bit-effect-quantities'

def witnesses(source, profile='WIDE'):
    table = re.search(r'const ' + profile + r'_CONTRACT:.*?= &\[(.*?)\n\];', source, re.S)
    if table is None:
        raise ValueError('missing native quantity API contract')
    return [(json.loads('"' + path + '"'), json.loads('"' + signature + '"'))
            for path, signature in re.findall(r'\(\s*"([^"\\]+)"\s*,\s*"((?:[^"\\]|\\.)*)"\s*,?\s*\)', table[1])]

def present(checkout, contract_source, profile='WIDE'):
    expected = witnesses(contract_source, profile)
    if not expected:
        raise ValueError('empty native quantity API contract')
    compact = lambda value: re.sub(r'\s+', '', value)
    files = {path: compact((Path(checkout) / 'crates' / path).read_text())
             for path, _ in expected}
    missing = [(path, signature) for path, signature in expected if compact(signature) not in files[path]]
    anchors = ([signature for _, signature in expected
                if ('damage(' in signature or 'damage_result(' in signature or 'damage_together(' in signature
                    or 'counters(' in signature or 'counters_together' in signature or 'counters_by' in signature
                    or 'counts:' in signature or 'pub counters:' in signature
                    or 'pub damage:' in signature or 'type PreventionAmount =' in signature)]
               if profile == 'BOUNDED_32' else [signature for _, signature in expected if 'i64' in signature])
    if missing and any(compact(anchor) in contents for anchor in anchors for contents in files.values()):
        raise ValueError('incomplete ' + ('32-bit' if profile == 'BOUNDED_32' else 'wide') + ' effect quantity API contract: ' + repr(missing))
    if not missing and profile == 'BOUNDED_32':
        structures = [("cantrip-api/src/chars.rs", "Chars", ["pub power: Option<i32>", "pub toughness: Option<i32>"]),
                      ("cantrip-state/src/object.rs", "Obj", ["pub x: u32"]),
                      ("cantrip-state/src/object.rs", "StackItem", ["pub x: u32"])]
        for path, name, fields in structures:
            source = (Path(checkout) / 'crates' / path).read_text()
            block = re.search(r'pub\s+struct\s+' + name + r'\s*\{(.*?)\n\}', source, re.S)
            if not block or any(compact(field) not in compact(block[1]) for field in fields):
                raise ValueError('incomplete 32-bit effect quantity API contract: ' + name)
    return not missing


def ceiling_present(checkout, *, wide=False, bounded_32=False):
    """The optional ceiling method must have this profile's placement operands."""
    compact = lambda value: re.sub(r'\s+', '', value)
    source = compact((Path(checkout) / 'crates/cantrip-api/src/game.rs').read_text())
    if 'fnadd_counters_with_limit(' not in source:
        return False
    if wide and bounded_32:
        raise ValueError('conflicting native quantity profiles')
    kind = 'i64' if wide else 'i32' if bounded_32 else 'i16'
    expected = f'fn add_counters_with_limit(&mut self, object: ObjectId, kind: CounterKind, amount: {kind}, maximum_total: {kind});'
    if compact(expected) not in source:
        raise ValueError('counter ceiling does not match native quantity profile: ' + kind)
    return True


def typed_prevention_present(checkout, *, wide=False, bounded_32=False):
    """Validate the prevention representation independently of quantity width."""
    compact = lambda value: re.sub(r'\s+', '', value)
    source = compact((Path(checkout) / 'crates/cantrip-api/src/replace.rs').read_text())
    kind = 'i64' if wide else 'u32' if bounded_32 else 'u16'
    signature = f'fn(&dyn Query, &Asker, &DamageEvent) -> {kind};'
    if 'pubenumPreventionAmount{' in source:
        required = ['pub type PreventionAmountFn = ' + signature, 'pub amount: PreventionAmount,']
        enum = re.search(r'pubenumPreventionAmount\{([^}]*)\}', source)
        variants = ['All,', 'Specific(PreventionAmountFn),', 'Relative(PreventionAmountFn),']
        if (any(compact(item) not in source for item in required)
                or enum is None or any(compact(item) not in enum[1] for item in variants)):
            raise ValueError('incomplete typed prevention API contract: ' + kind)
        return True
    required = ['pub type PreventionAmount = ' + signature, 'pub amount: Option<PreventionAmount>,']
    if any(compact(item) not in source for item in required):
        raise ValueError('incomplete option prevention API contract: ' + kind)
    return False
