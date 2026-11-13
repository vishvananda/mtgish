#![cfg(feature = "cantrip")]
use mtgish_ast::cantrip::planned;
use std::{collections::BTreeMap,path::PathBuf};
#[test]
fn mill_cost_is_payable_and_color_choices_are_nonempty_and_optional() {
    let fixtures=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let out=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-mill-colors.jsonl");
    planned::tracked(&fixtures.join("early_mill_colors.ronl"),&fixtures.join("early_mill_colors.oracle.json"),&out).unwrap();
    let rows:BTreeMap<String,serde_json::Value>=std::fs::read_to_string(out).unwrap().lines().map(|s| {let v:serde_json::Value=serde_json::from_str(s).unwrap();(v["name"].as_str().unwrap().into(),v)}).collect();
    for name in ["Deep Spawn","Dream Coat"] {assert!(rows[name]["faces"][0]["gaps"].as_array().unwrap().is_empty(),"{name}: {}",rows[name]);}
    let spawn=rows["Deep Spawn"]["faces"][0]["rust"].as_str().unwrap();
    assert!(spawn.contains("zone(Zone::library(payer_0)).len() >= 2 &&"),"{spawn}");
    assert!(spawn.contains("ask_may"),"{spawn}");
    let coat=rows["Dream Coat"]["faces"][0]["rust"].as_str().unwrap();
    assert!(coat.contains("RequestKind::ChooseModes") && coat.contains("count: 1, at_most: 5"),"{coat}");
    assert!(coat.contains("Label::Card") && coat.contains("repeat: false"),"{coat}");
}
