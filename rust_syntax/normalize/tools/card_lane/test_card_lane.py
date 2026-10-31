"""The lane's own logic, none of it needing a Cantrip tree.

    python3 rust_syntax/normalize/tools/card_lane/test_card_lane.py
"""
import os
import sys
import tempfile
import unittest
from pathlib import Path

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


class Usage(unittest.TestCase):
    def test_cached_input_is_not_charged_twice(self):
        usage = lane.Usage()
        usage.input, usage.cached, usage.output = 1_000_000, 900_000, 100_000
        self.assertEqual(usage.cost("gpt-6-luna"), round(0.01 + 0.009 + 0.05, 4))

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
