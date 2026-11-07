static FOUND: CardDef = CardDef {
    name: "Found creature with mana value five",
    cost: mana("{5}"),
    has_cost: true,
    ..BEAR
};
fn library(g: &mut RulesGame, cards: &[&'static CardDef]) {
    let zone = Zone::library(A);
    for id in g.state.zone(zone).to_vec() {
        g.state.zone_remove(zone, id);
    }
    for card in cards {
        put(g, card, A, zone);
    }
}
#[test]
fn yuna_captures_found_value_before_moving_the_card_even_without_a_remainder() {
    for prefix in [0, 2] {
        let (mut g, spell) = game(&yuna_s_whistle::CARD, &[&FOUND]);
        let friend = put(&mut g, &BEAR, A, Zone::BATTLEFIELD);
        let mut cards = vec![&FILLER; prefix];
        cards.push(&FOUND);
        library(&mut g, &cards);
        announce(&mut g, spell, ActionKind::CastSpell, 0, vec![]);
        finish(&mut g, true);
        assert_eq!(g.chars(friend).power, Some(7), "prefix {prefix}");
        assert!(g
            .state
            .zone(Zone::hand(A))
            .iter()
            .any(|&id| g.state.obj(id).card == g.cards.id_of(&FOUND)));
    }
}
#[test]
fn calibrated_blast_uses_matching_nonland_instead_of_the_first_revealed_land() {
    for prefix in [0, 2] {
        let (mut g, spell) = game(&calibrated_blast::CARD, &[&FOUND]);
        // Select a creature explicitly so the expected damage is observable.
        let target = put(&mut g, &FOUND, B, Zone::BATTLEFIELD);
        let mut cards = vec![&FILLER; prefix];
        cards.push(&FOUND);
        library(&mut g, &cards);
        announce(&mut g, spell, ActionKind::CastSpell, 0, vec![]);
        finish_with_target(&mut g, true, Some(Target::Object(target)));
        assert!(
            !g.state.battlefield().contains(&target),
            "prefix {prefix}"
        );
    }
}
#[test]
fn fixed_optional_exiles_do_not_partially_execute_below_the_required_population() {
    for (def, required) in [(&young_necromancer::CARD, 2), (&kroxa_and_kunoros::CARD, 5)] {
        for n in 0..required {
            let (mut g, spell) = game(def, &[]);
            for _ in 0..n {
                put(&mut g, &BEAR, A, Zone::graveyard(A));
            }
            announce(&mut g, spell, ActionKind::CastSpell, 0, vec![]);
            finish(&mut g, true);
            assert_eq!(
                g.state.zone(Zone::graveyard(A)).len(),
                n,
                "{} {n}",
                def.name
            );
            assert_eq!(g.state.zone(Zone::exile(A)).len(), 0, "{} {n}", def.name);
        }
    }
}
fn kill_for_grave_return(g: &mut dyn Game, _: &StackItem) -> Result<(), Suspend> {
    let ids = g.state().battlefield().to_vec();
    g.destroy_all(&ids);
    Ok(())
}
fn kill_and_move_for_grave_return(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
    kill_for_grave_return(g, item)?;
    let ids = g.state().zone(Zone::graveyard(A)).to_vec();
    g.exile_all(&ids);
    Ok(())
}
static KILL_FOR_RETURN: CardDef = CardDef {
    name: "Kill for return witness",
    types: TypeSet::of(&[CardType::Instant]),
    has_cost: true,
    spell: spell(&[], kill_for_grave_return),
    ..CardDef::BLANK
};
static KILL_AND_MOVE_FOR_RETURN: CardDef = CardDef {
    name: "Kill and move for return witness",
    spell: spell(&[], kill_and_move_for_grave_return),
    ..KILL_FOR_RETURN
};
#[test]
fn dead_library_returns_use_the_first_graveyard_incarnation_and_do_not_chase_moves() {
    for def in [&arashin_sovereign::CARD, &avenging_angel::CARD] {
        for moved_again in [false, true] {
            let killer = if moved_again {
                &KILL_AND_MOVE_FOR_RETURN
            } else {
                &KILL_FOR_RETURN
            };
            let (mut g, spell) = game(killer, &[def]);
            put(&mut g, def, A, Zone::BATTLEFIELD);
            announce(&mut g, spell, ActionKind::CastSpell, 0, vec![]);
            finish(&mut g, true);
            assert_eq!(
                g.state
                    .zone(Zone::library(A))
                    .iter()
                    .any(|&id| g.state.obj(id).card == g.cards.id_of(def)),
                !moved_again,
                "{} {moved_again}",
                def.name
            );
        }
    }
}
