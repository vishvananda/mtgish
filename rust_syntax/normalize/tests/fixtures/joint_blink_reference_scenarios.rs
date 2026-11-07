// Execute the generated attack body; combat dispatch is outside this offer contract.
static AMBER_PROGRESS: CardDef = CardDef {
    name: "Ambergris Progress attack callback fixture",
    types: TypeSet::of(&[CardType::Instant]),
    has_cost: true,
    spell: spell(&[], ambergris_progress::contract_callback),
    ..CardDef::BLANK
};
fn progress_with_empty_hand(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
    g.discard_hand(A);
    ambergris_progress::contract_callback(g, item)
}
static AMBER_PROGRESS_EMPTY: CardDef = CardDef {
    name: "Ambergris empty-hand attack callback fixture",
    spell: spell(&[], progress_with_empty_hand),
    ..AMBER_PROGRESS
};
#[test]
fn ambergris_joint_offer_respects_prohibitions_declines_replacements_and_empty_hands() {
    for (yes, forbidden, replaced, empty) in [
        (true, true, false, false),
        (false, false, false, false),
        (true, false, false, false),
        (true, false, true, false),
        (true, false, false, true),
    ] {
        let (mut g, spell) = game(
            if empty {
                &AMBER_PROGRESS_EMPTY
            } else {
                &AMBER_PROGRESS
            },
            &[&DRAW_REPLACEMENT],
        );
        if forbidden {
            put(&mut g, &DRAW_BAN, A, Zone::BATTLEFIELD);
        }
        if replaced {
            put(&mut g, &DRAW_REPLACEMENT, A, Zone::BATTLEFIELD);
        }
        announce(&mut g, spell, ActionKind::CastSpell, 0, vec![]);
        finish(&mut g, yes);
        assert_eq!(
            g.state.zone(Zone::hand(A)).len(),
            if !yes || forbidden {
                6
            } else if replaced {
                0
            } else {
                3
            },
            "yes={yes} forbidden={forbidden} replaced={replaced} empty={empty}"
        );
        assert_eq!(
            g.state.zone(Zone::graveyard(A)).len(),
            1 + if yes && !forbidden || empty { 6 } else { 0 }
        );
    }
}

static MELD_BACK: CardDef = CardDef {
    name: "Melded creature fixture",
    types: TypeSet::of(&[CardType::Creature]),
    power: Some(4),
    toughness: Some(4),
    ..CardDef::BLANK
};
static MELD_FIRST: CardDef = CardDef {
    name: "First meld card fixture",
    meld: Some(Meld {
        with: "Second meld card fixture",
        into: &MELD_BACK,
    }),
    ..BEAR
};
static MELD_SECOND: CardDef = CardDef {
    name: "Second meld card fixture",
    meld: Some(Meld {
        with: "First meld card fixture",
        into: &MELD_BACK,
    }),
    ..BEAR
};
fn blink_target(g: &mut RulesGame, melded: bool) -> ObjectId {
    let id = put(g, &MELD_FIRST, A, Zone::BATTLEFIELD);
    if melded {
        // The same state constructed by the engine's meld operation, with two physical cards.
        g.state.set_merge(
            id,
            cantrip_state::Merge {
                own_face: g.cards.id_of(&MELD_BACK),
                own_token: false,
                own_commander_id: None,
                own_face_down: None,
                parts: vec![cantrip_state::MergedPart {
                    card: g.cards.id_of(&MELD_SECOND),
                    face: g.cards.id_of(&MELD_SECOND),
                    token: false,
                    commander_id: None,
                    copy: None,
                    on_top: false,
                    timestamp: 1,
                    prior: None,
                    melded: true,
                }],
            },
        );
    }
    id
}
#[test]
fn blink_returns_every_meld_component_and_preserves_optional_decline() {
    for source in [
        &cloudshift::CARD,
        &conjurers_closet::CARD,
        &restoration_angel::CARD,
    ] {
        for melded in [false, true] {
            for yes in [false, true] {
                if source.name == cloudshift::CARD.name && !yes {
                    continue;
                }
                let (mut g, spell) = game(source, &[&MELD_FIRST, &MELD_SECOND, &MELD_BACK]);
                let victim = blink_target(&mut g, melded);
                if source.name == conjurers_closet::CARD.name {
                    put(&mut g, source, A, Zone::BATTLEFIELD);
                    g.state.update_turn(|t| t.set_step(Step::PostcombatMain));
                    // Let the real beginning-of-end-step trigger fire and choose its target.
                    for _ in 0..20 {
                        g.run();
                        if g.state.turn.step() == Step::End {
                            break;
                        }
                        let r = g.pending().cloned().unwrap();
                        assert!(matches!(r.kind, RequestKind::Priority { .. }));
                        g.answer(r.id, r.player, Answer::Pass).unwrap();
                    }
                    assert_eq!(g.state.turn.step(), Step::End);
                    finish_with_target(&mut g, yes, Some(Target::Object(victim)));
                } else {
                    announce(
                        &mut g,
                        spell,
                        ActionKind::CastSpell,
                        0,
                        if source.name == cloudshift::CARD.name {
                            vec![Target::Object(victim)]
                        } else {
                            vec![]
                        },
                    );
                    finish_with_target(&mut g, yes, Some(Target::Object(victim)));
                }
                let count = |def| {
                    g.state
                        .battlefield()
                        .iter()
                        .filter(|&&id| g.state.obj(id).card == g.cards.id_of(def))
                        .count()
                };
                assert_eq!(
                    count(&MELD_FIRST),
                    1,
                    "{} melded={melded} yes={yes}",
                    source.name
                );
                assert_eq!(
                    count(&MELD_SECOND),
                    usize::from(melded && yes),
                    "{} melded={melded} yes={yes}",
                    source.name
                );
                assert!(g.state.zone(Zone::exile(A)).is_empty());
                assert_eq!(
                    g.state.obj(victim).became.is_some(),
                    yes,
                    "{} melded={melded} yes={yes}",
                    source.name
                );
            }
        }
    }
}

