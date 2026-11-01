use mtgish_ast::{corpus, ir::*, lower, mtg_types as source, validate};
fn source(name: &str) -> source::OracleCard {
    include_str!("fixtures/tokens.ronl")
        .lines()
        .map(|line| ron::from_str(line).unwrap())
        .find(|card| {
            corpus::Identity::of(card)
                .unwrap()
                .names
                .iter()
                .any(|n| n == name)
        })
        .unwrap()
}
fn card(name: &str) -> Card {
    let c = lower::card(&source(name)).unwrap_or_else(|e| panic!("{name}: {e:?}"));
    validate::card(&c).unwrap();
    c
}
fn creations(card: &Card) -> (&[TokenSpec], &EntrySpec) {
    card.abilities
        .iter()
        .find_map(|a| {
            let body = match a {
                Ability::Spell { body, .. }
                | Ability::Triggered { body, .. }
                | Ability::Activated { body, .. } => body,
                _ => return None,
            };
            body.instructions.iter().find_map(|i| match i {
                Instruction::Effect {
                    operation: Operation::CreateTokens { tokens, entry, .. },
                    ..
                } => Some((tokens.as_slice(), entry)),
                _ => None,
            })
        })
        .unwrap()
}
#[test]
fn real_token_families_preserve_typed_ron_json_and_binary_data() {
    for name in [
        "Raise the Alarm",
        "Dragon Fodder",
        "Servo Exhibition",
        "Slime Molding",
        "Gelatinous Genesis",
        "Talrand's Invocation",
        "Sworn Companions",
        "Release the Dogs",
        "Spore Swarm",
        "Bestial Menace",
        "Shadow Summoning",
        "Bake into a Pie",
        "Tempting Witch",
        "Tolsimir Wolfblood",
        "Call the Bloodline",
        "Form a Posse",
        "Prickly Pair",
    ] {
        let c = card(name);
        let expected = serde_json::to_value(&c).unwrap();
        let bytes = bincode::encode_to_vec(&c, bincode::config::standard()).unwrap();
        let (binary, used): (Card, usize) =
            bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
        assert_eq!(used, bytes.len());
        for loaded in [
            binary,
            ron::from_str(&ron::ser::to_string(&c).unwrap()).unwrap(),
            serde_json::from_value(expected.clone()).unwrap(),
        ] {
            validate::card(&loaded).unwrap();
            assert_eq!(serde_json::to_value(loaded).unwrap(), expected);
        }
    }
}
#[test]
fn different_token_definitions_remain_one_creation_batch_and_x_has_two_roles() {
    let menace = card("Bestial Menace");
    let (tokens, entry) = creations(&menace);
    assert_eq!(tokens.len(), 3);
    assert!(!entry.tapped);
    for (i, t) in tokens.iter().enumerate() {
        assert!(matches!(t.amount, Expr::Constant(1)));
        assert!(
            matches!(&t.definition().unwrap().power_toughness,Some((Expr::Constant(p),Expr::Constant(q))) if *p==i as i64+1&&p==q)
        );
    }
    let Ability::Spell { body, .. } = &menace.abilities[0] else {
        panic!()
    };
    assert_eq!(body.instructions.len(), 1);
    for (name, multiple) in [("Slime Molding", false), ("Gelatinous Genesis", true)] {
        let c = card(name);
        let (t, _) = creations(&c);
        assert_eq!(t.len(), 1);
        assert_eq!(matches!(t[0].amount, Expr::AnnouncedX), multiple);
        assert!(matches!(
            t[0].definition().unwrap().power_toughness,
            Some((Expr::AnnouncedX, Expr::AnnouncedX))
        ));
    }
    let shadow = card("Shadow Summoning");
    let (t, entry) = creations(&shadow);
    assert!(entry.tapped);
    assert!(matches!(
        t[0].definition().unwrap().abilities.as_slice(),
        [Ability::Keyword(Keyword::Flying)]
    ));
}
#[test]
fn predefined_food_expands_the_existing_rust_recipe_in_a_fresh_ability_scope() {
    let c = card("Bake into a Pie");
    let (t, _) = creations(&c);
    assert!(matches!(t[0].words, TokenWords::Rules));
    let Ability::Activated {
        cost,
        announcement,
        body,
        ..
    } = &t[0].definition().unwrap().abilities[0]
    else {
        panic!()
    };
    assert!(announcement.targets.is_empty());
    assert!(cost.parts.iter().any(|p| matches!(
        p,
        CostPart::Sacrifice {
            objects: ObjectSet::One(ObjectRef::Source),
            choose: None
        }
    )));
    assert!(matches!(
        body.instructions.as_slice(),
        [Instruction::Effect {
            operation: Operation::GainLife {
                players: PlayerSet::One(PlayerRef::AbilityController),
                amount: Expr::Constant(3)
            },
            ..
        }]
    ));
    let named = card("Tolsimir Wolfblood");
    let (t, _) = creations(&named);
    assert!(matches!(&t[0].definition().unwrap().name,TokenName::Explicit(n) if n=="Voja"));
    assert_eq!(t[0].definition().unwrap().colors.len(), 2);
    let opportunist = card("Argothian Opportunist");
    let (tokens, entry) = creations(&opportunist);
    assert!(entry.tapped);
    let Ability::Activated { body, .. } = &tokens[0].definition().unwrap().abilities[0] else {
        panic!()
    };
    assert!(matches!(
        &body.instructions[0],
        Instruction::Effect {
            operation: Operation::AddMana {
                restriction: Some(ManaUse::Not(_)),
                ..
            },
            ..
        }
    ));
    let harvest = card("Acorn Harvest");
    assert!(harvest.abilities.iter().any(|ability| matches!(
        ability,
        Ability::Casting(CastingRule::Permission {
            origin: CastingOrigin::Flashback,
            ..
        })
    )));
}
fn probe(actions: Vec<source::Action>) -> source::OracleCard {
    let mut c = source("Raise the Alarm");
    let source::OracleCard::Card { name, rules, .. } = &mut c else {
        panic!()
    };
    *name = "Token binding probe".into();
    *rules = Some(vec![source::Rule::SpellActions(Box::new(
        source::Actions::ActionList(actions),
    ))]);
    c
}
fn create() -> source::Action {
    let source::OracleCard::Card {
        rules: Some(rules), ..
    } = source("Raise the Alarm")
    else {
        panic!()
    };
    let source::Rule::SpellActions(actions) = &rules[0] else {
        panic!()
    };
    let source::Actions::ActionList(actions) = actions.as_ref() else {
        panic!()
    };
    actions[0].clone()
}
fn pump() -> source::Action {
    source::Action::CreateEachPermanentLayerEffectUntil(
        Box::new(source::Permanents::TheTokensCreatedThisWay),
        vec![source::LayerEffect::AdjustPT(1, 1)],
        source::Expiration::UntilEndOfTurn,
    )
}
#[test]
fn created_results_are_typed_scoped_and_read_nothing_after_a_skipped_option() {
    let mut c = lower::card(&probe(vec![create(), pump()])).unwrap();
    validate::card(&c).unwrap();
    let Ability::Spell { body, .. } = &mut c.abilities[0] else {
        panic!()
    };
    let Instruction::Effect { result, .. } = &body.instructions[0] else {
        panic!()
    };
    let result = result.0;
    let Instruction::Effect {
        operation:
            Operation::CreateContinuousEffect {
                affected: EffectSubjects::Objects(ObjectSet::Created(reference)),
                ..
            },
        ..
    } = &body.instructions[1]
    else {
        panic!()
    };
    assert_eq!(reference.0, result);
    let Instruction::Effect { operation, .. } = &mut body.instructions[0] else {
        panic!()
    };
    *operation = Operation::Draw {
        players: PlayerSet::One(PlayerRef::AbilityController),
        amount: Expr::Constant(1),
    };
    assert!(validate::card(&c)
        .unwrap_err()
        .contains("token-creation result"));
    // As for moves, a reference after an optional body reads nothing if it never ran.
    validate::card(
        &lower::card(&probe(vec![
            source::Action::MayAction(Box::new(create())),
            pump(),
        ]))
        .unwrap(),
    )
    .unwrap();
    for actions in [
        vec![
            create(),
            source::Action::MayAction(Box::new(create())),
            pump(),
        ],
        vec![
            create(),
            source::Action::If(
                source::Condition::IsPlayersTurn(Box::new(source::Player::You)),
                vec![create()],
            ),
            pump(),
        ],
    ] {
        assert!(lower::card(&probe(actions))
            .unwrap_err()
            .iter()
            .any(|e| e.message.contains("tokens-created reference")));
    }
}
#[test]
fn creation_bindings_inside_branches_keep_the_parent_result_in_the_other_branch() {
    let condition = source::Condition::IsPlayersTurn(Box::new(source::Player::You));
    let c = lower::card(&probe(vec![
        create(),
        source::Action::IfElse(condition, vec![create(), pump()], vec![pump()]),
    ]))
    .unwrap();
    validate::card(&c).unwrap();
    let Ability::Spell { body, .. } = &c.abilities[0] else {
        panic!()
    };
    let Instruction::If {
        then, otherwise, ..
    } = &body.instructions[1]
    else {
        panic!()
    };
    let created = |i: &Instruction| {
        let Instruction::Effect {
            operation:
                Operation::CreateContinuousEffect {
                    affected: EffectSubjects::Objects(ObjectSet::Created(result)),
                    ..
                },
            ..
        } = i
        else {
            panic!()
        };
        result.0
    };
    assert_eq!(created(&then.instructions[1]), 1);
    assert_eq!(created(&otherwise.instructions[0]), 0);
}
#[test]
fn entry_counters_stay_in_the_creation_and_timed_entry_rules_are_named() {
    let mut action = create();
    let source::Action::CreateTokens(_, flags) = &mut action else {
        panic!()
    };
    flags.extend([
        source::TokenFlag::EntersTapped,
        source::TokenFlag::EntersWithNumberCounters(
            Box::new(source::GameNumber::Integer(2)),
            source::CounterType::PTCounter(1, 1),
        ),
    ]);
    let c = lower::card(&probe(vec![action.clone()])).unwrap();
    validate::card(&c).unwrap();
    let (_, entry) = creations(&c);
    assert!(entry.tapped);
    assert!(matches!(
        entry.counters.as_slice(),
        [(
            mtgish_ast::counter::CounterKind::PowerToughness(1, 1),
            Expr::Constant(2)
        )]
    ));
    let Ability::Spell { body, .. } = &c.abilities[0] else {
        panic!()
    };
    assert_eq!(body.instructions.len(), 1);
    #[cfg(feature = "cantrip")]
    assert!(mtgish_ast::cantrip::module(&c).unwrap().contains("counter"));
    let source::Action::CreateTokens(_, flags) = &mut action else {
        panic!()
    };
    flags.push(source::TokenFlag::EntersWithRuleEffectUntil(
        vec![],
        source::Expiration::UntilEndOfTurn,
    ));
    let named = format!("{:?}", lower::card(&probe(vec![action])).unwrap());
    assert!(named.contains("RulesUntil"), "{named}");
}
