import json
import threading
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import card_lane
import implementation_lane as impl


def setup_root(tmp_path, cards):
    queue = tmp_path / 'source.json'
    queue.write_text(json.dumps({'generator_commit': 'rev', 'format': 'test', 'cards': cards}))
    (tmp_path / 'config.json').write_text(json.dumps({'implementation_queue': str(queue), 'base': 'base',
        'lane_root': str(tmp_path), 'implementers': {'i1': {'tree': str(tmp_path / 'tree')}}}))
    return queue


class ImplementationLaneTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.tmp_path = Path(self.temporary.name)

    def test_atomic_claim_and_terminal_not_reimported(self):
        tmp_path = self.tmp_path
        setup_root(tmp_path, [{'name': 'A', 'status': 'pending'}, {'name': 'B', 'status': 'pending'}])
        assert impl.sync(tmp_path) == 2
        jobs = []
        def get(slot):
            with impl.connect(tmp_path) as db:
                jobs.append(impl.claim(db, slot))
        threads = [threading.Thread(target=get, args=(str(i),)) for i in range(3)]
        for thread in threads: thread.start()
        for thread in threads: thread.join()
        claimed = [j for j in jobs if j]
        assert len(claimed) == 2 and len({j['id'] for j in claimed}) == 2
        impl.finish(tmp_path, claimed[0], 'generator_bug', {})
        assert impl.sync(tmp_path) == 0
        with impl.connect(tmp_path) as db:
            assert impl.claim(db, 'another') is None


    def test_immediate_enqueue(self):
        tmp_path = self.tmp_path
        setup_root(tmp_path, [{'name': 'A', 'status': 'pending'}])
        lane = SimpleNamespace(Codex=lambda *args: object(), Usage=card_lane.Usage, spawn=lambda *args: None,
                               save_queue=lambda q: None, card_dir=lambda name: tmp_path / name)
        def run_card(card, backend, args, queue):
            card.update(status='committed', commit='abc')
            return 'committed'
        lane.run_card = run_card
        with mock.patch.object(impl.review, 'clean'), mock.patch.object(impl.review, 'git'), \
             mock.patch.object(impl.review, 'load_lane', return_value=lane):
            impl.worker(tmp_path, 'i1', once=True)
        with impl.review.connect(tmp_path) as db:
            row = db.execute('SELECT source FROM jobs').fetchone()
            source = json.loads(row['source'])
            assert source['commit'] == 'abc' and source['source_base'] == 'base'
            assert source['packet'] == str(tmp_path / 'A')
        assert impl.status(tmp_path)['counts'] == {'committed': 1}
        imported = impl.review.read(tmp_path / 'source.json')['cards'][0]
        assert imported['status'] == 'committed' and imported['commit'] == 'abc'

    def test_generator_hold_waits_without_claiming_and_reuses_session_when_resumed(self):
        root=self.tmp_path;setup_root(root,[{'name':'A','status':'pending'}])
        cfg=impl.review.config(root);cfg['implementation_enabled']=False;impl.review.save(root/'config.json',cfg)
        order=[]
        lane=SimpleNamespace(Codex=lambda *args:object(),Usage=card_lane.Usage,spawn=mock.Mock(),
                             save_queue=lambda q:None,card_dir=lambda name:root/name)
        model_spawn=lane.spawn
        def resume(_):
            order.append('wait');cfg['implementation_enabled']=True;impl.review.save(root/'config.json',cfg)
        def run_card(card,*args):
            order.append('card');card.update(status='committed',commit='abc')
            cfg['enabled']=False;impl.review.save(root/'config.json',cfg)
            return 'committed'
        lane.run_card=run_card
        with mock.patch.object(impl.review,'clean'),mock.patch.object(impl.review,'git'), \
             mock.patch.object(impl.review,'load_lane',return_value=lane) as loaded, \
             mock.patch.object(impl.time,'sleep',side_effect=resume) as wait, \
             mock.patch.object(impl,'Session',wraps=impl.Session) as sessions:
            impl.worker(root,'i1')
        self.assertEqual(order,['wait','card'])
        wait.assert_called_once();loaded.assert_called_once();sessions.assert_called_once()
        # Waiting is in the driver; it never starts a model turn.
        model_spawn.assert_not_called()
        self.assertEqual(impl.status(root)['counts'],{'committed':1})

    def test_finish_preserves_new_intake_and_does_not_clobber_new_revision(self):
        queue = setup_root(self.tmp_path, [{'name':'A','status':'pending'},{'name':'New','status':'pending'}])
        impl.sync(self.tmp_path)
        with impl.connect(self.tmp_path) as db:
            job = impl.claim(db, 'i1')
        impl.finish(self.tmp_path, job, 'committed', {'card':{'name':'A','commit':'one'}})
        cards = impl.review.read(queue)['cards']
        assert cards[0]['status'] == 'committed' and cards[1] == {'name':'New','status':'pending'}
        newer = impl.review.read(queue)
        newer['generator_commit'] = 'new-revision'; newer['cards'][0]['status'] = 'pending'
        impl.review.save(queue, newer)
        impl.finish(self.tmp_path, job, 'blocked', {})
        assert impl.review.read(queue) == newer

    def test_sync_recovers_outcome_after_database_write_before_queue_write(self):
        queue=setup_root(self.tmp_path,[{'name':'A','status':'pending'}])
        impl.sync(self.tmp_path)
        with impl.connect(self.tmp_path) as db:
            job=impl.claim(db,'i1')
            db.execute("UPDATE implementation_jobs SET status='committed',finished='then',result=? WHERE id=?",
                       (json.dumps({'card':{'name':'A','commit':'saved'}}),job['id']))
            db.commit()
        impl.sync(self.tmp_path)
        card=impl.review.read(queue)['cards'][0]
        assert card['status']=='committed' and card['commit']=='saved' and card['finished']=='then'

    def test_explicit_retry_archives_packet_and_keeps_old_outcome_without_overwriting_retry(self):
        queue=setup_root(self.tmp_path,[{'name':'A','status':'pending'}])
        impl.sync(self.tmp_path)
        with impl.connect(self.tmp_path) as db:old=impl.claim(db,'i1')
        impl.finish(self.tmp_path,old,'needs_review',{'card':{'name':'A','failure':'state read-only'}})
        packet=self.tmp_path/'implementation/i1/state/cards/a';packet.mkdir(parents=True)
        (packet/'work.patch').write_text('prior scenarios')
        impl.retry_stopped(self.tmp_path,old['id'],'State directory writable')
        self.assertEqual(impl.sync(self.tmp_path),1)
        self.assertEqual(impl.review.read(queue)['cards'][0]['status'],'pending')
        with impl.connect(self.tmp_path) as db:
            new=impl.claim(db,'i2')
            self.assertEqual(db.execute('SELECT status FROM implementation_jobs WHERE id=?',(old['id'],)).fetchone()[0],'needs_review')
        self.assertNotEqual(old['id'],new['id'])
        card=json.loads(new['card']);self.assertEqual(card['implementation_retry'],1)
        self.assertEqual((Path(card['implementation_previous_packet'])/'work.patch').read_text(),'prior scenarios')
        impl.finish(self.tmp_path,new,'committed',{'card':{'name':'A','commit':'new'}})
        impl.mirror_outcomes(self.tmp_path)
        self.assertEqual(impl.review.read(queue)['cards'][0]['commit'],'new')
        with self.assertRaises(ValueError):impl.retry_stopped(self.tmp_path,new['id'],'No duplicate')

    def test_handed_off_work_cannot_be_retried_as_a_new_implementation(self):
        queue=setup_root(self.tmp_path,[{'name':'A','status':'pending'}]);impl.sync(self.tmp_path)
        with impl.connect(self.tmp_path) as db: old=impl.claim(db,'i1')
        impl.finish(self.tmp_path,old,'needs_review',{'card':{'name':'A','failure':'incomplete'}})
        with impl.review.connect(self.tmp_path) as db:
            impl.review.enqueue(db,{'name':'A','commit':'provisional','implementation_job':old['id']})
        before=queue.read_bytes()
        for status in ('queued','running','blocked','passed'):
            with impl.review.connect(self.tmp_path) as db: db.execute('UPDATE jobs SET status=?',(status,))
            with self.assertRaisesRegex(ValueError,'review handoff'):
                impl.retry_stopped(self.tmp_path,old['id'],'retry')
            self.assertEqual(queue.read_bytes(),before)
        with impl.connect(self.tmp_path) as db:
            self.assertEqual(db.execute('SELECT status FROM implementation_jobs WHERE id=?',(old['id'],)).fetchone()[0],'needs_review')

    def test_one_card_can_advance_generator_revision_without_changing_an_active_lease(self):
        queue=setup_root(self.tmp_path,[{'name':'A','status':'pending'},{'name':'B','status':'pending'}])
        impl.sync(self.tmp_path)
        with impl.connect(self.tmp_path) as db:active=impl.claim(db,'i1')
        q=impl.review.read(queue);q['cards'][1]['implementation_generator_revision']='fixed';impl.review.save(queue,q)
        self.assertEqual(impl.sync(self.tmp_path),1)
        with impl.connect(self.tmp_path) as db:
            self.assertEqual(db.execute('select status from implementation_jobs where id=?',(active['id'],)).fetchone()[0],'running')
            updated=impl.claim(db,'i2')
        self.assertEqual((updated['name'],updated['revision']),('B','fixed'))
        self.assertEqual(json.loads(updated['queue_meta'])['generator_commit'],'fixed')
        impl.finish(self.tmp_path,active,'committed',{'card':{'name':'A','commit':'old-source'}})
        impl.finish(self.tmp_path,updated,'committed',{'card':{'name':'B','commit':'fixed-source'}})
        cards=impl.review.read(queue)['cards']
        self.assertEqual([c['commit'] for c in cards],['old-source','fixed-source'])


    def test_session_rotation_and_usage(self):
        tmp_path = self.tmp_path
        calls = []
        def spawn(backend, card, prompt, session, timeout, log):
            calls.append(session)
            usage = card_lane.Usage()
            usage.input = 10 * (len(calls) if session else 1)
            usage.turns = 1
            return 0, 'session', usage
        lane = SimpleNamespace(spawn=spawn, Usage=card_lane.Usage)
        state = impl.Session(lane, {'implementation_cards_per_context': 20}, tmp_path)
        backend = SimpleNamespace(cumulative=True, model='gpt-6.1-terra')
        with mock.patch.object(impl.context_budget, 'session_context', return_value={}):
            for i in range(21):
                state.start_card()
                _, _, used = state.spawn(backend, {'name': 'A'}, 'prompt', None, 10, tmp_path / 'log')
                assert used.input == 10
                state.cards += 1
        assert calls[0] is None and all(s == 'session' for s in calls[1:20]) and calls[20] is None


    def test_idle_session_restarts_without_reusing_cumulative_usage(self):
        clock, calls = [10.0], []
        def spawn(backend, card, prompt, session, timeout, log):
            calls.append(session)
            used = card_lane.Usage()
            used.input = (10, 20, 30, 10)[len(calls) - 1]
            used.turns = 1
            if len(calls) == 2:
                clock[0] += 7200  # A long current card does not make its next card stale.
            return 0, 'session', used
        lane = SimpleNamespace(spawn=spawn, Usage=card_lane.Usage)
        state = impl.Session(lane, {}, self.tmp_path)
        backend = SimpleNamespace(cumulative=True, model='gpt-6.1-terra')
        with mock.patch.object(impl.time, 'monotonic', side_effect=lambda: clock[0]), \
             mock.patch.object(impl.context_budget, 'session_context', return_value={}):
            for gap in (0, 3600, 0, 3600.001):
                clock[0] += gap
                state.start_card()
                _, _, used = state.spawn(backend, {'name': 'A'}, 'prompt', None, 10000, self.tmp_path/'log')
                self.assertEqual(used.input, 10)
                state.finish_card()
        self.assertEqual(calls, [None, 'session', 'session', None])
        self.assertEqual(state.cards, 1)

    def test_precompact_resumes_same_card(self):
        tmp_path = self.tmp_path
        calls = []
        def spawn(backend, card, prompt, session, timeout, log):
            calls.append((session, prompt))
            usage = card_lane.Usage()
            usage.input, usage.turns = 10, 1
            if len(calls) == 1:
                Path(card['context_marker']).write_text(json.dumps({'session_id': 'old', 'trigger': 'auto'}))
                return 1, 'old', usage
            return 0, 'new', usage
        lane = SimpleNamespace(spawn=spawn, Usage=card_lane.Usage)
        state = impl.Session(lane, {}, tmp_path)
        state.start_card()
        with mock.patch.object(impl.context_budget, 'session_context', return_value={}):
            code, session, usage = state.spawn(SimpleNamespace(cumulative=True, model='gpt-6.1-terra'),
                                                {'name': 'A'}, 'original', None, 10, tmp_path / 'log')
        assert code == 0 and session == 'new' and usage.input == 20
        assert calls[1][0] is None and 'SAME card' in calls[1][1]

    def test_context_cap_and_infrastructure_usage(self):
        def spawn(backend, card, prompt, session, timeout, log):
            used = card_lane.Usage()
            used.input, used.turns = 42, 1
            used.infrastructure_error = "capacity unavailable"
            return 1, "session", used
        lane = SimpleNamespace(spawn=spawn, Usage=card_lane.Usage)
        state = impl.Session(lane, {'implementation_cards_per_context': 0}, self.tmp_path)
        state.session, state.cards = 'session', 20
        with mock.patch.object(impl.context_budget, 'session_context', return_value={'last_tokens': 100}):
            state.start_card()
            self.assertEqual(state.session, 'session')
            _, _, used = state.spawn(SimpleNamespace(cumulative=True, model='gpt-6.1-terra'),
                                     {'name': 'A'}, 'prompt', None, 10, self.tmp_path / 'log')
        self.assertEqual(used.input, 42)
        self.assertEqual(used.infrastructure_error, 'capacity unavailable')
        state.cards = 20
        with mock.patch.object(impl.context_budget, 'session_context', return_value={'last_tokens': None}):
            state.start_card()
        self.assertIsNone(state.session)


if __name__ == '__main__':
    unittest.main()
