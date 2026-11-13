import hashlib, json, subprocess, tempfile, unittest
from pathlib import Path
from unittest import mock
import implementation_lane as impl
import integration_lane
import provisional_lane as provisional
import review_lane as review

class ProvisionalTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory(); self.root=Path(self.tmp.name); self.repo=self.root/'repo'; self.repo.mkdir()
        self.git('init','-q'); self.git('config','user.email','test@example.com'); self.git('config','user.name','Test Author')
        (self.repo/'README').write_text('base'); self.git('add','README'); self.git('commit','-qm','base'); self.base=self.git('rev-parse','HEAD')
        self.pool=self.root/'pool'; self.pool.mkdir(); self.lane=self.root/'lane'; self.lane.mkdir(); self.job='a'*20; self.revision='b'*40
        self.queue=self.root/'implementation-queue.json'; review.save(self.queue,{'generator_commit':self.revision,'cards':[]})
        review.save(self.root/'config.json',{'lane_root':str(self.lane),'implementation_queue':str(self.queue),'publisher':{'repo':str(self.repo),'worktree_root':str(self.pool)}})
        packet=self.root/'implementation/i1/state/cards/a';packet.mkdir(parents=True)
        paths={'content/mtg/cards/a.json':'{"name":"A"}\n','crates/cantrip-cards/src/cards/a.rs':'pub static CARD: () = ();\n','scenarios/a-case.cantrip':'[Actions]\n'}
        for name,text in paths.items():
            path=self.repo/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(text)
        self.git('add','-A');(packet/'work.patch').write_text(self.git('diff','--cached','--binary')+'\n');self.git('reset','--hard','-q');self.git('clean','-fdq')
        (packet/'content.json').write_text(paths['content/mtg/cards/a.json']);(packet/'reference.md').write_text('Oracle evidence')
        review.save(packet/'report.json',{'decision':'needs_review','reason':'scenario incomplete'})
        review.save(packet/'generator-row.json',{'revision':self.revision,'row':{'name':'A','rust':paths['crates/cantrip-cards/src/cards/a.rs'],'gap':None}})
        card={'name':'A','issue':123,'source_base':self.base}
        with impl.connect(self.root) as db:
            db.execute('INSERT INTO implementation_jobs(id,name,revision,card,queue_meta,status,enqueued,worker,finished,result) VALUES(?,?,?,?,?,?,?,?,?,?)',(self.job,'A',self.revision,json.dumps(card),json.dumps({'cantrip_commit':self.base}),'needs_review','2026-11-13T10:00:00+00:00','i1','2026-11-13T10:10:00+00:00',json.dumps({'card':card})))
    def tearDown(self): self.tmp.cleanup()
    def git(self,*args): return subprocess.check_output(['git','-C',str(self.repo),*args],text=True).strip()
    def test_handoff_recovery_deduplicates_after_enqueue_failure(self):
        with mock.patch.object(provisional,'compile_candidate', side_effect=lambda root,tree,folder,name,cfg: (folder/'compile.log').write_text('ok')) as compile_mock, mock.patch.object(review,'enqueue',side_effect=RuntimeError('interrupted')):
            with self.assertRaisesRegex(RuntimeError,'interrupted'): provisional.handoff(self.root,self.job)
        self.assertEqual(compile_mock.call_count,1);source=provisional.handoff(self.root,self.job);self.assertTrue(source['provisional'])
        self.assertTrue(self.git('show','-s','--format=%s',source['commit']).startswith('Review WIP:'));provisional.handoff(self.root,self.job)
        with review.connect(self.root) as db: rows=db.execute('SELECT status,candidate FROM jobs').fetchall()
        self.assertEqual(len(rows),1);self.assertEqual(rows[0]['status'],'queued');self.assertIsNone(rows[0]['candidate'])
    def test_rejects_wrong_outcome_and_changed_generated_content(self):
        with impl.connect(self.root) as db: db.execute("UPDATE implementation_jobs SET status='generator_bug' WHERE id=?",(self.job,))
        with self.assertRaisesRegex(ValueError,'needs_review'): provisional.handoff(self.root,self.job)
        with impl.connect(self.root) as db: db.execute("UPDATE implementation_jobs SET status='needs_review' WHERE id=?",(self.job,))
        (self.root/'implementation/i1/state/cards/a/content.json').write_text('different')
        with mock.patch.object(provisional,'compile_candidate') as compile_mock:
            with self.assertRaisesRegex(ValueError,'differs'): provisional.handoff(self.root,self.job)
        compile_mock.assert_not_called()
    def test_unmodified_provisional_requires_confirmation_and_wip_is_not_collected(self):
        self.assertTrue(review.needs_confirmation({'provisional':True}, []))
        self.assertFalse(review.needs_confirmation({}, []))
        with mock.patch.object(provisional,'compile_candidate', side_effect=lambda root,tree,folder,name,cfg: (folder/'compile.log').write_text('ok')):
            source=provisional.handoff(self.root,self.job)
        with review.connect(self.root) as db:
            db.execute("UPDATE jobs SET status='passed',candidate=?",(source['commit'],))
        cfg=review.config(self.root);cfg['workflow']='consolidate_then_publish';cfg['integration']={'tree':str(self.repo)};review.save(self.root/'config.json',cfg)
        with self.assertRaisesRegex(RuntimeError,'Provisional work'):
            integration_lane.sync(self.root)
    def test_reported_generator_disagreement_is_not_promoted(self):
        packet=self.root/'implementation/i1/state/cards/a'
        review.save(packet/'report.json',{'decision':'needs_review','validation':[{'agrees':False}]})
        with self.assertRaisesRegex(ValueError,'code disagreement'):
            provisional.handoff(self.root,self.job)
    def test_recovers_after_ref_before_receipt(self):
        save=review.save
        def interrupted(path, data):
            if Path(path).name == 'source.json': raise RuntimeError('interrupted after ref')
            return save(path, data)
        with mock.patch.object(provisional,'compile_candidate',side_effect=lambda root,tree,folder,name,cfg: (folder/'compile.log').write_text('ok')), \
             mock.patch.object(review,'save',side_effect=interrupted):
            with self.assertRaisesRegex(RuntimeError,'interrupted after ref'):
                provisional.handoff(self.root,self.job)
        with mock.patch.object(provisional,'compile_candidate') as compile_mock:
            source=provisional.handoff(self.root,self.job)
        compile_mock.assert_not_called()
        self.assertTrue(source['provisional'])
        self.assertFalse((self.pool/('provisional-'+self.job)).exists())
    def test_legacy_rows_remain_bound_after_live_rows_change(self):
        packet=self.root/'implementation/i1/state/cards/a'; bound=review.read(packet/'generator-row.json');(packet/'generator-row.json').unlink()
        rows=self.lane/'gen/rows.jsonl';rows.parent.mkdir();data=(json.dumps(bound['row'])+'\n').encode();rows.write_bytes(data)
        provisional.freeze_rows(self.root,self.revision,hashlib.sha256(data).hexdigest())
        rows.write_text('{}\n');review.save(self.queue,{'generator_commit':'c'*40,'cards':[]})
        with mock.patch.object(provisional,'compile_candidate',side_effect=lambda root,tree,folder,name,cfg: (folder/'compile.log').write_text('ok')):
            source=provisional.handoff(self.root,self.job)
        self.assertTrue(source['provisional'])
        self.assertEqual(review.read(self.root/'provisional'/self.job/'packet/generator-row.json')['revision'],self.revision)
if __name__=='__main__': unittest.main()
