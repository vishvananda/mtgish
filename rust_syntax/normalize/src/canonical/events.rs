//! Reference frames of independently matched created events.
use serde_json::{json, Value};

pub(super) struct Frame {
    pub subject: super::Subject,
    pub damage: Option<Value>,
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
    if owner == "EntryState::ExiledForWith" {
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
