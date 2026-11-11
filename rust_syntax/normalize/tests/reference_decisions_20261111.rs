use mtgish_ast::{canonical, corpus, ir::*, mtg_types::OracleCard, unified, validate};
use serde_json::Value;

fn normalized(abilities: &[Ability]) -> Value {
    validate::abilities(abilities).unwrap();
    let mut value = serde_json::to_value(abilities).unwrap();
    canonical::abilities(&mut value).unwrap();
    value
}

#[test]
fn reviewed_repairs_normal_forms_and_retained_contracts_match_complete_programs() {
    corpus::on_large_stack(|| {
        let expected: Value = serde_json::from_str(include_str!(
            "fixtures/reference_decisions_20261111_expected.json"
        ))
        .unwrap();
        let mut count = 0;
        for line in include_str!("fixtures/reference_decisions_20261111.ronl").lines() {
            let source: OracleCard = ron::from_str(line).unwrap();
            let lowered = unified::lower(&source);
            for card in &lowered.card.faces {
                validate::card(card).unwrap_or_else(|e| panic!("{}: {e}", card.name));
                let wanted: Vec<Ability> =
                    serde_json::from_value(expected[&card.name].clone()).unwrap();
                assert_eq!(
                    normalized(&card.abilities),
                    normalized(&wanted),
                    "{}",
                    card.name
                );
                count += 1;
            }
        }
        assert_eq!(count, expected.as_object().unwrap().len());
    });
}

#[path = "support/source_json.rs"]
mod source_json;

#[test]
fn tagged_source_and_ron_agree_for_repairs_and_the_retained_return_cost() {
    corpus::on_large_stack(|| {
        source_json::assert_fixture_matches(include_str!(
            "fixtures/reference_decisions_source_20261111.ronl"
        ))
    });
}

#[test]
fn alora_keeps_payment_choice_and_all_post_cost_outputs_without_a_perpetual_primitive() {
    corpus::on_large_stack(|| {
        let line = include_str!("fixtures/reference_decisions_source_20261111.ronl")
            .lines()
            .find(|l| l.contains("Name: \"Alora, Cheerful Scout\""))
            .unwrap();
        let source: OracleCard = ron::from_str(line).unwrap();
        let lowered = unified::lower(&source);
        assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
        let Ability::Triggered { body, .. } = &lowered.card.faces[0].abilities[0] else {
            panic!()
        };
        let Instruction::Effect {
            operation: Operation::CreateDelayedTrigger { body, .. },
            ..
        } = &body.instructions[1]
        else {
            panic!()
        };
        let Instruction::Effect {
            result,
            operation: Operation::PayCost {
                optional: false, ..
            },
        } = &body.instructions[0]
        else {
            panic!()
        };
        let Instruction::If {
            then, condition, ..
        } = &body.instructions[1]
        else {
            panic!()
        };
        let Instruction::Effect {
            operation: Operation::CreatePerpetualEffect { objects, .. },
            ..
        } = &then.instructions[0]
        else {
            panic!()
        };
        let ObjectSet::CostObjects {
            payment: PaymentRef::Resolution(payment),
            component,
            view: CostObjectView::Performed,
            phase: ObjectPhase::After,
        } = objects
        else {
            panic!("{objects:?}")
        };
        assert_eq!(payment.0, result.0);
        assert_eq!(component, &[0]);
        assert!(
            matches!(condition, Condition::PaymentChosen(paid) if paid.0 == result.0),
            "CR118.12: replacement of a started payment does not turn its guard false"
        );
    });
}
