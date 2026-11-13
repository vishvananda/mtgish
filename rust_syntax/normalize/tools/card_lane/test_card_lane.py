"""The lane's own logic, none of it needing a Cantrip tree.

    python3 rust_syntax/normalize/tools/card_lane/test_card_lane.py
"""
import io
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
import cantrip_tree as tree  # noqa: E402

class Source(unittest.TestCase):
    def test_isolated_file_import_finds_local_helpers(self):
        script = ("import importlib.util, sys; "
                  "spec = importlib.util.spec_from_file_location('isolated_lane', sys.argv[1]); "
                  "module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module); "
                  "assert module.context_budget.__file__")
        result = subprocess.run([sys.executable, '-I', '-c', script, lane.__file__], cwd='/tmp',
                                text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr)

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
    def test_shared_packet_includes_trigger_and_static_definitions(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch); mechanics = root/'crates/cantrip-mechanics/src'; mechanics.mkdir(parents=True)
            (mechanics/'triggers.rs').write_text('/// Color cast.\npub fn colored<const C: u8>(q: X) -> Y {\n    fired(q)\n}\n')
            (mechanics/'statics.rs').write_text('/// Layer recipe.\npub fn pump(q: X) -> Y {\n    chars(q)\n}\n')
            with mock.patch.object(lane, 'CANTRIP', root):
                result = lane.shared_code('cantrip_mechanics::triggers::colored::<3>; statics::pump;')
            self.assertIn('Color cast.', result)
            self.assertIn('Layer recipe.', result)
            self.assertEqual(result.count('## colored'), 1)

    def test_shared_packet_resolves_same_named_helpers_in_their_own_modules(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch); mechanics = root/'crates/cantrip-mechanics/src'; mechanics.mkdir(parents=True)
            (mechanics/'effects.rs').write_text('pub fn same() {\n    effect();\n}\n')
            (mechanics/'triggers.rs').write_text('pub fn same() {\n    trigger();\n}\n')
            with mock.patch.object(lane, 'CANTRIP', root):
                result = lane.shared_code('effects::same; triggers::same; effects::same;')
            self.assertIn('effect();', result)
            self.assertIn('trigger();', result)
            self.assertEqual(result.count('## same'), 2)

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

    def test_a_level_s_keyword_is_not_the_card_s(self):
        escort = "Level up {2} ({2}: Put a level counter on this. Level up only as a sorcery.)\nLEVEL 1-4\n2/2\nLEVEL 5+\n5/5\nFirst strike"
        self.assertIsNone(self.repaired({"oracleText": escort, "keywords": ["FirstStrike"]}))
        self.assertIsNone(self.repaired({"oracleText": "Station\n8+ | Trample", "keywords": ["Trample"]}))

    def test_a_keyword_printed_outside_every_stage_stays(self):
        self.assertEqual(self.repaired({"oracleText": "Trample\nLEVEL 2+\n3/3\nTrample, first strike", "keywords": ["Trample", "FirstStrike"]}),
                         ["Trample"])

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
        self.assertEqual(survivors, {"A": [{"key": "one", "detail": "one detail"}, {"key": "two", "detail": "two detail"}]})
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

    def test_a_broken_card_may_take_ten_times_its_own_scenarios(self):
        bench = lane.Shadow.__new__(lane.Shadow)
        bench.target, bench.tree = Path("/t"), Path("/t")
        clock = iter([0.0, 12.0, 100.0, 101.0])
        passed = (0, "3 scenarios: 3 pass, 0 fail, 0 known bugs, 0 need cards the engine does not have.")
        with mock.patch.object(lane.Shadow, "build", return_value=(0, "")), mock.patch.object(lane, "shell", return_value=passed), \
                mock.patch.object(lane.time, "time", side_effect=lambda: next(clock)), \
                mock.patch.object(lane.multiprocessing.pool, "ThreadPool", lambda n: mock.MagicMock(__enter__=lambda s: mock.Mock(map=map))):
            bench.ready(["Slow", "Quick"])
        self.assertEqual(bench.limits, {"Slow": 120.0, "Quick": 30})
        with mock.patch.object(lane, "corpus_verdict") as verdict:
            bench.play("Slow")
        self.assertEqual(verdict.call_args.args[3], 120.0)


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


class CapacityRetries(unittest.TestCase):
    def attempt(self, path, capacity=True):
        with path.open('a') as log:
            event = {'type': 'turn.failed', 'error': {'message': 'Selected model is at capacity. Please try a different model.'}} if capacity else {'type': 'turn.completed'}
            log.write(json.dumps(event) + '\n')
        return (1 if capacity else 0), 'same-session', lane.Usage()

    def test_capacity_retries_resume_the_session_and_keep_work(self):
        with tempfile.TemporaryDirectory() as scratch:
            path=Path(scratch)/'worker.jsonl'
            work=Path(scratch)/'scenario.cantrip';work.write_text('preserved')
            backend=mock.Mock(cumulative=True)
            calls=[]
            def run(backend, card, prompt, session, timeout, log):
                calls.append(session)
                return self.attempt(log,capacity=len(calls)<=3)
            with mock.patch.object(lane,'spawn_once',side_effect=run), mock.patch.object(lane.time,'sleep') as sleep:
                code,session,used=lane.spawn(backend,{},'continue',None,1000,path)
            self.assertEqual((code,session),(0,'same-session'))
            self.assertEqual(calls,[None,'same-session','same-session','same-session'])
            self.assertEqual([c.args[0] for c in sleep.call_args_list],[30,60,60])
            self.assertIsNone(used.infrastructure_error)
            self.assertEqual(work.read_text(),'preserved')

    def test_capacity_exhaustion_is_infrastructure_and_preserves_real_card_stops(self):
        with tempfile.TemporaryDirectory() as scratch:
            path=Path(scratch)/'worker.jsonl'
            with mock.patch.object(lane,'spawn_once',side_effect=lambda *args:self.attempt(path)), mock.patch.object(lane.time,'sleep'):
                code,_,used=lane.spawn(mock.Mock(cumulative=True),{},'continue',None,1000,path)
            self.assertEqual(code,1)
            self.assertIn('after 4 attempt(s)',used.infrastructure_error)
            self.assertEqual(lane.stream_outcome(None,used.infrastructure_error),'infrastructure')
            self.assertEqual(lane.stream_outcome({'outcome':'generator_bug'},used.infrastructure_error),'generator_bug')
            self.assertEqual(lane.stream_outcome({'outcome':'failed_gates'},used.infrastructure_error),'failed_gates')

    def test_old_capacity_error_and_quoted_tool_output_do_not_trigger_retry(self):
        with tempfile.TemporaryDirectory() as scratch:
            path=Path(scratch)/'worker.jsonl'
            self.attempt(path)
            offset=path.stat().st_size
            with path.open('a') as log:
                log.write(json.dumps({'type':'item.completed','item':{'text':'model is at capacity'}})+'\n')
            self.assertIsNone(lane.capacity_error(path,offset))

    def test_capacity_does_not_sleep_past_original_deadline(self):
        with tempfile.TemporaryDirectory() as scratch:
            path=Path(scratch)/'worker.jsonl'
            with mock.patch.object(lane,'spawn_once',side_effect=lambda *args:self.attempt(path)), mock.patch.object(lane.time,'sleep') as sleep:
                _,_,used=lane.spawn(mock.Mock(cumulative=True),{},'continue',None,10,path)
            sleep.assert_not_called()
            self.assertIsNotNone(used.infrastructure_error)