// Invoke the generated block trigger with the real blocker carried in its event slot.
fn basalt_block_callback(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
    let blocker = g
        .state()
        .battlefield()
        .iter()
        .copied()
        .find(|&id| g.chars(id).name == BEAR.name)
        .unwrap();
    let mut event = item.clone();
    event.data[0] = blocker.0;
    match basalt_golem::CARD.abilities[0] {
        AbilityDef::Triggered(def) => (def.effect)(g, &event),
        _ => panic!("expected block trigger"),
    }
}
static BASALT_CALLBACK: CardDef = CardDef {
    name: "Basalt block callback fixture",
    types: TypeSet::of(&[CardType::Instant]),
    cost: ManaCost::ZERO,
    spell: spell(&[], basalt_block_callback),
    abilities: &[],
    block_rules: &[],
    power: None,
    toughness: None,
    ..basalt_golem::CARD
};
#[test]
fn basalt_delayed_wall_goes_to_the_actual_sacrificing_controller() {
    for removed in [false, true] {
        let mut extra = basalt_golem::EXTRA.to_vec();
        extra.push(&REMOVE);
        let (mut g, spell) = game(&BASALT_CALLBACK, &extra);
        let blocker = put(&mut g, &BEAR, A, Zone::BATTLEFIELD);
        // Bob controls Alice's card: after sacrifice, use its last controller,
        // not its owner or the controller of Basalt's delayed trigger.
        g.state.update_obj(blocker, |o| o.controller = B);
        let removal = removed.then(|| put(&mut g, &REMOVE, A, Zone::hand(A)));
        announce(&mut g, spell, ActionKind::CastSpell, 0, vec![]);
        finish(&mut g, true);
        if let Some(removal) = removal {
            announce(
                &mut g,
                removal,
                ActionKind::CastSpell,
                0,
                vec![Target::Object(blocker)],
            );
            finish(&mut g, true);
        }
        g.state.update_turn(|t| t.set_step(Step::CombatDamage));
        for _ in 0..20 {
            g.run();
            if g.state.turn.step() == Step::EndOfCombat {
                break;
            }
            let r = g.pending().cloned().unwrap();
            assert!(matches!(r.kind, RequestKind::Priority { .. }));
            g.answer(r.id, r.player, Answer::Pass).unwrap();
        }
        assert_eq!(g.state.turn.step(), Step::EndOfCombat);
        finish(&mut g, true);
        let walls: Vec<_> = g
            .state
            .battlefield()
            .iter()
            .copied()
            .filter(|&id| g.chars(id).has_subtype(Subtype::Wall))
            .collect();
        assert_eq!(walls.len(), usize::from(!removed));
        if let Some(&wall) = walls.first() {
            assert_eq!(g.controller(wall), B);
        }
        assert!(g.state.obj(blocker).became.is_some());
    }
}
