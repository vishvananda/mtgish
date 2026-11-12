fn noted_counter_fixture(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
    if let Some(object) = cantrip_mechanics::targets::object(g, item, 0) {
        g.add_counters_together(&[
            (object, CounterKind::Charge, 2),
            (object, CounterKind::PlusOne, 3),
        ]);
    }
    Ok(())
}
static NOTED_COUNTERS: CardDef = CardDef {
    name: "Noted counters fixture",
    spell: spell(PERMANENT, noted_counter_fixture),
    ..BOUNCE
};
fn noted_untap(g: &mut RulesGame, source: ObjectId) {
    let spell = put(g, &UNTAP, A, Zone::hand(A));
    hand_announce(
        g,
        spell,
        ActionKind::CastSpell,
        0,
        vec![Target::Object(source)],
    );
    finish(g);
}
fn noted_counter_spell(g: &mut RulesGame, source: ObjectId) {
    let spell = put(g, &NOTED_COUNTERS, A, Zone::hand(A));
    hand_announce(
        g,
        spell,
        ActionKind::CastSpell,
        0,
        vec![Target::Object(source)],
    );
    finish(g);
}
#[test]
fn generated_amulet_records_the_paid_type_and_keeps_it_for_an_external_charge() {
    let mut g = setup(&[&jeweled_amulet::CARD, &NOTED_COUNTERS]);
    let source = put(&mut g, &jeweled_amulet::CARD, A, Zone::BATTLEFIELD);
    for kind in [Mana::R, Mana::C] {
        g.state.empty_mana(A, ManaLasts::Step);
        g.state
            .add_mana(A, kind, 1, ObjectId::NONE, None, ManaLasts::Step, false);
        hand_announce(&mut g, source, ActionKind::ActivateAbility, 0, vec![]);
        finish(&mut g);
        assert_eq!(g.state.obj(source).counters(CounterKind::Charge), 1);
        assert!(g.state.player(A).pool.is_empty());
        noted_untap(&mut g, source);
        hand_announce(&mut g, source, ActionKind::ActivateAbility, 1, vec![]);
        finish(&mut g);
        assert_eq!(g.state.player(A).pool.get(kind), 1);
        assert_eq!(g.state.player(A).pool.total(), 1);
        assert_eq!(g.state.obj(source).counters(CounterKind::Charge), 0);
        noted_untap(&mut g, source);
    }
    g.state.empty_mana(A, ManaLasts::Step);
    noted_counter_spell(&mut g, source);
    hand_announce(&mut g, source, ActionKind::ActivateAbility, 1, vec![]);
    finish(&mut g);
    assert_eq!(g.state.player(A).pool, ManaPool([0, 0, 0, 0, 0, 1]));
    assert_eq!(g.state.obj(source).counters(CounterKind::Charge), 1);
}
#[test]
fn generated_amulet_with_charge_counters_but_no_note_produces_nothing() {
    let mut g = setup(&[&jeweled_amulet::CARD, &NOTED_COUNTERS]);
    let source = put(&mut g, &jeweled_amulet::CARD, A, Zone::BATTLEFIELD);
    g.state.empty_mana(A, ManaLasts::Step);
    noted_counter_spell(&mut g, source);
    hand_announce(&mut g, source, ActionKind::ActivateAbility, 1, vec![]);
    finish(&mut g);
    assert!(g.state.player(A).pool.is_empty());
    assert!(g.state.obj(source).tapped);
    assert_eq!(g.state.obj(source).counters(CounterKind::Charge), 1);
    assert!(!g
        .events()
        .iter()
        .any(|r| matches!(r.event, Event::ValueNoted { .. })));
}
#[test]
fn generated_coffin_note_probe_snapshots_the_exiled_creatures_old_counters() {
    let mut g = setup(&[&coffin_notes_probe::CARD, &NOTED_COUNTERS]);
    let source = put(&mut g, &coffin_notes_probe::CARD, A, Zone::BATTLEFIELD);
    let creature = put(&mut g, &BEAR, B, Zone::BATTLEFIELD);
    noted_counter_spell(&mut g, creature);
    hand_announce(
        &mut g,
        source,
        ActionKind::ActivateAbility,
        0,
        vec![Target::Object(creature)],
    );
    finish(&mut g);
    let exiled = current(&g, creature);
    assert!(g.state.obj(exiled).zone.is(ZoneKind::Exile));
    assert_eq!(g.state.obj(exiled).counter_kinds().count(), 0);
    let record = g.state.noted_values.last().expect("noted snapshot");
    assert_eq!(record.by.object, source);
    let NotedValue::Counters(counters) = &record.value else {
        panic!("counter note")
    };
    assert!(counters.contains(&(CounterKind::Charge, 2)));
    assert!(counters.contains(&(CounterKind::PlusOne, 3)));
    // This probe intentionally stops before the still-unsupported delayed return and Aura reattachment instructions.
    assert!(g.state.delayed.is_empty());
}
