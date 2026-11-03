import json
from pathlib import Path
import subprocess
import tempfile
import unittest

import shared_base


class SharedBaseTests(unittest.TestCase):
    def test_pin_requires_validated_linear_helpers_and_excludes_card_commits(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo=Path(tmp)
            def git(*args):
                return subprocess.check_output(['git',*args],cwd=repo,text=True).strip()
            git('init','-q');git('config','user.name','A');git('config','user.email','a@example.com')
            (repo/'base').write_text('base');git('add','.');git('commit','-qm','base')
            base=git('rev-parse','HEAD')
            (repo/'helper').write_text('shared');git('add','.');git('commit','-qm','Mechanics: shared')
            head=git('rev-parse','HEAD')
            pin=dict(base=base,head=head,commits=[head],pr_url='https://github.com/org/repo/pull/1',
                     validation=dict(ok=True,head=head))
            path=repo/'pin.json';cfg=dict(shared_base_manifest=str(path))
            path.write_text(json.dumps(pin))
            self.assertEqual(shared_base.read_pin(cfg,repo),pin)
            self.assertEqual(shared_base.metadata(pin)['publication_base'],head)
            pin['validation']['head']=base;path.write_text(json.dumps(pin))
            with self.assertRaisesRegex(ValueError,'matching successful'):shared_base.read_pin(cfg,repo)
            pin['validation']['head']=head
            pin['commits']=[];path.write_text(json.dumps(pin))
            with self.assertRaisesRegex(ValueError,'exactly'):shared_base.read_pin(cfg,repo)
            (repo/'card').write_text('card');git('add','card');git('commit','-qm','Card: test')
            card=git('rev-parse','HEAD');pin.update(head=card,commits=[head,card],validation=dict(ok=True,head=card))
            path.write_text(json.dumps(pin))
            with self.assertRaisesRegex(ValueError,'only helper'):shared_base.read_pin(cfg,repo)

    def test_absent_pin_preserves_the_existing_pipeline_base(self):
        self.assertIsNone(shared_base.read_pin({},Path('/unused')))
