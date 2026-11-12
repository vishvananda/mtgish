
#[test]
fn generated_great_wall_permits_a_block_without_removing_plainswalk() {
    static WALKER:CardDef=CardDef { name:"Foundation plainswalker",keywords:Keywords::of(&[Keyword::Plainswalk,Keyword::Haste]),..BEAR };
    for exception in [false,true] {
        let mut g=setup(&[&great_wall::CARD,&WALKER]);let walker=put(&mut g,&WALKER,A,Zone::BATTLEFIELD);
        let blocker=put(&mut g,&BEAR,B,Zone::BATTLEFIELD);put(&mut g,&BASIC,B,Zone::BATTLEFIELD);
        if exception {put(&mut g,&great_wall::CARD,B,Zone::BATTLEFIELD);}
        let prime=put(&mut g,&SUPPLY,A,Zone::hand(A));announce(&mut g,prime,ActionKind::CastSpell,0,vec![]);finish(&mut g);
        let mut offered=None;
        for _ in 0..100 {
            g.run();let r=g.pending().cloned().unwrap();
            match &r.kind {
                RequestKind::DeclareBlockers {allowed,..}=>{offered=Some(allowed.contains(&(blocker,walker)));break;},
                RequestKind::Priority {..} if g.state.turn.step()==Step::DeclareBlockers=>{offered=Some(false);break;},
                _=>{},
            }
            let answer=if matches!(r.kind,RequestKind::DeclareAttackers {..}) {Answer::Attack(vec![(walker,Target::Player(B))])} else {default_answer(&g,&r)};
            g.answer(r.id,r.player,answer).unwrap();
        }
        assert_eq!(offered,Some(exception));assert!(g.chars(walker).keywords.has(Keyword::Plainswalk));
    }
}
#[test]
fn generated_world_cards_use_the_world_state_based_action() {
    let mut g=setup(&[&arboria::CARD,&concordant_crossroads::CARD]);
    let old=put(&mut g,&arboria::CARD,A,Zone::hand(A));let new=put(&mut g,&concordant_crossroads::CARD,A,Zone::hand(A));
    announce(&mut g,old,ActionKind::CastSpell,0,vec![]);finish(&mut g);
    let old=current(&g,old);assert!(g.chars(old).supertypes.world);
    announce(&mut g,new,ActionKind::CastSpell,0,vec![]);finish(&mut g);
    assert!(g.state.obj(current(&g,old)).zone.is(ZoneKind::Graveyard));
    assert!(g.state.obj(current(&g,new)).zone.is(ZoneKind::Battlefield));
}
#[test]
fn generated_gaze_of_pain_replaces_combat_assignment_with_its_chosen_noncombat_damage() {
    static ATTACKER:CardDef=CardDef {name:"Foundation attacker",power:Some(3),toughness:Some(6),keywords:Keywords::of(&[Keyword::Haste]),..BEAR};
    static VICTIM:CardDef=CardDef {name:"Foundation victim",toughness:Some(6),..BEAR};
    let mut g=setup(&[&gaze_of_pain::CARD,&ATTACKER,&VICTIM]);let attacker=put(&mut g,&ATTACKER,A,Zone::BATTLEFIELD);
    let victim=put(&mut g,&VICTIM,B,Zone::BATTLEFIELD);let spell=put(&mut g,&gaze_of_pain::CARD,A,Zone::hand(A));
    announce(&mut g,spell,ActionKind::CastSpell,0,vec![]);finish(&mut g);
    for _ in 0..150 {
        g.run();let r=g.pending().cloned().unwrap();
        if g.state.turn.step()==Step::EndOfCombat && matches!(r.kind,RequestKind::Priority {..}) {break;}
        let answer=match &r.kind {
            RequestKind::DeclareAttackers {..}=>Answer::Attack(vec![(attacker,Target::Player(B))]),
            RequestKind::ChooseTarget {candidates,..}=>{assert!(candidates.contains(&Target::Object(victim)));Answer::Targets(vec![Target::Object(victim)])},
            _=>default_answer(&g,&r),
        };
        g.answer(r.id,r.player,answer).unwrap();
    }
    assert_eq!(g.state.turn.step(),Step::EndOfCombat);
    assert_eq!(g.state.obj(victim).damage,3);assert_eq!(g.state.player(B).life,20);
    assert!(g.history().all().iter().any(|r|matches!(r.event,Event::Damage {source,kind,amount:3,..} if source==attacker && !kind.is_combat())));
    assert!(!g.history().all().iter().any(|r|matches!(r.event,Event::Damage {source,kind,..} if source==attacker && kind.is_combat())));
}
#[test]
fn generated_island_of_wak_wak_sets_power_without_freezing_toughness() {
    fn pump(g:&mut dyn Game,item:&StackItem)->Result<(),Suspend> {
        if let Some(object)=cantrip_mechanics::targets::object(g,item,0) {g.add_effect(item.object,object,EffectKind::Pump(2,1),Duration::EndOfTurn);} Ok(())
    }
    static PUMP:CardDef=CardDef {name:"Foundation pump",spell:spell(PERMANENT,pump),..BOUNCE};
    static FLIER:CardDef=CardDef {name:"Foundation flier",power:Some(3),toughness:Some(6),keywords:Keywords::of(&[Keyword::Flying]),..BEAR};
    let mut g=setup(&[&island_of_wak_wak::CARD,&FLIER,&PUMP]);
    let land=put(&mut g,&island_of_wak_wak::CARD,A,Zone::BATTLEFIELD);let flier=put(&mut g,&FLIER,B,Zone::BATTLEFIELD);
    let pump=put(&mut g,&PUMP,A,Zone::hand(A));
    announce(&mut g,land,ActionKind::ActivateAbility,0,vec![Target::Object(flier)]);finish(&mut g);
    assert_eq!((g.chars(flier).power,g.chars(flier).toughness),(Some(0),Some(6)));
    announce(&mut g,pump,ActionKind::CastSpell,0,vec![Target::Object(flier)]);finish(&mut g);
    assert_eq!((g.chars(flier).power,g.chars(flier).toughness),(Some(2),Some(7)));
}
