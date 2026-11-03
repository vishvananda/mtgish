import concurrent.futures
import json
import os
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

import review_lane as lane


class QueueTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def test_explicit_retry_preserves_work_and_round_evidence(self):
        lane.save(self.root / "config.json", {"max_verifier_rejections": 3})
        with lane.connect(self.root) as db:
            lane.enqueue(db, {"name": "A", "commit": "author", "review_round": 2,
                              "verifier_rejections": 1})
            job = dict(db.execute("SELECT * FROM jobs").fetchone())
            previous = {"prepared_parent": "parent", "wip_commit": "saved-fixes",
                        "report": {"decision": "blocked", "reason": "SDK unavailable"}}
            db.execute("UPDATE jobs SET status='blocked',branch='preserved',result=?,candidate='old'",
                       (json.dumps(previous),))
            db.commit()
        old = lane.job_folder(self.root, job, "review") / "review.json"
        lane.save(old, previous["report"])
        lane.retry_stopped(self.root, job["id"], "Existing prompt needs no new client capability")
        with lane.connect(self.root) as db:
            retried = dict(db.execute("SELECT * FROM jobs").fetchone())
        source, result = json.loads(retried["source"]), json.loads(retried["result"])
        self.assertEqual((retried["status"], retried["stage"], retried["candidate"]), ("queued", "review", None))
        self.assertEqual((source["review_round"], source["verifier_rejections"]), (3, 1))
        self.assertEqual(result, {"retry_requested": True, "prepared_parent": "parent", "wip_commit": "saved-fixes"})
        self.assertEqual(lane.read(old), previous["report"])
        self.assertNotEqual(old.parent, lane.job_folder(self.root, retried, "review"))
        archive = lane.read(self.root / "jobs" / job["id"] / "retry-3.json")
        self.assertEqual(json.loads(archive["job"]["result"]), previous)
        with self.assertRaisesRegex(ValueError, "only stopped"):
            lane.retry_stopped(self.root, job["id"], "Cannot enqueue it twice")

    def test_explicit_retry_does_not_bypass_rejection_cap_or_active_lease(self):
        lane.save(self.root / "config.json", {"max_verifier_rejections": 3})
        with lane.connect(self.root) as db:
            for name, status, stage, rejects in [("cap", "blocked", "review", 3),
                                                 ("active", "running", "review", 0),
                                                 ("confirm", "blocked", "confirm", 0)]:
                lane.enqueue(db, {"name": name, "commit": name, "verifier_rejections": rejects})
                db.execute("UPDATE jobs SET status=?,stage=?,branch='preserved',result=? WHERE name=?",
                           (status, stage, json.dumps({"prepared_parent": "parent"}), name))
            db.commit()
            jobs = [dict(r) for r in db.execute("SELECT * FROM jobs")]
        for job in jobs:
            with self.subTest(job=job["name"]), self.assertRaises(ValueError):
                lane.retry_stopped(self.root, job["id"], "Changed blocker")
        with lane.connect(self.root) as db:
            self.assertEqual([dict(r) for r in db.execute("SELECT * FROM jobs")], jobs)

    def test_verifier_limit_counts_rejections_instead_of_artifact_rounds(self):
        lane.save(self.root / "config.json", {"max_verifier_rejections": 3})
        with lane.connect(self.root) as db:
            lane.enqueue(db, {"name": "A", "commit": "author", "review_round": 5,
                              "verifier_rejections": 1})
            db.execute("UPDATE jobs SET stage='confirm',status='blocked',candidate='candidate'")
            db.commit()
        for rejected in (2, 3):
            with lane.connect(self.root) as db:
                job = dict(db.execute("SELECT * FROM jobs").fetchone())
            review = lane.job_folder(self.root, job, "review")
            lane.save(review / "checks.json", {"head": "base"})
            lane.save(review / "result.json", {"prepared_parent": "parent"})
            lane.save(lane.job_folder(self.root, job, "confirm") / "review.json",
                      {"decision": "blocked", "findings": [{"what": "Concrete defect"}]})
            self.assertEqual(lane.handoff_verifier_findings(self.root), int(rejected < 3))
            with lane.connect(self.root) as db:
                row = db.execute("SELECT * FROM jobs").fetchone()
                self.assertEqual(json.loads(row["source"])["verifier_rejections"], rejected)
                if rejected == 2:
                    db.execute("UPDATE jobs SET stage='confirm',status='blocked'")
                    db.commit()
        self.assertEqual(lane.handoff_verifier_findings(self.root), 0)
        with lane.connect(self.root) as db:
            row = db.execute("SELECT * FROM jobs").fetchone()
            self.assertEqual((row["status"], json.loads(row["source"])["verifier_rejections"]), ("blocked", 3))

    def test_concurrent_claims_are_exactly_once(self):
        with lane.connect(self.root) as db:
            for i in range(60):
                lane.enqueue(db, {"name": f"Card {i}", "commit": str(i)})

        def consume(worker):
            claimed = []
            with lane.connect(self.root) as db:
                while job := lane.claim(db, str(worker)):
                    claimed.append(job["id"])
            return claimed

        with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
            ids = [item for group in pool.map(consume, range(8)) for item in group]
        self.assertEqual(len(ids), 60)
        self.assertEqual(len(set(ids)), 60)
        with lane.connect(self.root) as db:
            self.assertEqual(db.execute("SELECT count(*) FROM events WHERE kind='claimed'").fetchone()[0], 60)

    def test_review_and_confirmation_pools_claim_only_their_role(self):
        with lane.connect(self.root) as db:
            lane.enqueue(db, {'name':'Original','commit':'one'})
            lane.enqueue(db, {'name':'Fix','commit':'two'},priority=100)
            db.execute("UPDATE jobs SET stage='confirm' WHERE name='Fix'")
            db.commit()
            self.assertEqual(lane.claim(db,'r1','review')['name'],'Original')
            self.assertIsNone(lane.claim(db,'r2','review'))
            self.assertEqual(lane.claim(db,'verifier','confirm')['name'],'Fix')

    def test_sync_is_idempotent_per_name_and_revision(self):
        state = self.root / "producer"
        state.mkdir()
        manifest = self.root / "manifest.json"
        manifest.write_text(json.dumps({"base": "base", "workers": [{"worker": "p", "tree": "/producer", "state": str(state)}]}))
        lane.save(self.root / "config.json", {"producer_manifests": [str(manifest)]})
        queue = {"cards": [{"name": "A", "status": "committed", "commit": "one"},
                           {"name": "B", "status": "running", "commit": "ignored"}]}
        lane.save(state / "queue.json", queue)
        self.assertEqual(lane.sync(self.root), 1)
        self.assertEqual(lane.sync(self.root), 0)
        queue["cards"][0]["commit"] = "two"
        lane.save(state / "queue.json", queue)
        self.assertEqual(lane.sync(self.root), 1)
        with lane.connect(self.root) as db:
            self.assertEqual({r[0] for r in db.execute("SELECT source_commit FROM jobs")}, {"one", "two"})

    def test_status_names_outcomes_and_counts_cards_once_across_stages(self):
        with lane.connect(self.root) as db:
            lane.enqueue(db, {"name":"Review card", "commit":"one"})
            job=lane.claim(db,"r1")
            job['worker']='r1'
            lane.finish(db,job,'queued',{'branch':'review/example/review','seconds':3},stage='confirm')
            job=lane.claim(db,'r1')
            job['worker']='r1'
            lane.finish(db,job,'passed',{'branch':'review/example/confirm','seconds':2})
        snapshot=lane.status(self.root)
        self.assertEqual(snapshot['worker_progress'],[{'worker':'r1','handled':1}])
        outcomes=[e for e in snapshot['recent'] if e['kind'] in ('passed','queued')]
        self.assertEqual([(e['name'],e['detail']['stage']) for e in outcomes],[('Review card','confirm'),('Review card','review')])

    def test_missing_checks_cannot_pass(self):
        tree = self.root / "tree"
        tree.mkdir()
        folder = self.root / "packet"
        folder.mkdir()
        lane.save(self.root / "config.json", {"reviewers": {"slot": {"tree": str(tree)}},
                  "review_model": "test", "repairs": 0})
        source = {"name": "A", "commit": "one"}
        with lane.connect(self.root) as db:
            lane.enqueue(db, source)
        lane.save(folder / "review.json", {"decision": "pass", "validation": [{"agrees": True}],
                  "affected_cards": ["A"], "client": "covered"})

        class Usage:
            def record(self, model):
                return {}

        fake = SimpleNamespace(environment=lambda: {}, Usage=Usage,
                               Codex=lambda model, effort: SimpleNamespace(model=model),
                               spawn=lambda *args: (0, None, Usage()))
        with mock.patch.object(lane, "clean"), mock.patch.object(lane, "prepare", return_value=("branch", "parent", source)), \
             mock.patch.object(lane, "load_lane", return_value=fake), mock.patch.object(lane, "packet", return_value=folder), \
             mock.patch.object(lane, "prompt", return_value="review"), mock.patch.object(lane, "allowed_changes", return_value=[]), \
             mock.patch.object(lane, "work_digest", return_value="digest"), \
             mock.patch.object(lane, "checks", return_value={"ok": False}) as check, \
             mock.patch.dict(os.environ, {}, clear=False):
            lane.worker(self.root, "slot", once=True)
        check.assert_called_once()
        with lane.connect(self.root) as db:
            self.assertEqual(db.execute("SELECT status FROM jobs").fetchone()[0], "blocked")

    def test_dedicated_verifier_retains_independent_context_between_cards(self):
        import card_lane
        tree=self.root/'tree';tree.mkdir()
        lane.save(self.root/'config.json',{'reviewers':{'verify':{'tree':str(tree),'role':'confirm'}},'review_model':'test','repairs':0})
        with lane.connect(self.root) as db:
            for name in ('A','B'):
                lane.enqueue(db,{'name':name,'commit':name})
            db.execute("UPDATE jobs SET stage='confirm',candidate='candidate'")
        sessions=[]
        def packet(root,tree,job,source,backend):
            folder=self.root/job['id'];folder.mkdir()
            lane.save(folder/'review.json',{'decision':'pass','validation':[{'agrees':True}],
                                           'affected_cards':[source['name']],'client':'unchanged'})
            return folder
        def spawn(backend,card,prompt,session,timeout,log):
            sessions.append(session)
            if len(sessions)==2:
                cfg=lane.config(self.root);cfg['enabled']=False;lane.save(self.root/'config.json',cfg)
            used=card_lane.Usage();used.turns=1
            return 0,'independent-verifier-session',used
        fake=SimpleNamespace(environment=lambda:{},Usage=card_lane.Usage,
                             Codex=lambda model,effort:SimpleNamespace(model=model),spawn=spawn)
        with mock.patch.object(lane,'clean'),mock.patch.object(lane,'git',return_value=''), \
             mock.patch.object(lane,'prepare',side_effect=lambda root,tree,job:('branch','parent',json.loads(job['source']))), \
             mock.patch.object(lane,'load_lane',return_value=fake),mock.patch.object(lane,'packet',side_effect=packet), \
             mock.patch.object(lane,'prompt',return_value='confirm'),mock.patch.object(lane,'allowed_changes',return_value=[]):
            lane.worker(self.root,'verify')
        self.assertEqual(sessions,[None,'independent-verifier-session'])


class GitTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        self.git("init", "-q")
        self.git("config", "user.name", "Reviewer")
        self.git("config", "user.email", "reviewer@example.com")
        self.write("base", "base")
        self.git("add", ".")
        self.git("commit", "-qm", "base")
        self.base = self.git("rev-parse", "HEAD")
        self.write("crates/cantrip-cards/src/cards/test_card.rs", "original")
        self.git("add", ".")
        env = dict(os.environ, GIT_AUTHOR_NAME="Original Author", GIT_AUTHOR_EMAIL="author@example.com", GIT_AUTHOR_DATE="2020-01-02T03:04:05+00:00")
        self.git("commit", "-qm", "Card: Test Card", env=env)
        self.source = self.git("rev-parse", "HEAD")

    def tearDown(self):
        self.tmp.cleanup()

    def git(self, *args, env=None):
        return subprocess.check_output(["git", *args], cwd=self.repo, text=True, env=env).strip()

    def write(self, path, text):
        file = self.repo / path
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_text(text)

    def test_candidate_preserves_author_and_places_shared_commit_first(self):
        self.git("branch", "producer", self.source)
        producer_tree = self.git("rev-parse", "producer^{tree}")
        self.git("checkout", "-qb", "review")
        self.write("crates/cantrip-cards/src/cards/test_card.rs", "fixed")
        self.write("crates/cantrip-mechanics/src/shared.rs", "shared")
        paths = ["crates/cantrip-cards/src/cards/test_card.rs", "crates/cantrip-mechanics/src/shared.rs"]
        candidate = lane.commit_candidate(self.repo, {"name": "Test Card", "commit": self.source, "issue": 42}, self.base, paths)
        self.assertEqual(self.git("rev-parse", "producer"), self.source)
        self.assertEqual(self.git("rev-parse", "producer^{tree}"), producer_tree)
        self.assertEqual(self.git("show", "-s", "--format=%an <%ae>", candidate), "Original Author <author@example.com>")
        self.assertIn("Closes #42", self.git("show", "-s", "--format=%B", candidate))
        self.assertEqual(self.git("rev-list", "--count", f"{self.base}..{candidate}"), "2")
        self.assertEqual(self.git("show", "--format=", "--name-only", candidate + "^"), paths[1])
        self.assertEqual(self.git("show", "--format=", "--name-only", candidate), paths[0])
        self.assertEqual(self.git("status", "--porcelain"), "")

    def test_prepare_rejects_dirty_tree_without_losing_changes(self):
        self.write("untracked", "keep me")
        job = {"id": "1", "stage": "review", "source": json.dumps({"name": "Test Card", "commit": self.source})}
        with self.assertRaisesRegex(RuntimeError, "unfinished work preserved"):
            lane.prepare(self.root, self.repo, job)
        self.assertEqual((self.repo / "untracked").read_text(), "keep me")
        self.assertEqual(self.git("rev-parse", "HEAD"), self.source)

    def test_new_review_freezes_helper_pin_separately_from_producer_history(self):
        self.git('checkout','-q','--detach',self.base)
        self.write('crates/cantrip-mechanics/src/shared.rs','validated helper')
        self.git('add','.');self.git('commit','-qm','Mechanics: shared')
        helper=self.git('rev-parse','HEAD')
        pin=dict(head=helper,commits=[helper],pr_url='https://github.com/org/repo/pull/1')
        lane.save(self.root/'config.json',dict(base=self.base))
        source=dict(name='Test Card',commit=self.source,source_base=self.base)
        with lane.connect(self.root) as db:
            lane.enqueue(db,source)
            job=dict(db.execute('SELECT * FROM jobs').fetchone())
        import shared_base
        with mock.patch.object(shared_base,'read_pin',return_value=pin):
            branch,parent,actual=lane.prepare(self.root,self.repo,job)
        self.assertEqual(parent,helper)
        self.assertEqual(actual['source_base'],self.base)
        self.assertEqual(actual['publication_base'],helper)
        with lane.connect(self.root) as db:
            self.assertEqual(json.loads(db.execute('SELECT source FROM jobs').fetchone()[0]),actual)
        self.assertEqual(self.git('show','HEAD:crates/cantrip-mechanics/src/shared.rs'),'validated helper')

    def test_new_card_boundary_test_stays_with_card_not_helper_prerequisite(self):
        self.git('checkout','-qb','boundary-review')
        test='crates/cantrip-mechanics/tests/test_card_boundary.rs'
        shared='crates/cantrip-mechanics/src/shared.rs'
        self.write(test,'executes Test Card');self.write(shared,'shared helper')
        candidate=lane.commit_candidate(self.repo,dict(name='Test Card',commit=self.source),self.base,[test,shared])
        self.assertEqual(self.git('diff-tree','--no-commit-id','--name-only','-r',candidate+'^'),shared)
        self.assertIn(test,self.git('diff-tree','--no-commit-id','--name-only','-r',candidate).splitlines())

    def test_blocked_edits_are_parked_without_publishing_or_discarding(self):
        self.git("branch", "producer", self.source)
        self.git("checkout", "-qb", "review-blocked")
        path = "crates/cantrip-cards/src/cards/test_card.rs"
        self.write(path, "partial fix to retain")
        folder = self.root / "packet"
        folder.mkdir()
        result = {}
        lane.park_blocked(self.repo, {"name": "Test Card"}, folder, [path], result)
        self.assertEqual(self.git("status", "--porcelain"), "")
        self.assertEqual(self.git("rev-parse", "producer"), self.source)
        self.assertEqual(self.git("show", "HEAD:" + path), "partial fix to retain")
        self.assertIn("not publishable", self.git("show", "-s", "--format=%s", "HEAD"))
        self.assertEqual(result["wip_commit"], self.git("rev-parse", "HEAD"))
        self.assertIn("partial fix to retain", (folder / "blocked.patch").read_text())

    def test_explicit_tooling_retry_reuses_parked_work_and_original_parent(self):
        lane.save(self.root / 'config.json', {})
        self.git('branch','parked',self.source)
        self.git('checkout','parked')
        path='crates/cantrip-cards/src/cards/test_card.rs'
        self.write(path,'retained fix')
        self.git('add',path)
        self.git('commit','-qm','Review WIP')
        wip=self.git('rev-parse','HEAD')
        self.git('checkout','-qb','another-job')
        source={'name':'Test Card','commit':self.source}
        job={'id':'one','stage':'review','branch':'parked','source':json.dumps(source),
             'result':json.dumps({'retry_requested':True,'prepared_parent':self.base,'wip_commit':wip})}
        branch,parent,actual=lane.prepare(self.root,self.repo,job)
        self.assertTrue(branch.startswith('parked-retry-'))
        self.assertEqual((parent,actual),(self.base,source))
        self.assertEqual(self.git('rev-parse','HEAD'),self.source)
        self.assertEqual(self.git('rev-parse','parked'),wip)
        self.assertEqual((self.repo/path).read_text(),'retained fix')
        self.assertEqual(lane.allowed_changes(self.repo,'Test Card'),[path])


    def test_verifier_handoff_materializes_candidate_and_preserves_evidence(self):
        lane.save(self.root / "config.json", {})
        path = "crates/cantrip-cards/src/cards/test_card.rs"
        self.git("branch", "producer", self.source)
        self.git("checkout", "-qb", "old-review")
        self.write(path, "first fix")
        candidate = lane.commit_candidate(self.repo, {"name": "Test Card", "commit": self.source}, self.base, [path])
        self.git("checkout", "-q", "producer")
        with lane.connect(self.root) as db:
            lane.enqueue(db, {"name": "Test Card", "commit": self.source})
            db.execute("UPDATE jobs SET stage='confirm',status='blocked',candidate=?,branch='old-review'", (candidate,))
            db.commit()
            job = dict(db.execute("SELECT * FROM jobs").fetchone())
        review, confirm = lane.job_folder(self.root, job, "review"), lane.job_folder(self.root, job, "confirm")
        lane.save(review / "checks.json", {"head": self.source})
        lane.save(review / "result.json", {"prepared_parent": self.base})
        finding = {"what": "A concrete issue with evidence"}
        lane.save(confirm / "review.json", {"decision": "blocked", "findings": [finding]})
        old_report = (confirm / "review.json").read_bytes()
        self.assertEqual(lane.handoff_verifier_findings(self.root), 1)
        self.assertEqual(lane.handoff_verifier_findings(self.root), 0)
        with lane.connect(self.root) as db:
            job = dict(db.execute("SELECT * FROM jobs").fetchone())
            events = [r[0] for r in db.execute("SELECT kind FROM events ORDER BY id")]
        self.assertEqual(events, ["verification_blocked", "returned_for_fixes"])
        self.assertEqual((job["stage"], job["status"]), ("review", "queued"))
        self.assertIn(finding["what"], json.loads(job["source"])["retry_note"])
        self.assertEqual(lane.job_folder(self.root, job, "confirm"), self.root / "jobs" / job["id"] / "round-1" / "confirm")
        _, parent, _ = lane.prepare(self.root, self.repo, job)
        self.assertEqual(parent, self.base)
        self.assertEqual(self.git("rev-parse", "HEAD"), self.source)
        self.assertEqual((self.repo / path).read_text(), "first fix")
        self.assertEqual(lane.allowed_changes(self.repo, "Test Card"), [path])
        self.assertEqual(self.git("rev-parse", "old-review"), candidate)
        self.assertEqual((confirm / "review.json").read_bytes(), old_report)
        self.write(path, "second fix")
        final = lane.commit_candidate(self.repo, {"name": "Test Card", "commit": self.source}, parent, [path])
        self.assertEqual(self.git("show", final + ":" + path), "second fix")
        self.assertEqual(self.git("show", "-s", "--format=%an", final), "Original Author")
        self.assertEqual(self.git("rev-list", "--count", f"{self.base}..{final}"), "1")

    def test_verifier_handoff_filters_and_caps(self):
        lane.save(self.root / "config.json", {"max_verifier_rounds": 3})
        with lane.connect(self.root) as db:
            for name in ("pass", "empty", "infra", "cap"):
                lane.enqueue(db, {"name": name, "commit": name, "review_round": 2 if name == "cap" else 0})
            db.execute("UPDATE jobs SET stage='confirm',status=CASE WHEN name='infra' THEN 'infrastructure' ELSE 'blocked' END,candidate='candidate'")
            db.commit()
            jobs = [dict(r) for r in db.execute("SELECT * FROM jobs")]
        for job in jobs:
            review = lane.job_folder(self.root, job, "review")
            lane.save(review / "checks.json", {"head": "base"})
            lane.save(review / "result.json", {"prepared_parent": "parent"})
            report = {"decision": "pass" if job["name"] == "pass" else "blocked",
                      "findings": [] if job["name"] == "empty" else [{"what": "issue"}]}
            lane.save(lane.job_folder(self.root, job, "confirm") / "review.json", report)
        self.assertEqual(lane.handoff_verifier_findings(self.root), 0)
        self.assertEqual(lane.handoff_verifier_findings(self.root), 0)
        with lane.connect(self.root) as db:
            self.assertEqual(db.execute("SELECT count(*) FROM events WHERE kind='verification_blocked'").fetchone()[0], 1)
            self.assertEqual(db.execute("SELECT count(*) FROM jobs WHERE stage='review'").fetchone()[0], 0)


