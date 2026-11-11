use cantrip_api::*;
use cantrip_rules::{Game as RulesGame, Setup};
const A: PlayerId = PlayerId(0);
const B: PlayerId = PlayerId(1);
static FILLER: CardDef = CardDef { name: "Filler", types: TypeSet::of(&[CardType::Land]), ..CardDef::BLANK };
static BEAR: CardDef = CardDef { name: "White fixture bear", cost: mana("{2}"), has_cost: true,
    types: TypeSet::of(&[CardType::Creature]), colors: Some(ColorSet::of(Color::White)), power: Some(2), toughness: Some(2), ..CardDef::BLANK };
static BASIC: CardDef = CardDef { name: "Fixture basic Plains", types: TypeSet::of(&[CardType::Land]),
    subtypes: SubtypeSet::of(&[Subtype::Plains]), supertypes: Supertypes { basic: true, legendary: false, snow: false }, ..CardDef::BLANK };
static DUAL: CardDef = CardDef { name: "Fixture nonbasic Plains Forest", subtypes: SubtypeSet::of(&[Subtype::Plains, Subtype::Forest]),
    supertypes: Supertypes { basic: false, legendary: false, snow: false }, ..BASIC };
fn permanent(q: &dyn Query, _: &Asker, t: Target) -> bool { t.object().is_some_and(|o| q.is_on_battlefield(o)) }
static PERMANENT: &[TargetSpec] = &[target(permanent)];
fn bounce(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
    if let Some(o) = cantrip_mechanics::targets::object(g, item, 0) { g.return_to_hand(&[o]); } Ok(())
}
fn untap(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
    if let Some(o) = cantrip_mechanics::targets::object(g, item, 0) { g.untap(o); } Ok(())
}
fn control(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
    if let Some(o) = cantrip_mechanics::targets::object(g, item, 0) { g.add_effect(item.object, o, EffectKind::Control(B), Duration::EndOfTurn); } Ok(())
}
fn supply(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> { g.add_mana(item.controller, Mana::C, 20); Ok(()) }
static BOUNCE: CardDef = CardDef { name: "Fixture bounce", types: TypeSet::of(&[CardType::Instant]), has_cost: true, spell: spell(PERMANENT, bounce), ..CardDef::BLANK };
static UNTAP: CardDef = CardDef { name: "Fixture untap", spell: spell(PERMANENT, untap), ..BOUNCE };
static CONTROL: CardDef = CardDef { name: "Fixture control", spell: spell(PERMANENT, control), ..BOUNCE };
static SUPPLY: CardDef = CardDef { name: "Fixture mana supply", spell: spell(&[], supply), ..BOUNCE };
fn setup(extra: &[&'static CardDef]) -> RulesGame {
    let mut defs = vec![&FILLER, &BEAR, &BASIC, &DUAL, &BOUNCE, &UNTAP, &CONTROL, &SUPPLY]; defs.extend(extra);
    let table = Box::leak(Box::new(CardTable::new(defs))); let fill = table.id_of(&FILLER);
    let mut g = RulesGame::new(table, &Setup { decks: vec![vec![fill; 30], vec![fill; 30]], shuffle: false, ..Default::default() });
    g.state.update_turn(|t| { t.number = 1; t.active = A; t.priority = A; t.set_step(Step::PrecombatMain); t.began = true; });
    g.state.update_player(A, |p| p.turn_began = 1);
    for player in [A,B] { for mana in Mana::ALL { g.state.add_mana(player, mana, 100, ObjectId::NONE, None, Default::default(), false); } }
    g
}
fn put(g: &mut RulesGame, def: &'static CardDef, player: PlayerId, zone: Zone) -> ObjectId {
    let id = g.state.push_obj(Obj::new(g.cards.id_of(def), player, zone)); g.state.zone_push(zone, id); id
}
fn current(g: &RulesGame, mut id: ObjectId) -> ObjectId { while let Some(next) = g.state.obj(id).became { id = next; } id }
fn announce(g: &mut RulesGame, id: ObjectId, kind: ActionKind, index: u8, targets: Vec<Target>) {
    g.state.see_statics();
    if g.pending().is_none() { g.state.frame_push(cantrip_state::Frame::new(cantrip_state::FrameKind::Priority)); }
    g.run(); let r = g.pending().cloned().expect("priority");
    let RequestKind::Priority { options } = &r.kind else { panic!("{:?}", r.kind) };
    let option = options.iter().find(|o| o.source == Some(id) && o.kind == kind && o.index == index)
        .unwrap_or_else(|| panic!("missing {kind:?}/{index} of {id:?}: {options:?}"));
    let mut action = Action::of(option); action.targets = targets;
    g.answer(r.id, r.player, Answer::Act(action)).unwrap();
}
fn default_answer(g: &RulesGame, r: &Request) -> Answer {
    match &r.kind {
        RequestKind::Priority { .. } => Answer::Pass,
        RequestKind::DeclareAttackers { .. } => Answer::Attack(vec![]),
        RequestKind::DeclareBlockers { .. } => Answer::Block(vec![]),
        RequestKind::PayCosts { announcing: true, .. } => Answer::Announce(0),
        RequestKind::PayCosts { options, .. } | RequestKind::UnlessPays { options, .. } => {
            let option = [ActionKind::AssignMana, ActionKind::PayCost, ActionKind::ConfirmCasting, ActionKind::DeclineCost]
                .into_iter().find_map(|kind| options.iter().find(|o| o.kind == kind))
                .unwrap_or_else(|| panic!("payment: {:?}", r.kind));
            Answer::Act(Action::of(option))
        }
        RequestKind::ChooseCostPayment { .. } => Answer::CostPayment(0),
        RequestKind::ChooseTarget { candidates, least, .. } => Answer::Targets(candidates.iter().copied().take(*least as usize).collect()),
        RequestKind::ChooseCards { cards, rule, .. } => {
            let n = match rule.count { Count::Exactly(n) | Count::AsManyAsCan(n) => n.min(cards.len() as u32), Count::AnyNumber | Count::UpTo(_) => 0, other => panic!("{other:?}") };
            Answer::Cards(cards.iter().copied().take(n as usize).collect())
        }
        RequestKind::ChooseModes { eligible, count, .. } => Answer::Modes(eligible.iter().copied().take(*count as usize).collect()),
        RequestKind::ChooseNumber { least, .. } => Answer::Announce(*least),
        RequestKind::ChooseSubtype { choices, .. } => Answer::Subtype(choices[0]),
        RequestKind::ChoosePlayer { candidates, .. } => Answer::Player(candidates[0]),
        RequestKind::May { .. } => Answer::Yes,
        RequestKind::OrderTriggers { sources, .. } => Answer::Order((0..sources.len() as u32).collect()),
        RequestKind::DiscardToHandSize { count, hand } => Answer::Cards(hand[..*count as usize].to_vec()),
        other => panic!("unhandled request: {other:?}, turn {:?}", g.state.turn),
    }
}
fn finish_with(g: &mut RulesGame, choose: &mut impl FnMut(&RulesGame, &Request) -> Option<Answer>) {
    for _ in 0..500 {
        g.run(); let Some(r) = g.pending().cloned() else { assert!(g.state.is_over()); return };
        if matches!(r.kind, RequestKind::Priority { .. }) && g.state.stack.is_empty() { return; }
        let answer = choose(g, &r).unwrap_or_else(|| default_answer(g, &r)); g.answer(r.id, r.player, answer).unwrap();
    } panic!("resolution did not finish");
}
fn finish(g: &mut RulesGame) { finish_with(g, &mut |_,_| None); }
fn advance_to(g: &mut RulesGame, test: impl Fn(&RulesGame) -> bool) {
    for _ in 0..1000 {
        g.run(); if test(g) { return; } let r = g.pending().cloned().expect("advance request");
        let answer = default_answer(g, &r); g.answer(r.id, r.player, answer).unwrap();
    } panic!("advance did not reach its condition");
}

#[test]
fn changed_land_type_replaces_printed_types_but_preserves_supertypes() {
    let mut g = setup(&[&orcish_farmer::CARD]);
    let farmer = put(&mut g, &orcish_farmer::CARD, A, Zone::BATTLEFIELD);
    let land = put(&mut g, &BASIC, B, Zone::BATTLEFIELD);
    announce(&mut g, farmer, ActionKind::ActivateAbility, 0, vec![Target::Object(land)]); finish(&mut g);
    assert!(g.chars(land).has_subtype(Subtype::Swamp)); assert!(!g.chars(land).has_subtype(Subtype::Plains));
    assert!(g.chars(land).supertypes.basic);
}
#[test]
fn event_delayed_list_survives_until_its_watched_source_leaves() {
    let mut g = setup(&[&war_barge::CARD]); let barge = put(&mut g, &war_barge::CARD, A, Zone::BATTLEFIELD);
    let bear = put(&mut g, &BEAR, A, Zone::BATTLEFIELD); let bounce = put(&mut g, &BOUNCE, A, Zone::hand(A));
    announce(&mut g, barge, ActionKind::ActivateAbility, 0, vec![Target::Object(bear)]); finish(&mut g);
    assert!(g.chars(bear).keywords.has(Keyword::Islandwalk));
    announce(&mut g, bounce, ActionKind::CastSpell, 0, vec![Target::Object(barge)]); finish(&mut g);
    assert!(g.state.obj(current(&g,bear)).zone.is(ZoneKind::Graveyard));
}
#[test]
fn meriekes_untap_trigger_destroys_the_captured_creature() {
    let mut g = setup(&[&merieke_ri_berit::CARD]); let merieke = put(&mut g, &merieke_ri_berit::CARD, A, Zone::BATTLEFIELD);
    let bear = put(&mut g, &BEAR, B, Zone::BATTLEFIELD); let untap = put(&mut g, &UNTAP, A, Zone::hand(A));
    announce(&mut g, merieke, ActionKind::ActivateAbility, 0, vec![Target::Object(bear)]); finish(&mut g);
    assert_eq!(g.state.obj(bear).controller, A);
    announce(&mut g, untap, ActionKind::CastSpell, 0, vec![Target::Object(merieke)]); finish(&mut g);
    assert!(g.state.obj(current(&g,bear)).zone.is(ZoneKind::Graveyard));
}
#[test]
fn control_loss_watches_the_exact_taken_creature() {
    let mut g = setup(&[&ray_of_command::CARD]); let ray = put(&mut g, &ray_of_command::CARD, A, Zone::hand(A));
    let control = put(&mut g, &CONTROL, A, Zone::hand(A)); let bear = put(&mut g, &BEAR, B, Zone::BATTLEFIELD);
    announce(&mut g, ray, ActionKind::CastSpell, 0, vec![Target::Object(bear)]); finish(&mut g);
    assert_eq!(g.state.obj(bear).controller, A); assert!(!g.state.obj(bear).tapped);
    announce(&mut g, control, ActionKind::CastSpell, 0, vec![Target::Object(bear)]); finish(&mut g);
    assert_eq!(g.state.obj(bear).controller, B); assert!(g.state.obj(bear).tapped);
}
#[test]
fn dreams_of_the_dead_uses_the_leave_battlefield_replacement() {
    let mut g = setup(&[&dreams_of_the_dead::CARD]); let dreams = put(&mut g, &dreams_of_the_dead::CARD, A, Zone::BATTLEFIELD);
    let bear = put(&mut g, &BEAR, A, Zone::graveyard(A)); let bounce = put(&mut g, &BOUNCE, A, Zone::hand(A));
    announce(&mut g, dreams, ActionKind::ActivateAbility, 0, vec![Target::Object(bear)]); finish(&mut g);
    let bear = current(&g,bear); assert!(g.state.obj(bear).zone.is(ZoneKind::Battlefield));
    announce(&mut g, bounce, ActionKind::CastSpell, 0, vec![Target::Object(bear)]); finish(&mut g);
    assert!(g.state.obj(current(&g,bear)).zone.is(ZoneKind::Exile));
}
#[test]
fn mirror_universe_exchanges_life_after_paying_its_sacrifice() {
    let mut g = setup(&[&mirror_universe::CARD]); let mirror = put(&mut g, &mirror_universe::CARD, A, Zone::BATTLEFIELD);
    g.state.update_turn(|t| t.set_step(Step::Upkeep)); g.state.update_player(B, |p| p.life = 7);
    announce(&mut g, mirror, ActionKind::ActivateAbility, 0, vec![Target::Player(B)]); finish(&mut g);
    assert_eq!(g.state.player(A).life,7); assert_eq!(g.state.player(B).life,20);
    assert!(g.state.obj(current(&g,mirror)).zone.is(ZoneKind::Graveyard));
}
#[test]
fn necropolis_reads_the_card_exiled_for_this_activation() {
    let mut g = setup(&[&necropolis::CARD]); let wall = put(&mut g, &necropolis::CARD, A, Zone::BATTLEFIELD);
    let bear = put(&mut g, &BEAR, A, Zone::graveyard(A));
    announce(&mut g, wall, ActionKind::ActivateAbility, 0, vec![]); finish(&mut g);
    assert_eq!(g.state.obj(wall).counters(CounterKind::Plus0Plus1), 2);
    assert!(g.state.obj(current(&g,bear)).zone.is(ZoneKind::Exile));
}
#[test]
fn illusionary_terrain_keeps_two_distinct_types_and_only_changes_basic_lands() {
    let mut g = setup(&[&illusionary_terrain::CARD]); let spell = put(&mut g, &illusionary_terrain::CARD, A, Zone::hand(A));
    let basic = put(&mut g, &BASIC, A, Zone::BATTLEFIELD); let dual = put(&mut g, &DUAL, A, Zone::BATTLEFIELD);
    let mut choices = vec![Subtype::Plains,Subtype::Island].into_iter();
    announce(&mut g, spell, ActionKind::CastSpell, 0, vec![]);
    finish_with(&mut g, &mut |_,r| match &r.kind { RequestKind::ChooseSubtype { choices: offered, .. } => {
        let selected = choices.next().unwrap(); assert!(offered.contains(&selected)); Some(Answer::Subtype(selected)) }, _ => None });
    assert!(g.chars(basic).has_subtype(Subtype::Island)); assert!(!g.chars(basic).has_subtype(Subtype::Plains));
    assert!(g.chars(basic).supertypes.basic); assert!(g.chars(dual).has_subtype(Subtype::Plains));
    assert!(g.chars(dual).has_subtype(Subtype::Forest));
}
#[test]
fn nameless_race_stores_the_amount_paid_at_entry() {
    let mut g = setup(&[&nameless_race::CARD]); let spell = put(&mut g, &nameless_race::CARD, A, Zone::hand(A));
    put(&mut g, &BEAR, B, Zone::BATTLEFIELD); put(&mut g, &BEAR, B, Zone::graveyard(B));
    announce(&mut g, spell, ActionKind::CastSpell, 0, vec![]);
    finish_with(&mut g, &mut |_,r| match r.kind { RequestKind::ChooseNumber { most, .. } => { assert_eq!(most,2); Some(Answer::Announce(2)) }, _ => None });
    let creature = current(&g,spell); assert!(g.state.obj(creature).zone.is(ZoneKind::Battlefield));
    assert_eq!(g.chars(creature).power,Some(2)); assert_eq!(g.chars(creature).toughness,Some(2)); assert_eq!(g.state.player(A).life,18);
}
#[test]
fn forgotten_lore_excludes_previous_choices_and_returns_the_last_one() {
    let mut g = setup(&[&forgotten_lore::CARD]); let spell = put(&mut g, &forgotten_lore::CARD, A, Zone::hand(A));
    let first = put(&mut g, &BEAR, A, Zone::graveyard(A)); let last = put(&mut g, &BASIC, A, Zone::graveyard(A));
    let mut picks = 0;
    announce(&mut g, spell, ActionKind::CastSpell, 0, vec![Target::Player(B)]);
    finish_with(&mut g, &mut |_,r| match &r.kind {
        RequestKind::ChooseCards { cards, .. } if r.player == B => { picks += 1; let selected = if picks == 1 { first } else { assert!(!cards.contains(&first)); last }; Some(Answer::Cards(vec![selected])) },
        RequestKind::UnlessPays { options, .. } => { let kind = if picks == 1 { ActionKind::PayCost } else { ActionKind::DeclineCost }; options.iter().find(|o| o.kind==kind).map(|o|Answer::Act(Action::of(o))) },
        _ => None,
    });
    assert_eq!(picks,2); assert!(g.state.obj(current(&g,last)).zone.is(ZoneKind::Hand));
    assert!(g.state.obj(first).zone.is(ZoneKind::Graveyard));
}

fn begin_upkeep(g: &mut RulesGame) {
    g.state.update_turn(|t| t.set_step(Step::Upkeep));
    g.state.frame_push(cantrip_state::Frame::new(cantrip_state::FrameKind::BeginStep));
}
#[test]
fn compound_upkeep_pays_mana_and_life_together() {
    let mut g = setup(&[&infernal_darkness::CARD]);
    let source = put(&mut g, &infernal_darkness::CARD, A, Zone::BATTLEFIELD);
    begin_upkeep(&mut g); finish(&mut g);
    assert_eq!(g.state.obj(source).counters(CounterKind::Age), 1);
    assert_eq!(g.state.player(A).life, 19);
    assert!(g.state.obj(source).zone.is(ZoneKind::Battlefield));
}
#[test]
fn unpayable_compound_upkeep_never_pays_only_the_life_component() {
    let mut g = setup(&[&infernal_darkness::CARD]);
    let source = put(&mut g, &infernal_darkness::CARD, A, Zone::BATTLEFIELD);
    g.state.empty_mana(A, ManaLasts::Step);
    begin_upkeep(&mut g); finish(&mut g);
    assert_eq!(g.state.player(A).life, 20);
    assert!(g.state.obj(current(&g,source)).zone.is(ZoneKind::Graveyard));
}
#[test]
fn tetravus_uses_the_resolution_payment_and_keeps_the_remaining_counters() {
    let mut defs = vec![&tetravus::CARD]; defs.extend(tetravus::EXTRA);
    let mut g = setup(&defs); let spell = put(&mut g, &tetravus::CARD, A, Zone::hand(A));
    announce(&mut g, spell, ActionKind::CastSpell, 0, vec![]); finish(&mut g);
    let creature = current(&g, spell); assert_eq!(g.state.obj(creature).counters(CounterKind::PlusOne), 3);
    advance_to(&mut g, |g| g.state.turn.number > 1 && g.state.turn.active == A && g.state.turn.step() == Step::Upkeep);
    finish_with(&mut g, &mut |_,r| match &r.kind {
        RequestKind::UnlessPays { x: Some(x), options, .. } if *x < 2 => {
            Some(Answer::Act(Action::of(options.iter().find(|o| o.kind == ActionKind::IncreaseX).unwrap())))
        }, _ => None,
    });
    assert_eq!(g.state.obj(creature).counters(CounterKind::PlusOne), 1);
    let tokens: Vec<_> = g.state.battlefield().iter().copied().filter(|&id| g.chars(id).has_subtype(Subtype::Tetravite)).collect();
    assert_eq!(tokens.len(), 2);
    for token in tokens { assert!(g.chars(token).keywords.has(Keyword::Flying)); }
}
#[test]
fn island_sanctuary_skips_draw_and_restricts_new_attackers_until_next_turn() {
    static FLIER: CardDef = CardDef { name: "Fixture flying bear", keywords: Keywords::of(&[Keyword::Flying]), ..BEAR };
    let mut g = setup(&[&island_sanctuary::CARD, &FLIER]);
    put(&mut g, &island_sanctuary::CARD, A, Zone::BATTLEFIELD);
    let ground = put(&mut g, &BEAR, B, Zone::BATTLEFIELD); let flying = put(&mut g, &FLIER, B, Zone::BATTLEFIELD);
    let hand = g.state.zone(Zone::hand(A)).len();
    g.state.update_turn(|t| { t.number = 3; t.set_step(Step::Draw); });
    g.state.frame_push(cantrip_state::Frame::new(cantrip_state::FrameKind::BeginStep)); finish(&mut g);
    assert_eq!(g.state.zone(Zone::hand(A)).len(), hand);
    let new_ground = put(&mut g, &BEAR, B, Zone::BATTLEFIELD);
    advance_to(&mut g, |g| matches!(g.pending().map(|r| &r.kind), Some(RequestKind::DeclareAttackers { .. })));
    let RequestKind::DeclareAttackers { eligible, requirements, .. } = &g.pending().unwrap().kind else { panic!() };
    assert!(!eligible.contains(&ground) || requirements.barred.contains(&(ground,Target::Player(A))));
    assert!(!eligible.contains(&new_ground) || requirements.barred.contains(&(new_ground,Target::Player(A))));
    assert!(eligible.contains(&flying)); assert!(!requirements.barred.contains(&(flying,Target::Player(A))));
    advance_to(&mut g, |g| g.state.turn.number > 3 && g.state.turn.active == A && g.state.turn.step() == Step::Upkeep);
    assert!(g.state.effects.iter().all(|e| !matches!(e.kind, EffectKind::LastingRule { .. })));
}

#[test]
fn frankensteins_monster_chooses_each_counter_independently() {
    let mut g = setup(&[&frankenstein_s_monster::CARD]);
    let spell = put(&mut g, &frankenstein_s_monster::CARD, A, Zone::hand(A));
    put(&mut g, &BEAR, A, Zone::graveyard(A)); put(&mut g, &BEAR, A, Zone::graveyard(A));
    let mut counters = vec![0,2].into_iter();
    announce(&mut g, spell, ActionKind::CastSpell, 0, vec![]);
    finish_with(&mut g, &mut |_,r| match &r.kind {
        RequestKind::PayCosts { announcing: true, .. } => Some(Answer::Announce(2)),
        RequestKind::ChooseModes { modes, .. } if modes.iter().all(|m| matches!(m, Label::Counter(_))) => Some(Answer::Modes(vec![counters.next().unwrap()])),
        _ => None,
    });
    let creature = current(&g,spell); assert!(g.state.obj(creature).zone.is(ZoneKind::Battlefield));
    assert_eq!(g.state.obj(creature).counters(CounterKind::Plus2Plus0),1);
    assert_eq!(g.state.obj(creature).counters(CounterKind::Plus0Plus2),1);
    assert_eq!(g.chars(creature).power,Some(2)); assert_eq!(g.chars(creature).toughness,Some(3));
}
#[test]
fn frankensteins_monster_accepts_a_zero_card_entry_payment() {
    let mut g = setup(&[&frankenstein_s_monster::CARD]);
    let spell = put(&mut g, &frankenstein_s_monster::CARD, A, Zone::hand(A));
    announce(&mut g, spell, ActionKind::CastSpell, 0, vec![]); finish(&mut g);
    let creature = current(&g,spell); assert!(g.state.obj(creature).zone.is(ZoneKind::Battlefield));
    assert_eq!(g.chars(creature).power,Some(0)); assert_eq!(g.chars(creature).toughness,Some(1));
}
#[test]
fn game_of_chaos_doubles_the_stake_between_flips() {
    let mut g = setup(&[&game_of_chaos::CARD]); let spell = put(&mut g, &game_of_chaos::CARD, A, Zone::hand(A));
    let mut flips = 0;
    announce(&mut g, spell, ActionKind::CastSpell, 0, vec![Target::Player(B)]);
    finish_with(&mut g, &mut |g,r| match r.kind {
        RequestKind::May { .. } => { flips += 1;
            let delta = (g.state.player(A).life - 20).abs();
            if flips == 1 { assert_eq!(delta,1); Some(Answer::Yes) }
            else { assert!(delta == 1 || delta == 3); Some(Answer::No) }
        }, _ => None,
    });
    assert_eq!(flips,2); assert_eq!(g.state.player(A).life + g.state.player(B).life,40);
}

#[test]
fn clockwork_choice_is_not_clipped_to_room_before_replacements() {
    fn halve_applies(_: &dyn Query, _: &Asker, p: &Proposal) -> bool {
        matches!(p, Proposal::Counters(c) if !c.entering && c.counts[CounterKind::Plus1Plus0 as usize] > 0)
    }
    fn halve(_: &dyn Query, _: &Asker, p: &mut Proposal) {
        if let Proposal::Counters(c) = p { c.counts[CounterKind::Plus1Plus0 as usize] /= 2; }
    }
    fn trim(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
        if let Some(object) = cantrip_mechanics::targets::object(g,item,0) { g.remove_counters(object,CounterKind::Plus1Plus0,1); } Ok(())
    }
    static HALVE: CardDef = CardDef { name: "Fixture halve counters", types: TypeSet::of(&[CardType::Enchantment]),
        replacements: &[replacement(ReplacementKind::Counters,halve_applies,halve,"Halve counters")], ..CardDef::BLANK };
    static TRIM: CardDef = CardDef { name: "Fixture remove counter", spell: spell(PERMANENT,trim), ..BOUNCE };
    let mut g = setup(&[&clockwork_beast::CARD,&HALVE,&TRIM]);
    let clock = put(&mut g,&clockwork_beast::CARD,A,Zone::hand(A)); let trim = put(&mut g,&TRIM,A,Zone::hand(A));
    let supply = put(&mut g,&SUPPLY,A,Zone::hand(A)); put(&mut g,&HALVE,A,Zone::BATTLEFIELD);
    announce(&mut g,clock,ActionKind::CastSpell,0,vec![]); finish(&mut g); let clock=current(&g,clock);
    announce(&mut g,trim,ActionKind::CastSpell,0,vec![Target::Object(clock)]); finish(&mut g);
    assert_eq!(g.state.obj(clock).counters(CounterKind::Plus1Plus0),6);
    advance_to(&mut g, |g| g.state.turn.number>1 && g.state.turn.active==A && g.state.turn.step()==Step::Upkeep);
    finish(&mut g); announce(&mut g,supply,ActionKind::CastSpell,0,vec![]); finish(&mut g);
    announce(&mut g,clock,ActionKind::ActivateAbility,1,vec![]);
    let mut chosen=false;
    finish_with(&mut g,&mut |_,r| match &r.kind {
        RequestKind::PayCosts { announcing:true,.. } => Some(Answer::Announce(2)),
        RequestKind::ChooseNumber { most,heading,.. } if *heading=="Put counters" => {
            assert_eq!(*most,2); chosen=true; Some(Answer::Announce(2))
        }, _=>None,
    });
    assert!(chosen); assert_eq!(g.state.obj(clock).counters(CounterKind::Plus1Plus0),7);
}
#[test]
fn anti_magic_aura_removes_existing_auras_and_prevents_new_attachments() {
    fn creature(q:&dyn Query,_:&Asker,t:Target)->bool { t.object().is_some_and(|id| q.chars(id).is(CardType::Creature)) }
    static AURA:CardDef=CardDef { name:"Fixture Aura", types:TypeSet::of(&[CardType::Enchantment]),
        subtypes:SubtypeSet::of(&[Subtype::Aura]),has_cost:true,enchant:Some(target(creature)),..CardDef::BLANK };
    let mut g=setup(&[&anti_magic_aura::CARD,&AURA]);let bear=put(&mut g,&BEAR,A,Zone::BATTLEFIELD);
    let first=put(&mut g,&AURA,A,Zone::hand(A));let ward=put(&mut g,&anti_magic_aura::CARD,A,Zone::hand(A));
    let other=put(&mut g,&AURA,A,Zone::hand(A));
    announce(&mut g,first,ActionKind::CastSpell,0,vec![Target::Object(bear)]);finish(&mut g);
    let first=current(&g,first);assert_eq!(g.state.obj(first).attached_to,Some(bear));
    announce(&mut g,ward,ActionKind::CastSpell,0,vec![Target::Object(bear)]);finish(&mut g);
    assert_eq!(g.state.obj(current(&g,ward)).attached_to,Some(bear));
    assert!(g.state.obj(current(&g,first)).zone.is(ZoneKind::Graveyard));
    assert!(!g.can_attach_to(other,bear));
}

#[test]
fn vision_charm_reads_the_second_resolution_choice_without_a_query_placeholder() {
    let mut g=setup(&[&vision_charm::CARD]);let spell=put(&mut g,&vision_charm::CARD,A,Zone::hand(A));
    let land=put(&mut g,&BASIC,B,Zone::BATTLEFIELD);let mut choices=vec![Subtype::Plains,Subtype::Island].into_iter();
    announce(&mut g,spell,ActionKind::CastSpell,0,vec![]);
    finish_with(&mut g,&mut |_,r| match &r.kind {
        RequestKind::ChooseModes {..}=>Some(Answer::Modes(vec![1])),
        RequestKind::ChooseSubtype {choices:offered,..}=>{let kind=choices.next().unwrap();assert!(offered.contains(&kind));Some(Answer::Subtype(kind))},
        _=>None,
    });
    assert!(g.chars(land).has_subtype(Subtype::Island));assert!(!g.chars(land).has_subtype(Subtype::Plains));
}

#[test]
fn soul_exchange_reads_the_exact_creature_exiled_as_its_additional_cost() {
    static THRULL:CardDef=CardDef { name:"Fixture Thrull",subtypes:SubtypeSet::of(&[Subtype::Thrull]),..BEAR };
    for (paid,expected) in [(&THRULL,1),(&BEAR,0)] {
        let mut g=setup(&[&soul_exchange::CARD,&THRULL]);let spell=put(&mut g,&soul_exchange::CARD,A,Zone::hand(A));
        let payment=put(&mut g,paid,A,Zone::BATTLEFIELD);let returned=put(&mut g,&BEAR,A,Zone::graveyard(A));
        announce(&mut g,spell,ActionKind::CastSpell,0,vec![Target::Object(returned)]);finish(&mut g);
        let returned=current(&g,returned);assert!(g.state.obj(returned).zone.is(ZoneKind::Battlefield));
        assert!(g.state.obj(current(&g,payment)).zone.is(ZoneKind::Exile));
        assert_eq!(g.state.obj(returned).counters(CounterKind::Plus2Plus2),expected);
    }
}
