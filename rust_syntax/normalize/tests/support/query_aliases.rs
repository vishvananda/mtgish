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
