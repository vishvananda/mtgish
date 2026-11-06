#!/usr/bin/env python3
"""Verify a complete set of shard receipts for one exact exported plan, without building."""
import argparse, json
from pathlib import Path
import validate as v

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--repo', type=Path, required=True)
p.add_argument('--plan', type=Path, required=True)
p.add_argument('--export', type=Path, required=True)
p.add_argument('--expected-base', required=True)
p.add_argument('--foundation-receipt', type=Path, required=True)
p.add_argument('--shard-receipts', type=Path, nargs='+', required=True)
a = p.parse_args()
plan=json.loads(a.plan.read_text()); export=json.loads(a.export.read_text())
foundation, groups=v.verify_plan(a.repo.resolve(),plan,export,a.expected_base)
foundation_sha=v.sha(a.foundation_receipt); first=json.loads(a.foundation_receipt.read_text())
expected={'status':'passed','mode':'foundation','plan_sha256':v.sha(a.plan),'export_sha256':v.sha(a.export), 'foundation':foundation,'expected_base':a.expected_base,'validator_sha256':v.sha(Path(v.__file__))}
if any(first.get(k)!=value for k,value in expected.items()) or len(first.get('records',[]))!=len(export.get('foundation_commits', [foundation])):
    raise SystemExit('invalid foundation receipt')
def check(record,label,head):
    if (record.get('label')!=label or record.get('head')!=head or record.get('head_after')!=head or record.get('exit_code')!=0 or record.get('dirty_after') or v.sha(Path(record['log']))!=record.get('log_sha256')):
        raise SystemExit(f'invalid test record {label}')
for number, (head, record) in enumerate(zip(export.get('foundation_commits', [foundation]), first['records']), 1):
    check(record, 'foundation' if head == foundation else f'foundation-{number:02d}', head)
v.verify_native_evidence(a.repo.resolve(), first['records'])
seen=set(); count=len(a.shard_receipts); heads=[]
for path in a.shard_receipts:
    x=json.loads(path.read_text())
    expected_shard={**expected,'mode':'shard','foundation_receipt_sha256':foundation_sha,'shard_count':count}
    if any(x.get(k)!=value for k,value in expected_shard.items()):
        raise SystemExit(f'stale or invalid shard {path}')
    index=x.get('shard_index')
    if not isinstance(index,int) or not 0<=index<count or index in seen: raise SystemExit('duplicate or invalid shard index')
    seen.add(index)
    selected=[(i,g) for i,g in enumerate(groups,1) if (i-1)%count==index]
    if len(x['records'])!=len(selected): raise SystemExit('incomplete shard')
    for record,(i,g) in zip(x['records'],selected):
        check(record,f'group-{i:03d}',g['head']);heads.append(g['head'])
        if record.get('gate_kind') != 'full_native_tests':
            raise SystemExit('each sibling requires a complete native test run')
    v.verify_native_evidence(a.repo.resolve(), x['records'])
if seen!=set(range(count)) or len(heads)!=len(groups): raise SystemExit('missing groups')
print(json.dumps({'status':'passed','foundation':foundation,'foundation_receipt_sha256':foundation_sha,'plan_sha256':v.sha(a.plan),'export_sha256':v.sha(a.export),'validator_sha256':v.sha(Path(v.__file__)),'shards':{str(p):v.sha(p) for p in a.shard_receipts},'group_count':len(heads)},indent=2))
