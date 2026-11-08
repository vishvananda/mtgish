//! Browser entry point to the same validator used by native IR consumers.
use mtgish_ast::{ir::UnifiedCard, validate};
use serde::Deserialize;
use serde_json::{json, Value};
use wasm_bindgen::prelude::*;

fn check(input: &str) -> Result<UnifiedCard, String> {
    if input.len() > 2 * 1024 * 1024 {
        return Err("This demo accepts IR documents up to 2 MiB.".into());
    }
    // Bound nesting before entering Serde so malformed pasted JSON cannot
    // overflow the Wasm stack before the ordinary validation errors run.
    let (mut depth, mut quoted, mut escaped) = (0usize, false, false);
    for b in input.bytes() {
        if quoted {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                quoted = false;
            }
        } else {
            match b {
                b'"' => quoted = true,
                b'{' | b'[' => {
                    depth += 1;
                    if depth >= 256 {
                        return Err("This demo accepts IR nesting up to 255 levels.".into());
                    }
                }
                b'}' | b']' => depth = depth.saturating_sub(1),
                _ => (),
            }
        }
    }
    let mut reader = serde_json::Deserializer::from_str(input);
    reader.disable_recursion_limit();
    let value = Value::deserialize(&mut reader).map_err(|e| e.to_string())?;
    reader.end().map_err(|e| e.to_string())?;
    fn bounded(v: &Value, depth: usize) -> bool {
        depth < 256
            && match v {
                Value::Array(v) => v.iter().all(|v| bounded(v, depth + 1)),
                Value::Object(v) => v.values().all(|v| bounded(v, depth + 1)),
                _ => true,
            }
    }
    if !bounded(&value, 0) {
        return Err("This demo accepts IR nesting up to 255 levels.".into());
    }
    let card: UnifiedCard = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
    if serde_json::to_value(&card).map_err(|e| e.to_string())? != value {
        return Err(
            "The document has unknown or missing fields; import the exact Rust IR wire shape."
                .into(),
        );
    }
    validate::unified(&card)?;
    for face in &card.faces {
        validate::card(face).map_err(|e| format!("{}: {e}", face.name))?;
    }
    // Layout metadata can also contain nested abilities and unreduced forms.
    fn reduced(v: &Value) -> bool {
        match v {
            Value::Object(v) => {
                !(v.contains_key("source_type")
                    && v.contains_key("form")
                    && v.contains_key("operands"))
                    && v.values().all(reduced)
            }
            Value::Array(v) => v.iter().all(reduced),
            _ => true,
        }
    }
    if !reduced(&value) {
        return Err("Unreduced Named forms cannot be exported as complete IR.".into());
    }
    Ok(card)
}

/// JSON-in/JSON-out keeps every i64/u64 digit on the Rust side of the bridge.
#[wasm_bindgen]
pub fn validate_ir(input: &str) -> String {
    match check(input) {
        Ok(card) => json!({"valid":true,"schema":mtgish_ast::ir::SCHEMA_VERSION,
            "faces":card.faces.len(),"ir":serde_json::to_string_pretty(&card).unwrap()})
        .to_string(),
        Err(error) => json!({"valid":false,"error":error}).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn blank() -> Value {
        json!({"schema_version":mtgish_ast::ir::SCHEMA_VERSION,"layout":"Card","data":[],"faces":[{
            "schema_version":mtgish_ast::ir::SCHEMA_VERSION,"name":"New card",
            "characteristics":{"supertypes":[],"card_types":["Creature"],"subtypes":[],"mana_cost":[],
                "color_indicator":[],"power_toughness":[1,1],"loyalty":null,"defense":null},"abilities":[]}]})
    }
    #[test]
    fn validates_native_shape_and_rejects_wrong_schema_unknown_fields_and_scope() {
        assert!(check(&blank().to_string()).is_ok());
        let mut value = blank();
        value["schema_version"] = json!(0);
        assert!(check(&value.to_string()).unwrap_err().contains("schema"));
        let mut value = blank();
        value["extra"] = json!(true);
        assert!(check(&value.to_string()).unwrap_err().contains("unknown"));
        let mut value = blank();
        value["faces"][0]["abilities"] = json!([{"kind":"Spell","args":{
            "announcement":{"targets":[],"modes":null},"body":{"instructions":[{
                "kind":"Effect","args":{"result":4294967295u32,"operation":{"kind":"Draw",
                "args":{"players":{"kind":"One","args":{"kind":"AbilityController"}},
                "amount":{"kind":"BoundValue","args":42}}}}}]}}}]);
        assert!(check(&value.to_string()).is_err());
    }
}
