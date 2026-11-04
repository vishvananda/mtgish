//! Reference corrections are checked on typed IR without enabling a backend.
#![cfg(feature = "legacy")]
use mtgish_ast::{lower, validate};
use serde_json::Value;

fn values<'a>(value: &'a Value, kind: &str, out: &mut Vec<&'a Value>) {
    match value {
        Value::Object(fields) => {
            if value["kind"] == kind {
                out.push(value);
            }
            for value in fields.values() {
                values(value, kind, out);
            }
        }
        Value::Array(items) => {
            for value in items {
                values(value, kind, out);
            }
        }
        _ => {}
    }
}

#[test]
fn compulsory_effects_keep_real_receipts_and_independent_optional_payments() {
    for line in include_str!("fixtures/mandatory_outcome.ronl").lines() {
        let source = ron::from_str(line).unwrap();
        let card = lower::card(&source).unwrap();
        validate::card(&card).unwrap();
        let value = serde_json::to_value(&card).unwrap();
        let mut choices = Vec::new();
        values(&value, "PaymentChosen", &mut choices);
        assert_eq!(
            choices.len(),
            usize::from(card.name != "Infinite Authority"),
            "{}",
            card.name
        );
        let mut transitions = Vec::new();
        values(&value, "Transition", &mut transitions);
        assert!(!transitions.is_empty(), "{}", card.name);
        assert!(
            transitions.iter().any(|v| v["args"]["phase"] == "After"),
            "{}",
            card.name
        );
        if card.name == "Puppet Master" {
            let mut filters = Vec::new();
            values(&value, "InZone", &mut filters);
            assert!(filters
                .iter()
                .any(|v| v["args"]["Players"]["zone"] == "Hand"));
        }
    }
}
