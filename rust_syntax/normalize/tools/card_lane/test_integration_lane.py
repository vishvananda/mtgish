import json
from pathlib import Path
import subprocess
import tempfile
import unittest

import integration_lane as lane
import publish_lane
import shared_base


class IntegrationTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.root=Path(self.tmp.name);self.repo=self.root/'tree';self.repo.mkdir()
        self.git('init','-q');self.git('config','user.name','Integrator');self.git('config','user.email','integrator@example.com')
        (self.repo/'helper').write_text('base');self.git('add','.');self.git('commit','-qm','base')
        self.base=self.git('rev-parse','HEAD')
        self.cfg=dict(workflow='consolidate_then_publish',base=self.base,publisher={},integration=dict(
            enabled=True,tree=str(self.repo),branch='integration/local',base=self.base))
        (self.root/'config.json').write_text(json.dumps(self.cfg));(self.root/'integration').mkdir()

    def tearDown(self):self.tmp.cleanup()

    def git(self,*args):return subprocess.check_output(['git',*args],cwd=self.repo,text=True,stderr=subprocess.STDOUT).strip()

    def candidate(self,name,issue,helper='shared'):
        self.git('checkout','-q','--detach',self.base)
        (self.repo/'helper').write_text(helper);self.git('add','.');self.git('commit','-qm','Mechanics: helper')
        folder=self.repo/'content/mtg/cards';folder.mkdir(parents=True,exist_ok=True)
        (folder/(name.lower()+'.json')).write_text(json.dumps(dict(name=name)))
        (self.repo/(name+'.rs')).write_text(name);self.git('add','.')
        self.git('-c','user.name=Original Author','commit','-qm',f'Card: {name}\n\nCloses #{issue}')
        sha=self.git('rev-parse','HEAD')
        with lane.connect(self.root) as db:
            db.execute("INSERT INTO jobs(id,name,source_commit,source,status,enqueued,candidate) VALUES(?,?,?,?,?,?,?)",
                       (name,name,sha,json.dumps(dict(name=name,issue=issue)), 'passed','now',sha));db.commit()
        return sha

    def start(self):self.git('checkout','-q','-b','integration/local',self.base)

    def test_integrates_approved_work_once_and_reuses_already_present_helpers(self):
        self.candidate('Alpha',1);self.candidate('Beta',2);self.start()
        self.assertEqual(lane.sync(self.root),2)
        self.assertIn('Alpha',lane.integrate_one(self.root));self.assertIn('Beta',lane.integrate_one(self.root))
        self.assertIsNone(lane.integrate_one(self.root));self.assertEqual(lane.sync(self.root),0)
        subjects=self.git('log','--format=%s',self.base+'..HEAD').splitlines()
        self.assertEqual(len([s for s in subjects if s.startswith('Mechanics:')]),1)
        self.assertEqual(self.git('log','-1','--format=%an'),'Original Author')
        self.assertEqual(lane.snapshot(self.root)['counts'],dict(integrated=2))
        with lane.connect(self.root) as db:
            imported = dict(db.execute("SELECT * FROM integration_jobs WHERE name='Alpha'").fetchone())
        approval = json.loads((self.root/'integration/jobs'/imported['id']/'approval.json').read_text())
        self.assertEqual((approval['name'], approval['status'], approval['candidate']),
                         ('Alpha', 'passed', imported['candidate']))

    def test_conflict_is_retained_and_explicit_resolution_resumes_without_replaying(self):
        self.candidate('Alpha',1,'first');self.candidate('Beta',2,'second');self.start();lane.sync(self.root)
        lane.integrate_one(self.root)
        with self.assertRaises(RuntimeError):lane.integrate_one(self.root)
        state=lane.snapshot(self.root);self.assertEqual(state['held'][0]['name'],'Beta')
        self.assertIn('UU helper',self.git('status','--porcelain'))
        with self.assertRaises(RuntimeError):lane.integrate_one(self.root)
        (self.repo/'helper').write_text('canonical union');self.git('add','helper');self.git('-c','core.editor=true','cherry-pick','--continue')
        key=state['held'][0]['id'];lane.resume(self.root,key,'Both approved helper intents consolidated here')
        lane.integrate_one(self.root)
        self.assertTrue((self.repo/'Beta.rs').exists())
        self.assertTrue((self.root/'integration/jobs'/key/'resolution-000.json').exists())
        self.assertEqual(lane.snapshot(self.root)['counts'],dict(integrated=2))

    def test_assembly_collects_callers_without_choosing_between_shared_implementations(self):
        self.candidate('Alpha',1,'first');self.candidate('Beta',2,'second');self.start();lane.sync(self.root)
        lane.integrate_one(self.root,defer_prerequisites=True)
        lane.integrate_one(self.root,defer_prerequisites=True)
        self.assertEqual((self.repo/'helper').read_text(),'base')
        self.assertTrue((self.repo/'Alpha.rs').exists());self.assertTrue((self.repo/'Beta.rs').exists())
        self.assertEqual(lane.snapshot(self.root)['counts'],dict(assembled=2))
        self.assertEqual(lane.dependency_inventory(self.root),2)
        self.assertNotIn('local_base_manifest',json.loads((self.root/'config.json').read_text()))

    def test_existing_upstream_introduction_keeps_upstream_implementation(self):
        self.candidate('Alpha',1);self.git('checkout','-q','--detach',self.base)
        folder=self.repo/'content/mtg/cards';folder.mkdir(parents=True,exist_ok=True)
        (folder/'alpha.json').write_text(json.dumps(dict(name='Alpha')));(self.repo/'Alpha.rs').write_text('upstream canonical')
        self.git('add','.');self.git('commit','-qm','Card: Alpha upstream');upstream=self.git('rev-parse','HEAD')
        self.cfg['integration']['base']=upstream;(self.root/'config.json').write_text(json.dumps(self.cfg))
        self.git('checkout','-q','-b','integration/local',upstream);lane.sync(self.root)
        self.assertIn('already upstream',lane.integrate_one(self.root))
        self.assertEqual((self.repo/'Alpha.rs').read_text(),'upstream canonical')
        self.assertEqual(lane.snapshot(self.root)['counts'],dict(upstream=1))

    def test_snapshot_accepts_local_card_history_but_freezes_validated_head(self):
        self.candidate('Alpha',1);self.start();lane.sync(self.root);lane.integrate_one(self.root)
        head=self.git('rev-parse','HEAD');path=lane.promote_snapshot(self.root,head,dict(ok=True,head=head))
        cfg=json.loads((self.root/'config.json').read_text());pin=shared_base.read_work_base(cfg,self.repo)
        self.assertEqual(pin['head'],head)
        self.assertEqual(shared_base.metadata(pin),dict(publication_base=head,local_integration_base=head,shared_dependencies=[]))
        with self.assertRaises(ValueError):lane.promote_snapshot(self.root,head,dict(ok=False,head=head))
        self.assertEqual(json.loads(path.read_text())['head'],head)

    def test_local_mode_cannot_reserve_or_publish_legacy_batches(self):
        with lane.connect(self.root) as db:
            self.assertIsNone(publish_lane.reserve(db,self.cfg))
            with self.assertRaisesRegex(RuntimeError,'Publication deferred'):
                publish_lane.publish(self.root,self.cfg,db,1)
