//! Only a known destination of this event can be identified with its subject.
use serde_json::{json, Value};

pub(super) struct Frame {
    pub subject: super::Subject,
    pub damage: Option<Value>,
    pub destination: Option<Value>,
}

pub(super) fn subject(event: &Value) -> super::Subject {
    let source = json!({"kind":"SameObject","args":{"kind":"Source"}});
    if super::event_objects(event) == Some(&source) {
        super::Subject::Source
    } else {
        super::Subject::Other
    }
}

pub(super) fn created(owner: &str, args: &Value) -> Option<Frame> {
    (owner == "Operation::CreateTrigger").then(|| Frame {
        subject: subject(&args["event"]),
        damage: super::damage_subject(&args["event"]),
        destination: zone(&args["event"]),
    })
}

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
