static BLOCK_LARGE: CardDef = CardDef {
    name: "Blocking large fixture",
    toughness: Some(4),
    ..BEAR
};
static BLOCK_FLYER: CardDef = CardDef {
    name: "Blocking flying fixture",
    keywords: Keywords::of(&[Keyword::Flying]),
    ..BEAR
};
static BLOCK_FIRST: CardDef = CardDef {
    name: "Blocking first strike fixture",
    keywords: Keywords::of(&[Keyword::FirstStrike]),
    ..BLOCK_LARGE
};
static BLOCK_MOUNTAIN: CardDef = CardDef {
    name: "Blocking basic Mountain fixture",
    subtypes: SubtypeSet::of(&[Subtype::Mountain]),
    ..BASIC
};
fn blocking_shroud(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
    if let Some(id) = cantrip_mechanics::targets::object(g, item, 0) {
        g.add_effect(
            item.object,
            id,
            EffectKind::AddKeywords(Keywords::of(&[Keyword::Shroud])),
            Duration::EndOfTurn,
        );
    }
    Ok(())
}
static BLOCK_SHROUD: CardDef = CardDef {
    name: "Blocking shroud response fixture",
    spell: spell(PERMANENT, blocking_shroud),
    ..BOUNCE
};
static BLOCK_WALKER: CardDef = CardDef {
    name: "Blocking planeswalker fixture",
    types: TypeSet::of(&[CardType::Planeswalker]),
    loyalty: Some(6),
    ..CardDef::BLANK
};
fn blocking_create_walker(g: &mut dyn Game, _: &StackItem) -> Result<(), Suspend> {
    let card = g.card_id(&BLOCK_WALKER);
    g.create_tokens(card, B, 1, false)?;
    Ok(())
}
static BLOCK_CREATE_WALKER: CardDef = CardDef {
    name: "Blocking create planeswalker fixture",
    spell: spell(&[], blocking_create_walker),
    ..BOUNCE
};
fn finish_block_targets(g: &mut RulesGame, targets: &[ObjectId]) {
    let mut chose = false;
    finish_with(g, &mut |_, r| match &r.kind {
        RequestKind::ChooseTarget {
            purpose: Purpose::Targets,
            candidates,
            ..
        } => {
            assert!(targets
                .iter()
                .all(|&id| candidates.contains(&Target::Object(id))));
            chose = true;
            Some(Answer::Targets(
                targets.iter().copied().map(Target::Object).collect(),
            ))
        }
        _ => None,
    });
    assert!(
        chose,
        "multi-target announcement asks its own selection question"
    );
}
fn declare_block_position(
    g: &mut RulesGame,
    attackers: &[ObjectId],
    blocks: &[(ObjectId, ObjectId)],
) {
    declare_block_position_to(
        g,
        &attackers
            .iter()
            .map(|&a| (a, Target::Player(B)))
            .collect::<Vec<_>>(),
        blocks,
    );
}
fn declare_block_position_to(
    g: &mut RulesGame,
    attackers: &[(ObjectId, Target)],
    blocks: &[(ObjectId, ObjectId)],
) {
    let r = group_attack_request(g);
    g.answer(r.id, r.player, Answer::Attack(attackers.to_vec()))
        .unwrap();
    advance_to(g, |g| {
        matches!(
            g.pending().map(|r| &r.kind),
            Some(RequestKind::DeclareBlockers { .. })
        )
    });
    let r = g.pending().cloned().unwrap();
    g.answer(r.id, r.player, Answer::Block(blocks.to_vec()))
        .unwrap();
    advance_to(g, |g| {
        matches!(
            g.pending().map(|r| &r.kind),
            Some(RequestKind::Priority { .. })
        ) && g.state.stack.is_empty()
    });
    assert_eq!(g.state.turn.step(), Step::DeclareBlockers);
}
#[test]
fn generated_false_orders_unblocks_only_the_historical_sole_blocker_and_reblocks_without_targeting()
{
    for (had_other, reblock) in [(false, false), (false, true), (true, true)] {
        let mut g = setup(&[&false_orders::CARD, &BLOCK_MOUNTAIN]);
        let mountain = put(&mut g, &BLOCK_MOUNTAIN, A, Zone::BATTLEFIELD);
        let a = put(&mut g, &BEAR, A, Zone::BATTLEFIELD);
        let other = put(&mut g, &BEAR, A, Zone::BATTLEFIELD);
        let blocker = put(&mut g, &BEAR, B, Zone::BATTLEFIELD);
        let another = put(&mut g, &BEAR, B, Zone::BATTLEFIELD);
        let spell = put(&mut g, &false_orders::CARD, A, Zone::hand(A));
        let blocks = if had_other {
            vec![(blocker, a), (another, a)]
        } else {
            vec![(blocker, a)]
        };
        declare_block_position(&mut g, &[a, other], &blocks);
        if had_other {
            let bounce = put(&mut g, &BOUNCE, A, Zone::hand(A));
            hand_announce(
                &mut g,
                bounce,
                ActionKind::CastSpell,
                0,
                vec![Target::Object(another)],
            );
            finish(&mut g);
        }
        hand_announce(
            &mut g,
            spell,
            ActionKind::CastSpell,
            0,
            vec![Target::Object(blocker)],
        );
        let mut chose = false;
        finish_with(&mut g, &mut |_, r| match &r.kind {
            RequestKind::PayCosts { options, .. } => options
                .iter()
                .find(|o| o.kind == ActionKind::ActivateAbility && o.source == Some(mountain))
                .map(|o| Answer::Act(Action::of(o))),
            RequestKind::May { .. } => Some(if reblock { Answer::Yes } else { Answer::No }),
            RequestKind::ChooseTarget {
                purpose: Purpose::Choose,
                candidates,
                ..
            } => {
                assert!(reblock && candidates.contains(&Target::Object(other)));
                chose = true;
                Some(Answer::Targets(vec![Target::Object(other)]))
            }
            _ => None,
        });
        assert_eq!(chose, reblock);
        assert_eq!(g.is_blocked(a), had_other);
        assert_eq!(g.is_blocked(other), reblock);
        assert_eq!(g.is_blocking(blocker), reblock);
        assert!(!g.history().all().iter().any(
            |r| matches!(r.event,Event::BecameTarget{target:Target::Object(id),..}if id==other)
        ));
        assert!(!g
            .history()
            .all()
            .iter()
            .any(|r| matches!(r.event,Event::Unblocked{attacker,..}if attacker==a)));
    }
}
#[test]
fn generated_jarkeld_exchanges_legal_blockers_and_refuses_an_illegal_flying_exchange() {
    for flying in [false, true] {
        let mut g = setup(&[&general_jarkeld::CARD, &BLOCK_FLYER]);
        let general = put(&mut g, &general_jarkeld::CARD, A, Zone::BATTLEFIELD);
        let a = put(
            &mut g,
            if flying { &BLOCK_FLYER } else { &BEAR },
            A,
            Zone::BATTLEFIELD,
        );
        let b = put(&mut g, &BEAR, A, Zone::BATTLEFIELD);
        let x = put(
            &mut g,
            if flying { &BLOCK_FLYER } else { &BEAR },
            B,
            Zone::BATTLEFIELD,
        );
        let y = put(&mut g, &BEAR, B, Zone::BATTLEFIELD);
        declare_block_position(&mut g, &[a, b], &[(x, a), (y, b)]);
        hand_announce(&mut g, general, ActionKind::ActivateAbility, 0, vec![]);
        finish_block_targets(&mut g, &[a, b]);
        assert!(g.state.obj(general).tapped);
        assert!(g
            .state
            .blocks
            .iter()
            .any(|p| p.blocker == x && p.attacker == if flying { a } else { b }));
        assert!(g
            .state
            .blocks
            .iter()
            .any(|p| p.blocker == y && p.attacker == if flying { b } else { a }));
        assert!(g.is_blocked(a) && g.is_blocked(b));
        if !flying {
            assert_eq!(
                g.history()
                    .all()
                    .iter()
                    .filter(|r| matches!(
                        r.event,
                        Event::Blocks {
                            declared: false,
                            ..
                        }
                    ))
                    .count(),
                2
            );
        }
    }
}
#[test]
fn generated_sorrows_path_swaps_blockers_and_keeps_its_tap_damage_trigger_separate() {
    let mut g = setup(&[&sorrow_s_path::CARD, &BLOCK_LARGE]);
    let land = put(&mut g, &sorrow_s_path::CARD, A, Zone::BATTLEFIELD);
    let a = put(&mut g, &BLOCK_LARGE, A, Zone::BATTLEFIELD);
    let b = put(&mut g, &BLOCK_LARGE, A, Zone::BATTLEFIELD);
    let x = put(&mut g, &BEAR, B, Zone::BATTLEFIELD);
    let y = put(&mut g, &BEAR, B, Zone::BATTLEFIELD);
    declare_block_position(&mut g, &[a, b], &[(x, a), (y, b)]);
    hand_announce(&mut g, land, ActionKind::ActivateAbility, 0, vec![]);
    finish_block_targets(&mut g, &[x, y]);
    assert!(g.state.obj(land).tapped);
    assert_eq!(g.state.player(A).life, 18);
    assert_eq!((g.state.obj(a).damage, g.state.obj(b).damage), (2, 2));
    assert!(g
        .state
        .blocks
        .iter()
        .any(|p| p.blocker == x && p.attacker == b));
    assert!(g
        .state
        .blocks
        .iter()
        .any(|p| p.blocker == y && p.attacker == a));
    assert_eq!(
        g.history()
            .all()
            .iter()
            .filter(|r| matches!(r.event, Event::Blocks { declared: true, .. }))
            .count(),
        4
    );
}
#[test]
fn generated_sorrows_path_preserves_first_strike_when_its_own_damage_removes_the_old_attacker() {
    let mut g = setup(&[&sorrow_s_path::CARD, &BLOCK_LARGE, &BLOCK_FIRST]);
    let land = put(&mut g, &sorrow_s_path::CARD, A, Zone::BATTLEFIELD);
    let a = put(&mut g, &BLOCK_LARGE, A, Zone::BATTLEFIELD);
    let b = put(&mut g, &BLOCK_LARGE, A, Zone::BATTLEFIELD);
    let x = put(&mut g, &BLOCK_FIRST, B, Zone::BATTLEFIELD);
    let y = put(&mut g, &BLOCK_LARGE, B, Zone::BATTLEFIELD);
    declare_block_position(&mut g, &[a, b], &[(x, a), (y, b)]);
    advance_to(&mut g, |g| {
        g.state.turn.step() == Step::CombatDamage
            && g.state.turn.damage_round == 1
            && matches!(
                g.pending().map(|r| &r.kind),
                Some(RequestKind::Priority { .. })
            )
    });
    assert_eq!(g.state.obj(a).damage, 2);
    hand_announce(&mut g, land, ActionKind::ActivateAbility, 0, vec![]);
    finish_block_targets(&mut g, &[x, y]);
    assert!(g.state.obj(current(&g, a)).zone.is(ZoneKind::Graveyard));
    assert!(!g.is_blocking(y));
    assert!(g.is_blocking(x));
    assert_eq!(g.state.blocks.len(), 1);
    assert!(g.state.blocks[0].first_strike);
    advance_to(&mut g, |g| {
        g.state.turn.step() == Step::CombatDamage
            && g.state.turn.damage_round == 2
            && matches!(
                g.pending().map(|r| &r.kind),
                Some(RequestKind::Priority { .. })
            )
    });
    assert!(g.is_on_battlefield(b));
    assert_eq!(g.state.obj(b).damage, 2);
}
#[test]
fn generated_jarkeld_does_not_act_on_an_illegal_target_when_its_other_target_stays_legal() {
    let mut g = setup(&[&general_jarkeld::CARD, &BLOCK_SHROUD]);
    let general = put(&mut g, &general_jarkeld::CARD, A, Zone::BATTLEFIELD);
    let response = put(&mut g, &BLOCK_SHROUD, A, Zone::hand(A));
    let a = put(&mut g, &BEAR, A, Zone::BATTLEFIELD);
    let b = put(&mut g, &BEAR, A, Zone::BATTLEFIELD);
    let x = put(&mut g, &BEAR, B, Zone::BATTLEFIELD);
    let y = put(&mut g, &BEAR, B, Zone::BATTLEFIELD);
    declare_block_position(&mut g, &[a, b], &[(x, a), (y, b)]);
    hand_announce(&mut g, general, ActionKind::ActivateAbility, 0, vec![]);
    g.run();
    let r = g.pending().cloned().unwrap();
    assert!(matches!(
        r.kind,
        RequestKind::ChooseTarget {
            purpose: Purpose::Targets,
            ..
        }
    ));
    g.answer(
        r.id,
        r.player,
        Answer::Targets(vec![Target::Object(a), Target::Object(b)]),
    )
    .unwrap();
    advance_to(&mut g, |g| {
        g.state.stack.iter().any(|i| i.object == general)
            && matches!(
                g.pending().map(|r| &r.kind),
                Some(RequestKind::Priority { .. })
            )
    });
    hand_announce(
        &mut g,
        response,
        ActionKind::CastSpell,
        0,
        vec![Target::Object(a)],
    );
    finish(&mut g);
    assert!(g.chars(a).has(Keyword::Shroud));
    assert!(g.state.obj(general).tapped);
    assert!(g
        .state
        .blocks
        .iter()
        .any(|p| p.blocker == x && p.attacker == a));
    assert!(g
        .state
        .blocks
        .iter()
        .any(|p| p.blocker == y && p.attacker == b));
}
#[test]
fn generated_false_orders_targets_the_defender_of_a_planeswalker_even_after_it_leaves() {
    for departed in [false, true] {
        let mut g = setup(&[
            &false_orders::CARD,
            &BLOCK_MOUNTAIN,
            &BLOCK_WALKER,
            &BLOCK_CREATE_WALKER,
        ]);
        let mountain = put(&mut g, &BLOCK_MOUNTAIN, A, Zone::BATTLEFIELD);
        let a = put(&mut g, &BEAR, A, Zone::BATTLEFIELD);
        let blocker = put(&mut g, &BEAR, B, Zone::BATTLEFIELD);
        let spell = put(&mut g, &false_orders::CARD, A, Zone::hand(A));
        let make = put(&mut g, &BLOCK_CREATE_WALKER, A, Zone::hand(A));
        hand_announce(&mut g, make, ActionKind::CastSpell, 0, vec![]);
        finish(&mut g);
        let walker = *g
            .state
            .battlefield()
            .iter()
            .find(|&&id| g.state.obj(id).card == g.cards.id_of(&BLOCK_WALKER))
            .unwrap();
        declare_block_position_to(&mut g, &[(a, Target::Object(walker))], &[(blocker, a)]);
        if departed {
            let bounce = put(&mut g, &BOUNCE, A, Zone::hand(A));
            hand_announce(
                &mut g,
                bounce,
                ActionKind::CastSpell,
                0,
                vec![Target::Object(walker)],
            );
            finish(&mut g);
            assert_eq!(g.state.attackers[0].target, Target::None);
        }
        hand_announce(
            &mut g,
            spell,
            ActionKind::CastSpell,
            0,
            vec![Target::Object(blocker)],
        );
        finish_with(&mut g, &mut |_, r| match &r.kind {
            RequestKind::PayCosts { options, .. } => options
                .iter()
                .find(|o| o.kind == ActionKind::ActivateAbility && o.source == Some(mountain))
                .map(|o| Answer::Act(Action::of(o))),
            RequestKind::May { .. } => Some(Answer::No),
            _ => None,
        });
        assert!(!g.is_blocked(a));
        assert!(!g.is_blocking(blocker));
        assert_eq!(g.state.attackers[0].defending, B);
    }
}
#[test]
fn generated_defender_predicate_uses_combat_roles_and_has_no_defender_outside_combat() {
    let mut g = setup(&[&false_orders::CARD]);
    let spell = put(&mut g, &false_orders::CARD, A, Zone::hand(A));
    let theirs = put(&mut g, &BEAR, B, Zone::BATTLEFIELD);
    let ours = put(&mut g, &BEAR, A, Zone::BATTLEFIELD);
    let asker = Asker::new(spell, A);
    let filter = false_orders::CARD.spell.as_ref().unwrap().targets[0].filter;
    assert!(!filter(&g, &asker, Target::Object(theirs)));
    g.state.update_turn(|t| t.set_step(Step::BeginCombat));
    assert!(g.state.attackers.is_empty());
    assert!(filter(&g, &asker, Target::Object(theirs)));
    assert!(!filter(&g, &asker, Target::Object(ours)));
    g.state.update_turn(|t| t.set_step(Step::PostcombatMain));
    assert!(!filter(&g, &asker, Target::Object(theirs)));
    g.state.update_turn(|t| {
        t.active = B;
        t.set_step(Step::DeclareBlockers);
    });
    assert!(!filter(&g, &asker, Target::Object(theirs)));
    assert!(filter(&g, &asker, Target::Object(ours)));
}
