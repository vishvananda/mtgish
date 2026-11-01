//! `pr_form` keeps what a generated card does and says it as a card file does.
#![cfg(feature = "cantrip")]
use mtgish_ast::cantrip::pr_form::{render, Characteristics};

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
    assert!(file.contains("t_0.contains(t)"), "{file}");
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
pub static BUG: CardDef = CardDef {{ {fields} ..token!(Insect) }};
fn ability_0(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {{ let def = g.card_id(&BUG); g.create_tokens_worded(&[(def, item.controller)], false, None, g.words(item.object))?; Ok(()) }}
pub static CARD: CardDef = CardDef {{ name: "Sample", oracle: true, spell: spell(&[], ability_0), abilities: &[], ..CardDef::DEFAULT }};
pub static EXTRA: &[&CardDef] = &[&BUG];
"#
    )
}

const PLAIN_INSECT: &str = "types: TypeSet::of(&[CardType::Creature]), colors: Some(ColorSet(16)), power: Some(1), toughness: Some(1),";

#[test]
fn a_token_cantrip_shares_is_named() {
    let text = "Create a 1/1 green Insect creature token.";
    let file = render(&token_card(PLAIN_INSECT), "Sample", text, Characteristics::Oracle).unwrap();
    println!("{file}");
    assert!(file.contains("use cantrip_mechanics::tokens;"), "{file}");
    assert!(file.contains("g.card_id(&tokens::INSECT)"), "{file}");
    assert!(file.contains("&[&tokens::INSECT]"), "{file}");
    assert!(!file.contains("pub static BUG"), "{file}");
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
