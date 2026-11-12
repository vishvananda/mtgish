#[test]
fn generated_aura_costs_tap_the_named_land_even_when_it_is_a_summoning_sick_creature() {
    static ANIMATED:CardDef=CardDef {name:"Attached cost animated land",types:TypeSet::of(&[CardType::Land,CardType::Creature]),power:Some(2),toughness:Some(2),..BASIC};
    for (def,index,blocking) in [(&earthlore::CARD,0,true),(&tourach_s_gate::CARD,2,false)] {
        let mut g=setup(&[def,&ANIMATED]);
        let land=put(&mut g,&ANIMATED,A,Zone::BATTLEFIELD);let other=put(&mut g,&BASIC,A,Zone::BATTLEFIELD);
        g.state.update_obj(land,|o|o.controlled_since=1);
        assert!(g.chars(land).is(CardType::Creature));
        assert!(g.state.obj(land).controlled_since>=g.state.player(A).turn_began);
        let aura=put(&mut g,def,A,Zone::BATTLEFIELD);g.state.update_obj(aura,|o|o.attached_to=Some(land));
        let target=put(&mut g,&BEAR,A,Zone::BATTLEFIELD);
        if blocking {
            let attacker=put(&mut g,&BEAR,B,Zone::BATTLEFIELD);
            g.state.set_attackers(vec![Attacker::new(attacker,Target::Player(A),A,TypeSet::NONE)]);
            g.state.set_blocks(vec![Block {attacker,blocker:target,first_strike:false}]);
        } else {g.state.set_attackers(vec![Attacker::new(target,Target::Player(B),B,TypeSet::NONE)]);}
        g.state.update_turn(|t|{t.active=if blocking {B}else{A};t.set_step(Step::DeclareBlockers);});
        announce(&mut g,aura,ActionKind::ActivateAbility,index,if blocking {vec![Target::Object(target)]}else{vec![]});finish(&mut g);
        assert!(g.state.obj(land).tapped);assert!(!g.state.obj(other).tapped);assert!(!g.state.obj(aura).tapped);
        assert_eq!((g.chars(target).power,g.chars(target).toughness),if blocking {(Some(3),Some(4))}else{(Some(4),Some(1))});
        let RequestKind::Priority {options}=&g.pending().unwrap().kind else {panic!("priority")};
        assert!(!options.iter().any(|o|o.source==Some(aura)&&o.kind==ActionKind::ActivateAbility&&o.index==index),"a tapped host cannot pay again");
    }
}
#[test]
fn generated_controlled_land_auras_fall_off_after_the_host_changes_controller() {
    for def in [&earthlore::CARD,&tourach_s_gate::CARD] {
        let mut g=setup(&[def]);let land=put(&mut g,&BASIC,A,Zone::BATTLEFIELD);
        let aura=put(&mut g,def,A,Zone::hand(A));
        let steal=put(&mut g,&CONTROL,A,Zone::hand(A));
        announce(&mut g,aura,ActionKind::CastSpell,0,vec![Target::Object(land)]);finish(&mut g);
        let aura=current(&g,aura);assert_eq!(g.attached_to(aura),Some(land));
        announce(&mut g,steal,ActionKind::CastSpell,0,vec![Target::Object(land)]);finish(&mut g);
        assert_eq!(g.controller(land),B);assert!(g.state.obj(current(&g,aura)).zone.is(ZoneKind::Graveyard));
    }
}
