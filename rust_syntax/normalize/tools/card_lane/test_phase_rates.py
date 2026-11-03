import json
from pathlib import Path
import tempfile
import unittest

import implementation_lane
import phase_rates
import review_lane


class PhaseRateTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.db = implementation_lane.connect(self.root)
        self.db.__enter__()
        self.end = '2026-11-03T12:00:00+00:00'
        self.cfg = dict(implementers={'i1': {}}, reviewers={'r1': {'role': 'review'}, 'r5': {'role': 'confirm'}})

    def tearDown(self):
        self.db.__exit__(None, None, None)
        self.tmp.cleanup()

    def job(self, name, status='queued', stage='review', round_number=0, worker=None, started=None):
        review_lane.enqueue(self.db, dict(name=name, commit=name, review_round=round_number))
        self.db.execute('UPDATE jobs SET status=?,stage=?,worker=?,started=? WHERE name=?',
                        (status, stage, worker, started, name))
        self.db.commit()
        return self.db.execute('SELECT id FROM jobs WHERE name=?', (name,)).fetchone()[0]

    def event(self, job, kind, branch, seconds, at='2026-11-03T11:50:00+00:00'):
        detail = dict(branch='pipeline/'+branch, seconds=seconds, session='retained-session')
        worker = 'r5' if branch.startswith('confirm') else 'r1'
        self.db.execute('INSERT INTO events(job,worker,at,kind,detail) VALUES(?,?,?,?,?)',
                        (job, worker, at, kind, json.dumps(detail)))
        self.db.commit()

    def test_phase_history_deduplication_and_shared_pool(self):
        job = self.job('A', status='passed', stage='confirm', round_number=3)
        self.event(job, 'queued', 'review', 60)
        self.event(job, 'infrastructure', 'review', 60)  # deterministic gate replay
        self.event(job, 'blocked', 'confirm', 20)
        self.event(job, 'queued', 'review-round-1', 90)
        self.event(job, 'passed', 'confirm-round-1', 30)
        other = self.job('B')
        self.event(other, 'passed', 'review', 30)
        phases = phase_rates.summary(self.db, self.cfg, self.end)['phases']
        self.assertEqual(phases['review']['advanced_per_hour'], 4)
        self.assertEqual(phases['fix']['advanced_per_hour'], 2)
        self.assertEqual(phases['verify']['advanced_per_hour'], 2)
        self.assertEqual(phases['verify']['failed_attempts'], 1)
        self.assertEqual(phases['review']['completed_attempts'], 2)
        self.assertEqual(phases['review_pool']['advanced_cards'], 2)  # A seen in both review and fix
        self.assertEqual(phases['review_pool']['workers'], 1)
        self.assertEqual(phases['review_pool']['queued'], 1)

    def test_rolling_window_ignores_old_success_and_later_replay(self):
        job = self.job('A')
        self.event(job, 'passed', 'review', 60, '2026-11-03T11:29:00+00:00')
        self.event(job, 'passed', 'review', 60)  # replay of an old result
        self.event(job, 'passed', 'review', 61, '2026-11-03T12:01:00+00:00')
        phases = phase_rates.summary(self.db, self.cfg, self.end)['phases']
        self.assertEqual(phases['review']['advanced_cards'], 0)
        self.assertEqual(phases['review']['advanced_per_hour'], 0)

    def test_active_work_and_overlapping_intervals_do_not_inflate_utilization(self):
        job = self.job('A', status='running', stage='review', round_number=1, worker='r1',
                       started='2026-11-03T11:20:00+00:00')
        self.event(job, 'queued', 'review-round-1', 1200)
        phases = phase_rates.summary(self.db, self.cfg, self.end)['phases']
        self.assertEqual(phases['fix']['active_workers'], 1)
        self.assertEqual(phases['fix']['busy_seconds'], 1800)
        self.assertEqual(phases['review_pool']['utilization'], 1)
        self.assertEqual(phases['verify']['utilization'], 0)

    def test_implementation_counts_only_new_successes_and_measures_failures(self):
        for name, state, began, finished in [
                ('new', 'committed', '11:48', '11:50'), ('failed', 'generator_bug', '11:50', '11:51'),
                ('old', 'committed', '11:00', '11:20'), ('live', 'running', '11:55', None),
                ('queued', 'queued', None, None)]:
            stamp = lambda t: f'2026-11-03T{t}:00+00:00' if t else None
            self.db.execute('INSERT INTO implementation_jobs(id,name,revision,card,queue_meta,status,enqueued,worker,started,finished,result) VALUES(?,?,?,?,?,?,?,?,?,?,?)',
                            (name, name, 'gen', '{}', '{}', state, self.end, 'i1', stamp(began), stamp(finished), '{}'))
        self.db.commit()
        phase = phase_rates.summary(self.db, self.cfg, self.end)['phases']['implementation']
        self.assertEqual(phase['advanced_per_hour'], 2)
        self.assertEqual(phase['attempts_per_hour'], 4)
        self.assertEqual(phase['failed_attempts'], 1)
        self.assertEqual(phase['mean_seconds'], 90)
        self.assertEqual(phase['busy_seconds'], 480)
        self.assertEqual(phase['queued'], 1)

    def test_empty_history_has_zero_throughput_and_no_invented_duration(self):
        result = phase_rates.summary(self.db, self.cfg, self.end)
        self.assertEqual(result['window_seconds'], 1800)
        for phase in result['phases'].values():
            self.assertEqual(phase['advanced_per_hour'], 0)
            self.assertIsNone(phase['mean_seconds'])


if __name__ == '__main__':
    unittest.main()
