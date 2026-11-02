"""The lane's own logic, none of it needing a Cantrip tree.

    python3 rust_syntax/normalize/tools/card_lane/test_card_lane.py
"""
import json
import os
import subprocess
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

    def changes(self, card):
        self.path.write_text(json.dumps(card), encoding="utf-8")
        missing = Path(self.tree.name) / "no-file.rs"
        with mock.patch.multiple(lane, card_file=lambda name: missing, names_of=lambda name: ("card", "card", "CARD")):
            return lane.changes_of("Card", None)

    def test_each_keyword_is_a_change_that_takes_only_it_off(self):
        changes, total = self.changes({"name": "Card", "keywords": ["Trample", "Banding"]})
        self.assertEqual(total, 0)
        self.assertEqual([c["key"] for c in changes], ["keyword Trample", "keyword Banding"])
        self.assertIn("Keywords::of(&[Keyword::Banding])", changes[0]["text"])
        self.assertIn("Keywords::of(&[Keyword::Trample])", changes[1]["text"])

    def test_the_last_keyword_taken_off_leaves_none(self):
        changes, _ = self.changes({"name": "Card", "keywords": ["Trample"]})
        self.assertIn("Keywords::of(&[])", changes[0]["text"])

    def test_a_word_the_oracle_crate_ignores_has_nothing_to_take_off(self):
        changes, _ = self.changes({"name": "Card", "keywords": ["Trample", "Equip"]})
        self.assertEqual([c["key"] for c in changes], ["keyword Trample"])
        self.assertIn("Keywords::of(&[])", changes[0]["text"])


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
            shadow = mock.Mock()
            shadow.return_value.ready.return_value = []  # so the tree's bench, whose build and play are faked here
            with mock.patch.multiple(lane, Shadow=shadow, tool_module=mock.Mock(), changes_of=mock.Mock(return_value=(changes, 2)), card_file=mock.Mock(return_value=source),
                                     content_file=mock.Mock(return_value=content), card_dir=mock.Mock(return_value=root / "state"),
                                     scenarios_digest=mock.Mock(return_value="now"), scenario_digest=mock.Mock(return_value="said"),
                                     build_corpus=mock.Mock(side_effect=lambda: builds.append(source.read_text(encoding="utf-8")) or (0, "")),
                                     corpus_verdict=mock.Mock(return_value=(1, "s"))), mock.patch("builtins.print"):
                survivors = lane.mutation_gates(["A"], gates)
            return survivors, builds, source.read_text(encoding="utf-8"), json.loads((root / "state" / "mutants.json").read_text(encoding="utf-8"))

    def test_only_what_is_unknown_is_built(self):
        known = {lane.digest("one"): {"change": lane.digest("one"), "verdict": "caught", "by": "s", "said": "said", "together": "now"}}
        survivors, builds, source, memo = self.run_gate(known)
        self.assertEqual(survivors, {"A": []})
        self.assertEqual(builds, ["two", "real"])  # the one left, then the card put back
        self.assertEqual(source, "real")
        self.assertEqual(memo[lane.digest("two")]["verdict"], "caught")

    def test_a_kept_survivor_is_still_reported_and_nothing_is_built(self):
        known = {lane.digest(k): {"change": lane.digest(k), "verdict": "SURVIVED", "by": None, "said": None, "together": "now"} for k in ("one", "two")}
        survivors, builds, _, _ = self.run_gate(known)
        self.assertEqual(survivors, {"A": [("one", "one detail"), ("two", "two detail")]})
        self.assertEqual(builds, [])


