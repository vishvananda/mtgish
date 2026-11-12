#[test]
fn generated_land_edge_uses_the_activators_discard_and_keeps_the_enchantments_controller() {
    for land in [true,false] {
        let mut g=setup(&[&land_s_edge::CARD]);let source=put(&mut g,&land_s_edge::CARD,B,Zone::BATTLEFIELD);
        // Initial cards are all lands. Remove the activator's other choices to make its one paid object exact.
        let old_hand=g.state.zone(Zone::hand(A)).to_vec();
        for id in old_hand { g.state.zone_remove(Zone::hand(A),id);g.state.zone_push(Zone::graveyard(A),id);g.state.update_obj(id,|o|o.zone=Zone::graveyard(A)); }
        let paid=put(&mut g,if land {&BASIC}else{&BEAR},A,Zone::hand(A));
        announce(&mut g,source,ActionKind::ActivateAbility,0,vec![Target::Player(B)]);finish(&mut g);
        assert_eq!(g.state.obj(source).controller,B);
        assert!(g.state.obj(current(&g,paid)).zone.is(ZoneKind::Graveyard));
        assert_eq!(g.state.player(B).life,if land {18}else{20});
        assert_eq!(g.state.player(A).life,20);
        assert!(g.history().all().iter().any(|r|matches!(r.event,Event::AbilityActivated {source:s,player:A,..} if s==source)));
    }
}
#[test]
fn generated_clergy_allows_an_opponent_to_pay_for_its_regeneration_prohibition() {
    let mut g=setup(&[&clergy_of_the_holy_nimbus::CARD]);let source=put(&mut g,&clergy_of_the_holy_nimbus::CARD,B,Zone::BATTLEFIELD);
    announce(&mut g,source,ActionKind::ActivateAbility,0,vec![]);finish(&mut g);
    assert_eq!(g.state.obj(source).controller,B);
    assert!(g.state.effects.iter().any(|e|e.affected==source && matches!(e.kind,EffectKind::CantBeRegenerated)));
    assert!(g.history().all().iter().any(|r|matches!(r.event,Event::AbilityActivated {source:s,player:A,..} if s==source)));
}
