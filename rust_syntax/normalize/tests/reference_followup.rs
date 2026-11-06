use mtgish_ast::{corpus, ir::*, mtg_types as source, unified, validate};

fn card(name: &str) -> Card {
    corpus::on_large_stack(|| {
        let source: source::OracleCard = include_str!("fixtures/reference_followup.ronl")
            .lines()
            .filter(|line| line.contains(&format!("Name: {name:?}")))
            .map(|line| ron::from_str(line).unwrap())
            .find(|card| {
                corpus::Identity::of(card)
                    .unwrap()
                    .names
                    .iter()
                    .any(|n| n == name)
            })
            .unwrap_or_else(|| panic!("missing fixture {name}"));
        let lowered = unified::lower(&source);
        assert!(
            lowered.diagnostics.is_empty(),
            "{name}: {:?}",
            lowered.diagnostics
        );
        let card = lowered
            .card
            .faces
            .into_iter()
            .find(|c| c.name == name)
            .unwrap();
        validate::card(&card).unwrap();
        card
    })
}

#[test]
fn computed_counter_tax_uses_the_value_bound_before_payment() {
    for name in ["Rethink", "Spell Rupture", "Thassa's Rebuff"] {
        let c = card(name);
        let Ability::Spell { body, announcement } = &c.abilities[0] else {
            panic!("{name}")
        };
        assert_eq!(announcement.targets.len(), 1);
        let [Instruction::Effect {
            result: value,
            operation: Operation::BindValue { .. },
        }, Instruction::Effect {
            result: payment,
            operation:
                Operation::PayCost {
                    player,
                    cost,
                    optional: true,
                },
        }, Instruction::If {
            condition,
            then,
            otherwise,
        }] = body.instructions.as_slice()
        else {
            panic!("{name}: {body:?}")
        };
        assert!(
            matches!(player, PlayerRef::ControllerOf(object) if matches!(object.as_ref(), ObjectRef::Target { slot, index: 0 } if slot.0 == 0)),
            "{name}"
        );
        assert!(
            matches!(cost.parts.as_slice(), [CostPart::ManaWithX { mana, x: Some(Expr::BoundValue(bound)) }]
            if bound.0 == value.0 && matches!(mana.as_slice(), [ManaSymbol::X])),
            "{name}: {cost:?}"
        );
        assert!(
            matches!(condition, Condition::Not(paid) if matches!(paid.as_ref(), Condition::PaymentChosen(result) if result.0 == payment.0))
        );
        assert!(matches!(
            then.instructions.as_slice(),
            [Instruction::Effect {
                operation: Operation::Counter { .. },
                ..
            }]
        ));
        assert!(otherwise.instructions.is_empty());
    }
}

#[test]
fn independent_counterspell_followups_are_outside_the_nonpayment_branch() {
    for name in [
        "Offering to Asha",
        "Stymied Hopes",
        "Frightful Delusion",
        "Sage's Dousing",
    ] {
        let c = card(name);
        let Ability::Spell { body, .. } = &c.abilities[0] else {
            panic!("{name}")
        };
        let [Instruction::Effect {
            operation: Operation::PayCost { .. },
            ..
        }, Instruction::If {
            then, otherwise, ..
        }, followup] = body.instructions.as_slice()
        else {
            panic!("{name}: {body:?}")
        };
        assert!(matches!(
            then.instructions.as_slice(),
            [Instruction::Effect {
                operation: Operation::Counter { .. },
                ..
            }]
        ));
        assert!(otherwise.instructions.is_empty());
        assert!(
            match (name, followup) {
                (
                    "Offering to Asha",
                    Instruction::Effect {
                        operation:
                            Operation::GainLife {
                                amount: Expr::Constant(4),
                                ..
                            },
                        ..
                    },
                ) => true,
                (
                    "Stymied Hopes",
                    Instruction::Effect {
                        operation:
                            Operation::Scry {
                                amount: Expr::Constant(1),
                                ..
                            },
                        ..
                    },
                ) => true,
                (
                    "Frightful Delusion",
                    Instruction::Effect {
                        operation: Operation::Discard { .. },
                        ..
                    },
                ) => true,
                ("Sage's Dousing", Instruction::If { then, .. }) => matches!(
                    then.instructions.as_slice(),
                    [Instruction::Effect {
                        operation: Operation::Draw { .. },
                        ..
                    }]
                ),
                _ => false,
            },
            "{name}: {followup:?}"
        );
    }
}

#[test]
fn counterspells_using_cast_x_keep_their_announced_mana_cost() {
    for name in ["Condescend", "Overrule"] {
        let c = card(name);
        let Ability::Spell { body, .. } = &c.abilities[0] else {
            panic!("{name}")
        };
        assert!(
            matches!(body.instructions.first(),
            Some(Instruction::Effect { operation: Operation::PayCost { cost, .. }, .. })
                if matches!(cost.parts.as_slice(), [CostPart::Mana(mana)]
                    if matches!(mana.as_slice(), [ManaSymbol::X]))),
            "{name}: {body:?}"
        );
    }
}

#[test]
fn primordial_ooze_pays_and_damages_using_the_same_counter_snapshot() {
    let c = card("Primordial Ooze");
    let Ability::Triggered { body, .. } = &c.abilities[1] else {
        panic!("expected upkeep trigger")
    };
    let [Instruction::Effect {
        operation: Operation::PutCounters { .. },
        ..
    }, Instruction::Effect {
        result: value,
        operation: Operation::BindValue { .. },
    }, Instruction::Effect {
        operation: Operation::PayCost { cost, .. },
        ..
    }, Instruction::If { then, .. }] = body.instructions.as_slice()
    else {
        panic!("{body:?}")
    };
    assert!(
        matches!(cost.parts.as_slice(), [CostPart::ManaWithX { x: Some(Expr::BoundValue(bound)), .. }] if bound.0 == value.0)
    );
    assert!(matches!(then.instructions.as_slice(), [
        Instruction::Effect { operation: Operation::Tap { .. }, .. },
        Instruction::Effect { operation: Operation::DealDamage { amount: Expr::BoundValue(bound), .. }, .. }
    ] if bound.0 == value.0));
}
