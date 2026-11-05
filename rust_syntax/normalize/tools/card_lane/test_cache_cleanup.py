import fcntl
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock

import cache_cleanup as cleanup
import cache_leases


class CacheCleanupTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.pool = self.root / 'worktrees'
        self.pool.mkdir()
        self.cfg = {'publisher': {'worktree_root': str(self.pool)}}
        self.leases = mock.patch.object(cache_leases, 'LEASE_DIR', self.root / 'leases')
        self.leases.start()

    def tearDown(self):
        for path, (_, count) in list(cache_leases._held.items()):
            if path.is_relative_to(self.root):
                for _ in range(count):
                    cache_leases.release(path)
        self.leases.stop()
        self.tmp.cleanup()

    def cache(self, name='retired', *, age=8 * 3600, external=False):
        tree = self.pool / name
        path = tree if external else tree / 'target'
        (path / 'checked').mkdir(parents=True)
        (path / '.rustc_info.json').write_text('{}')
        (path / 'checked/.cargo-lock').touch()
        (path / 'checked/artifact').write_bytes(b'reproducible compiler output')
        when = time.time() - age
        for p in sorted(path.rglob('*'), reverse=True) + [path]:
            os.utime(p, (when, when))
        return path

    def test_removes_old_cache_preserving_source_dirty_work_and_evidence(self):
        cache = self.cache()
        source = cache.parent / 'uncommitted.rs'
        source.write_text('valuable work')
        evidence = cache.parent / 'validation.log'
        evidence.write_text('actual test evidence')
        result = cleanup.sweep(self.root, self.cfg, now=time.time() + 9 * 3600)
        self.assertEqual(result['removed'], [str(cache)])
        self.assertFalse(cache.exists())
        self.assertEqual(source.read_text(), 'valuable work')
        self.assertEqual(evidence.read_text(), 'actual test evidence')

    def test_keeps_configured_workers_and_explicit_seed_including_nested_pin(self):
        worker, seed, nested = [self.cache(n) for n in ('worker', 'seed', 'nested')]
        self.cfg.update(reviewers={'r1': {'tree': str(worker.parent)}})
        self.cfg['cache_cleanup'] = {'protected_paths': [str(seed), str(nested / 'checked/artifact')]}
        for path in (worker, seed, nested):
            cleanup.retire(self.root, path)
        result = cleanup.sweep(self.root, self.cfg)
        self.assertEqual(result['removed'], [])
        self.assertTrue(all(p.exists() for p in (worker, seed, nested)))

    def test_recent_cache_is_retained_until_temporary_owner_retires_it(self):
        cache = self.cache(age=0)
        self.assertEqual(cleanup.sweep(self.root, self.cfg)['removed'], [])
        cleanup.retire(self.root, cache)
        self.assertEqual(cleanup.sweep(self.root, self.cfg)['removed'], [str(cache)])

    def test_any_recent_nested_output_preserves_old_root(self):
        cache = self.cache()
        (cache / 'checked/artifact').write_text('newly compiled')
        later = time.time() + 9 * 3600
        os.utime(cache / 'checked/artifact', (later, later))
        self.assertEqual(cleanup.sweep(self.root, self.cfg, now=later)['removed'], [])

    def test_fresh_cow_clone_is_recent_despite_preserved_old_mtimes(self):
        cache = self.cache()
        self.assertEqual(cleanup.sweep(self.root, self.cfg)['removed'], [])

    def test_new_use_after_retirement_cancels_immediate_eviction(self):
        cache = self.cache()
        cleanup.retire(self.root, cache)
        later = time.time() + 1
        os.utime(cache / 'checked/artifact', (later, later))
        self.assertEqual(cleanup.sweep(self.root, self.cfg, now=later)['removed'], [])

    def test_cargo_lock_and_open_output_prevent_deletion(self):
        locked, opened = self.cache('locked'), self.cache('opened')
        with (locked / 'checked/.cargo-lock').open('r+b') as lock, (opened / 'checked/artifact').open('rb'):
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            result = cleanup.sweep(self.root, self.cfg)
        self.assertEqual(result['removed'], [])
        self.assertEqual(result['skipped'][str(locked)], 'Cargo cache is locked')
        self.assertEqual(result['skipped'][str(opened)], 'process has cache open')

    def test_symlink_and_source_named_target_are_preserved(self):
        outside = self.root / 'outside'
        outside.mkdir()
        (outside / 'important').write_text('keep')
        (self.pool / 'link').symlink_to(outside, target_is_directory=True)
        cache = self.cache('source')
        (cache / 'Cargo.toml').write_text('[package]')
        self.assertEqual(cleanup.sweep(self.root, self.cfg)['removed'], [])
        self.assertEqual((outside / 'important').read_text(), 'keep')
        self.assertTrue(cache.exists())

    def test_tracked_target_is_never_deleted(self):
        cache = self.cache()
        subprocess.run(['git', 'init', '-q', str(cache.parent)], check=True)
        subprocess.run(['git', '-C', str(cache.parent), 'add', 'target'], check=True)
        result = cleanup.sweep(self.root, self.cfg, now=time.time() + 9 * 3600)
        self.assertEqual(result['removed'], [])
        self.assertEqual(result['skipped'][str(cache)], 'contains tracked files')

    def test_dry_run_and_external_target_directory(self):
        cache = self.cache('validation-target', external=True)
        later = time.time() + 9 * 3600
        result = cleanup.sweep(self.root, self.cfg, dry_run=True, now=later)
        self.assertEqual(result['eligible'], [str(cache)])
        self.assertEqual(result['removed'], [])
        self.assertTrue(cache.exists())
        self.assertEqual(cleanup.sweep(self.root, self.cfg, now=later)['removed'], [str(cache)])

    def test_launch_is_throttled_and_configurable(self):
        with mock.patch.object(cleanup.subprocess, 'Popen') as launch:
            launch.return_value.pid = 123
            self.assertTrue(cleanup.maybe_start(self.root, self.cfg))
            self.assertFalse(cleanup.maybe_start(self.root, self.cfg))
            self.assertEqual(launch.call_count, 1)
            self.cfg['cache_cleanup'] = {'enabled': False}
            self.assertFalse(cleanup.maybe_start(self.root, self.cfg, force=True))

    def test_other_campaigns_prepared_workers_are_protected(self):
        cache = self.cache()
        current, other = self.root / 'pipeline-a', self.root / 'pipeline-b'
        current.mkdir(); other.mkdir()
        (current / 'config.json').write_text(json.dumps(self.cfg))
        (other / 'config.json').write_text(json.dumps({'implementers': {'i1': {'tree': str(cache.parent)}}}))
        result = cleanup.sweep(current, self.cfg)
        self.assertEqual(result['removed'], [])
        self.assertTrue(cache.exists())

    def test_maintenance_failure_does_not_fail_card_work(self):
        with mock.patch.object(cleanup, 'maybe_start', side_effect=OSError('read-only metadata')):
            self.assertFalse(cleanup.after_lane_work(self.root, self.cfg))

    def test_reuse_explicitly_unretires_read_only_warm_cache(self):
        cache = self.cache()
        cleanup.retire(self.root, cache)
        cleanup.reuse(self.root, cache)
        self.assertNotIn(str(cache), json.loads((self.root / 'cache-cleanup/retired.json').read_text()))
        self.assertTrue((cache / '.card-lane-last-used').is_file())
        self.assertEqual(cleanup.sweep(self.root, self.cfg)['removed'], [])

    def test_active_lease_covers_cargo_startup_and_read_only_use(self):
        cache = self.cache()
        cleanup.retire(self.root, cache)
        cleanup.reuse(self.root, cache)
        later = time.time() + 9 * 3600
        result = cleanup.sweep(self.root, self.cfg, now=later)
        self.assertEqual(result['removed'], [])
        self.assertEqual(result['skipped'][str(cache)], 'active cache lease')
        cleanup.release(self.root, cache)
        self.assertEqual(cleanup.sweep(self.root, self.cfg, now=later)['removed'], [str(cache)])

    def test_shared_lease_protects_another_campaign_and_concurrent_process(self):
        cache = self.cache()
        cleanup.reuse(self.root, cache)
        child = subprocess.Popen([sys.executable, '-c',
            'import sys; from pathlib import Path; import cache_leases; '
            'cache_leases.LEASE_DIR=Path(sys.argv[1]); cache_leases.acquire(Path(sys.argv[2])); '
            'print("leased",flush=True); sys.stdin.read(1)', str(cache_leases.LEASE_DIR), str(cache)],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        try:
            self.assertEqual(child.stdout.readline().strip(), 'leased')
            cleanup.release(self.root, cache)
            self.assertFalse(cleanup.retire(self.root, cache))
            self.assertNotIn(str(cache), json.loads((self.root / 'cache-cleanup/retired.json').read_text()))
            result = cleanup.sweep(self.root, self.cfg, now=time.time() + 9 * 3600)
            self.assertEqual(result['removed'], [])
            self.assertEqual(result['skipped'][str(cache)], 'active cache lease')
        finally:
            child.stdin.close(); child.wait(timeout=5); child.stdout.close()
        self.assertEqual(cleanup.sweep(self.root, self.cfg)['removed'], [])
        self.assertEqual(cleanup.sweep(self.root, self.cfg, now=time.time() + 9 * 3600)['removed'], [str(cache)])

    def test_symlink_scan_root_is_rejected(self):
        cache = self.cache()
        link = self.root / 'pool-link'
        link.symlink_to(self.pool, target_is_directory=True)
        self.cfg['cache_cleanup'] = {'worktree_roots': [str(link)]}
        result = cleanup.sweep(self.root, self.cfg, now=time.time() + 9 * 3600)
        self.assertEqual(result['status'], 'blocked')
        self.assertTrue(cache.exists())

    def test_external_cache_with_tracked_files_is_preserved(self):
        cache = self.cache('external-cache', external=True)
        subprocess.run(['git', 'init', '-q', str(self.pool)], check=True)
        subprocess.run(['git', '-C', str(self.pool), 'add', 'external-cache'], check=True)
        result = cleanup.sweep(self.root, self.cfg, now=time.time() + 9 * 3600)
        self.assertEqual(result['removed'], [])
        self.assertEqual(result['skipped'][str(cache)], 'contains tracked files')

    def test_unreadable_sibling_protection_stops_pool_cleanup(self):
        cache = self.cache()
        current, other = self.root / 'pipeline-a', self.root / 'pipeline-b'
        current.mkdir(); other.mkdir()
        (current / 'config.json').write_text(json.dumps(self.cfg))
        (other / 'config.json').write_text('{broken')
        result = cleanup.sweep(current, self.cfg, now=time.time() + 9 * 3600)
        self.assertEqual(result['status'], 'blocked')
        self.assertTrue(cache.exists())

    def test_optional_import_failure_cannot_escape_lane_hook(self):
        import builtins
        import review_lane
        original = builtins.__import__
        def missing(name, *args, **kwargs):
            if name == 'cache_cleanup':
                raise ImportError('optional maintenance unavailable')
            return original(name, *args, **kwargs)
        with mock.patch.object(builtins, '__import__', side_effect=missing):
            self.assertFalse(review_lane.maintain_caches(self.root, self.cfg))
            cache = self.cache()
            self.assertFalse(review_lane.maintain_caches(self.root, self.cfg, begin_target=cache))
        result = cleanup.sweep(self.root, self.cfg, now=time.time() + 9 * 3600)
        self.assertEqual(result['removed'], [])
        self.assertEqual(result['skipped'][str(cache)], 'active cache lease')


if __name__ == '__main__':
    unittest.main()
