import json
from pathlib import Path
import subprocess
import tempfile
import unittest

import export_lane as lane
import integration_lane
import review_lane


class ExportTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.card_names = []
        self.tree = self.root/'tree'
        self.tree.mkdir()
        self.git('init', '-q')
        self.git('config', 'user.name', 'Coordinator')
        self.git('config', 'user.email', 'coordinator@example.com')
        self.write('helper', 'old shared implementation')
        self.write('foundation-deleted.txt', 'obsolete')
        self.write('scenarios/alpha-obsolete.cantrip', '# removed during consolidation')
        self.base = self.save('baseline')
        self.git('switch', '-qc', 'integration/local')
        self.cfg = dict(workflow='consolidate_then_publish', base=self.base,
                        integration=dict(tree=str(self.tree), branch='integration/local', base=self.base,
                                         card_group_size=1))
        review_lane.save(self.root/'config.json', self.cfg)
        with integration_lane.connect(self.root):
            pass

    def tearDown(self):
        self.tmp.cleanup()

    def git(self, *args):
        return subprocess.check_output(['git', *args], cwd=self.tree, text=True, stderr=subprocess.STDOUT).strip()

    def write(self, path, contents):
        path = self.tree/path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents)

    def save(self, message, author=None):
        self.git('add', '.')
        args = ['commit', '-qm', message]
        if author:
            args += ['--author', author]
        self.git(*args)
        return self.git('rev-parse', 'HEAD')

    def card(self, name, issue):
        self.card_names.append(name)
        stem, slug = review_lane.names(name)
        self.write(f'content/mtg/cards/{slug}.json', json.dumps(dict(name=name)))
        self.write(f'crates/cantrip-cards/src/cards/{stem}.rs', 'isolated '+name)
        self.write(f'scenarios/{slug}-works.cantrip', f'[Bot1 Deck]\n{name}\n[Actions]\n')
        candidate = self.save('Card: '+name, f'{name} Author <{stem}@example.com>')
        with integration_lane.connect(self.root) as db:
            db.execute('INSERT INTO jobs(id,name,source_commit,source,status,enqueued,candidate) VALUES(?,?,?,?,?,?,?)',
                       (name, name, candidate, json.dumps(dict(name=name, issue=issue)), 'passed', 'now', candidate))
            db.execute('''INSERT INTO integration_jobs(id,name,candidate,source,status,steps,last_head,created)
                          VALUES(?,?,?,?,?,?,?,?)''',
                       (name, name, candidate, '{}', 'assembled', '[]', candidate, 'now'))
            db.commit()

    def finish(self):
        self.write('helper', 'canonical shared implementation')
        for name in self.card_names:
            stem, _ = review_lane.names(name)
            self.write(f'crates/cantrip-cards/src/cards/{stem}.rs', 'final canonical caller '+name.lower())
        (self.tree/'foundation-deleted.txt').unlink(missing_ok=True)
        (self.tree/'scenarios/alpha-obsolete.cantrip').unlink(missing_ok=True)
        artifact = self.tree/'foundation.bin'
        artifact.write_bytes(b'\x00\xffbinary\n')
        artifact.chmod(0o755)
        self.head = self.save('Consolidate final callers')
        snapshot = self.root/'snapshot.json'
        review_lane.save(snapshot, dict(head=self.head, validation=dict(ok=True, head=self.head)))
        self.cfg['local_base_manifest'] = str(snapshot)
        review_lane.save(self.root/'config.json', self.cfg)

    def ready(self):
        self.card('Alpha', 1)
        self.card('Beta', 2)
        self.finish()

    def test_builds_final_file_objects_with_original_authors_and_independent_sibling_parents(self):
        self.ready()
        plan = lane.plan(self.root, self.head)
        self.assertEqual(plan['blockers'], [])
        self.assertEqual(plan['groups'], [[0], [1]])
        before = self.git('status', '--porcelain')
        result = lane.build(self.root, plan, 'publish/test')
        self.assertEqual(before, self.git('status', '--porcelain'))
        self.assertEqual(self.git('rev-parse', 'HEAD'), self.head)
        foundation = result['foundation']
        self.assertEqual(self.git('rev-parse', foundation+'^'), self.base)
        self.assertEqual(self.git('show', foundation+':helper'), 'canonical shared implementation')
        self.assertNotIn('content/mtg/cards/alpha.json', lane.files(self.tree, foundation))
        self.assertEqual(lane.files(self.tree, foundation)['foundation.bin']['mode'], '100755')
        for group, name in zip(result['groups'], ('Alpha', 'Beta')):
            self.assertEqual(self.git('rev-parse', group['head']+'^'), foundation)
            self.assertEqual(self.git('show', group['head']+f':crates/cantrip-cards/src/cards/{name.lower()}.rs'),
                             'final canonical caller '+name.lower())
            self.assertEqual(self.git('show', '-s', '--format=%an', group['head']), name+' Author')
            self.assertIn(f'Closes #{1 if name == "Alpha" else 2}', self.git('show', '-s', '--format=%B', group['head']))
        self.assertEqual(lane.build(self.root, plan, 'publish/test'), result)

    def test_cross_card_scenarios_force_coherent_groups_and_never_enter_foundation(self):
        self.ready()
        self.write('scenarios/existing-card-with-both.cantrip', '[Bot1 Deck]\nAlpha\nBeta x2\n[Actions]\n')
        self.finish()
        plan = lane.plan(self.root, self.head)
        self.assertTrue(any('exceed group size' in blocker for blocker in plan['blockers']))
        self.assertNotIn('scenarios/existing-card-with-both.cantrip', plan['foundation_paths'])
        with self.assertRaisesRegex(ValueError, 'Export is held'):
            lane.build(self.root, plan, 'publish/oversized')
        self.cfg['integration']['card_group_size'] = 2
        review_lane.save(self.root/'config.json', self.cfg)
        plan = lane.plan(self.root, self.head)
        self.assertEqual(plan['groups'], [[0, 1]])
        self.assertEqual(plan['blockers'], [])

    def test_tampering_stale_approval_and_unvalidated_head_cannot_export(self):
        self.ready()
        plan = lane.plan(self.root, self.head)
        altered = dict(plan, groups=[])
        with self.assertRaisesRegex(ValueError, 'digest'):
            lane.build(self.root, altered, 'publish/tampered')
        with integration_lane.connect(self.root) as db:
            db.execute("UPDATE jobs SET status='blocked' WHERE name='Beta'")
            db.commit()
        with self.assertRaisesRegex(ValueError, 'receipts changed'):
            lane.build(self.root, plan, 'publish/stale')
        self.cfg.pop('local_base_manifest')
        review_lane.save(self.root/'config.json', self.cfg)
        self.assertTrue(any('validated local checkpoint' in blocker for blocker in lane.plan(self.root, self.head)['blockers']))
        self.assertEqual(self.git('for-each-ref', '--format=%(refname)', 'refs/heads/publish/'), '')

    def test_approved_upstream_revision_is_in_every_siblings_foundation(self):
        self.write('content/mtg/cards/alpha.json', json.dumps(dict(name='Alpha')))
        self.write('crates/cantrip-cards/src/cards/alpha.rs', 'upstream Alpha')
        self.base = self.save('Upstream implements Alpha')
        self.cfg['base'] = self.base
        self.cfg['integration']['base'] = self.base
        self.card('Alpha', 1)
        self.card('Beta', 2)
        self.write('scenarios/beta-works.cantrip', '[Bot1 Deck]\nAlpha\nBeta\n[Actions]\n')
        self.finish()
        plan = lane.plan(self.root, self.head)
        self.assertEqual(plan['blockers'], [])
        self.assertEqual([card['name'] for card in plan['cards']], ['Beta'])
        self.assertEqual([card['name'] for card in plan['foundation_cards']], ['Alpha'])
        self.assertEqual(plan['groups'], [[0]])
        result = lane.build(self.root, plan, 'publish/upstream-reference')
        beta = result['groups'][0]['head']
        self.assertEqual(self.git('show', beta+':crates/cantrip-cards/src/cards/alpha.rs'), 'final canonical caller alpha')
        self.assertEqual(self.git('show', result['foundation']+':crates/cantrip-cards/src/cards/alpha.rs'),
                         'final canonical caller alpha')
        self.assertEqual(plan['foundation_cards'][0]['author'][0], 'Alpha Author')
        self.assertIn('Co-authored-by: Alpha Author <alpha@example.com>',
                      self.git('show', '-s', '--format=%B', result['foundation']))
        self.assertNotIn('Closes', self.git('show', '-s', '--format=%B', result['foundation']))
        self.assertEqual(self.git('show', beta+':scenarios/beta-works.cantrip'),
                         '[Bot1 Deck]\nAlpha\nBeta\n[Actions]')

    def test_upstream_cards_new_scenario_travels_with_the_new_card_it_uses(self):
        self.write('content/mtg/cards/alpha.json', json.dumps(dict(name='Alpha')))
        self.base = self.save('Upstream implements Alpha')
        self.cfg['base'] = self.base
        self.cfg['integration']['base'] = self.base
        self.card('Alpha', 1)
        self.card('Beta', 2)
        self.write('scenarios/alpha-new-interaction.cantrip', '[Bot1 Deck]\nAlpha\nBeta\n[Actions]\n')
        self.finish()
        plan = lane.plan(self.root, self.head)
        self.assertEqual(plan['groups'], [[0]])
        self.assertEqual(plan['blockers'], [])
        self.assertIn('scenarios/alpha-new-interaction.cantrip', plan['cards'][0]['paths'])
        self.assertNotIn('scenarios/alpha-new-interaction.cantrip', plan['foundation_paths'])
        result = lane.build(self.root, plan, 'publish/upstream-new-scenario')
        self.assertNotIn('scenarios/alpha-new-interaction.cantrip', lane.files(self.tree, result['foundation']))
        self.assertIn('scenarios/alpha-new-interaction.cantrip', lane.files(self.tree, result['groups'][0]['head']))
        # Alpha's metadata alone existed upstream; its approved behavior must
        # still reach the foundation, covering the review's missing-module case.
        self.assertEqual(self.git('show', result['foundation']+':crates/cantrip-cards/src/cards/alpha.rs'),
                         'final canonical caller alpha')

    def existing_name_overlap(self, existing, new):
        stem, slug = review_lane.names(existing)
        self.write(f'content/mtg/cards/{slug}.json', json.dumps(dict(name=existing)))
        self.write(f'crates/cantrip-cards/src/cards/{stem}.rs', 'upstream '+existing)
        self.base = self.save('Upstream implements '+existing)
        self.cfg['base'] = self.base
        self.cfg['integration']['base'] = self.base
        self.card(existing, 1)
        self.card(new, 2)
        self.finish()
        plan = lane.plan(self.root, self.head)
        self.assertEqual(plan['blockers'], [])
        self.assertIn(f'scenarios/{slug}-works.cantrip', plan['foundation_paths'])
        _, new_slug = review_lane.names(new)
        self.assertIn(f'scenarios/{new_slug}-works.cantrip', plan['cards'][0]['paths'])
        self.assertNotIn(f'scenarios/{slug}-works.cantrip', plan['cards'][0]['paths'])

    def test_longer_existing_card_name_wins_over_new_prefix(self):
        self.existing_name_overlap('Desert Twister', 'Desert')

    def test_longer_new_card_name_wins_over_existing_prefix(self):
        self.existing_name_overlap('Desert', 'Desert Twister')

    def test_existing_different_branch_is_preserved_without_creating_other_refs(self):
        self.ready()
        self.git('branch', 'publish/collision/cards-002', self.base)
        with self.assertRaisesRegex(ValueError, 'Existing export branch differs'):
            lane.build(self.root, lane.plan(self.root, self.head), 'publish/collision')
        self.assertEqual(self.git('for-each-ref', '--format=%(refname)', 'refs/heads/publish/'),
                         'refs/heads/publish/collision/cards-002')
        self.assertEqual(self.git('rev-parse', 'publish/collision/cards-002'), self.base)

    def test_complete_card_names_win_over_shorter_scenario_prefixes(self):
        self.card('Desert', 1)
        self.card('Desert Twister', 2)
        self.finish()
        plan = lane.plan(self.root, self.head)
        self.assertEqual(plan['blockers'], [])
        self.assertIn('scenarios/desert-twister-works.cantrip', plan['cards'][1]['paths'])
        self.assertNotIn('scenarios/desert-twister-works.cantrip', plan['cards'][0]['paths'])

    def test_coherent_components_are_packed_without_avoidable_tiny_groups(self):
        names = ['A1', 'A2', 'A3', 'B1', 'B2', 'C1', 'C2', 'C3']
        for issue, name in enumerate(names, 1):
            self.card(name, issue)
        self.write('scenarios/a1-with-friends.cantrip', '[Bot1 Deck]\nA1\nA2\nA3\n[Actions]\n')
        self.write('scenarios/b1-with-friend.cantrip', '[Bot1 Deck]\nB1\nB2\n[Actions]\n')
        self.cfg['integration']['card_group_size'] = 4
        self.finish()
        plan = lane.plan(self.root, self.head)
        self.assertEqual(plan['blockers'], [])
        self.assertEqual([len(group) for group in plan['groups']], [4, 4])
        self.assertTrue(any({0, 1, 2}.issubset(group) for group in plan['groups']))
        self.assertTrue(any({3, 4}.issubset(group) for group in plan['groups']))


if __name__ == '__main__':
    unittest.main()
