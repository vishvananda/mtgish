use cantrip_api::*;
use cantrip_rules::{Game as RulesGame, Setup};
const A: PlayerId = PlayerId(0);
const B: PlayerId = PlayerId(1);
static LAND: CardDef = CardDef { name: "Backend filler", types: TypeSet::of(&[CardType::Land]), ..CardDef::BLANK };
static HORRID_CALL: CardDef = CardDef { name: "Horrid callback", types: TypeSet::of(&[CardType::Instant]), has_cost: true, spell: spell(&[], horrid::callback), ..CardDef::BLANK };
static ZERO_CALL: CardDef = CardDef { name: "Zero draw callback", types: TypeSet::of(&[CardType::Instant]), has_cost: true, spell: spell(&[], zero::callback), ..CardDef::BLANK };
static CREW_CALL: CardDef = CardDef { name: "Crew callback", types: TypeSet::of(&[CardType::Instant]), has_cost: true, spell: spell(&[], locomotive::callback), ..CardDef::BLANK };
fn no_draw(_: &dyn Query, _: &Asker, _: PlayerId, n: u32) -> bool { n > 0 }
static DRAW_BAN: CardDef = CardDef { name: "Draw prohibition", types: TypeSet::of(&[CardType::Enchantment]), cant_draw: &[no_draw], ..CardDef::BLANK };
fn replace_draw(_: &dyn Query, _: &Asker, p: &Proposal) -> bool { matches!(p, Proposal::Draw(_)) }
fn skip_draw(_: &dyn Query, _: &Asker, p: &mut Proposal) { if let Proposal::Draw(d) = p { d.replaced = true; } }
static DRAW_REPLACEMENT: CardDef = CardDef {
    name: "Draw replacement", types: TypeSet::of(&[CardType::Enchantment]),
    replacements: &[replacement(ReplacementKind::Draw, replace_draw, skip_draw, "Skip draw")], ..CardDef::BLANK
};
fn grow_after_replacing_draw(g: &mut dyn Game, _: &Asker, _: &Proposal, _: &Proposal) {
    let source = g.state().battlefield().iter().copied().find(|&id| g.chars(id).card == g.card_id(&horrid::CARD)).unwrap();
    g.add_counters(source, CounterKind::PlusOne, 1);
}
static GROWING_REPLACEMENT: CardDef = CardDef {
    name: "Replace draw and change power", types: TypeSet::of(&[CardType::Enchantment]),
    replacements: &[ReplacementDef { then: Some(grow_after_replacing_draw), ..replacement(ReplacementKind::Draw, replace_draw, skip_draw, "Replace draw and grow") }],
    ..CardDef::BLANK
};
fn setup(callback: &'static CardDef, source: &'static CardDef) -> (RulesGame, ObjectId, ObjectId) {
    let mut defs = vec![&LAND, &HORRID_CALL, &ZERO_CALL, &CREW_CALL, &DRAW_BAN, &DRAW_REPLACEMENT, &GROWING_REPLACEMENT, &horrid::CARD, &locomotive::CARD];
    defs.extend(locomotive::EXTRA);
    let table = Box::leak(Box::new(CardTable::new(defs)));
    let fill = table.id_of(&LAND);
    let mut mine = vec![fill; 30]; mine[0] = table.id_of(callback);
    let mut g = RulesGame::new(table, &Setup { decks: vec![mine, vec![fill; 30]], ..Default::default() });
    g.state.update_turn(|t| { t.number = 1; t.active = A; t.priority = A; t.set_step(Step::PrecombatMain); t.began = true; });
    g.state.update_player(A, |p| p.turn_began = 1);
    let spell = g.state.zone(Zone::hand(A))[0];
    let source = put(&mut g, source, A, Zone::BATTLEFIELD);
    (g, spell, source)
}
fn put(g: &mut RulesGame, def: &'static CardDef, owner: PlayerId, zone: Zone) -> ObjectId {
    let id = g.state.push_obj(Obj::new(g.cards.id_of(def), owner, zone));
    g.state.zone_push(zone, id); id
}
fn cast(g: &mut RulesGame, source: ObjectId) {
    g.state.see_statics();
    g.state.frame_push(cantrip_state::Frame::new(cantrip_state::FrameKind::Priority));
    g.run(); let r = g.pending().cloned().expect("priority");
    let RequestKind::Priority { options } = &r.kind else { panic!("{:?}", r.kind) };
    let o = options.iter().find(|o| o.source == Some(source) && o.kind == ActionKind::CastSpell).expect("cast action");
    g.answer(r.id, r.player, Answer::Act(Action::of(o))).unwrap();
}
fn finish(g: &mut RulesGame, yes: bool) -> usize {
    let mut may = 0;
    for _ in 0..150 {
        g.run(); let Some(r) = g.pending().cloned() else { assert!(g.state.is_over()); return may };
        let answer = match &r.kind {
            RequestKind::Priority { .. } if g.state.stack.is_empty() => return may,
            RequestKind::Priority { .. } => Answer::Pass,
            RequestKind::May { .. } => { may += 1; if yes { Answer::Yes } else { Answer::No } },
            RequestKind::ChooseCards { cards, rule, .. } => {
                let n = match rule.count {
                    cantrip_mechanics::select::Count::Exactly(n) | cantrip_mechanics::select::Count::AsManyAsCan(n) => n.min(cards.len() as u32),
                    other => panic!("{other:?}"),
                };
                Answer::Cards(cards.iter().copied().take(n as usize).collect())
            }
            RequestKind::OrderTriggers { sources, .. } => Answer::Order((0..sources.len() as u32).collect()),
            other => panic!("unhandled {other:?}"),
        };
        g.answer(r.id, r.player, answer).unwrap();
    }
    panic!("resolution did not complete");
}
fn discarded(g: &RulesGame) -> usize { g.log.iter().filter(|r| matches!(r.event, Event::Discarded { player: A, .. })).count() }
#[test]
fn requested_draw_cost_survives_replacement_and_declining_does_nothing() {
    for replace in [false, true] { for yes in [false, true] {
        let (mut g, spell, _) = setup(&HORRID_CALL, &horrid::CARD);
        if replace { put(&mut g, &DRAW_REPLACEMENT, A, Zone::BATTLEFIELD); }
        cast(&mut g, spell); let prompts = finish(&mut g, yes);
        assert_eq!(prompts, 1);
        assert_eq!(discarded(&g), if yes { 2 } else { 0 }, "replaced={replace} yes={yes}");
        assert_eq!(g.state.zone(Zone::hand(A)).len(), 6 + if yes && !replace { 2 } else { 0 } - if yes { 2 } else { 0 });
    }}
}
#[test]
fn forbidden_draw_is_never_offered_or_followed_by_discard() {
    let (mut g, spell, _) = setup(&HORRID_CALL, &horrid::CARD);
    put(&mut g, &DRAW_BAN, A, Zone::BATTLEFIELD);
    cast(&mut g, spell); assert_eq!(finish(&mut g, true), 0);
    assert_eq!(discarded(&g), 0); assert_eq!(g.state.zone(Zone::hand(A)).len(), 6);
}
fn record(g: &mut RulesGame, event: Event, turn: u32) {
    g.log.push(Record { serial: g.log.len() as u32, turn, step: Step::PrecombatMain, active: A, cause: Cause::Rules, event });
}
#[test]
fn crew_counts_each_incarnation_once_and_keeps_departed_creatures() {
    let (mut g, spell, vehicle) = setup(&CREW_CALL, &locomotive::CARD);
    let a = put(&mut g, &horrid::CARD, A, Zone::BATTLEFIELD);
    let b = put(&mut g, &horrid::CARD, A, Zone::graveyard(A));
    let c = put(&mut g, &horrid::CARD, B, Zone::BATTLEFIELD);
    let d = put(&mut g, &horrid::CARD, A, Zone::BATTLEFIELD);
    let other_vehicle = put(&mut g, &locomotive::CARD, B, Zone::BATTLEFIELD);
    // Forced historical position: repeated crew, departed crew member, another
    // vehicle, a previous turn and a member now controlled by the opponent.
    for (creature, v, turn) in [(d, vehicle, 0), (a, vehicle, 1), (a, vehicle, 1), (b, vehicle, 1), (c, vehicle, 1), (d, other_vehicle, 1)] {
        record(&mut g, Event::Crewed { creature, vehicle: v }, turn);
    }
    cast(&mut g, spell); finish(&mut g, true);
    let treasures = g.state.battlefield().iter().filter(|&&o| g.chars(o).subtypes.has(Subtype::Treasure)).count();
    assert_eq!(treasures, 3);
    let AbilityDef::Activated(crew) = locomotive::CARD.abilities[1] else { panic!() };
    assert!(crew.once_each_turn, "printed crew limit must survive shared-recipe generation");
}

