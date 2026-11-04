use mtgish_ast::{
    ir::*,
    mtg_types::OracleCard,
    result_reads::ResultReads,
    unified, validate,
    visit::{Visitor, Walk},
};

fn membership(result: u32) -> ObjectFilter {
    ObjectFilter::InSet(Box::new(ObjectSet::Named(ResultId(result))))
}
fn effect(operation: Operation) -> Instruction {
    Instruction::Effect {
        result: ResultId(100),
        operation,
    }
}
fn program(instruction: Instruction) -> Program {
    Program {
        instructions: vec![instruction],
    }
}
fn consume(objects: ObjectSet) -> Instruction {
    effect(Operation::Tap {
        objects,
        tapped: true,
    })
}
fn test_filter(filter: ObjectFilter) -> Instruction {
    Instruction::If {
        condition: Condition::ObjectMatches {
            object: ObjectRef::Source,
            filter,
        },
        then: Program::empty(),
        otherwise: Program::empty(),
    }
}
fn read(after: Instruction) -> ResultReads {
    ResultReads::instructions(&[after], &ResultId(3))
}
fn number(expr: Expr) -> Instruction {
    effect(Operation::Draw {
        players: PlayerSet::One(PlayerRef::AbilityController),
        amount: expr,
    })
}

#[test]
fn membership_is_distinct_from_plain_objects_and_kept_on_top() {
    let reads = read(test_filter(membership(3)));
    assert!(reads.membership && reads.objects && reads.graveyard());
    assert!(!reads.kept_on_top);
    let reads = read(consume(ObjectSet::Named(ResultId(3))));
    assert!(reads.objects);
    assert!(!reads.graveyard() && !reads.kept_on_top);
    let reads = read(number(Expr::SurveilKeptOnTop(ResultId(3))));
    assert!(reads.kept_on_top);
    assert!(!reads.objects && !reads.graveyard());
    assert_eq!(read(test_filter(membership(4))), ResultReads::default());
}

