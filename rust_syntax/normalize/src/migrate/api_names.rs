//! Schema-185 role/type names and counter-source origins. The archived input
//! descriptor validates the original fields before these bottom-up rewrites.
use super::{entry_payloads::Region, *};
pub(super) fn variant(owner: &str, value: &mut Value, path: &str) -> Result<Vec<Region>, String> {
    let rename = match owner {
        "ObjectRef::Named" | "ObjectSet::Named" => Some("Recorded"),
        "ObjectRef::KeywordHost" => Some("AbilityHolder"),
        "ObjectRef::Granter" => Some("GrantingObject"),
        "Expr::Power" => Some("Exponentiate"),
        "Operation::Tap" => Some("SetTapped"),
        _ => None,
    };
    let mut regions = vec![];
    if let Some(new) = rename {
        let old = owner.split("::").nth(1).unwrap();
        if value.get("kind").is_some() {
            value["kind"] = json!(new);
        } else if value.is_string() {
            *value = json!(new);
        } else {
            let args = value
                .as_object_mut()
                .ok_or("enum payload requires an object")?
                .remove(old)
                .ok_or("missing old enum payload")?;
            let mut object = serde_json::Map::new();
            object.insert(new.into(), args);
            *value = Value::Object(object);
            regions.push(Region {
                source: path.into(),
                moves: vec![(format!("{path}/{old}"), format!("{path}/{new}"))],
            });
        }
    }
    let renames: &[(&str, &str)] = match owner {
        "Operation::Search" => &[("player", "searcher"), ("library", "library_owner")],
        "Operation::ReorderLibrary" => &[("player", "chooser"), ("library", "library_owner")],
        "Operation::ChooseValue" => &[("actor", "choosers")],
        _ => &[],
    };
    for (old, new) in renames {
        let field = value["args"]
            .as_object_mut()
            .ok_or("operation requires named fields")?
            .remove(*old)
            .ok_or("missing old role field")?;
        value["args"][*new] = field;
        let source = format!("{path}/args/{old}");
        regions.push(Region {
            source: source.clone(),
            moves: vec![(source, format!("{path}/args/{new}"))],
        });
    }
    if owner == "Operation::DuplicateCounters" {
        let at = format!("{path}/args/from");
        let old = value["args"]["from"].clone();
        if old["kind"] == "CountersTheEventPut" {
            value["args"]["from"] = json!("EventPut");
            regions.push(Region {
                source: at,
                moves: vec![],
            });
        } else {
            value["args"]["from"] = json!({"On":old});
            regions.push(Region {
                source: at.clone(),
                moves: vec![(at.clone(), format!("{at}/On"))],
            });
        }
    }
    Ok(regions)
}
