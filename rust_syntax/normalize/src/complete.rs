//! Completeness checks shared by validation and optional legacy lowering.
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// A fieldless form that names something in scope, not a word from a vocabulary.
pub fn reads_scope(form: &str) -> bool {
    form == "AllCoinsCameUpHeads"
        || form.starts_with("Ref_")
        || form.starts_with("RefOuter_")
        || form.starts_with("Trigger_")
        || form.starts_with("Eachable")
        || form.ends_with("ThisWay") && form != "TheNameStickerPutOnPermanentThisWay"
}

/// Unreduced and primitive forms in a value, nested ones included, with counts.
#[derive(Default)]
pub struct Forms {
    pub named: BTreeMap<String, usize>,
    pub primitives: BTreeMap<String, usize>,
}

pub fn forms(value: &impl Serialize) -> Forms {
    fn walk(value: &Value, forms: &mut Forms) {
        match value {
            Value::Object(fields) => {
                if let (Some(ty), Some(form), true) = (
                    fields.get("source_type").and_then(Value::as_str),
                    fields.get("form").and_then(Value::as_str),
                    fields.contains_key("operands"),
                ) {
                    *forms.named.entry(format!("{ty}::{form}")).or_default() += 1;
                }
                // Tagged enums nest it under `args`, and untagged ones under the variant.
                let primitive = if fields.get("kind").and_then(Value::as_str) == Some("Primitive") {
                    fields.get("args")
                } else {
                    fields.get("Primitive")
                };
                if let Some(name) = primitive.and_then(|p| p["name"].as_str()) {
                    *forms.primitives.entry(name.into()).or_default() += 1;
                }
                fields.values().for_each(|child| walk(child, forms));
            }
            Value::Array(items) => items.iter().for_each(|child| walk(child, forms)),
            _ => {}
        }
    }
    let mut forms = Forms::default();
    walk(
        &serde_json::to_value(value).expect("serializable AST"),
        &mut forms,
    );
    forms
}

pub fn named_forms(value: &impl Serialize) -> BTreeMap<String, usize> {
    forms(value).named
}