class Streams(unittest.TestCase):
    """`submit` closes the open card on a pass or a stop, and hands over the next; a card that keeps failing is set aside."""

    def submit(self, state, report=None, ok=True):
        card = mock.Mock(ok=ok)
        card.failures.return_value = [] if ok else [{"gate": "corpus", "detail": "line 3 failed"}]
        with tempfile.TemporaryDirectory() as scratch:
            for name in state["order"]:
                (Path(scratch) / name).mkdir()
                (Path(scratch) / name / "section.md").write_text(f"## Card: {name}", encoding="utf-8")
            stream = Path(scratch) / "stream.json"
            stream.write_text(json.dumps(state), encoding="utf-8")
            with mock.patch.multiple(lane, STREAM=stream, reopen_batch=mock.Mock(), expand_reports=mock.Mock(), report_of=mock.Mock(return_value=report),
                                     batch_gates=mock.Mock(return_value={state["current"]: card}), owned_digest=mock.Mock(return_value="d"),
                                     card_dir=lambda name: Path(scratch) / name), \
                 mock.patch("sys.stdout", new_callable=io.StringIO) as out:
                code = lane.cmd_submit(None)
                return code, json.loads(stream.read_text(encoding="utf-8")), out.getvalue()

    def state(self, **extra):
        return dict({"order": ["A", "B"], "current": "A", "opened": 0, "closed": {}, "tries": {}, "max_tries": 2}, **extra)

    def test_a_pass_closes_the_card_and_prints_the_next(self):
        code, state, out = self.submit(self.state())
        self.assertEqual((code, state["current"], state["closed"]["A"]["outcome"], state["closed"]["A"]["digest"]), (0, "B", "passed", "d"))
        self.assertIn("## Card: B", out)

    def test_a_stop_closes_the_card_without_the_gates(self):
        code, state, out = self.submit(self.state(), {"decision": "engine_gap", "reason": "no such rule"})
        self.assertEqual((state["current"], state["closed"]["A"]["outcome"], state["closed"]["A"]["detail"]), ("B", "engine_gap", "no such rule"))

    def test_a_failure_is_counted_and_the_card_stays_open(self):
        code, state, out = self.submit(self.state(), ok=False)
        self.assertEqual((code, state["current"], state["tries"]), (1, "A", {"A": 1}))
        self.assertIn("line 3 failed", out)

    def test_the_last_failure_sets_the_card_aside(self):
        code, state, out = self.submit(self.state(tries={"A": 1}), ok=False)
        self.assertEqual((state["current"], state["closed"]["A"]["outcome"]), ("B", "failed_gates"))
        self.assertIn("## Card: B", out)

    def test_after_the_last_card_the_worker_is_told_its_turn_is_over(self):
        code, state, out = self.submit(self.state(order=["A"]))
        self.assertIsNone(state["current"])
        self.assertIn("Every card is done", out)

    def test_the_prompt_says_to_go_on_without_stopping(self):
        text = lane.WAYS["stream"].replace("\n", " ")
        for words in ("submit", "Your turn ends only when `submit` says that every card is done", "Do not stop between cards"):
            self.assertIn(words, text)


class Rulings(unittest.TestCase):
    """As Cantrip's add-cards skill: a scenario is owed only to a ruling the card's own code decides."""

    def problems(self, ruling):
        report = {"validation": [], "test_points": [], "rulings": [dict(ruling, ruling="Exiling a card with suspend isn't casting it.")],
                  "look_answers": [], "equivalent_mutants": []}
        with mock.patch.multiple(lane, report_of=mock.Mock(return_value=report), content_file=mock.Mock(return_value=mock.Mock(read_text=mock.Mock(return_value='{"oracleText": ""}'))),
                                 expectation_problem=mock.Mock(return_value="ruling: no such line")):
            return lane.report_problems("A", [], [], [])

    def test_a_ruling_left_empty_is_owed_nothing(self):
        self.assertEqual(self.problems({"scenario": "", "line": 0, "no_decision": ""}), [])

    def test_a_ruling_paired_must_name_an_expectation(self):
        self.assertEqual(len(self.problems({"scenario": "a-suspend", "line": 3, "no_decision": ""})), 1)


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

    def test_codex_can_write_its_lane_state_but_does_not_add_other_roots(self):
        with mock.patch.multiple(lane,STATE=Path('/worker/state'),TARGET=Path('/worker/target')):
            command=lane.Codex('model','medium').command({'dirs':['/worker/state/cards/a']},None)
        roots=next(x.split('=',1)[1] for x in command if x.startswith('sandbox_workspace_write.writable_roots='))
        self.assertEqual(json.loads(roots),['/worker/target','/worker/state','/worker/state/cards/a'])

    def test_style_keeps_fixed_width_bounds_but_rejects_rule_caps(self):
        with mock.patch.object(lane,'card_text',return_value='A test'):
            for expression, allowed in [('n.clamp(-32768, 32767) as i16',True),
                                         ('n.clamp(0, 65535) as u16',True),
                                         ('n.clamp(-2147483648, 2147483647) as i32',True),
                                         ('n.clamp(-2147483648, 2147483647) as i16',False),
                                         ('n.clamp(-2147483648, 2147483647)',False),
                                         ('n.clamp(0, 2147483647) as i32',False),
                                         ('n.clamp(-2147483648, 20) as i32',False),
                                         ('n.clamp(0, 4294967295) as u32',True),
                                         ('n.clamp(0, 4294967295) as u16',False),
                                         ('n.clamp(0, 4294967294) as u32',False),
                                         ('n.clamp(-1, 4294967295) as u32',False),
                                         ('n.clamp(0, i64::from(u32::MAX)) as u32',True),
                                         ('n.clamp(0, i64::from(u32::MAX)) as i32',False),
                                         ('n.clamp(0, i64::from(u32::MAX))',False),
                                         ('n.clamp(0, 20) as u32',False),
                                         ('n.clamp(0, 20) as u16',False),
                                         ('n.clamp(-32768, 32767) as u16',False),
                                         ('n.clamp(-32768, 32767)',False)]:
                with self.subTest(expression=expression):
                    problems=lane.style_problems('A',expression)
                    self.assertEqual(not any('a clamp:' in p for p in problems),allowed)

    def test_claude_reads_its_result(self):
        backend, usage = lane.Claude(None, None), lane.Usage()
        line = '{"type":"result","session_id":"s1","num_turns":3,"total_cost_usd":0.5,"usage":{"input_tokens":5,"cache_read_input_tokens":7,"output_tokens":2}}'
        self.assertEqual(backend.read(line, usage), "s1")
        self.assertEqual((usage.input, usage.cached, usage.turns, usage.reported_cost), (12, 7, 3, 0.5))


HARLEQUIN = '''use cantrip_api::*;
use cantrip_mechanics::{freerunning, triggers};
use cantrip_oracle as oracle;

/// "When this creature enters, you draw a card and you lose 1 life." (CR 608.2, 121.1, 119.3).
fn draw_card_lose_1_life(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
    g.draw(item.controller, 1);
    g.end_sentence();
    g.lose_life(item.controller, 1);
    Ok(())
}

pub static CARD: CardDef = CardDef {
    abilities: &[
        triggered(
            triggers::THIS_ENTERS,
            draw_card_lose_1_life,
            "When this creature enters, you draw a card and you lose 1 life.",
        ),
    ],
    ways: &[freerunning::way(mana("{1}{B}"), "Freerunning {1}{B}")],
    ..oracle::MERCILESS_HARLEQUIN
};
'''
HARLEQUIN_TEXT = "Freerunning {1}{B} (You may cast this spell for its freerunning cost.)\nWhen this creature enters, you draw a card and you lose 1 life."
HARLEQUIN_POINTS = ["[1] Freerunning {1}{B}: shown at least once", "[1] the cost '{1}{B}': paid exactly", "[2] the amount 1: an expectation that a value one off would fail"]


def survived(source, number, what, now):
    lines = source.split("\n")
    return lane.survivor({"key": f"line {number}: {what}", "line": number, "what": what, "was": lines[number - 1], "now": now,
                          "detail": f"card.rs:{number}: {what}, was: {lines[number - 1].strip()} now: {now.strip()}", "text": "x", "path": None})


