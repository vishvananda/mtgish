use mtgish_ast::{corpus, ir::*, mtg_types as source, pipeline, validate};

fn custom(rule: &str) -> source::OracleCard {
    let mut card: source::OracleCard = ron::from_str(
        include_str!("fixtures/pipeline.ronl")
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    let source::OracleCard::Card { rules, .. } = &mut card else {
        panic!()
    };
    *rules = Some(vec![ron::from_str(rule).unwrap()]);
    card
}
fn activated(card: &Card) -> (&Cost, &Program) {
    let Ability::Activated { cost, body, .. } = &card.abilities[0] else {
        panic!()
    };
    (cost, body)
}
fn roundtrip(card: &Card) {
    let bytes = bincode::encode_to_vec(card, bincode::config::standard()).unwrap();
    let (binary, used): (Card, usize) =
        bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
    assert_eq!(used, bytes.len());
    for value in [
        binary,
        ron::from_str(&ron::ser::to_string(card).unwrap()).unwrap(),
        serde_json::from_value(serde_json::to_value(card).unwrap()).unwrap(),
    ] {
        validate::card(&value).unwrap();
        assert_eq!(
            serde_json::to_value(value).unwrap(),
            serde_json::to_value(card).unwrap()
        );
    }
}

#[test]
fn grouped_payments_attach_totals_to_the_chosen_inputs_and_keep_strict_counts() {
    let source=custom("Activated(TapNumberGroupPermanents(Integer(2),AnyPermanent,TotalPowerIs(GreaterThanOrEqualTo(Integer(10)))),ActionList([DrawACard]))");
    let staged = pipeline::Intermediate::new(&source).unwrap();
    let direct = pipeline::finalize(&source).unwrap();
    assert_eq!(
        serde_json::to_value(&direct).unwrap(),
        serde_json::to_value(staged.final_card.unwrap()).unwrap()
    );
    let [CostPart::Constrained { part, constraints }] = activated(&direct).0.parts.as_slice()
    else {
        panic!()
    };
    assert!(
        matches!(part.as_ref(),CostPart::ChangeTapState {tapped:true,symbol:false,selections} if matches!(selections.as_slice(),[CostObjectSelection {choose:Some(Count::Exactly(Expr::Constant(2))),..}]))
    );
    assert!(matches!(
        constraints.as_slice(),
        [SelectionConstraint::TotalProperty {
            property: Property::Power,
            comparison: Comparison::GreaterOrEqual,
            amount: Expr::Constant(10)
        }]
    ));
    // Eligibility contains no total: meeting the threshold with unchosen objects
    // cannot satisfy a payment. The strict count lives inside that same component.
    roundtrip(&direct);
    assert_eq!(
        bincode::encode_to_vec(&activated(&direct).0.parts[0], bincode::config::standard())
            .unwrap()[0],
        13
    );
}

#[test]
fn shared_group_constraints_cover_distinct_values_common_values_and_nonbasic_land_types() {
    for (group, kind) in [
        ("DifferentNames", "DifferentNames"),
        ("DifferentManaValues", "DifferentProperty"),
        ("DifferentPowers", "DifferentProperty"),
        ("SameNames", "CommonProperty"),
        ("ShareACreatureType", "CommonProperty"),
        ("ShareACardType", "CommonProperty"),
        ("ShareAColor", "CommonProperty"),
        ("ShareALandType", "CommonProperty"),
    ] {
        let source = custom(&format!(
            "Activated(DiscardNumberGroupCards(Integer(2),{group}),ActionList([DrawACard]))"
        ));
        let card = pipeline::finalize(&source).unwrap();
        let [CostPart::Constrained { constraints, .. }] = activated(&card).0.parts.as_slice()
        else {
            panic!()
        };
        let value = serde_json::to_value(&constraints[0]).unwrap();
        assert!(value == kind || value.get(kind).is_some());
        if group == "ShareALandType" {
            assert!(matches!(
                constraints[0],
                SelectionConstraint::CommonProperty(DistinctProperty::LandTypes)
            ));
        }
        roundtrip(&card);
    }
}

#[test]
fn unsupported_group_qualifications_survive_staging_and_block_native_export() {
    for group in [
        "SharesNoCreatureTypes",
        "ShareACreatureTypeOfChoice",
        "ShareAllCardTypes",
    ] {
        let source=custom(&format!("Activated(TapAnyNumberOfGroupPermanents(AnyPermanent,{group}),ActionList([DrawACard]))"));
        let staged = pipeline::Intermediate::new(&source).unwrap();
        assert!(staged.structural.operands.iter().any(|r| matches!(
            &r.recipe,
            pipeline::operands::OperandRecipe::Cost(
                pipeline::operands::CostRecipe::Constrained { .. }
            )
        )));
        assert!(staged.final_card.is_none());
        assert!(staged
            .diagnostics
            .iter()
            .any(|d| d.source_type == "GroupFilter" && d.form == group));
        assert_eq!(
            mtgish_ast::inspect::value(&staged.structural.source.restore().unwrap()).unwrap(),
            mtgish_ast::inspect::value(&source).unwrap()
        );
    }
}

#[test]
fn payment_group_validation_rejects_ignored_constraints_and_effect_only_choices() {
    let base=pipeline::finalize(&custom("Activated(TapNumberGroupPermanents(Integer(2),AnyPermanent,DifferentNames),ActionList([DrawACard]))")).unwrap();
    for case in 0..5 {
        let mut card = base.clone();
        let Ability::Activated { cost, .. } = &mut card.abilities[0] else {
            panic!()
        };
        let CostPart::Constrained { part, constraints } = &mut cost.parts[0] else {
            panic!()
        };
        match case {
            0 => constraints.clear(),
            1 => *part = Box::new(CostPart::Mana(vec![])),
            2 => *part = Box::new(CostPart::TapSymbol),
            3 => *part = Box::new(CostPart::Choice(vec![Cost { parts: vec![] }])),
            _ => {
                let CostPart::ChangeTapState { selections, .. } = part.as_mut() else {
                    panic!()
                };
                selections[0].choose = Some(Count::AsManyAsCan(Expr::Constant(2)));
            }
        }
        assert!(validate::card(&card).is_err(), "case {case}");
    }
}

#[test]
fn crew_expands_to_shared_forms_with_payment_identity_and_fresh_scope() {
    for rule in ["Crew(0)", "Crew(3)", "CrewOnceEachTurn(1)"] {
        let source = custom(rule);
        let stage = pipeline::Intermediate::new(&source).unwrap();
        assert_eq!(stage.structural.mechanics.len(), 1);
        let card = pipeline::bind(&stage.structural).unwrap();
        let Ability::Activated {
            origin: ActivationOrigin::Crew,
            cost,
            limits,
            announcement,
            body,
        } = &card.abilities[0]
        else {
            panic!()
        };
        assert_eq!(limits.once_each_turn, rule.starts_with("CrewOnce"));
        assert!(!limits.sorcery && announcement.targets.is_empty());
        assert!(
            matches!(cost.parts.as_slice(),[CostPart::Constrained {part,..}] if matches!(part.as_ref(),CostPart::ChangeTapState {symbol:false,tapped:true,selections} if matches!(selections.as_slice(),[CostObjectSelection {choose:Some(Count::AnyNumber),..}])))
        );
        assert!(matches!(
            body.instructions.as_slice(),
            [Instruction::Effect {
                result: ResultId(0),
                operation: Operation::CreateContinuousEffect {
                    source: ObjectRef::Source,
                    affected: EffectSubjects::Objects(ObjectSet::One(ObjectRef::Source)),
                    duration: Duration::EndOfTurn,
                    evaluation: EffectEvaluation::Snapshot,
                    ..
                }
            }]
        ));
        roundtrip(&card);
    }
    assert!(pipeline::finalize(&custom("Crew(-1)")).is_err());
}

#[test]
fn crew_validation_preserves_other_untapped_controlled_creatures_and_resolution_timing() {
    let base = pipeline::finalize(&custom("Crew(2)")).unwrap();
    for case in 0..8 {
        let mut card = base.clone();
        let Ability::Activated {
            cost, limits, body, ..
        } = &mut card.abilities[0]
        else {
            panic!()
        };
        let CostPart::Constrained { part, constraints } = &mut cost.parts[0] else {
            panic!()
        };
        let CostPart::ChangeTapState {
            selections, symbol, ..
        } = part.as_mut()
        else {
            panic!()
        };
        match case {
            0..=3 => {
                let ObjectSet::Battlefield(ObjectFilter::All(filters)) = &mut selections[0].objects
                else {
                    panic!()
                };
                filters.remove(case);
            }
            4 => constraints.clear(),
            5 => *symbol = true,
            6 => limits.sorcery = true,
            _ => body.instructions.clear(),
        }
        assert!(validate::card(&card).is_err(), "case {case}");
    }
}

#[test]
fn saved_mechanic_artifacts_verify_recipe_identity_and_registry() {
    let stage = pipeline::Intermediate::new(&custom("Crew(2)")).unwrap();
    let loaded: pipeline::Intermediate =
        serde_json::from_value(serde_json::to_value(stage).unwrap()).unwrap();
    assert!(pipeline::bind(&loaded.structural).is_ok());
    for case in 0..3 {
        let mut structural = loaded.structural.clone();
        match case {
            0 => structural.mechanics[0].recipe.threshold += 1,
            1 => structural.mechanics[0].source_key = "another source".into(),
            _ => {
                structural
                    .registries
                    .remove("rust_syntax/normalize/rewrites/mechanics.json5");
            }
        }
        assert!(pipeline::bind(&structural).is_err());
    }
    let original = serde_json::to_value(pipeline::mechanics::registry()).unwrap();
    for case in 0..5 {
        let mut value = original.clone();
        match case {
            0 => value["version"] = 2.into(),
            1 => value["rules"][0]["arity"] = 2.into(),
            2 => value["rules"][0]["threshold"] = 1.into(),
            3 => value["rules"][0]["ignored"] = true.into(),
            _ => value["rules"][1] = value["rules"][0].clone(),
        }
        assert!(mtgish_ast::mechanic_spec::Registry::parse(&value.to_string()).is_err());
    }
}

#[test]
fn real_group_and_crew_cards_keep_unrelated_missing_abilities_visible() {
    let mut complete = 0;
    for line in include_str!("fixtures/groups.ronl").lines() {
        let source: source::OracleCard = ron::from_str(line).unwrap();
        let name = corpus::Identity::of(&source).unwrap().names.remove(0);
        match pipeline::finalize(&source) {
            Ok(card) => {
                roundtrip(&card);
                complete += 1;
                println!("complete: {name}");
            }
            Err(errors) => {
                assert!(
                    [
                        "Heart of Kiran",
                        "Luxurious Locomotive",
                        "Mossbridge Troll",
                        "Shorikai, Genesis Engine",
                        "Colossal Plow"
                    ]
                    .contains(&name.as_str()),
                    "{name}: {errors:?}"
                );
                assert!(
                    errors
                        .iter()
                        .all(|d| d.form != "Crew" && d.form != "CrewOnceEachTurn"),
                    "{name}"
                );
                println!("incomplete: {name}: {errors:?}");
            }
        }
    }
    assert_eq!(complete, 12);
}

#[test]
fn grouped_cost_adapters_preserve_their_underlying_payment_and_every_field() {
    for cost in [
        "TapNumberGroupPermanents(Integer(2),AnyPermanent,ShareACreatureType)",
        "TapAnyNumberOfGroupPermanents(AnyPermanent,TotalPowerIs(GreaterThanOrEqualTo(Integer(10))))",
        "SacrificeNumberGroupPermanents(Integer(2),AnyPermanent,TotalManaValueIs(LessThanOrEqualTo(Integer(5))))",
        "SacrificeAnyNumberOfGroupPermanents(AnyPermanent,ShareACardType)",
        "DiscardNumberGroupCards(Integer(2),SameNames)",
        "DiscardNumberGroupCardsOfType(Integer(2),AnyCard,ShareAColor)",
    ] {
        let source=custom(&format!("Activated({cost},ActionList([DrawACard]))"));
        let stage=pipeline::Intermediate::new(&source).unwrap();
        let card=stage.final_card.unwrap();
        let [CostPart::Constrained {part,constraints}]=activated(&card).0.parts.as_slice() else {panic!()};
        assert_eq!(constraints.len(),1);
        assert!(match cost.split('(').next().unwrap() {
            "TapNumberGroupPermanents"|"TapAnyNumberOfGroupPermanents"=>matches!(part.as_ref(),CostPart::ChangeTapState {tapped:true,symbol:false,..}),
            "SacrificeNumberGroupPermanents"|"SacrificeAnyNumberOfGroupPermanents"=>matches!(part.as_ref(),CostPart::Sacrifice {..}),
            _=>matches!(part.as_ref(),CostPart::Discard {random:false,count:Expr::Constant(2),..}),
        });
        if cost.starts_with("SacrificeNumber") {assert!(matches!(constraints.as_slice(),[SelectionConstraint::TotalProperty {property:Property::ManaValue,comparison:Comparison::LessOrEqual,amount:Expr::Constant(5)}]));}
        roundtrip(&card);
    }
    let mut registry = serde_json::to_value(pipeline::operands::registry()).unwrap();
    let row = registry["rules"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|r| r["source"] == "TapNumberGroupPermanents")
        .unwrap();
    row["recipe"]["group"] = 0.into();
    assert!(mtgish_ast::operand_spec::Registry::parse(&registry.to_string()).is_err());
}

#[test]
fn constrained_costs_remain_visible_to_mana_ability_classification() {
    let mut card=pipeline::finalize(&custom("Activated(TapNumberGroupPermanents(Integer(2),AnyPermanent,DifferentNames),ActionList([AddMana(ManaProduceG)]))")).unwrap();
    let Ability::Activated {
        cost,
        announcement,
        body,
        ..
    } = &mut card.abilities[0]
    else {
        panic!()
    };
    assert_eq!(
        mtgish_ast::semantics::activation_kind(cost, announcement, body),
        mtgish_ast::semantics::ActivationKind::Mana
    );
    let CostPart::Constrained { part, .. } = &mut cost.parts[0] else {
        panic!()
    };
    *part = Box::new(CostPart::Move {
        selections: vec![CostObjectSelection {
            objects: ObjectSet::Zone {
                scope: ZoneScope::Players {
                    zone: EventZone::Library,
                    players: PlayerSet::One(PlayerRef::AbilityController),
                },
                kind: ObjectKind::Card,
                filter: ObjectFilter::Any,
            },
            choose: Some(Count::Exactly(Expr::Constant(2))),
        }],
        destination: Destination::Exile,
        entry: EntrySpec::default(),
        face_down: false,
    });
    assert_eq!(
        mtgish_ast::semantics::activation_kind(cost, announcement, body),
        mtgish_ast::semantics::ActivationKind::Stack
    );
    validate::card(&card).unwrap();
}
