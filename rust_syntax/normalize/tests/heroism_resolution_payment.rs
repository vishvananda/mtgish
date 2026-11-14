use mtgish_ast::{ir::*, lower, mtg_types::OracleCard, validate};

fn source() -> OracleCard {
    ron::from_str(include_str!("fixtures/heroism_resolution_payment.ronl").trim()).unwrap()
}

#[test]
fn each_controller_pays_before_a_shield_is_created_for_that_creature() {
    let card = lower::card(&source()).unwrap();
    validate::card(&card).unwrap();
    let Ability::Activated { body, .. } = &card.abilities[0] else {
        panic!()
    };
    let [Instruction::ForEachObject { binding, body, .. }] = body.instructions.as_slice() else {
        panic!("the ability must enumerate current attackers before damage: {body:?}")
    };
    let [Instruction::Effect {
        result,
        operation: Operation::PayCost {
            player, optional, ..
        },
    }, Instruction::If {
        condition,
        then,
        otherwise,
    }] = body.instructions.as_slice()
    else {
        panic!("the payment must precede conditional shield creation: {body:?}")
    };
    assert!(*optional);
    assert!(
        matches!(player, PlayerRef::ControllerOf(object) if matches!(**object, ObjectRef::Bound(ref b) if b.0 == binding.0))
    );
    assert!(
        matches!(condition, Condition::Not(test) if matches!(**test, Condition::PaymentChosen(ref paid) if paid.0 == result.0))
    );
    assert!(otherwise.instructions.is_empty());
    let [Instruction::Effect {
        operation: Operation::CreateReplacement { event, body, .. },
        ..
    }] = then.instructions.as_slice()
    else {
        panic!()
    };
    let ReplacementEvent::Damage(damage) = event else {
        panic!()
    };
    assert!(
        matches!(damage.sources, ObjectFilter::SameObject(ObjectRef::Bound(ref b)) if b.0 == binding.0)
    );
    // The waiting effect only prevents; it cannot ask for a late payment.
    assert!(matches!(
        body.instructions.as_slice(),
        [Instruction::Effect {
            operation: Operation::ModifyEvent {
                change: EventChange::PreventDamage
            },
            ..
        }]
    ));
}

#[cfg(feature = "cantrip")]
#[test]
fn corrected_payment_timing_does_not_claim_correct_payment_ordering() {
    use mtgish_ast::{cantrip, unified};
    let source = source();
    let result = cantrip::whole(&source, &unified::lower(&source).card).unwrap();
    assert!(result.faces[0].gaps.iter().any(|gap|
        gap.feature == "per-object controller payment choice order and simultaneous follow-up"
    ), "{:?}", result.faces[0].gaps);
    let code = &result.faces[0].rust;
    syn::parse_file(code).unwrap();
    assert!(code.contains("planned!"), "{code}");
    assert!(!code.contains("g.may_pay("), "{code}");
}

#[cfg(feature = "cantrip")]
#[test]
fn the_ordering_gap_follows_the_program_not_the_card_name() {
    use mtgish_ast::{cantrip, unified};
    let source: OracleCard = ron::from_str(
        &include_str!("fixtures/heroism_resolution_payment.ronl")
            .replace("Heroism", "Controller payment ordering witness"),
    ).unwrap();
    let result = cantrip::whole(&source, &unified::lower(&source).card).unwrap();
    assert!(result.faces[0].gaps.iter().any(|gap|
        gap.feature == "per-object controller payment choice order and simultaneous follow-up"
    ), "{:?}", result.faces[0].gaps);
}
