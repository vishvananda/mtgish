#[test]
fn generated_manabarbs_and_burning_earth_queue_damage_to_the_payer_after_mana_resolves() {
    static LAND:CardDef=CardDef {name:"Observed nonbasic land",abilities:&[mana_ability(TAP,add(Mana::G),"{T}: Add {G}.")],..FILLER};
    static BASIC_LAND:CardDef=CardDef {name:"Observed basic land",supertypes:Supertypes {basic:true,..CardDef::BLANK.supertypes},..LAND};
    static ART:CardDef=CardDef {name:"Observed artifact mana",types:TypeSet::of(&[CardType::Artifact]),..LAND};
    for (observer,source,damage) in [(&manabarbs::CARD,&BASIC_LAND,true),(&manabarbs::CARD,&ART,false),(&burning_earth::CARD,&BASIC_LAND,false),(&burning_earth::CARD,&LAND,true)] {
        let mut g=setup(&[observer,&LAND,&BASIC_LAND,&ART]);put(&mut g,observer,B,Zone::BATTLEFIELD);let source=put(&mut g,source,A,Zone::BATTLEFIELD);
        announce(&mut g,source,ActionKind::ActivateAbility,0,vec![]);g.run();
        assert_eq!(g.state.player(A).pool.get(Mana::G),101);assert_eq!(g.state.player(A).life,20);
        assert_eq!(g.state.stack.len(),usize::from(damage));finish(&mut g);
        assert_eq!(g.state.player(A).life,if damage {19}else{20});assert_eq!(g.state.player(B).life,20);
    }
}
