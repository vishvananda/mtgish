use mtgish_ast::{human, ir::*, validate};

fn effect(id: u32, operation: Operation) -> Instruction {
    Instruction::Effect {
        result: ResultId(id),
        operation,
    }
}
fn scalar() -> Operation {
    Operation::FlipCoin {
        player: PlayerRef::AbilityController,
        protocol: CoinProtocol::Called,
    }
}
fn check(id: u32, outcome: CoinOutcome) -> Instruction {
    Instruction::If {
        condition: Condition::CoinFlip {
            result: ResultId(id),
            outcome,
        },
        then: Program::empty(),
        otherwise: Program::empty(),
    }
}
fn count(id: u32) -> Instruction {
    effect(
        99,
        Operation::Draw {
            players: PlayerSet::One(PlayerRef::AbilityController),
            amount: Expr::CountCoinOutcomes {
                source: CoinCountSource::Receipt(ResultId(id)),
                outcome: CoinOutcome::Heads,
            },
        },
    )
}
fn card(instructions: Vec<Instruction>) -> UnifiedCard {
    let mut c = human::read(
        "Card{name:\"Coin receipts\",properties:{card_types:[Instant]},abilities:[Spell{do:[]}]}",
    )
    .unwrap();
    let Ability::Spell { body, .. } = &mut c.faces[0].abilities[0] else {
        panic!()
    };
    body.instructions = instructions;
    c
}
fn validate(c: &UnifiedCard) -> Result<(), String> {
    validate::unified(c)?;
    validate::card(&c.faces[0])
}
fn unrelated() -> Vec<Operation> {
    vec![
        Operation::Draw {
            players: PlayerSet::One(PlayerRef::AbilityController),
            amount: Expr::Constant(1),
        },
        Operation::DealDamage {
            source: ObjectRef::Source,
            recipients: Recipients::Players(PlayerSet::One(PlayerRef::AbilityController)),
            amount: Expr::Constant(1),
            preventable: true,
        },
        Operation::PayCost {
            player: PlayerRef::AbilityController,
            cost: Cost { parts: vec![] },
            optional: false,
        },
    ]
}
#[test]
fn another_available_receipt_does_not_make_a_coin() {
    for operation in unrelated() {
        for outcome in [
            CoinOutcome::Won,
            CoinOutcome::Lost,
            CoinOutcome::Heads,
            CoinOutcome::Tails,
        ] {
            let error =
                validate(&card(vec![effect(7, operation.clone()), check(7, outcome)])).unwrap_err();
            assert!(error.contains("not a single coin flip"), "{error}");
        }
        let error = validate(&card(vec![effect(7, operation), count(7)])).unwrap_err();
        assert!(error.contains("flipped no coins"), "{error}");
    }
}
#[test]
fn a_scalar_flip_has_faces_and_win_loss_observations_without_backend_restrictions() {
    // CR 705.3 allows replacements to change winning/losing. Receipt validation
    // establishes the coin family, not which channels a backend can implement.
    for outcome in [
        CoinOutcome::Won,
        CoinOutcome::Lost,
        CoinOutcome::Heads,
        CoinOutcome::Tails,
    ] {
        let c = card(vec![effect(7, scalar()), check(7, outcome), count(7)]);
        validate(&c).unwrap();
        let text = human::write(&c).unwrap();
        human::read(&text).unwrap();
    }
}
#[test]
fn even_a_counted_single_coin_remains_a_batch_receipt() {
    for operation in [
        Operation::FlipCoins {
            players: PlayerSet::One(PlayerRef::AbilityController),
            count: Expr::Constant(1),
            protocol: CoinProtocol::Called,
        },
        Operation::FlipCoinSeries {
            players: PlayerSet::One(PlayerRef::AbilityController),
            repetition: CoinRepetition::Count(Expr::Constant(1)),
            protocol: CoinProtocol::Called,
        },
    ] {
        validate(&card(vec![effect(7, operation.clone()), count(7)])).unwrap();
        for outcome in [
            CoinOutcome::Won,
            CoinOutcome::Lost,
            CoinOutcome::Heads,
            CoinOutcome::Tails,
        ] {
            assert!(
                validate(&card(vec![effect(7, operation.clone()), check(7, outcome)]))
                    .unwrap_err()
                    .contains("not a single coin flip")
            );
        }
    }
}
#[test]
fn loops_gather_coins_without_exposing_an_arbitrary_scalar() {
    let each = Instruction::ForEachPlayer {
        players: PlayerSet::All,
        binding: LoopBinding(0),
        starting_with: None,
        in_chosen_direction: false,
        body: Program {
            instructions: vec![
                effect(
                    7,
                    Operation::FlipCoin {
                        player: PlayerRef::Bound(LoopBinding(0)),
                        protocol: CoinProtocol::Uncalled,
                    },
                ),
                check(7, CoinOutcome::Heads),
            ],
        },
    };
    validate(&card(vec![each.clone(), count(7)])).unwrap();
    assert!(validate(&card(vec![each, check(7, CoinOutcome::Heads)]))
        .unwrap_err()
        .contains("not a single coin flip"));
}
#[test]
fn absent_future_optional_and_independent_coins_stay_out_of_scope() {
    for instructions in [
        vec![check(7, CoinOutcome::Won)],
        vec![check(7, CoinOutcome::Won), effect(7, scalar())],
        vec![
            Instruction::Optional {
                decider: PlayerRef::AbilityController,
                body: Program {
                    instructions: vec![effect(7, scalar())],
                },
            },
            check(7, CoinOutcome::Won),
        ],
    ] {
        assert!(validate(&card(instructions))
            .unwrap_err()
            .contains("not a single coin flip"));
    }
    let mut c = card(vec![effect(7, scalar())]);
    c.faces[0].abilities.push(Ability::Spell {
        announcement: Announcement::empty(),
        body: Program {
            instructions: vec![check(7, CoinOutcome::Won)],
        },
    });
    assert!(validate(&c).unwrap_err().contains("not a single coin flip"));
}
#[test]
fn captured_coin_and_shadowed_noncoin_keep_their_actual_families() {
    let trigger = |body| {
        effect(
            8,
            Operation::CreateDelayedTrigger {
                event: Box::new(EventSpec::Turn {
                    point: TurnPoint::EndStepBegins,
                    players: PlayerSet::All,
                }),
                start: DelayedStart::Immediately,
                expires: None,
                intervening_if: None,
                body,
            },
        )
    };
    validate(&card(vec![
        effect(7, scalar()),
        trigger(Program {
            instructions: vec![check(7, CoinOutcome::Won)],
        }),
    ]))
    .unwrap();
    // A fresh granted ability has its own r7. It must not inherit the outer r7's
    // coin family just because the numeric labels match.
    let local = Ability::Spell {
        announcement: Announcement::empty(),
        body: Program {
            instructions: vec![
                effect(7, unrelated().remove(0)),
                check(7, CoinOutcome::Heads),
            ],
        },
    };
    let grant = effect(
        8,
        Operation::CreateContinuousEffect {
            source: ObjectRef::Source,
            affected: EffectSubjects::Objects(ObjectSet::One(ObjectRef::Source)),
            changes: vec![Change::GrantAbility(Box::new(local))],
            duration: Duration::EndOfTurn,
            evaluation: EffectEvaluation::Snapshot,
        },
    );
    assert!(validate(&card(vec![effect(7, scalar()), grant]))
        .unwrap_err()
        .contains("not a single coin flip"));
}
#[test]
fn selection_preparation_preserves_the_scalar_operation_contract() {
    let wrapped = Operation::WithSelections {
        steps: vec![],
        action: Box::new(scalar()),
    };
    validate(&card(vec![
        effect(7, wrapped),
        check(7, CoinOutcome::Heads),
        count(7),
    ]))
    .unwrap();
}
