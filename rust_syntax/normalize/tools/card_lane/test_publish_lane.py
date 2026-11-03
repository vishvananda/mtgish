import json
import sqlite3
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import publish_lane as p


class PublisherTests(unittest.TestCase):
    def test_counter_merge_retains_ids_and_appends_distinct_kinds(self):
        base='prefix\ncounter_kinds! {\n    A = 0, "a", "A";\n}\nsuffix\n'
        ours=base.replace('\n}\n','\n    Vitality = 1, "vitality", "Vitality";\n}\n')
        theirs=base.replace('\n}\n','\n    Scream = 1, "scream", "Scream";\n}\n')
        merged=p.merge_counter_additions(base,ours,theirs)
        self.assertIn('Vitality = 1',merged)
        self.assertIn('Scream = 2',merged)
        self.assertEqual(p.merge_counter_additions(base,merged,theirs),merged)
        wind=theirs.replace('\n}\n','\n    Wind = 2, "wind", "Wind";\n}\n')
        self.assertIn('Wind = 3',p.merge_counter_additions(theirs,merged,wind))
        with self.assertRaises(ValueError):p.merge_counter_additions(base,ours,theirs.replace('suffix','changed'))
        with self.assertRaises(ValueError):p.merge_counter_additions(base,ours,theirs.replace('A = 0','A = 2'))
        with self.assertRaises(ValueError):p.merge_counter_additions(base,ours,theirs.replace('"a"','"other"'))

    def test_counter_conflict_is_resolved_without_replacing_existing_kind(self):
        path=self.repo/'crates/cantrip-state/src/vocab.rs';path.parent.mkdir(parents=True)
        base='prefix\ncounter_kinds! {\n    A = 0, "a", "A";\n}\nsuffix\n'
        path.write_text(base);self.git('add','.');self.git('commit','-qm','counter base')
        parent=self.git('rev-parse','HEAD')
        path.write_text(base.replace('\n}\n','\n    Scream = 1, "scream", "Scream";\n}\n'))
        self.git('add','.');self.git('commit','-qm','Engine: scream counter');incoming=self.git('rev-parse','HEAD')
        self.git('checkout','-q','--detach',parent)
        path.write_text(base.replace('\n}\n','\n    Vitality = 1, "vitality", "Vitality";\n}\n'))
        self.git('add','.');self.git('commit','-qm','Engine: vitality counter')
        p.pick_prerequisite(self.repo,incoming)
        self.assertIn('Vitality = 1',path.read_text());self.assertIn('Scream = 2',path.read_text())
        self.assertEqual(self.git('status','--porcelain'),'')

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.repo = self.root / 'repo'; self.repo.mkdir()
        self.git('init', '-q')
        self.git('config', 'user.name', 'Publisher')
        self.git('config', 'user.email', 'publisher@example.com')
        (self.repo / 'base').write_text('base')
        self.git('add', '.'); self.git('commit', '-qm', 'base')
        self.base = self.git('rev-parse', 'HEAD')
        self.db = p.connect(self.root)
        self.cfg = {'base': self.base, 'lane_root': str(self.root), 'publisher': {'enabled': True, 'batch_size': 2, 'repo': str(self.repo), 'upstream_repo': 'org/repo', 'fork_owner': 'me', 'worktree_root': str(self.root)}}

    def tearDown(self):
        self.db.close(); self.tmp.cleanup()

    def git(self, *args, env=None):
        return subprocess.check_output(['git', *args], cwd=self.repo, text=True, env=env).strip()

    def add_job(self, name, issue, sha, job=None):
        source = json.dumps({'name': name, 'issue': issue, 'commit': sha})
        self.db.execute("INSERT INTO jobs(id,name,source_commit,source,status,enqueued,candidate) VALUES(?,?,?,?,?,?,?)", (job or name, name, sha, source, 'passed', 'now', sha))
        self.db.commit()

    def candidate(self, name, issue):
        self.git('checkout', '-q', '--detach', self.base)
        (self.repo / 'mechanic').write_text('shared')
        self.git('add', '.'); self.git('commit', '-qm', 'Engine: shared')
        (self.repo / name).write_text(name)
        self.git('add', '.')
        self.git('-c', 'user.name=Original Author', '-c', 'user.email=author@example.com', 'commit', '-qm', f'Card: {name}\n\nCloses #{issue}')
        return self.git('rev-parse', 'HEAD')

    def test_reservation_order_and_uniqueness(self):
        self.add_job('Z', 4, 'a'); self.add_job('A', 2, 'b'); self.add_job('C', 3, 'c')
        number = p.reserve(self.db, self.cfg)
        batch, cards = p.batch(self.db, number)
        self.assertEqual([c['name'] for c in cards], ['A', 'C'])
        self.assertEqual(batch['branch'], 'card/unlimited-20261103-batch-001')
        self.assertIsNone(p.reserve(self.db, self.cfg))
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO publish_cards(batch,position,job,name,issue,candidate) VALUES(1,3,'other','A',7,'head')")
        self.db.rollback()
        folder = p.manifest(self.root, batch, cards, self.cfg)
        p.manifest(self.root, batch, cards, self.cfg)
        self.assertEqual(json.loads((folder / 'manifest.json').read_text())['cards'], cards)

    def test_composition_deduplicates_prerequisites_and_preserves_card_authors(self):
        a = self.candidate('Alpha', 1); b = self.candidate('Beta', 2)
        cards = [dict(name='Alpha', issue=1, candidate=a), dict(name='Beta', issue=2, candidate=b)]
        shared, finals = p.composition(self.repo, self.base, cards)
        self.assertEqual(len(shared), 1)
        self.assertEqual(finals, [a, b])
        self.git('checkout', '-q', '--detach', self.base)
        for sha in shared + finals:
            self.git('cherry-pick', sha)
        self.assertEqual(self.git('log', '-1', '--format=%an'), 'Original Author')
        self.assertIn('Closes #2', self.git('log', '-1', '--format=%B'))

    def test_mixed_pins_import_helpers_without_importing_upstream_card_history(self):
        a = self.candidate('Alpha', 1)
        shared = self.git('rev-parse', a+'^')
        self.git('checkout', '-q', '--detach', shared)
        (self.repo/'upstream-card').write_text('landed independently')
        self.git('add', '.'); self.git('commit', '-qm', 'Card: Already upstream')
        (self.repo/'new-helper').write_text('second helper')
        self.git('add', '.'); self.git('commit', '-qm', 'Mechanics: second helper')
        pin = self.git('rev-parse', 'HEAD')
        (self.repo/'Beta').write_text('Beta')
        self.git('add', '.'); self.git('commit', '-qm', 'Card: Beta\n\nCloses #2')
        b = self.git('rev-parse', 'HEAD')
        cards = [dict(name='Alpha',issue=1,candidate=a),
                 dict(name='Beta',issue=2,candidate=b,source_base=pin,
                      dependencies=[dict(commit=pin,pr_url='https://github.com/org/repo/pull/7')])]
        prerequisites, finals = p.composition(self.repo, self.base, cards)
        self.assertEqual(prerequisites,[shared,pin])
        self.assertEqual(finals,[a,b])
        self.assertIn('https://github.com/org/repo/pull/7',p.pr_body(cards, prerequisites, []))

    def test_reservation_freezes_per_card_pins_and_helper_dependencies(self):
        self.add_job('Alpha',1,'candidate'); self.add_job('Beta',2,'other')
        source=dict(name='Alpha',issue=1,commit='candidate',publication_base='pin',
                    shared_dependencies=[dict(commit='helper',pr_url='https://github.com/org/repo/pull/7')])
        self.db.execute('UPDATE jobs SET source=? WHERE name=?',(json.dumps(source),'Alpha'));self.db.commit()
        number=p.reserve(self.db,self.cfg)
        b,cards=p.batch(self.db,number)
        folder=p.manifest(self.root,b,cards,self.cfg)
        self.cfg['shared_base_manifest']='a-newer-pin'
        self.assertEqual(p.batch(self.db,number)[1],cards)
        self.assertEqual(cards[0]['source_base'],'pin')
        self.assertEqual(cards[0]['dependencies'],source['shared_dependencies'])
        self.assertEqual(json.loads((folder/'manifest.json').read_text())['cards'],cards)

    def test_validation_failure_never_pushes_or_opens_pr(self):
        self.add_job('Alpha', 1, self.base); self.add_job('Beta', 2, self.base)
        num = p.reserve(self.db, self.cfg)
        calls = []
        def fake_git(repo, *args, **kw):
            calls.append(args)
            if args[:2] == ('rev-parse', 'origin/main^{commit}'): return self.base
            if args[:2] == ('rev-parse', 'HEAD'): return self.base
            return ''
        def fake_command(repo, *args, **kw):
            calls.append(args)
            if args[:2] == ('git', 'show-ref'): return 1, ''
            return 0, ''
        with mock.patch.object(p, 'validate_remote'), mock.patch.object(p, 'composition', return_value=([], [])), mock.patch.object(p, 'git', side_effect=fake_git), mock.patch.object(p, 'command', side_effect=fake_command), mock.patch.object(p, 'integration_checks', side_effect=RuntimeError('failed checks')):
            with self.assertRaisesRegex(RuntimeError, 'failed checks'):
                p.publish(self.root, self.cfg, self.db, num)
        self.assertEqual(p.batch(self.db, num)[0]['status'], 'blocked')
        self.assertFalse(any('push' in c or 'gh' in c for c in calls))

    def test_recovery_after_create_uses_existing_pr(self):
        self.add_job('Alpha', 1, self.base); self.add_job('Beta', 2, self.base)
        num = p.reserve(self.db, self.cfg)
        p.update(self.db, num, status='creating', base=self.base, head=self.base)
        calls = []
        def fake_git(repo, *args, **kw):
            calls.append(args)
            if args[:2] == ('rev-parse', 'HEAD'): return self.base
            if args[0] == 'ls-remote': return self.base + '\trefs/heads/test'
            return ''
        def fake_command(repo, *args, **kw):
            calls.append(args)
            return 0, json.dumps([{'url': 'https://example/pr/1', 'headRefOid': self.base, 'headRepositoryOwner': {'login': 'me'}}])
        with mock.patch.object(p, 'validate_remote'), mock.patch.object(p, 'git', side_effect=fake_git), mock.patch.object(p, 'command', side_effect=fake_command):
            p.publish(self.root, self.cfg, self.db, num)
        self.assertEqual(p.batch(self.db, num)[0]['pr_url'], 'https://example/pr/1')
        self.assertFalse(any('create' in c or 'push' in c for c in calls))

    def test_frozen_base_survives_config_change(self):
        self.add_job('Alpha', 1, self.base); self.add_job('Beta', 2, self.base)
        num = p.reserve(self.db, self.cfg)
        self.cfg['base'] = 'different'
        b, cards = p.batch(self.db, num)
        self.assertEqual(b['source_base'], self.base)
        folder = p.manifest(self.root, b, cards, self.cfg)
        self.assertEqual(json.loads((folder / 'manifest.json').read_text())['source_base'], self.base)
        with mock.patch.object(p, 'validate_remote'), mock.patch.object(p, 'composition', side_effect=RuntimeError('stop')) as compose:
            with self.assertRaisesRegex(RuntimeError, 'stop'):
                p.publish(self.root, self.cfg, self.db, num)
        self.assertEqual(compose.call_args.args[1], self.base)

    def test_changed_approval_stops_before_external_actions(self):
        a = self.candidate('Alpha', 1); b = self.candidate('Beta', 2)
        self.add_job('Alpha', 1, a); self.add_job('Beta', 2, b)
        num = p.reserve(self.db, self.cfg)
        self.db.execute("UPDATE jobs SET status='blocked' WHERE name='Alpha'"); self.db.commit()
        with mock.patch.object(p, 'command', wraps=p.command) as command, mock.patch.object(p, 'git', wraps=p.git) as git:
            with self.assertRaisesRegex(RuntimeError, 'approval changed'):
                p.publish(self.root, self.cfg, self.db, num)
        self.assertEqual(command.call_count, 0)
        self.assertEqual(git.call_count, 0)
        self.assertEqual(p.batch(self.db, num)[0]['status'], 'blocked')

    def test_changed_candidate_stops_before_push(self):
        a = self.candidate('Alpha', 1); b = self.candidate('Beta', 2)
        self.add_job('Alpha', 1, a); self.add_job('Beta', 2, b)
        num = p.reserve(self.db, self.cfg)
        p.update(self.db, num, status='validated', base=self.base, head=b)
        self.db.execute("UPDATE jobs SET candidate=? WHERE name='Alpha'", (self.base,)); self.db.commit()
        with mock.patch.object(p, 'validate_remote'), mock.patch.object(p, 'git', side_effect=lambda repo, *args, **kw: b if args[:2] == ('rev-parse', 'HEAD') else '') as git, mock.patch.object(p, 'command') as command:
            with self.assertRaisesRegex(RuntimeError, 'approval changed'):
                p.publish(self.root, self.cfg, self.db, num)
        self.assertFalse(any('push' in call.args for call in git.call_args_list))
        command.assert_not_called()
        self.assertEqual(p.batch(self.db, num)[0]['status'], 'blocked')

    def test_push_succeeded_then_client_failure_recovers_without_second_push(self):
        self.add_job('Alpha', 1, self.base); self.add_job('Beta', 2, self.base)
        num = p.reserve(self.db, self.cfg)
        p.update(self.db, num, status='validated', base=self.base, head=self.base)
        remote = {'head': None, 'pushes': 0}
        def fake_git(repo, *args, **kw):
            if args[:2] == ('rev-parse', 'HEAD'): return self.base
            if args[0] == 'ls-remote': return self.base + '\trefs/heads/test' if remote['head'] else ''
            if args[0] == 'push':
                remote['head'] = self.base; remote['pushes'] += 1
                raise SystemExit('process died after successful push')
            return ''
        def fake_command(repo, *args, **kw):
            if args[:3] == ('gh', 'pr', 'list'):
                return 0, json.dumps([{'url': 'https://example/pr/1', 'headRefOid': self.base, 'headRepositoryOwner': {'login': 'me'}}])
            raise AssertionError(args)
        with mock.patch.object(p, 'validate_remote'), mock.patch.object(p, 'git', side_effect=fake_git), mock.patch.object(p, 'command', side_effect=fake_command):
            with self.assertRaises(SystemExit):
                p.publish(self.root, self.cfg, self.db, num)
            self.assertEqual(p.batch(self.db, num)[0]['status'], 'pushing')
            p.publish(self.root, self.cfg, self.db, num)
        self.assertEqual(remote['pushes'], 1)
        self.assertEqual(p.batch(self.db, num)[0]['status'], 'published')

    def test_pr_query_failure_is_retryable_and_wrong_head_blocks(self):
        self.add_job('Alpha', 1, self.base); self.add_job('Beta', 2, self.base)
        num = p.reserve(self.db, self.cfg)
        p.update(self.db, num, status='pushed', base=self.base, head=self.base)
        def fake_git(repo, *args, **kw):
            if args[:2] == ('rev-parse', 'HEAD'): return self.base
            if args[0] == 'ls-remote': return self.base + '\trefs/heads/test'
            return ''
        with mock.patch.object(p, 'validate_remote'), mock.patch.object(p, 'git', side_effect=fake_git), mock.patch.object(p, 'command', side_effect=RuntimeError('network error')):
            with self.assertRaises(p.ExternalOperationError):
                p.publish(self.root, self.cfg, self.db, num)
        self.assertEqual(p.batch(self.db, num)[0]['status'], 'pushed')
        def wrong_git(repo, *args, **kw):
            if args[0] == 'ls-remote': return 'different\trefs/heads/test'
            return fake_git(repo, *args, **kw)
        with mock.patch.object(p, 'validate_remote'), mock.patch.object(p, 'git', side_effect=wrong_git):
            with self.assertRaisesRegex(RuntimeError, 'different head'):
                p.publish(self.root, self.cfg, self.db, num)
        self.assertEqual(p.batch(self.db, num)[0]['status'], 'blocked')


if __name__ == '__main__':
    unittest.main()