class NoiseMutants(unittest.TestCase):
    """What no scenario can catch costs a worker nothing: an empty range is no change, a sentence's end left out is named by the lane."""

    def test_a_range_made_empty_is_no_change(self):
        self.assertTrue(lane.invalid_range("    .choose(2, 2);", "    .choose(2, 1);"))
        self.assertTrue(lane.invalid_range(".targets(&[t.between(1, 1)])", ".targets(&[t.between(1, 0)])"))
        self.assertTrue(lane.invalid_range("TargetSpec { min: 1, max: 1, filter: f }", "TargetSpec { min: 2, max: 1, filter: f }"))
        for now in ("    .choose(1, 2);", "    .choose(2, 3);"):
            self.assertFalse(lane.invalid_range("    .choose(2, 2);", now))
        self.assertFalse(lane.invalid_range("g.draw(p, 2);", "g.draw(p, 1);"))

    def test_the_empty_range_is_never_played_and_the_other_change_of_its_key_is(self):
        with tempfile.TemporaryDirectory() as scratch:
            source, content = Path(scratch) / "card.rs", Path(scratch) / "card.json"
            source.write_text("static MODES: ModalDef = modes(&[])\n    .choose(2, 2);\n", encoding="utf-8")
            content.write_text("{}", encoding="utf-8")
            mutate = mock.Mock()
            mutate.mutants.return_value = [(2, "2 to 1", "    .choose(1, 2);"), (2, "2 to 3", "    .choose(2, 3);"), (2, "2 to 1", "    .choose(2, 1);")]
            with mock.patch.multiple(lane, card_file=lambda name: source, content_file=lambda name: content, core_keywords=lambda: {}):
                changes, total = lane.changes_of("Card", mutate)
        self.assertEqual(total, 2)
        self.assertEqual([c["now"].strip() for c in changes], [".choose(1, 2);", ".choose(2, 3);"])
        self.assertEqual((changes[0]["line"], changes[0]["what"], changes[0]["was"]), (2, "2 to 1", "    .choose(2, 2);"))

    def test_a_sentence_end_left_out_is_named_by_the_lane_with_the_rule(self):
        why = lane.known_equivalent(survived(HARLEQUIN, 8, "statement left out", "    // (left out by card-mutate)"))
        self.assertIn("CR 603.8", why)
        self.assertIn("Means the same", why)
        self.assertIsNone(lane.known_equivalent(survived(HARLEQUIN, 9, "statement left out", "    // (left out by card-mutate)")))
        self.assertIsNone(lane.known_equivalent(survived(HARLEQUIN, 7, "1 to 2", "    g.draw(item.controller, 2);")))
        self.assertIsNone(lane.known_equivalent({"key": "keyword Trample", "detail": "d"}))


