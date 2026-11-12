fn begin_fixture_untap(g:&mut RulesGame) {
    while !g.state.agenda.is_empty(){g.state.frame_remove(0);}
    g.state.frame_push(cantrip_state::Frame::new(cantrip_state::FrameKind::BeginStep));
    g.state.update_turn(|t|{t.number=2;t.active=A;t.priority=A;t.set_step(Step::Untap);t.began=false;});g.state.see_statics();g.run();
}
#[test]
fn generated_smoke_damping_field_and_winter_orb_limit_only_their_permanents() {
    static ART:CardDef=CardDef {name:"Untap quota artifact",types:TypeSet::of(&[CardType::Artifact]),..CardDef::BLANK};
    for (rule,matching,unaffected) in [(&smoke::CARD,&BEAR,&BASIC),(&damping_field::CARD,&ART,&BEAR),(&winter_orb::CARD,&BASIC,&BEAR)] {
        let mut g=setup(&[rule,&ART]);put(&mut g,rule,B,Zone::BATTLEFIELD);
        let one=put(&mut g,matching,A,Zone::BATTLEFIELD);let two=put(&mut g,matching,A,Zone::BATTLEFIELD);let free=put(&mut g,unaffected,A,Zone::BATTLEFIELD);
        for id in [one,two,free]{g.state.update_obj(id,|o|o.tapped=true);}
        begin_fixture_untap(&mut g);
        let request=g.pending().cloned().unwrap();let RequestKind::ChooseCards {purpose:Purpose::Untap,cards,..}=&request.kind else {panic!("{:?}",request.kind)};
        assert_eq!(cards,&vec![one,two]);assert!(g.answer(request.id,A,Answer::Cards(vec![one,two])).is_err());
        g.answer(request.id,A,Answer::Cards(vec![two])).unwrap();finish(&mut g);
        assert!(g.state.obj(one).tapped);assert!(!g.state.obj(two).tapped&&!g.state.obj(free).tapped);
    }
}
#[test]
fn generated_smoke_and_winter_orb_share_one_intersected_choice() {
    static ANIMATED:CardDef=CardDef {name:"Untap quota animated land",types:TypeSet::of(&[CardType::Land,CardType::Creature]),..BEAR};
    for choose_animated in [false,true] {
        let mut g=setup(&[&smoke::CARD,&winter_orb::CARD,&ANIMATED]);put(&mut g,&smoke::CARD,B,Zone::BATTLEFIELD);put(&mut g,&winter_orb::CARD,B,Zone::BATTLEFIELD);
        let land=put(&mut g,&BASIC,A,Zone::BATTLEFIELD);let bear=put(&mut g,&BEAR,A,Zone::BATTLEFIELD);let dual=put(&mut g,&ANIMATED,A,Zone::BATTLEFIELD);
        for id in [land,bear,dual]{g.state.update_obj(id,|o|o.tapped=true);}
        begin_fixture_untap(&mut g);let request=g.pending().cloned().unwrap();assert!(matches!(request.kind,RequestKind::ChooseCards {purpose:Purpose::Untap,..}));
        g.answer(request.id,A,Answer::Cards(if choose_animated{vec![dual]}else{vec![land,bear]})).unwrap();finish(&mut g);
        assert_eq!(g.state.obj(dual).tapped,!choose_animated);assert_eq!(g.state.obj(land).tapped,choose_animated);assert_eq!(g.state.obj(bear).tapped,choose_animated);
    }
}
