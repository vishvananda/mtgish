use cantrip_api::*;
use cantrip_rules::{Game as RulesGame, Setup};
const A: PlayerId = PlayerId(0);
const B: PlayerId = PlayerId(1);
static FILLER: CardDef = CardDef {
    name: "Filler",
    types: TypeSet::of(&[CardType::Land]),
    ..CardDef::BLANK
};
static BEAR: CardDef = CardDef {
    name: "Bear",
    types: TypeSet::of(&[CardType::Creature]),
    power: Some(2),
    toughness: Some(2),
    ..CardDef::BLANK
};
static ARTIFACT_LAND: CardDef = CardDef {
    name: "Artifact land enchantment fixture",
    types: TypeSet::of(&[CardType::Artifact, CardType::Land, CardType::Enchantment]),
    cost: mana("{2}"),
    has_cost: true,
    ..CardDef::BLANK
};
fn no_draw(_: &dyn Query, _: &Asker, _: PlayerId, _: u32) -> bool {
    true
}
static DRAW_BAN: CardDef = CardDef {
    name: "Draw prohibition fixture",
    types: TypeSet::of(&[CardType::Enchantment]),
    cant_draw: &[no_draw],
    ..CardDef::BLANK
};
static IDOL: CardDef = CardDef {
    name: "Extra-type Chimeric Idol fixture",
    types: TypeSet::of(&[CardType::Artifact, CardType::Land, CardType::Enchantment]),
    ..chimeric_idol::CARD
};
static AMBER: CardDef = CardDef {
    name: "Ambergris callback fixture",
    types: TypeSet::of(&[CardType::Instant]),
    has_cost: true,
    spell: spell(&[], ambergris__agent_of_law::contract_callback),
    ..CardDef::BLANK
};
static AMBER_LEGACY: CardDef = CardDef {
    name: "Legacy optional Ambergris fixture",
    types: TypeSet::of(&[CardType::Instant]),
    has_cost: true,
    spell: spell(&[], ambergris_legacy::contract_callback),
    ..CardDef::BLANK
};
fn game(source: &'static CardDef, extra: &[&'static CardDef]) -> (RulesGame, ObjectId) {
    let mut defs = vec![&FILLER, &BEAR, &ARTIFACT_LAND, &DRAW_BAN, source];
    defs.extend(extra);
    let table = Box::leak(Box::new(CardTable::new(defs)));
    let fill = table.id_of(&FILLER);
    let mut mine = vec![fill; 30];
    mine[0] = table.id_of(source);
    let mut g = RulesGame::new(
        table,
        &Setup {
            decks: vec![mine, vec![fill; 30]],
            ..Default::default()
        },
    );
    g.state.update_turn(|t| {
        t.number = 1;
        t.active = A;
        t.priority = A;
        t.set_step(Step::PrecombatMain);
        t.began = true;
    });
    g.state.update_player(A, |p| p.turn_began = 1);
    for mana in Mana::ALL {
        g.state.add_mana(
            A,
            mana,
            100,
            ObjectId::NONE,
            None,
            Default::default(),
            false,
        );
    }
    let id = g.state.zone(Zone::hand(A))[0];
    (g, id)
}
fn put(g: &mut RulesGame, def: &'static CardDef, p: PlayerId, zone: Zone) -> ObjectId {
    let id = g.state.push_obj(Obj::new(g.cards.id_of(def), p, zone));
    g.state.zone_push(zone, id);
    id
}
fn announce(g: &mut RulesGame, id: ObjectId, kind: ActionKind, index: u8, targets: Vec<Target>) {
    g.state.see_statics();
    if g.pending().is_none() {
        g.state.frame_push(cantrip_state::Frame::new(
            cantrip_state::FrameKind::Priority,
        ));
    }
    g.run();
    let r = g.pending().cloned().expect("priority");
    let RequestKind::Priority { options } = &r.kind else {
        panic!("{:?}", r.kind)
    };
    let o = options
        .iter()
        .find(|o| o.source == Some(id) && o.kind == kind && o.index == index)
        .unwrap_or_else(|| panic!("missing action {kind:?}/{index} {options:?}"));
    let mut a = Action::of(o);
    a.targets = targets;
    g.answer(r.id, r.player, Answer::Act(a)).unwrap();
}
fn finish(g: &mut RulesGame, yes: bool) {
    for _ in 0..200 {
        g.run();
        let Some(r) = g.pending().cloned() else {
            assert!(g.state.is_over());
            return;
        };
        let answer = match &r.kind {
            RequestKind::Priority { .. } if g.state.stack.is_empty() => return,
            RequestKind::Priority { .. } => Answer::Pass,
            RequestKind::PayCosts { options, .. } => {
                let o = options
                    .iter()
                    .find(|o| o.kind == ActionKind::AssignMana)
                    .or_else(|| {
                        options
                            .iter()
                            .find(|o| o.kind == ActionKind::ConfirmCasting)
                    })
                    .expect("payment option");
                Answer::Act(Action::of(o))
            }
            RequestKind::ChooseTarget { candidates, .. } => Answer::Targets(vec![candidates[0]]),
            RequestKind::ChooseModes { .. } => Answer::Modes(vec![0]),
            RequestKind::ChooseCostPayment { .. } => Answer::CostPayment(0),
            RequestKind::May { .. } => {
                if yes {
                    Answer::Yes
                } else {
                    Answer::No
                }
            }
            RequestKind::ChooseCards {
                cards,
                rule,
                purpose,
                ..
            } => {
                let least = match rule.count {
                    cantrip_mechanics::select::Count::Exactly(n)
                    | cantrip_mechanics::select::Count::AsManyAsCan(n) => n.min(cards.len() as u32),
                    cantrip_mechanics::select::Count::UpTo(_)
                    | cantrip_mechanics::select::Count::AnyNumber => 0,
                    other => panic!("count {other:?}"),
                };
                {
                    let mut choices = cards.clone();
                    if *purpose == Purpose::Exile {
                        choices.sort_by_key(|&id| !g.chars(id).is(CardType::Creature));
                    }
                    Answer::Cards(choices.into_iter().take(least as usize).collect())
                }
            }
            RequestKind::OrderTriggers { sources, .. } => {
                Answer::Order((0..sources.len() as u32).collect())
            }
            other => panic!("unhandled {other:?}"),
        };
        g.answer(r.id, r.player, answer).unwrap();
    }
    panic!("did not finish")
}
#[test]
fn artifact_creature_retains_land_and_enchantment() {
    let (mut g, spell) = game(&karn_s_touch::CARD, &[]);
    let target = put(&mut g, &ARTIFACT_LAND, A, Zone::BATTLEFIELD);
    announce(
        &mut g,
        spell,
        ActionKind::CastSpell,
        0,
        vec![Target::Object(target)],
    );
    finish(&mut g, true);
    let c = g.chars(target);
    for t in [
        CardType::Artifact,
        CardType::Land,
        CardType::Enchantment,
        CardType::Creature,
    ] {
        assert!(c.is(t), "{t:?}")
    }
    assert_eq!(c.power, Some(2));
}
#[test]
fn chimeric_idol_retains_extra_types_and_sets_turtle() {
    let (mut g, _) = game(&IDOL, &[]);
    let id = put(&mut g, &IDOL, A, Zone::BATTLEFIELD);

    announce(&mut g, id, ActionKind::ActivateAbility, 0, vec![]);
    finish(&mut g, true);
    let c = g.chars(id);
    for t in [
        CardType::Artifact,
        CardType::Land,
        CardType::Enchantment,
        CardType::Creature,
    ] {
        assert!(c.is(t))
    }
    assert!(c.has_subtype(Subtype::Turtle));
    assert_eq!(c.power, Some(3));
}
#[test]
fn haunted_fengraf_empty_one_and_two() {
    for n in 0usize..=2 {
        let (mut g, _) = game(&haunted_fengraf::CARD, &[]);
        let id = put(&mut g, &haunted_fengraf::CARD, A, Zone::BATTLEFIELD);
        let _ids: Vec<_> = (0..n)
            .map(|_| put(&mut g, &BEAR, A, Zone::graveyard(A)))
            .collect();
        let before = g.state.zone(Zone::hand(A)).len();
        announce(&mut g, id, ActionKind::ActivateAbility, 1, vec![]);
        finish(&mut g, true);
        assert_eq!(
            g.state.zone(Zone::hand(A)).len(),
            before + usize::from(n > 0)
        );
        assert_eq!(
            g.state
                .zone(Zone::graveyard(A))
                .iter()
                .filter(|&&o| g.state.obj(o).card == g.cards.id_of(&BEAR))
                .count(),
            n.saturating_sub(1)
        );
    }
}
#[test]
fn laquatus_native_discards_the_original_hand_count() {
    let (mut g, spell) = game(&laquatus_s_creativity::CARD, &[]);
    announce(
        &mut g,
        spell,
        ActionKind::CastSpell,
        0,
        vec![Target::Player(B)],
    );
    finish(&mut g, true);
    assert_eq!(g.state.zone(Zone::hand(B)).len(), 7);
}
#[test]
fn ambergris_declines_unaffordable_draw_cost_without_discarding_or_pumping() {
    for def in [&AMBER, &AMBER_LEGACY] {
        let (mut g, spell) = game(def, &[]);
        let friend = put(&mut g, &BEAR, A, Zone::BATTLEFIELD);
        put(&mut g, &DRAW_BAN, A, Zone::BATTLEFIELD);
        announce(&mut g, spell, ActionKind::CastSpell, 0, vec![]);
        finish(&mut g, true);
        assert_eq!(g.state.zone(Zone::hand(A)).len(), 6, "{}", def.name);
        assert_eq!(g.chars(friend).power, Some(2), "{}", def.name);
    }
}
static CAPTURE: CardDef = CardDef {
    ..laquatus_s_creativity::CARD
};
#[test]
fn laquatus_captured_request_survives_prevention_and_partial_draws() {
    for forbidden in [false, true] {
        for remaining in [0usize, 1, 20] {
            let (mut g, spell) = game(&CAPTURE, &[]);
            if forbidden {
                put(&mut g, &DRAW_BAN, A, Zone::BATTLEFIELD);
            }
            // Seven cards initially; keep a controlled short library for the recipient.
            let old = g.state.zone(Zone::library(B)).to_vec();
            for &o in old.iter().skip(remaining) {
                g.state.zone_remove(Zone::library(B), o);
            }
            announce(
                &mut g,
                spell,
                ActionKind::CastSpell,
                0,
                vec![Target::Player(B)],
            );
            finish(&mut g, true);
            let actual = if forbidden { 0 } else { remaining.min(7) };
            assert_eq!(
                g.state.zone(Zone::hand(B)).len(),
                actual,
                "ban={forbidden}, remaining={remaining}"
            );
            assert_eq!(
                g.history()
                    .this_turn()
                    .iter()
                    .filter(|r| matches!(r.event, Event::Discarded { player: B, .. }))
                    .count(),
                7
            );
        }
    }
}
fn bounce(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
    g.return_to_hand(&[item.target(0).object().unwrap()]);
    Ok(())
}
fn any_creature(q: &dyn Query, _: &Asker, t: Target) -> bool {
    matches!(t,Target::Object(o) if q.chars(o).is(CardType::Creature))
}
static REMOVE: CardDef = CardDef {
    name: "Bounce fixture",
    types: TypeSet::of(&[CardType::Instant]),
    has_cost: true,
    spell: spell(&[target(any_creature)], bounce),
    ..CardDef::BLANK
};
fn to_end(g: &mut RulesGame) {
    for _ in 0..100 {
        g.run();
        let r = g.pending().cloned().unwrap();
        if g.state.turn.step() == Step::End
            && g.state.stack.is_empty()
            && matches!(r.kind, RequestKind::Priority { .. })
        {
            return;
        }
        let ans = match r.kind {
            RequestKind::Priority { .. } => Answer::Pass,
            RequestKind::DeclareAttackers { .. } => Answer::Attack(vec![]),
            RequestKind::OrderTriggers { sources, .. } => {
                Answer::Order((0..sources.len() as u32).collect())
            }
            x => panic!("end {x:?}"),
        };
        g.answer(r.id, r.player, ans).unwrap();
    }
    panic!("end not reached")
}
#[test]
fn artisan_source_and_captured_departing_subject_keep_provenance() {
    for def in [&arcane_artisan::CARD] {
        let (mut g, remove) = game(&REMOVE, &[def]);
        let first = put(&mut g, def, A, Zone::BATTLEFIELD);
        let second = put(&mut g, def, A, Zone::BATTLEFIELD);
        put(&mut g, &BEAR, B, Zone::hand(B));
        put(&mut g, &BEAR, B, Zone::hand(B));
        announce(
            &mut g,
            first,
            ActionKind::ActivateAbility,
            0,
            vec![Target::Player(B)],
        );
        finish(&mut g, true);
        let first_token = *g
            .state
            .battlefield()
            .iter()
            .find(|&&o| g.state.obj(o).token)
            .expect("first token");
        announce(
            &mut g,
            second,
            ActionKind::ActivateAbility,
            0,
            vec![Target::Player(B)],
        );
        finish(&mut g, true);
        let second_token = *g
            .state
            .battlefield()
            .iter()
            .find(|&&o| g.state.obj(o).token && o != first_token)
            .unwrap_or_else(|| {
                panic!(
                    "second token: board={:?}, hand={:?}, history={:?}",
                    g.state.battlefield(),
                    g.state.zone(Zone::hand(B)),
                    g.history().this_turn()
                )
            });
        announce(
            &mut g,
            remove,
            ActionKind::CastSpell,
            0,
            vec![Target::Object(first)],
        );
        finish(&mut g, true);
        assert!(g.state.obj(first).became.is_some());
        to_end(&mut g);
        assert!(!g.state.battlefield().contains(&first_token));
        assert!(g.state.battlefield().contains(&second_token));
    }
}

