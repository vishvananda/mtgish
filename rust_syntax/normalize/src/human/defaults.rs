//! Semantic defaults shared by the readable codec and the block editor.
use crate::{ir::*, ir_schema::Ty};
use serde_json::{json, Value};
pub fn type_value(name: &str) -> Option<Value> {
    Some(match name {
        "Announcement" => json!({"targets":[],"modes":null}),
        "ActivationLimits" => serde_json::to_value(ActivationLimits::unrestricted()).unwrap(),
        "TriggerLimits" => json!({"per_turn":null,"total":null}),
        "EntrySpec" => serde_json::to_value(EntrySpec::default()).unwrap(),
        "MovementTerms" => serde_json::to_value(MovementTerms::default()).unwrap(),
        _ => return None,
    })
}
pub fn field(owner: &str, key: &str, ty: &Ty) -> Option<Value> {
    if key == "schema_version" && matches!(owner, "Card" | "UnifiedCard") {
        return Some(json!(SCHEMA_VERSION));
    }
    if owner == "UnifiedCard" && key == "layout" {
        return Some(json!("Card"));
    }
    if owner == "TargetSpec" && key == "count" {
        return Some(json!({"Exactly":{"kind":"Constant","args":1}}));
    }
    if key == "preventable" {
        return Some(json!(true));
    }
    if key == "regeneration" && owner == "Operation::Destroy" {
        return Some(json!(true));
    }
    if ty == &Ty::Named("bool".into())
        && (owner == "EntrySpec"
            || owner == "ActivationLimits"
            || matches!(key, "face_down" | "random" | "into_exile"))
    {
        return Some(json!(false));
    }
    match ty {
        Ty::Vec(_) => Some(json!([])),
        Ty::Option(_) => Some(Value::Null),
        Ty::Named(n) => type_value(n),
        _ => None,
    }
}
