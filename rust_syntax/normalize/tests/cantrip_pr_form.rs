//! `pr_form` keeps what a generated card does and says it as a card file does.
#![cfg(feature = "cantrip")]
use mtgish_ast::cantrip::pr_form::{render, render_two_faces, Characteristics};

#[test]
fn shared_damage_requires_the_same_source_slot_and_entire_effect() {
    let raw = r#"
use cantrip_api::*;
fn resolve(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
    let t = vec![g.state().target(item, 0)];
    let recipients = t.clone();
    let batch: Vec<(ObjectId, Target, u16)> = recipients.iter().map(|t| (item.object, *t, 4u16)).collect();
    g.damage_together(&batch);
    Ok(())
}
pub static CARD: CardDef = CardDef { spell: spell(&[cantrip_mechanics::targets::ANY], resolve), ..CardDef::BLANK };
"#;
    let render = |raw: &str| render(raw, "Sample", "This spell deals 4 damage to any target.", Characteristics::Oracle).unwrap();
    assert!(render(raw).contains("effects::damage_first_target::<4>"));
    for changed in [raw.replace("target(item, 0)", "target(item, 1)"),
                    raw.replace("(item.object, *t,", "(item.subject.object().unwrap(), *t,"),
                    raw.replace("Ok(())", "g.draw(item.controller, 1); Ok(())")] {
        assert!(!render(&changed).contains("effects::damage_first_target::<"));
    }
}

#[test]
fn narrowing_casts_preserve_both_bounds_even_for_signed_values() {
    let raw = r#"
use cantrip_api::*;
fn resolve(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
    let amount = i64::from(item.data[0]) - 65536;
    g.add_counters(item.object, CounterKind::PlusOne, amount.clamp(-32768, 32767) as i16);
    Ok(())
}
pub static CARD: CardDef = CardDef { spell: spell(&[], resolve), ..CardDef::BLANK };
"#;
    let file = render(raw, "Sample", "Remove or put counters on this permanent.", Characteristics::Oracle).unwrap();
    assert!(file.contains(".clamp(-32768, 32767) as i16"), "{file}");
}

#[test]
fn entering_counter_fields_do_not_cite_countering_a_spell() {
    let raw = r#"
use cantrip_api::*;
fn enters(_: &mut dyn Game, entering: &mut Entering) -> Result<(), Suspend> {
    entering.counters[CounterKind::PlusOne as usize] = entering.counters[CounterKind::PlusOne as usize].saturating_add(2);
    Ok(())
}
pub static CARD: CardDef = CardDef { name: "Sample", enters: Some(enters), ..CardDef::BLANK };
"#;
    let file = render(raw, "Sample", "This creature enters with two +1/+1 counters on it.", Characteristics::Oracle).unwrap();
    assert!(file.contains("CR 614.1c, 122.6"), "{file}");
    assert!(!file.contains("701.5a") && !file.contains("701.6a"), "{file}");
}

#[test]
fn countering_a_spell_cites_the_counter_action() {
    let file = render(&spell_card("g.counter_items(&[]);"), "Sample", "Counter target spell.", Characteristics::Oracle).unwrap();
    assert!(file.contains("701.6a"), "{file}");
    assert!(!file.contains("701.5a"), "{file}");
}

