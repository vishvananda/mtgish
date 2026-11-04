"""Validate the complete generator quantity API profile; inert signatures only."""
import json
from pathlib import Path
import re

FEATURE = 'wide-effect-quantities'

def witnesses(source):
    table = re.search(r'const WIDE_CONTRACT:.*?= &\[(.*?)\n\];', source, re.S)
    if table is None:
        raise ValueError('missing native quantity API contract')
    return [(json.loads('"' + path + '"'), json.loads('"' + signature + '"'))
            for path, signature in re.findall(r'\(\s*"([^"\\]+)"\s*,\s*"((?:[^"\\]|\\.)*)"\s*,?\s*\)', table[1])]

def present(checkout, contract_source):
    expected = witnesses(contract_source)
    if not expected:
        raise ValueError('empty native quantity API contract')
    compact = lambda value: re.sub(r'\s+', '', value)
    files = {path: compact((Path(checkout) / 'crates' / path).read_text())
             for path, _ in expected}
    missing = [(path, signature) for path, signature in expected if compact(signature) not in files[path]]
    if missing and any('i64' in signature and compact(signature) in files[path]
                       for path, signature in expected):
        raise ValueError('incomplete wide effect quantity API contract: ' + repr(missing))
    return not missing


def ceiling_present(checkout, *, wide):
    """The optional ceiling method must have this profile's placement operands."""
    compact = lambda value: re.sub(r'\s+', '', value)
    source = compact((Path(checkout) / 'crates/cantrip-api/src/game.rs').read_text())
    if 'fnadd_counters_with_limit(' not in source:
        return False
    kind = 'i64' if wide else 'i16'
    expected = f'fn add_counters_with_limit(&mut self, object: ObjectId, kind: CounterKind, amount: {kind}, maximum_total: {kind});'
    if compact(expected) not in source:
        raise ValueError('counter ceiling does not match native quantity profile: ' + kind)
    return True
