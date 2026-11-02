//! Only a known destination of this event can be identified with its subject.
use serde_json::{json, Value};

pub(super) fn destination(ability: &Value) -> Option<Value> {
    if ability["args"]["active"] == "SourceEntering" {
        return Some(json!("Battlefield"));
    }
    if ability["kind"] != "Triggered" {
        return None;
    }
    zone(&ability["args"]["event"])
}

fn zone(event: &Value) -> Option<Value> {
    if let Some(event) = event.pointer("/Filtered/event") {
        return zone(event);
    }
    if let Some(cases) = event.get("Either").and_then(Value::as_array) {
        let first = zone(cases.first()?)?;
        return cases
            .iter()
            .all(|event| zone(event).as_ref() == Some(&first))
            .then_some(first);
    }
    event
        .pointer("/ZoneChange/to")
        .filter(|to| !to.is_null())
        .cloned()
}
