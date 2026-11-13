#![cfg(feature = "cantrip")]
use mtgish_ast::cantrip::planned;
use std::{collections::BTreeMap, path::PathBuf};
#[test]
fn resolving_aura_uses_last_attachment_and_unconditional_choice_has_no_dead_fallback() {
    let f=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let out=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-aura-choice.jsonl");
    planned::tracked(&f.join("early_aura_choice.ronl"),&f.join("early_aura_choice.oracle.json"),&out).unwrap();
    let rows:BTreeMap<String,serde_json::Value>=std::fs::read_to_string(out).unwrap().lines().map(|s| {let v:serde_json::Value=serde_json::from_str(s).unwrap();(v["name"].as_str().unwrap().into(),v)}).collect();
    for name in ["Dream Coat","Dwarven Armorer"] {assert!(rows[name]["faces"][0]["gaps"].as_array().unwrap().is_empty(),"{name}: {}",rows[name]);}
    let coat=rows["Dream Coat"]["faces"][0]["rust"].as_str().unwrap();
    assert!(coat.contains("q.obj(o).attached_to.filter") && coat.contains("cantrip_api::is_live(q, host)"),"{coat}");
    let armorer=rows["Dwarven Armorer"]["faces"][0]["rust"].as_str().unwrap();
    assert!(armorer.contains("choose::one_of") && armorer.contains("if picked_") && !armorer.contains("let offered_"),"{armorer}");
    let clock=rows["Agonizing Remorse"]["faces"][0]["rust"].as_str().unwrap();
    assert!(clock.contains("let offered_"),"{clock}");
}