class ShadowTree(unittest.TestCase):
    def tree(self, root):
        """A small Cantrip: two crates, three cards, two scenarios."""
        cards = root / "real" / lane.CARDS_DIR
        cards.mkdir(parents=True)
        (root / "real" / "crates" / "cantrip-cards" / "Cargo.toml").write_text("x", encoding="utf-8")
        (root / "real" / "crates" / "cantrip-cards" / "src" / "lib.rs").write_text("x", encoding="utf-8")
        (root / "real" / "crates" / "cantrip-engine").mkdir()
        (root / "real" / "Cargo.toml").write_text("x", encoding="utf-8")
        (root / "real" / ".git").mkdir()
        (root / "real" / "target").mkdir()
        (cards / "alpha.rs").write_text("prepare: Some(&crate::beta::CARD)", encoding="utf-8")
        (cards / "beta.rs").write_text("pub static CARD", encoding="utf-8")
        (cards / "gamma.rs").write_text("pub static CARD", encoding="utf-8")
        (cards / "filler.rs").write_text("pub static CARD", encoding="utf-8")
        (cards / "provider.rs").write_text("pub static EXTRA: &[&CardDef] = &[&tokens::MAP];\n", encoding="utf-8")
        (cards / "another.rs").write_text("pub static EXTRA: &[&CardDef] = &[&tokens::MAP, &LOCAL];\n", encoding="utf-8")
        scenarios = root / "real" / "scenarios"
        scenarios.mkdir()
        (scenarios / "a.cantrip").write_text("# x\n[Bot1 Deck]\nAlpha\nFiller x3\n[Bot2 Deck]\n2 Island\n[Actions]\nGamma\n", encoding="utf-8")
        (scenarios / "b.cantrip").write_text("[Bot1 Deck]\nGamma\n[Actions]\n", encoding="utf-8")
        return root / "real"

    def shadow(self, root, names=("Alpha",)):
        real = self.tree(root)
        with mock.patch.multiple(lane, CANTRIP=real, SHADOW=root / "shadow", names_of=mock.Mock(side_effect=lambda n: (n.lower(), n.lower(), n.upper()))):
            return real, lane.Shadow(list(names))

    def test_it_holds_the_cards_played_and_what_they_reach(self):
        with tempfile.TemporaryDirectory() as scratch:
            _, bench = self.shadow(Path(scratch))
            self.assertEqual(sorted(p.name for p in (bench.tree / lane.CARDS_DIR).glob("*.rs")), ["alpha.rs", "another.rs", "beta.rs", "filler.rs"])

    def test_the_cards_are_copies_and_the_rest_are_links(self):
        with tempfile.TemporaryDirectory() as scratch:
            real, bench = self.shadow(Path(scratch))
            self.assertFalse((bench.tree / lane.CARDS_DIR / "alpha.rs").is_symlink())
            self.assertTrue((bench.tree / "scenarios").is_symlink())
            self.assertTrue((bench.tree / "crates" / "cantrip-engine").is_symlink())
            self.assertTrue((bench.tree / "crates" / "cantrip-cards" / "Cargo.toml").is_symlink())
            self.assertFalse((bench.tree / ".git").exists())
            self.assertFalse((bench.tree / "target").exists())

    def test_a_change_lands_in_the_shadow_and_never_in_the_tree(self):
        with tempfile.TemporaryDirectory() as scratch:
            real, bench = self.shadow(Path(scratch))
            with mock.patch.object(lane, "CANTRIP", real):
                place = bench.place(real / lane.CARDS_DIR / "alpha.rs")
            self.assertEqual(place, bench.tree / lane.CARDS_DIR / "alpha.rs")

    def test_a_card_no_longer_played_goes(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            real, bench = self.shadow(root)
            with mock.patch.multiple(lane, CANTRIP=real, SHADOW=root / "shadow", names_of=mock.Mock(side_effect=lambda n: (n.lower(), n.lower(), n.upper()))):
                lane.Shadow(["Gamma"])
            self.assertEqual(sorted(p.name for p in (bench.tree / lane.CARDS_DIR).glob("*.rs")), ["another.rs", "gamma.rs"])

    def test_a_shadow_of_another_tree_starts_again(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            real, bench = self.shadow(root)
            moved = root / "moved"
            real.rename(moved)
            with mock.patch.multiple(lane, CANTRIP=moved, SHADOW=root / "shadow", names_of=mock.Mock(side_effect=lambda n: (n.lower(), n.lower(), n.upper()))):
                lane.Shadow(["Alpha"])
            self.assertEqual((bench.tree / "Cargo.toml").resolve(), (moved / "Cargo.toml").resolve())

    def test_a_card_the_shadow_cannot_play_goes_to_the_tree(self):
        gates = {}
        shadow = mock.Mock()
        shadow.return_value.ready.return_value = ["A"]
        with mock.patch.object(lane, "Shadow", shadow), mock.patch.object(lane, "break_cards", side_effect=lambda names, g, bench: {n: bench for n in names}) as broken, mock.patch("builtins.print"):
            out = lane.mutation_gates(["A", "B"], gates)
        self.assertIs(out["A"], shadow.return_value)
        self.assertIsInstance(out["B"], lane.Bench)
        self.assertEqual(broken.call_count, 2)

    def test_ready_needs_every_scenario_found_and_passing(self):
        bench = lane.Shadow.__new__(lane.Shadow)
        bench.target, bench.tree = Path("/t"), Path("/t")
        said = {"Good": (0, "3 scenarios: 3 pass, 0 fail, 0 known bugs, 0 need cards the engine does not have."),
                "Missing": (0, "3 scenarios: 2 pass, 0 fail, 0 known bugs, 1 need cards the engine does not have."),
                "Fails": (1, "3 scenarios: 2 pass, 1 fail, 0 known bugs, 0 need cards the engine does not have."),
                "None": (0, "14108 scenarios: 0 pass, 0 fail, 0 known bugs, 0 need cards the engine does not have.")}
        with mock.patch.object(lane.Shadow, "build", return_value=(0, "")), mock.patch.object(lane, "shell", side_effect=lambda *a, **k: said[a[2]]):
            self.assertEqual(bench.ready(list(said)), ["Good"])


class DeckNames(unittest.TestCase):
    def test_every_way_of_saying_a_card(self):
        text = "# Mox Ruby in a comment\n[Bot1 Deck]\nMountain\n2 Lightning Bolt\nIsland x20\nIsland X3\n[Bot1 Sideboard]\nFury\n[Bot2 Companion]\nLurrus\n[Life]\nBot1 7\n[Actions]\nBot1.Hand.Fury.Cast\n"
        self.assertEqual(lane.deck_names(text), ["Mountain", "Lightning Bolt", "Island", "Island", "Fury", "Lurrus"])


class KeywordChanges(unittest.TestCase):
    CORE = {"Flying", "Trample"}

    def change(self, source, kept, name="A Card"):
        with mock.patch.object(lane, "names_of", return_value=("a_card", "a-card", "A_CARD")):
            return lane.without_keyword(name, source, kept, self.CORE)

    def test_a_card_with_a_file_keeps_its_file(self):
        source = "use x;\npub static CARD: CardDef = CardDef {\n    spell: None,\n    ..oracle::A_CARD\n};\n"
        text = self.change(source, ["Flying"])
        self.assertIn("keywords: Keywords::of(&[Keyword::Flying]), toxic: 0,\n    ..oracle::A_CARD", text)
        self.assertIn("spell: None", text)

    def test_a_card_with_no_file_gets_one_of_that_field(self):
        text = self.change(None, [])
        self.assertIn("keywords: Keywords::of(&[]), toxic: 0, ..oracle::A_CARD", text)
        self.assertIn("pub static CARD", text)

    def test_toxic_stays_a_number(self):
        self.assertIn("toxic: 3", self.change(None, ["Toxic 1", "Toxic 2"]))

    def test_a_file_taking_nothing_from_the_oracle_cannot_change(self):
        self.assertIsNone(self.change("pub static CARD: CardDef = CardDef { name: \"X\" };", []))


class TwoChangesOnOneLine(unittest.TestCase):
    def test_each_keeps_its_own_verdict(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            source = root / "card.rs"
            source.write_text("real", encoding="utf-8")
            content = root / "card.json"
            content.write_text("{}", encoding="utf-8")
            (root / "state").mkdir()
            changes = [{"key": "line 4: 1 to 2", "text": text, "path": source, "detail": "d"} for text in ("first", "second")]
            builds = []
            gates = {"A": mock.Mock()}
            gates["A"].results = [{}]
            shadow = mock.Mock()
            shadow.return_value.ready.return_value = []
            with mock.patch.multiple(lane, Shadow=shadow, tool_module=mock.Mock(), changes_of=mock.Mock(return_value=(changes, 2)), card_file=mock.Mock(return_value=source),
                                     content_file=mock.Mock(return_value=content), card_dir=mock.Mock(return_value=root / "state"),
                                     scenarios_digest=mock.Mock(return_value="now"), scenario_digest=mock.Mock(return_value="said"),
                                     build_corpus=mock.Mock(side_effect=lambda: builds.append(source.read_text(encoding="utf-8")) or (0, "")),
                                     corpus_verdict=mock.Mock(return_value=(1, "s"))), mock.patch("builtins.print"):
                lane.mutation_gates(["A"], gates)
                self.assertEqual(builds, ["first", "second", "real"])
                builds.clear()
                lane.mutation_gates(["A"], gates)
                self.assertEqual(builds, [])


class Repairs(unittest.TestCase):
    def failed(self):
        card = mock.Mock(ok=False)
        card.failures.return_value = [{"gate": "corpus", "detail": "line 3 failed"}]
        return {"A": card}

    def test_a_worker_is_told_to_keep_going_and_play_what_it_changes(self):
        text = lane.batch_repair(self.failed(), "draft", 3)
        for words in ("keep going", "Play a scenario after you change it", "Do not finish with a failure you know of", "whole `batch-check`", "3 more rounds", "line 3 failed"):
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
        self.assertIn("keep going until `batch-check` passes for every card", lane.WAYS["work"])
        self.assertIn("the last check is the whole `batch-check`", lane.WAYS["work"].replace("\n", " "))

    def test_no_way_rations_a_workers_actions(self):
        for way in lane.WAYS.values():
            self.assertNotIn("one action", way)


class PointsNoGameCanShow(unittest.TestCase):
    def problems(self, pair):
        report = {"validation": [], "test_points": [dict(pair, id=1)], "rulings": [], "look_answers": [], "equivalent_mutants": []}
        with mock.patch.multiple(lane, report_of=mock.Mock(return_value=report), content_file=mock.Mock(return_value=mock.Mock(read_text=mock.Mock(return_value='{"oracleText": ""}'))),
                                 expectation_problem=mock.Mock(return_value="test point 1: no scenario")):
            return lane.report_problems("A", ["the order"], [], [])

    def test_a_reason_answers_it(self):
        self.assertEqual(self.problems({"no_decision": "a search and its shuffle all end the same"}), [])

    def test_a_word_is_not_a_reason(self):
        self.assertEqual(self.problems({"no_decision": "n/a"}), ["test point 1: no scenario"])

    def test_nothing_at_all_still_fails(self):
        self.assertEqual(self.problems({}), ["test point 1: no scenario"])

    def test_a_sentence_in_the_short_report_becomes_the_reason(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            (root / "reports.json").write_text(json.dumps({"A": {"decision": "done", "points": ["no game shows a search before its shuffle"]}}), encoding="utf-8")
            (root / "report.skeleton.json").write_text(json.dumps({"validation": [], "test_points": [{"id": 1}], "rulings": []}), encoding="utf-8")
            with mock.patch.multiple(lane, BATCH_DIR=root, card_dir=mock.Mock(return_value=root)):
                lane.expand_reports(["A"])
            written = json.loads((root / "report.json").read_text(encoding="utf-8"))
        self.assertEqual(written["test_points"][0]["no_decision"], "no game shows a search before its shuffle")


class EveryCardIsBroken(unittest.TestCase):
    def test_a_card_with_an_unfinished_report_is_broken_all_the_same(self):
        gate = mock.Mock(ok=True)
        contexts = {"source": "", "points": []}
        broken = mock.Mock(return_value={"A": []})
        with mock.patch.multiple(lane, Gates=mock.Mock(return_value=gate), build_corpus=mock.Mock(return_value=(0, "")), early_gates=mock.Mock(return_value=contexts),
                                 coverage_gates=mock.Mock(return_value=["A"]), report_gate=mock.Mock(), finish=mock.Mock(),
                                 report_problems=mock.Mock(return_value=["test point 1 unpaired"]), mutation_gates=broken):
            lane.batch_gates(["A"])
        broken.assert_called_once()


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


TABLE = '''macro_rules! counter_kinds {
    ($($kind:ident = $n:literal, $name:literal, $token:literal;)*) => {};
}

counter_kinds! {
    PlusOne = 0, "+1/+1", "PlusOnePlusOne";
    Charge = 1, "charge", "Charge";
}

pub struct After;
'''


class Counters(unittest.TestCase):
    def test_a_card_wants_the_counters_it_uses_and_cantrip_has_no_line_for(self):
        code = ('let a = CounterKind::Spore; let b = CounterKind::Charge; for k in CounterKind::ALL {} let c = CounterKind::Spore;'
                ' let d = planned_value!("counter kind Rad", CounterKind::Charge => CounterKind::Rad);')
        self.assertEqual(lane.counters_wanted(code, TABLE), ["Spore"])

    def test_lines_run_on_from_the_last_number_and_leave_the_rest_alone(self):
        table = lane.with_counters(TABLE, ["Spore", "Verse"])
        self.assertIn('    Charge = 1, "charge", "Charge";\n    Spore = 2, "spore", "Spore";\n    Verse = 3, "verse", "Verse";\n}\n\npub struct After;', table)
        self.assertEqual(lane.counters_wanted("CounterKind::Verse", table), [])

    def repository(self, root):
        table = root / lane.COUNTER_TABLE
        table.parent.mkdir(parents=True)
        table.write_text(TABLE, encoding="utf-8")
        (root / "card.rs").write_text("CounterKind::Verse", encoding="utf-8")

        def shell(*command, **_):
            done = subprocess.run([str(c) for c in command], cwd=root, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
            return done.returncode, done.stdout
        identity = ("-c", "user.name=lane", "-c", "user.email=lane@example.org")
        shell("git", "init", "-q")
        shell("git", *identity, "add", lane.COUNTER_TABLE)
        shell("git", *identity, "commit", "-q", "-m", "start")
        shell("git", "config", "user.name", "lane")
        shell("git", "config", "user.email", "lane@example.org")
        return shell

    def test_a_commit_comes_from_head_so_a_card_set_aside_leaves_no_gap_in_the_numbers(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            shell = self.repository(root)
            with mock.patch.object(lane, "CANTRIP", root), mock.patch.object(lane, "shell", shell), \
                    mock.patch.object(lane, "card_file", lambda _: root / "card.rs"):
                lane.add_counters("CounterKind::Spore CounterKind::Verse")
                worktree = (root / lane.COUNTER_TABLE).read_text(encoding="utf-8")
                self.assertTrue(lane.only_counter_lines())
                lane.commit_counters("Some Card")
                self.assertEqual((root / lane.COUNTER_TABLE).read_text(encoding="utf-8"), worktree)
                committed = lane.git("show", f"HEAD:{lane.COUNTER_TABLE}")
                self.assertIn('    Verse = 2, "verse", "Verse";', committed)
                self.assertNotIn("Spore", committed)
                self.assertEqual(lane.git("log", "-1", "--format=%s").strip(), "Engine: the verse counter (CR 122.1), for Some Card")
                self.assertTrue(lane.only_counter_lines())
                lane.commit_counters("Some Card")
                self.assertEqual(lane.git("rev-list", "--count", "HEAD").strip(), "2")

    def test_anything_else_in_the_table_is_out_of_scope(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            shell = self.repository(root)
            with mock.patch.object(lane, "CANTRIP", root), mock.patch.object(lane, "shell", shell):
                (root / lane.COUNTER_TABLE).write_text(TABLE.replace("pub struct After;", "pub struct Before;"), encoding="utf-8")
                self.assertFalse(lane.only_counter_lines())


if __name__ == "__main__":
    unittest.main()
