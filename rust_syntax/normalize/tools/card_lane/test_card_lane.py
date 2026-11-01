"""The lane's own logic, none of it needing a Cantrip tree.

    python3 rust_syntax/normalize/tools/card_lane/test_card_lane.py
"""
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.dont_write_bytecode = True
os.environ.setdefault("CARD_LANE", tempfile.mkdtemp(prefix="card-lane-test-"))
sys.path.insert(0, str(Path(__file__).resolve().parent))
import card_lane as lane  # noqa: E402

class Source(unittest.TestCase):
    def test_reminder_text_is_not_a_line_to_validate(self):
        self.assertEqual(lane.printed_lines("Flying (This can't be blocked.)\n({T}: Add {U}.)\nWhen this enters, draw a card."),
                         ["Flying", "When this enters, draw a card."])

    def test_a_definition_comes_with_its_doc_comment_and_ends_where_it_ends(self):
        with tempfile.TemporaryDirectory() as scratch:
            path = Path(scratch) / "effects.rs"
            path.write_text('/// Draws.\npub fn draw(g: X) -> Y {\n    if a {\n        b();\n    }\n}\n\nfn other() {}\npub const LAND: T = target(land_filter);\n')
            self.assertEqual(lane.definition(path, "draw"), "/// Draws.\npub fn draw(g: X) -> Y {\n    if a {\n        b();\n    }\n}")
            self.assertEqual(lane.definition(path, "LAND"), "pub const LAND: T = target(land_filter);")
            self.assertIsNone(lane.definition(path, "dr"))

    def test_a_cards_recipe_is_its_shared_call(self):
        self.assertEqual(lane.recipe_of("spell: spell(&[targets::CREATURE], effects::target_creature_gets::<3, 2>),"), "effects::target_creature_gets")
        self.assertEqual(lane.recipe_of("    destroy_target(g, item, 0);"), "destroy_target")
        self.assertIsNone(lane.recipe_of(""))

    def test_ability_text_is_the_last_string_of_the_call(self):
        code = 'abilities: &[\n  ability(cost("{1}"), go, "{1}: Draw a card."),\n  triggered(THIS_ENTERS, go, "When this enters, draw."),\n]'
        self.assertEqual([t for t, _ in lane.called_texts(code)], ["{1}: Draw a card.", "When this enters, draw."])

    def test_a_functions_doc_comment_travels_with_it(self):
        code = '/// "Draw a card." (CR 121.1)\n#[inline]\nfn draw(g: X) -> Y {\n    a();\n}\n\nfn bare() {}\n'
        found = {name: doc for name, doc, _ in lane.functions(code)}
        self.assertEqual(found, {"draw": '"Draw a card." (CR 121.1)', "bare": ""})


class Comparing(unittest.TestCase):
    def test_typography_is_not_a_difference(self):
        self.assertFalse(lane.oracle_differs("Don’t — draw", "Don't - draw"))

    def test_bracketed_loyalty_is_not_a_difference(self):
        self.assertFalse(lane.oracle_differs("+1: Draw.\n−2: Scry.", "[+1]: Draw.\n[−2]: Scry."))

    def test_words_are(self):
        self.assertTrue(lane.oracle_differs("Draw a card.", "Draw two cards."))

    def test_test_points_come_from_their_section(self):
        out = "=== Prior art\n  [1] not this\n\n=== Test points: each\n  [1] the amount 4: x\n  [2] 'you control': y\n\n=== wrote"
        self.assertEqual(lane.parse_test_points(out), ["[1] the amount 4: x", "[2] 'you control': y"])


class Packet(unittest.TestCase):
    def test_the_report_starts_with_every_test_point_and_is_not_done(self):
        report = lane.report_skeleton(["[1] the amount 4", "[2] 'you control'"])
        self.assertEqual(report["decision"], "todo")
        self.assertEqual([p["id"] for p in report["test_points"]], [1, 2])

    def test_rulings_come_with_their_dates_off(self):
        out = "  2 rulings:\n  - 2019-07-12  If Bishop dies at once.\n  - 2020-01-01  Another.\n\n=== Next\n"
        self.assertEqual(lane.parse_rulings(out), ["If Bishop dies at once.", "Another."])
        self.assertEqual(lane.report_skeleton([], ["One."])["rulings"][0]["ruling"], "One.")

    def test_the_report_lists_every_printed_line_to_validate(self):
        report = lane.report_skeleton([], [], ["Flying", "Draw a card."])
        self.assertEqual([v["line"] for v in report["validation"]], ["Flying", "Draw a card."])
        self.assertIsNone(report["validation"][0]["agrees"], "a line is not agreed until a worker says so")

    def test_only_a_worker_can_stop_a_card_with_a_verdict(self):
        self.assertIn("generator_bug", lane.STOPS)
        self.assertNotIn("done", lane.STOPS)


