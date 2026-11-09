//! Browser entry point to the same validator used by native IR consumers.
use mtgish_ast::{ir::UnifiedCard, validate};
use serde_json::{json, Value};
use wasm_bindgen::prelude::*;

fn check(input: &str) -> Result<UnifiedCard, String> {
    let value = mtgish_ast::json_input::value(input)?;
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

/// Explicitly upgrade a persisted normalized document, retaining strict field
/// and scope checks. Ordinary validation never guesses an old schema.
#[wasm_bindgen]
pub fn migrate_ir(input: &str) -> String {
    match mtgish_ast::migrate::read_json(input) {
        Ok(card) => {
            json!({"valid":true,"ir":serde_json::to_string_pretty(&card).unwrap()}).to_string()
        }
        Err(error) => json!({"valid":false,"error":error}).to_string(),
    }
}

/// Render card code using the Rust schema, shared defaults and naming policy.
#[wasm_bindgen]
pub fn format_card_code(input: &str) -> String {
    match check(input).and_then(|card| mtgish_ast::human::write(&card)) {
        Ok(code) => json!({"valid":true,"code":code}).to_string(),
        Err(error) => json!({"valid":false,"error":error}).to_string(),
    }
}

/// Parse symbolic authoring code, allocate internal IDs and validate the full IR.
#[wasm_bindgen]
pub fn parse_card_code(input: &str) -> String {
    match mtgish_ast::human::document::read(input) {
        Ok(document) => {
            json!({"valid":true,"ir":serde_json::to_string_pretty(document.card()).unwrap(),
                "document":serde_json::to_string(&document).unwrap()})
            .to_string()
        }
        Err(error) => json!({"valid":false,"error":error}).to_string(),
    }
}

/// Inspect typed names without changing the caller's numeric compiler IDs.
#[wasm_bindgen]
pub fn describe_card_bindings(input: &str) -> String {
    match check(input).and_then(|card| mtgish_ast::human::document::from_card(&card)) {
        Ok(document) => {
            json!({"valid":true,"document":serde_json::to_string(&document).unwrap()}).to_string()
        }
        Err(error) => json!({"valid":false,"error":error}).to_string(),
    }
}
/// Format a persisted authoring document; its names never enter engine IR.
#[wasm_bindgen]
pub fn format_card_document(input: &str) -> String {
    match mtgish_ast::human::document::Document::from_json(input)
        .and_then(|doc| doc.write().map(|code| (doc, code)))
    {
        Ok((document, code)) => {
            json!({"valid":true,"code":code,"document":serde_json::to_string(&document).unwrap()})
                .to_string()
        }
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
    fn card_code_bridge_uses_the_same_strict_validator() {
        let formatted: Value =
            serde_json::from_str(&format_card_code(&blank().to_string())).unwrap();
        assert_eq!(formatted["valid"], true);
        let parsed: Value =
            serde_json::from_str(&parse_card_code(formatted["code"].as_str().unwrap())).unwrap();
        assert_eq!(parsed["valid"], true);
        assert_eq!(
            serde_json::from_str::<Value>(parsed["ir"].as_str().unwrap()).unwrap(),
            blank()
        );
        for code in ["Card {name: \"bad\",properties:{card_types:[Creature]},extra:true}",
            "Card {name: \"bad\",properties:{card_types:[Instant]},abilities:[Spell{do:[DrawCards{players:You,amount:BoundValue(b42)}]}]}"] {
            let response: Value = serde_json::from_str(&parse_card_code(code)).unwrap();
            assert_eq!(response["valid"], false);
        }
    }
    #[test]
    fn document_bridge_preserves_names_without_adding_them_to_ir() {
        let code = r#"Card{name:"Names",properties:{card_types:[Instant]},abilities:[Spell{targets:[Target{name:victim,select:AnyTarget}],do:[hit=DealDamage{from:This,to:Targets(victim),amount:3}]}]}"#;
        let parsed: Value = serde_json::from_str(&parse_card_code(code)).unwrap();
        assert_eq!(parsed["valid"], true);
        assert!(!parsed["ir"].as_str().unwrap().contains("victim"));
        let formatted: Value =
            serde_json::from_str(&format_card_document(parsed["document"].as_str().unwrap()))
                .unwrap();
        assert_eq!(formatted["valid"], true);
        assert!(formatted["code"]
            .as_str()
            .unwrap()
            .contains("hit = DealDamage"));
        assert!(formatted["code"]
            .as_str()
            .unwrap()
            .contains("Targets(victim)"));
        let described: Value =
            serde_json::from_str(&describe_card_bindings(parsed["ir"].as_str().unwrap())).unwrap();
        assert_eq!(described["valid"], true);
        let mut extra: Value = serde_json::from_str(parsed["document"].as_str().unwrap()).unwrap();
        extra["card"]["unexpected"] = json!(true);
        let rejected: Value =
            serde_json::from_str(&format_card_document(&extra.to_string())).unwrap();
        assert_eq!(rejected["valid"], false);
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
