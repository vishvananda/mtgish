import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import control


class ControlTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory()
        self.root=Path(self.temp.name)/'campaign with spaces'
        self.cantrip=Path(self.temp.name)/'cantrip'
        self.pool=Path(self.temp.name)/'pool with spaces'

    def tearDown(self):
        self.temp.cleanup()

    def configure(self):
        with patch.object(control.review,'git',side_effect=['a'*40,'b'*40]):
            return control.initialize(self.root,Path(self.temp.name)/'lane',self.cantrip,self.pool,
                                      'main',2,3,2,'example-model')

    def test_fresh_campaign_has_safe_defaults_and_complete_slots(self):
        cfg=self.configure()
        self.assertFalse(cfg['enabled'])
        self.assertFalse(cfg['publisher']['enabled'])
        self.assertEqual(set(cfg['implementers']),{'i1','i2'})
        self.assertEqual(set(cfg['reviewers']),{'r1','r2','r3','v1','v2'})
        self.assertEqual(cfg['reviewers']['v2']['role'],'confirm')
        self.assertEqual(cfg['cards_per_context'],20)
        queue=json.loads((self.root/'implementation-queue.json').read_text())
        self.assertEqual(queue['generator_commit'],'b'*40)
        self.assertEqual(queue['cards'],[])
        self.assertEqual(cfg['integration']['branch'],'integration/'+cfg['unit_prefix'])

    def test_initialization_preserves_existing_state(self):
        self.configure()
        before=(self.root/'config.json').read_bytes()
        with self.assertRaisesRegex(ValueError,'already exists'):
            self.configure()
        self.assertEqual((self.root/'config.json').read_bytes(),before)

    def test_dry_start_never_launches_or_enables_workers(self):
        cfg=self.configure()
        with patch.object(control.subprocess,'run') as run:
            commands=control.start(self.root,cfg,True)
        run.assert_not_called()
        self.assertEqual(len(commands),10)  # 7 workers, watcher, integration, CPU monitor
        self.assertTrue(all(str(control.HERE) in ' '.join(cmd) for cmd in commands))
        self.assertFalse(json.loads((self.root/'config.json').read_text())['enabled'])
        self.assertFalse(any('publish_lane.py' in ' '.join(cmd) for cmd in commands))

    def test_services_use_campaign_prefix_and_skip_draining_workers(self):
        cfg=self.configure();cfg['reviewers']['r2']['draining']=True
        planned=list(control.services(self.root,cfg))
        self.assertTrue(all(s['unit'].startswith(cfg['unit_prefix']) for s in planned))
        self.assertFalse(any(s['unit'].endswith('-review-r2') for s in planned))
        monitor=next(s for s in planned if s['unit'].endswith('-resources'))
        self.assertEqual(monitor['command'][-3:],['--unit-prefix',cfg['unit_prefix'],'watch'])
        self.assertNotEqual(control.unit_prefix(self.root,{}),control.unit_prefix(self.root.parent/'other'/self.root.name,{}))

    def test_drain_preserves_unrelated_configuration(self):
        cfg=self.configure();cfg['unrelated']={'keep':True};control.review.save(self.root/'config.json',cfg)
        control.set_enabled(self.root,True);control.set_enabled(self.root,False)
        after=control.review.config(self.root)
        self.assertEqual(after['unrelated'],{'keep':True})
        self.assertFalse(after['enabled'])

    def test_implementation_hold_leaves_reviews_and_global_enablement_unchanged(self):
        self.configure();control.set_enabled(self.root,True)
        control.set_implementation_enabled(self.root,False)
        held=control.review.config(self.root)
        self.assertTrue(held['enabled'])
        self.assertFalse(held['implementation_enabled'])
        self.assertEqual(len(list(control.services(self.root,held))),10)
        control.set_implementation_enabled(self.root,True)
        self.assertTrue(control.review.config(self.root)['implementation_enabled'])


if __name__=='__main__':
    unittest.main()
