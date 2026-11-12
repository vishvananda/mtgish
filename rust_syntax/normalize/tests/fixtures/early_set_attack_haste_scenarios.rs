#[test]
fn generated_instill_energy_permits_an_attack_without_enabling_tap_symbol_costs() {
    static ELF:CardDef=CardDef {name:"Attack-only fixture elf",abilities:&[mana_ability(TAP,add(Mana::G),"{T}: Add {G}.")],..BEAR};
    let mut g=setup(&[&instill_energy::CARD,&ELF]);let creature=put(&mut g,&ELF,A,Zone::BATTLEFIELD);g.state.update_obj(creature,|o|o.controlled_since=1);
    let aura=put(&mut g,&instill_energy::CARD,A,Zone::hand(A));announce(&mut g,aura,ActionKind::CastSpell,0,vec![Target::Object(creature)]);finish(&mut g);
    assert!(!g.chars(creature).has(Keyword::Haste));
    let RequestKind::Priority {options}=&g.pending().unwrap().kind else {panic!()};assert!(!options.iter().any(|o|o.source==Some(creature)&&o.kind==ActionKind::ActivateAbility));
    advance_to(&mut g,|g|g.state.turn.step()==Step::DeclareAttackers);
    let RequestKind::DeclareAttackers {eligible,..}=&g.pending().unwrap().kind else {panic!("the enchanted creature may attack")};assert!(eligible.contains(&creature));
}
#[test]
fn generated_chaos_lord_distinguishes_entering_this_turn_from_changing_control_this_turn() {
    fn take(g:&mut dyn Game,item:&StackItem)->Result<(),Suspend>{if let Some(o)=cantrip_mechanics::targets::object(g,item,0){g.add_effect(item.object,o,EffectKind::Control(item.controller),Duration::EndOfTurn);}Ok(())}
    static TAKE:CardDef=CardDef {name:"Attack-only fixture control",spell:spell(PERMANENT,take),..BOUNCE};
    for entered in [false,true] {
        let mut g=setup(&[&chaos_lord::CARD,&TAKE]);
        let lord=put(&mut g,&chaos_lord::CARD,if entered {A}else{B},if entered {Zone::hand(A)}else{Zone::BATTLEFIELD});
        let take=put(&mut g,&TAKE,A,Zone::hand(A));
        if entered {announce(&mut g,lord,ActionKind::CastSpell,0,vec![]);}else{announce(&mut g,take,ActionKind::CastSpell,0,vec![Target::Object(lord)]);}
        finish(&mut g);let lord=current(&g,lord);assert_eq!(g.controller(lord),A);assert!(!g.chars(lord).has(Keyword::Haste));
        advance_to(&mut g,|g|g.state.turn.step()==Step::DeclareAttackers);
        let offered=match &g.pending().unwrap().kind{RequestKind::DeclareAttackers {eligible,..}=>eligible.contains(&lord),RequestKind::Priority {..}=>false,other=>panic!("{other:?}")};
        assert_eq!(offered,!entered);
    }
}
