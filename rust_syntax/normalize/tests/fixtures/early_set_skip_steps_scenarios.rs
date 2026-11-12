#[test]
fn generated_stasis_skips_untap_then_releases_it_after_its_upkeep_payment_is_declined() {
    let mut g=setup(&[&stasis::CARD]);let stasis=put(&mut g,&stasis::CARD,B,Zone::BATTLEFIELD);
    let own=put(&mut g,&BEAR,A,Zone::BATTLEFIELD);let theirs=put(&mut g,&BEAR,B,Zone::BATTLEFIELD);
    for id in [own,theirs] {g.state.update_obj(id,|o|o.tapped=true);}
    put(&mut g,&BOUNCE,A,Zone::hand(A));put(&mut g,&BOUNCE,B,Zone::hand(B));g.state.see_statics();
    advance_to(&mut g,|g|g.state.turn.active==B&&g.state.turn.step()==Step::Upkeep);
    assert!(g.state.obj(theirs).tapped);assert_eq!(g.state.player(B).turn_began,2);
    finish(&mut g);assert!(g.state.obj(current(&g,stasis)).zone.is(ZoneKind::Graveyard));
    advance_to(&mut g,|g|g.state.turn.active==A&&g.state.turn.step()==Step::Upkeep&&g.state.turn.number==3);
    assert!(!g.state.obj(own).tapped);assert!(g.state.obj(theirs).tapped);
}
#[test]
fn generated_necropotence_skips_its_controllers_whole_draw_step_but_not_the_opponents() {
    let mut g=setup(&[&necropotence::CARD]);put(&mut g,&necropotence::CARD,A,Zone::BATTLEFIELD);
    put(&mut g,&BOUNCE,A,Zone::hand(A));put(&mut g,&BOUNCE,B,Zone::hand(B));g.state.see_statics();
    advance_to(&mut g,|g|g.state.turn.active==B&&g.state.turn.step()==Step::Upkeep);
    let other=g.state.zone(Zone::hand(B)).len();
    advance_to(&mut g,|g|g.state.turn.active==B&&g.state.turn.step()==Step::PrecombatMain);
    assert_eq!(g.state.zone(Zone::hand(B)).len(),other+1);
    advance_to(&mut g,|g|g.state.turn.active==A&&g.state.turn.step()==Step::Upkeep&&g.state.turn.number==3);
    let own=g.state.zone(Zone::hand(A)).len();
    advance_to(&mut g,|g|g.state.turn.active==A&&g.state.turn.step()==Step::PrecombatMain&&g.state.turn.number==3);
    assert_eq!(g.state.zone(Zone::hand(A)).len(),own);
    assert!(!g.history().this_turn().iter().any(|r|matches!(r.event,Event::StepBegan {step:Step::Draw})));
}
