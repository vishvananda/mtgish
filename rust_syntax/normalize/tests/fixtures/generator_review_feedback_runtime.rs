//! Copy into cantrip-script/tests after installing the freshly emitted fixture cards.
use cantrip_api::*;
use cantrip_engine::{Game as RulesGame, Setup, card, cards};
use cantrip_state::{Frame, FrameKind, Span, INLINE_TARGETS};

fn game() -> RulesGame {
    RulesGame::new(cards(), &Setup { decks: vec![vec![], vec![]], ..Default::default() })
}

fn put(game: &mut RulesGame, name: &str, owner: PlayerId, zone: Zone) -> ObjectId {
    let id = game.state.push_obj(Obj::new(card(name).unwrap(), owner, zone));
    game.state.zone_push(zone, id);
    game.state.see_statics();
    id
}

fn effect(game: &mut RulesGame, object: ObjectId, kind: EffectKind) {
    let timestamp = game.state.timestamp();
    game.state.effect_push(ContinuousEffect {
        source: object, controller: PlayerId(0), timestamp, affected: object, kind,
        duration: Duration::Indefinite, words: Words::NONE,
    });
}

fn counters_after_damage(amount: u32) -> i16 {
    let mut game = game();
    let you = PlayerId(0);
    let aura = put(&mut game, "Living Artifact", you, Zone::BATTLEFIELD);
    resolve_trigger(&mut game, "Living Artifact", aura, you, Target::Player(you), [amount, 0], 0);
    game.state.obj(aura).counters(CounterKind::Vitality)
}

fn resolve_trigger(game: &mut RulesGame, name: &str, object: ObjectId, controller: PlayerId, subject: Target, data: [u32; 2], index: u8) {
    let item = StackItem {
        kind: StackKind::Triggered, object,
        ability: AbilityRef { card: card(name).unwrap(), index }, controller,
        inline_targets: [Target::None; INLINE_TARGETS], target_count: 0, inline_spec_of: [0; INLINE_TARGETS],
        x: 0, kicked: 0, counted: 0, paid: 0, modes: 0, way: 0, choice: 0,
        subject, data, members: Span::default(), serial: 0,
        gift: None, shares: StackItem::NO_SHARES, spill: 0, exile_instead: ExileInstead::Never,
        flashback: false, attacking: false, since: 0, copied: false, cast_known: u32::MAX,
        acquired: Acquired::OWN, words: Words::NONE,
    };
    game.state.stack_push(item);
    let serial = game.state.stack.last().unwrap().serial;
    game.state.frame_push(Frame::new(FrameKind::Resolve { serial }));
    game.run();
}

#[test]
fn generated_counter_effect_never_wraps_a_positive_damage_amount_negative() {
    for amount in [32767, 32768, 65535, u32::MAX] {
        assert_eq!(counters_after_damage(amount), 32767, "damage {amount}");
    }
}

#[test]
fn generated_protection_follows_changed_words_and_can_be_removed() {
    let mut game = game();
    let knight = put(&mut game, "Black Knight", PlayerId(0), Zone::BATTLEFIELD);
    assert!(game.chars(knight).protection.has(Color::White));
    effect(&mut game, knight, EffectKind::ChangeText {
        from: Word::Color(Color::White), to: Word::Color(Color::Blue),
    });
    assert!(game.chars(knight).protection.has(Color::Blue));
    assert!(!game.chars(knight).protection.has(Color::White));
    effect(&mut game, knight, EffectKind::LoseAllAbilities);
    assert_eq!(game.chars(knight).protection, ColorSet::NONE);
}

#[test]
fn generated_card_targets_reject_tokens_and_spell_copies_before_state_actions() {
    for name in ["Regrowth", "Raise Dead"] {
        let mut game = game();
        let you = PlayerId(0);
        let source = put(&mut game, name, you, Zone::STACK);
        let target = put(&mut game, "Ornithopter", you, Zone::graveyard(you));
        let definition = cards().get(card(name).unwrap());
        let filter = definition.spell.as_ref().unwrap().targets[0].filter;
        let asker = Asker::new(source, you);
        assert!(filter(&game, &asker, Target::Object(target)), "{name}: real card");
        game.state.update_obj(target, |o| o.token = true);
        assert!(!filter(&game, &asker, Target::Object(target)), "{name}: token");
        game.state.update_obj(target, |o| { o.token = false; o.card_copy = true; });
        assert!(!filter(&game, &asker, Target::Object(target)), "{name}: spell copy");
    }
}

#[test]
fn recorded_upkeep_player_who_has_left_is_not_instructed_to_draw() {
    let mut game = RulesGame::new(cards(), &Setup {
        decks: vec![vec![], vec![], vec![]], ..Default::default()
    });
    let departed = PlayerId(0);
    let controller = PlayerId(1);
    let aura = put(&mut game, "Super Intelligence", controller, Zone::BATTLEFIELD);
    let host = put(&mut game, "Ornithopter", controller, Zone::BATTLEFIELD);
    game.state.update_obj(aura, |o| o.attached_to = Some(host));
    game.state.update_player(departed, |p| { p.lost = true; p.drew_from_empty = false; });
    // This player's upkeep triggered before control changed and they left.
    resolve_trigger(&mut game, "Super Intelligence", aura, controller, Target::Object(host), [u32::from(departed.0), 0], 0);
    assert!(!game.state.player(departed).drew_from_empty);
}

#[test]
fn paralyze_untaps_the_triggering_creature_after_the_aura_moves() {
    let plains = card("Plains").unwrap();
    let mut game = RulesGame::new(cards(), &Setup {
        decks: vec![vec![plains; 20], vec![plains; 20]], shuffle: false, ..Default::default()
    });
    let owner = PlayerId(0);
    let payer = PlayerId(1);
    let aura = put(&mut game, "Paralyze", owner, Zone::BATTLEFIELD);
    let original = put(&mut game, "Ornithopter", payer, Zone::BATTLEFIELD);
    let current = put(&mut game, "Llanowar Elves", owner, Zone::BATTLEFIELD);
    game.state.update_obj(original, |o| o.tapped = true);
    game.state.update_obj(current, |o| o.tapped = true);
    game.state.update_obj(aura, |o| o.attached_to = Some(current));
    game.state.turn.active = payer;
    game.state.update_player(payer, |p| p.pool.0[Mana::C as usize] = 4);
    resolve_trigger(&mut game, "Paralyze", aura, owner, Target::Object(original), [u32::from(payer.0), 0], 1);
    let request = game.pending().cloned().expect("optional payment");
    assert_eq!(request.player, payer, "the triggering player chooses");
    let RequestKind::UnlessPays { options, .. } = &request.kind else { panic!("{request:?}") };
    let pay = options.iter().find(|o| o.kind == ActionKind::PayCost).expect("pay four");
    game.answer(request.id, payer, Answer::Act(Action::of(pay))).unwrap();
    game.run();
    assert!(!game.state.obj(original).tapped, "untap the creature the trigger referred to");
    assert!(game.state.obj(current).tapped, "the new enchanted creature stays tapped");
}
