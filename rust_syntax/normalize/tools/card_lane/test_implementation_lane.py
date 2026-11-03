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
