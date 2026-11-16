#[path = "support/fresh_producer.rs"]
mod fresh_producer;
#[path = "support/generated_locals.rs"]
mod generated_locals;
#[path = "support/legacy_entries.rs"]
mod legacy_entries;
use mtgish_ast::{
    ir::*,
    movement,
    mtg_types::OracleCard,
    validate,
    visit::{Visitor, Walk},
};
fn you() -> PlayerSet {
    PlayerSet::One(PlayerRef::AbilityController)
}
fn field() -> ObjectSet {
    ObjectSet::battlefield(ObjectFilter::Any)
}
fn picked(objects: ObjectSet) -> ObjectSet {
    ObjectSet::DraftChosen {
        chooser: you(),
        objects: Box::new(objects),
        count: Count::Exactly(Expr::Constant(1)),
    }
}
fn effect(result: u32, operation: Operation) -> Instruction {
    Instruction::Effect {
        result: ResultId(result),
        operation,
    }
}
fn tap(objects: ObjectSet) -> Operation {
    Operation::SetTapped {
        objects,
        tapped: true,
    }
}
fn program(instructions: Vec<Instruction>) -> Program {
    Program { instructions }
}
fn card(body: Program) -> Card {
    let source:OracleCard=ron::from_str(r#"Card(Name: "Preparation", Typeline: (Supertypes: [], Cardtypes: [Sorcery], Subtypes: []), Rules: [SpellActions(ActionList([DrawACard]))])"#).unwrap();
    let mut card = mtgish_ast::lower::card(&source).unwrap();
    card.abilities = vec![Ability::Spell {
        announcement: Announcement::empty(),
        body,
    }];
    card
}
fn action(instruction: &Instruction) -> &Operation {
    let Instruction::Effect { operation, .. } = instruction else {
        panic!("{instruction:?}")
    };
    operation
}
#[test]
fn nested_choices_are_prepared_in_dependency_order_with_distinct_result_provenance() {
    let mut p = program(vec![effect(
        0,
        tap(picked(ObjectSet::DraftRandom {
            objects: Box::new(field()),
            count: Expr::Constant(3),
        })),
    )]);
    movement::normalize_program(&mut p);
    let Operation::WithSelections { steps, action } = action(&p.instructions[0]) else {
        panic!("{p:?}")
    };
    assert_eq!(steps.len(), 2);
    assert!(matches!(steps[0].chooser, SelectionChooser::Chance));
    assert!(matches!(
        steps[1].objects,
        ObjectSet::Prepared(SelectionId(0))
    ));
    assert!(matches!(
        **action,
        Operation::SetTapped {
            objects: ObjectSet::Prepared(SelectionId(1)),
            ..
        }
    ));
    assert!(matches!(
        p.instructions[0],
        Instruction::Effect {
            result: ResultId(0),
            ..
        }
    ));
    validate::card(&card(p.clone())).unwrap();
    let before = serde_json::to_value(&p).unwrap();
    movement::normalize_program(&mut p);
    assert_eq!(before, serde_json::to_value(&p).unwrap());
}
#[test]
fn preparation_stays_at_the_nearest_nested_operation_and_future_program() {
    let delayed = Operation::CreateDelayedTrigger {
        event: Box::new(EventSpec::Turn {
            point: TurnPoint::EndStepBegins,
            players: PlayerSet::All,
        }),
        start: DelayedStart::Immediately,
        expires: None,
        intervening_if: None,
        body: program(vec![effect(7, tap(picked(field())))]),
    };
    let mut p = program(vec![effect(
        0,
        Operation::InOrder(vec![
            Operation::Draw {
                players: you(),
                amount: Expr::Constant(1),
            },
            tap(picked(field())),
            delayed,
        ]),
    )]);
    movement::normalize_program(&mut p);
    let Operation::InOrder(ops) = action(&p.instructions[0]) else {
        panic!("{p:?}")
    };
    assert!(matches!(ops[0], Operation::Draw { .. }));
    assert!(matches!(ops[1], Operation::WithSelections { .. }));
    let Operation::CreateDelayedTrigger { body, .. } = &ops[2] else {
        panic!()
    };
    assert!(matches!(
        action(&body.instructions[0]),
        Operation::WithSelections { .. }
    ));
    validate::card(&card(p)).unwrap();
}
#[test]
fn costs_targets_and_filter_predicates_do_not_become_resolution_picks() {
    let random = ObjectSet::DraftRandom {
        objects: Box::new(field()),
        count: Expr::Constant(2),
    };
    let mut p = program(vec![
        effect(
            1,
            Operation::PayCost {
                player: PlayerRef::AbilityController,
                cost: Cost {
                    parts: vec![CostPart::Sacrifice {
                        objects: random.clone(),
                        choose: Some(Count::Exactly(Expr::Constant(2))),
                    }],
                },
                optional: false,
            },
        ),
        effect(
            2,
            tap(ObjectSet::battlefield(ObjectFilter::InSet(Box::new(
                random,
            )))),
        ),
    ]);
    let before = serde_json::to_value(&p).unwrap();
    movement::normalize_program(&mut p);
    assert_eq!(before, serde_json::to_value(&p).unwrap());
}
#[test]
fn local_selection_bindings_cannot_escape_or_forward_reference_and_duplicates_reject() {
    let step = SelectionStep {
        binding: SelectionId(0),
        chooser: SelectionChooser::Players(you()),
        objects: field(),
        count: Count::Exactly(Expr::Constant(1)),
        constraints: vec![],
    };
    let with = |steps| Operation::WithSelections {
        steps,
        action: Box::new(tap(ObjectSet::Prepared(SelectionId(0)))),
    };
    validate::card(&card(program(vec![effect(0, with(vec![step.clone()]))]))).unwrap();
    let bad = card(program(vec![
        effect(0, with(vec![step.clone()])),
        effect(1, tap(ObjectSet::Prepared(SelectionId(0)))),
    ]));
    assert!(validate::card(&bad)
        .unwrap_err()
        .contains("outside its preparation scope"));
    let mut forward = step.clone();
    forward.objects = ObjectSet::Prepared(SelectionId(0));
    assert!(validate::card(&card(program(vec![effect(0, with(vec![forward]))]))).is_err());
    assert!(validate::card(&card(program(vec![effect(
        0,
        with(vec![step.clone(), step])
    )])))
    .unwrap_err()
    .contains("duplicate local selection"));
}
#[test]
fn prepared_pool_zone_guards_survive_scope_and_codec_roundtrips() {
    let operation = Operation::WithSelections {
        steps: vec![SelectionStep {
            binding: SelectionId(5),
            chooser: SelectionChooser::Chance,
            objects: ObjectSet::Zone {
                scope: ZoneScope::Players {
                    players: you(),
                    zone: EventZone::Graveyard,
                },
                kind: ObjectKind::Card,
                filter: ObjectFilter::Any,
            },
            count: Count::AsManyAsCan(Expr::Constant(1)),
            constraints: vec![],
        }],
        action: Box::new(tap(ObjectSet::Prepared(SelectionId(5)))),
    };
    let c = card(program(vec![effect(0, operation)]));
    assert!(validate::card(&c).unwrap_err().contains("Battlefield"));
    let bytes = bincode::encode_to_vec(&c, bincode::config::standard()).unwrap();
    let (decoded, used): (Card, usize) =
        bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
    assert_eq!(used, bytes.len());
    assert_eq!(
        serde_json::to_value(c).unwrap(),
        serde_json::to_value(decoded).unwrap()
    );
}
#[test]
fn all_captured_producer_selectors_have_lexical_execution_sites_and_stable_normalization() {
    struct Counts {
        steps: usize,
        prepared: usize,
    }
    impl Visitor for Counts {
        fn operation(&mut self, op: &Operation) -> bool {
            if let Operation::WithSelections { steps, .. } = op {
                self.steps += steps.len();
            }
            true
        }
        fn object_set(&mut self, objects: &ObjectSet) -> bool {
            if matches!(objects, ObjectSet::Prepared(_)) {
                self.prepared += 1;
            }
            true
        }
    }
    let mut counts = Counts {
        steps: 0,
        prepared: 0,
    };
    for line in include_str!("fixtures/movement_selections.ronl").lines() {
        let source: OracleCard = ron::from_str(line).unwrap();
        let mut lowered = mtgish_ast::unified::lower(&source);
        assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
        lowered.card.walk(&mut counts);
        let before = serde_json::to_value(&lowered.card).unwrap();
        movement::normalize_unified(&mut lowered.card);
        assert_eq!(before, serde_json::to_value(&lowered.card).unwrap());
    }
    assert_eq!(counts.steps, 41);
    assert_eq!(counts.prepared, 41);
}

#[test]
fn complete_captured_old_records_keep_all_operands_except_reviewed_domain_corrections() {
    let old = include_str!("fixtures/movement_selections_before.ronl").lines();
    let source = include_str!("fixtures/movement_selections.ronl").lines();
    for (before, line) in old.zip(source) {
        let mut expected: UnifiedCard = legacy_entries::ron(before);
        expected.schema_version = SCHEMA_VERSION;
        for f in &mut expected.faces {
            f.schema_version = SCHEMA_VERSION;
        }
        movement::normalize_unified(&mut expected);
        // Preserve the captured historical IR. These exact filters were emitted
        // by the old Permanents::IsPermanent rule; its six-type restriction
        // excluded typeless battlefield permanents. Every other field must
        // still agree with the captured migration expectation.
        let corrections: &[&str] = match expected.faces[0].name.as_str() {
            "Chaos Defiler" => &[
                "/faces/0/abilities/1/args/body/instructions/0/args/body/instructions/0/args/operation/args/objects/args/filter/args/1",
                "/faces/0/abilities/2/args/body/instructions/0/args/body/instructions/0/args/operation/args/objects/args/filter/args/1",
            ],
            "Descent into Madness" => &[
                "/faces/0/abilities/0/args/body/instructions/1/args/body/instructions/0/args/operation/args/steps/0/objects/args/0/args/filter/args/0",
            ],
            _ => &[],
        };
        let mut expected = serde_json::to_value(expected).unwrap();
        for pointer in corrections {
            let old_filter = expected.pointer_mut(pointer).unwrap();
            assert_eq!(
                *old_filter,
                serde_json::json!({"kind":"AnyOf","args":[
                    {"kind":"CardType","args":"Artifact"},
                    {"kind":"CardType","args":"Battle"},
                    {"kind":"CardType","args":"Creature"},
                    {"kind":"CardType","args":"Enchantment"},
                    {"kind":"CardType","args":"Land"},
                    {"kind":"CardType","args":"Planeswalker"}
                ]})
            );
            *old_filter = serde_json::json!({"kind":"Any"});
        }
        let source: OracleCard = ron::from_str(line).unwrap();
        let actual = mtgish_ast::unified::lower(&source);
        assert!(actual.diagnostics.is_empty());
        fresh_producer::assert_matches("movement_selections_before.ronl", &expected, &serde_json::to_value(actual.card).unwrap());
    }
}
#[test]
fn granted_ability_local_choices_stay_inside_their_independent_program() {
    let grant = Operation::CreateContinuousEffect {
        source: ObjectRef::Source,
        affected: EffectSubjects::Objects(ObjectSet::One(ObjectRef::Source)),
        changes: vec![Change::GrantAbility(Box::new(Ability::Spell {
            announcement: Announcement::empty(),
            body: program(vec![effect(0, tap(picked(field())))]),
        }))],
        duration: Duration::EndOfTurn,
        evaluation: EffectEvaluation::Snapshot,
    };
    let mut p = program(vec![effect(0, grant)]);
    movement::normalize_program(&mut p);
    let Operation::CreateContinuousEffect { changes, .. } = action(&p.instructions[0]) else {
        panic!("{p:?}")
    };
    let Change::GrantAbility(a) = &changes[0] else {
        panic!()
    };
    let Ability::Spell { body, .. } = a.as_ref() else {
        panic!()
    };
    assert!(matches!(
        action(&body.instructions[0]),
        Operation::WithSelections { .. }
    ));
}

#[test]
fn simultaneous_members_prepare_all_choices_before_mutation_without_aliasing_bindings() {
    let mut p = program(vec![effect(
        0,
        Operation::Together(vec![
            tap(picked(field())),
            Operation::Destroy {
                objects: picked(field()),
                regeneration: true,
            },
        ]),
    )]);
    movement::normalize_program(&mut p);
    let Operation::WithSelections { steps, action } = action(&p.instructions[0]) else {
        panic!("{p:?}")
    };
    assert_eq!(steps.len(), 2);
    assert_eq!((steps[0].binding.0, steps[1].binding.0), (0, 1));
    let Operation::Together(members) = action.as_ref() else {
        panic!()
    };
    assert!(matches!(
        members.as_slice(),
        [
            Operation::SetTapped {
                objects: ObjectSet::Prepared(SelectionId(0)),
                ..
            },
            Operation::Destroy {
                objects: ObjectSet::Prepared(SelectionId(1)),
                ..
            }
        ]
    ));
    validate::card(&card(p.clone())).unwrap();
    let before = serde_json::to_value(&p).unwrap();
    movement::normalize_program(&mut p);
    assert_eq!(before, serde_json::to_value(&p).unwrap());
}


#[test]
fn test_shape_comparison_changes_only_scratch_counters_and_keeps_payload_slots() {
    assert!(generated_locals::contains(
        "(carried_4_0,) = (selected_3.clone(),);",
        "(carried_5_0,) = (selected_4.clone(),);"
    ));
    assert!(!generated_locals::contains(
        "carried_4_1.clone()",
        "carried_5_0.clone()"
    ));
    assert!(!generated_locals::contains("bound_4", "bound_3"));
}
