#!/usr/bin/env python3
"""Exercise manifest/tree preflight on temporary Git fixtures; no Cargo commands."""
import pathlib
import fcntl
import runpy
import subprocess
import tempfile

validate = runpy.run_path(str(pathlib.Path(__file__).with_name('validate.py')), run_name='validator_import')
with tempfile.TemporaryDirectory(prefix='export-validation-selfcheck-') as directory:
    repo = pathlib.Path(directory)
    def git(*args):
        return subprocess.check_output(['git', *args], cwd=repo, text=True, stderr=subprocess.DEVNULL).strip()
    git('init'); git('config', 'user.name', 'Fixture'); git('config', 'user.email', 'fixture@example.org')
    (repo / 'base').write_text('base'); git('add', '.'); git('commit', '-m', 'base'); base = git('rev-parse', 'HEAD')
    (repo / 'engine').write_text('foundation'); git('add', '.'); git('commit', '-m', 'foundation'); foundation = git('rev-parse', 'HEAD')
    (repo / 'card').write_text('card'); git('add', '.'); git('commit', '-m', 'card'); head = git('rev-parse', 'HEAD')
    def obj(commit, path):
        meta, actual = git('ls-tree', commit, path).split('\t')
        mode, kind, oid = meta.split(); assert actual == path
        return {'mode': mode, 'kind': kind, 'oid': oid}
    plan = {'base':base, 'head':head, 'blockers':[], 'foundation_paths':['engine'],
            'cards':[{'name':'Fixture Card', 'paths':['card']}], 'groups':[[0]],
            'changes':{'engine':obj(foundation, 'engine'), 'card':obj(head, 'card')}}
    plan['digest'] = validate['digest'](plan)
    receipt = {'source':head, 'plan_digest':plan['digest'], 'foundation':foundation,
               'groups':[{'head':head, 'base':foundation, 'cards':['Fixture Card']}]}
    assert validate['verify_plan'](repo, plan, receipt, base)[0] == foundation
    try:
        validate['verify_plan'](repo, plan, receipt, head)
    except ValueError:
        pass
    else:
        raise AssertionError('wrong publication base was accepted')
    receipt['groups'][0]['cards'] = ['Other']
    try:
        validate['verify_plan'](repo, plan, receipt, base)
    except ValueError:
        pass
    else:
        raise AssertionError('wrong group card was accepted')
    receipt['groups'][0]['cards'] = ['Fixture Card']
    (repo / 'missing').write_text('unexported source'); git('add', '.'); git('commit', '-m', 'unexported')
    plan['head'] = git('rev-parse', 'HEAD'); plan.pop('digest'); plan['digest'] = validate['digest'](plan)
    receipt['source'] = plan['head']; receipt['plan_digest'] = plan['digest']
    try:
        validate['verify_plan'](repo, plan, receipt, base)
    except ValueError as e:
        assert 'union' in str(e)
    else:
        raise AssertionError('missing source file was accepted')
with tempfile.TemporaryDirectory(prefix='export-series-selfcheck-') as directory:
    repo = pathlib.Path(directory)
    def git(*args):
        return subprocess.check_output(['git', *args], cwd=repo, text=True, stderr=subprocess.DEVNULL).strip()
    git('init'); git('config', 'user.name', 'Fixture'); git('config', 'user.email', 'fixture@example.org')
    (repo/'baseline').write_text('base'); git('add','.'); git('commit','-m','base'); base=git('rev-parse','HEAD')
    series=[]
    for name in ['api','client']:
        (repo/name).write_text(name); git('add','.'); git('commit','-m',name); series.append(git('rev-parse','HEAD'))
    foundation=series[-1]
    (repo/'card').write_text('card'); git('add','.'); git('commit','-m','card'); head=git('rev-parse','HEAD')
    changes={name:validate['entry'](repo,head,name) for name in ['api','client','card']}
    plan={'base':base,'head':head,'blockers':[],'foundation_paths':['api','client'],
          'cards':[{'name':'Card','paths':['card']}],'groups':[[0]],'changes':changes}
    plan['digest']=validate['digest'](plan)
    receipt={'source':head,'plan_digest':plan['digest'],'foundation':foundation,'foundation_commits':series,
             'groups':[{'head':head,'base':foundation,'cards':['Card']}]}
    assert validate['verify_plan'](repo,plan,receipt,base)[0]==foundation
    for bad in [series[::-1],series[1:]]:
        invalid=dict(receipt,foundation_commits=bad)
        try: validate['verify_plan'](repo,plan,invalid,base)
        except ValueError: pass
        else: raise AssertionError('wrong ordered foundation history accepted')
