import json
from pathlib import Path
import tempfile
import unittest

import implementation_lane
import review_lane
import usage_costs as costs


class CostTests(unittest.TestCase):
    def test_cached_input_is_discounted_and_unknown_is_not_free(self):
        self.assertAlmostEqual(costs.estimate({'input': 1000000, 'cached': 800000, 'output': 100000},
                                               'gpt-6.1-terra'), .74)
        self.assertIsNone(costs.estimate({}, 'gpt-6.1-terra'))
        self.assertIsNone(costs.estimate({'input': 1, 'cached': 2, 'output': 0}, 'gpt-6.1-terra'))
        self.assertIsNone(costs.estimate({'input': 1, 'cached': 0, 'output': 0}, 'unknown'))

    def test_retry_costs_deduplicate_copies_and_average_completed_cards(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            cfg = dict(producer_model='gpt-6.1-terra', review_model='gpt-6.1-terra')
            usage = dict(input=1000000, cached=0, output=0)
            with implementation_lane.connect(root) as db:
                for name in ('A', 'B'):
                    source = dict(name=name, commit=name, usage=usage)
                    review_lane.enqueue(db, source)
                db.execute("UPDATE jobs SET status='passed' WHERE name='A'")
                db.execute("INSERT INTO implementation_jobs(id,name,revision,card,queue_meta,status,enqueued,result) VALUES(?,?,?,?,?,?,?,?)",
                           ('impl-A', 'A', 'gen', '{}', '{}', 'committed', review_lane.now(),
                            json.dumps(dict(card=dict(name='A', commit='A', usage=usage),
                                            attempts=[dict(session='shared', usage=usage)]))))
                a, b = [dict(r) for r in db.execute('SELECT * FROM jobs ORDER BY name')]
                # Same session, distinct attempts: both charge, and the sum in
                # result.usage is a duplicate of its attempt breakdown.
                first = dict(model='gpt-6.1-terra', branch='x/review',
                             usage=dict(input=2000000, cached=0, output=0),
                             attempts=[dict(session='shared', seconds=10, usage=usage),
                                       dict(session='shared', seconds=11, usage=usage)])
                review_lane.event(db, a['id'], 'r1', 'queued', first)
                review_lane.event(db, a['id'], 'r1', 'infrastructure', first)  # replayed gates
                for stage, seconds, kind in [('confirm', 20, 'blocked'), ('review-round-1', 30, 'queued'),
                                             ('confirm-round-1', 40, 'passed')]:
                    review_lane.event(db, a['id'], 'r5', kind, dict(model='gpt-6.1-terra', branch='x/'+stage,
                                          attempts=[dict(session='shared', seconds=seconds, usage=usage)]))
                review_lane.event(db, b['id'], 'r2', 'queued', dict(model='gpt-6.1-terra', branch='x/review', usage=usage))
                result = costs.pipeline_summary(db, cfg)
            self.assertEqual(result['cards']['A']['stages'], dict(implement=1, review=3, verify=2))
            self.assertEqual(result['cards']['A']['attempts'], 6)
            self.assertEqual(result['completed_cards'], 1)
            self.assertEqual(result['average_usd_per_completed_card'], 6)
            self.assertEqual(result['total_usd'], 8)  # B's in-progress $2 remains in total

    def test_unknown_model_and_missing_implementation_excluded_from_average(self):
        with tempfile.TemporaryDirectory() as tmp:
            with review_lane.connect(Path(tmp)) as db:
                for name in ('Unknown', 'Missing'):
                    review_lane.enqueue(db, dict(name=name, commit=name))
                db.execute("UPDATE jobs SET status='passed'")
                for row in db.execute('SELECT id,name FROM jobs').fetchall():
                    review_lane.event(db, row['id'], 'r1', 'passed', dict(
                        model='unknown' if row['name'] == 'Unknown' else 'gpt-6.1-terra',
                        usage=dict(input=1000000, cached=0, output=0)))
                result = costs.pipeline_summary(db, {})
            self.assertEqual(result['completed_cards'], 0)
            self.assertIsNone(result['average_usd_per_completed_card'])
            self.assertEqual(result['unpriced_attempts'], 1)
            self.assertIsNone(result['cards']['Unknown']['usd_api_equivalent'])
            self.assertEqual(result['total_usd'], 1)

    def test_legacy_queue_estimates_stopped_work_without_double_counting(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            queue = root/'queue.json'
            usage = dict(input=500000, cached=0, output=0)
            queue.write_text(json.dumps(dict(cards=[dict(name='Old', status='deferred_blocked', usage=usage),
                                                      dict(name='New', status='pending') ])))
            with review_lane.connect(root) as db:
                result = costs.pipeline_summary(db, dict(implementation_queue=str(queue), producer_model='gpt-6.1-terra'))
            self.assertEqual(result['total_usd'], .5)
            self.assertTrue(result['cards']['Old']['batch_apportioned'])
            self.assertIsNone(result['cards']['New']['usd_api_equivalent'])


if __name__ == '__main__':
    unittest.main()
