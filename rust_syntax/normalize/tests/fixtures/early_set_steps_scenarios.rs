#[test]
fn generated_elkin_bottle_play_permission_ends_at_the_next_upkeep_and_the_card_stays_exiled() {
    let mut g=setup(&[&elkin_bottle::CARD]);let bottle=put(&mut g,&elkin_bottle::CARD,A,Zone::BATTLEFIELD);
    announce(&mut g,bottle,ActionKind::ActivateAbility,0,vec![]);finish(&mut g);
    let card=g.state.permissions.iter().find(|p|p.player==A).unwrap().what.card().unwrap();
    assert_eq!(g.state.permissions[0].grant.expiry,Expiry::UntilYourNextStep(Step::Upkeep));
    advance_to(&mut g,|g|g.state.turn.active==A && g.state.turn.step()==Step::Upkeep);
    assert!(g.state.permissions.is_empty());assert!(g.state.obj(card).zone.is(ZoneKind::Exile));
}
#[test]
fn generated_halfdane_keeps_its_setting_through_the_next_upkeep_after_its_ability_is_lost() {
    fn strip(g:&mut dyn Game,item:&StackItem)->Result<(),Suspend> {
        if let Some(object)=cantrip_mechanics::targets::object(g,item,0){g.add_effect(item.object,object,EffectKind::LoseAllAbilities,Duration::Indefinite);}Ok(())
    }
    static STRIP:CardDef=CardDef {name:"Step fixture strip",spell:spell(PERMANENT,strip),..BOUNCE};
    let mut g=setup(&[&halfdane::CARD,&STRIP]);let half=put(&mut g,&halfdane::CARD,A,Zone::BATTLEFIELD);put(&mut g,&BEAR,B,Zone::BATTLEFIELD);
    let strip=put(&mut g,&STRIP,A,Zone::hand(A));
    // Prime priority, then traverse actual steps until its first upkeep ability has resolved.
    let prime=put(&mut g,&SUPPLY,A,Zone::hand(A));announce(&mut g,prime,ActionKind::CastSpell,0,vec![]);finish(&mut g);
    advance_to(&mut g,|g|g.state.turn.active==A && g.state.turn.step()==Step::Upkeep && g.state.turn.number>1 && g.state.stack.is_empty());
    assert_eq!((g.chars(half).power,g.chars(half).toughness),(Some(2),Some(2)));
    announce(&mut g,strip,ActionKind::CastSpell,0,vec![Target::Object(half)]);finish(&mut g);
    let turn=g.state.turn.number;
    advance_to(&mut g,|g|g.state.turn.active==A && g.state.turn.step()==Step::Upkeep && g.state.turn.number>turn);
    assert_eq!((g.chars(half).power,g.chars(half).toughness),(Some(2),Some(2)));
    advance_to(&mut g,|g|g.state.turn.active==A && g.state.turn.step()==Step::Draw);
    assert_eq!((g.chars(half).power,g.chars(half).toughness),(Some(3),Some(3)));
}
#[test]
fn generated_xenic_poltergeist_stops_animating_before_its_next_upkeep() {
    static ARTIFACT:CardDef=CardDef {name:"Step fixture artifact",types:TypeSet::of(&[CardType::Artifact]),cost:mana("{3}"),has_cost:true,..CardDef::BLANK};
    let mut g=setup(&[&xenic_poltergeist::CARD,&ARTIFACT]);let poltergeist=put(&mut g,&xenic_poltergeist::CARD,A,Zone::BATTLEFIELD);let artifact=put(&mut g,&ARTIFACT,B,Zone::BATTLEFIELD);
    announce(&mut g,poltergeist,ActionKind::ActivateAbility,0,vec![Target::Object(artifact)]);finish(&mut g);
    assert!(g.chars(artifact).is(CardType::Creature));assert_eq!((g.chars(artifact).power,g.chars(artifact).toughness),(Some(3),Some(3)));
    advance_to(&mut g,|g|g.state.turn.active==A && g.state.turn.step()==Step::Upkeep);
    assert!(!g.chars(artifact).is(CardType::Creature));assert_eq!((g.chars(artifact).power,g.chars(artifact).toughness),(None,None));
}
#[test]
fn generated_telekinesis_keeps_the_creature_tapped_for_two_of_its_controllers_untap_steps() {
    let mut g=setup(&[&telekinesis::CARD]);let creature=put(&mut g,&BEAR,B,Zone::BATTLEFIELD);let spell=put(&mut g,&telekinesis::CARD,A,Zone::hand(A));
    announce(&mut g,spell,ActionKind::CastSpell,0,vec![Target::Object(creature)]);finish(&mut g);
    assert!(g.state.obj(creature).tapped);
    for n in 0..3 {
        let turn=g.state.turn.number;
        advance_to(&mut g,|g|g.state.turn.active==B && g.state.turn.step()==Step::Upkeep && g.state.turn.number>turn);
        assert_eq!(g.state.obj(creature).tapped,n<2,"untap step {}",n+1);
    }
}
