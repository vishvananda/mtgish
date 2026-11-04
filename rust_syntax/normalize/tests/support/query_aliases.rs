use serde_json::{json, Value};

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