#[test]
fn independent_granted_abilities_and_their_costs_do_not_read_current_results() {
    // This ability defines result 3 locally. Its membership and kept-on-top
    // expressions refer to that local result, not an outer result with id 3.
    let local_body = Program {
        instructions: vec![
            Instruction::Effect {
                result: ResultId(3),
                operation: Operation::Surveil {
                    players: PlayerSet::One(PlayerRef::AbilityController),
                    amount: Expr::Constant(1),
                },
            },
            test_filter(membership(3)),
            number(Expr::SurveilKeptOnTop(ResultId(3))),
        ],
    };
    let local = Ability::Activated {
        origin: ActivationOrigin::Ordinary,
        cost: Cost {
            parts: vec![CostPart::PlayerAction {
                player: PlayerRef::AbilityController,
                body: local_body.clone(),
            }],
        },
        limits: ActivationLimits::unrestricted(),
        announcement: Announcement::empty(),
        body: local_body,
    };
    let instruction = effect(Operation::CreateContinuousEffect {
        source: ObjectRef::Source,
        affected: EffectSubjects::Objects(ObjectSet::One(ObjectRef::Source)),
        evaluation: EffectEvaluation::Snapshot,
        changes: vec![Change::GrantAbility(Box::new(local))],
        duration: Duration::EndOfTurn,
    });
    assert_eq!(read(instruction.clone()), ResultReads::default());
    let source:OracleCard=ron::from_str(r#"Card(Name: "Scopes", Typeline: (Supertypes: [], Cardtypes: [Sorcery], Subtypes: []), Rules: [SpellActions(ActionList([DrawACard]))])"#).unwrap();
    let mut card = mtgish_ast::lower::card(&source).unwrap();
    let Ability::Spell { body, .. } = &mut card.abilities[0] else {
        panic!()
    };
    body.instructions = vec![
        Instruction::Effect {
            result: ResultId(3),
            operation: Operation::Surveil {
                players: PlayerSet::One(PlayerRef::AbilityController),
                amount: Expr::Constant(1),
            },
        },
        instruction,
    ];
    validate::card(&card).unwrap();
}

#[test]
fn inline_payment_selections_and_nested_control_flow_are_current_reads() {
    let cost = effect(Operation::PayCost {
        player: PlayerRef::AbilityController,
        cost: Cost {
            parts: vec![CostPart::Sacrifice {
                objects: ObjectSet::Battlefield(membership(3)),
                choose: Some(Count::Exactly(Expr::Constant(1))),
            }],
        },
        optional: false,
    });
    let instruction = Instruction::Optional {
        decider: PlayerRef::AbilityController,
        body: program(Instruction::Group(program(cost))),
    };
    assert!(read(instruction).graveyard());
}

#[test]
fn delayed_and_reflexive_programs_capture_the_creator_result() {
    let body = program(test_filter(membership(3)));
    let reflexive = effect(Operation::ReflexiveTrigger {
        performed: None,
        condition: None,
        announcement: Announcement::empty(),
        body: body.clone(),
    });
    assert!(read(reflexive).graveyard());
    let delayed = effect(Operation::CreateDelayedTrigger {
        event: EventSpec::Turn {
            point: TurnPoint::EndStepBegins,
            players: PlayerSet::All,
        },
        start: DelayedStart::Immediately,
        expires: None,
        intervening_if: None,
        body,
    });
    assert!(read(delayed).graveyard());
}

#[test]
fn typed_operands_are_visited_but_text_cannot_impersonate_a_result_read() {
    let instruction = effect(Operation::Primitive {
        actor: PlayerSet::One(PlayerRef::AbilityController),
        name: "InResult(ResultId(3)) Named(ResultId(3)) SurveilKeptOnTop(ResultId(3))".into(),
        operands: vec![Operand::Text("InResult(ResultId(3))".into())],
    });
    assert_eq!(read(instruction), ResultReads::default());
    let instruction = effect(Operation::Primitive {
        actor: PlayerSet::One(PlayerRef::AbilityController),
        name: "query".into(),
        operands: vec![Operand::Record(vec![Field {
            name: "filter".into(),
            value: Operand::List(vec![Operand::Filter(membership(3))]),
        }])],
    });
    assert!(read(instruction).graveyard());
}

#[test]
fn result_views_keep_selected_observed_and_created_provenance() {
    for objects in [
        ObjectSet::Selected(ResultId(3)),
        ObjectSet::Observed(ResultId(3)),
        ObjectSet::Created(ResultId(3)),
    ] {
        assert_eq!(read(consume(objects)), ResultReads::default());
    }
    assert!(
        read(consume(ObjectSet::Transition {
            result: ResultId(3),
            phase: ObjectPhase::After
        }))
        .objects
    );
    assert!(read(consume(ObjectSet::Affected(ResultId(3)))).objects);
    assert!(
        read(consume(ObjectSet::One(ObjectRef::Named {
            result: ResultId(3),
            by: None
        })))
        .objects
    );
}

#[test]
fn graveyard_counts_and_joined_objects_have_explicit_uses() {
    let reads = read(number(Expr::Earlier {
        what: EarlierObjects::PutInGraveyard,
        measure: ObjectMeasure::Count,
        result: Some(ResultId(3)),
        filter: None,
        by: None,
    }));
    assert!(reads.graveyard_count && reads.graveyard());
    assert!(!reads.membership);
    assert!(
        read(Instruction::Group(program(effect(
            Operation::JoinResults {
                results: vec![ResultId(3), ResultId(4)]
            }
        ))))
        .objects
    );
}

#[test]
fn membership_records_validate_and_roundtrip_with_the_same_result_views() {
    let mut membership_nodes = 0;
    struct Count(usize);
    impl Visitor for Count {
        fn object_filter(&mut self, filter: &ObjectFilter) -> bool {
            self.0 += usize::from(filter.named_result().is_some());
            true
        }
    }
    for line in include_str!("fixtures/result_membership.ronl").lines() {
        let source: OracleCard = ron::from_str(line).unwrap();
        let lowered = unified::lower(&source);
        assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
        validate::unified(&lowered.card).unwrap();
        unified::round_trip(&lowered.card).unwrap();
        let mut count = Count(0);
        lowered.card.walk(&mut count);
        membership_nodes += count.0;
    }
    assert!(membership_nodes >= 25);
}

#[test]
fn surveil_can_report_graveyard_and_kept_cards_independently() {
    let after = Instruction::Group(Program {
        instructions: vec![
            test_filter(membership(3)),
            number(Expr::SurveilKeptOnTop(ResultId(3))),
        ],
    });
    let reads = read(after);
    assert!(reads.graveyard() && reads.kept_on_top && reads.objects);
}
