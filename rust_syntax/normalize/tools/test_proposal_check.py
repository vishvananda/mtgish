"""The stub rules of proposal_check.py, none of them needing a Cantrip tree or rustc.

    python3 rust_syntax/normalize/tools/test_proposal_check.py
"""
import collections
import sys
import tempfile
import unittest
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
import proposal_check as pc  # noqa: E402

# A Cantrip in miniature: an enum the game state keeps, a table and its entry, a trait and a mechanics module.
API = """
pub trait Game {}
pub trait Query {}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Duration { EndOfTurn, UntilYourNextTurn, ForAsLongAs { card: CardId, index: u8 } }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EffectKind { CantBlock, Prevention { card: CardId, index: u8, to: Target } }
#[derive(Clone, Copy)]
pub struct CardDef {
    pub spell: Option<Spell>,
    pub triggers: &'static [TriggerDef],
    pub loyalty_by: Option<fn(&dyn Query) -> i32>,
}
impl CardDef {
    pub const BLANK: CardDef = CardDef { spell: None, triggers: &[], loyalty_by: None };
}
#[derive(Clone, Copy, Debug)]
pub struct TriggerDef { pub text: &'static str }
"""
MECHANICS = "pub mod tokens;\npub mod counters;\n"


def model():
    root = Path(tempfile.mkdtemp(prefix="proposal-check-test-"))
    for base in pc.SOURCES:
        (root / base).mkdir(parents=True)
    (root / "crates/cantrip-api/src/lib.rs").write_text(API)
    (root / "crates/cantrip-mechanics/src/lib.rs").write_text(MECHANICS)
    return pc.Model(root)


MODEL = model()


def diag(code, message, text, at, length=1):
    """A rustc JSON error at `text[at:at+length]` of the card file `card.rs`."""
    span = {"file_name": "card.rs", "byte_start": at, "byte_end": at + length, "is_primary": True}
    return {"code": {"code": code} if code else None, "message": message, "spans": [span], "rendered": message}


def learnt(*errors):
    """A Spec that learnt `errors`, each (code, message, text, the name it is about)."""
    spec = pc.Spec(MODEL)
    texts = {}
    diags = []
    for code, message, text, name in errors:
        at = text.index(name)
        texts["card.rs"] = text
        diags.append(diag(code, message, text, at, len(name)))
        spec.learn(diags[-1:], texts.get)
    return spec


def method(name, text, on="dyn Game"):
    return ("E0599", f"no method named `{name}` found for mutable reference `&mut {on}` in the current scope", text, name)


class Text(unittest.TestCase):
    def test_a_closures_commas_are_not_the_calls(self):
        self.assertEqual(pc.split_args("source, move |q, asker, id| q.is(id), EffectKind::CantBlock, d"),
                         ["source", "move |q, asker, id| q.is(id)", "EffectKind::CantBlock", "d"])
        self.assertEqual(pc.split_args("applies: |q, id| true, text: \"a, b\""), ["applies: |q, id| true", 'text: "a, b"'])
        self.assertEqual(pc.split_args("a || b, f::<A, B>(x)"), ["a || b", "f::<A, B>(x)"])
        # A closure's return type has generics of its own.
        self.assertEqual(pc.split_args("|q: &dyn Query| p, |g: &mut dyn Game| -> Result<(), Suspend> { Ok(()) }, x < y"),
                         ["|q: &dyn Query| p", "|g: &mut dyn Game| -> Result<(), Suspend> { Ok(()) }", "x < y"])

    def test_how_a_name_is_used(self):
        text = "X::new(a, b)? ; Y { a: 1, b } ; Z::W ; v"
        self.assertEqual(pc.after(text, text.index("new") + 3), ("call", ["a", "b"], True))
        self.assertEqual(pc.after(text, text.index("Y") + 1), ("struct", ["a", "b"]))
        self.assertEqual(pc.after(text, text.index("Z") + 1)[:2], ("path", "W"))
        self.assertEqual(pc.after(text, len(text)), ("bare",))

    def test_a_call_whose_value_nothing_reads_is_a_statement(self):
        text = "g.note(a); let x = g.ask(b) + 1;"
        self.assertTrue(pc.statement_after(text, text.index("note") + 4))
        self.assertFalse(pc.statement_after(text, text.index("ask") + 3))


class Model(unittest.TestCase):
    def test_what_cantrip_has(self):
        self.assertEqual(MODEL.derives["Duration"], ["Clone", "Copy", "Debug", "PartialEq", "Eq", "Hash"])
        self.assertEqual(MODEL.value["Duration"], "EndOfTurn")
        self.assertEqual(MODEL.value["CardDef"], "BLANK")
        self.assertEqual(MODEL.fields["CardDef"]["triggers"], "&'static [TriggerDef]")
        self.assertEqual(MODEL.field_type("EffectKind", "index"), "u8")
        self.assertEqual(MODEL.modules, {"tokens", "counters"})
        self.assertIn("Game", MODEL.traits)

    def test_state_keeps_what_it_derives_and_a_table_is_not_state(self):
        self.assertEqual(MODEL.storage("Duration"), ["Clone", "Copy", "Debug", "PartialEq", "Eq", "Hash"])
        self.assertIsNone(MODEL.storage("CardDef"))


