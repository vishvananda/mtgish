"""The comparison population must be tied to the original immutable source."""
import copy
import gzip
import json
from pathlib import Path
import tempfile
import subprocess
import sys
import unittest

import oracle_export as gate


class ExportGateTests(unittest.TestCase):
    def records(self):
        return [{'oracle_id':'one','name':'Same','layout':'normal','type_line':'Creature'},
                {'oracle_id':'two','name':'Pair','layout':'split','card_faces':[
                    {'name':'Same','type_line':'Instant'}, {'name':'Other','type_line':'Instant'}]}]

    def outcomes(self):
        rows=[]
        for i,record in enumerate(self.records()):
            for j,source in enumerate(record.get('card_faces',[record])):
                rows.append({'format_version':1,'schema_version':190,
                             'identity':{'provider_id':record['oracle_id'],'record_index':i,'face_index':j,
                                         'layout':record['layout'],'record_name':record['name']},
                             'name':source['name'],'source':source,'status':'unique','semantic_readings':1,
                             'readings':[{'card':{'name':source['name']}}]})
        return rows

    def test_same_face_names_on_different_records_keep_parent_identity(self):
        requests=gate.requests_for(self.records(),self.outcomes())
        self.assertEqual(requests[0]['name'],requests[1]['name'])
        self.assertNotEqual(requests[0]['record_names'],requests[1]['record_names'])
        self.assertNotEqual(requests[0]['identity'],requests[1]['identity'])

    def test_missing_duplicate_and_changed_source_occurrences_are_rejected(self):
        rows=self.outcomes()
        changed=copy.deepcopy(rows); changed[0]['source']['oracle_text']='Different source'
        wrong_parent=copy.deepcopy(rows); wrong_parent[0]['identity']['record_name']='Other record'
        for bad in (rows[:-1],rows+[rows[0]],changed,wrong_parent):
            with self.assertRaises(ValueError): gate.requests_for(self.records(),bad)

    def test_no_first_reading_or_partial_success_from_limits(self):
        rows=self.outcomes(); rows[0].update(status='ambiguous',semantic_readings=2)
        rows[0]['readings'].append({'card':{'name':'Different meaning'}})
        self.assertIsNone(gate.requests_for(self.records(),rows)[0]['expected'])
        rows[0]['status']='limit'
        with self.assertRaisesRegex(ValueError,'partial success'): gate.requests_for(self.records(),rows)
        rows[0].update(readings=[],status='refused')
        self.assertIsNone(gate.requests_for(self.records(),rows)[0]['expected'])

    def test_population_selection_is_explicit_and_complete(self):
        records=self.records()
        self.assertEqual(gate.selected_records(records,{'kind':'oracle_ids','policy':'test','oracle_ids':['two']}),records[1:])
        for population in ({}, {'kind':'oracle_ids','policy':'test','oracle_ids':['missing']},
                           {'kind':'oracle_ids','policy':'test','oracle_ids':['one','one']}):
            with self.assertRaises(ValueError): gate.selected_records(records,population)

    def test_unsupported_source_layout_does_not_corrupt_preprocessed_json(self):
        values=[{'name':'Synthetic known', 'layout':'normal', 'type_line':'Instant',
                 'mana_cost':'{U}', 'oracle_text':'Draw a card.'},
                {'name':'Synthetic future layout', 'layout':'future_layout', 'type_line':'Instant'}]
        with tempfile.TemporaryDirectory() as directory:
            source=Path(directory)/'oracle.json'; gate.write(source,values)
            result=subprocess.run([sys.executable,str(gate.ROOT/'preprocess_scryfall'),str(source)],
                                  capture_output=True,text=True,check=True)
        self.assertEqual([r['name'] for r in json.loads(result.stdout)],['Synthetic known'])
        self.assertIn('future_layout Synthetic future layout',result.stderr)

    def test_original_gzip_json_lines_and_legacy_arrays(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'oracle.jsonl.gz'
            with gzip.open(path,'wt') as stream:
                for row in self.records(): stream.write(json.dumps(row)+'\n')
            self.assertEqual(gate.original_records(path,{'encoding':'jsonl-gzip'}),self.records())
            path=Path(directory)/'oracle.json'; gate.write(path,self.records())
            self.assertEqual(gate.original_records(path,{}),self.records())
            with self.assertRaisesRegex(ValueError,'encoding'):
                gate.original_records(path,{'encoding':'unknown'})

    def test_pinned_manifest_and_asset_checksums_are_required(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            for name in ('faces.jsonl.gz','outcomes.jsonl.gz'):
                with gzip.open(root/name,'wt') as out: out.write('{}\n')
            (root/'coverage.json').write_text('{}')
            assets={p.name:{'sha256':gate.digest(p),'bytes':p.stat().st_size} for p in root.iterdir()}
            manifest={'format_version':1,'kind':'oracle-grammar-native-faces','assets':assets}
            gate.write(root/'manifest.json',manifest)
            pinned=gate.digest(root/'manifest.json')
            (root/'SHA256SUMS').write_text(''.join(f'{gate.digest(p)}  {p.name}\n' for p in sorted(root.iterdir())))
            self.assertEqual(gate.verify_bundle(root,pinned),manifest)
            with self.assertRaisesRegex(ValueError,'manifest checksum'): gate.verify_bundle(root,'0'*64)
            (root/'faces.jsonl.gz').write_bytes(b'changed')
            with self.assertRaisesRegex(ValueError,'asset checksum'): gate.verify_bundle(root,pinned)


if __name__=='__main__': unittest.main()
