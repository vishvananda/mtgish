#[test]
fn generated_voodoo_doll_pays_double_the_defined_x_without_a_number_prompt() {
    fn add_pins(g:&mut dyn Game,item:&StackItem)->Result<(),Suspend> { if let Some(o)=cantrip_mechanics::targets::object(g,item,0) {g.add_counters(o,CounterKind::Pin,2);}Ok(()) }
    static PINS:CardDef=CardDef {name:"X fixture pin counters",spell:spell(PERMANENT,add_pins),..BOUNCE};
    let mut g=setup(&[&voodoo_doll::CARD,&PINS]);let doll=put(&mut g,&voodoo_doll::CARD,A,Zone::BATTLEFIELD);
    let pins=put(&mut g,&PINS,A,Zone::hand(A));announce(&mut g,pins,ActionKind::CastSpell,0,vec![Target::Object(doll)]);finish(&mut g);
    let before=g.state.player(A).pool.total();
    announce(&mut g,doll,ActionKind::ActivateAbility,2,vec![Target::Player(B)]);
    let mut observed=false;
    finish_with(&mut g,&mut |g,r| {
        assert!(!matches!(r.kind,RequestKind::PayCosts {announcing:true,..}),"printed X is not freely chosen");
        if let Some(item)=g.state.stack.last().filter(|i|i.object==doll) {assert_eq!(item.x,2);observed=true;}
        None
    });
    assert!(observed);assert_eq!(before-g.state.player(A).pool.total(),4);
    assert_eq!(g.state.player(B).life,18);assert!(g.state.obj(doll).tapped);
}
#[test]
fn generated_chromatic_armor_pays_the_old_counter_count_before_adding_a_counter() {
    let mut g=setup(&[&chromatic_armor::CARD]);let bear=put(&mut g,&BEAR,A,Zone::BATTLEFIELD);
    let armor=put(&mut g,&chromatic_armor::CARD,A,Zone::hand(A));
    announce(&mut g,armor,ActionKind::CastSpell,0,vec![Target::Object(bear)]);
    finish_with(&mut g,&mut |_,r|matches!(r.kind,RequestKind::ChooseColors {..}).then_some(Answer::Colors(vec![Color::White])));
    let armor=current(&g,armor);assert_eq!(g.state.obj(armor).counters(CounterKind::Sleight),1);
    for (count,color) in [(1,Color::Blue),(2,Color::Red)] {
        let before=g.state.player(A).pool.total();
        announce(&mut g,armor,ActionKind::ActivateAbility,0,vec![]);
        finish_with(&mut g,&mut |_,r| {
            assert!(!matches!(r.kind,RequestKind::PayCosts {announcing:true,..}));
            matches!(r.kind,RequestKind::ChooseColors {..}).then_some(Answer::Colors(vec![color]))
        });
        assert_eq!(before-g.state.player(A).pool.total(),count);
        assert_eq!(g.state.obj(armor).counters(CounterKind::Sleight),count as i32+1);
        assert_eq!(g.state.chosen_color(g.linked_to(&Asker::new(armor,A))),Some(color));
    }
}
#[test]
fn generated_spoils_counts_only_artifact_and_creature_cards_in_the_chosen_graveyard() {
    static RELIC:CardDef=CardDef {name:"X count fixture relic",types:TypeSet::of(&[CardType::Artifact,CardType::Creature]),..BEAR};
    let mut g=setup(&[&spoils_of_war::CARD,&RELIC]);let target=put(&mut g,&BEAR,A,Zone::BATTLEFIELD);
    put(&mut g,&BEAR,B,Zone::graveyard(B));put(&mut g,&RELIC,B,Zone::graveyard(B));
    put(&mut g,&FILLER,B,Zone::graveyard(B));put(&mut g,&BOUNCE,B,Zone::graveyard(B));
    for _ in 0..4 {put(&mut g,&BEAR,A,Zone::graveyard(A));}
    let spell=put(&mut g,&spoils_of_war::CARD,A,Zone::hand(A));let before=g.state.player(A).pool.total();
    announce(&mut g,spell,ActionKind::CastSpell,0,vec![]);let mut observed=false;
    finish_with(&mut g,&mut |g,r| {
        assert!(!matches!(r.kind,RequestKind::PayCosts {announcing:true,..}));
        if let Some(item)=g.state.stack.last().filter(|i|g.state.obj(i.object).card==g.cards.id_of(&spoils_of_war::CARD)) {assert_eq!(item.x,2);observed=true;}
        match &r.kind { RequestKind::ChooseDividedTargets {total,..}=>{assert_eq!(*total,2);Some(Answer::Divided(vec![(Target::Object(target),2)]))},_=>None }
    });
    assert!(observed);assert_eq!(before-g.state.player(A).pool.total(),3);
    assert_eq!(g.state.obj(target).counters(CounterKind::PlusOne),2);
}
#[test]
fn generated_winters_chill_limits_x_to_the_casters_snow_lands_and_keeps_the_delayed_destruction() {
    static SNOW:CardDef=CardDef {name:"X limit fixture snow land",supertypes:Supertypes {snow:true,..BASIC.supertypes},..BASIC};
    let mut g=setup(&[&winter_s_chill::CARD,&SNOW]);let first=put(&mut g,&BEAR,B,Zone::BATTLEFIELD);let second=put(&mut g,&BEAR,B,Zone::BATTLEFIELD);
    put(&mut g,&SNOW,A,Zone::BATTLEFIELD);put(&mut g,&BASIC,A,Zone::BATTLEFIELD);for _ in 0..3 {put(&mut g,&SNOW,B,Zone::BATTLEFIELD);}
    let spell=put(&mut g,&winter_s_chill::CARD,A,Zone::hand(A));
    g.state.update_turn(|t|{t.number=2;t.active=B;t.priority=A;t.set_step(Step::DeclareAttackers);t.began=true;});
    g.state.set_attackers(vec![cantrip_state::Attacker::new(first,Target::Player(A),A,TypeSet::of(&[CardType::Creature])),cantrip_state::Attacker::new(second,Target::Player(A),A,TypeSet::of(&[CardType::Creature]))]);
    announce(&mut g,spell,ActionKind::CastSpell,0,vec![]);let mut limited=false;
    finish_with(&mut g,&mut |_,r| match &r.kind {
        RequestKind::PayCosts {announcing:true,most,..}=>{assert_eq!(*most,1);limited=true;Some(Answer::Announce(1))},
        RequestKind::ChooseTarget {..}=>Some(Answer::Targets(vec![Target::Object(first)])),
        RequestKind::UnlessPays {options,..}|RequestKind::PayCosts {announcing:false,options,..} if r.player==B=>options.iter().find(|o|o.kind==ActionKind::DeclineCost).map(|o|Answer::Act(Action::of(o))),
        _=>None,
    });
    assert!(limited);advance_to(&mut g,|g|g.state.turn.step()==Step::PostcombatMain);
    assert!(g.state.obj(current(&g,first)).zone.is(ZoneKind::Graveyard));
    assert!(g.state.obj(second).zone.is(ZoneKind::Battlefield));
}