class Stubs(unittest.TestCase):
    def test_a_game_method_that_keeps_what_it_is_given_takes_stored_data(self):
        text = "g.add_open_effect(source, move |q, asker, id| true, EffectKind::CantBlock, Duration::EndOfTurn);"
        stubs = learnt(method("add_open_effect", text)).write()
        self.assertIn("pub trait Proposed_Game {", stubs)
        self.assertIn("fn add_open_effect<A0: crate::proposals::Stored, A1: crate::proposals::Stored, "
                      "A2: crate::proposals::Stored, A3: crate::proposals::Stored, R>(&mut self, a0: A0, a1: A1, "
                      "a2: A2, a3: A3) -> R", stubs)
        self.assertIn("impl Proposed_Game for dyn Game + '_ {}", stubs)

    def test_a_storing_name_alone_makes_a_method_keep(self):
        stubs = learnt(method("remember_choice", "g.remember_choice(x);")).write()
        self.assertIn("A0: crate::proposals::Stored", stubs)

    def test_any_other_game_method_and_every_query_method_take_data(self):
        stubs = learnt(method("reveal_hand", "g.reveal_hand(p);"),
                       method("votes", "q.votes(p) > 1", on="dyn Query")).write()
        self.assertIn("fn reveal_hand<A0: crate::proposals::Data, R>(&mut self, a0: A0)", stubs)
        self.assertIn("fn votes<A0: crate::proposals::Data, R>(&self, a0: A0) -> R", stubs)
        self.assertIn("impl Proposed_Query for dyn Query + '_ {}", stubs)

    def test_cantrip_keeps_state_as_copy_eq_hash_data(self):
        self.assertIn("pub trait Stored: Copy + Eq + std::hash::Hash {}", pc.PRELUDE)
        self.assertIn("pub trait TableEntry: Copy + Sync + 'static {}", pc.PRELUDE)

    def test_a_new_variant_of_a_cantrip_enum_carries_what_the_enum_derives(self):
        text = "Duration::DuringNextUntapStepOf(p)"
        spec = learnt(("E0599", "no variant or associated item named `DuringNextUntapStepOf` found for enum `Duration` "
                       "in the current scope", text, "DuringNextUntapStepOf"))
        stubs = spec.write()
        self.assertIn("pub trait Derives_Duration: Clone + Copy + std::fmt::Debug + PartialEq + Eq + std::hash::Hash "
                      "+ 'static {}", stubs)
        self.assertIn("pub const fn DuringNextUntapStepOf<A0: crate::proposals::Derives_Duration>(a0: A0) -> Duration "
                      "{ core::mem::forget(a0); Duration::EndOfTurn }", stubs)
        self.assertEqual(pc.rewrite(text, MODEL, spec), "crate::proposals::on::Duration::DuringNextUntapStepOf(p)")

    def test_a_new_unit_variant_is_a_constant_of_the_enum(self):
        text = "x == EffectKind::DoesntUntap"
        spec = learnt(("E0599", "no variant or associated item named `DoesntUntap` found for enum `EffectKind` in the "
                       "current scope", text, "DoesntUntap"))
        self.assertIn("pub const DoesntUntap: EffectKind = EffectKind::CantBlock;", spec.write())
        self.assertEqual(pc.rewrite(text, MODEL, spec), "x == crate::proposals::on::EffectKind::DoesntUntap")

    def test_a_new_struct_variant_is_made_by_make_and_read_by_of(self):
        made = "let k = EffectKind::Shielded { card: c, index: 0 };"
        spec = learnt(("E0599", "no variant named `Shielded` found for enum `EffectKind`", made, "Shielded"))
        stubs = spec.write()
        self.assertIn("pub struct Shielded { pub card: CardId, pub index: u8 }", stubs)
        self.assertIn("impl<F0: crate::proposals::Derives_EffectKind, F1: crate::proposals::Derives_EffectKind> "
                      "Shielded_new<F0, F1>", stubs)
        self.assertEqual(pc.rewrite(made, MODEL, spec),
                         "let k = crate::proposals::on::EffectKind::Shielded_new { card: c, index: 0 }.make();")
        read = "if let EffectKind::Shielded { card, .. } = kind { }"
        self.assertEqual(pc.rewrite(read, MODEL, spec),
                         "if let crate::proposals::on::EffectKind::Shielded { card, .. } = "
                         "crate::proposals::on::EffectKind::Shielded::of(kind) { }")

    def test_a_new_cardef_field_holds_a_table_entry(self):
        text = "pub static CARD: CardDef = CardDef { spell: None, lasting_rules: &[RULE], ..CardDef::BLANK };"
        spec = learnt(("E0560", "struct `cantrip_api::CardDef` has no field named `lasting_rules`", text,
                       "lasting_rules"))
        self.assertIn("pub const fn lasting_rules<V: crate::proposals::TableEntry>(v: V)", spec.write())
        self.assertEqual(pc.rewrite(text, MODEL, spec),
                         "pub static CARD: CardDef = { crate::proposals::field::CardDef::lasting_rules(&[RULE]); "
                         "CardDef { spell: None, ..CardDef::BLANK } };")

    def test_a_proposed_struct_holds_fn_pointers_and_data_as_a_table_does(self):
        text = ("fn rule_0(q: &dyn Query, _: &Asker, about: Target, id: ObjectId) -> bool { true }\n"
                "static RULES: &[LastingRule] = &[LastingRule { text: \"x\", rule: LastingRuleKind::CantBlock, "
                "applies: rule_0, n: 3 }];")
        at = text.index("LastingRule {")
        spec = learnt(("E0412", "cannot find type `LastingRule` in this scope", text, "LastingRule"),
                      ("E0433", "failed to resolve: use of undeclared type `LastingRuleKind`", text, "LastingRuleKind"))
        spec.learn([diag("E0422", "cannot find struct, variant or union type `LastingRule` in this scope", text, at,
                         len("LastingRule"))], {"card.rs": text}.get)
        stubs = spec.write()
        self.assertIn("pub struct LastingRule<T0> { pub applies: fn(&dyn Query, &Asker, Target, ObjectId) -> bool, "
                      "pub n: T0, pub rule: LastingRuleKind, pub text: &'static str }", stubs)
        self.assertIn("pub enum LastingRuleKind { __Proposed, CantBlock }", stubs)

    def test_a_screaming_item_of_a_type_whose_constants_are_its_own_is_a_constant_of_it(self):
        text = "let c = CardDef::EMPTY;"
        spec = learnt(("E0599", "no associated item named `EMPTY` found for struct `CardDef` in the current scope",
                       text, "EMPTY"))
        self.assertIn("pub const EMPTY: CardDef = CardDef::BLANK;", spec.write())
        self.assertEqual(pc.rewrite(text, MODEL, spec), "let c = crate::proposals::on::CardDef::EMPTY;")

    def test_a_free_function_takes_the_game_borrowed_and_anything_else_generically(self):
        text = "grant_play(g, item.object, Duration::EndOfTurn)"
        spec = learnt(("E0425", "cannot find function `grant_play` in this scope", text, "grant_play"))
        self.assertIn("pub const fn grant_play<G0: ?Sized, A1: crate::proposals::Stored, "
                      "A2: crate::proposals::Stored, R>(a0: &mut G0, a1: A1, a2: A2) -> R", spec.write())

    def test_a_call_followed_by_a_question_mark_returns_a_result(self):
        spec = learnt(method("choose_pile", "let pile = g.choose_pile(p)?;"))
        self.assertIn("-> Result<R, Suspend>", spec.write())

    def test_a_return_no_call_site_settles_defaults_by_how_it_is_used(self):
        text = "g.note_choice(x); if q.was_chosen(x) { }"
        spec = learnt(method("note_choice", text), method("was_chosen", text, on="dyn Query"))
        for name in ("note_choice", "was_chosen"):
            at = text.index(name)
            spec.learn([diag("E0282", "type annotations needed", text, at, len(name))], {"card.rs": text}.get)
        self.assertEqual(spec.rets, {"m::note_choice": "()", "m::was_chosen": "bool"})
        self.assertIn("fn was_chosen<A0: crate::proposals::Data>(&self, a0: A0) -> bool", spec.write())

    def test_a_new_mechanics_module_lives_under_the_stubs(self):
        text = "cantrip_mechanics::villainous::choose(g, p)"
        spec = learnt(("E0433", "failed to resolve: could not find `villainous` in `cantrip_mechanics`", text,
                       "villainous"))
        self.assertIn("pub mod villainous {", spec.write())
        self.assertEqual(pc.rewrite(text, MODEL, spec), "crate::proposals::mechanics::villainous::choose(g, p)")

    def test_the_stubs_converge(self):
        spec = learnt(method("add_open_effect", "g.add_open_effect(a, b);"))
        before = spec.fingerprint()
        self.assertFalse(spec.learn([], {}.get))
        self.assertEqual(spec.fingerprint(), before)


