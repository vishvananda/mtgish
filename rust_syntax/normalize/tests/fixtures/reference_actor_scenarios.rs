// Run the generated attack body with a targeted callback fixture. No combat
// damage is included in this witness: this isolates the trigger's damage actor.
fn hamsphere_attack(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
    match rolling_hamsphere::CARD.abilities[0] {
        AbilityDef::Triggered(def) => (def.effect)(g, item),
        _ => panic!("attack trigger"),
    }
}
static HAMSPHERE_CALLBACK: CardDef = CardDef {
    name: "Hamsphere attack callback fixture",
    types: TypeSet::of(&[CardType::Instant]),
    cost: ManaCost::ZERO,
    spell: spell(&[cantrip_mechanics::targets::ANY], hamsphere_attack),
    abilities: &[],
    statics: &[],
    ..rolling_hamsphere::CARD
};
#[test]
fn hamsphere_damage_is_once_from_the_holder_and_counts_all_hamsters() {
    for prior in [0, 2] {
        let (mut g, spell) = game(&HAMSPHERE_CALLBACK, rolling_hamsphere::EXTRA);
        let hamster = rolling_hamsphere::EXTRA[0];
        for _ in 0..prior {
            put(&mut g, hamster, A, Zone::BATTLEFIELD);
        }
        announce(
            &mut g,
            spell,
            ActionKind::CastSpell,
            0,
            vec![Target::Player(B)],
        );
        finish(&mut g, true);
        assert_eq!(g.state.player(B).life, 20 - (prior + 3));
        let damage: Vec<_> = g
            .history()
            .this_turn()
            .iter()
            .filter_map(|r| match r.event {
                Event::Damage {
                    source,
                    target: Target::Player(p),
                    amount,
                    ..
                } if p == B => Some((source, amount)),
                _ => None,
            })
            .collect();
        assert_eq!(
            damage,
            vec![(g.state.obj(spell).became.unwrap(), (prior + 3) as u32)]
        );
    }
}
fn pool_free_spell(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
    g.gain_life(item.controller, 7);
    Ok(())
}
static POOL_OFFER: CardDef = CardDef {
    name: "Pool offered spell fixture",
    types: TypeSet::of(&[CardType::Instant]),
    cost: ManaCost::ZERO,
    has_cost: true,
    spell: spell(&[], pool_free_spell),
    ..CardDef::BLANK
};
static POOL_HAND_SPELL: CardDef = CardDef {
    name: "Pool triggering spell fixture",
    ..POOL_OFFER
};
#[test]
fn pool_cast_choice_and_free_spell_belong_to_the_triggering_opponent() {
    for accept in [false, true] {
        let (mut g, _) = game(&knowledge_pool::CARD, &[&POOL_OFFER, &POOL_HAND_SPELL]);
        let pool = put(&mut g, &knowledge_pool::CARD, A, Zone::BATTLEFIELD);
        // Seed a legal, different card in Pool's linked exile, without invoking
        // its independent ETB trigger. The new hand spell must be excluded below.
        let offered = put(&mut g, &POOL_OFFER, A, Zone::exile(A));
        let link = g.text_linked(pool);
        g.state.update_obj(offered, |o| {
            o.exiled = Some(cantrip_state::ExileLink::new(link, Zone::hand(A), false))
        });
        let triggering = put(&mut g, &POOL_HAND_SPELL, B, Zone::hand(B));
        g.state.update_turn(|t| {
            t.active = B;
            t.priority = B;
        });
        g.state.update_player(B, |p| p.turn_began = 1);
        announce(&mut g, triggering, ActionKind::CastSpell, 0, vec![]);
        let mut choices = 0;
        for _ in 0..100 {
            g.run();
            let r = g.pending().cloned().expect("decision");
            let answer = match &r.kind {
                RequestKind::Priority { .. } if g.state.stack.is_empty() => break,
                RequestKind::Priority { .. } => Answer::Pass,
                RequestKind::PayCosts { options, .. } => {
                    let option = options
                        .iter()
                        .find(|o| o.kind == ActionKind::AssignMana)
                        .or_else(|| {
                            options
                                .iter()
                                .find(|o| o.kind == ActionKind::ConfirmCasting)
                        })
                        .expect("payment option");
                    Answer::Act(Action::of(option))
                }
                RequestKind::ChooseCards {
                    cards,
                    purpose: Purpose::CastFree,
                    ..
                } => {
                    choices += 1;
                    assert_eq!(r.player, B);
                    assert_eq!(cards.as_slice(), &[offered], "only other linked cards may be offered");
                    Answer::Cards(if accept { vec![offered] } else { vec![] })
                }
                RequestKind::May {
                    purpose: Purpose::CastFree,
                    ..
                } => {
                    choices += 1;
                    assert_eq!(r.player, B);
                    if accept {
                        Answer::Yes
                    } else {
                        Answer::No
                    }
                }
                RequestKind::OrderTriggers { sources, .. } => {
                    Answer::Order((0..sources.len() as u32).collect())
                }
                other => panic!("unhandled Pool decision {other:?}"),
            };
            g.answer(r.id, r.player, answer).unwrap();
        }
        assert_eq!(choices, 1);
        assert!(g.state.stack.is_empty());
        assert_eq!(g.state.player(A).life, 20);
        assert_eq!(g.state.player(B).life, if accept { 27 } else { 20 });
        assert!(g.state.obj(triggering).became.is_some());
        let cast: Vec<_> = g
            .history()
            .this_turn()
            .iter()
            .filter_map(|r| match r.event {
                Event::SpellCast { spell, player, .. }
                    if g.state.obj(spell).card == g.cards.id_of(&POOL_OFFER) =>
                {
                    Some(player)
                }
                _ => None,
            })
            .collect();
        assert_eq!(cast, if accept { vec![B] } else { vec![] });
    }
}
