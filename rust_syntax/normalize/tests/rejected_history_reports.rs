//! Upstream #120/#125 clarify Source operations' existing semantic contracts.
use mtgish_ast::{corpus, ir::*, mtg_types as source, unified, validate};

fn card(name: &str) -> Card {
    let source: source::OracleCard = include_str!("fixtures/rejected_history_reports.ronl")
        .lines()
        .map(|line| ron::from_str(line).unwrap())
        .find(|card| {
            corpus::Identity::of(card)
                .unwrap()
                .names
                .iter()
                .any(|n| n == name)
        })
        .unwrap();
    let lowered = unified::lower(&source);
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    validate::unified(&lowered.card).unwrap();
    lowered.card.faces.into_iter().next().unwrap()
}

#[test]
fn combat_copy_participants_come_from_damage_time_not_the_current_battlefield() {
    let card = card("Maelstrom Archangel Avatar");
    let text = serde_json::to_string(&card).unwrap();
    assert!(
        text.contains("FromHistory"),
        "departed creatures must remain participants: {text}"
    );
    assert!(text.contains("ThisTurn"));
    assert!(text.contains("Combat"));
    assert!(
        text.contains("ControlledBy"),
        "control is tested on the historical source: {text}"
    );
    assert!(
        !text.contains("EventHistory"),
        "do not rescan live creatures and test their damage history: {text}"
    );
    assert!(
        text.contains("Union"),
        "one creature is copied once even if it dealt several damage events: {text}"
    );
    assert!(text.contains("ManaCosts"));
}

#[test]
fn speed_reduction_preserves_the_operations_intrinsic_floor() {
    let card = card("Spikeshell Harrier");
    let value = serde_json::to_value(card).unwrap();
    fn find(v: &serde_json::Value) -> Option<&serde_json::Value> {
        if v.get("kind").and_then(|v| v.as_str()) == Some("ReduceSpeed") {
            return v.get("args");
        }
        match v {
            serde_json::Value::Object(o) => o.values().find_map(find),
            serde_json::Value::Array(a) => a.iter().find_map(find),
            _ => None,
        }
    }
    let op = find(&value).unwrap();
    assert_eq!(
        op["amount"],
        serde_json::json!({"kind":"Constant","args":1})
    );
    assert_eq!(op["floor"], serde_json::json!({"kind":"Constant","args":1}));
}