with tempfile.TemporaryDirectory(prefix='export-cache-selfcheck-') as directory:
    root = pathlib.Path(directory); seed = root / 'seed' / 'checked'; seed.mkdir(parents=True)
    (seed / '.cargo-lock').touch(); (seed / 'artifact').write_text('cached')
    target = root / 'private'; target.mkdir()
    receipt = validate['seed_cache'](seed, target, [target.resolve()])
    assert (target / 'checked' / 'artifact').read_text() == 'cached'
    (target / 'checked' / 'artifact').write_text('changed')
    assert (seed / 'artifact').read_text() == 'cached'
    assert receipt['path'] == str(seed.resolve())
    try:
        validate['seed_cache'](seed, root / 'other', [seed.resolve()])
    except ValueError: pass
    else: raise AssertionError('overlapping seed accepted')
    with (seed / '.cargo-lock').open('rb') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        try: validate['seed_cache'](seed, root / 'other', [])
        except ValueError as e: assert 'locked' in str(e)
        else: raise AssertionError('active cache accepted')
    (seed / 'link').symlink_to(seed / 'artifact')
    try: validate['seed_cache'](seed, root / 'other', [])
    except ValueError as e: assert 'unsafe' in str(e)
    else: raise AssertionError('cache symlink accepted')
print('Synthetic preflight: valid export accepted; wrong base, group and missing source file rejected. No build run.')
with tempfile.TemporaryDirectory(prefix='export-equivalent-native-') as directory:
    import json
    repo=pathlib.Path(directory)
    def git(*args):
        return subprocess.check_output(['git',*args],cwd=repo,text=True,stderr=subprocess.DEVNULL).strip()
    git('init');git('config','user.name','Fixture');git('config','user.email','fixture@example.org')
    (repo/'crates').mkdir();(repo/'crates/input.rs').write_text('native input');git('add','.');git('commit','-m','native')
    original=git('rev-parse','HEAD');log=repo/'.git/gate.log';log.write_text('synthetic success fixture')
    fingerprint=validate['native_inputs'](repo,original)
    full={'head':original,'head_after':original,'exit_code':0,'dirty_after':'','native_inputs_sha256':fingerprint,
          'gate_kind':'full_native_tests','command':['cargo','test'],'log':str(log),'log_sha256':validate['sha'](log)}
    (repo/'docs').mkdir();(repo/'docs/note.md').write_text('documentation');git('add','.');git('commit','-m','docs');doc=git('rev-parse','HEAD')
    reused={**full,'head':doc,'head_after':doc,'gate_kind':'compile_with_equivalent_native_tests','command':['cargo','check'],
            'equivalent_native_tests':{'head':original,'log_sha256':full['log_sha256']}}
    validate['verify_native_evidence'](repo,[full,reused])
    compile_only={**reused,'gate_kind':'compile_unchanged_native','unchanged_native_parent':original}
    compile_only.pop('equivalent_native_tests')
    validate['verify_native_evidence'](repo,[compile_only])
    (repo/'crates/input.rs').write_text('changed native');git('add','.');git('commit','-m','new native');changed=git('rev-parse','HEAD')
    invalid={**reused,'head':changed,'head_after':changed,'native_inputs_sha256':validate['native_inputs'](repo,changed)}
    try:validate['verify_native_evidence'](repo,[full,invalid])
    except ValueError:pass
    else:raise AssertionError('reused tests after native input changed')
    log.write_text('tampered')
    try:validate['verify_native_evidence'](repo,[full,reused])
    except ValueError:pass
    else:raise AssertionError('reused invalid prior test log')
print('Equivalent-input preflight: documentation-only reuse accepted; changed native input and tampered evidence rejected.')
