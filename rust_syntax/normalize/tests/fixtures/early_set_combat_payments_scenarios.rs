static COMBAT_GREEN: CardDef = CardDef { name: "Combat payment green creature", colors: Some(ColorSet::of(Color::Green)), ..BEAR };
static COMBAT_BLACK: CardDef = CardDef { name: "Combat payment black creature", colors: Some(ColorSet::of(Color::Black)), ..BEAR };
static COMBAT_ISLAND: CardDef = CardDef { name: "Combat payment basic Island", subtypes: SubtypeSet::of(&[Subtype::Island]), ..BASIC };
fn sacrifice_attack_cost(g:&mut RulesGame, ids:&[ObjectId]) {
    g.run();let r=g.pending().cloned().expect("attack cost");
    let RequestKind::ChooseCards {purpose:Purpose::SacrificeCost,ref cards,ref rule,..}=r.kind else {panic!("{:?}",r.kind)};
    assert_eq!(rule.count,Count::Exactly(ids.len() as u32));assert!(ids.iter().all(|id|cards.contains(id)));
    g.answer(r.id,r.player,Answer::Cards(ids.to_vec())).unwrap();
}
fn generated_colored_land_tax(card:&'static CardDef,creature:&'static CardDef) {
    let mut g=setup(&[card,creature]);let creatures=[put(&mut g,creature,A,Zone::BATTLEFIELD),put(&mut g,creature,A,Zone::BATTLEFIELD)];
    let free=put(&mut g,&BEAR,A,Zone::BATTLEFIELD);let lands=[put(&mut g,&BASIC,A,Zone::BATTLEFIELD),put(&mut g,&BASIC,A,Zone::BATTLEFIELD)];
    let tax=put(&mut g,card,A,Zone::hand(A));hand_announce(&mut g,tax,ActionKind::CastSpell,0,vec![]);finish(&mut g);
    let r=group_attack_request(&mut g);let RequestKind::DeclareAttackers{ref costs,..}=r.kind else{panic!()};
    for &id in &creatures {assert!(!cantrip_state::AttackCost::free(costs,id,Target::Player(B)));}
    assert!(cantrip_state::AttackCost::free(costs,free,Target::Player(B)));
    let attacks=creatures.into_iter().chain([free]).map(|id|(id,Target::Player(B))).collect();
    g.answer(r.id,r.player,Answer::Attack(attacks)).unwrap();
    sacrifice_attack_cost(&mut g,&lands[..1]);sacrifice_attack_cost(&mut g,&lands[1..]);g.run();
    assert_eq!(g.state.attackers.len(),3);assert!(lands.iter().all(|&id|g.state.obj(id).became.is_some()));
}
#[test]
fn generated_flooded_woodlands_charges_each_green_attacker_only(){generated_colored_land_tax(&flooded_woodlands::CARD,&COMBAT_GREEN);}
#[test]
fn generated_reclamation_charges_each_black_attacker_only(){generated_colored_land_tax(&reclamation::CARD,&COMBAT_BLACK);}
#[test]
fn generated_leviathan_attacks_by_sacrificing_two_islands(){
    let mut g=setup(&[&leviathan::CARD,&COMBAT_ISLAND]);let islands=[put(&mut g,&COMBAT_ISLAND,A,Zone::BATTLEFIELD),put(&mut g,&COMBAT_ISLAND,A,Zone::BATTLEFIELD)];
    let forest=put(&mut g,&DUAL,A,Zone::BATTLEFIELD);let beast=put(&mut g,&leviathan::CARD,A,Zone::BATTLEFIELD);
    // Deliberate established position: the creature was already controlled
    // before this turn and is untapped. Its actual generated attack rule runs.
    g.state.update_obj(beast,|o|o.controlled_since=0);g.state.see_statics();
    let r=group_attack_request(&mut g);g.answer(r.id,r.player,Answer::Attack(vec![(beast,Target::Player(B))])).unwrap();
    g.run();let r=g.pending().cloned().unwrap();let RequestKind::ChooseCards{ref cards,..}=r.kind else{panic!("{:?}",r.kind)};assert!(!cards.contains(&forest));
    g.answer(r.id,r.player,Answer::Cards(islands.to_vec())).unwrap();g.run();assert_eq!(g.state.attackers.len(),1);assert_eq!(g.state.attackers[0].id,beast);
}
