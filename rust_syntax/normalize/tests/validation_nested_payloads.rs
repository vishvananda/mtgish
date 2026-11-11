use mtgish_ast::{human, ir::*, validate};

fn spell(operation: Operation) -> Card {
    let mut card = human::read("Card{name:\"Probe\",properties:{card_types:[Instant]}}")
        .unwrap()
        .faces
        .remove(0);
    card.abilities.push(Ability::Spell {
        announcement: Announcement::empty(),
        body: Program {
            instructions: vec![Instruction::Effect {
                result: ResultId(0),
                operation,
            }],
        },
    });
    card
}
fn shape() -> FaceDownAs {
    FaceDownAs {
        power: Expr::Constant(2),
        toughness: Expr::Constant(2),
        creature_type: None,
        artifact: false,
    }
}
fn moving(state: BattlefieldState) -> Card {
    spell(Operation::Move {
        objects: ObjectSet::One(ObjectRef::Source),
        destination: MovementDestination::Battlefield {
            controller: EntryController::Owner,
            arrival: BattlefieldArrival {
                states: vec![state],
                ..BattlefieldArrival::default()
            },
        },
        qualifiers: vec![],
    })
}

#[test]
fn entry_payloads_cannot_hide_missing_references() {
    for state in [
        BattlefieldState::ChangesUntil {
            changes: vec![Change::Rule(RuleChange::RequireCombat {
                action: ObjectAction::Attack(AttackDefenders::Objects(ObjectFilter::InSet(
                    Box::new(ObjectSet::Targets(TargetSlot(99))),
                ))),
                amount: RequirementAmount::AtLeast(Expr::Constant(1)),
            })],
            duration: Duration::EndOfTurn,
        },
        BattlefieldState::Optional {
            decider: PlayerRef::AbilityController,
            entry: Box::new(BattlefieldArrival {
                states: vec![BattlefieldState::FaceDownAs(FaceDownAs {
                    power: Expr::CountObjects(Box::new(ObjectSet::Targets(TargetSlot(99)))),
                    ..shape()
                })],
                ..BattlefieldArrival::default()
            }),
        },
        BattlefieldState::FaceDownWith {
            face_down: shape(),
            abilities: vec![Ability::Spell {
                announcement: Announcement::empty(),
                body: Program {
                    instructions: vec![Instruction::Effect {
                        result: ResultId(0),
                        operation: Operation::Reveal {
                            objects: ObjectSet::Targets(TargetSlot(99)),
                        },
                    }],
                },
            }],
            noted: None,
        },
        BattlefieldState::FaceDownAs(FaceDownAs {
            power: Expr::CountObjects(Box::new(ObjectSet::Targets(TargetSlot(99)))),
            ..shape()
        }),
    ] {
        assert!(validate::card(&moving(state))
            .unwrap_err()
            .contains("unbound target slot 99"));
    }
    let card = spell(Operation::Move {
        objects: ObjectSet::One(ObjectRef::Source),
        destination: MovementDestination::Exile {
            visibility: ExileVisibility::FaceUp,
            arrival: ExileArrival {
                states: vec![ExileState::ExiledForWith {
                    until: Duration::EndOfTurn,
                    objects: ObjectSet::One(ObjectRef::Source),
                    body: Program {
                        instructions: vec![Instruction::Effect {
                            result: ResultId(0),
                            operation: Operation::Reveal {
                                objects: ObjectSet::Targets(TargetSlot(99)),
                            },
                        }],
                    },
                }],
                ..ExileArrival::default()
            },
        },
        qualifiers: vec![],
    });
    assert!(validate::card(&card)
        .unwrap_err()
        .contains("unbound target slot 99"));
}

