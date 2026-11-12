#[test]
fn generated_mana_short_taps_the_targets_lands_and_empties_only_their_mana() {
    let mut g=setup(&[&mana_short::CARD]);let land=put(&mut g,&BASIC,B,Zone::BATTLEFIELD);let other=put(&mut g,&BEAR,B,Zone::BATTLEFIELD);
    let spell=put(&mut g,&mana_short::CARD,A,Zone::hand(A));
    announce(&mut g,spell,ActionKind::CastSpell,0,vec![Target::Player(B)]);finish(&mut g);
    assert!(g.state.obj(land).tapped);assert!(!g.state.obj(other).tapped);
    assert!(g.state.player(B).pool.is_empty());assert_eq!(g.state.mana_from_of(B).count(),0);
    assert!(g.state.player(A).pool.total()>0);
}
#[test]
fn generated_power_sink_only_empties_mana_and_taps_mana_lands_when_its_payment_is_declined() {
    for paid in [false,true] {
        let mut g=setup(&[&power_sink::CARD]);let land=put(&mut g,&BASIC,A,Zone::BATTLEFIELD);let barren=put(&mut g,&FILLER,A,Zone::BATTLEFIELD);
        let original=put(&mut g,&SUPPLY,A,Zone::hand(A));let sink=put(&mut g,&power_sink::CARD,A,Zone::hand(A));
        announce(&mut g,original,ActionKind::CastSpell,0,vec![]);
        for _ in 0..50 {
            g.run();let r=g.pending().cloned().unwrap();
            if matches!(r.kind,RequestKind::Priority {..}) {break;}
            let a=default_answer(&g,&r);g.answer(r.id,r.player,a).unwrap();
        }
        assert_eq!(g.state.stack.len(),1);let spell=current(&g,original);
        announce(&mut g,sink,ActionKind::CastSpell,0,vec![Target::Object(spell)]);
        finish_with(&mut g,&mut |_,r|match &r.kind {
            RequestKind::PayCosts {announcing:true,..}=>Some(Answer::Announce(1)),
            RequestKind::UnlessPays {options,..}=>{
                let kind=if paid {ActionKind::PayCost}else{ActionKind::DeclineCost};
                Some(Answer::Act(Action::of(options.iter().find(|o|o.kind==kind).unwrap())))
            },
            _=>None,
        });
        assert_eq!(g.state.obj(land).tapped,!paid);assert!(!g.state.obj(barren).tapped);
        assert_eq!(g.state.player(A).pool.is_empty(),!paid);
        assert!(g.state.obj(current(&g,original)).zone.is(ZoneKind::Graveyard));
    }
}