fn draw_applies(_: &dyn Query, _: &Asker, p: &Proposal) -> bool {
    matches!(p, Proposal::Draw(_))
}
fn draw_replace(_: &dyn Query, _: &Asker, p: &mut Proposal) {
    if let Proposal::Draw(d) = p {
        d.replaced = true;
    }
}
static DRAW_REPLACEMENT: CardDef = CardDef {
    name: "Replace draw fixture",
    types: TypeSet::of(&[CardType::Enchantment]),
    replacements: &[ReplacementDef {
        hint: "Skip draw",
        kind: ReplacementKind::Draw,
        own_everywhere: false,
        applies: draw_applies,
        apply: draw_replace,
        then: None,
        damage: None,
        optional: None,
        in_graveyard: false,
    }],
    ..CardDef::BLANK
};
#[test]
fn laquatus_requested_count_survives_replaced_draws() {
    let (mut g, spell) = game(&CAPTURE, &[&DRAW_REPLACEMENT]);
    put(&mut g, &DRAW_REPLACEMENT, A, Zone::BATTLEFIELD);
    announce(
        &mut g,
        spell,
        ActionKind::CastSpell,
        0,
        vec![Target::Player(B)],
    );
    finish(&mut g, true);
    assert_eq!(g.state.zone(Zone::hand(B)).len(), 0);
    assert_eq!(
        g.history()
            .this_turn()
            .iter()
            .filter(|r| matches!(r.event, Event::Discarded { player: B, .. }))
            .count(),
        7
    );
    assert_eq!(g.history().drawn(B), 0);
}
#[test]
fn ambergris_replaced_draw_payment_counts_and_declined_payment_does_not() {
    for def in [&AMBER, &AMBER_LEGACY] {
        for yes in [true, false] {
            let (mut g, spell) = game(def, &[&DRAW_REPLACEMENT]);
            let friend = put(&mut g, &BEAR, A, Zone::BATTLEFIELD);
            put(&mut g, &DRAW_REPLACEMENT, A, Zone::BATTLEFIELD);
            announce(&mut g, spell, ActionKind::CastSpell, 0, vec![]);
            finish(&mut g, yes);
            assert_eq!(g.chars(friend).power, Some(if yes { 8 } else { 2 }));
            assert_eq!(g.history().drawn(A), 0);
        }
    }
}