class CheckCopy(unittest.TestCase):
    def test_entries_of_a_cardef_list_are_marked_by_what_the_list_is(self):
        text = ('use cantrip_api::*;\nplanned_item!("a" => triggers: TriggerDef { text: "x" });\n'
                'planned_item!("b" => loyalty_by: Some(f));\nplanned_item!("c" => lasting_rules: R);\n')
        out = pc.base_rewrite(text, MODEL)
        self.assertIn("use cantrip_api::*; #[allow(unused_imports)] use crate::proposals::*;", out)
        self.assertIn('planned_item!("a" => triggers: TriggerDef', out)
        self.assertIn('planned_item!("b" => @value loyalty_by: Some(f));', out)
        self.assertIn('planned_item!("c" => @new lasting_rules: R);', out)

    def test_an_item_hole_is_left_as_it_stands(self):
        text = 'planned_item!("a" => static RULES: &[LastingRule] = &[]);\nplanned_item!("b" => fn rule_0() {});\n'
        self.assertEqual(pc.base_rewrite(text, MODEL), text)

    def test_a_function_given_to_a_method_that_takes_data_is_its_fn_pointer(self):
        text = ("fn who_0(q: &dyn Query, asker: &Asker, player: PlayerId) -> bool { true }\n"
                "static A: AbilityDef = ability(c, e, \"x\").activated_by(who_0).also(|q| true);\n"
                "fn f(g: &mut dyn Game) { g.add_rule(who_0); }\n")
        spec = learnt(method("activated_by", text, on="dyn Query"), method("also", text, on="dyn Query"),
                      method("add_rule", text))
        out = pc.rewrite(text, MODEL, spec)
        self.assertIn(".activated_by((who_0 as fn(&dyn Query, &Asker, PlayerId) -> bool))", out)
        self.assertIn(".also(|q| true)", out)
        # What the game state keeps is no function, nor a pointer to one.
        self.assertIn("g.add_rule(who_0)", out)

    def test_a_held_matcher_replaces_its_placeholder(self):
        text = ("fn fired_0(q: &dyn Query, asker: &Asker, event: &Event) -> Option<Fired> { None }\n"
                'planned_item!("a" => matcher: fn fired_0(q: &dyn Query, asker: &Asker, event: &Event) -> '
                "Option<Fired> { Some(x) });\n")
        self.assertEqual(pc.base_rewrite(text, MODEL), text.split("\n", 1)[1])


