#[test]
fn generated_cursed_rack_limits_opponents_at_cleanup_without_limiting_its_controller() {
    let mut g=setup(&[&cursed_rack::CARD]);let rack=put(&mut g,&cursed_rack::CARD,A,Zone::hand(A));
    announce(&mut g,rack,ActionKind::CastSpell,0,vec![]);finish(&mut g);
    // Casting selects B as the chosen opponent; a raw battlefield insertion would have no choice.
    put(&mut g,&BOUNCE,A,Zone::hand(A));put(&mut g,&BOUNCE,B,Zone::hand(B));
    for _ in 0..7 {put(&mut g,&FILLER,A,Zone::hand(A));}for _ in 0..9 {put(&mut g,&FILLER,B,Zone::hand(B));}g.state.see_statics();
    for (player,limit) in [(A,7),(B,4)] {
        advance_to(&mut g,|g|g.pending().is_some_and(|r|r.player==player&&matches!(r.kind,RequestKind::DiscardToHandSize {..})));
        let r=g.pending().cloned().unwrap();let RequestKind::DiscardToHandSize {count,hand}=&r.kind else {panic!()};
        assert_eq!(*count as usize,hand.len()-limit);let answer=default_answer(&g,&r);g.answer(r.id,r.player,answer).unwrap();g.run();
        assert_eq!(g.state.zone(Zone::hand(player)).len(),limit);
    }
}
#[test]
fn generated_shapeshifter_reselects_its_number_without_reentering_or_losing_its_original_link() {
    let mut g=setup(&[&shapeshifter::CARD]);let spell=put(&mut g,&shapeshifter::CARD,A,Zone::hand(A));
    put(&mut g,&BOUNCE,A,Zone::hand(A));put(&mut g,&BOUNCE,B,Zone::hand(B));
    announce(&mut g,spell,ActionKind::CastSpell,0,vec![]);
    finish_with(&mut g,&mut |_,r|matches!(r.kind,RequestKind::ChooseNumber {..}).then_some(Answer::Announce(3)));
    let creature=current(&g,spell);assert_eq!((g.chars(creature).power,g.chars(creature).toughness),(Some(3),Some(4)));
    advance_to(&mut g,|g|g.state.turn.active==A&&g.state.turn.step()==Step::Upkeep&&g.state.turn.number==3);
    finish_with(&mut g,&mut |_,r|matches!(r.kind,RequestKind::ChooseNumber {..}).then_some(Answer::Announce(2)));
    assert_eq!((g.chars(creature).power,g.chars(creature).toughness),(Some(2),Some(5)));
    advance_to(&mut g,|g|g.state.turn.active==A&&g.state.turn.step()==Step::Upkeep&&g.state.turn.number==5);
    finish_with(&mut g,&mut |_,r|matches!(r.kind,RequestKind::May {..}).then_some(Answer::No));
    assert_eq!(current(&g,spell),creature);assert_eq!((g.chars(creature).power,g.chars(creature).toughness),(Some(2),Some(5)));
}
