use mtgish_ast::{mtg_types::OracleCard, unified, validate};
use serde_json::Value;

// Compare complete semantic trees, permitting only the reported mode changes.
// This guards operands and references as well as the two animations on Sphere
// and Glint Hawk Idol. The fixture is also parsed from English in the Go test.
fn replace_modes(value: &mut Value, paths: &mut Vec<String>, path: &str) {
    match value {
        Value::Object(fields) => {
            if fields.get("kind").and_then(Value::as_str) == Some("CreatureTypes") {
                let args = fields.get_mut("args").unwrap();
                assert_eq!(args["mode"], "Set");
                args["mode"] = Value::String("Add".into());
                paths.push(format!("{path}/args/mode"));
            }
            for (key, value) in fields {
                replace_modes(value, paths, &format!("{path}/{key}"));
            }
        }
        Value::Array(values) => {
            for (index, value) in values.iter_mut().enumerate() {
                replace_modes(value, paths, &format!("{path}/{index}"));
            }
        }
        _ => {}
    }
}

#[test]
fn named_artifact_animations_replace_only_creature_types() {
    let mut nodes = 0;
    let mut cards = 0;
    for line in include_str!("fixtures/artifact_animations.ronl").lines() {
        if !line.contains("SetCreatureType(") {
            continue;
        }
        let source: OracleCard = ron::from_str(line).unwrap();
        let lowered = unified::lower(&source);
        assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
        validate::unified(&lowered.card).unwrap();
        unified::round_trip(&lowered.card).unwrap();
        let mut repaired = serde_json::to_value(&lowered.card).unwrap();
        let old: OracleCard =
            ron::from_str(&line.replace("SetCreatureType(", "AddCreatureType(")).unwrap();
        let old = serde_json::to_value(unified::lower(&old).card).unwrap();
        let mut paths = Vec::new();
        replace_modes(&mut repaired, &mut paths, "");
        assert_eq!(paths.len(), line.matches("SetCreatureType(").count());
        assert_eq!(repaired, old, "only creature type modes may change");
        nodes += paths.len();
        cards += 1;
    }
    assert_eq!((cards, nodes), (19, 21));
}

#[test]
fn bare_and_explicit_retention_animations_keep_creature_types() {
    let mut count = 0;
    for line in include_str!("fixtures/artifact_animations.ronl").lines() {
        if line.contains("SetCreatureType(") {
            continue;
        }
        let source: OracleCard = ron::from_str(line).unwrap();
        let card = unified::lower(&source).card;
        validate::unified(&card).unwrap();
        let json = serde_json::to_value(&card).unwrap();
        let text = serde_json::to_string(&json).unwrap();
        if line.contains("AddCreatureType(") {
            assert!(text.contains(r#""mode":"Add""#));
            assert!(!text.contains(r#""mode":"Set""#));
        } else {
            assert!(!text.contains("CreatureTypes"));
        }
        count += 1;
    }
    assert_eq!(count, 4);
}

#[test]
fn restless_cottage_can_target_any_graveyard_without_other_changes() {
    let line = include_str!("fixtures/restless_cottage.ronl").trim();
    let source: OracleCard = ron::from_str(line).unwrap();
    let lowered = unified::lower(&source);
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    validate::unified(&lowered.card).unwrap();
    unified::round_trip(&lowered.card).unwrap();
    let mut repaired = serde_json::to_value(&lowered.card).unwrap();
    let target = repaired
        .pointer("/faces/0/abilities/3/args/announcement/targets/0")
        .unwrap();
    assert_eq!(target["filter"], serde_json::json!({"kind": "Any"}));
    assert_eq!(
        target["kind"],
        serde_json::json!({"Object": {"kind": "Card", "zones": ["Graveyard"]}})
    );
    assert_eq!(
        target["count"],
        serde_json::json!({"UpTo": {"kind": "Constant", "args": 1}})
    );
    let old: OracleCard = ron::from_str(&line.replace(
        "UptoOneTargetGraveyardCard(AnyCardInAnyGraveyard)",
        "UptoOneTargetGraveyardCard(InAPlayersGraveyard(SinglePlayer(You)))",
    ))
    .unwrap();
    let old = serde_json::to_value(unified::lower(&old).card).unwrap();
    let path = "/faces/0/abilities/3/args/announcement/targets/0/filter";
    *repaired.pointer_mut(path).unwrap() = old.pointer(path).unwrap().clone();
    assert_eq!(repaired, old, "only the target filter may change");
}