#[test]
fn countering_checks_destination_and_entry_scopes() {
    for destination in [
        MovementDestination::OwnersHand(CardArrival {
            states: vec![CardArrivalState::ChangesUntil {
                changes: vec![],
                duration: Duration::WhileItMatches(ObjectFilter::InSet(Box::new(
                    ObjectSet::Targets(TargetSlot(99)),
                ))),
            }],
            ..CardArrival::default()
        }),
        MovementDestination::Battlefield {
            controller: EntryController::Player(PlayerRef::Target {
                slot: TargetSlot(99),
                index: 0,
            }),
            arrival: BattlefieldArrival::default(),
        },
    ] {
        let card = spell(Operation::Counter { objects: ObjectSet::One(ObjectRef::Source), destination_when: None, destination: destination });
        assert!(validate::card(&card)
            .unwrap_err()
            .contains("unbound target slot 99"));
    }
}

#[test]
fn duration_operands_validate_their_captured_references() {
    let card = moving(BattlefieldState::ChangesUntil {
        changes: vec![],
        duration: Duration::WhileItMatches(ObjectFilter::InSet(Box::new(ObjectSet::Targets(
            TargetSlot(99),
        )))),
    });
    assert!(validate::card(&card)
        .unwrap_err()
        .contains("unbound target slot 99"));
}

#[test]
fn bounded_counts_reject_negative_or_reversed_literals() {
    let code = |count: &str| {
        format!("Card{{name:\"Bounds\",properties:{{card_types:[Instant]}},abilities:[Spell{{targets:[Target{{name:victim,select:Creature,count:{count}}}],do:[Reveal{{objects:Targets(victim)}}]}}]}}")
    };
    for count in [
        "Exactly(-1)",
        "Between{minimum:-1,maximum:1}",
        "Between{minimum:0,maximum:-1}",
        "Between{minimum:3,maximum:1}",
    ] {
        assert!(human::read(&code(count)).is_err(), "{count}");
    }
    for count in [
        "Exactly(0)",
        "Between{minimum:0,maximum:0}",
        "Between{minimum:1,maximum:3}",
        "Between{minimum:0,maximum:AnnouncedX}",
    ] {
        human::read(&code(count)).unwrap_or_else(|error| panic!("{count}: {error}"));
    }
}

#[test]
fn mode_bounds_share_the_same_literal_checks() {
    let code = |minimum: &str, maximum: &str| {
        format!("Card{{name:\"Modes\",properties:{{card_types:[Instant]}},abilities:[Spell{{modes:{{count:{{minimum:{minimum},maximum:{maximum}}},rules:{{repetition:MayRepeat}},options:[{{weight:1,do:[DrawCards{{players:You,amount:1}}]}}]}},do:[]}}]}}")
    };
    for (minimum, maximum) in [("-1", "AnnouncedX"), ("AnnouncedX", "-1"), ("3", "1")] {
        assert!(human::read(&code(minimum, maximum)).is_err());
    }
    human::read(&code("0", "AnnouncedX")).unwrap();
}

#[test]
#[ignore = "explicit full current normalized corpus"]
fn validate_the_current_corpus() {
    use std::io::BufRead;
    mtgish_ast::corpus::on_large_stack(|| {
        let file = std::env::var("AST_CLARITY_CORPUS").unwrap();
        let mut failures = Vec::new();
        let mut count = 0;
        for line in std::io::BufReader::new(std::fs::File::open(file).unwrap()).lines() {
            let card: UnifiedCard = serde_json::from_str(&line.unwrap()).unwrap();
            let checked = validate::unified(&card)
                .and_then(|_| card.faces.iter().try_for_each(validate::card));
            if let Err(error) = checked {
                failures.push((
                    card.faces
                        .iter()
                        .map(|f| f.name.clone())
                        .collect::<Vec<_>>(),
                    error,
                ));
            }
            count += 1;
        }
        for (names, error) in &failures {
            eprintln!("{names:?}: {error}");
        }
        assert_eq!(count, 33529);
        assert!(failures.is_empty(), "{} invalid records", failures.len());
        println!("All {count} records validate");
    });
}
