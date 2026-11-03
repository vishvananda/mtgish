import subprocess
import tempfile
import unittest
from pathlib import Path

import pending_publications as p


class PendingPublicationTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.repo = Path(self.tmp.name)
        self.git('init', '-q')
        self.git('config', 'user.name', 'Publisher')
        self.git('config', 'user.email', 'publisher@example.com')
        self.base = self.commit('helper.rs', 'original', 'base')

    def tearDown(self):
        self.tmp.cleanup()

    def git(self, *args):
        return subprocess.check_output(['git', *args], cwd=self.repo, text=True).strip()

    def commit(self, path, contents, message):
        (self.repo/path).write_text(contents)
        self.git('add', '.'); self.git('commit', '-qm', message)
        return self.git('rev-parse', 'HEAD')

    def test_common_foundation_with_disjoint_cards_composes(self):
        foundation = self.commit('helper.rs', 'canonical recipe', 'Mechanics: canonical')
        left = self.commit('alpha.rs', 'alpha', 'Card: Alpha')
        self.git('checkout', '-q', '--detach', foundation)
        right = self.commit('beta.rs', 'beta', 'Card: Beta')
        result = p.check_heads(self.repo, 'card/alpha', left,
                               [dict(number=2, headRefName='card/beta', headRefOid=right)])
        self.assertTrue(result['ok'])
        self.assertEqual(self.git('status', '--porcelain'), '')

    def test_independently_changed_helper_blocks_an_otherwise_valid_batch(self):
        left = self.commit('helper.rs', 'one recipe', 'Mechanics: first')
        self.git('checkout', '-q', '--detach', self.base)
        right = self.commit('helper.rs', 'competing recipe', 'Mechanics: second')
        result = p.check_heads(self.repo, 'card/first', left,
                               [dict(number=7, headRefName='card/second', headRefOid=right)])
        self.assertFalse(result['ok'])
        self.assertEqual(result['checks'][0]['conflicts'], ['helper.rs'])
        self.assertEqual(self.git('status', '--porcelain'), '')

    def test_rewriting_a_pr_does_not_compare_it_with_itself(self):
        result = p.check_heads(self.repo, 'card/alpha', self.base,
                               [dict(number=1, headRefName='card/alpha', headRefOid='unknown')])
        self.assertTrue(result['ok'])
        self.assertEqual(result['checks'], [])

    def test_unpromoted_helper_is_held_even_if_its_name_would_not_conflict(self):
        new = self.commit('different-helper.rs', 'another spelling', 'Mechanics: private shared fork')
        with self.assertRaisesRegex(RuntimeError, 'need promotion'):
            p.require_promoted(self.repo, self.base, [new], dict(commits=[]))
        p.require_promoted(self.repo, self.base, [new], dict(commits=[new]))
        p.require_promoted(self.repo, self.base, [new], dict(commits=[]), [dict(commit=new)])

    def test_already_landed_and_cherry_picked_helpers_need_no_new_promotion(self):
        original = self.commit('helper.rs', 'canonical recipe', 'Mechanics: canonical')
        p.require_promoted(self.repo, original, [original], dict(commits=[]))
        self.git('checkout', '-q', '--detach', self.base)
        self.commit('unrelated', 'upstream work', 'upstream')
        self.git('cherry-pick', original)
        landed = self.git('rev-parse', 'HEAD')
        self.assertNotEqual(landed, original)
        p.require_promoted(self.repo, landed, [original], dict(commits=[]))

    def test_invalid_pending_head_fails_closed(self):
        with self.assertRaises(RuntimeError):
            p.check_heads(self.repo, 'card/alpha', self.base,
                          [dict(number=9, headRefName='card/other', headRefOid='unknown')])