const ONE_TARGET: &str = r#"
#[allow(unused_imports)] use cantrip_api::*;
static ABILITY_0_TARGETS: &[cantrip_api::TargetSpec] = &[cantrip_mechanics::targets::CREATURE];
fn ability_0(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> { let asker = &Asker::new(item.object, item.controller);
let t_0_raw = announced(g, item, 0, false); let t_0 = announced(g, item, 0, true);
let objects_0 = { let q: &dyn Query = g; (t_0_raw.get(0).copied().filter(|t| t_0.contains(t)).and_then(|t: Target| t.object()).into_iter().collect::<Vec<_>>()).into_iter().filter(|&o| q.state().obj(o).became.is_none()).collect::<Vec<ObjectId>>() }; let source_0 = { let q: &dyn Query = g; Some(asker.source) }; let duration_0 = Duration::EndOfTurn;
let kind_0 = { let q: &dyn Query = g; EffectKind::Pump((2i64) as i16, (0i64) as i16) };
if let Some(source) = source_0 { cantrip_api::instruction(g, |g| { for &object in &objects_0 { g.add_effect(source, object, kind_0, duration_0);  } }); }
g.end_sentence();
 Ok(()) }
fn announced(g: &dyn Game, item: &StackItem, spec: u8, legal: bool) -> Vec<Target> { (0..g.state().targets(item).len()).filter(|&i| g.state().spec(item, i) == spec && (!legal || g.target_legal(item, i))).map(|i| g.state().target(item, i)).collect() }
pub static CARD: CardDef = CardDef { name: "Sample", oracle: true, spell: spell(ABILITY_0_TARGETS, ability_0), abilities: &[], ..CardDef::BLANK };
pub static EXTRA: &[&CardDef] = &[];
"#;

#[test]
fn a_destructuring_assignment_keeps_its_bindings_mutable() {
    let raw = r#"
use cantrip_api::*;
fn ability_0(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
    let mut cards: Vec<ObjectId> = Vec::new();
    if let Some(object) = item.subject.object() {
        (cards,) = (g.return_to_hand(&[object]),);
    }
    g.exile_all(&cards);
    Ok(())
}

pub static CARD: CardDef = CardDef { spell: spell(&[], ability_0), ..CardDef::BLANK };
"#;
    let file = render(raw, "Sample", "Return it to its owner's hand, then exile it.", Characteristics::Oracle).unwrap();
    assert!(file.contains("let mut cards"), "{file}");
    assert!(file.contains("(cards,) ="), "{file}");
}

#[test]
fn an_emblems_rule_callbacks_quote_the_ability_that_creates_it() {
    let raw = r#"
use cantrip_api::*;
fn emblem_0_ability_0_rule(_: &dyn Query, asker: &Asker, player: PlayerId, _: LossReason) -> bool { player == asker.controller }
pub static EMBLEM: CardDef = CardDef { name: "Sample emblem", has_cost: false, loss_rules: &[emblem_0_ability_0_rule], ..CardDef::BLANK };
fn ability_0(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
    cantrip_mechanics::tokens::create_emblem(g, item.controller, &EMBLEM);
    Ok(())
}
pub static CARD: CardDef = CardDef { abilities: &[ability(loyalty(0), ability_0, "0: You get an emblem with 'You can't lose the game.'")], ..CardDef::BLANK };
"#;
    let printed = "+1: Draw a card.\n0: You get an emblem with 'You can't lose the game.'";
    let file = render(raw, "Sample", printed, Characteristics::Oracle).unwrap();
    let rule = file.split("_: LossReason,").next().unwrap();
    assert!(rule.contains("You get an emblem with"), "{file}");
    assert!(!rule.contains("Draw a card"), "{file}");
}

#[test]
fn one_target_reads_as_one_object() {
    let file = render(
        ONE_TARGET,
        "Sample",
        "Target creature gets +2/+0 until end of turn.",
        Characteristics::Oracle,
    )
    .unwrap();
    println!("{file}");
    assert!(!file.contains("fn ability_"), "{file}");
    assert!(!file.contains("#[allow"), "{file}");
    assert!(!file.contains("for &object"), "{file}");
    assert!(!file.contains("announced"), "{file}");
    assert!(file.contains("let Some(target) = targets::object(g, item, 0)"), "{file}");
    assert!(file.contains("..oracle::SAMPLE"), "{file}");
    assert!(file.contains("/// \"Target creature gets +2/+0 until end of turn.\""), "{file}");
}

const EMBLEM: &str = r#"
#[allow(unused_imports)] use cantrip_api::*;
pub mod emblem_2 {
    #[allow(unused_imports)] use cantrip_api::*;
    pub mod token_1 {
        #[allow(unused_imports)] use cantrip_api::*;
        pub static CARD: CardDef = CardDef { name: token!(Cat).name, token_words: token!(Cat).token_words, oracle: false, has_cost: false, power: Some(1), keywords: Keywords::of(&[Keyword::Lifelink]), ..CardDef::BLANK };
        pub static EXTRA: &[&CardDef] = &[];
    }
    fn ability_0(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> { cantrip_mechanics::tokens::create_token(g, item.controller, &token_1::CARD); Ok(()) }
    fn ability_0_event(q: &dyn Query, asker: &Asker, event: &Event) -> Option<Fired> { None }
    pub static CARD: CardDef = CardDef { name: "Sample emblem", token_words: &[], oracle: false, has_cost: false, abilities: &[triggered(Trigger::when_asked(EventMask::ALL, ability_0_event), ability_0, "Whenever a cat attacks, make another.")], statics: &[], ..CardDef::BLANK };
    pub static EXTRA: &[&CardDef] = &[&token_1::CARD];
}
fn ability_1_target_0(q: &dyn Query, asker: &Asker, t: Target) -> bool { true }
fn ability_0(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> { cantrip_mechanics::tokens::create_emblem(g, item.controller, &emblem_2::CARD); Ok(()) }
fn ability_1(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> { g.draw(item.controller, 1); Ok(()) }
pub static CARD: CardDef = CardDef { name: "Sample", oracle: true, abilities: &[
    ability(loyalty(-7), ability_0, "[\u{2212}7]: You get an emblem with \"Whenever a cat attacks, make another.\""),
    ability(loyalty(1), ability_1, "[+1]: Target player draws a card.").targets(&[target(ability_1_target_0)]),
], ..CardDef::BLANK };
pub static EXTRA: &[&CardDef] = &[&emblem_2::CARD, &emblem_2::token_1::CARD];
"#;

#[test]
fn an_emblem_sits_beside_the_card() {
    let printed = "[+1]: Target player draws a card.\n[\u{2212}7]: You get an emblem with \"Whenever a cat attacks, make another.\"";
    let file = render(EMBLEM, "Sample", printed, Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(!file.contains("mod "), "{file}");
    assert!(!file.contains("#[allow"), "{file}");
    assert!(!file.contains("emblem_2"), "{file}");
    assert!(!file.contains("token_1"), "{file}");
    assert!(file.contains("&[&EMBLEM, &CAT]"), "{file}");
    assert!(file.contains("\"+1: Target player draws a card.\""), "{file}");
    // A target's function is documented by its own ability.
    assert!(file.contains("/// \"+1: Target player draws a card.\" (CR 115"), "{file}");
    syn::parse_file(&file).unwrap();
}

#[test]
fn a_bestowed_face_keeps_what_the_creature_says() {
    let raw = r#"
#[allow(unused_imports)] use cantrip_api::*;
fn ability_0(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> { g.draw(item.controller, 1); Ok(()) }
fn ability_0_event(q: &dyn Query, asker: &Asker, event: &Event) -> Option<Fired> { None }
const FACE: CardDef = CardDef { name: "Sample", oracle: true, power: Some(1), keywords: Keywords::of(&[]), abilities: &[triggered(Trigger::when_asked(EventMask::ALL, ability_0_event), ability_0, "Whenever you win, draw a card.")], ..CardDef::BLANK };
pub static BESTOWED: CardDef = cantrip_mechanics::bestow::bestowed(FACE);
pub static CARD: CardDef = CardDef { bestow: Some(&BESTOWED), bestow_cost: ManaCost { generic: 3, x: 0, symbols: [0, 2, 0, 0, 0, 0], hybrid: [None, None, None, None, None, None], twobrid: [0, 0, 0, 0, 0], phyrexian: [0, 0, 0, 0, 0] }, ..FACE };
pub static EXTRA: &[&CardDef] = &[&BESTOWED];
"#;
    let printed = "Bestow {3}{U}{U}\nWhenever you win, draw a card.";
    let file = render(raw, "Sample", printed, Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(file.contains("..FACE"), "{file}");
    assert!(file.contains("..oracle::SAMPLE"), "{file}");
    assert!(!file.contains("power:"), "{file}");
    assert!(file.contains("abilities:"), "{file}");
    assert!(!file.contains("ManaCost"), "{file}");
    assert!(!file.contains("Keywords::of"), "{file}");
}

#[test]
fn what_a_target_owns_is_not_the_target() {
    let raw = r#"
#[allow(unused_imports)] use cantrip_api::*;
static ABILITY_0_TARGETS: &[cantrip_api::TargetSpec] = &[cantrip_mechanics::targets::PLAYER];
fn ability_0(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
let t_0_raw = announced(g, item, 0, false); let t_0 = announced(g, item, 0, true);
let objects_0 = { let q: &dyn Query = g; q.state().battlefield().iter().copied().filter(|&id| t_0_raw.get(0).copied().filter(|t| t_0.contains(t)).and_then(|t: Target| t.player()) == Some(q.controller(id))).collect::<Vec<ObjectId>>() };
g.destroy_all(&objects_0);
 Ok(()) }
fn announced(g: &dyn Game, item: &StackItem, spec: u8, legal: bool) -> Vec<Target> { (0..g.state().targets(item).len()).filter(|&i| g.state().spec(item, i) == spec && (!legal || g.target_legal(item, i))).map(|i| g.state().target(item, i)).collect() }
pub static CARD: CardDef = CardDef { name: "Sample", oracle: true, spell: spell(ABILITY_0_TARGETS, ability_0), ..CardDef::BLANK };
pub static EXTRA: &[&CardDef] = &[];
"#;
    let file = render(raw, "Sample", "Destroy all permanents target player controls.", Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(!file.contains("targets::player(g, item, 0)"), "{file}");
    assert!(file.contains("q.controller(id)"), "{file}");
}

#[test]
fn a_top_level_loop_over_one_target_unrolls() {
    let raw = r#"
#[allow(unused_imports)] use cantrip_api::*;
static ABILITY_0_TARGETS: &[cantrip_api::TargetSpec] = &[cantrip_mechanics::targets::PLAYER];
fn ability_0(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
let t_0_raw = announced(g, item, 0, false); let t_0 = announced(g, item, 0, true);
let players_0 = t_0_raw.get(0).copied().filter(|t| t_0.contains(t)).and_then(|t: Target| t.player()).into_iter().collect::<Vec<PlayerId>>();
for player in players_0 { g.lose_life(player, 4); }
 Ok(()) }
fn announced(g: &dyn Game, item: &StackItem, spec: u8, legal: bool) -> Vec<Target> { (0..g.state().targets(item).len()).filter(|&i| g.state().spec(item, i) == spec && (!legal || g.target_legal(item, i))).map(|i| g.state().target(item, i)).collect() }
pub static CARD: CardDef = CardDef { name: "Sample", oracle: true, spell: spell(ABILITY_0_TARGETS, ability_0), ..CardDef::BLANK };
pub static EXTRA: &[&CardDef] = &[];
"#;
    let file = render(raw, "Sample", "Target player loses 4 life.", Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(!file.contains("for "), "{file}");
    assert!(file.contains("g.lose_life(player, 4)"), "{file}");
}

#[test]
fn a_function_that_is_a_shared_effect_is_named_for_it() {
    let raw = r#"
#[allow(unused_imports)] use cantrip_api::*;
static ABILITY_0_TARGETS: &[cantrip_api::TargetSpec] = &[cantrip_mechanics::targets::CREATURE];
fn ability_0(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
let t_0_raw = announced(g, item, 0, false); let t_0 = announced(g, item, 0, true);
let objects_0 = { let q: &dyn Query = g; (t_0_raw.get(0).copied().filter(|t| t_0.contains(t)).and_then(|t: Target| t.object()).into_iter().collect::<Vec<_>>()).into_iter().filter(|&o| q.state().obj(o).became.is_none()).collect::<Vec<ObjectId>>() };
g.destroy_all(&objects_0);
g.end_sentence();
 Ok(()) }
fn announced(g: &dyn Game, item: &StackItem, spec: u8, legal: bool) -> Vec<Target> { (0..g.state().targets(item).len()).filter(|&i| g.state().spec(item, i) == spec && (!legal || g.target_legal(item, i))).map(|i| g.state().target(item, i)).collect() }
pub static CARD: CardDef = CardDef { name: "Sample", oracle: true, spell: spell(ABILITY_0_TARGETS, ability_0), ..CardDef::BLANK };
pub static EXTRA: &[&CardDef] = &[];
"#;
    let file = render(raw, "Sample", "Destroy target creature.", Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(file.contains("effects::destroy_first_target"), "{file}");
    assert!(!file.contains("fn "), "{file}");
}

#[test]
fn a_list_being_built_is_not_left_out() {
    let raw = r#"
#[allow(unused_imports)] use cantrip_api::*;
fn ability_0(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
let mut picks_0: Vec<(PlayerId, ObjectId)> = Vec::new();
let taken = choose_things(g)?;
for object in taken { picks_0.push((item.controller, object)); }
let mut done_0: i64 = 0;
done_0 += i64::from(g.sacrifice_each(&picks_0).len() as u32);
 Ok(()) }
pub static CARD: CardDef = CardDef { name: "Sample", oracle: true, spell: spell(&[], ability_0), ..CardDef::BLANK };
pub static EXTRA: &[&CardDef] = &[];
"#;
    let file = render(raw, "Sample", "Sacrifice a thing.", Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(file.contains("picks_0.push((item.controller, object));"), "{file}");
    assert!(file.contains("g.sacrifice_each(&picks_0)"), "{file}");
    assert!(!file.contains("done_0"), "{file}");
}

#[test]
fn a_statement_called_for_what_it_changes_stays() {
    let raw = r#"
#[allow(unused_imports)] use cantrip_api::*;
fn ability_0_apply_0(q: &dyn LayerQuery, source: ObjectId, chars: &mut Chars) { chars.set_land_subtypes(&[Subtype::Mountain]); }
fn ability_0_affects(q: &dyn LayerQuery, source: ObjectId, id: ObjectId, so_far: &Chars) -> bool { true }
pub static CARD: CardDef = CardDef { name: "Sample", oracle: true, statics: &[sets_land_subtypes(ability_0_affects, ability_0_apply_0, "Nonbasic lands are Mountains.")], ..CardDef::BLANK };
pub static EXTRA: &[&CardDef] = &[];
"#;
    let file = render(raw, "Sample", "Nonbasic lands are Mountains.", Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(file.contains("chars.set_land_subtypes(&[Subtype::Mountain]);"), "{file}");
}

#[test]
fn a_named_token_keeps_what_it_is() {
    let raw = r#"
#[allow(unused_imports)] use cantrip_api::*;
pub mod token_1 {
    #[allow(unused_imports)] use cantrip_api::*;
    pub static CARD: CardDef = CardDef { name: "Cadet", token_words: &[], oracle: false, cost: ManaCost::ZERO, has_cost: false, types: TypeSet::of(&[CardType::Creature]), subtypes: SubtypeSet::of(&[Subtype::Wizard, Subtype::Soldier]), colors: Some(ColorSet(0)), power: Some(2), toughness: Some(2), keywords: Keywords::of(&[]), ..CardDef::BLANK };
    pub static EXTRA: &[&CardDef] = &[];
}
fn ability_0(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> { let def = g.card_id(&token_1::CARD); g.create_tokens(&[(def, item.controller)]); Ok(()) }
pub static CARD: CardDef = CardDef { name: "Sample", oracle: true, spell: spell(&[], ability_0), ..CardDef::BLANK };
pub static EXTRA: &[&CardDef] = &[&token_1::CARD];
"#;
    let file = render(raw, "Sample", "Create a 2/2 colorless Wizard Soldier creature token named Cadet.", Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(file.contains("static CADET"), "{file}");
    assert!(file.contains("types: TypeSet::of(&[CardType::Creature])"), "{file}");
    assert!(file.contains("Subtype::Wizard"), "{file}");
    assert!(file.contains("power: Some(2)"), "{file}");
    assert!(file.contains("colors: Some(ColorSet(0))"), "{file}");
}

#[test]
fn a_target_read_after_the_game_changed_keeps_the_first_answer() {
    let raw = r#"
#[allow(unused_imports)] use cantrip_api::*;
static ABILITY_0_TARGETS: &[cantrip_api::TargetSpec] = &[cantrip_mechanics::targets::CREATURE];
fn ability_0(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
let t_0_raw = announced(g, item, 0, false); let t_0 = announced(g, item, 0, true);
g.draw(item.controller, 1);
let objects_0 = t_0_raw.get(0).copied().filter(|t| t_0.contains(t)).and_then(|t: Target| t.object()).into_iter().collect::<Vec<_>>();
g.destroy_all(&objects_0);
 Ok(()) }
fn announced(g: &dyn Game, item: &StackItem, spec: u8, legal: bool) -> Vec<Target> { (0..g.state().targets(item).len()).filter(|&i| g.state().spec(item, i) == spec && (!legal || g.target_legal(item, i))).map(|i| g.state().target(item, i)).collect() }
pub static CARD: CardDef = CardDef { name: "Sample", oracle: true, spell: spell(ABILITY_0_TARGETS, ability_0), ..CardDef::BLANK };
pub static EXTRA: &[&CardDef] = &[];
"#;
    let file = render(raw, "Sample", "Draw a card. Destroy target creature.", Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(!file.contains("targets::object(g, item, 0)"), "{file}");
    // Read once, as the item starts to resolve: its one target, legal then, or it would not resolve (CR 608.2b).
    assert!(file.contains("let t_0 = vec![g.state().target(item, 0)];"), "{file}");
    assert!(!file.contains("announced"), "{file}");
}

#[test]
fn a_target_of_a_choice_of_several_modes_is_asked_whether_it_is_legal() {
    let raw = r#"
#[allow(unused_imports)] use cantrip_api::*;
static ABILITY_0_TARGETS: &[cantrip_api::TargetSpec] = &[cantrip_mechanics::targets::CREATURE];
fn ability_0(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
let t_0_raw = announced(g, item, 0, false); let t_0 = announced(g, item, 0, true);
g.draw(item.controller, 1);
let objects_0 = t_0_raw.get(0).copied().filter(|t| t_0.contains(t)).and_then(|t: Target| t.object()).into_iter().collect::<Vec<_>>();
g.destroy_all(&objects_0);
 Ok(()) }
fn announced(g: &dyn Game, item: &StackItem, spec: u8, legal: bool) -> Vec<Target> { (0..g.state().targets(item).len()).filter(|&i| g.state().spec(item, i) == spec && (!legal || g.target_legal(item, i))).map(|i| g.state().target(item, i)).collect() }
pub static CARD: CardDef = CardDef { name: "Sample", oracle: true, spell: spell(ABILITY_0_TARGETS, ability_0), ..CardDef::BLANK };
pub static EXTRA: &[&CardDef] = &[];
static ABILITY_1_MODES: ModalDef = ModalDef::new(&[]).choose(1, 2);
"#;
    let file = render(raw, "Sample", "Draw a card. Destroy target creature.", Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(!file.contains("targets::object(g, item, 0)"), "{file}");
    // With more than one mode chosen, the item resolves for another mode's target though this one is illegal (CR 608.2b).
    assert!(file.contains("announced(g, item, 0, true)"), "{file}");
}

#[test]
fn what_doesnt_need_the_target_happens_without_it() {
    let raw = r#"
#[allow(unused_imports)] use cantrip_api::*;
static ABILITY_0_TARGETS: &[cantrip_api::TargetSpec] = &[cantrip_mechanics::targets::PLAYER];
fn ability_0(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
let t_0_raw = announced(g, item, 0, false); let t_0 = announced(g, item, 0, true);
let players_0 = t_0_raw.get(0).copied().filter(|t| t_0.contains(t)).and_then(|t: Target| t.player()).into_iter().collect::<Vec<PlayerId>>();
for player in players_0 { g.lose_life(player, 2); }
g.end_sentence();
g.gain_life(item.controller, 2);
 Ok(()) }
fn announced(g: &dyn Game, item: &StackItem, spec: u8, legal: bool) -> Vec<Target> { (0..g.state().targets(item).len()).filter(|&i| g.state().spec(item, i) == spec && (!legal || g.target_legal(item, i))).map(|i| g.state().target(item, i)).collect() }
pub static CARD: CardDef = CardDef { name: "Sample", oracle: true, spell: spell(ABILITY_0_TARGETS, ability_0), ..CardDef::BLANK };
pub static EXTRA: &[&CardDef] = &[];
"#;
    let file = render(raw, "Sample", "Target opponent loses 2 life and you gain 2 life.", Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(!file.contains("return Ok"), "{file}");
    assert!(file.contains("if let Some(player) = players_0"), "{file}");
    assert!(file.contains("g.gain_life(item.controller, 2)"), "{file}");
}

#[test]
fn a_difference_no_one_reads_goes_with_what_it_measured() {
    let raw = r#"
#[allow(unused_imports)] use cantrip_api::*;
fn ability_0(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
let before = g.history().drawn(item.controller) as i64;
g.draw(item.controller, 1);
g.history().drawn(item.controller) as i64 - before;
let life = i64::from(g.state().player(item.controller).life);
g.lose_life(item.controller, 1);
(i64::from(g.state().player(item.controller).life) - life).abs();
 Ok(()) }
pub static CARD: CardDef = CardDef { name: "Sample", oracle: true, spell: spell(&[], ability_0), ..CardDef::BLANK };
pub static EXTRA: &[&CardDef] = &[];
"#;
    let file = render(raw, "Sample", "Draw a card, then lose 1 life.", Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(!file.contains("before"), "{file}");
    assert!(!file.contains(".abs()"), "{file}");
    assert!(file.contains("g.draw(item.controller, 1);"), "{file}");
    assert!(file.contains("g.lose_life(item.controller, 1);"), "{file}");
}

fn spell_card(body: &str) -> String {
    format!(
        r#"
#[allow(unused_imports)] use cantrip_api::*;
fn ability_0(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {{ {body} Ok(()) }}
pub static CARD: CardDef = CardDef {{ name: "Sample", oracle: true, spell: spell(&[], ability_0), abilities: &[], ..CardDef::DEFAULT }};
pub static EXTRA: &[&CardDef] = &[];
"#
    )
}

#[test]
fn each_opponent_loses_life_together() {
    let body = "let players_0 = { let q: &dyn Query = g; q.state().opponents(item.controller).collect::<Vec<PlayerId>>() };
        for player in players_0 { g.lose_life(player, 2); }";
    let file = render(&spell_card(body), "Sample", "Each opponent loses 2 life.", Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(file.contains("g.each_opponent_loses_life(item.controller, 2);"), "{file}");
    assert!(!file.contains("for player"), "{file}");
}

#[test]
fn a_list_of_opponents_read_again_is_left_alone() {
    let body = "let players_0 = { let q: &dyn Query = g; q.state().opponents(item.controller).collect::<Vec<PlayerId>>() };
        for player in players_0 { g.lose_life(player, 2); }
        for player in players_0 { g.draw(player, 1); }";
    let file = render(&spell_card(body), "Sample", "Each opponent loses 2 life and draws a card.", Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(!file.contains("each_opponent_loses_life"), "{file}");
}

#[test]
fn what_a_spell_does_is_not_its_keyword_line() {
    let body = "g.gain_life(item.controller, 3);";
    for printed in [
        "You gain 3 life.\nFlashback—{1}{G}, Pay 3 life. (You may cast this card from your graveyard for its flashback cost.)",
        "You gain 3 life.\nCycling {2} ({2}, Discard this card: Draw a card.)",
        "Foretell {1}{R}{R} (During your turn, you may pay {2} and exile this card from your hand face down.)\nYou gain 3 life.",
    ] {
        let file = render(&spell_card(body), "Sample", printed, Characteristics::Oracle).unwrap();
        println!("{file}");
        assert!(file.contains("/// \"You gain 3 life.\""), "{file}");
        assert!(file.contains("fn gain_3_life") || file.contains("fn you_gain_3_life"), "{file}");
    }
}

fn token_card(fields: &str) -> String {
    format!(
        r#"
#[allow(unused_imports)] use cantrip_api::*;
pub static INSECT: CardDef = CardDef {{ {fields} ..token!(Insect) }};
fn ability_0(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {{ let def = g.card_id(&INSECT); g.create_tokens_worded(&[(def, item.controller)], false, None, g.words(item.object))?; Ok(()) }}
pub static CARD: CardDef = CardDef {{ name: "Sample", oracle: true, spell: spell(&[], ability_0), abilities: &[], ..CardDef::DEFAULT }};
pub static EXTRA: &[&CardDef] = &[&INSECT];
"#
    )
}

const PLAIN_INSECT: &str = "types: TypeSet::of(&[CardType::Creature]), colors: Some(ColorSet(16)), power: Some(1), toughness: Some(1),";

#[test]
fn a_token_cantrip_shares_is_named() {
    // `shared_tokens.tsv` says which: Ant Queen's Insect is Cantrip's.
    let text = "Create a 1/1 green Insect creature token.";
    let file = render(&token_card(PLAIN_INSECT), "Ant Queen", text, Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(file.contains("use cantrip_mechanics::tokens;"), "{file}");
    assert!(file.contains("g.card_id(&tokens::INSECT)"), "{file}");
    assert!(file.contains("&[&tokens::INSECT]"), "{file}");
    assert!(!file.contains("pub static INSECT"), "{file}");
}

#[test]
fn a_token_that_differs_in_anything_is_its_own() {
    let text = "Create a 1/1 black Insect creature token.";
    let fields = PLAIN_INSECT.replace("ColorSet(16)", "ColorSet(4)");
    let file = render(&token_card(&fields), "Sample", text, Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(!file.contains("tokens::"), "{file}");
    assert!(file.contains("..token!(Insect)"), "{file}");
    let fields = format!("{PLAIN_INSECT} keywords: Keywords::of(&[Keyword::Flying]),");
    let file = render(&token_card(&fields), "Sample", text, Characteristics::Oracle).unwrap();
    assert!(!file.contains("tokens::"), "{file}");
}

#[test]
fn a_guard_stays_on_the_line_it_guards() {
    let body = "let Some(target) = targets::object(g, item, 0) else { return Ok(()); }; g.destroy(target);";
    let file = render(&spell_card(body), "Sample", "Destroy target artifact.", Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(file.contains("else { return Ok(()) };"), "{file}");
    assert!(!file.contains("return Ok(());\n"), "{file}");
}

#[test]
fn an_events_pattern_stays_on_one_line() {
    let body = "let seen = g.history().all().iter().rev().find_map(|r| match r.event {
            Event::ZoneChange { old, new, reason: ZoneReason::Resolve, .. } if new == item.object => Some(old),
            _ => None,
        });
        if seen.is_some() { g.gain_life(item.controller, 1); }";
    let file = render(&spell_card(body), "Sample", "You gain 1 life.", Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(file.contains("Event::ZoneChange { old, new, reason: ZoneReason::Resolve, .. } if"), "{file}");
}

fn trigger_card(function: &str) -> String {
    format!(
        "{function}
        fn draw(g: &mut Game, item: &StackItem) -> GameResult {{ g.draw(item.controller, 1)?; Ok(()) }}
        pub static CARD: CardDef = CardDef {{
            abilities: &[triggered(when_it_happens, draw, \"Whenever a spell is cast, draw a card.\")],
            ..BLANK
        }};"
    )
}

#[test]
fn a_trigger_says_its_event_first() {
    let raw = trigger_card(
        "fn when_it_happens(q: &dyn Query, asker: &Asker, event: &Event) -> Option<Fired> {
            if let Event::SpellCast { spell, player, .. } = event {
                let id = *spell;
                if asker.controller == *player { return Fired::about(Target::Object(id)); }
            }
            None
        }",
    );
    let file = render(&raw, "Sample", "Whenever a spell is cast, draw a card.", Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(file.contains("let Event::SpellCast { spell, player, .. } = event else { return None };\n"), "{file}");
    assert!(!file.contains("if let Event::"), "{file}");
    assert!(file.contains("    None\n}"), "{file}");
}

#[test]
fn two_events_keep_their_branches() {
    let raw = trigger_card(
        "fn when_it_happens(q: &dyn Query, asker: &Asker, event: &Event) -> Option<Fired> {
            if let Event::SpellCast { spell, .. } = event { return Fired::about(Target::Object(*spell)); }
            if let Event::Attacks { attacker, .. } = event { return Fired::about(Target::Object(*attacker)); }
            None
        }",
    );
    let file = render(&raw, "Sample", "Whenever a spell is cast, draw a card.", Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(file.contains("if let Event::SpellCast"), "{file}");
    assert!(file.matches("Fired::about").count() == 2, "{file}");
}

#[test]
fn a_cost_is_written_as_its_symbols() {
    let raw = "pub static CARD: CardDef = CardDef {
        madness: Some(ManaCost { generic: 3, x: 0, symbols: [0, 1, 0, 0, 0, 0], hybrid: [None, None, None, None, None, None],
            twobrid: [0, 0, 0, 0, 0], phyrexian: [0, 0, 0, 0, 0] }),
        foretell: Some(ManaCost { generic: 0, x: 1, symbols: [0, 0, 0, 2, 0, 0], ..ManaCost::ZERO }),
        ..BLANK
    };";
    let file = render(raw, "Sample", "Madness {3}{U}", Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(file.contains("madness: Some(mana(\"{3}{U}\"))"), "{file}");
    assert!(file.contains("foretell: Some(mana(\"{X}{R}{R}\"))"), "{file}");
    assert!(!file.contains("generic:"), "{file}");
}

#[test]
fn a_cost_of_nothing_is_zero_and_a_hybrid_one_stays() {
    let raw = "pub static CARD: CardDef = CardDef {
        madness: Some(ManaCost { generic: 0, x: 0, symbols: [0, 0, 0, 0, 0, 0], hybrid: [None, None, None, None, None, None],
            twobrid: [0, 0, 0, 0, 0], phyrexian: [0, 0, 0, 0, 0] }),
        foretell: Some(ManaCost { generic: 1, x: 0, symbols: [0, 0, 0, 0, 0, 0], hybrid: [Some((Mana::W, Mana::U)), None, None, None, None, None],
            twobrid: [0, 0, 0, 0, 0], phyrexian: [0, 0, 0, 0, 0] }),
        ..BLANK
    };";
    let file = render(raw, "Sample", "Madness {0}", Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(file.contains("madness: Some(ManaCost::ZERO)"), "{file}");
    assert!(file.contains("Some((Mana::W, Mana::U))"), "{file}");
}

#[test]
fn a_trigger_that_always_fires_ends_in_what_it_fires() {
    for last in ["return Fired::about(Target::Object(id));", "{ return Fired::about(Target::Object(id)); }"] {
        let raw = trigger_card(&format!(
            "fn when_it_happens(q: &dyn Query, asker: &Asker, event: &Event) -> Option<Fired> {{
                if let Event::SpellCast {{ spell, .. }} = event {{
                    let id = *spell;
                    {last}
                }}
                None
            }}"
        ));
        let file = render(&raw, "Sample", "Whenever a spell is cast, draw a card.", Characteristics::Oracle).unwrap();
        println!("{file}");
        assert!(file.contains("Fired::about(Target::Object(id))\n}"), "{file}");
        assert!(!file.contains("None\n}"), "{file}");
        assert!(!file.contains("return Fired"), "{file}");
    }
}

#[test]
fn mana_of_any_color_is_the_shared_ability() {
    for amount in ["1", "1u32"] {
        let raw = format!(
            "fn ability_0_mana(g: &mut dyn Game, asker: &Asker, chosen: Option<Color>) -> Result<(), Suspend> {{
                let Some(color) = chosen else {{ return Ok(()); }};
                g.add_mana(asker.controller, color.mana(), {amount});
                Ok(())
            }}
            pub static CARD: CardDef = CardDef {{
                abilities: &[mana_ability(TAP, add_by(ability_0_mana).choose_color().colors(ColorSet(31)).yields_any(1), \"{{T}}: Add one mana of any color.\")],
                ..BLANK
            }};"
        );
        let file = render(&raw, "Sample", "{T}: Add one mana of any color.", Characteristics::Oracle).unwrap();
        println!("{file}");
        assert!(file.contains("add_any_color(1)"), "{file}");
        assert!(!file.contains("yields_any"), "{file}");
        assert!(!file.contains("fn "), "{file}");
    }
}

const TWO_FACES: &str = r#"
#[allow(unused_imports)] use cantrip_api::*;
fn ability_0(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> { g.may_pay(item.controller, item.object, item.object, life(1), &PAY_QUESTION)?; g.draw(item.controller, 1); Ok(()) }
static PAY_QUESTION: Question = Question::new("Pay", "pay {cost}?", "", "Pay {cost}", "Decline");
pub static CARD: CardDef = CardDef { name: "Front Face", oracle: true, spell: spell(&[], ability_0), back: Some(&back::CARD), ..CardDef::BLANK };
pub static EXTRA: &[&CardDef] = &[];
pub mod back {
#[allow(unused_imports)] use cantrip_api::*;
fn ability_0(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> { g.may_pay(item.controller, item.object, item.object, life(1), &PAY_QUESTION)?; g.gain_life(item.controller, 2); Ok(()) }
static PAY_QUESTION: Question = Question::new("Pay", "pay {cost}?", "", "Pay {cost}", "Decline");
pub static CARD: CardDef = CardDef { name: "Back Face", oracle: true, spell: spell(&[], ability_0), ..CardDef::BLANK };
pub static EXTRA: &[&CardDef] = &[];
}
"#;

#[test]
fn two_faces_are_one_file_with_one_namespace() {
    let faces = [("Back Face", "You gain 2 life."), ("Front Face", "Draw a card.")];
    let file = render_two_faces(TWO_FACES, "back", faces).unwrap();
    println!("{file}");
    assert!(!file.contains("mod "), "{file}");
    assert!(!file.contains("back::CARD"), "{file}");
    assert!(file.contains("pub static BACK: CardDef"), "{file}");
    assert!(file.contains("back: Some(&BACK)"), "{file}");
    assert!(file.contains("..oracle::BACK_FACE"), "{file}");
    assert!(file.contains("..oracle::FRONT_FACE"), "{file}");
    // What the faces share is there once.
    assert_eq!(file.matches("static PAY_QUESTION").count(), 1, "{file}");
    assert!(file.find("pub static BACK").unwrap() < file.find("pub static CARD").unwrap(), "{file}");
    syn::parse_file(&file).unwrap();
}

#[test]
fn a_name_the_faces_mean_differently_is_numbered() {
    let inner = "static PAY_QUESTION: Question = Question::new(\"Pay\", \"pay {cost}?\", \"\", \"Pay {cost}\", \"Decline\");\npub static CARD: CardDef = CardDef { name: \"Back Face\"";
    let other = inner.replace("\"Pay\", \"pay", "\"Other\", \"pay");
    let raw = TWO_FACES.replace(inner, &other);
    let faces = [("Front Face", "Draw a card."), ("Back Face", "You gain 2 life.")];
    let file = render_two_faces(&raw, "back", faces).unwrap();
    println!("{file}");
    assert!(file.contains("static PAY_QUESTION:"), "{file}");
    assert!(file.contains("static PAY_QUESTION_2:"), "{file}");
    assert!(file.contains("&PAY_QUESTION_2"), "{file}");
    syn::parse_file(&file).unwrap();
}

const LEAVES_DAMAGE: &str = r#"
#[allow(unused_imports)] use cantrip_api::*;
#[allow(unused_variables)] fn ability_0_event(q: &dyn Query, asker: &Asker, event: &Event) -> Option<Fired> { if let Event::ZoneChange { old, new, from, .. } = event { let id = *old; let subject = Target::Object(id); let data = [0u32, u32::MAX]; if from.is(ZoneKind::Battlefield) && Some(asker.source) == Some(id) { return Some(Fired { subject, data }); } }
 None }
fn ability_0(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> { let asker = &Asker::new(item.object, item.controller);
let source_0 = { let q: &dyn Query = g; item.subject.object() }; let amount_0 = { let q: &dyn Query = g; (1i64).max(0) as u16 }; let recipients_0 = { let q: &dyn Query = g; (q.state().battlefield().iter().copied().filter(|&id| q.chars(id).is(CardType::Creature)).collect::<Vec<ObjectId>>()).into_iter().map(Target::Object).collect::<Vec<Target>>() }; if let Some(source) = source_0 { let batch: Vec<(ObjectId, Target, u16)> = recipients_0.iter().map(|t| (source, *t, amount_0)).collect(); g.damage_together(&batch); }
g.end_sentence();
 Ok(()) }
pub static CARD: CardDef = CardDef { name: "Sample", oracle: true, abilities: &[triggered(when_asked(EventMask(EventMask::ZONE_CHANGE.0), ability_0_event), ability_0, "When this enchantment leaves the battlefield, it deals 1 damage to each creature.")], ..CardDef::BLANK };
pub static EXTRA: &[&CardDef] = &[];
"#;

/// Goblin Shrine: the trigger's subject is always the enchantment that left, so an `if let` around the damage has a closing
/// brace no game runs, which Cantrip's coverage gate refuses. The guard is a `let .. else` on one line instead.
#[test]
fn a_guard_at_the_end_is_a_let_else() {
    let printed = "When this enchantment leaves the battlefield, it deals 1 damage to each creature.";
    let file = render(LEAVES_DAMAGE, "Sample", printed, Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(file.contains("let Some(source) = source_0 else { return Ok(()) };"), "{file}");
    assert!(!file.contains("if let Some(source)"), "{file}");
}

const SPECTRUM: &str = r##"
#[allow(unused_imports)] use cantrip_api::*;
fn ability_0(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> { let asker = &Asker::new(item.object, item.controller);
let amount_0 = { let q: &dyn Query = g; (Some(asker.source).map_or(0, |o: ObjectId| { let spent = q.obj(o).spent; [Mana::W, Mana::U, Mana::B, Mana::R, Mana::G, Mana::C].into_iter().filter(|mana| *mana != Mana::C).filter(|mana| spent.get(*mana) > 0).count() as i64 })) as u32 }; let players_0 = { let q: &dyn Query = g; Some(asker.controller).into_iter().collect::<Vec<PlayerId>>() };
for player in players_0 { g.draw(player, amount_0); }
g.end_sentence();
let amount_1 = { let q: &dyn Query = g; (2i64).max(0) as u32 }; let players_1: Vec<PlayerId> = { let q: &dyn Query = g; Some(asker.controller).into_iter().collect::<Vec<PlayerId>>() }.into_iter().collect(); let mut cards_1 = Vec::new(); for player in players_1 { cards_1.extend(cantrip_mechanics::choose_from_hand(g, player, asker.source, amount_1)?); } let discarded_1 = g.discard_together(&cards_1);
g.end_sentence();
 Ok(()) }

pub static CARD: CardDef = CardDef { name: "Brilliant Spectrum", token_words: &[], oracle: true, cost: ManaCost { generic: 3, x: 0, symbols: [0, 1, 0, 0, 0, 0], hybrid: [None,None,None,None,None,None], twobrid: [0, 0, 0, 0, 0], phyrexian: [0, 0, 0, 0, 0] }, has_cost: true, types: TypeSet::of(&[CardType::Sorcery]), subtypes: SubtypeSet::of(&[]), supertypes: Supertypes { basic: false, legendary: false, snow: false }, colors: None, power: None, toughness: None, loyalty: None, defense: None, keywords: Keywords::of(&[]), protection: ColorSet(0), spell: spell(&[], ability_0), abilities: &[], statics: &[], replacements: &[], preventions: &[], block_rules: &[], block_requirements: &[], attack_rules: &[], untap_bans: &[], delayed: &[], enters: None, modal: None, enchant: None, ..CardDef::BLANK };
pub static EXTRA: &[&CardDef] = &[];
"##;

const ABOMINATION: &str = r##"
#[allow(unused_imports)] use cantrip_api::*;
#[allow(unused_variables)] fn ability_1_event(q: &dyn Query, asker: &Asker, event: &Event) -> Option<Fired> { if let Event::Damage { source, target, amount, kind, .. } = event { let (source, amount, combat) = (*source, *amount, kind.is_combat()); let subject = Target::Object(source); let data = [u32::from(amount), match target { Target::Player(p) => u32::from(p.0), _ => u32::MAX }]; if combat && !q.state().obj(source).zone.is(ZoneKind::Stack) && ((q.chars(source).has_subtype(q.words(asker.source).subtype(Subtype::Gamma)) || q.chars(source).has_subtype(q.words(asker.source).subtype(Subtype::Villain))) && Some(asker.controller) == Some(q.controller(source))) && matches!(*target, Target::Player(p) if true) { return Some(Fired { subject, data }); } }
 None }
fn ability_1(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> { let asker = &Asker::new(item.object, item.controller);
let mut counters_0: Vec<(ObjectId, CounterKind, i16)> = Vec::new();
let amount = { let q: &dyn Query = g; (i64::from(item.data[0])).min(i16::MAX.into()) as i16 }; for t in { let q: &dyn Query = g; (Some(asker.source).into_iter().collect::<Vec<ObjectId>>()).into_iter().map(Target::Object).collect::<Vec<Target>>() } { if let Target::Object(object) = t { counters_0.push((object, CounterKind::PlusOne, amount)); } }
let had_0: Vec<_> = counters_0.iter().map(|c| g.state().obj(c.0).counters(c.1)).collect(); g.add_counters_together(&counters_0); let applied_0: Vec<ObjectId> = counters_0.iter().zip(had_0).filter(|(c, had)| g.state().obj(c.0).counters(c.1) > *had).map(|(c, _)| c.0).collect();
g.end_sentence();
 Ok(()) }

pub static CARD: CardDef = CardDef { name: "Abomination, Irradiated Brute", token_words: &[], oracle: true, cost: ManaCost { generic: 2, x: 0, symbols: [0, 0, 0, 0, 2, 0], hybrid: [None,None,None,None,None,None], twobrid: [0, 0, 0, 0, 0], phyrexian: [0, 0, 0, 0, 0] }, has_cost: true, types: TypeSet::of(&[CardType::Creature]), subtypes: SubtypeSet::of(&[Subtype::Gamma,Subtype::Villain]), supertypes: Supertypes { basic: false, legendary: true, snow: false }, colors: None, power: Some(4), toughness: Some(4), loyalty: None, defense: None, keywords: Keywords::of(&[Keyword::Trample]), protection: ColorSet(0), spell: None, abilities: &[triggered(when_asked(EventMask(EventMask::DAMAGE.0), ability_1_event), ability_1, "Whenever a Gamma or Villain you control deals combat damage to a player, put that many +1/+1 counters on Abomination.").targets(&[])], statics: &[], replacements: &[], preventions: &[], block_rules: &[], block_requirements: &[], attack_rules: &[], untap_bans: &[], delayed: &[], enters: None, modal: None, enchant: None, ..CardDef::BLANK };
pub static EXTRA: &[&CardDef] = &[];
"##;

/// An amount that cannot be negative is cast as it is, and a read of the card's own object reads it: a floor or a fallback
/// no game reaches is code no scenario can test.
#[test]
fn an_amount_that_cannot_be_negative_has_no_floor() {
    let spectrum = render(
        SPECTRUM,
        "Brilliant Spectrum",
        "Converge — Draw X cards, where X is the number of colors of mana spent to cast this spell. Then discard two cards.",
        Characteristics::Oracle,
    )
    .unwrap();
    assert!(!spectrum.contains(".max(0)"), "{spectrum}");
    assert!(!spectrum.contains("map_or"), "{spectrum}");
    assert!(spectrum.contains("let spent = q.obj(item.object).spent;"), "{spectrum}");
    assert!(spectrum.contains(".count() as i64\n        }) as u32"), "{spectrum}");
    let abomination = render(
        ABOMINATION,
        "Abomination, Irradiated Brute",
        "Whenever a Gamma or Villain you control deals combat damage to a player, put that many +1/+1 counters on Abomination.",
        Characteristics::Oracle,
    )
    .unwrap();
    assert!(abomination.contains("min(i16::MAX.into()) as i16"), "{abomination}");
}

/// "When this artifact enters and when you sacrifice it": two zone changes, one `let .. else` binding what both read. An
/// `if let Event::ZoneChange` of the first would fail only on an event the matcher is never asked about, which leaves its
/// closing brace a line no game runs; what the second asks of its reason is a guard of its own, after the first's test.
#[test]
fn two_zone_changes_bind_the_event_once() {
    let raw = trigger_card(
        "fn when_it_happens(q: &dyn Query, asker: &Asker, event: &Event) -> Option<Fired> {
            if let Event::ZoneChange { new, to, .. } = event {
                let id = *new;
                if to.is(ZoneKind::Battlefield) && asker.source == id { return Fired::about(Target::Object(id)); }
            }
            if let Event::ZoneChange { old, from, reason: ZoneReason::Sacrifice, .. } = event {
                let id = *old;
                if from.is(ZoneKind::Battlefield) && asker.source == id { return Fired::about(Target::Object(id)); }
            }
            None
        }",
    );
    let file = render(&raw, "Sample", "Whenever a spell is cast, draw a card.", Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(file.contains("let Event::ZoneChange { new, to, old, from, reason, .. } = event else { return None };"), "{file}");
    assert!(file.contains("let ZoneReason::Sacrifice = reason else { return None };"), "{file}");
    assert!(!file.contains("if let Event::"), "{file}");
    assert_eq!(file.matches("Fired::about").count(), 2, "{file}");
}

/// Where the first event's code names a value as the second binds a field ("deals combat damage to a player or
/// planeswalker": `let (source, combat) = (*source, ..)`), the first stays in a block of its own, so the second reads
/// the field.
#[test]
fn two_events_of_one_kind_keep_their_own_names() {
    let raw = trigger_card(
        "fn when_it_happens(q: &dyn Query, asker: &Asker, event: &Event) -> Option<Fired> {
            if let Event::Damage { source, target, kind, .. } = event {
                let (source, combat) = (*source, kind.is_combat());
                if combat && asker.source == source && matches!(*target, Target::Player(_)) { return Fired::about(Target::Object(source)); }
            }
            if let Event::Damage { source, target, kind, .. } = event {
                let (source, combat) = (*source, kind.is_combat());
                if combat && asker.source == source && matches!(*target, Target::Object(_)) { return Fired::about(Target::Object(source)); }
            }
            None
        }",
    );
    let file = render(&raw, "Sample", "Whenever a spell is cast, draw a card.", Characteristics::Oracle).unwrap();
    println!("{file}");
    let first = file.find("let (source, combat)").unwrap();
    assert!(file[..first].trim_end().ends_with('{'), "{file}");
    assert_eq!(file.matches("Fired::about").count(), 2, "{file}");
}

fn replacement_card(applies: &str, apply: &str) -> String {
    format!(
        "{applies}
        {apply}
        pub static CARD: CardDef = CardDef {{
            replacements: &[ReplacementDef {{ kind: ReplacementKind::ZoneChange, applies, apply, text: \"If a card would be put into a graveyard, exile it instead.\" }}],
            ..BLANK
        }};"
    )
}

/// A replacement registered for one kind of proposal is shown only that kind: its test of the kind is a `let .. else` on
/// one line, whatever the function's end is (`false`, nothing), so no line stands for the kind that never comes.
#[test]
fn a_proposals_kind_is_a_guard_on_one_line() {
    let raw = replacement_card(
        "fn applies(q: &dyn Query, asker: &Asker, proposal: &Proposal) -> bool {
            if let Proposal::ZoneChange(z) = proposal { if z.to.is(ZoneKind::Graveyard) { return true; } }
            false
        }",
        "fn apply(q: &dyn Query, asker: &Asker, proposal: &mut Proposal) {
            if let Proposal::ZoneChange(z) = proposal { z.to = Zone::exile(q.state().obj(z.object).owner); }
        }",
    );
    let file = render(&raw, "Sample", "If a card would be put into a graveyard, exile it instead.", Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(!file.contains("if let Proposal::"), "{file}");
    assert!(file.contains("let Proposal::ZoneChange(z) = proposal else { return false };"), "{file}");
    assert!(file.contains("let Proposal::ZoneChange(z) = proposal else { return };"), "{file}");
}

/// `Some(x)` matched against `Some(item.object)` always matches: the part goes, and the tuple with it, so the code under it
/// is not a block whose end stands for a `None` no game has.
#[test]
fn a_value_that_is_always_some_is_not_tested() {
    let body = "let values_1 = g.state().obj(item.object).spent.first().copied();
        if let (Some(source), Some(values)) = (Some(item.object), values_1) { g.gain_life(item.controller, values); g.tap(source); }";
    let file = render(&spell_card(body), "Sample", "You gain 1 life.", Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(!file.contains("Some(source)"), "{file}");
    assert!(!file.contains("Some(item.object)"), "{file}");
    assert!(file.contains("g.tap(item.object)"), "{file}");
    assert!(file.contains("let Some(values) = values_1 else { return Ok(()) };"), "{file}");
}

/// A `let .. else` whose `else { return .. }` the formatter puts on a line of its own is joined to it where the two fit: the
/// line would count as run only when the guard fails. A loop's `break` is written the same way.
#[test]
fn a_guard_broken_by_the_margin_is_one_line() {
    let body = "let Some(the_permanent_that_was_chosen_first) = g.state().battlefield().first().copied().filter(|&o| o != item.object) else { return Ok(()); };
        g.tap(the_permanent_that_was_chosen_first);
        loop { let Some(next_card_in_the_library) = g.state().zone(Zone::library(item.controller)).first().copied() else { break; }; g.tap(next_card_in_the_library); }";
    let file = render(&spell_card(body), "Sample", "Tap a permanent.", Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(!file.lines().any(|line| line.trim().starts_with("else {")), "{file}");
    assert!(!file.lines().any(|line| line.trim() == "break;"), "{file}");
    assert!(file.contains("else { break };"), "{file}");
}

const LONE_TARGET_MATCHED: &str = r#"
#[allow(unused_imports)] use cantrip_api::*;
fn ability_0_target_0(q: &dyn Query, asker: &Asker, t: Target) -> bool { matches!(t, Target::Object(id) if q.chars(id).is(CardType::Creature)) }
fn ability_0(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> { let asker = &Asker::new(item.object, item.controller);
let t_0_raw = announced(g, item, 0, false); let t_0 = announced(g, item, 0, true);
let original_0 = { let q: &dyn Query = g; t_0_raw.get(0).copied().filter(|t| t_0.contains(t)).and_then(|t: Target| t.object()) }; let count_0 = { let q: &dyn Query = g; (1i64).max(0) as u32 }; let player_0 = { let q: &dyn Query = g; Some(asker.controller).expect("a reference that always names a player") }; let created_0 = match original_0 { Some(original) => g.create_token_copies(original, player_0, count_0)?, None => Vec::new() };
 Ok(()) }
fn announced(g: &dyn Game, item: &StackItem, spec: u8, legal: bool) -> Vec<Target> { (0..g.state().targets(item).len()).filter(|&i| g.state().spec(item, i) == spec && (!legal || g.target_legal(item, i))).map(|i| g.state().target(item, i)).collect() }
pub static CARD: CardDef = CardDef { name: "Sample", oracle: true, spell: spell(&[target(ability_0_target_0)], ability_0), abilities: &[], ..CardDef::BLANK };
pub static EXTRA: &[&CardDef] = &[];
"#;

#[test]
fn a_lone_target_matched_is_a_guard() {
    // Replicate: a spell whose only target is illegal doesn't resolve (CR 608.2b), so the `None` arm is never run.
    let printed = "Create a token that's a copy of target creature you control.";
    let file = render(LONE_TARGET_MATCHED, "Sample", printed, Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(file.contains("let Some(original) = targets::object(g, item, 0) else { return Ok(()) };"), "{file}");
    assert!(file.contains("g.create_token_copies(original, item.controller, 1)?;"), "{file}");
    assert!(!file.contains("None =>"), "{file}");
}

const SUBJECT_AN_OBJECT: &str = r#"
#[allow(unused_imports)] use cantrip_api::*;
fn ability_0_event(q: &dyn Query, asker: &Asker, event: &Event) -> Option<Fired> { if let Event::Mutated { permanent, .. } = event { let id = *permanent; let subject = Target::Object(id); if asker.source == id { return Fired::about(subject); } } None }
fn ability_0(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> { let source_0 = item.subject.object(); if let Some(source) = source_0 { g.damage_together(&[(source, Target::Player(item.controller), 4u16)]); } g.end_sentence(); g.gain_life(item.controller, 4); Ok(()) }
pub static CARD: CardDef = CardDef { name: "Sample", oracle: true, abilities: &[triggered(Trigger::when_asked(EventMask::MUTATED, ability_0_event), ability_0, "Whenever this creature mutates, it deals 4 damage to you and you gain 4 life.")], ..CardDef::BLANK };
pub static EXTRA: &[&CardDef] = &[];
"#;

#[test]
fn a_subject_that_is_always_an_object_is_a_guard() {
    // Snapdax: the matcher fires only about an object, so the ability's subject is one.
    let printed = "Whenever this creature mutates, it deals 4 damage to you and you gain 4 life.";
    let file = render(SUBJECT_AN_OBJECT, "Sample", printed, Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(!file.contains("if let Some(source)"), "{file}");
    assert!(
        file.contains("let Some(source) = item.subject.object() else { return Ok(()) };")
            || file.contains("let Some(source) = source_0 else { return Ok(()) };"),
        "{file}"
    );
    assert!(file.contains("g.gain_life(item.controller, 4);"), "{file}");
    // A matcher that can fire about a player leaves the test as it is.
    let player = SUBJECT_AN_OBJECT.replace("let subject = Target::Object(id);", "let subject = Target::Player(asker.controller);");
    let file = render(&player, "Sample", printed, Characteristics::Oracle).unwrap();
    assert!(file.contains("if let Some(source)"), "{file}");
}
#[test]
fn ignored_turbofish_collections_keep_the_action_without_a_must_use_warning() {
    use mtgish_ast::cantrip::pr_form::{render, Characteristics};
    let raw = r#"use cantrip_api::*;
fn action(g: &mut dyn Game, _: &StackItem) -> Result<(), Suspend> {
    let moved = g.put_into_graveyard(&[]).into_iter().filter(|&o| g.obj(o).zone.is(ZoneKind::Graveyard)).collect::<Vec<ObjectId>>();
    Ok(())
}
pub static CARD: CardDef = CardDef { spell: spell(&[], action), ..CardDef::BLANK };
"#;
    let output = render(raw, "Fixture", "Put the cards into their owners' graveyards.", Characteristics::Oracle).unwrap();
    assert!(output.contains("let _ = g"), "{output}");
    assert!(output.contains("put_into_graveyard") && output.contains("collect::<Vec<ObjectId>>()"), "{output}");
}
