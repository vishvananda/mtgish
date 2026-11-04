#![cfg(feature = "cantrip")]
use mtgish_ast::cantrip::{self, planned};
use std::path::Path;

fn printed(name: &str, features: &[&str]) -> String {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    planned::with_features(features, || cantrip::modules_printed(
        &fixtures.join("combat_entry_authority.ronl"),
        &fixtures.join("combat_entry_authority.oracle.json"), &[name.into()],
    )).unwrap().pop().unwrap().1
}

#[test]
fn the_chants_pay_printed_counter_costs_without_performing_blight() {
    for name in ["Thelon's Chant", "Tourach's Chant"] {
        let module = printed(name, &["entry-actor", "resolving-counter-costs"]);
        assert!(module.contains("counters::may_pay_on_one("), "{name}: {module}");
        assert!(!module.contains("blight"), "{name}: {module}");
        assert!(module.contains("CounterKind::MinusOne"), "{name}: {module}");
        assert!(module.contains("put_by: Some(put_by)"), "{name}: {module}");
        syn::parse_file(&module).unwrap();
    }
}

#[test]
fn a_target_without_the_counter_payment_api_keeps_an_explicit_gap() {
    for name in ["Thelon's Chant", "Tourach's Chant"] {
        let module = printed(name, &["entry-actor"]);
        assert!(module.contains("selected-recipient-counter-cost"), "{name}: {module}");
        assert!(!module.contains("blight"), "{name}: {module}");
    }
}

#[test]
fn a_real_blight_cost_keeps_its_keyword_action() {
    use mtgish_ast::{corpus, ir::*, lower, mtg_types::OracleCard};
    let source: OracleCard = include_str!("fixtures/combat_entry_authority.ronl").lines()
        .map(|line| ron::from_str(line).unwrap())
        .find(|card| corpus::Identity::of(card).unwrap().names.iter().any(|n| n == "Thelon's Chant")).unwrap();
    let mut card = lower::card(&source).unwrap();
    let mut changed = false;
    for ability in &mut card.abilities {
        if let Ability::Triggered { body, .. } = ability {
            for instruction in &mut body.instructions {
                if let Instruction::Effect { operation: Operation::PayCost { cost, .. }, .. } = instruction {
                    if matches!(cost.parts.as_slice(), [CostPart::PutCounters(_)]) {
                        cost.parts = vec![CostPart::Keyword { action: CostKeyword::Blight, amount: Some(Expr::Constant(1)) }];
                        changed = true;
                    }
                }
            }
        }
    }
    assert!(changed);
    let module = planned::with_features(&["entry-actor", "resolving-counter-costs"], || cantrip::module(&card)).unwrap();
    assert!(module.contains("blight::cost(1)"), "{module}");
    assert!(!module.contains("counters::may_pay_on_one("), "{module}");
}
