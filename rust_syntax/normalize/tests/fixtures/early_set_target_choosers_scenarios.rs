fn chooser_target(g:&mut RulesGame,player:PlayerId,target:Target){
 g.run();let r=g.pending().cloned().expect("target choice");assert_eq!(r.player,player);assert!(matches!(r.kind,RequestKind::ChooseTarget{..}),"{:?}",r.kind);g.answer(r.id,r.player,Answer::Targets(vec![target])).unwrap();
}
#[test]
fn generated_cuombajj_witches_gives_each_player_their_own_target_slot(){
 let mut g=setup(&[&cuombajj_witches::CARD]);let source=put(&mut g,&cuombajj_witches::CARD,A,Zone::BATTLEFIELD);
 hand_announce(&mut g,source,ActionKind::ActivateAbility,0,vec![]);chooser_target(&mut g,A,Target::Player(B));chooser_target(&mut g,B,Target::Player(A));finish(&mut g);
 assert_eq!((g.state.player(A).life,g.state.player(B).life),(19,19));assert!(g.state.obj(source).tapped);
}
#[test]
fn generated_preacher_uses_the_opponents_creature_then_control_expires_on_untap(){
 let mut g=setup(&[&preacher::CARD]);let source=put(&mut g,&preacher::CARD,A,Zone::BATTLEFIELD);let creature=put(&mut g,&BEAR,B,Zone::BATTLEFIELD);
 hand_announce(&mut g,source,ActionKind::ActivateAbility,0,vec![]);chooser_target(&mut g,B,Target::Object(creature));finish(&mut g);assert_eq!(g.state.obj(creature).controller,A);
 let untap=put(&mut g,&UNTAP,A,Zone::hand(A));hand_announce(&mut g,untap,ActionKind::CastSpell,0,vec![Target::Object(source)]);finish(&mut g);assert_eq!(g.state.obj(creature).controller,B);
}