class Classify(unittest.TestCase):
    def error(self, code, message, rendered="", file="card.rs"):
        d = diag(code, message, "x" * 10, 0)
        d["spans"][0]["file_name"] = file
        d["rendered"] = rendered or message
        d["children"] = [{"message": rendered}] if rendered else []
        return d

    def test_a_closure_kept_in_the_game_state_is_the_proposals_fault(self):
        d = self.error("E0277", "the trait bound `{closure@card.rs:3:5}: Stored` is not satisfied",
                       "required by a bound in `proposals::Proposed_Game::add_open_effect`")
        self.assertEqual(pc.classify(d, "x" * 10, pc.Spec(MODEL)),
                         ("kept in the game state but not Copy + Eq + Hash (Proposed_Game::add_open_effect)", False))

    def test_what_rust_cannot_stub_is_the_stubs_fault(self):
        spec = pc.Spec(MODEL)
        self.assertTrue(pc.classify(self.error("E0080", f"evaluation panicked: {pc.PANIC}"), "", spec)[1])
        self.assertTrue(pc.classify(self.error("E0107", "missing generics for struct `LastingRule`"), "", spec)[1])
        self.assertTrue(pc.classify(self.error("E0308", "mismatched types", file="/w/stubs.rs"), "", spec)[1])
        self.assertTrue(pc.classify(self.error("E0282", "type annotations needed",
                                               "cannot infer type of the type parameter `R`"), "", spec)[1])

    def test_a_bare_none_given_to_a_stub_is_the_stubs_fault(self):
        text = "x(crate::proposals::mechanics::m::f(g, None)); let y: Option<u8> = keep(None);"
        d = diag("E0282", "type annotations needed", text, text.index("None"), 4)
        d["rendered"] = "cannot infer type of the type parameter `T` declared on the enum `Option`"
        self.assertTrue(pc.classify(d, text, pc.Spec(MODEL))[1])
        d = diag("E0282", "type annotations needed", text, text.rindex("None"), 4)
        d["rendered"] = "cannot infer type of the type parameter `T` declared on the enum `Option`"
        self.assertEqual(pc.classify(d, text, pc.Spec(MODEL)), ("type annotations needed", False))

    def test_a_local_the_hole_names_out_of_scope_is_the_proposals_fault(self):
        self.assertEqual(pc.classify(self.error("E0425", "cannot find value `result_0` in this scope"), "",
                                     pc.Spec(MODEL)), ("a local the hole names isn't in scope", False))


if __name__ == "__main__":
    unittest.main()
