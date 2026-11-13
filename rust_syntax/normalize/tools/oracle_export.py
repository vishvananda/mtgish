#!/usr/bin/env python3
"""Verify a pinned native face bundle against fresh MTGish parsing and AST lowering."""
import argparse
from collections import Counter
import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[3]


def digest(path):
    result = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for block in iter(lambda: stream.read(1024*1024), b''):
            result.update(block)
    return result.hexdigest()


def write(path, value):
    Path(path).write_text(json.dumps(value, ensure_ascii=False, indent=2)+'\n')


def verify_bundle(directory, manifest_sha256):
    directory = Path(directory)
    if digest(directory/'manifest.json') != manifest_sha256:
        raise ValueError('release manifest checksum mismatch')
    manifest = json.loads((directory/'manifest.json').read_text())
    if manifest.get('format_version') != 1 or manifest.get('kind') != 'oracle-grammar-native-faces':
        raise ValueError('expected native face release format 1')
    sums = {}
    for line in (directory/'SHA256SUMS').read_text().splitlines():
        checksum, name = line.split('  ', 1)
        if not name or Path(name).name != name or name in sums:
            raise ValueError('invalid release checksum inventory')
        if digest(directory/name) != checksum:
            raise ValueError('release asset checksum mismatch: '+name)
        sums[name] = checksum
    if set(sums) != set(manifest['assets']) | {'manifest.json'}:
        raise ValueError('manifest/checksum inventories differ')
    for name, asset in manifest['assets'].items():
        if sums[name] != asset['sha256'] or (directory/name).stat().st_size != asset['bytes']:
            raise ValueError('manifest asset mismatch: '+name)
    if not {'faces.jsonl.gz', 'outcomes.jsonl.gz', 'coverage.json'} <= set(manifest['assets']):
        raise ValueError('native release is missing required assets')
    return manifest


def selected_records(records, population):
    if not isinstance(records, list) or not population.get('policy'):
        raise ValueError('expected source array and explicit population policy')
    if population.get('kind') == 'all':
        return records
    if population.get('kind') != 'oracle_ids':
        raise ValueError('unsupported release population')
    ids = population.get('oracle_ids', [])
    if not ids or len(ids) != len(set(ids)):
        raise ValueError('missing or repeated selected Oracle IDs')
    wanted = set(ids)
    selected = [r for r in records if r.get('oracle_id') in wanted]
    if {r['oracle_id'] for r in selected} != wanted:
        raise ValueError('selected Oracle ID absent from snapshot')
    return selected


def read_gzip(path):
    with gzip.open(path, 'rt', encoding='utf-8') as stream:
        return [json.loads(line) for line in stream]


def original_records(path, source):
    encoding = source.get('encoding', 'json')
    if encoding == 'jsonl-gzip':
        with gzip.open(path, 'rt', encoding='utf-8') as stream:
            return [json.loads(line) for line in stream if line.strip()]
    if encoding != 'json':
        raise ValueError('unsupported original Oracle source encoding')
    return json.loads(Path(path).read_text())


def requests_for(records, outcomes):
    expected = {}
    for i, record in enumerate(records):
        faces = record.get('card_faces', [record])
        names = [face['name'] for face in faces]
        for j, face in enumerate(faces):
            source = dict(face); source.pop('card_faces', None)
            expected[(i,j)] = (record, source, names)
    seen, requests = set(), []
    for row in outcomes:
        identity = row['identity']
        position = (identity['record_index'], identity['face_index'])
        if position in seen or position not in expected:
            raise ValueError('duplicate or unknown exported source occurrence')
        seen.add(position)
        record, source, names = expected[position]
        if (identity['provider_id'] != record.get('oracle_id', record.get('id'))
            or identity['layout'] != record['layout'] or identity.get('record_name') != record['name']
            or row['name'] != source['name']
            or row['source'] != source):
            raise ValueError('exported identity/source differs from the pinned Oracle snapshot')
        status, readings = row['status'], row['readings']
        values = {json.dumps(r['card'], sort_keys=True, ensure_ascii=False) for r in readings}
        if status == 'unique' and (len(values) != 1 or row.get('semantic_readings') != 1):
            raise ValueError('unique export does not contain one distinct value')
        if status == 'ambiguous' and (len(values) < 2 or row.get('semantic_readings') != len(values)):
            raise ValueError('ambiguous export lost readings')
        if status in ('refused', 'limit') and readings:
            raise ValueError('refusal contains partial success')
        if status not in ('unique', 'ambiguous', 'refused', 'limit'):
            raise ValueError('unknown export status')
        requests.append({'identity':identity, 'record_names':names, 'name':row['name'],
                         'grammar_status':status,
                         'expected':json.loads(next(iter(values))) if status == 'unique' else None})
    if seen != set(expected):
        raise ValueError('release omitted source face outcomes')
    return requests


