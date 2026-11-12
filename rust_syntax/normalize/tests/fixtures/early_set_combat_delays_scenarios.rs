#[test]
fn generated_goblin_flotilla_watches_block_events_only_for_the_unpaid_combat() {
    static RED_SOURCE:CardDef=CardDef {name:"Combat duration red source",abilities:&[mana_ability(TAP,add(Mana::R),"{T}: Add {R}.")],..FILLER};
    for pay in [false,true] {
        let mut g=setup(&[&goblin_flotilla::CARD,&RED_SOURCE]);
        let flotilla=put(&mut g,&goblin_flotilla::CARD,A,Zone::BATTLEFIELD);
        let blocker=put(&mut g,&BEAR,B,Zone::BATTLEFIELD);
        let mana=put(&mut g,&RED_SOURCE,A,Zone::BATTLEFIELD);
        put(&mut g,&BOUNCE,A,Zone::hand(A));put(&mut g,&BOUNCE,B,Zone::hand(B));
        let prime=put(&mut g,&SUPPLY,A,Zone::hand(A));announce(&mut g,prime,ActionKind::CastSpell,0,vec![]);finish(&mut g);
        let mut blocked=false;let mut checked=false;
        for _ in 0..200 {
            g.run();let r=g.pending().cloned().expect("combat question");
            if blocked&&g.state.turn.step()==Step::DeclareBlockers&&g.state.stack.is_empty()&&matches!(r.kind,RequestKind::Priority {..}) {
                assert_eq!(g.chars(blocker).has(Keyword::FirstStrike),!pay);
                assert_eq!(g.state.delayed.iter().filter(|d|d.when==DelayedWhen::ThisCombat).count(),usize::from(!pay));
                checked=true;break;
            }
            let answer=match &r.kind {
                RequestKind::DeclareAttackers {..}=>Answer::Attack(vec![(flotilla,Target::Player(B))]),
                RequestKind::DeclareBlockers {..}=>{blocked=true;Answer::Block(vec![(blocker,flotilla)])},
                RequestKind::May {..}=>if pay {Answer::Yes} else {Answer::No},
                RequestKind::UnlessPays {options,..}|RequestKind::PayCosts {options,announcing:false,..}=>{
                    let preferred=if pay {options.iter().find(|o|o.source==Some(mana)&&o.kind==ActionKind::ActivateAbility)}
                        else {options.iter().find(|o|o.kind==ActionKind::DeclineCost)};
                    preferred.map(|o|Answer::Act(Action::of(o))).unwrap_or_else(||default_answer(&g,&r))
                },
                _=>default_answer(&g,&r),
            };
            g.answer(r.id,r.player,answer).unwrap();
        }
        assert!(checked,"block trigger resolved; pay={pay}");
        advance_to(&mut g,|g|g.state.turn.step()==Step::PostcombatMain&&g.state.stack.is_empty());
        assert!(!g.state.delayed.iter().any(|d|d.when==DelayedWhen::ThisCombat));
    }
}
