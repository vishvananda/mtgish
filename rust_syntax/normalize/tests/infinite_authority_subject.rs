//! The first creature stays the subject of the original pair event, even in deferred work.
use serde_json::Value;

fn collect<'a>(value: &'a Value, kind: &str, result: &mut Vec<&'a Value>) {
    match value {
        Value::Object(fields) => {
            if fields.get("kind").and_then(Value::as_str) == Some(kind) {
                result.push(value);
            }
            for value in fields.values() {
                collect(value, kind, result);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect(value, kind, result);
            }
        }
        _ => {}
    }
}

#[test]
fn delayed_counter_names_the_original_creature_not_the_auras_later_host() {
    let source = ron::from_str(include_str!("fixtures/infinite_authority_subject.ronl").trim()).unwrap();
    let card = mtgish_ast::lower::card(&source).unwrap();
    let value = serde_json::to_value(card).unwrap();
    let mut counters = Vec::new();
    collect(&value, "PutCounters", &mut counters);
    assert_eq!(counters.len(), 1, "{value}");
    let counter = serde_json::to_string(counters[0]).unwrap();
    assert!(counter.contains("EventSubject"), "{counter}");
    assert!(!counter.contains("AttachedTo"), "{counter}");
}