class CliTests(unittest.TestCase):
    def test_updated_driver_reloads_only_at_clean_boundary_without_old_resume(self):
        with mock.patch.object(lane,'driver_stamp',return_value=('new',)), \
             mock.patch.object(lane,'clean') as clean,mock.patch.object(lane.os,'execv') as execv:
            lane.reload_at_boundary('/campaign','r6',Path('/tree'),'review_lane.py',('old',))
        clean.assert_called_once_with(Path('/tree'))
        args=execv.call_args.args[1]
        self.assertEqual(args[-4:],['--root','/campaign','worker','r6'])
        self.assertNotIn('--resume',args)

    def test_updated_driver_preserves_dirty_card_work_instead_of_reloading(self):
        with mock.patch.object(lane,'driver_stamp',return_value=('new',)), \
             mock.patch.object(lane,'clean',side_effect=RuntimeError('unfinished work preserved')), \
             mock.patch.object(lane.os,'execv') as execv:
            with self.assertRaisesRegex(RuntimeError,'preserved'):
                lane.reload_at_boundary('/campaign','r6',Path('/tree'),'review_lane.py',('old',))
        execv.assert_not_called()

    def test_only_additive_local_test_dependencies_and_matching_lockfile_are_allowed(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);package=root/'crates/cantrip-cards';package.mkdir(parents=True)
            (root/'Cargo.toml').write_text('[workspace.dependencies]\ncantrip-rules = { path="crates/cantrip-rules" }\n')
            before='[package]\nname="cantrip-cards"\n[dependencies]\ncantrip-api = { workspace=true }\n[dev-dependencies]\n'
            after=before+'cantrip-rules = { workspace=true }\n'
            (package/'Cargo.toml').write_text(after)
            old_lock='version=4\n[[package]]\nname="cantrip-cards"\nversion="0.1.0"\ndependencies=["cantrip-api"]\n'
            new_lock=old_lock.replace('["cantrip-api"]','["cantrip-api","cantrip-rules"]')
            (root/'Cargo.lock').write_text(new_lock)
            paths=['Cargo.lock','crates/cantrip-cards/Cargo.toml']
            def git(_, *args):
                if args[0]=='diff':return '\n'.join(paths)
                if args[0]=='ls-files':return ''
                return old_lock if args[-1]=='HEAD:Cargo.lock' else before
            with mock.patch.object(lane,'git',side_effect=git):
                self.assertEqual(lane.allowed_changes(root,'A'),paths)
                (package/'Cargo.toml').write_text(after.replace('cantrip-api = { workspace=true }','cantrip-api="1"'))
                with self.assertRaisesRegex(RuntimeError,'only additive local'):lane.allowed_changes(root,'A')
                (package/'Cargo.toml').write_text(after+'remote-crate="1"\n')
                with self.assertRaisesRegex(RuntimeError,'existing local workspace'):lane.allowed_changes(root,'A')
                (package/'Cargo.toml').write_text(after)
                (root/'Cargo.lock').write_text(new_lock.replace('"cantrip-rules"','"remote-crate"'))
                with self.assertRaisesRegex(RuntimeError,'lockfile changes'):lane.allowed_changes(root,'A')

    def test_shared_rust_test_file_is_in_scope_but_core_edits_are_not(self):
        tree=Path('/tree')
        with mock.patch.object(lane,'git',side_effect=['crates/cantrip-mechanics/tests/boundary.rs','']):
            self.assertEqual(lane.allowed_changes(tree,'A'),['crates/cantrip-mechanics/tests/boundary.rs'])
        with mock.patch.object(lane,'git',side_effect=['crates/cantrip-rules/src/resolve.rs','']):
            with self.assertRaisesRegex(RuntimeError,'out-of-scope'):lane.allowed_changes(tree,'A')

    def test_shared_callers_keep_scenario_coverage_without_unrelated_mutation_work(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);card=root/'a.rs';card.write_text('pub static CARD: X;')
            lane.save(root/'review.json', {'affected_cards':['A','B','C','D']})
            mutation=mock.Mock(return_value={})
            gate=SimpleNamespace(results=[],failures=lambda:[])
            tools=mock.Mock(return_value=(0,'matches Scryfall'))
            corpus=mock.Mock(return_value=(0,'1 pass, 0 fail, 0 known bugs, 0 need cards'))
            fake=SimpleNamespace(card_dir=lambda _:root/'state'/'cards'/'a',card_file=lambda _:card,
                                 card_scenarios=lambda _:[],focus_cards=lambda _:None,run_corpus=corpus,
                                 tool=tools,Gates=lambda *args:gate,mutation_gates=mutation)
            with mock.patch.object(lane,'current',return_value={'tree':str(root),'folder':str(root),'name':'A'}), \
                 mock.patch.object(lane,'load_lane',return_value=fake),mock.patch.object(lane,'git',return_value='head'), \
                 mock.patch.object(lane,'work_digest',return_value='stable'), \
                 mock.patch.object(lane,'allowed_changes',return_value=['Cargo.lock',
                                                                       'crates/cantrip-cards/Cargo.toml',
                                                                       'crates/cantrip-mechanics/src/targets.rs',
                                                                       'crates/cantrip-cards/src/cards/c.rs',
                                                                       'scenarios/d-caller-fix.cantrip']):
                result=lane.checks(root)
            self.assertTrue(result['ok'])
            self.assertEqual([c.args[0] for c in corpus.call_args_list],['A','C','D','B'])
            self.assertEqual([c.args[0] for c in mutation.call_args_list],[['A'],['C']])
            self.assertIn(mock.call('card-coverage.py','B'),tools.call_args_list)
            self.assertNotIn(mock.call('new-card.py','B','--no-write'),tools.call_args_list)
            self.assertIn(mock.call('card-coverage.py','D'),tools.call_args_list)
            self.assertIn(mock.call('new-card.py','D','--no-write'),tools.call_args_list)
            self.assertIn('shared citations',[g['gate'] for g in result['gates']])

    def test_edited_coverage_failure_returns_before_shared_caller_sweep_or_mutations(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);card=root/'a.rs';card.write_text('pub static CARD: X;')
            lane.save(root/'review.json', {'affected_cards':['A','B','C']})
            mutation=mock.Mock(return_value={})
            corpus=mock.Mock(return_value=(0,'1 pass, 0 fail, 0 known bugs, 0 need cards'))
            def tool(command,*args):
                return (1,'uncovered decision') if command=='card-coverage.py' and args[0]=='C' else (0,'matches Scryfall')
            fake=SimpleNamespace(card_dir=lambda _:root/'state'/'cards'/'a',card_file=lambda _:card,
                                 card_scenarios=lambda _:[],focus_cards=lambda _:None,run_corpus=corpus,
                                 tool=tool,Gates=lambda *args:SimpleNamespace(results=[],failures=lambda:[]),mutation_gates=mutation)
            with mock.patch.object(lane,'current',return_value={'tree':str(root),'folder':str(root),'name':'A'}), \
                 mock.patch.object(lane,'load_lane',return_value=fake),mock.patch.object(lane,'git',return_value='head'), \
                 mock.patch.object(lane,'work_digest',return_value='stable'), \
                 mock.patch.object(lane,'allowed_changes',return_value=['crates/cantrip-cards/src/cards/c.rs']):
                result=lane.checks(root)
            self.assertFalse(result['ok'])
            self.assertEqual([c.args[0] for c in corpus.call_args_list],['A','C'])
            self.assertEqual([c.args[0] for c in mutation.call_args_list],[['A']])
            self.assertEqual(lane.read(root/'checks-live.json')['state'],'failed')

    def test_completed_review_requires_exact_source_and_report_for_gate_resume(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);report=root/'review.json';report.write_text('{"decision":"pass"}')
            import hashlib
            previous={'completed_review':{'report_sha256':hashlib.sha256(report.read_bytes()).hexdigest(),
                                          'source_digest':'unchanged'}}
            with mock.patch.object(lane,'work_digest',return_value='unchanged'):
                self.assertTrue(lane.completed_review_matches(root,report,previous))
            with mock.patch.object(lane,'work_digest',return_value='changed'):
                self.assertFalse(lane.completed_review_matches(root,report,previous))
            report.write_text('{"decision":"blocked"}')
            with mock.patch.object(lane,'work_digest',return_value='unchanged'):
                self.assertFalse(lane.completed_review_matches(root,report,previous))

    def test_review_scratch_probe_uses_common_dependency_aware_runner(self):
        fake=SimpleNamespace(cmd_corpus=mock.Mock(return_value=7))
        argv=['review_lane.py','corpus','--file','/tmp/probe.cantrip','--next']
        with mock.patch('sys.argv',argv), mock.patch.object(lane,'current',return_value={'tree':'/tree','folder':'/packet'}), \
             mock.patch.object(lane,'load_lane',return_value=fake):
            self.assertEqual(lane.main(),7)
        fake.cmd_corpus.assert_called_once_with(argv[2:])

    def test_checks_reject_source_changes_during_validation(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);card=root/'a.rs';card.write_text('pub static CARD: X;')
            gate=SimpleNamespace(results=[],failures=lambda:[])
            fake=SimpleNamespace(card_dir=lambda _:root/'state'/'cards'/'a', card_file=lambda _:card,
                                 card_scenarios=lambda _:[], focus_cards=lambda _:None,
                                 run_corpus=lambda _:(0,'1 pass, 0 fail, 0 known bugs, 0 need cards'),
                                 tool=lambda *args:(0,'matches Scryfall'), Gates=lambda *args:gate,
                                 mutation_gates=lambda *args:{})
            assignment={'tree':str(root),'folder':str(root),'name':'A'}
            with mock.patch.object(lane,'current',return_value=assignment),mock.patch.object(lane,'load_lane',return_value=fake), \
                 mock.patch.object(lane,'allowed_changes',return_value=[]),mock.patch.object(lane,'git',return_value='head'), \
                 mock.patch.object(lane,'work_digest',side_effect=['before','after']):
                result=lane.checks(root)
            self.assertFalse(result['ok'])
            self.assertEqual(result['initial_digest'],'before')
            self.assertEqual(result['diff_digest'],'after')
            self.assertEqual(result['gates'][-1],{'gate':'stable source','ok':False,'seconds':0})

    def test_typographic_apostrophe_does_not_hide_a_declared_caller(self):
        self.assertEqual(lane.names('Hawkeye’s Bow'),lane.names("Hawkeye's Bow"))
        report={'decision':'pass','validation':[{'agrees':True}], 'affected_cards':['Hawkeye’s Bow'],'client':'existing'}
        self.assertTrue(lane.report_valid(report,"Hawkeye's Bow"))

    def test_missing_affected_card_is_repair_feedback_not_an_infrastructure_crash(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp)
            fake=SimpleNamespace(focus_cards=lambda _:None,card_dir=lambda _:root/'state'/'cards'/'a')
            with mock.patch.object(lane,'current',return_value={'tree':str(root),'folder':str(root),'name':'A'}), \
                 mock.patch.object(lane,'load_lane',return_value=fake), \
                 mock.patch.object(lane,'allowed_changes',return_value=['crates/cantrip-cards/src/cards/another.rs']), \
                 mock.patch.object(lane,'work_digest',return_value='same'):
                result=lane.checks(root)
            self.assertFalse(result['ok'])
            self.assertEqual(result['gates'][0]['gate'],'affected cards')
            self.assertIn('another',result['gates'][0]['detail'])


if __name__ == "__main__":
    unittest.main()