def fingerprints():
    files = [ROOT/'preprocess_scryfall', ROOT/'rust_syntax/Cargo.lock',
             ROOT/'data/grammar_audit_review.json', Path(__file__)]
    for directory, suffix in (('grammars', '.json5'), ('go_mtg_parser', '.go'),
                              ('rust_syntax/src', '.rs'), ('rust_syntax/normalize/src', '.rs')):
        files += list((ROOT/directory).rglob('*'+suffix))
    files += [ROOT/p for p in ('go_mtg_parser/go.mod', 'rust_syntax/Cargo.toml',
                              'rust_syntax/normalize/Cargo.toml', 'rust_syntax/ir/Cargo.toml',
                              'rust_syntax/ir/src/lib.rs')]
    return {str(p.relative_to(ROOT)): digest(p) for p in sorted(set(files))}


def model_packages():
    packages = tomllib.loads((ROOT/'rust_syntax/Cargo.lock').read_text())['package']
    result = {}
    for name in ('oracle-ir', 'oracle-ir-contract'):
        values = [p for p in packages if p['name'] == name]
        if len(values) != 1 or not values[0].get('source', '').startswith('registry+'):
            raise ValueError('expected one published registry dependency for '+name)
        result[name] = {k: values[0][k] for k in ('version', 'source', 'checksum')}
    return result


def build_parser(cache):
    files = sorted((ROOT/'go_mtg_parser').glob('*.go'))+[ROOT/'go_mtg_parser/go.mod']
    compiler = subprocess.check_output(['go','version'],text=True).strip()
    key = hashlib.sha256((compiler+''.join(digest(p) for p in files)).encode()).hexdigest()
    cache.mkdir(parents=True, exist_ok=True)
    binary, receipt = cache/'go_mtg_parser', cache/'parser.json'
    if binary.exists() and receipt.exists():
        previous = json.loads(receipt.read_text())
        if previous == {'inputs':key, 'binary':digest(binary)}:
            return binary
    subprocess.run(['go', 'build', '-o', str(binary), '.'], cwd=ROOT/'go_mtg_parser', check=True)
    write(receipt, {'inputs':key, 'binary':digest(binary)})
    return binary


def parse_source(records, work, cache):
    original, prepared = work/'selected-oracle.json', work/'preprocessed.json'
    write(original, records)
    with prepared.open('w') as output:
        subprocess.run([sys.executable, str(ROOT/'preprocess_scryfall'), str(original)], stdout=output, check=True)
    subset = work/'subset.json'
    write(subset, [r['name'] for r in json.loads(prepared.read_text())])
    command = [str(build_parser(cache)), '--ignore-list-file', str(ROOT/'grammars/ignore.json5'),
               '--short-names-file', str(ROOT/'grammars/short_names.json5'),
               '--find-replace-file', str(ROOT/'grammars/find_replace.json5'), '--english-grammar-files',
               *map(str, sorted((ROOT/'grammars').glob('english_grammar*.json5'))), '--mtgish-grammar-files',
               *map(str, sorted((ROOT/'grammars').glob('mtgish_grammar*.json5'))),
               '--mtgjson-files', str(prepared), '--limit-to-subset', str(subset),
               '--audit-review', str(ROOT/'data/grammar_audit_review.json'),
               '--continue-after-failures', '--print-mtgish']
    with (work/'parser-stdout.log').open('w') as stdout, (work/'parser-stderr.log').open('w') as stderr:
        parsed = subprocess.run(command, cwd=ROOT, stdout=stdout, stderr=stderr)
    failures, source = [], []
    for line in (work/'parser-stdout.log').read_text().splitlines():
        if line.startswith('FAIL '): failures.append(line)
        elif line.strip(): source.append(line)
    if parsed.returncode and not (parsed.returncode == 1 and failures):
        raise ValueError('MTGish parser failed; see parser-stderr.log')
    output = work/'fresh-source.ronl'
    output.write_text(''.join(line+'\n' for line in source))
    write(work/'parser-failures.json', failures)
    return output, parsed.returncode


