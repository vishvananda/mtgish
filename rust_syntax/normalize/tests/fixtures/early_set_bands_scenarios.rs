static BAND_WHITE:CardDef=CardDef {name:"Foundation white legend",supertypes:Supertypes {legendary:true,..CardDef::BLANK.supertypes},..BEAR};
static BAND_BLUE:CardDef=CardDef {name:"Foundation blue legend",colors:Some(ColorSet::of(Color::Blue)),..BAND_WHITE};
static BAND_BLACK:CardDef=CardDef {name:"Foundation black legend",colors:Some(ColorSet::of(Color::Black)),..BAND_WHITE};
static BAND_RED:CardDef=CardDef {name:"Foundation red legend",colors:Some(ColorSet::of(Color::Red)),..BAND_WHITE};
static BAND_GREEN:CardDef=CardDef {name:"Foundation green legend",colors:Some(ColorSet::of(Color::Green)),..BAND_WHITE};
fn has_other_banding(g:&RulesGame,id:ObjectId)->bool {
    g.chars(id).abilities().any(|a|matches!(g.cards.ability(a),AbilityDef::BandsWithOthers(_)))
}
fn join_generated_band(g:&mut RulesGame,source:ObjectId,peer:ObjectId) {
    let mut formed=false;
    for _ in 0..120 {
        g.run();let r=g.pending().cloned().expect("band question");
        let answer=match &r.kind {
            RequestKind::DeclareAttackers {..}=>Answer::Attack(vec![(source,Target::Player(B)),(peer,Target::Player(B))]),
            RequestKind::ChooseCards {purpose:Purpose::Band,cards,..}=>{
                assert!(cards.contains(&peer));formed=true;Answer::Cards(vec![peer])
            }
            _=>default_answer(g,&r),
        };
        g.answer(r.id,r.player,answer).unwrap();
        if formed {g.run();break;}
    }
    assert!(formed);let band=g.state.attackers.iter().find(|a|a.id==source).unwrap().band;
    assert_ne!(band,0);assert_eq!(g.state.attackers.iter().find(|a|a.id==peer).unwrap().band,band);
}
#[test]
fn generated_legendary_lands_choose_recipients_by_color_but_band_across_colors() {
    for (land,provider,peer) in [
        (&adventurers_guildhouse::CARD,&BAND_GREEN,&BAND_WHITE),
        (&cathedral_of_serra::CARD,&BAND_WHITE,&BAND_RED),
        (&mountain_stronghold::CARD,&BAND_RED,&BAND_BLUE),
        (&seafarer_s_quay::CARD,&BAND_BLUE,&BAND_BLACK),
        (&unholy_citadel::CARD,&BAND_BLACK,&BAND_GREEN),
    ] {
        let mut g=setup(&[land,provider,peer]);put(&mut g,land,A,Zone::BATTLEFIELD);
        let source=put(&mut g,provider,A,Zone::BATTLEFIELD);let peer=put(&mut g,peer,A,Zone::BATTLEFIELD);
        let nonlegend=put(&mut g,&BEAR,A,Zone::BATTLEFIELD);let opponent=put(&mut g,provider,B,Zone::BATTLEFIELD);
        put(&mut g,&BOUNCE,A,Zone::hand(A));put(&mut g,&BOUNCE,B,Zone::hand(B));
        let prime=put(&mut g,&SUPPLY,A,Zone::hand(A));announce(&mut g,prime,ActionKind::CastSpell,0,vec![]);finish(&mut g);
        assert!(has_other_banding(&g,source));assert!(!g.chars(source).has(Keyword::Banding));
        assert!(!has_other_banding(&g,peer));assert!(!has_other_banding(&g,nonlegend));assert!(!has_other_banding(&g,opponent));
        join_generated_band(&mut g,source,peer);
    }
}
#[test]
fn generated_master_creates_the_real_named_token_and_its_tokens_form_a_band() {
    let mut g=setup(&[&master_of_the_hunt::CARD,&master_of_the_hunt::token_1::CARD]);
    let master=put(&mut g,&master_of_the_hunt::CARD,A,Zone::BATTLEFIELD);
    put(&mut g,&BOUNCE,A,Zone::hand(A));put(&mut g,&BOUNCE,B,Zone::hand(B));
    for _ in 0..2 {announce(&mut g,master,ActionKind::ActivateAbility,0,vec![]);finish(&mut g);}
    let wolves:Vec<_>=g.state.battlefield().iter().copied().filter(|&id|g.chars(id).name=="Wolves of the Hunt").collect();
    assert_eq!(wolves.len(),2);assert!(wolves.iter().all(|&id|has_other_banding(&g,id)));
    assert!(!has_other_banding(&g,master));
    advance_to(&mut g,|g|g.state.turn.number==3&&g.state.turn.active==A&&g.state.turn.step()==Step::PrecombatMain);
    join_generated_band(&mut g,wolves[0],wolves[1]);
}
#[test]
fn generated_shelkin_and_tolaria_remove_the_right_families_at_the_right_time() {
    static ORDINARY:CardDef=CardDef {name:"Foundation legend with ordinary banding",keywords:Keywords::of(&[Keyword::Banding]),..BAND_WHITE};
    let mut g=setup(&[&cathedral_of_serra::CARD,&shelkin_brownie::CARD,&tolaria::CARD,&ORDINARY]);
    put(&mut g,&cathedral_of_serra::CARD,A,Zone::BATTLEFIELD);let creature=put(&mut g,&ORDINARY,A,Zone::BATTLEFIELD);
    let brownie=put(&mut g,&shelkin_brownie::CARD,A,Zone::BATTLEFIELD);let tolaria=put(&mut g,&tolaria::CARD,A,Zone::BATTLEFIELD);
    put(&mut g,&BOUNCE,A,Zone::hand(A));put(&mut g,&BOUNCE,B,Zone::hand(B));
    let prime=put(&mut g,&SUPPLY,A,Zone::hand(A));announce(&mut g,prime,ActionKind::CastSpell,0,vec![]);finish(&mut g);
    assert!(has_other_banding(&g,creature));assert!(g.chars(creature).has(Keyword::Banding));
    let RequestKind::Priority {options}=&g.pending().unwrap().kind else {panic!()};
    assert!(!options.iter().any(|o|o.source==Some(tolaria)&&o.index==1&&o.kind==ActionKind::ActivateAbility));
    announce(&mut g,brownie,ActionKind::ActivateAbility,0,vec![Target::Object(creature)]);finish(&mut g);
    assert!(!has_other_banding(&g,creature));assert!(g.chars(creature).has(Keyword::Banding));
    advance_to(&mut g,|g|g.state.turn.number==3&&g.state.turn.active==A&&g.state.turn.step()==Step::Upkeep);
    assert!(has_other_banding(&g,creature));
    announce(&mut g,tolaria,ActionKind::ActivateAbility,1,vec![Target::Object(creature)]);finish(&mut g);
    assert!(!has_other_banding(&g,creature));assert!(!g.chars(creature).has(Keyword::Banding));
}
