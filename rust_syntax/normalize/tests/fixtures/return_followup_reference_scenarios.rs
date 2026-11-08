static RETURN_MELD_BACK: CardDef = CardDef {
    name: "Return-grant melded creature fixture",
    types: TypeSet::of(&[CardType::Creature]),
    power: Some(4),
    toughness: Some(4),
    ..CardDef::BLANK
};
static RETURN_MELD_FIRST: CardDef = CardDef {
    name: "Return-grant first meld card fixture",
    meld: Some(Meld {
        with: "Return-grant second meld card fixture",
        into: &RETURN_MELD_BACK,
    }),
    subtypes: SubtypeSet::of(&[Subtype::Spirit]),
    ..BEAR
};
static RETURN_MELD_SECOND: CardDef = CardDef {
    name: "Return-grant second meld card fixture",
    meld: Some(Meld {
        with: "Return-grant first meld card fixture",
        into: &RETURN_MELD_BACK,
    }),
    subtypes: SubtypeSet::of(&[Subtype::Spirit]),
    ..BEAR
};
static RETURN_NONSPIRIT_SECOND: CardDef = CardDef {
    name: "Non-Spirit return component fixture",
    ..BEAR
};
fn return_blink_target(g: &mut RulesGame, melded: bool, second: &'static CardDef) -> ObjectId {
    let id = put(g, &RETURN_MELD_FIRST, A, Zone::BATTLEFIELD);
    if melded {
        // The same state constructed by the engine's meld operation, with two physical cards.
        g.state.set_merge(
            id,
            cantrip_state::Merge {
                own_face: g.cards.id_of(&RETURN_MELD_BACK),
                own_token: false,
                own_commander_id: None,
                own_face_down: None,
                parts: vec![cantrip_state::MergedPart {
                    card: g.cards.id_of(second),
                    face: g.cards.id_of(second),
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
fn post_return_actions_cover_both_meld_components() {
    for source in [&portal::CARD, &flux::CARD, &respite::CARD] {
        for owner in [A, B] {
            if source.name != respite::CARD.name && owner != A {
                continue;
            }
            for melded in [false, true] {
                let (mut g, spell) = game(
                    source,
                    &[&RETURN_MELD_FIRST, &RETURN_MELD_SECOND, &RETURN_MELD_BACK],
                );
                let victim = return_blink_target(&mut g, melded, &RETURN_MELD_SECOND);
                if owner != A {
                    g.state.update_obj(victim, |o| {
                        o.owner = owner;
                        o.controller = owner;
                    });
                }
                announce(
                    &mut g,
                    spell,
                    ActionKind::CastSpell,
                    0,
                    vec![Target::Object(victim)],
                );
                finish(&mut g, true);
                let returned: Vec<_> = g
                    .state
                    .battlefield()
                    .iter()
                    .copied()
                    .filter(|&id| {
                        let c = g.state.obj(id).card;
                        c == g.cards.id_of(&RETURN_MELD_FIRST)
                            || c == g.cards.id_of(&RETURN_MELD_SECOND)
                    })
                    .collect();
                assert_eq!(returned.len(), if melded { 2 } else { 1 });
                for id in returned {
                    if source.name == portal::CARD.name {
                        assert!(
                            g.chars(id).has(Keyword::FirstStrike),
                            "{} {id:?}",
                            source.name
                        );
                    } else {
                        assert_eq!(
                            g.state.obj(id).counters(CounterKind::PlusOne),
                            i32::from(owner == A),
                            "{} {id:?} owner={owner:?}",
                            source.name
                        );
                        assert_eq!(
                            g.state.obj(id).tapped,
                            owner != A,
                            "{} {id:?} owner={owner:?}",
                            source.name
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn essence_flux_does_not_counter_non_spirits() {
    let (mut g, spell) = game(&flux::CARD, &[]);
    let victim = put(&mut g, &BEAR, A, Zone::BATTLEFIELD);
    announce(
        &mut g,
        spell,
        ActionKind::CastSpell,
        0,
        vec![Target::Object(victim)],
    );
    finish(&mut g, true);
    for &id in g.state.battlefield() {
        assert_eq!(g.state.obj(id).counters(CounterKind::PlusOne), 0);
    }
}

#[test]
fn essence_flux_tests_each_returned_component_separately() {
    let (mut g, spell) = game(
        &flux::CARD,
        &[
            &RETURN_MELD_FIRST,
            &RETURN_NONSPIRIT_SECOND,
            &RETURN_MELD_BACK,
            &RETURN_MELD_SECOND,
        ],
    );
    let victim = return_blink_target(&mut g, true, &RETURN_NONSPIRIT_SECOND);
    announce(
        &mut g,
        spell,
        ActionKind::CastSpell,
        0,
        vec![Target::Object(victim)],
    );
    finish(&mut g, true);
    let returned: Vec<_> = g.state.battlefield().iter().copied().collect();
    assert_eq!(returned.len(), 2);
    for id in returned {
        let spirit = g.state.obj(id).card == g.cards.id_of(&RETURN_MELD_FIRST);
        assert_eq!(
            g.state.obj(id).counters(CounterKind::PlusOne),
            i32::from(spirit)
        );
    }
}
