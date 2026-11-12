#[test]
fn generated_rapid_fire_adds_rampage_only_if_the_creature_does_not_already_have_it() {
    static RAMPER:CardDef=CardDef {name:"Family fixture ramper",abilities:&[cantrip_mechanics::rampage::trigger::<1>("Rampage 1")],..BEAR};
    for already in [false,true] {
        let mut g=setup(&[&rapid_fire::CARD,&RAMPER]);let creature=put(&mut g,if already {&RAMPER}else{&BEAR},B,Zone::BATTLEFIELD);
        let spell=put(&mut g,&rapid_fire::CARD,A,Zone::hand(A));
        announce(&mut g,spell,ActionKind::CastSpell,0,vec![Target::Object(creature)]);finish(&mut g);
        assert!(g.chars(creature).has(Keyword::FirstStrike));
        let families:Vec<_>=g.chars(creature).abilities().filter(|&a|g.triggered_ability(a).is_some_and(|t|t.keyword_family==Some(KeywordFamily::Rampage))).collect();
        assert_eq!(families.len(),1);
        if already {assert_eq!(families[0],AbilityRef {card:g.cards.id_of(&RAMPER),index:0});}
    }
}
#[test]
fn generated_shaman_excludes_existing_upkeep_and_its_own_newly_granted_upkeep() {
    static WHITE:CardDef=CardDef {name:"Family fixture white enchantment",types:TypeSet::of(&[CardType::Enchantment]),colors:Some(ColorSet::of(Color::White)),..CardDef::BLANK};
    static CU:CardDef=CardDef {name:"Family fixture white upkeep",abilities:&[cantrip_mechanics::cumulative_upkeep!(cantrip_mechanics::cumulative_upkeep::Payment::Mana(mana("{1}")),"Cumulative upkeep {1}")],..WHITE};
    let mut g=setup(&[&balduvian_shaman::CARD,&WHITE,&CU]);let shaman=put(&mut g,&balduvian_shaman::CARD,A,Zone::BATTLEFIELD);
    let target=put(&mut g,&WHITE,A,Zone::BATTLEFIELD);let other=put(&mut g,&WHITE,A,Zone::BATTLEFIELD);let existing=put(&mut g,&CU,A,Zone::BATTLEFIELD);
    let untap=put(&mut g,&UNTAP,A,Zone::hand(A));
    announce(&mut g,shaman,ActionKind::ActivateAbility,0,vec![Target::Object(target)]);
    finish_with(&mut g,&mut |_,r|match &r.kind {
        RequestKind::ChooseColors {choices,..}=>Some(Answer::Colors(vec![Color::ALL.into_iter().find(|&c|choices.has(c)).unwrap()])),_=>None,
    });
    assert!(g.has_keyword_family(target,KeywordFamily::CumulativeUpkeep));
    announce(&mut g,untap,ActionKind::CastSpell,0,vec![Target::Object(shaman)]);finish(&mut g);
    let RequestKind::Priority {options}=&g.pending().unwrap().kind else {panic!("priority")};
    let option=options.iter().find(|o|o.source==Some(shaman)).unwrap();
    assert!(option.target_candidates.contains(&Target::Object(other)));
    assert!(!option.target_candidates.contains(&Target::Object(target)));assert!(!option.target_candidates.contains(&Target::Object(existing)));
}
