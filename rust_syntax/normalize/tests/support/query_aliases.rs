#[path = "legacy_entries.rs"]
mod legacy_entries;
use serde_json::{json, Value};

/// Declared schema-176 migration for these earlier captured fixtures: ordinary
/// move defaults and an unselected/unflagged zone change. Richer transfers must
/// use the separate full migration oracle, rather than relaxing this comparison.
pub fn movement_defaults(value: &mut Value) {
    legacy_entries::json(value);
    if value.get("kind").and_then(Value::as_str) == Some("ChangeZone") {
        let a = &value["args"];
        let normal=a["entry"].as_array().unwrap().iter().all(|flag|*flag==json!({"kind":"Term","args":{"family":"EnterFlag","form":"EntersNormally","operands":[]}}));
        assert!(
            a["count"].is_null()
                && a["group"].is_null()
                && a["random"] == false
                && a["face_down"].is_null()
                && normal,
            "a richer captured transfer needs its declared migration: {value}"
        );
        let placement = if a["destination"] == "OwnersLibraryTopOrBottom" {
            json!({"By":a["actor"]})
        } else {
            json!("DestinationDefault")
        };
        *value = json!({"kind":"Move","args":{"objects":a["cards"],"destination":a["destination"],"entry":{"tapped":false,"counters":[],"states":[]},"face_down":false,"terms":{"order":"DestinationDefault","placement":placement,"qualifiers":[]}}});
    }
    if value.get("kind").and_then(Value::as_str) == Some("Move") {
        if let Some(args) = value.get_mut("args").and_then(Value::as_object_mut) {
            if args.contains_key("objects") && args.contains_key("entry") {
                args.entry("terms").or_insert_with(||json!({"order":"DestinationDefault","placement":"DestinationDefault","qualifiers":[]}));
            }
        }
    }
    match value {
        Value::Object(fields) => fields.values_mut().for_each(movement_defaults),
        Value::Array(values) => values.iter_mut().for_each(movement_defaults),
        _ => {}
    }
    legacy_entries::json(value);
}

/// Only the declared portable-query alias migration; every filter and other
/// operand is retained. Captured producer fixtures remain byte-for-byte intact.
pub fn battlefield(value: &mut Value) {
    if value.get("kind").and_then(Value::as_str) == Some("Battlefield") {
        let filter = value["args"].take();
        *value = json!({"kind":"Zone","args":{"scope":{"Shared":"Battlefield"},"kind":"Permanent","filter":filter}});
    }
    match value {
        Value::Object(fields) => fields.values_mut().for_each(battlefield),
        Value::Array(values) => values.iter_mut().for_each(battlefield),
        _ => {}
    }
}

/// Schema 173 replaces both value-choice spellings with one operation and an
/// explicit cardinality. This migration is only for captured producer proofs.
pub fn value_choices(value: &mut Value) {
    match value {
        Value::Object(fields) => fields.values_mut().for_each(value_choices),
        Value::Array(values) => values.iter_mut().for_each(value_choices),
        _ => {}
    }
    match value.get("kind").and_then(Value::as_str) {
        Some("ChooseValue") if value["args"].get("count").is_some() => {
            let args = value["args"].as_object_mut().unwrap();
            let count = args.remove("count").unwrap().as_u64().unwrap();
            let options = args.remove("options").unwrap();
            let count = if count == 0 {
                json!("AnyNumber")
            } else {
                json!({"Exactly":{"kind":"Constant","args":count}})
            };
            args.insert(
                "terms".into(),
                json!({"options":options,"except":[],"count":count,
                "secret":false,"unchosen":false,"random":false,"ordered":false}),
            );
        }
        Some("ChooseValueWith") => {
            value["kind"] = json!("ChooseValue");
            let terms = value["args"]["terms"].as_object_mut().unwrap();
            let up_to = terms.remove("up_to").unwrap().as_bool().unwrap();
            let count = terms.remove("count").unwrap();
            let count = if count.is_null() {
                json!({"kind":"Constant","args":1})
            } else {
                count
            };
            terms.insert(
                "count".into(),
                if up_to {
                    json!({"UpTo":count})
                } else {
                    json!({"Exactly":count})
                },
            );
        }
        _ => {}
    }
}
