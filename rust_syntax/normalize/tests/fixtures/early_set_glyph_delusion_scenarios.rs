#[test]
fn generated_glyph_delusion_keeps_its_two_granted_abilities_after_the_casting_turn() {
    static WALL:CardDef=CardDef {name:"Glyph fixture Wall",subtypes:SubtypeSet::of(&[Subtype::Wall]),power:Some(0),toughness:Some(6),..BEAR};
    static BLUE:CardDef=CardDef {name:"Glyph fixture blue land",abilities:&[mana_ability(TAP,add(Mana::U),"{T}: Add {U}.")],..FILLER};
    let mut g=setup(&[&glyph_of_delusion::CARD,&WALL,&BLUE]);let creature=put(&mut g,&BEAR,B,Zone::BATTLEFIELD);let wall=put(&mut g,&WALL,A,Zone::BATTLEFIELD);
    let glyph=put(&mut g,&glyph_of_delusion::CARD,A,Zone::hand(A));let blue=put(&mut g,&BLUE,A,Zone::BATTLEFIELD);
    // Both seats retain a legal response so the probe can inspect each exact boundary.
    put(&mut g,&BOUNCE,A,Zone::hand(A));put(&mut g,&BOUNCE,B,Zone::hand(B));
    g.state.update_turn(|t|{t.number=2;t.active=B;t.priority=B;});g.state.update_player(B,|p|p.turn_began=2);
    for _ in 0..100 {
        g.run();let r=g.pending().cloned().unwrap();
        if g.state.turn.step()==Step::DeclareBlockers && r.player==A && matches!(r.kind,RequestKind::Priority {..}) {break;}
        let answer=match &r.kind {RequestKind::DeclareAttackers {..}=>Answer::Attack(vec![(creature,Target::Player(A))]),RequestKind::DeclareBlockers {..}=>Answer::Block(vec![(wall,creature)]),_=>default_answer(&g,&r)};
        g.answer(r.id,r.player,answer).unwrap();
    }
    assert!(g.is_blocking(wall));announce(&mut g,blue,ActionKind::ActivateAbility,0,vec![]);finish(&mut g);
    assert!(g.state.player(A).pool.get(Mana::U)>0);
    assert!(g.history().this_turn().iter().any(|r| matches!(r.event,Event::Blocks {blocker,attacker,..} if blocker==wall&&attacker==creature)),"block history {:?}",g.history().this_turn());
    announce(&mut g,glyph,ActionKind::CastSpell,0,vec![]);finish(&mut g);
    assert_eq!(g.state.obj(creature).counters(CounterKind::Glyph),2);assert!(g.chars(creature).has(Keyword::DoesNotUntap));
    for (turn,before,after) in [(4,2,1),(6,1,0)] {
        advance_to(&mut g,|g|g.state.turn.active==B&&g.state.turn.step()==Step::Upkeep&&g.state.turn.number==turn);
        assert!(g.state.obj(creature).tapped);assert_eq!(g.state.obj(creature).counters(CounterKind::Glyph),before);
        finish(&mut g);assert_eq!(g.state.obj(creature).counters(CounterKind::Glyph),after);
    }
    assert!(!g.chars(creature).has(Keyword::DoesNotUntap));
    advance_to(&mut g,|g|g.state.turn.active==B&&g.state.turn.step()==Step::Upkeep&&g.state.turn.number==8);
    assert!(!g.state.obj(creature).tapped);
}
