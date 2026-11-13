#![cfg(feature = "cantrip")]
use mtgish_ast::cantrip::planned;
use std::{collections::BTreeMap,path::PathBuf};
#[test]
fn sacrifice_tracks_both_incarnations_and_followups_require_revealed_cards() {
    let f=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let out=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-history-reveal.jsonl");
    planned::tracked(&f.join("early_history_reveal.ronl"), &f.join("early_history_reveal.oracle.json"), &out).unwrap();
    let rows:BTreeMap<String,serde_json::Value>=std::fs::read_to_string(out).unwrap().lines().map(|s| {let v:serde_json::Value=serde_json::from_str(s).unwrap();(v["name"].as_str().unwrap().into(),v)}).collect();
    for name in ["Thermokarst","Extinguish the Light","Vexing Arcanix","Urza's Miter"] {assert!(rows[name]["faces"][0]["gaps"].as_array().unwrap().is_empty(),"{name}: {}",rows[name]);}
    let miter=rows["Urza's Miter"]["faces"][0]["rust"].as_str().unwrap();
    assert!(miter.contains("matches!(event, Event::ZoneChange") && miter.contains("*old == __history_candidate || *new == __history_candidate"),"{miter}");
    assert!(miter.contains("old == __history_candidate || new == __history_candidate"),"{miter}");
    for name in ["Thermokarst","Extinguish the Light"] {let rust=rows[name]["faces"][0]["rust"].as_str().unwrap();assert!(rust.contains("t_0_raw"),"{name}: {rust}");}
    let vex=rows["Vexing Arcanix"]["faces"][0]["rust"].as_str().unwrap();assert!(vex.contains("revealed"),"{vex}");
}
