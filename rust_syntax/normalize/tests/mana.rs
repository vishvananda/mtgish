use mtgish_ast::{
    corpus,
    ir::*,
    lower, mtg_types as source,
    semantics::{activation_kind, ActivationKind},
    validate,
};
fn source(name: &str) -> source::OracleCard {
    include_str!("fixtures/mana.ronl")
        .lines()
        .map(|s| ron::from_str(s).unwrap())
        .find(|c| {
            corpus::Identity::of(c)
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
fn kind(ability: &Ability) -> ActivationKind {
    let Ability::Activated {
        cost,
        announcement,
        body,
        ..
    } = ability
    else {
        panic!()
    };
    activation_kind(cost, announcement, body)
}
#[test]
fn complete_mana_families_roundtrip_in_typed_rust() {
    for name in [
        "Llanowar Elves",
        "Birds of Paradise",
        "Boreal Druid",
        "Sol Ring",
        "Mind Stone",
        "Dark Ritual",
        "Seething Song",
        "Selesnya Signet",
        "Izzet Signet",
        "Crystal Quarry",
        "Manamorphose",
        "Chromatic Sphere",
        "Chromatic Star",
        "Millikin",
        "Deranged Assistant",
        "Darkwater Egg",
        "Gilded Lotus",
        "Lotus Petal",
        "Goldhound",
        "Bog Witch",
        "Priest of Titania",
        "Gaea's Cradle",
        "Mishra's Workshop",
        "Omen Hawker",
        "Gilded Goose",
        "Argothian Opportunist",
    ] {
        let c = card(name);
        let expected = serde_json::to_value(&c).unwrap();
        let bytes = bincode::encode_to_vec(&c, bincode::config::standard()).unwrap();
        let (bin, used): (Card, usize) =
            bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
        assert_eq!(used, bytes.len());
        for loaded in [
            bin,
            ron::from_str(&ron::ser::to_string(&c).unwrap()).unwrap(),
            serde_json::from_value(expected.clone()).unwrap(),
        ] {
            validate::card(&loaded).unwrap();
            assert_eq!(serde_json::to_value(loaded).unwrap(), expected);
        }
    }
    let star = card("Chromatic Star");
    assert!(matches!(&star.abilities[1], Ability::Triggered {
        event: EventSpec::ZoneChange { from: Some(EventZone::Battlefield), to: EventZone::Graveyard, .. },
        body: Program { instructions }, ..
    } if matches!(instructions.as_slice(), [Instruction::Effect { operation: Operation::Draw { amount: Expr::Constant(1), .. }, .. }])));
}
#[test]
fn classification_accounts_for_targets_loyalty_library_moves_and_nested_definitions() {
    for name in [
        "Llanowar Elves",
        "Birds of Paradise",
        "Boreal Druid",
        "Sol Ring",
        "Bog Witch",
    ] {
        let c = card(name);
        let a = c
            .abilities
            .iter()
            .find(|a| matches!(a, Ability::Activated { .. }))
            .unwrap();
        assert_eq!(kind(a), ActivationKind::Mana, "{name}");
    }
    for name in [
        "Chromatic Sphere",
        "Millikin",
        "Deranged Assistant",
        "Darkwater Egg",
    ] {
        let c = card(name);
        assert_eq!(kind(&c.abilities[0]), ActivationKind::Stack, "{name}");
    }
    let stone = card("Mind Stone");
    assert_eq!(kind(&stone.abilities[0]), ActivationKind::Mana);
    assert_eq!(kind(&stone.abilities[1]), ActivationKind::Stack);
    let goose = card("Gilded Goose");
    assert_eq!(kind(&goose.abilities[2]), ActivationKind::Stack);
    assert_eq!(kind(&goose.abilities[3]), ActivationKind::Mana);
    let mut c = card("Llanowar Elves");
    let Ability::Activated {
        cost,
        announcement,
        body,
        ..
    } = &mut c.abilities[0]
    else {
        panic!()
    };
    announcement.targets.push(TargetSpec {
        slot: Slot(0),
        kind: TargetKind::Object(ObjectDomain::permanent()),
        count: Count::UpTo(Expr::Constant(1)),
        filter: ObjectFilter::Any,
        players: None,
        distribute: None,
        chooser: None,
        constraints: vec![],
    });
    assert_eq!(
        activation_kind(cost, announcement, body),
        ActivationKind::Stack
    );
    announcement.targets.clear();
    cost.parts.push(CostPart::Loyalty(Expr::Constant(1)));
    assert_eq!(
        activation_kind(cost, announcement, body),
        ActivationKind::Stack
    );
    // A creature with a mana ability entering is not itself a mana-producing
    // effect of the ability that created it.
    let mut c = card("Gilded Goose");
    let Ability::Activated {
        cost,
        announcement,
        body,
        ..
    } = &mut c.abilities[2]
    else {
        panic!()
    };
    let Instruction::Effect {
        operation: Operation::CreateTokens { tokens, .. },
        ..
    } = &mut body.instructions[0]
    else {
        panic!()
    };
    tokens[0].definition_mut().unwrap().abilities = card("Llanowar Elves").abilities;
    assert_eq!(
        activation_kind(cost, announcement, body),
        ActivationKind::Stack
    );
}
#[test]
fn one_color_repetition_and_any_combination_preserve_different_choice_timing() {
    let lotus = card("Gilded Lotus");
    let Ability::Activated { body, .. } = &lotus.abilities[0] else {
        panic!()
    };
    assert!(matches!(
        body.instructions.as_slice(),
        [Instruction::Effect {
            operation: Operation::AddMana {
                mana: ManaProduction::Repeat {
                    amount: Expr::Constant(3),
                    choice: ManaChoiceTiming::Once,
                    ..
                },
                ..
            },
            ..
        }]
    ));
    let morph = card("Manamorphose");
    let Ability::Spell { body, .. } = &morph.abilities[0] else {
        panic!()
    };
    assert!(matches!(
        &body.instructions[0],
        Instruction::Effect {
            operation: Operation::AddMana {
                mana: ManaProduction::Repeat {
                    amount: Expr::Constant(2),
                    choice: ManaChoiceTiming::EachUnit,
                    ..
                },
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        &body.instructions[1],
        Instruction::Effect {
            operation: Operation::Draw {
                amount: Expr::Constant(1),
                ..
            },
            ..
        }
    ));
}
#[test]
fn spending_predicates_and_milling_costs_survive_lowering() {
    let workshop = card("Mishra's Workshop");
    let Ability::Activated { body, .. } = &workshop.abilities[0] else {
        panic!()
    };
    assert!(matches!(
        &body.instructions[0],
        Instruction::Effect {
            operation: Operation::AddMana {
                restriction: Some(ManaUse::CastSpell(ObjectFilter::CardType(
                    source::CardType::Artifact
                ))),
                ..
            },
            ..
        }
    ));
    let hawker = card("Omen Hawker");
    let Ability::Activated { body, .. } = &hawker.abilities[0] else {
        panic!()
    };
    assert!(matches!(
        &body.instructions[0],
        Instruction::Effect {
            operation: Operation::AddMana {
                restriction: Some(ManaUse::ActivateAbility {
                    sources: ObjectFilter::Any,
                    permanents_only: false
                }),
                ..
            },
            ..
        }
    ));
    let mill = card("Millikin");
    let Ability::Activated { cost, .. } = &mill.abilities[0] else {
        panic!()
    };
    assert!(cost.parts.iter().any(|c| matches!(
        c,
        CostPart::Mill {
            count: Expr::Constant(1)
        }
    )));
    #[cfg(feature = "cantrip")]
    assert!(mtgish_ast::cantrip::module(&mill)
        .unwrap_err()
        .feature
        .contains("cost part"));
}
