#!/usr/bin/env python3
"""Assemble a self-contained, offline HTML demo; no network dependencies at runtime."""
import argparse
import base64
import gzip
import hashlib
import json
import re
from pathlib import Path


def compressed(data):
    return base64.b64encode(gzip.compress(data, compresslevel=9, mtime=0)).decode()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('build', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    source = Path(__file__).resolve().parent
    frontend = '\n'.join((source / name).read_text() for name in ('bindings.js', 'sections.js', 'projections.js', 'named-abilities.js', 'selectors.js', 'properties.js', 'costs.js', 'app.js'))
    schema = json.loads((args.build / 'schema.json').read_text())
    wasm = (args.build / 'wasm/validator_bg.wasm').read_bytes()
    records, catalog, shards = [], [], []
    count = 0
    with (args.build / 'cards.jsonl').open() as stream:
        for line in stream:
            value = json.loads(line)
            if value['schema_version'] != schema['version']:
                raise ValueError('IR schema/catalog version mismatch')
            titles = [f['name'] for f in value['faces']]
            if titles:
                catalog.append({'name': ' // '.join(titles), 'shard': len(shards), 'index': len(records),
                                'types': ' · '.join(dict.fromkeys(t for f in value['faces'] for t in f['characteristics']['card_types']))})
            records.append(value)
            count += 1
            if len(records) == 64:
                shards.append(compressed(json.dumps(records, separators=(',', ':'), ensure_ascii=False).encode()))
                records = []
    if records:
        shards.append(compressed(json.dumps(records, separators=(',', ':'), ensure_ascii=False).encode()))
    catalog.sort(key=lambda c: c['name'].casefold())
    payload = {'schema': schema, 'record_count': count, 'cards': catalog, 'shards': shards, 'wasm': compressed(wasm),
               'model': hashlib.sha256(json.dumps(schema, sort_keys=True).encode() + wasm + frontend.encode()).hexdigest()[:16]}
    payload['legacy_models'] = {}
    for legacy_path in sorted((source / 'fixtures').glob('schema-*.json')):
        legacy = json.loads(legacy_path.read_text())
        if legacy['version'] < schema['version']:
            payload['legacy_models'][str(legacy['version'])] = legacy
    core = (source / 'vendor/blockly_compressed.js').read_text()
    messages = (source / 'vendor/msg/en.js').read_text()
    replacements = {
        'PAYLOAD': json.dumps(payload, separators=(',', ':'), ensure_ascii=False).replace('<', '\\u003c'),
        'BLOCKLY': re.sub(r'//# sourceMappingURL=.*', '', core + '\n' + messages).replace('</script', '<\\/script'),
        'GLUE': json.dumps((args.build / 'wasm/validator.js').read_text()).replace('<', '\\u003c'),
        'APP': frontend.replace('</script', '<\\/script'),
        'SPRITE': 'data:image/svg+xml;base64,' + base64.b64encode((source / 'vendor/media/sprites.svg').read_bytes()).decode(),
        'LICENSE': (source / 'vendor/LICENSE.blockly').read_text().replace('--', '—'),
    }
    html = (source / 'index.html').read_text()
    for name, value in replacements.items():
        html = html.replace('@@' + name + '@@', value)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(html)
    print(f'{args.output}: {count:,} records, {len(catalog):,} searchable cards, schema {schema["version"]}, {args.output.stat().st_size / 1024 / 1024:.1f} MiB')


if __name__ == '__main__':
    main()
