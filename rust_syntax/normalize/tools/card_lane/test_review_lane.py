import concurrent.futures
import json
import os
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

import review_lane as lane


class QueueTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def test_concurrent_claims_are_exactly_once(self):
        with lane.connect(self.root) as db:
            for i in range(60):
                lane.enqueue(db, {"name": f"Card {i}", "commit": str(i)})

        def consume(worker):
            claimed = []
            with lane.connect(self.root) as db:
                while job := lane.claim(db, str(worker)):
                    claimed.append(job["id"])
            return claimed

        with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
            ids = [item for group in pool.map(consume, range(8)) for item in group]
        self.assertEqual(len(ids), 60)
        self.assertEqual(len(set(ids)), 60)
        with lane.connect(self.root) as db:
            self.assertEqual(db.execute("SELECT count(*) FROM events WHERE kind='claimed'").fetchone()[0], 60)

    def test_sync_is_idempotent_per_name_and_revision(self):
        state = self.root / "producer"
        state.mkdir()
        manifest = self.root / "manifest.json"
        manifest.write_text(json.dumps({"base": "base", "workers": [{"worker": "p", "tree": "/producer", "state": str(state)}]}))
        lane.save(self.root / "config.json", {"producer_manifests": [str(manifest)]})
        queue = {"cards": [{"name": "A", "status": "committed", "commit": "one"},
                           {"name": "B", "status": "running", "commit": "ignored"}]}
        lane.save(state / "queue.json", queue)
        self.assertEqual(lane.sync(self.root), 1)
        self.assertEqual(lane.sync(self.root), 0)
        queue["cards"][0]["commit"] = "two"
        lane.save(state / "queue.json", queue)
        self.assertEqual(lane.sync(self.root), 1)
        with lane.connect(self.root) as db:
            self.assertEqual({r[0] for r in db.execute("SELECT source_commit FROM jobs")}, {"one", "two"})

    def test_missing_checks_cannot_pass(self):
        tree = self.root / "tree"
        tree.mkdir()
        folder = self.root / "packet"
        folder.mkdir()
        lane.save(self.root / "config.json", {"reviewers": {"slot": {"tree": str(tree)}},
                  "review_model": "test", "repairs": 0})
        source = {"name": "A", "commit": "one"}
        with lane.connect(self.root) as db:
            lane.enqueue(db, source)
        lane.save(folder / "review.json", {"decision": "pass", "validation": [{"agrees": True}],
                  "affected_cards": ["A"], "client": "covered"})

        class Usage:
            def record(self, model):
                return {}

        fake = SimpleNamespace(environment=lambda: {}, Usage=Usage,
                               Codex=lambda model, effort: SimpleNamespace(model=model),
                               spawn=lambda *args: (0, None, Usage()))
        with mock.patch.object(lane, "clean"), mock.patch.object(lane, "prepare", return_value=("branch", "parent", source)), \
             mock.patch.object(lane, "load_lane", return_value=fake), mock.patch.object(lane, "packet", return_value=folder), \
             mock.patch.object(lane, "prompt", return_value="review"), mock.patch.object(lane, "allowed_changes", return_value=[]), \
             mock.patch.object(lane, "work_digest", return_value="digest"), \
             mock.patch.object(lane, "checks", return_value={"ok": False}) as check, \
             mock.patch.dict(os.environ, {}, clear=False):
            lane.worker(self.root, "slot", once=True)
        check.assert_called_once()
        with lane.connect(self.root) as db:
            self.assertEqual(db.execute("SELECT status FROM jobs").fetchone()[0], "blocked")


class GitTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        self.git("init", "-q")
        self.git("config", "user.name", "Reviewer")
        self.git("config", "user.email", "reviewer@example.com")
        self.write("base", "base")
        self.git("add", ".")
        self.git("commit", "-qm", "base")
        self.base = self.git("rev-parse", "HEAD")
        self.write("crates/cantrip-cards/src/cards/test_card.rs", "original")
        self.git("add", ".")
        env = dict(os.environ, GIT_AUTHOR_NAME="Original Author", GIT_AUTHOR_EMAIL="author@example.com", GIT_AUTHOR_DATE="2020-01-02T03:04:05+00:00")
        self.git("commit", "-qm", "Card: Test Card", env=env)
        self.source = self.git("rev-parse", "HEAD")

    def tearDown(self):
        self.tmp.cleanup()

    def git(self, *args, env=None):
        return subprocess.check_output(["git", *args], cwd=self.repo, text=True, env=env).strip()

    def write(self, path, text):
        file = self.repo / path
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_text(text)

    def test_candidate_preserves_author_and_places_shared_commit_first(self):
        self.git("branch", "producer", self.source)
        producer_tree = self.git("rev-parse", "producer^{tree}")
        self.git("checkout", "-qb", "review")
        self.write("crates/cantrip-cards/src/cards/test_card.rs", "fixed")
        self.write("crates/cantrip-mechanics/src/shared.rs", "shared")
        paths = ["crates/cantrip-cards/src/cards/test_card.rs", "crates/cantrip-mechanics/src/shared.rs"]
        candidate = lane.commit_candidate(self.repo, {"name": "Test Card", "commit": self.source, "issue": 42}, self.base, paths)
        self.assertEqual(self.git("rev-parse", "producer"), self.source)
        self.assertEqual(self.git("rev-parse", "producer^{tree}"), producer_tree)
        self.assertEqual(self.git("show", "-s", "--format=%an <%ae>", candidate), "Original Author <author@example.com>")
        self.assertIn("Closes #42", self.git("show", "-s", "--format=%B", candidate))
        self.assertEqual(self.git("rev-list", "--count", f"{self.base}..{candidate}"), "2")
        self.assertEqual(self.git("show", "--format=", "--name-only", candidate + "^"), paths[1])
        self.assertEqual(self.git("show", "--format=", "--name-only", candidate), paths[0])
        self.assertEqual(self.git("status", "--porcelain"), "")

    def test_prepare_rejects_dirty_tree_without_losing_changes(self):
        self.write("untracked", "keep me")
        job = {"id": "1", "stage": "review", "source": json.dumps({"name": "Test Card", "commit": self.source})}
        with self.assertRaisesRegex(RuntimeError, "unfinished work preserved"):
            lane.prepare(self.root, self.repo, job)
        self.assertEqual((self.repo / "untracked").read_text(), "keep me")
        self.assertEqual(self.git("rev-parse", "HEAD"), self.source)

    def test_blocked_edits_are_parked_without_publishing_or_discarding(self):
        self.git("branch", "producer", self.source)
        self.git("checkout", "-qb", "review-blocked")
        path = "crates/cantrip-cards/src/cards/test_card.rs"
        self.write(path, "partial fix to retain")
        folder = self.root / "packet"
        folder.mkdir()
        result = {}
        lane.park_blocked(self.repo, {"name": "Test Card"}, folder, [path], result)
        self.assertEqual(self.git("status", "--porcelain"), "")
        self.assertEqual(self.git("rev-parse", "producer"), self.source)
        self.assertEqual(self.git("show", "HEAD:" + path), "partial fix to retain")
        self.assertIn("not publishable", self.git("show", "-s", "--format=%s", "HEAD"))
        self.assertEqual(result["wip_commit"], self.git("rev-parse", "HEAD"))
        self.assertIn("partial fix to retain", (folder / "blocked.patch").read_text())


if __name__ == "__main__":
    unittest.main()