class Reporting(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory()
        self.addCleanup(self.scratch.cleanup)
        patcher = mock.patch.object(lane, "CANTRIP", Path(self.scratch.name))
        patcher.start()
        self.addCleanup(patcher.stop)
        scenarios = Path(self.scratch.name) / lane.SCENARIOS_DIR
        scenarios.mkdir()
        (scenarios / "a-card.cantrip").write_text("# Shows it.\n\n[Actions]\nBot1.Pass\n? Bot1.Life = 20\n[Expect]\nBot1.Life = 20\n")

    def test_a_scenario_is_named_with_or_without_its_extension(self):
        self.assertIsNone(lane.expectation_problem({"scenario": "a-card", "line": 5}, "x"))
        self.assertIsNone(lane.expectation_problem({"scenario": "a-card.cantrip", "line": 7}, "x"))

    def test_a_missing_scenario_is_not_called_a_bad_line(self):
        self.assertIn("no scenario", lane.expectation_problem({"scenario": "other", "line": 5}, "x"))

    def test_a_line_past_the_end_says_how_long_the_file_is(self):
        self.assertIn("which has 7", lane.expectation_problem({"scenario": "a-card", "line": 508}, "x"))

    def test_a_line_that_expects_nothing_is_refused(self):
        self.assertIn("no expectation", lane.expectation_problem({"scenario": "a-card", "line": 4}, "x"))


class Batches(unittest.TestCase):
    def setUp(self):
        self.names_of = lane.names_of
        lane.names_of = lambda name: (name.lower().replace(" ", "_"), name.lower().replace(" ", "-"), name.upper())
        lane.BATCH[:] = ["Angel", "Angel of Mercy"]

    def tearDown(self):
        lane.names_of = self.names_of
        lane.BATCH.clear()

    def test_a_scenario_belongs_to_the_longest_card_its_name_starts_with(self):
        self.assertEqual(lane.owner("angel-of-mercy-gains-three.cantrip"), "Angel of Mercy")
        self.assertEqual(lane.owner("angel-flies.cantrip"), "Angel")

    def test_a_scenario_of_no_card_in_the_batch_has_no_owner(self):
        self.assertIsNone(lane.owner("angels-grace.cantrip"))


class Keywords(unittest.TestCase):
    def setUp(self):
        self.tree = tempfile.TemporaryDirectory()
        self.path = Path(self.tree.name) / "card.json"
        self.content_file, self.core_keywords = lane.content_file, lane.core_keywords
        lane.content_file = lambda name: self.path
        lane.core_keywords = lambda: {"trample": "Trample", "banding": "Banding", "first strike": "FirstStrike"}

    def tearDown(self):
        lane.content_file, lane.core_keywords = self.content_file, self.core_keywords
        self.tree.cleanup()

    def repaired(self, card):
        self.path.write_text(json.dumps(card), encoding="utf-8")
        lane.repair_keywords("Card")
        return json.loads(self.path.read_text(encoding="utf-8")).get("keywords")

    def test_a_keyword_line_split_by_semicolons_keeps_its_keywords(self):
        self.assertEqual(self.repaired({"oracleText": "Trample; banding (Bands are blocked as a group.)"}), ["Trample", "Banding"])

    def test_a_keyword_already_there_is_not_added_twice(self):
        self.assertEqual(self.repaired({"oracleText": "First strike; banding", "keywords": ["FirstStrike"]}), ["FirstStrike", "Banding"])

    def test_a_sentence_with_a_semicolon_is_no_keyword_line(self):
        self.assertIsNone(self.repaired({"oracleText": "Choose one; trample."}))

    def test_a_keyword_it_cannot_play_is_left_out(self):
        self.assertEqual(self.repaired({"oracleText": "Protection from red; banding"}), ["Banding"])

    def test_each_keyword_is_a_change_that_takes_only_it_off(self):
        self.path.write_text(json.dumps({"name": "Card", "keywords": ["Trample", "Banding"]}), encoding="utf-8")
        missing = Path(self.tree.name) / "no-file.rs"
        card_file, lane.card_file = lane.card_file, lambda name: missing
        try:
            changes, total = lane.changes_of("Card", None)
        finally:
            lane.card_file = card_file
        self.assertEqual(total, 0)
        self.assertEqual([c["key"] for c in changes], ["keyword Trample", "keyword Banding"])
        self.assertEqual(json.loads(changes[0]["text"])["keywords"], ["Banding"])

    def test_the_last_keyword_taken_off_leaves_no_list(self):
        self.path.write_text(json.dumps({"name": "Card", "keywords": ["Trample"]}), encoding="utf-8")
        card_file, lane.card_file = lane.card_file, lambda name: Path(self.tree.name) / "no-file.rs"
        try:
            changes, _ = lane.changes_of("Card", None)
        finally:
            lane.card_file = card_file
        self.assertNotIn("keywords", json.loads(changes[0]["text"]))


class Prompt(unittest.TestCase):
    def test_a_keywords_first_word_comes_from_its_line(self):
        self.assertEqual(lane.keyword_words("Trample; banding (Bands are blocked.)\nCrew 2\nFlashback—{1}{G}, Pay 3 life.\nDraw a card."),
                         ["Trample", "banding", "Crew", "Flashback"])

    def test_a_sentence_has_no_keyword(self):
        self.assertEqual(lane.keyword_words("Draw a card, then discard a card."), [])

    def test_a_keywords_lines_come_shortest_first_and_each_once(self):
        with mock.patch.object(lane, "ACTION_LINES", ["Bot1.Hand.Long Named Card.Activate(Ability: Foretell)", "Bot1.Hand.X.Activate(Ability: Foretell)",
                                                      "Bot1.Hand.X.Activate(Ability: Foretell)", "Bot1.Pass"]):
            self.assertEqual(lane.keyword_lines("Foretell {1}{R}", wanted=1), ["- Foretell: `Bot1.Hand.X.Activate(Ability: Foretell)`"])


    def test_a_card_named_for_a_keyword_is_not_its_action(self):
        with mock.patch.object(lane, "ACTION_LINES", ["Bot1.Hand.Hire a Crew.Cast", "Bot1.Battlefield.Turtle Van.Activate(Ability: Crew)"]):
            self.assertEqual(lane.keyword_lines("Crew 2"), ["- Crew: `Bot1.Battlefield.Turtle Van.Activate(Ability: Crew)`"])

    def test_a_card_passes_on_a_refusal_last(self):
        with mock.patch.object(lane, "ACTION_LINES", ["! Bot1.Hand.A.Suspend", "Bot1.Hand.Lotus Bloom.Suspend"]):
            self.assertEqual(lane.keyword_lines("Suspend 3", wanted=1), ["- Suspend: `Bot1.Hand.Lotus Bloom.Suspend`"])


class RememberedMutants(unittest.TestCase):
    def setUp(self):
        self.change = {"key": "line 4: 1 to 2", "text": "mutated", "path": None, "detail": "d"}

    def held(self, verdict, together="now", said="a", now="a", text="mutated"):
        known = {"change": lane.digest(text), "verdict": verdict, "by": "s", "said": said, "together": "now"}
        with mock.patch.object(lane, "scenario_digest", return_value=now):
            return lane.verdict_of(self.change, known, together)

    def test_nothing_known_is_played(self):
        self.assertIsNone(lane.verdict_of(self.change, None, "now"))

    def test_caught_holds_while_its_scenario_says_the_same(self):
        self.assertEqual(self.held("caught"), "caught")

    def test_an_edited_catcher_plays_it_again(self):
        self.assertIsNone(self.held("caught", now="b"))

    def test_a_deleted_catcher_plays_it_again(self):
        self.assertIsNone(self.held("caught", now=None))

    def test_a_crash_with_no_name_is_played_again(self):
        known = {"change": lane.digest("mutated"), "verdict": "caught", "by": None, "said": None, "together": "now"}
        self.assertIsNone(lane.verdict_of(self.change, known, "now"))

    def test_a_survivor_waits_for_new_scenarios(self):
        self.assertEqual(self.held("SURVIVED"), "SURVIVED")
        self.assertIsNone(self.held("SURVIVED", together="new"))

    def test_a_change_to_the_mutant_plays_it_again(self):
        self.assertIsNone(self.held("caught", text="another"))

    def test_what_does_not_compile_never_will(self):
        self.assertEqual(self.held("no build", together="new"), "no build")

    def run_gate(self, known):
        """mutation_gates over one card with two changes, on a scratch tree, the build and the scenarios faked."""
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            source = root / "card.rs"
            source.write_text("real", encoding="utf-8")
            content = root / "card.json"
            content.write_text("{}", encoding="utf-8")
            (root / "state").mkdir()
            (root / "state" / "mutants.json").write_text(json.dumps(known), encoding="utf-8")
            changes = [{"key": k, "text": k, "path": source, "detail": f"{k} detail"} for k in ("one", "two")]
            builds = []
            gates = {"A": mock.Mock()}
            gates["A"].results = [{}]
            with mock.patch.multiple(lane, tool_module=mock.Mock(), changes_of=mock.Mock(return_value=(changes, 2)), card_file=mock.Mock(return_value=source),
                                     content_file=mock.Mock(return_value=content), card_dir=mock.Mock(return_value=root / "state"),
                                     scenarios_digest=mock.Mock(return_value="now"), scenario_digest=mock.Mock(return_value="said"),
                                     build_corpus=mock.Mock(side_effect=lambda: builds.append(source.read_text(encoding="utf-8")) or (0, "")),
                                     corpus_verdict=mock.Mock(return_value=(1, "s"))), mock.patch("builtins.print"):
                survivors = lane.mutation_gates(["A"], gates)
            return survivors, builds, source.read_text(encoding="utf-8"), json.loads((root / "state" / "mutants.json").read_text(encoding="utf-8"))

    def test_only_what_is_unknown_is_built(self):
        known = {"one": {"change": lane.digest("one"), "verdict": "caught", "by": "s", "said": "said", "together": "now"}}
        survivors, builds, source, memo = self.run_gate(known)
        self.assertEqual(survivors, {"A": []})
        self.assertEqual(builds, ["two", "real"])  # the one left, then the card put back
        self.assertEqual(source, "real")
        self.assertEqual(memo["two"]["verdict"], "caught")

    def test_a_kept_survivor_is_still_reported_and_nothing_is_built(self):
        known = {k: {"change": lane.digest(k), "verdict": "SURVIVED", "by": None, "said": None, "together": "now"} for k in ("one", "two")}
        survivors, builds, _, _ = self.run_gate(known)
        self.assertEqual(survivors, {"A": [("one", "one detail"), ("two", "two detail")]})
        self.assertEqual(builds, [])


class Repairs(unittest.TestCase):
    def failed(self):
        card = mock.Mock(ok=False)
        card.failures.return_value = [{"gate": "corpus", "detail": "line 3 failed"}]
        return {"A": card}

    def test_a_worker_is_told_to_keep_going_and_play_what_it_changes(self):
        text = lane.batch_repair(self.failed(), "draft", 3)
        for words in ("keep going", "Play a scenario after you change it", "Do not finish with a failure you know of", "batch-check --fast", "3 more rounds", "line 3 failed"):
            self.assertIn(words, text)
        self.assertNotIn("one action", text)

    def test_the_last_round_says_so(self):
        self.assertIn("after 1 more round a card", lane.batch_repair(self.failed(), "draft", 1).replace("\n", " "))

    def test_a_card_that_named_nothing_failing_is_caught_all_the_same(self):
        with mock.patch.object(lane, "shell", return_value=(1, "panic: boom")):
            self.assertEqual(lane.corpus_verdict("A"), (1, None))

    def test_the_first_failure_is_named(self):
        with mock.patch.object(lane, "shell", return_value=(1, "  FAIL first-one\nx\n  FAIL second\n")):
            self.assertEqual(lane.corpus_verdict("A"), (1, "first-one"))


class Ways(unittest.TestCase):
    def test_a_worker_with_the_tools_is_told_to_finish_the_job(self):
        self.assertIn("keep going until `batch-check --fast` passes for every card", lane.WAYS["work"])

    def test_no_way_rations_a_workers_actions(self):
        for way in lane.WAYS.values():
            self.assertNotIn("one action", way)


class BlindStops(unittest.TestCase):
    def cleared(self, reports):
        with tempfile.TemporaryDirectory() as scratch:
            path = Path(scratch) / "reports.json"
            path.write_text(json.dumps(reports), encoding="utf-8")
            lane.clear_blind_stops(path)
            return json.loads(path.read_text(encoding="utf-8"))

    def test_unproved_is_no_stop(self):
        out = self.cleared({"A": {"decision": "needs_review", "reason": "needs validation"}, "B": {"decision": "engine_gap", "reason": "x"}})
        self.assertEqual(out["A"], {"decision": "done", "notes": "needs validation"})
        self.assertEqual(out["B"]["decision"], "done")

    def test_a_generator_bug_found_by_reading_stands(self):
        report = {"decision": "generator_bug", "reason": "the amount is 2, the text says 3"}
        self.assertEqual(self.cleared({"A": report}), {"A": report})

    def test_a_missing_or_broken_file_is_left(self):
        lane.clear_blind_stops(Path("/nonexistent/reports.json"))


class Sharing(unittest.TestCase):
    def test_a_function_that_is_a_shared_effect_over_again_is_named(self):
        with tempfile.TemporaryDirectory() as scratch:
            effects = Path(scratch) / "crates" / "cantrip-mechanics" / "src" / "effects.rs"
            effects.parent.mkdir(parents=True)
            effects.write_text("pub fn destroy_first_target(g: X, item: Y) -> Z {\n    crate::destroy_target(g, item, 0);\n    Ok(())\n}\n")
            with mock.patch.object(lane, "CANTRIP", Path(scratch)):
                card = "fn effect(g: X, item: Y) -> Z {\n    destroy_target(g, item, 0);\n    Ok(())\n}\n"
                self.assertEqual(lane.shared_copies(card), ["fn effect is effects::destroy_first_target"])
                self.assertEqual(lane.shared_copies(card.replace("destroy_target", "damage_target")), [])


class Commands(unittest.TestCase):
    def test_the_corpus_takes_its_own_flags_with_or_without_a_separator(self):
        for argv in (["corpus", "--card", "X", "--brief"], ["corpus", "--", "--card", "X", "--brief"]):
            with mock.patch.object(lane.subprocess, "run") as run, mock.patch.object(sys, "argv", ["card_lane.py", *argv]):
                run.return_value.returncode = 0
                self.assertEqual(lane.main(), 0)
                self.assertEqual(run.call_args[0][0][-3:], ["--card", "X", "--brief"])
                self.assertEqual(run.call_args[0][0].count("--"), 1)


class Usage(unittest.TestCase):
    def test_cached_input_is_not_charged_twice(self):
        usage = lane.Usage()
        usage.input, usage.cached, usage.output = 1_000_000, 900_000, 100_000
        self.assertEqual(usage.cost("gpt-6-luna"), round(0.01 + 0.009 + 0.05, 4))

    def test_a_resumed_codex_session_reports_its_totals_so_they_are_not_added_up(self):
        total, first, second = lane.Usage(), lane.Usage(), lane.Usage()
        first.input, first.cached, first.output, first.turns = 3_000, 2_000, 200, 1
        second.input, second.cached, second.output, second.turns = 3_500, 2_400, 210, 1
        total.add(first, lane.Codex.cumulative)
        total.add(second, lane.Codex.cumulative)
        self.assertEqual((total.input, total.cached, total.output, total.turns), (3_500, 2_400, 210, 2))

    def test_a_cumulative_run_that_reported_nothing_changes_nothing(self):
        total, first = lane.Usage(), lane.Usage()
        first.input, first.turns = 3_000, 1
        total.add(first, True)
        total.add(lane.Usage(), True)
        self.assertEqual(total.input, 3_000)

    def test_a_claude_run_is_its_own(self):
        total, one = lane.Usage(), lane.Usage()
        one.input, one.turns = 100, 1
        total.add(one, lane.Claude.cumulative)
        total.add(one, lane.Claude.cumulative)
        self.assertEqual(total.input, 200)

    def test_a_reported_cost_wins_and_an_unknown_model_has_none(self):
        usage = lane.Usage()
        self.assertIsNone(usage.cost("unknown-model"))
        usage.reported_cost = 1.5
        self.assertEqual(usage.cost("unknown-model"), 1.5)

    def test_codex_reads_session_and_turns(self):
        backend, usage = lane.Codex(None, None), lane.Usage()
        self.assertEqual(backend.read('{"type":"thread.started","thread_id":"t1"}', usage), "t1")
        backend.read('{"type":"turn.completed","usage":{"input_tokens":10,"cached_input_tokens":4,"output_tokens":2}}', usage)
        self.assertEqual((usage.input, usage.cached, usage.output, usage.turns), (10, 4, 2, 1))

    def test_claude_reads_its_result(self):
        backend, usage = lane.Claude(None, None), lane.Usage()
        line = '{"type":"result","session_id":"s1","num_turns":3,"total_cost_usd":0.5,"usage":{"input_tokens":5,"cache_read_input_tokens":7,"output_tokens":2}}'
        self.assertEqual(backend.read(line, usage), "s1")
        self.assertEqual((usage.input, usage.cached, usage.turns, usage.reported_cost), (12, 7, 3, 0.5))


if __name__ == "__main__":
    unittest.main()
