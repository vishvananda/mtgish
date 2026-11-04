#!/usr/bin/env python3
"""Snapshot inert vocabulary and the API used by the Rust generator.

Usage: python3 rust_syntax/normalize/tools/update_cantrip_vocabulary.py ../cantrip
No executable card semantics are inferred or copied from Cantrip. FEATURES names the
generator's SWITCHED features (src/cantrip/planned.rs) whose API the tree has.
"""
import hashlib
import argparse
import json
from pathlib import Path
import re
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('checkout', type=Path)
parser.add_argument('--check', action='store_true', help='verify the snapshot without writing it')
parser.add_argument('--out', help='write the snapshot here instead of src/cantrip_vocab.rs')
args = parser.parse_args()
checkout = args.checkout.resolve()
state = checkout / 'crates/cantrip-state/src'

def variants(path, name, *, required=True):
    match = re.search(r'pub enum ' + name + r'\s*{(.*?)\n}', path.read_text(), re.S)
    if match is None:
        if not required:
            return []
        raise ValueError(f'cannot locate {name} in {path}')
    result = re.findall(r'^    ([A-Za-z_][A-Za-z_0-9]*)\s*(?:,|=)', match[1], re.M)
    if (required and not result) or len(result) != len(set(result)):
        raise ValueError(f'invalid {name} vocabulary')
    return result

def counter_kinds(path):
    match = re.search(r'^counter_kinds! \{(.*?)^\}', path.read_text(), re.S | re.M)
    if match is None:
        raise ValueError(f'cannot locate counter kinds in {path}')
    result = re.findall(r'^\s+([A-Z][A-Za-z0-9]*) = \d+,', match[1], re.M)
    if not result or len(result) != len(set(result)):
        raise ValueError('invalid counter vocabulary')
    return result

inputs = sorted(p for crate in ('cantrip-api', 'cantrip-state', 'cantrip-mechanics')
                for p in (checkout / 'crates' / crate / 'src').rglob('*.rs'))
fingerprints = {str(p.relative_to(checkout)): hashlib.sha256(p.read_bytes()).hexdigest() for p in inputs}
api_hash = hashlib.sha256(json.dumps(fingerprints, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
text = '// Cantrip vocabulary snapshot; regenerate with tools/update_cantrip_vocabulary.py.\n'
text += '// These names are inert metadata, never behavior.\n'
text += f'pub const API_FINGERPRINT: &str = {json.dumps(api_hash)};\n'
vocabularies = [('SUBTYPES', variants(state / 'subtypes.rs', 'Subtype')), ('KEYWORDS', variants(state / 'vocab.rs', 'Keyword')), ('COUNTERS', counter_kinds(state / 'vocab.rs')), ('LASTING_RULE_KINDS', variants(checkout / 'crates/cantrip-api/src/card.rs', 'LastingRuleKind', required=False))]
for name, values in vocabularies:
    text += 'pub const ' + name + ': &[&str] = &[\n' + ''.join('    ' + json.dumps(v) + ',\n' for v in values) + '];\n'
def switched_features():
    """SWITCHED features present in the same API/state/mechanics inputs that the snapshot fingerprints."""
    planned = Path(__file__).resolve().parents[1].joinpath('src/cantrip/planned.rs').read_text()
    table = re.search(r'^const SWITCHED: [^=]*= &\[\n(.*?)^\];', planned, re.S | re.M)
    entries = re.findall(r'^    \("([a-z0-9-]+)", "((?:[^"\\]|\\.)*)", &\[', table[1], re.M) if table else []
    if not entries:
        raise ValueError('cannot locate SWITCHED in planned.rs')
    api = '\n'.join(p.read_text() for p in inputs)
    from native_quantity_contract import FEATURE, present, ceiling_present
    quantity_source = Path(__file__).resolve().parents[1].joinpath('src/cantrip/quantity.rs').read_text()
    wide = present(checkout, quantity_source)
    return [name for name, shown in entries
            if (wide if name == FEATURE else
                ceiling_present(checkout, wide=wide) if name == 'counter-placement-ceiling' else
                json.loads(f'"{shown}"') in api)]

features = switched_features()
text += 'pub const FEATURES: &[&str] = &[' + ', '.join(json.dumps(f) for f in features) + '];\n'
destination = Path(args.out).resolve() if args.out else Path(__file__).resolve().parents[1].joinpath('src/cantrip_vocab.rs')
if args.check:
    snapshot = destination.read_text()
    assert re.search(r'API_FINGERPRINT:\s*&str\s*=\s*"' + api_hash + '"', snapshot), 'Cantrip API/mechanics inputs changed; review and regenerate the snapshot'
    for name, values in vocabularies:
        section = re.search(r'pub const ' + name + r':.*?=\s*&\[(.*?)\];', snapshot, re.S)
        assert section and json.loads('[' + section[1].rstrip().rstrip(',') + ']') == values, 'Cantrip vocabulary changed: ' + name
    section = re.search(r'pub const FEATURES:.*?=\s*&\[(.*?)\];', snapshot, re.S)
    assert section and json.loads('[' + section[1].rstrip().rstrip(',') + ']') == features, 'Cantrip features changed'
else:
    destination.write_text(text)
print(api_hash)
