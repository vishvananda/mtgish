// Test positions inserted between actions must precede the next frame's mark:
// a payment question may replay that whole frame.
fn hand_announce(g:&mut RulesGame,id:ObjectId,kind:ActionKind,index:u8,targets:Vec<Target>){
    if g.pending().is_some_and(|r|matches!(r.kind,RequestKind::Priority{..})) {
        let at=g.state.agenda.len()-1;g.state.frame_remove(at);
        g.state.frame_push(cantrip_state::Frame::new(cantrip_state::FrameKind::Priority));
    }
    announce(g,id,kind,index,targets);
}
fn hand_draw(g:&mut dyn Game,_:&StackItem)->Result<(),Suspend>{g.draw(A,1);Ok(())}
static HAND_DRAW:CardDef=CardDef{name:"Public hand fixture draw",spell:spell(&[],hand_draw),..BOUNCE};
fn hand_top(g:&mut RulesGame,def:&'static CardDef)->ObjectId{
    let id=put(g,def,A,Zone::library(A));g.state.zone_remove(Zone::library(A),id);g.state.zone_insert(Zone::library(A),0,id);id
}
#[test]
fn generated_revelation_opens_both_hands_only_while_it_remains_in_play(){
    let mut g=setup(&[&revelation::CARD]);put(&mut g,&BEAR,B,Zone::hand(B));
    let source=put(&mut g,&revelation::CARD,A,Zone::hand(A));let bounce=put(&mut g,&BOUNCE,A,Zone::hand(A));
    assert_eq!(g.revealed_hands(),0);hand_announce(&mut g,source,ActionKind::CastSpell,0,vec![]);finish(&mut g);
    let source=current(&g,source);assert_eq!(g.revealed_hands(),3);
    assert!(g.history().all().iter().any(|r|matches!(r.event,Event::Shown{player:A,..})));
    hand_announce(&mut g,bounce,ActionKind::CastSpell,0,vec![Target::Object(source)]);finish(&mut g);
    assert_eq!(g.revealed_hands(),0);
}
#[test]
fn generated_enduring_renewal_opens_only_its_controllers_hand_and_keeps_its_draw_replacement(){
    let mut g=setup(&[&enduring_renewal::CARD,&HAND_DRAW]);
    let source=put(&mut g,&enduring_renewal::CARD,A,Zone::hand(A));let draw=put(&mut g,&HAND_DRAW,A,Zone::hand(A));let control=put(&mut g,&CONTROL,A,Zone::hand(A));
    hand_announce(&mut g,source,ActionKind::CastSpell,0,vec![]);finish(&mut g);
    let source=current(&g,source);assert_eq!(g.revealed_hands(),1);
    let bear=hand_top(&mut g,&BEAR);
    hand_announce(&mut g,draw,ActionKind::CastSpell,0,vec![]);finish(&mut g);
    assert!(g.state.obj(current(&g,bear)).zone.is(ZoneKind::Graveyard));
    hand_announce(&mut g,control,ActionKind::CastSpell,0,vec![Target::Object(source)]);finish(&mut g);
    assert_eq!(g.state.obj(source).controller,B);assert_eq!(g.revealed_hands(),2);
}
#[test]
fn generated_zurs_weirding_opens_hands_and_keeps_both_payment_outcomes(){
    for pay in [false,true]{
        let mut g=setup(&[&zur_s_weirding::CARD,&HAND_DRAW]);
        let source=put(&mut g,&zur_s_weirding::CARD,A,Zone::hand(A));let draw=put(&mut g,&HAND_DRAW,A,Zone::hand(A));hand_announce(&mut g,source,ActionKind::CastSpell,0,vec![]);finish(&mut g);assert_eq!(g.revealed_hands(),3);
        let card=hand_top(&mut g,&BASIC);let life=g.state.player(B).life;
        hand_announce(&mut g,draw,ActionKind::CastSpell,0,vec![]);
        finish_with(&mut g,&mut |_,r|match &r.kind{
            RequestKind::May{..}=>{assert_eq!(r.player,B);Some(if pay{Answer::Yes}else{Answer::No})},
            RequestKind::PayCosts{options,announcing:false,..}|RequestKind::UnlessPays{options,..}=>options.iter().find(|o|o.kind==if pay{ActionKind::PayCost}else{ActionKind::DeclineCost}).map(|o|Answer::Act(Action::of(o))),
            _=>None
        });
        assert_eq!(g.state.player(B).life,life-if pay{2}else{0});
        assert_eq!(g.state.obj(current(&g,card)).zone,if pay{Zone::graveyard(A)}else{Zone::hand(A)});
    }
}
