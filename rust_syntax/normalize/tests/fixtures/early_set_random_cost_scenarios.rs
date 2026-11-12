#[test]
fn generated_random_discard_cards_pay_without_a_card_choice_then_apply_their_effects() {
    for (index,def) in [&coral_helm::CARD,&draconian_cylix::CARD,&stormbind::CARD].into_iter().enumerate() {
        let mut g=setup(&[def]);let source=put(&mut g,def,A,Zone::BATTLEFIELD);let creature=put(&mut g,&BEAR,B,Zone::BATTLEFIELD);
        put(&mut g,&FILLER,A,Zone::hand(A));put(&mut g,&FILLER,A,Zone::hand(A));let before=g.state.zone(Zone::hand(A)).len();
        announce(&mut g,source,ActionKind::ActivateAbility,0,vec![if index==2 {Target::Player(B)}else{Target::Object(creature)}]);
        finish_with(&mut g,&mut |_,r| {
            if matches!(r.kind,RequestKind::ChooseCards {purpose:Purpose::Discard,..}) {panic!("a random discard offered a card choice");} None
        });
        assert_eq!(g.state.zone(Zone::hand(A)).len(),before-1);
        assert!(g.history().all().iter().any(|r|matches!(r.event,Event::ChosenAtRandom {..})));
        match index {
            0=>assert_eq!((g.chars(creature).power,g.chars(creature).toughness),(Some(4),Some(4))),
            1=>{assert!(g.state.obj(source).tapped);assert!(g.state.effects.iter().any(|e|e.affected==creature && matches!(e.kind,EffectKind::RegenerationShield)));},
            _=>assert_eq!(g.state.player(B).life,18),
        }
    }
}