def build_ast(work):
    command = ['cargo', 'build', '--locked', '--manifest-path', str(ROOT/'rust_syntax/Cargo.toml'),
               '-p', 'mtgish-ast', '--bin', 'ast', '--features', 'cantrip,serde_json/preserve_order',
               '--message-format=json']
    with (work/'rust-build.log').open('w') as log:
        result = subprocess.run(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
    if result.returncode:
        raise ValueError('AST build failed; see rust-build.log')
    for line in (work/'rust-build.log').read_text().splitlines():
        if not line.startswith('{'): continue
        value = json.loads(line)
        if value.get('reason') == 'compiler-artifact' and value['target']['name'] == 'ast' and value.get('executable'):
            return Path(value['executable'])
    raise ValueError('Cargo did not report the AST executable')


def check(bundle, oracle, manifest_sha256, work):
    bundle, oracle, work = Path(bundle).resolve(), Path(oracle).resolve(), Path(work).resolve()
    manifest = verify_bundle(bundle, manifest_sha256)
    if manifest['source_oracle']['provider'] != 'scryfall' or digest(oracle) != manifest['source_oracle']['sha256']:
        raise ValueError('Oracle provider/snapshot checksum differs from export')
    packages = model_packages()
    for name, package in packages.items():
        if manifest['model']['packages'][name] != package['version']:
            raise ValueError('model version differs from export: '+name)
    records = selected_records(original_records(oracle, manifest['source_oracle']), manifest['population'])
    outcomes = read_gzip(bundle/'outcomes.jsonl.gz')
    if read_gzip(bundle/'faces.jsonl.gz') != [r for r in outcomes if r['status'] == 'unique']:
        raise ValueError('accepted faces and outcomes disagree')
    coverage = json.loads((bundle/'coverage.json').read_text())
    actual_counts = dict(Counter(r['status'] for r in outcomes))
    recorded_counts = {k:v for k,v in coverage['counts'].items() if v}
    if (coverage != manifest['counts'] or coverage['input_faces'] != len(outcomes)
        or actual_counts != recorded_counts):
        raise ValueError('export coverage counts disagree with outcomes')
    if any(r.get('format_version') != 1 or r.get('schema_version') != manifest['model']['schema_version'] for r in outcomes):
        raise ValueError('outcome container/schema differs from manifest')
    requests = requests_for(records, outcomes)
    if not requests: raise ValueError('comparison population is empty')
    before = fingerprints()
    work.mkdir(parents=True, exist_ok=False)
    write(work/'requests.json', requests)
    source, parser_status = parse_source(records, work, work.parent/'parser-cache')
    binary = build_ast(work)
    comparison = work/'comparison.json'
    result = subprocess.run([str(binary), 'compare-oracle-export', str(source), str(work/'requests.json'), str(comparison)], cwd=ROOT)
    if not comparison.exists(): raise ValueError('AST comparison failed before producing a report')
    report = json.loads(comparison.read_text())
    if before != fingerprints(): raise ValueError('MTGish inputs changed during comparison')
    if report['input_faces'] != len(requests) or len(report['findings']) != len(outcomes):
        raise ValueError('comparison omitted input occurrences')
    if result.returncode not in (0,1) or report['passed'] != (result.returncode == 0):
        raise ValueError('comparison report and process exit status disagree')
    for finding, row in zip(report['findings'], outcomes):
        if finding['identity'] != row['identity']:
            raise ValueError('comparison changed source occurrence identity')
        finding['oracle_text'] = row['source'].get('oracle_text', '')
    write(comparison, report)
    receipt = {'format_version':1, 'passed':report['passed'], 'complete_agreement':report['complete_agreement'],
               'mtgish_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
               'toolchain':{name:subprocess.check_output(command,text=True).strip() for name,command in
                            (('rustc',['rustc','-V']),('cargo',['cargo','-V']),('go',['go','version']))},
               'tracked_changes':bool(subprocess.check_output(['git','diff','HEAD','--name-only'],cwd=ROOT,text=True).strip()),
               'bundle_manifest_sha256':manifest_sha256, 'source_oracle':manifest['source_oracle'],
               'grammar':manifest['grammar'], 'model':manifest['model'], 'registry_packages':packages,
               'population':manifest['population'], 'bundle_tag':manifest.get('tag'),
               'build_features':['cantrip','serde_json/preserve_order'],
               'inputs':before, 'fresh_source_sha256':digest(source), 'ast_binary_sha256':digest(binary),
               'parser_exit_code':parser_status, 'comparison_exit_code':result.returncode,
               'comparison_sha256':digest(comparison), 'counts':report['counts']}
    write(work/'receipt.json', receipt)
    print(json.dumps({k:receipt[k] for k in ('passed','complete_agreement','counts')},indent=2))
    return report['passed']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bundle', required=True, help='downloaded native face release directory')
    parser.add_argument('--oracle', required=True, help='original Scryfall JSON named by that release')
    parser.add_argument('--manifest-sha256', required=True, help='independently pinned release manifest hash')
    parser.add_argument('--work', required=True, help='new ignored directory or a directory outside the checkout')
    args = parser.parse_args()
    try:
        passed = check(args.bundle,args.oracle,args.manifest_sha256,args.work)
    except (ValueError, OSError, KeyError, subprocess.CalledProcessError) as error:
        raise SystemExit(str(error)) from error
    raise SystemExit(0 if passed else 1)


if __name__ == '__main__':
    main()
