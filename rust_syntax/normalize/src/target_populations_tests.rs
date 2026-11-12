use super::normalize_value;
use crate::{
    ir::*,
    visit::{Visitor, Walk},
};
fn scalar() -> ObjectSet {
    ObjectSet::One(ObjectRef::Target {
        slot: TargetSlot(0),
        index: 0,
    })
}
fn target(count: Count) -> TargetSpec {
    TargetSpec {
        slot: TargetSlot(0),
        kind: TargetKind::Object(ObjectDomain::permanent()),
        count,
        filter: ObjectFilter::Any,
        players: None,
        distribute: None,
        constraints: vec![],
        chooser: None,
    }
}
fn announcement(count: Count) -> Announcement {
    Announcement {
        targets: vec![target(count)],
        modes: None,
    }
}
fn program(ops: Vec<Operation>) -> Program {
    Program {
        instructions: ops
            .into_iter()
            .enumerate()
            .map(|(i, operation)| Instruction::Effect {
                result: ResultId(i as u32),
                operation,
            })
            .collect(),
    }
}
fn spell(count: Count, ops: Vec<Operation>) -> Ability {
    Ability::Spell {
        announcement: announcement(count),
        body: program(ops),
    }
}
fn optional() -> Count {
    Count::UpTo(Expr::Constant(1))
}
fn mandatory() -> Count {
    Count::Exactly(Expr::Constant(1))
}
fn tap() -> Operation {
    Operation::SetTapped {
        objects: scalar(),
        tapped: true,
    }
}
fn populations(a: &Ability) -> (usize, usize) {
    struct V(usize, usize);
    impl Visitor for V {
        fn object_set(&mut self, s: &ObjectSet) -> bool {
            match s {
                ObjectSet::Targets(_) => self.0 += 1,
                ObjectSet::One(ObjectRef::Target { .. }) => self.1 += 1,
                _ => {}
            }
            true
        }
    }
    let mut v = V(0, 0);
    a.walk(&mut v);
    (v.0, v.1)
}
#[test]
fn optional_population_normal_form_is_independent_of_operation_and_destination() {
    let mut ops = vec![
        tap(),
        Operation::Attach {
            attachments: scalar(),
            host: ObjectRef::Source,
        },
        Operation::Counter {
            objects: scalar(),
            destination: MovementDestination::OwnersGraveyard(Default::default()),
            destination_when: None,
        },
        Operation::DealDamage {
            source: ObjectRef::Source,
            recipients: Recipients::Objects(scalar()),
            amount: Expr::Constant(1),
            preventable: true,
        },
        Operation::PutCounters {
            placements: vec![CounterPlacement {
                recipients: Recipients::Objects(scalar()),
                kind: crate::counter::CounterKind::PowerToughness(1, 1),
                amount: Expr::Constant(1),
                selection: None,
            }],
        },
        Operation::PayCost {
            player: PlayerRef::AbilityController,
            cost: Cost {
                parts: vec![CostPart::Sacrifice {
                    objects: scalar(),
                    choose: None,
                }],
            },
            optional: true,
        },
    ];
    let destinations = vec![
        MovementDestination::Battlefield {
            controller: EntryController::Owner,
            arrival: Default::default(),
        },
        MovementDestination::Exile {
            visibility: ExileVisibility::FaceUp,
            arrival: Default::default(),
        },
        MovementDestination::Library {
            position: LibraryPosition::Bottom,
            order: Default::default(),
            arrival: Default::default(),
        },
        MovementDestination::OwnersHand(Default::default()),
        MovementDestination::Hand {
            player: PlayerRef::AbilityController,
            arrival: Default::default(),
        },
        MovementDestination::OwnersGraveyard(Default::default()),
        MovementDestination::CommandZone(Default::default()),
        MovementDestination::Ante(Default::default()),
        MovementDestination::OwnersPlanarDeck {
            end: SequenceEnd::Bottom,
            arrival: Default::default(),
        },
    ];
    for destination in destinations {
        ops.push(Operation::Move {
            objects: scalar(),
            destination,
            qualifiers: vec![],
        });
    }
    let n = ops.len();
    let mut a = spell(optional(), ops);
    normalize_value(&mut a);
    assert_eq!(populations(&a), (n, 0));
    let once = serde_json::to_value(&a).unwrap();
    normalize_value(&mut a);
    assert_eq!(serde_json::to_value(&a).unwrap(), once);
}
#[test]
fn mandatory_targets_and_explicit_elements_of_larger_slots_stay_scalar() {
    for count in [
        mandatory(),
        Count::UpTo(Expr::Constant(2)),
        Count::Exactly(Expr::Constant(2)),
    ] {
        let mut a = spell(count, vec![tap()]);
        normalize_value(&mut a);
        assert_eq!(populations(&a), (0, 1));
    }
}
#[test]
fn scalar_identity_fields_remain_scalar_while_player_and_damage_populations_normalize() {
    let mut t = target(optional());
    t.kind = TargetKind::Player;
    let mut a = Ability::Spell {
        announcement: Announcement {
            targets: vec![t],
            modes: None,
        },
        body: program(vec![Operation::Draw {
            players: PlayerSet::One(PlayerRef::Target {
                slot: TargetSlot(0),
                index: 0,
            }),
            amount: Expr::Constant(1),
        }]),
    };
    normalize_value(&mut a);
    let Ability::Spell { body, .. } = a else {
        panic!()
    };
    assert!(matches!(
        &body.instructions[0],
        Instruction::Effect {
            operation: Operation::Draw {
                players: PlayerSet::Targets(_),
                ..
            },
            ..
        }
    ));
    let mut t = target(optional());
    t.kind = TargetKind::DamageRecipient;
    let mut a = Ability::Spell {
        announcement: Announcement {
            targets: vec![t],
            modes: None,
        },
        body: program(vec![
            Operation::DealDamage {
                source: ObjectRef::Source,
                recipients: Recipients::Target {
                    slot: TargetSlot(0),
                    index: 0,
                },
                amount: Expr::Constant(1),
                preventable: true,
            },
            Operation::Attach {
                attachments: scalar(),
                host: ObjectRef::Target {
                    slot: TargetSlot(0),
                    index: 0,
                },
            },
        ]),
    };
    normalize_value(&mut a);
    let Ability::Spell { body, .. } = a else {
        panic!()
    };
    assert!(matches!(
        &body.instructions[0],
        Instruction::Effect {
            operation: Operation::DealDamage {
                recipients: Recipients::Targets(_),
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        &body.instructions[1],
        Instruction::Effect {
            operation: Operation::Attach {
                host: ObjectRef::Target { .. },
                ..
            },
            ..
        }
    ));
}
#[test]
fn created_trigger_targets_shadow_captures_and_restore_the_outer_slot() {
    let child = Operation::ReflexiveTrigger {
        performed: None,
        condition: None,
        announcement: announcement(mandatory()),
        body: program(vec![tap()]),
    };
    let mut a = spell(optional(), vec![tap(), child, tap()]);
    normalize_value(&mut a);
    assert_eq!(populations(&a), (2, 1));
    let child = Operation::ReflexiveTrigger {
        performed: None,
        condition: None,
        announcement: Announcement::empty(),
        body: program(vec![tap()]),
    };
    let mut a = spell(optional(), vec![child]);
    normalize_value(&mut a);
    assert_eq!(populations(&a), (1, 0));
}
#[test]
fn a_granted_ability_does_not_inherit_its_grantors_target_cardinality() {
    let child = spell(mandatory(), vec![tap()]);
    let mut a = spell(
        optional(),
        vec![Operation::CreateContinuousEffect {
            source: ObjectRef::Source,
            affected: EffectSubjects::Objects(scalar()),
            changes: vec![Change::GrantAbility(Box::new(child))],
            duration: Duration::EndOfTurn,
            evaluation: EffectEvaluation::Snapshot,
        }],
    );
    normalize_value(&mut a);
    assert_eq!(populations(&a), (1, 1));
}
#[test]
fn sibling_modes_and_sibling_abilities_do_not_leak_target_declarations() {
    let mode = |count| Mode {
        announcement: announcement(count),
        body: program(vec![tap()]),
        weight: 1,
        requires: None,
    };
    let modes = ModeSelection {
        count: ModeCount {
            minimum: Expr::Constant(1),
            maximum: Expr::Constant(1),
        },
        options: vec![mode(optional()), mode(mandatory())],
        rules: ModeRules {
            repetition: ModeRepetition::OnceEach,
            availability: ModeAvailability::Any,
            surcharge: None,
            targets: ModeTargetPolicy::MayShare,
            acceptance: ModeAcceptance::Required,
        },
        mechanism: ModeMechanism::Implied,
        budget: None,
    };
    let mut a = Ability::Spell {
        announcement: Announcement {
            targets: vec![],
            modes: Some(Box::new(modes)),
        },
        body: program(vec![]),
    };
    normalize_value(&mut a);
    assert_eq!(populations(&a), (1, 1));
    let source: crate::mtg_types::OracleCard = ron::from_str(
        include_str!("../tests/fixtures/reference_other_targets_20261112.ronl")
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    let mut card = crate::lower::card(&source).unwrap();
    card.abilities = vec![
        spell(optional(), vec![tap()]),
        spell(mandatory(), vec![tap()]),
    ];
    normalize_value(&mut card);
    assert_eq!(populations(&card.abilities[0]), (1, 0));
    assert_eq!(populations(&card.abilities[1]), (0, 1));
}

#[test]
fn zero_or_one_target_queries_keep_their_singleton_proof_without_accepting_plural_sets() {
    for (count, index, valid) in [
        (optional(), 0, true),
        (mandatory(), 0, true),
        (optional(), 1, false),
        (Count::UpTo(Expr::Constant(2)), 0, false),
    ] {
        let mut a = spell(
            count,
            vec![Operation::SetTapped {
                objects: ObjectSet::One(ObjectRef::Query {
                    objects: Box::new(ObjectSet::InZone {
                        objects: Box::new(ObjectSet::Targets(TargetSlot(0))),
                        zone: EventZone::Battlefield,
                    }),
                    index,
                }),
                tapped: true,
            }],
        );
        normalize_value(&mut a);
        assert_eq!(
            crate::validate::validate_ability(&a).is_ok(),
            valid,
            "index={index}: {a:?}"
        );
    }
}

#[test]
fn created_event_rebinding_and_populations_reach_the_normal_form_in_one_walk() {
    let line = include_str!("../tests/fixtures/reference_other_targets_20261112.ronl")
        .lines()
        .find(|l| l.contains("Name: \"Niko Aris\","))
        .unwrap();
    let source: crate::mtg_types::OracleCard = ron::from_str(line).unwrap();
    let mut card = crate::unified::lower(&source).card;
    let once = serde_json::to_value(&card).unwrap();
    normalize_value(&mut card);
    assert_eq!(serde_json::to_value(&card).unwrap(), once);
}

#[test]
fn required_payment_quantities_are_explicit_and_unquantified_costs_keep_the_guard() {
    let mut a = spell(
        optional(),
        vec![Operation::PayCost {
            player: PlayerRef::AbilityController,
            optional: true,
            cost: Cost {
                parts: vec![
                    CostPart::Move {
                        selections: vec![CostObjectSelection {
                            objects: scalar(),
                            choose: None,
                        }],
                        destination: MovementDestination::OwnersHand(Default::default()),
                    },
                    CostPart::ObjectAction {
                        objects: scalar(),
                        action: CostObjectAction::Suspect,
                    },
                ],
            },
        }],
    );
    normalize_value(&mut a);
    let Ability::Spell { body, .. } = a else {
        panic!()
    };
    let Instruction::Effect {
        operation: Operation::PayCost { cost, .. },
        ..
    } = &body.instructions[0]
    else {
        panic!()
    };
    let CostPart::Move { selections, .. } = &cost.parts[0] else {
        panic!()
    };
    assert!(matches!(selections[0].objects, ObjectSet::Targets(_)));
    assert!(matches!(
        selections[0].choose,
        Some(Count::Exactly(Expr::Constant(1)))
    ));
    assert!(crate::cost_history::singular(&cost.parts[0]));
    assert!(matches!(
        &cost.parts[1],
        CostPart::ObjectAction {
            objects: ObjectSet::One(ObjectRef::Target { .. }),
            ..
        }
    ));
}