class SurvivorReport(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory()
        self.addCleanup(self.scratch.cleanup)
        root = Path(self.scratch.name)
        (root / "card.rs").write_text(HARLEQUIN, encoding="utf-8")
        (root / "card.json").write_text(json.dumps({"oracleText": HARLEQUIN_TEXT}), encoding="utf-8")
        self.report = root / "report.json"
        self.validation = [{"line": line, "code": "it", "agrees": True} for line in lane.printed_lines(HARLEQUIN_TEXT)]
        self.report.write_text(json.dumps({"decision": "done", "validation": self.validation, "test_points": [], "rulings": [], "look_answers": [], "equivalent_mutants": []}), encoding="utf-8")
        patcher = mock.patch.multiple(lane, card_file=lambda name: root / "card.rs", content_file=lambda name: root / "card.json", card_dir=lambda name: root)
        patcher.start()
        self.addCleanup(patcher.stop)
        self.noise = survived(HARLEQUIN, 8, "statement left out", "    // (left out by card-mutate)")
        self.real = survived(HARLEQUIN, 9, "1 to 2", "    g.lose_life(item.controller, 2);")

    def gate(self, survivors):
        gates = lane.Gates("Merciless Harlequin", False)
        lane.report_gate(gates, "Merciless Harlequin", {"points": [], "looks": [], "source": HARLEQUIN, "overlaps": [],
                                                      "ran": {"harlequin-enters.cantrip": {7, 8, 9}, "harlequin-freerunning.cantrip": {7}}}, survivors)
        return gates.results[-1], json.loads(self.report.read_text(encoding="utf-8"))

    def test_a_sentence_end_left_out_costs_no_submit_and_stays_in_the_report(self):
        result, report = self.gate([self.noise])
        self.assertTrue(result["ok"], result["detail"])
        self.assertEqual([(e["mutant"], e["by"]) for e in report["equivalent_mutants"]], [("line 8: statement left out", "lane")])
        self.assertIn("CR 603.8", report["equivalent_mutants"][0]["why"])
        self.gate([self.noise])
        self.assertEqual(len(json.loads(self.report.read_text(encoding="utf-8"))["equivalent_mutants"]), 1, "named once, however many submits")

    def test_a_key_the_worker_named_is_left_to_it(self):
        self.report.write_text(json.dumps({"decision": "done", "validation": self.validation, "equivalent_mutants": [{"mutant": "line 8: statement left out", "why": "mine"}]}), encoding="utf-8")
        result, report = self.gate([self.noise])
        self.assertTrue(result["ok"])
        self.assertEqual(report["equivalent_mutants"], [{"mutant": "line 8: statement left out", "why": "mine"}])

    def test_a_real_survivor_still_fails_and_says_where_it_is(self):
        result, _ = self.gate([self.noise, self.real])
        self.assertFalse(result["ok"])
        detail = result["detail"]
        self.assertNotIn("line 8", detail)
        self.assertIn("line 9: g.lose_life(item.controller, 1);", detail)
        self.assertIn("printed line 2: When this creature enters, you draw a card and you lose 1 life.", detail)
        self.assertIn("ran in: harlequin-enters\n", detail)
        self.assertIn("`line 9: 1 to 2`, now: g.lose_life(item.controller, 2);", detail)
        self.assertTrue(detail.startswith("Each change below leaves every scenario passing."), "the code comes first, not the escape hatch")
        self.assertTrue(detail.rstrip().endswith("to build the game that tells)."))
        self.assertEqual(detail.count("equivalent"), 1)

    def test_changes_of_one_line_are_one_entry(self):
        other = survived(HARLEQUIN, 9, "1 to 0", "    g.lose_life(item.controller, 0);")
        text = lane.survivor_text(HARLEQUIN, [self.real, other], lane.printed_lines(HARLEQUIN_TEXT), HARLEQUIN_POINTS)
        self.assertEqual(text.count("line 9: g.lose_life"), 1)
        self.assertIn("printed line 2: When this creature enters, you draw a card and you lose 1 life. (test points 3)", text)
        self.assertIn("`line 9: 1 to 0`", text)
        self.assertNotIn("ran in", text)
        draw = survived(HARLEQUIN, 7, "1 to 2", "    g.draw(item.controller, 2);")
        text = lane.survivor_text(HARLEQUIN, [draw, self.real], lane.printed_lines(HARLEQUIN_TEXT), HARLEQUIN_POINTS)
        self.assertEqual(text.count("you lose 1 life. (test points 3)"), 1)
        self.assertIn("line 9: g.lose_life(item.controller, 1);\n  printed line 2, as above", text)

    def test_a_keyword_taken_off_says_so(self):
        text = lane.survivor_text("", [{"key": "keyword Trample", "detail": "Trample taken off the card"}], ["Trample"], [])
        self.assertIn("`keyword Trample`: Trample taken off the card", text)


class PrintedLineOfCode(unittest.TestCase):
    PRINTED = ["Freerunning {1}{B}", "When this creature enters, you draw a card and you lose 1 life."]

    def test_a_function_is_the_line_its_doc_comment_quotes(self):
        self.assertEqual(lane.printed_line_of(HARLEQUIN.split("\n"), 9, self.PRINTED), 2)

    def test_a_line_of_the_card_is_the_text_of_the_call_it_is_in(self):
        lines = HARLEQUIN.split("\n")
        self.assertEqual(lane.printed_line_of(lines, lines.index("    ways: &[freerunning::way(mana(\"{1}{B}\"), \"Freerunning {1}{B}\")],") + 1, self.PRINTED), 1)

    def test_a_quote_of_the_whole_card_names_no_line(self):
        source = '/// "Choose two — • Draw a card. • Gain 3 life." (CR 700.2).\nfn mode_0(g: G) {\n    g.draw(p, 1);\n}\n'
        self.assertIsNone(lane.printed_line_of(source.split("\n"), 3, ["Choose two —", "• Draw a card.", "• Gain 3 life."]))

    def test_code_before_any_item_names_none(self):
        self.assertIsNone(lane.printed_line_of(HARLEQUIN.split("\n"), 1, self.PRINTED))


WEAVERS = '''fn kinship(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
    if effects::ask_may(g, item.controller, item.object, Purpose::Optional)? {
        let paid_2 = {
            let mut revealed = false;
            if let Some(card) = {
                let q: &dyn Query = g;
                q.state().zone(Zone::library(item.controller)).first().copied()
            } {
                if {
                    let q: &dyn Query = g;
                    Some(item.object).is_some_and(|o| shares(q, card, o))
                }
                    && effects::ask_may(
                        g,
                        item.controller,
                        "a } in a string",
                    )?
                {
                    g.reveal(item.controller, &[card]);
                    revealed = true;
                }
            }
            revealed
        };
        if paid_2 {
            g.draw(item.controller, 1);
        } else {
            g.mill(item.controller, 1);
        }
    }
    Ok(())
}'''


class CoverageLines(unittest.TestCase):
    def test_a_closing_brace_names_the_block_it_closes(self):
        lines = WEAVERS.split("\n")
        self.assertEqual(lane.never_ran(lines, 22), "line 22 never ran: } (the block of line 5: if let Some(card) = {)")

    def test_a_block_opened_after_a_long_condition_names_the_condition(self):
        lines = WEAVERS.split("\n")
        self.assertEqual(lines[20].strip(), "}")
        self.assertEqual(lane.block_opener(lines, 21), 9)

    def test_an_else_names_its_if(self):
        lines = WEAVERS.split("\n")
        number = lines.index("        } else {") + 1
        self.assertEqual(lane.never_ran(lines, number), f"line {number} never ran: }} else {{ (the block of line {number - 2}: if paid_2 {{)")

    def test_a_line_of_code_says_only_itself(self):
        lines = WEAVERS.split("\n")
        self.assertEqual(lane.never_ran(lines, 19), "line 19 never ran: g.reveal(item.controller, &[card]);")

    def test_braces_that_do_not_match_name_nothing(self):
        self.assertEqual(lane.never_ran(["    }"], 1), "line 1 never ran: }")


class FindingCards(unittest.TestCase):
    CARDS = [{"name": "Hulk, Bruce Banner", "type": "Legendary Creature — Gamma Berserker Hero", "power": 7, "toughness": 4, "keywords": ["Trample"], "oracleText": "Trample"},
             {"name": "Red Hulk", "type": "Legendary Creature — Gamma Berserker Villain", "power": 6, "toughness": 7, "keywords": ["Reach", "Trample"],
              "oracleText": "Reach, trample\nWhenever Red Hulk is dealt damage, put a +1/+1 counter on him."},
             {"name": "Serra Angel", "type": "Creature — Angel", "power": 4, "toughness": 4, "keywords": ["Flying", "Vigilance"], "oracleText": "Flying, vigilance"},
             {"name": "Keeneye Aven", "type": "Creature — Bird Soldier", "keywords": ["Flying", "Cycling"], "oracleText": "Flying\nCycling {2} ({2}, Discard this card: Draw a card.)"},
             {"name": "Gamma Ray", "type": "Instant", "oracleText": "Gamma Ray deals 3 damage to any target."}]

    def names(self, **wanted):
        return [c["name"] for c in self.CARDS if lane.finds(c, **wanted)]

    def test_a_subtype_is_a_word_after_the_dash(self):
        self.assertEqual(self.names(subtypes=["gamma"]), ["Hulk, Bruce Banner", "Red Hulk"])
        self.assertEqual(self.names(subtypes=["Gamma"], without=["Villain"]), ["Hulk, Bruce Banner"])

    def test_a_type_is_any_word_of_the_type_line(self):
        self.assertEqual(self.names(types=["legendary", "creature"]), ["Hulk, Bruce Banner", "Red Hulk"])

    def test_keywords_and_text(self):
        self.assertEqual(self.names(keywords=["flying"]), ["Serra Angel", "Keeneye Aven"])
        self.assertEqual(self.names(keywords=["first strike"]), [])
        self.assertEqual(self.names(texts=["+1/+1 counter"]), ["Red Hulk"])
        self.assertEqual(self.names(texts=["draw a card"]), [], "reminder text is no rule")

    def test_the_command_lists_the_plainest_first_and_leaves_out_the_batch(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            (root / "content" / "mtg" / "cards").mkdir(parents=True)
            for i, card in enumerate(self.CARDS):
                (root / "content" / "mtg" / "cards" / f"{i}.json").write_text(json.dumps(card), encoding="utf-8")
            (root / "batch").mkdir()
            (root / "batch" / "open.json").write_text(json.dumps(["Serra Angel"]), encoding="utf-8")
            args = mock.Mock(cantrip=str(root), type=["creature"], subtype=[], keyword=[], text=[], without=[], n=2)
            with mock.patch.object(lane, "BATCH_DIR", root / "batch"), mock.patch("sys.stdout", new_callable=io.StringIO) as out:
                lane.cmd_find(args)
        lines = out.getvalue().splitlines()
        self.assertTrue(lines[0].startswith("Hulk, Bruce Banner  "), lines)
        self.assertIn("7/4", lines[0])
        self.assertEqual(lines[-1], "3 found, 1 more (-n)")
        self.assertNotIn("Serra Angel", out.getvalue())

    def test_the_stream_and_the_skill_say_to_look_before_calling_a_change_unreachable(self):
        self.assertIn("find --subtype", lane.WAYS["stream"])
        skill = (lane.SKILL / "SKILL.md").read_text(encoding="utf-8")
        self.assertIn("card_lane.py find", skill)
        self.assertIn("`find`", skill)


TABLE = '''macro_rules! counter_kinds {
    ($($kind:ident = $n:literal, $name:literal, $token:literal;)*) => {};
}

counter_kinds! {
    PlusOne = 0, "+1/+1", "PlusOnePlusOne";
    Charge = 1, "charge", "Charge";
}

pub struct After;
'''


class Rewrites(unittest.TestCase):
    def test_a_rule_the_source_adds_is_reported_until_it_has_a_verdict(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "grammars").mkdir()
            (root / "data").mkdir()
            (root / "grammars/find_replace.json5").write_text(
                '[\n  // old "find": "not a rule"\n  {"reason": "x", "cards": ["A"], "find": "a \\"b\\" c", "replace": "d"},\n'
                '  {"reason": "y", "cards": ["B"], "find": "new", "replace": "e"},\n]\n')
            (root / "data/cantrip_rewrites.json").write_text(
                json.dumps([{"find": 'a "b" c', "cards": ["A"], "verdict": "same"}, {"find": "gone", "cards": ["C"], "verdict": "same"}]))
            rewrites, unread = tree.rewrites_of(root)
        self.assertEqual([row["find"] for row in rewrites], ['a "b" c'])
        self.assertEqual(unread, ["new"])


class WorkerStream(unittest.TestCase):
    def test_a_quoted_line_of_a_file_in_the_error_log_is_no_event(self):
        usage = lane.Usage()
        self.assertIsNone(lane.Codex("m", "e").read('      "Your upkeep trigger adds a muster counter."\n', usage))
        self.assertIsNone(lane.Claude("m", "e").read('[1, 2]\n', usage))
        self.assertEqual(usage.turns, 0)


class Counters(unittest.TestCase):
    def test_a_card_wants_the_counters_it_uses_and_cantrip_has_no_line_for(self):
        code = ('let a = CounterKind::Spore; let b = CounterKind::Charge; for k in CounterKind::ALL {} let c = CounterKind::Spore;'
                ' let d = planned_value!("counter kind Rad", CounterKind::Charge => CounterKind::Rad);')
        self.assertEqual(lane.counters_wanted(code, TABLE), ["Spore"])
        self.assertEqual(lane.counters_wanted("CounterKind::KEYWORDS; CounterKind::POWER_TOUGHNESS", TABLE), [])

    def test_lines_run_on_from_the_last_number_and_leave_the_rest_alone(self):
        table = lane.with_counters(TABLE, ["Spore", "Verse"])
        self.assertIn('    Charge = 1, "charge", "Charge";\n    Spore = 2, "spore", "Spore";\n    Verse = 3, "verse", "Verse";\n}\n\npub struct After;', table)
        self.assertEqual(lane.counters_wanted("CounterKind::Verse", table), [])

    def test_appending_a_named_counter_preserves_intrinsic_pt_metadata(self):
        metadata = 'PlusOne = 0, "+1/+1", "PlusOnePlusOne", (1, 1);'
        base = TABLE.replace('PlusOne = 0, "+1/+1", "PlusOnePlusOne";', metadata)
        grown = lane.with_counters(base, ["Spore"])
        self.assertIn(metadata, grown)
        self.assertIn('Spore = 2, "spore", "Spore";', grown)
        self.assertEqual(lane.counters_wanted("CounterKind::PlusOne; CounterKind::Spore", grown), [])

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
                lane.commit_brought("Some Card")
                self.assertEqual((root / lane.COUNTER_TABLE).read_text(encoding="utf-8"), worktree)
                committed = lane.git("show", f"HEAD:{lane.COUNTER_TABLE}")
                self.assertIn('    Verse = 2, "verse", "Verse";', committed)
                self.assertNotIn("Spore", committed)
                self.assertEqual(lane.git("log", "-1", "--format=%s").strip(), "Engine: the verse counter (CR 122.1), for Some Card")
                self.assertTrue(lane.only_counter_lines())
                lane.commit_brought("Some Card")
                self.assertEqual(lane.git("rev-list", "--count", "HEAD").strip(), "2")

    def test_a_card_of_keywords_alone_has_no_file_and_brings_no_counter(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            shell = self.repository(root)
            with mock.patch.object(lane, "CANTRIP", root), mock.patch.object(lane, "shell", shell), \
                    mock.patch.object(lane, "card_file", lambda _: root / "noble_elephant.rs"):
                before = lane.git("rev-parse", "HEAD")
                lane.commit_brought("Noble Elephant")
                self.assertEqual(lane.git("rev-parse", "HEAD"), before)

    def test_a_new_token_is_a_commit_of_its_own_before_the_card(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            shell = self.repository(root)
            table = root / lane.TOKEN_TABLE
            table.parent.mkdir(parents=True)
            table.write_text("pub static OLD: CardDef = x;\n\n#[cfg(test)]\nmod tests {}\n", encoding="utf-8")
            shell("git", "-C", root, "add", "-A")
            shell("git", "-C", root, "commit", "-q", "-m", "tokens")
            (root / "card.rs").write_text("use cantrip_mechanics::tokens::{NEW, OLD};\n", encoding="utf-8")
            token = {"title": "the 1/1 Thing token", "cards": 2, "source": "/// A thing.\npub static NEW: CardDef = y;\n"}
            with mock.patch.object(lane, "CANTRIP", root), mock.patch.object(lane, "shell", shell), \
                    mock.patch.object(lane, "card_file", lambda _: root / "card.rs"), \
                    mock.patch.dict(tree.NEW_TOKENS, {"NEW": token}, clear=True):
                lane.add_tokens((root / "card.rs").read_text(encoding="utf-8"))
                self.assertTrue(lane.only_new_tokens())
                worktree = table.read_text(encoding="utf-8")
                self.assertLess(worktree.index("pub static NEW"), worktree.index("#[cfg(test)]"))
                lane.commit_brought("Some Card")
                self.assertEqual(lane.git("show", f"HEAD:{lane.TOKEN_TABLE}"), worktree)
                self.assertEqual(lane.git("log", "-1", "--format=%s").strip(), "Mechanics: the 1/1 Thing token, for Some Card")
                lane.commit_brought("Some Card")
                self.assertEqual(lane.git("log", "-1", "--format=%s").strip(), "Mechanics: the 1/1 Thing token, for Some Card")
                table.write_text(worktree.replace("OLD", "OLDER"), encoding="utf-8")
                self.assertFalse(lane.only_new_tokens())

    def test_anything_else_in_the_table_is_out_of_scope(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            shell = self.repository(root)
            with mock.patch.object(lane, "CANTRIP", root), mock.patch.object(lane, "shell", shell):
                (root / lane.COUNTER_TABLE).write_text(TABLE.replace("pub struct After;", "pub struct Before;"), encoding="utf-8")
                self.assertFalse(lane.only_counter_lines())


SPLIT = {"name": "Claim // Fame", "oracleText": "", "halves": [
    {"name": "Claim", "manaCost": "{B}", "type": "Sorcery", "oracleText": "Return target creature card with mana value 2 or less from your graveyard to the battlefield."},
    {"name": "Fame", "manaCost": "{1}{R}", "type": "Sorcery", "oracleText": "Aftermath (Cast this spell only from your graveyard. Then exile it.)\nTarget creature gets +2/+0 and gains haste until end of turn.", "aftermath": True}]}
DFC = {"name": "Delver of Secrets", "manaCost": "{U}", "type": "Creature — Human Wizard", "oracleText": "At the beginning of your upkeep, look at the top card of your library.",
       "backFace": {"name": "Insectile Aberration", "colorIndicator": ["Blue"], "type": "Creature — Human Insect", "oracleText": "Flying", "keywords": ["Flying"]}}


class SeveralFaces(unittest.TestCase):
    """A card of several faces as the lane names, queues, reads and drafts it."""

    def test_cantrip_names_a_split_card_by_both_halves_and_another_by_its_front(self):
        self.assertEqual(lane.lane_name({"name": "Claim // Fame", "layout": "Split", "faces": ["Claim", "Fame"]}), "Claim // Fame")
        self.assertEqual(lane.lane_name({"name": "A // B", "layout": "Room", "faces": ["A", "B"]}), "A // B")
        self.assertEqual(lane.lane_name({"name": "Rona, Herald of Invasion // Rona, Tolarian Obliterator", "layout": "Transforming",
                                         "faces": ["Rona, Herald of Invasion", "Rona, Tolarian Obliterator"]}), "Rona, Herald of Invasion")
        self.assertEqual(lane.lane_name({"name": "The Gold Saucer", "layout": "Card", "faces": ["The Gold Saucer"]}), "The Gold Saucer")
        self.assertEqual(lane.lane_name({"name": "Old Row"}), "Old Row")  # a row of before `faces`

    def test_a_list_finds_a_card_by_its_name_the_generators_or_a_face(self):
        stomp = {"name": "Stomp", "layout": "Card", "faces": ["Stomp"]}
        giant = {"name": "Bonecrusher Giant // Stomp", "layout": "Adventurer", "faces": ["Bonecrusher Giant", "Stomp"]}
        claim = {"name": "Claim // Fame", "layout": "Split", "faces": ["Claim", "Fame"]}
        index = lane.name_index([giant, stomp, claim])
        self.assertIs(index["Bonecrusher Giant"], giant)
        self.assertIs(index["Bonecrusher Giant // Stomp"], giant)
        self.assertIs(index["Stomp"], stomp)  # a card's own name before another's face
        self.assertIs(index["Claim // Fame"], claim)
        self.assertIs(index["Fame"], claim)

    def test_each_face_is_compared_with_scryfall(self):
        found = {"layout": "split", "faces": {"Claim": "Return it.", "Fame": "Target creature gets +2/+0."}}
        oracle = {"Claim": {"text": "Return it."}, "Fame": {"text": "Target creature gets +3/+0."}}
        reason, differs = lane.scryfall_problem(found, ["Claim", "Fame"], oracle, lane.LAYOUTS)
        self.assertEqual(reason, "Oracle text differs from mtgish's oracle.json")
        self.assertEqual([d["name"] for d in differs], ["Fame"])
        oracle["Fame"]["text"] = "Target creature gets +2/+0."
        self.assertEqual(lane.scryfall_problem(found, ["Claim", "Fame"], oracle, lane.LAYOUTS), (None, []))
        self.assertEqual(lane.scryfall_problem(found, ["Claim"], oracle, lane.LAYOUTS)[0], "faces differ from Scryfall's")
        self.assertEqual(lane.scryfall_problem(dict(found, layout="flip"), ["Claim", "Fame"], oracle, lane.LAYOUTS)[0], "layout flip")
        self.assertEqual(lane.scryfall_problem(None, ["Claim"], oracle, lane.LAYOUTS)[0], "not on Scryfall")

    def test_every_face_s_text_is_the_card_s(self):
        self.assertEqual(lane.content_faces(SPLIT), [("Claim", SPLIT["halves"][0]["oracleText"]), ("Fame", SPLIT["halves"][1]["oracleText"])])
        self.assertEqual([f for f, _ in lane.content_faces(DFC)], ["Delver of Secrets", "Insectile Aberration"])
        self.assertEqual(lane.content_faces({"name": "Bear", "oracleText": ""}), [("Bear", "")])
        with tempfile.TemporaryDirectory() as scratch:
            path = Path(scratch) / "card.json"
            path.write_text(json.dumps(SPLIT), encoding="utf-8")
            with mock.patch.object(lane, "content_file", lambda name: path):
                self.assertEqual(lane.printed_lines(lane.card_text("Claim // Fame")),
                                 [SPLIT["halves"][0]["oracleText"], "Aftermath", "Target creature gets +2/+0 and gains haste until end of turn."])
                self.assertTrue(lane.costs_mana("Claim // Fame"))  # its halves' costs, the whole card having none

    def test_a_back_face_s_keywords_are_repaired_too(self):
        with tempfile.TemporaryDirectory() as scratch:
            path = Path(scratch) / "card.json"
            path.write_text(json.dumps(dict(DFC, backFace=dict(DFC["backFace"], oracleText="Trample; banding", keywords=[]))), encoding="utf-8")
            with mock.patch.multiple(lane, content_file=lambda name: path, core_keywords=lambda: {"trample": "Trample", "banding": "Banding"}):
                lane.repair_keywords("Delver of Secrets")
            self.assertEqual(json.loads(path.read_text(encoding="utf-8"))["backFace"]["keywords"], ["Trample", "Banding"])

    def test_facts_leave_out_every_face_s_text(self):
        facts = lane.facts_of(SPLIT)
        self.assertEqual([sorted(h) for h in facts["halves"]], [["manaCost", "name", "type"], ["aftermath", "manaCost", "name", "type"]])
        self.assertNotIn("oracleText", lane.facts_of(DFC)["backFace"])
        single = {"name": "Bear", "manaCost": "{1}{G}", "oracleText": "x", "power": 2}
        self.assertEqual(lane.facts_of(single), {"manaCost": "{1}{G}", "power": 2})

    def test_how_each_layout_s_other_face_is_reached(self):
        with mock.patch.dict(lane.FACE_ART, {kind: "" for kind in lane.FACE_PLAY}):
            self.assertIn("`Bot1.Graveyard.Claim // Fame.Cast(Ability: Fame)`", lane.faces_section({"layout": "split"}, SPLIT))
            self.assertIn("`? Bot1.Battlefield.Insectile Aberration.Transformed = true`", lane.faces_section({"layout": "transform"}, DFC))
            self.assertIn("`Bot1.Hand.Delver of Secrets.Cast(Ability: back face)`", lane.faces_section({"layout": "modal_dfc"}, DFC))
            room = {"name": "A // B", "halves": [{"name": "A", "type": "Enchantment — Room", "oracleText": "x"}, {"name": "B", "type": "Enchantment — Room", "oracleText": "y"}]}
            self.assertIn("`Bot1.Battlefield.A.Activate(Ability: right door)`", lane.faces_section({}, room))
            for key, words in (("adventure", "Cast(Ability: Adventure)"), ("omen", "Cast(Ability: Omen)"), ("prepare", "`Bot1.Exile.Spell.Cast`")):
                card = {"name": "Card", "oracleText": "x", key: {"name": "Spell", "oracleText": "y"}}
                self.assertIn(words, lane.faces_section({}, card))
            self.assertIn("no scenario melds it", lane.faces_section({"layout": "meld"}, {"name": "Garrison", "oracleText": "x"}))
            self.assertEqual(lane.faces_section({"layout": "normal"}, {"name": "Bear", "oracleText": "x"}), "")

    def test_a_split_prepare_or_meld_card_is_drafted_as_cantrip_has_one(self):
        def face(f):
            return {"name": f["name"], "manaCost": f["mana_cost"], "type": f["type_line"], "oracleText": f["oracle_text"], "keywords": ["Haste"]}
        claim = {"name": "Claim // Fame", "layout": "split", "faces": [
            {"name": "Claim", "mana_cost": "{B}", "type_line": "Sorcery", "oracle_text": "Return it."},
            {"name": "Fame", "mana_cost": "{1}{R}", "type_line": "Sorcery", "oracle_text": "Aftermath (Cast this spell only from your graveyard.)\nHaste."}]}
        drafted = lane.drafted_content(claim, face)
        self.assertEqual(drafted["name"], "Claim // Fame")
        self.assertEqual(drafted["oracleText"], "")
        self.assertEqual([sorted(h) for h in drafted["halves"]], [["manaCost", "name", "oracleText", "type"], ["aftermath", "manaCost", "name", "oracleText", "type"]])
        prepare = {"name": "Abigale // Heroic Stanza", "layout": "prepare", "faces": [
            {"name": "Abigale", "mana_cost": "{1}{W}{B}", "type_line": "Creature", "oracle_text": "Flying"},
            {"name": "Heroic Stanza", "mana_cost": "{1}{W/B}", "type_line": "Sorcery", "oracle_text": "Put a counter."}]}
        drafted = lane.drafted_content(prepare, face)
        self.assertEqual((drafted["name"], sorted(drafted["prepare"])), ("Abigale", ["manaCost", "name", "oracleText", "type"]))
        meld = {"name": "Garrison", "layout": "meld", "faces": [{"name": "Garrison", "mana_cost": "{2}{R}", "type_line": "Creature", "oracle_text": "x"}]}
        self.assertEqual(lane.drafted_content(meld, face)["name"], "Garrison")
        self.assertIsNone(lane.drafted_content(dict(claim, layout="transform"), face))  # new-card.py writes those


class Levels(unittest.TestCase):
    ESCORT = "Level up {2} ({2}: Put a level counter on this. Level up only as a sorcery.)\nLEVEL 1-4\n2/2\nLEVEL 5+\n5/5\nFirst strike"

    def problems(self, said):
        source = f'/// "{said}" (CR 711.2a).\nstatic_ability(Layer::PtSet, itself, base_pt::<2, 2>, "{said}")\n'
        with mock.patch.object(lane, "card_text", lambda name: self.ESCORT):
            return [p for p in lane.style_problems("Caravan Escort", source) if "ability text" in p]

    def test_a_level_s_line_is_said_with_its_level(self):
        self.assertEqual(self.problems("LEVEL 1-4: 2/2"), [])
        self.assertEqual(self.problems("LEVEL 5+: First strike"), [])

    def test_a_level_is_not_said_with_words_it_does_not_print(self):
        self.assertTrue(self.problems("LEVEL 6+: First strike"))
        self.assertTrue(self.problems("LEVEL 1-4: Flying"))


class RandomNote(unittest.TestCase):
    """A card that flips, rolls or chooses at random is told how a scenario forces the result (The Gold Saucer's stop)."""

    def test_random_words(self):
        for text in ("Flip a coin. If you win the flip, create a Treasure token.", "Roll a d20.", "Roll two dice.",
                     "Target player discards a card at random."):
            self.assertTrue(lane.RANDOM.search(text), text)

    def test_other_words(self):
        for text in ("Draw a card.", "Transform this creature.", "Turn it face up.", "Each player chooses a creature."):
            self.assertFalse(lane.RANDOM.search(text), text)


class SyntaxNotes(unittest.TestCase):
    """A card is told the scenario syntax its words need: no modes refused (Deafening Clarion), a companion (Keruga)."""

    def test_modes(self):
        for text in ("Choose one or both —\n• Deafening Clarion deals 3 damage", "Choose one —\n• Destroy X target artifacts",
                     "Choose two. You may choose the same mode more than once. —"):
            self.assertTrue(lane.MODES.search(text), text)
        for text in ("Choose a creature type.", "Each player chooses a creature they control."):
            self.assertFalse(lane.MODES.search(text), text)

    def test_a_stream_card_is_told(self):
        """A stream worker reads only the handbook and each card's section, never `brief()`."""
        import inspect
        self.assertIn("NOTES", inspect.getsource(lane.card_section))
        handbook = (Path(lane.__file__).parent / "handbook.md").read_text(encoding="utf-8")
        for form in ("Modes(Count: 0)", "Arrange(A | B; C)", "Choose(A; B)"):
            self.assertIn(form, handbook)

    def test_companion(self):
        self.assertTrue(lane.COMPANION.search("Companion — Each nonland card in your starting deck has mana value 3 or greater."))
        self.assertFalse(lane.COMPANION.search("Each creature you control is a companion of yours."))


class ContextBudget(unittest.TestCase):
    def test_idle_context_expires_only_after_an_hour_since_completion(self):
        idle = lane.context_budget.idle_restart
        self.assertFalse(idle(None, now=10000))
        self.assertFalse(idle(100, now=99))
        self.assertFalse(idle(100, now=3699))
        self.assertFalse(idle(100, now=3700))
        self.assertTrue(idle(100, now=3700.001))

    def test_rollout_uses_latest_request_and_counts_compactions(self):
        with tempfile.TemporaryDirectory() as scratch:
            path = Path(scratch) / 'sessions' / '2026' / 'rollout-abc.jsonl'
            path.parent.mkdir(parents=True)
            def event(last, total):
                return {'type': 'event_msg', 'payload': {'type': 'token_count', 'info': {
                    'last_token_usage': {'input_tokens': last, 'output_tokens': 10},
                    'total_token_usage': {'input_tokens': total, 'output_tokens': 500},
                    'model_context_window': 500000}}}
            path.write_text('\n'.join(map(json.dumps, [event(400, 1000), {'type': 'compacted', 'payload': {}}, event(90, 9000)])) + '\n{', encoding='utf-8')
            metrics = lane.context_budget.session_context('abc', scratch)
            self.assertEqual((metrics['last_tokens'], metrics['peak_tokens'], metrics['compactions'], metrics['context_window']), (100, 410, 1, 500000))
            self.assertFalse(lane.context_budget.budget_restart(metrics, 200))
            self.assertTrue(lane.context_budget.budget_restart(metrics, 100))

    def test_precompact_marker_is_bound_to_the_worker_session(self):
        with tempfile.TemporaryDirectory() as scratch:
            marker = Path(scratch) / 'marker.json'
            with mock.patch('sys.stdin', io.StringIO(json.dumps({'session_id': 's1', 'trigger': 'auto'}))), mock.patch('sys.stdout', new_callable=io.StringIO) as output:
                lane.context_budget.precompact_hook(marker)
            self.assertFalse(json.loads(output.getvalue())['continue'])
            self.assertTrue(lane.context_budget.precompact_stopped(marker, 's1'))
            self.assertFalse(lane.context_budget.precompact_stopped(marker, 's2'))
            marker.write_text('{', encoding='utf-8')
            self.assertFalse(lane.context_budget.precompact_stopped(marker, 's1'))

    def test_unavailable_measurement_never_triggers_token_limit(self):
        metrics = lane.context_budget.session_context('missing', '/nonexistent')
        self.assertIsNone(metrics['last_tokens'])
        self.assertFalse(lane.context_budget.budget_restart(metrics, 100))


class StreamContextCheckpoint(Streams):
    def test_card_limit_closes_card_and_preserves_pending_card(self):
        code, state, out = self.submit(self.state(context_budget=100, context_cards=1))
        self.assertEqual(state['closed']['A']['outcome'], 'passed')
        self.assertEqual(state['current'], 'B')
        self.assertTrue(state['rotate'])
        self.assertIn('Context checkpoint', out)
        self.assertNotIn('## Card: B', out)

    def test_unmeasured_unlimited_uses_twenty_card_fallback(self):
        code, state, out = self.submit(self.state(context_budget=100, context_cards=0, session_cards=19))
        self.assertTrue(state['rotate'])
        self.assertEqual(state['current'], 'B')



class BootstrapAliases(unittest.TestCase):
    STUB = 'use cantrip_api::*;\nuse cantrip_oracle as oracle;\n\npub static CARD: CardDef = CardDef { ..oracle::DAND_N };\n'

    def reference(self, path):
        return f'=== wrote {path} (characteristics only; add the behavior)\n'

    def test_only_the_reported_empty_alias_is_removed(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); cards = root / lane.CARDS_DIR; cards.mkdir(parents=True)
            alias = cards / 'dand_n.rs'; alias.write_text(self.STUB)
            destination = cards / 'dandan.rs'; destination.write_text('actual generated behavior')
            other = cards / 'unrelated.rs'; other.write_text(self.STUB)
            with mock.patch.object(lane, 'CANTRIP', root), mock.patch.object(lane, 'git', return_value=''):
                lane.remove_bootstrap_alias(self.reference(alias.relative_to(root)), destination)
            self.assertFalse(alias.exists()); self.assertTrue(other.exists())
            self.assertEqual(destination.read_text(), 'actual generated behavior')

    def test_the_requested_module_is_left_for_the_generator_to_overwrite(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); cards = root / lane.CARDS_DIR; cards.mkdir(parents=True)
            destination = cards / 'dandan.rs'; destination.write_text(self.STUB)
            with mock.patch.object(lane, 'CANTRIP', root), mock.patch.object(lane, 'git') as git:
                lane.remove_bootstrap_alias(self.reference(destination.relative_to(root)), destination)
            self.assertTrue(destination.exists()); git.assert_not_called()

    def test_tracked_aliases_and_aliases_with_behavior_are_never_removed(self):
        for tracked, text, error in [(True, self.STUB, 'tracked'), (False, self.STUB + 'fn effect() {}', 'behavior')]:
            with self.subTest(tracked=tracked), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp); cards = root / lane.CARDS_DIR; cards.mkdir(parents=True)
                alias = cards / 'dand_n.rs'; alias.write_text(text)
                with mock.patch.object(lane, 'CANTRIP', root), mock.patch.object(lane, 'git', return_value=str(alias) if tracked else ''):
                    with self.assertRaisesRegex(RuntimeError, error):
                        lane.remove_bootstrap_alias(self.reference(alias.relative_to(root)), cards / 'dandan.rs')
                self.assertEqual(alias.read_text(), text)

    def test_paths_outside_the_card_directory_and_symlinks_are_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); cards = root / lane.CARDS_DIR; cards.mkdir(parents=True)
            outside = root / 'outside.rs'; outside.write_text(self.STUB)
            alias = cards / 'dand_n.rs'; alias.symlink_to(outside)
            with mock.patch.object(lane, 'CANTRIP', root):
                for relative in ['outside.rs', str(alias.relative_to(root))]:
                    with self.subTest(relative=relative), self.assertRaisesRegex(RuntimeError, 'path'):
                        lane.remove_bootstrap_alias(self.reference(relative), cards / 'dandan.rs')
            self.assertTrue(alias.is_symlink()); self.assertEqual(outside.read_text(), self.STUB)

    def test_placeholder_names_use_the_same_canonical_slug_as_scope_ownership(self):
        import cantrip_scenarios as cs
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); scripts = root / lane.SCENARIOS_DIR; scripts.mkdir(parents=True)
            content = root / 'content.json'
            content.write_text(json.dumps({'name': 'Dandân', 'type': 'Creature', 'oracleText': 'When you control no Islands, sacrifice this creature.'}))
            def write_placeholder(*args):
                path = scripts / 'dand-n-todo-line-1.cantrip'; path.write_text('[Expect]\nTODO = unfinished\n')
                return 0, f'wrote {path.relative_to(root)}\n'
            with mock.patch.multiple(lane, CANTRIP=root, BATCH=['Dandân'], content_file=lambda _: content), \
                 mock.patch.object(lane, 'tool', side_effect=write_placeholder), mock.patch.object(cs, 'scenarios', return_value=[]), \
                 mock.patch.object(cs, 'Pieces'):
                self.assertEqual(lane.placeholders('Dandân'), ['dandan-todo-line-1.cantrip'])
                self.assertEqual(lane.owner('dandan-todo-line-1.cantrip'), 'Dandân')
            self.assertFalse((scripts / 'dand-n-todo-line-1.cantrip').exists())
            self.assertIn('TODO = unfinished', (scripts / 'dandan-todo-line-1.cantrip').read_text())


class FocusedBuilds(unittest.TestCase):
    def test_existing_legacy_non_ascii_module_is_selected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);cards=root/lane.CARDS_DIR;cards.mkdir(parents=True)
            (cards/'bespoke_b.rs').write_text('pub static CARD: X;')
            with mock.patch.object(lane,'CANTRIP',root):
                self.assertEqual(lane.names_of('Bespoke Bō')[0],'bespoke_b')
                self.assertEqual(lane.names_of('New Bō')[0],'new_bo')
                (cards/'bespoke_bo.rs').write_text('pub static CARD: X;')
                self.assertEqual(lane.names_of('Bespoke Bō')[0],'bespoke_bo')

    def test_closure_refresh_and_stable_list(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            cards = root / lane.CARDS_DIR
            scenarios = root / lane.SCENARIOS_DIR
            cards.mkdir(parents=True)
            scenarios.mkdir(parents=True)
            (cards / "a.rs").write_text("use super::dependency::X;")
            (cards / "b.rs").write_text("pub static CARD: X = X;")
            (cards / "dependency.rs").write_text("use crate::nested::X;")
            (cards / "nested.rs").write_text("")
            (cards / "provider.rs").write_text("pub static EXTRA: &[&X] = &[&tokens::THING];")
            scenario = scenarios / "a.cantrip"
            scenario.write_text("[Bot1 Deck]\nA\nB\n[Actions]\n")
            naming = lambda name: (name.lower(), name.lower(), name.upper())
            with mock.patch.multiple(lane, CANTRIP=root, STATE=root / "state", names_of=naming), mock.patch.dict(os.environ, CARD_LANE_FOCUSED="1"):
                lane.focus_cards(["A", "Keyword"])
                path = lane.STATE / "focused-cards.txt"
                self.assertEqual(set(path.read_text().splitlines()), {"a", "b", "dependency", "nested", "provider", "keyword"})
                stamp = path.stat().st_mtime_ns
                lane.focus_cards(["A", "Keyword"])
                self.assertEqual(path.stat().st_mtime_ns, stamp)
                self.assertEqual(lane.environment()["CANTRIP_CARDS_ONLY"], str(path))
                self.assertEqual(lane.environment()["CARD_LANE_FOCUSED"], "1")
                self.assertNotIn("CANTRIP_CARDS_ONLY", lane.full_environment())
                (cards / "c.rs").write_text("")
                scenario.write_text("[Bot1 Deck]\nA\nC\n[Actions]\n")
                lane.focus_cards(["A"])
                self.assertIn("c", path.read_text().splitlines())
                self.assertNotIn("b", path.read_text().splitlines())

    def test_focused_registry_keeps_implicit_undercity(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp); cards=root/lane.CARDS_DIR; cards.mkdir(parents=True)
            (cards/'undercity.rs').write_text('pub static CARD: X;')
            with mock.patch.object(lane,'CANTRIP',root):
                self.assertIn('undercity',lane.files_played([]))

    def test_focused_registry_keeps_named_cards_without_adding_them_to_decks(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);cards=root/lane.CARDS_DIR;cards.mkdir(parents=True)
            scenarios=root/lane.SCENARIOS_DIR;scenarios.mkdir()
            for name in ['a','b','wasteland','lightning_bolt','unrelated']:
                (cards/(name+'.rs')).write_text('pub static CARD: X;')
            (scenarios/'a.cantrip').write_text('[Bot1 Deck]\nA\n[Actions]\nBot1.Name(Wasteland)\nBot1.Cannot.Name(Lightning Bolt)\nBot1.Name(Unimplemented Card)\n')
            (scenarios/'b.cantrip').write_text('[Bot1 Deck]\nB\n[Actions]\nBot1.Name(Unrelated)\n')
            with mock.patch.object(lane,'CANTRIP',root):
                self.assertEqual(lane.files_played(['A']),{'a','wasteland','lightning_bolt'})

    def test_scratch_decks_do_not_expand_unrelated_scenarios(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            cards, scenarios = root / lane.CARDS_DIR, root / lane.SCENARIOS_DIR
            cards.mkdir(parents=True)
            scenarios.mkdir(parents=True)
            for stem in ("a", "mox", "dingus", "balance", "expensiveother", "dependency"):
                (cards / f"{stem}.rs").write_text("use super::dependency::X;" if stem == "dingus" else "")
            (cards / "provider.rs").write_text("pub static EXTRA: &[&X] = &[&tokens::THING];")
            (scenarios / "unrelated.cantrip").write_text("[Bot1 Deck]\nMox\nExpensiveOther\n[Actions]\n")
            scratch = root / "scratch.cantrip"
            scratch.write_text("[Bot1 Deck]\nA\nMox\nDingus\nBalance\n[Actions]\n")
            naming = lambda name: (name.lower(), name.lower(), name.upper())
            with mock.patch.multiple(lane, CANTRIP=root, STATE=root / "state", STREAM=root / "missing-stream", QUEUE=root / "missing-queue", names_of=naming), \
                 mock.patch.dict(os.environ, CARD_LANE_FOCUSED="1"), \
                 mock.patch.object(lane.subprocess, "run", return_value=mock.Mock(returncode=0)):
                lane.cmd_corpus(["--file", str(scratch)])
                selected = set((lane.STATE / "focused-cards.txt").read_text().splitlines())
                self.assertEqual(selected, {"a", "mox", "dingus", "balance", "dependency", "provider"})
                lane.focus_cards(["A"], extra_decks=["Mox"])
                self.assertNotIn("expensiveother", (lane.STATE / "focused-cards.txt").read_text().splitlines())

    def test_full_fallback_rejects_missing_cards_before_mutation(self):
        gates = {"A": mock.Mock()}
        shadow = mock.Mock()
        shadow.ready.return_value = []
        with mock.patch.dict(os.environ, CARD_LANE_FOCUSED="1"), \
             mock.patch.object(lane, "Shadow", return_value=shadow), \
             mock.patch.object(lane, "build_corpus", return_value=(0, "")), \
             mock.patch.object(lane, "shell", return_value=(0, "1 pass, 1 need cards")), \
             mock.patch.object(lane, "break_cards") as breaking:
            lane.mutation_gates(["A"], gates)
            breaking.assert_not_called()
            self.assertFalse(gates["A"].add.call_args.args[1])

    def test_corpus_refreshes_card_before_build(self):
        with mock.patch.object(lane, "focus_cards") as focus, \
             mock.patch.object(lane.subprocess, "run", return_value=mock.Mock(returncode=0)):
            lane.cmd_corpus(["--card", "A", "--next"])
            focus.assert_called_once_with(["A"])

    def test_corpus_next_refreshes_open_card(self):
        with tempfile.TemporaryDirectory() as tmp:
            state = Path(tmp) / "stream.json"
            state.write_text(json.dumps({"current": "A"}))
            with mock.patch.object(lane, "STREAM", state), mock.patch.object(lane, "focus_cards") as focus, \
                 mock.patch.object(lane.subprocess, "run", return_value=mock.Mock(returncode=0)):
                lane.cmd_corpus(["--next"])
                focus.assert_called_once_with(["A"])

    def test_worker_infrastructure_error_wins_over_turn_count(self):
        with tempfile.TemporaryDirectory() as tmp:
            card = {"name": "A"}
            backend = mock.Mock(name="backend")
            backend.name, backend.model, backend.cumulative = "codex", "model", False
            used = lane.Usage()
            used.turns = 2
            used.infrastructure_error = "capacity unavailable"
            with mock.patch.object(lane, "card_dir", return_value=Path(tmp)), \
                 mock.patch.object(lane, "begin"), mock.patch.object(lane, "prepare"), \
                 mock.patch.object(lane, "brief", return_value="prompt"), \
                 mock.patch.object(lane, "spawn", return_value=(1, None, used)), \
                 mock.patch.object(lane, "check") as check, mock.patch.object(lane, "save_patch"), \
                 mock.patch.object(lane, "clean_tree"), mock.patch.object(lane, "conclude"):
                self.assertEqual(lane.run_card(card, backend, mock.Mock(timeout=60), {}), "infrastructure")
                self.assertEqual(card["failure"], "capacity unavailable")
                check.assert_not_called()

    def test_restored_source_still_invokes_cargo(self):
        with mock.patch.object(lane, "shell", return_value=(0, "")) as shell:
            lane.build_corpus()
            lane.build_corpus()
            lane.build_corpus()
            self.assertEqual(shell.call_count, 3)

if __name__ == "__main__":
    unittest.main()