#[test]
fn zero_draw_cost_is_a_choice_and_discards_zero() {
    for yes in [false, true] {
        let (mut g, spell, _) = setup(&ZERO_CALL, &horrid::CARD);
        cast(&mut g, spell); assert_eq!(finish(&mut g, yes), 1);
        assert_eq!(discarded(&g), 0); assert_eq!(g.state.zone(Zone::hand(A)).len(), 6);
    }
}
#[test]
fn insufficient_library_does_not_reduce_the_requested_discard() {
    let (mut g, spell, _) = setup(&HORRID_CALL, &horrid::CARD);
    let library = g.state.zone(Zone::library(A)).to_vec();
    for &id in library.iter().skip(1) {
        g.state.zone_remove(Zone::library(A), id);
        g.state.update_obj(id, |o| o.zone = Zone::exile(A));
        g.state.zone_push(Zone::exile(A), id);
    }
    cast(&mut g, spell); finish(&mut g, true);
    assert_eq!(discarded(&g), 2);
    assert_eq!(g.log.iter().filter(|r| matches!(r.event, Event::Drew { player: A, .. })).count(), 1);
}

#[test]
fn changing_power_during_drawing_does_not_recompute_the_requested_count() {
    let (mut g, spell, source) = setup(&HORRID_CALL, &horrid::CARD);
    put(&mut g, &GROWING_REPLACEMENT, A, Zone::BATTLEFIELD);
    cast(&mut g, spell); finish(&mut g, true);
    assert_eq!(g.chars(source).power, Some(4));
    assert_eq!(discarded(&g), 2, "discard the original request, not the new power");
}
