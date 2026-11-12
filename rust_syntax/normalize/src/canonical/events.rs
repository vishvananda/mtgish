//! Reference frames of independently matched created events.
use serde_json::{json, Value};

pub(super) struct Frame {
    pub subject: super::Subject,
    pub damage: Option<Value>,
}

pub(super) fn subject(event: &Value) -> super::Subject {
    // The same existing self-reference proof applies to an alternative event
    // only when every branch independently identifies that same source.
    // In particular, a self entry OR a haunted creature's death is not self.
    if let Some(events) = event.get("AnyOf").and_then(Value::as_array) {
        return if !events.is_empty()
            && events
                .iter()
                .all(|event| matches!(subject(event), super::Subject::Source))
        {
            super::Subject::Source
        } else {
            super::Subject::Other
        };
    }
    let source = json!({"kind":"SameObject","args":{"kind":"Source"}});
    if super::event_objects(event) == Some(&source) {
        super::Subject::Source
    } else {
        super::Subject::Other
    }
}

pub(super) fn created(owner: &str, args: &Value) -> Option<Frame> {
    if matches!(
        owner,
        "EntryState::ExiledForWith" | "ExileState::ExiledForWith"
    ) {
        let event = serde_json::to_value(crate::semantics::entry_return_event())
            .expect("typed returning entry event");
        return Some(Frame {
            subject: subject(&event),
            damage: None,
        });
    }
    (owner == "Operation::CreateTrigger").then(|| Frame {
        subject: subject(&args["event"]),
        damage: super::damage_subject(&args["event"]),
    })
}
