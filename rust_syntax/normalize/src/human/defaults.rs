//! Semantic defaults shared by the readable codec and the block editor.
use crate::{ir::*, ir_schema::Ty};
use serde_json::{json, Value};
pub fn type_value(name: &str) -> Option<Value> {
    Some(match name {
        "CoinProtocol" => json!("Called"),
        "CoinCountSource" => json!("Latest"),
        "ModeCount" => {
            json!({"minimum":{"kind":"Constant","args":1},"maximum":{"kind":"Constant","args":1}})
        }
        "ModeRules" => {
            json!({"repetition":"OnceEach","availability":"Any","surcharge":null,"targets":"MayShare","acceptance":"Required"})
        }
        "ModeMechanism" => json!({"Announced":{"chooser":"Controller","method":"PlayerChoice"}}),
        "ModeChooser" => json!("Controller"),
        "ModePicking" => json!("PlayerChoice"),
        "ModeRepetition" => json!("OnceEach"),
        "ModeAvailability" => json!("Any"),
        "ModeTargetPolicy" => json!("MayShare"),
        "ModeAcceptance" => json!("Required"),
        "Announcement" => json!({"targets":[],"modes":null}),
        "ActivationLimits" => serde_json::to_value(ActivationLimits::unrestricted()).unwrap(),
        "TriggerLimits" => json!({"per_turn":null,"total":null}),
        "EntrySpec" => serde_json::to_value(crate::movement::legacy::EntrySpec::default()).unwrap(),
        "BattlefieldArrival" => serde_json::to_value(BattlefieldArrival::default()).unwrap(),
        "ExileArrival" => serde_json::to_value(ExileArrival::default()).unwrap(),
        "CardArrival" => serde_json::to_value(CardArrival::default()).unwrap(),
        "LibraryChooser" => json!("Owners"),
        "ExileVisibility" => json!("FaceUp"),
        "ConjurePosition" => json!("Random"),
        "MovementOrder" => json!("DestinationDefault"),
        "MovementTerms" => serde_json::to_value(MovementTerms::default()).unwrap(),
        _ => return None,
    })
}
pub fn field(owner: &str, key: &str, ty: &Ty) -> Option<Value> {
    if owner == "KeywordAction::Amass" && key == "creature_type" {
        return Some(json!("Zombie"));
    }
    if key == "schema_version" && matches!(owner, "Card" | "UnifiedCard") {
        return Some(json!(SCHEMA_VERSION));
    }
    if owner == "UnifiedCard" && key == "layout" {
        return Some(json!("Card"));
    }
    if owner == "TargetSpec" && key == "count" {
        return Some(json!({"Exactly":{"kind":"Constant","args":1}}));
    }
    if matches!(
        owner,
        "EntryState::Optional"
            | "BattlefieldState::Optional"
            | "ExileState::Optional"
            | "CardArrivalState::Optional"
    ) && key == "decider"
    {
        return Some(json!({"kind":"AbilityController"}));
    }
    if owner == "FaceDownAs" && key == "artifact" {
        return Some(json!(false));
    }
    if key == "preventable" {
        return Some(json!(true));
    }
    if key == "regeneration" && owner == "Operation::Destroy" {
        return Some(json!(true));
    }
    if ty == &Ty::Named("bool".into())
        && (matches!(owner, "EntrySpec" | "BattlefieldArrival")
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
