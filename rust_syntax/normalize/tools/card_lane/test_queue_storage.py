import json
from pathlib import Path
import sqlite3
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, call, patch

import control
import implementation_lane as implementation
import queue_storage
import review_lane as review


class StorageRecoveryTests(unittest.TestCase):
    def test_failed_initializers_close_connections_then_recover(self):
        first, second, good = Mock(), Mock(), Mock()
        initialize = Mock(side_effect=[sqlite3.OperationalError('disk I/O error'),
                                       sqlite3.OperationalError('database is locked'), None])
        with patch.object(queue_storage.sqlite3, 'connect', side_effect=[first, second, good]) as connect, \
             patch.object(queue_storage.time, 'sleep') as sleep:
            self.assertIs(queue_storage.open_database('queue', sqlite3.Connection, initialize), good)
        self.assertEqual(connect.call_count, 3)
        first.close.assert_called_once(); second.close.assert_called_once(); good.close.assert_not_called()
        self.assertEqual(sleep.call_args_list, [call(1), call(2)])

    def test_repeated_storage_failure_is_bounded_and_nonstorage_errors_are_not_retried(self):
        for message, count in [('disk I/O error', 3), ('no such table: typo', 1)]:
            with self.subTest(message=message), \
                 patch.object(queue_storage.sqlite3, 'connect', side_effect=sqlite3.OperationalError(message)) as connect, \
                 patch.object(queue_storage.time, 'sleep') as sleep:
                with self.assertRaises(sqlite3.OperationalError):
                    queue_storage.open_database('queue', sqlite3.Connection, Mock())
                self.assertEqual(connect.call_count, count)
                self.assertEqual(sleep.call_count, count-1)

    def test_transactions_after_initialization_are_never_replayed(self):
        db = Mock()
        with patch.object(queue_storage.sqlite3, 'connect', return_value=db) as connect, \
             patch.object(queue_storage.time, 'sleep') as sleep:
            connection = queue_storage.open_database('queue', sqlite3.Connection, Mock())
            connection.commit.side_effect = sqlite3.OperationalError('disk I/O error')
            with self.assertRaises(sqlite3.OperationalError):
                connection.commit()
            connect.assert_called_once(); connection.commit.assert_called_once(); sleep.assert_not_called()

    def test_sqlite_primary_code_recognizes_extended_io_errors(self):
        error = sqlite3.OperationalError('platform-specific text')
        error.sqlite_errorcode = sqlite3.SQLITE_IOERR | (7 << 8)
        self.assertTrue(queue_storage.retryable(error))
        error.sqlite_errorcode = sqlite3.SQLITE_CORRUPT
        self.assertFalse(queue_storage.retryable(error))

    def test_launcher_restarts_failed_workers_and_watchers(self):
        command = control.launch_command(dict(folder=Path('/tmp/lane'), unit='lane-r1', command=['python', 'worker']))
        self.assertIn('--property=Restart=on-failure', command)
        self.assertIn('--property=RestartSec=5', command)


class LeaseAndPacketTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def test_review_restart_preserves_its_old_lease_before_cleaning_or_claiming(self):
        tree = self.root/'tree'
        review.save(self.root/'config.json', dict(reviewers={'r1': {'tree': str(tree)}}))
        with review.connect(self.root) as db:
            review.enqueue(db, dict(name='Unfinished', commit='old'))
            job = review.claim(db, 'r1')
            review.enqueue(db, dict(name='Next', commit='new'))
        with patch.object(review, 'clean') as clean, patch.object(review, 'maintain_caches') as caches:
            review.worker(self.root, 'r1', once=True)
        clean.assert_not_called(); caches.assert_not_called()
        with review.connect(self.root) as db:
            self.assertEqual(db.execute('select status from jobs where id=?', (job['id'],)).fetchone()[0], 'running')
            self.assertEqual(db.execute("select status from jobs where name='Next'").fetchone()[0], 'queued')
        self.assertTrue(review.read(self.root/'workers/r1/live.json')['recovery_required'])

    def test_implementation_restart_also_preserves_the_unfinished_assignment(self):
        tree = self.root/'tree'
        review.save(self.root/'config.json', dict(implementers={'i1': {'tree': str(tree)}}))
        with implementation.connect(self.root) as db:
            db.execute("insert into implementation_jobs(id,name,revision,card,queue_meta,status,enqueued,worker) values(?,?,?,?,?,?,?,?)",
                       ('old','Unfinished','rev','{}','{}','running',review.now(),'i1'))
        with patch.object(review, 'clean') as clean, patch.object(review, 'maintain_caches') as caches:
            implementation.worker(self.root, 'i1', once=True)
        clean.assert_not_called(); caches.assert_not_called()
        self.assertTrue(review.read(self.root/'implementation/i1/live.json')['recovery_required'])

    def test_disabled_workers_do_not_touch_retired_worktrees_or_queue(self):
        review.save(self.root/'config.json', dict(enabled=False, reviewers={'r1': {'tree': str(self.root/'gone-r1')}},
                                                  implementers={'i1': {'tree': str(self.root/'gone-i1')}}))
        with patch.object(review, 'connect') as connect, patch.object(review, 'clean') as clean:
            review.worker(self.root, 'r1', once=True)
            implementation.worker(self.root, 'i1', once=True)
        connect.assert_not_called(); clean.assert_not_called()
        self.assertFalse((self.root/'gone-r1').exists()); self.assertFalse((self.root/'gone-i1').exists())

    def test_missing_packet_without_required_evidence_is_an_explicit_infrastructure_error(self):
        with self.assertRaisesRegex(ValueError, 'review input lacks producer evidence'):
            review.packet(self.root, self.root/'tree', dict(id='job',stage='review',source='{}'), dict(name='A',commit='one'), Mock())

    def test_candidate_bound_cached_packet_remains_usable_without_deleted_producer_metadata(self):
        review.save(self.root/'config.json', {'review_model':'test'})
        job = dict(id='job', stage='review', source='{}')
        folder = review.job_folder(self.root, job, 'review'); folder.mkdir(parents=True)
        (folder/'author-reference.md').write_text('Original Oracle and rules evidence')
        (folder/'author-report.json').write_text('{"original_report":true}')
        content = self.root/'content.json'; content.write_text('{}')
        backend = SimpleNamespace(card_file=lambda name:self.root/'absent.rs', card_scenarios=lambda name:[],
                                  content_file=lambda name:content)
        review.packet(self.root, self.root/'tree', job, dict(name='A',commit='one'), backend)
        packet = (folder/'packet.md').read_text()
        self.assertIn('Original Oracle and rules evidence',packet)
        self.assertIn('original_report',packet)
        self.assertIsNone(json.loads((folder/'prior-findings.json').read_text()))


if __name__ == '__main__':
    unittest.main()