#[test]
fn abstract_performance_preserves_an_unrelated_exiled_card() {
    let (mut g, spell) = game(&abstract_performance::CARD, &[]);
    let unrelated = put(&mut g, &FILLER, B, Zone::exile(B));
    announce(&mut g, spell, ActionKind::CastSpell, 0, vec![]);
    finish(&mut g, true);
    assert!(g.state.zone(Zone::exile(B)).contains(&unrelated));
    assert!(g.state.obj(unrelated).became.is_none());
    assert_eq!(g.state.zone(Zone::hand(A)).len(), 10);
    assert_eq!(g.state.zone(Zone::graveyard(A)).len(), 5);
}
static BALANCE: CardDef = CardDef {
    name: "Balance attack-callback fixture",
    supertypes: CardDef::BLANK.supertypes,
    types: TypeSet::of(&[CardType::Instant]),
    power: None,
    toughness: None,
    abilities: &[],
    spell: spell(&[], ambergris_balance::contract_callback),
    ..ambergris_balance::CARD
};
#[test]
fn replaced_compound_draws_trigger_when_you_do_but_forbidden_draws_do_not() {
    for forbidden in [false, true] {
        let (mut g, source) = game(&BALANCE, &[&DRAW_REPLACEMENT]);
        let friend = put(&mut g, &BEAR, A, Zone::BATTLEFIELD);
        put(
            &mut g,
            if forbidden {
                &DRAW_BAN
            } else {
                &DRAW_REPLACEMENT
            },
            A,
            Zone::BATTLEFIELD,
        );
        announce(&mut g, source, ActionKind::CastSpell, 0, vec![]);
        finish(&mut g, true);
        assert_eq!(g.chars(friend).power, Some(if forbidden { 2 } else { 8 }));
        assert_eq!(
            g.state.zone(Zone::hand(A)).len(),
            if forbidden { 6 } else { 0 }
        );
    }
}
