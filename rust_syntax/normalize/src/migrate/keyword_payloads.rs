//! Keyword migrations preserve each parameter subtree at its typed new home.
use super::entry_payloads::Region;
use super::*;
use crate::keyword_actions::{self, KeywordKind};
fn decode<T: serde::de::DeserializeOwned>(v: &Value) -> Result<T, String> {
    serde_json::from_value(v.clone()).map_err(|e| e.to_string())
}
pub(super) fn variant(owner: &str, value: &mut Value, path: &str) -> Result<Vec<Region>, String> {
    if !matches!(owner, "Operation::Keyword" | "Operation::KeywordEach") {
        return Ok(vec![]);
    }
    let args = &value["args"];
    let kind: KeywordKind = decode(&args["action"])?;
    let actor: PlayerSet = decode(&args["actor"])?;
    let source = format!("{path}/args");
    let mut moves = vec![(format!("{source}/actor"), format!("{source}/actor"))];
    let operation = if owner == "Operation::Keyword" {
        let action = keyword_actions::from_parts(
            kind,
            decode(&args["object"])?,
            decode(&args["other"])?,
            decode(&args["amount"])?,
            decode(&args["subtype"])?,
        )?;
        let rendered = serde_json::to_value(&action).map_err(|e| e.to_string())?;
        if let Some(object) = rendered.as_object() {
            let variant = object
                .keys()
                .next()
                .ok_or("keyword requires a constructor")?;
            for (old, new) in keyword_actions::legacy_fields(&action) {
                moves.push((
                    format!("{source}/{old}"),
                    format!("{source}/action/{variant}/{new}"),
                ));
            }
        }
        Operation::Keyword { actor, action }
    } else {
        let action = keyword_actions::each_from_parts(kind, decode(&args["until"])?)?;
        let objects: ObjectSet = decode(&args["objects"])?;
        moves.push((format!("{source}/objects"), format!("{source}/objects")));
        if let Some(object) = serde_json::to_value(&action)
            .map_err(|e| e.to_string())?
            .as_object()
        {
            let variant = object
                .keys()
                .next()
                .ok_or("set keyword requires a constructor")?;
            moves.push((
                format!("{source}/until"),
                format!("{source}/action/{variant}/until"),
            ));
        }
        Operation::KeywordEach {
            actor,
            action,
            objects,
        }
    };
    *value = serde_json::to_value(operation).map_err(|e| e.to_string())?;
    Ok(vec![Region { source, moves }])
}
