fn group_goad_effect(g:&mut dyn Game,item:&StackItem)->Result<(),Suspend>{
    if let Some(id)=cantrip_mechanics::targets::object(g,item,0){g.add_effect(item.object,id,EffectKind::Goaded,Duration::EndOfTurn);}Ok(())
}
static GROUP_GOAD:CardDef=CardDef{name:"Combat group goad fixture",spell:spell(PERMANENT,group_goad_effect),..BOUNCE};
fn make_group_goaded(g:&mut RulesGame,ids:&[ObjectId]){
    for &id in ids {let spell=put(g,&GROUP_GOAD,A,Zone::hand(A));hand_announce(g,spell,ActionKind::CastSpell,0,vec![Target::Object(id)]);finish(g);}
}
fn group_attack_request(g:&mut RulesGame)->Request {
    advance_to(g,|g|g.state.turn.step()==Step::DeclareAttackers);
    g.pending().cloned().expect("attack declaration")
}
#[test]
fn generated_caverns_caps_whole_attacks_and_blocks_with_requirement_maximization(){
    let mut g=setup(&[&caverns_of_despair::CARD,&GROUP_GOAD]);
    let attackers:Vec<_>=(0..3).map(|_|put(&mut g,&BEAR,A,Zone::BATTLEFIELD)).collect();
    let blockers:Vec<_>=(0..3).map(|_|put(&mut g,&BEAR,B,Zone::BATTLEFIELD)).collect();
    let caverns=put(&mut g,&caverns_of_despair::CARD,A,Zone::hand(A));
    hand_announce(&mut g,caverns,ActionKind::CastSpell,0,vec![]);finish(&mut g);
    make_group_goaded(&mut g,&attackers);
    let r=group_attack_request(&mut g);
    let RequestKind::DeclareAttackers{eligible,players,requirements,costs,..}=&r.kind else{panic!("{:?}",r.kind)};
    let targets:Vec<_>=players.iter().copied().map(Target::Player).collect();
    let attacks:Vec<_>=attackers.iter().copied().map(|a|(a,Target::Player(B))).collect();
    assert_eq!(requirements.most,4);assert!(!requirements.judge(eligible,&targets,costs,&attacks,false).complete);
    assert!(requirements.judge(eligible,&targets,costs,&attacks[..2],false).complete);
    g.answer(r.id,r.player,Answer::Attack(attacks[..2].to_vec())).unwrap();
    advance_to(&mut g,|g|matches!(g.pending().map(|r|&r.kind),Some(RequestKind::DeclareBlockers{..})));
    let r=g.pending().cloned().unwrap();let RequestKind::DeclareBlockers{allowed,requirements,..}=&r.kind else{panic!()};
    let blocks:Vec<_>=blockers.iter().copied().map(|b|(b,attackers[0])).collect();
    assert!(!requirements.judge(allowed,&blocks,false).complete);assert!(requirements.judge(allowed,&blocks[..2],false).complete);
    g.answer(r.id,r.player,Answer::Block(blocks[..2].to_vec())).unwrap();
}
#[test]
fn generated_errantry_loses_to_two_other_required_attackers(){
    let mut g=setup(&[&errantry::CARD,&GROUP_GOAD]);let creatures:Vec<_>=(0..3).map(|_|put(&mut g,&BEAR,A,Zone::BATTLEFIELD)).collect();
    let aura=put(&mut g,&errantry::CARD,A,Zone::hand(A));hand_announce(&mut g,aura,ActionKind::CastSpell,0,vec![Target::Object(creatures[0])]);finish(&mut g);
    make_group_goaded(&mut g,&creatures);
    let r=group_attack_request(&mut g);let RequestKind::DeclareAttackers{eligible,players,requirements,costs,..}=&r.kind else{panic!()};
    let targets:Vec<_>=players.iter().copied().map(Target::Player).collect();
    assert_eq!(requirements.most,4);
    assert!(!requirements.judge(eligible,&targets,costs,&[(creatures[0],Target::Player(B))],false).complete);
    let attacks=vec![(creatures[1],Target::Player(B)),(creatures[2],Target::Player(B))];
    assert!(requirements.judge(eligible,&targets,costs,&attacks,false).complete);g.answer(r.id,r.player,Answer::Attack(attacks)).unwrap();
}
#[test]
fn generated_conscripts_requires_two_other_distinct_attackers(){
    let mut g=setup(&[&orcish_conscripts::CARD,&GROUP_GOAD]);let conscript=put(&mut g,&orcish_conscripts::CARD,A,Zone::hand(A));
    let peers:Vec<_>=(0..2).map(|_|put(&mut g,&BEAR,A,Zone::BATTLEFIELD)).collect();
    hand_announce(&mut g,conscript,ActionKind::CastSpell,0,vec![]);finish(&mut g);let conscript=current(&g,conscript);
    advance_to(&mut g,|g|g.state.turn.number==3&&g.state.turn.active==A&&g.state.turn.step()==Step::PrecombatMain);
    make_group_goaded(&mut g,&[conscript]);
    let r=group_attack_request(&mut g);let RequestKind::DeclareAttackers{eligible,players,requirements,costs,..}=&r.kind else{panic!()};
    let targets:Vec<_>=players.iter().copied().map(Target::Player).collect();let mut attacks=vec![(conscript,Target::Player(B))];
    assert_eq!(requirements.most,2);assert!(!requirements.judge(eligible,&targets,costs,&attacks,false).complete);
    attacks.extend(peers.iter().copied().map(|a|(a,Target::Player(B))));assert!(requirements.judge(eligible,&targets,costs,&attacks,false).complete);
    g.answer(r.id,r.player,Answer::Attack(attacks)).unwrap();
}
