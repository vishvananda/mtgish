//! Reuse native simultaneous preparation; never hoist choices across a closure.
use crate::ir::Ability;
use serde_json::Value;
fn needed(value: &Value) -> bool {
    if value["kind"] == "Together"
        && value["args"].as_array().is_some_and(|actions| {
            actions
                .iter()
                .any(|action| action["kind"] == "WithSelections")
        })
    {
        return true;
    }
    match value {
        Value::Object(fields) => fields.values().any(needed),
        Value::Array(items) => items.iter().any(needed),
        _ => false,
    }
}
pub(super) fn groups(value: &mut Value) -> Result<(), String> {
    if needed(value) {
        let mut abilities: Vec<Ability> =
            serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
        crate::movement::normalize_abilities(&mut abilities);
        *value = serde_json::to_value(abilities).map_err(|e| e.to_string())?;
    }
    Ok(())
}
