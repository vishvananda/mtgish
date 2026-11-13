#![cfg(feature = "cantrip")]
use mtgish_ast::cantrip::planned;
use std::{collections::BTreeMap, path::PathBuf};
#[test]
fn named_damage_source_and_both_exchange_steps_are_preserved() {
    let f = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-mercenaries-juxtapose.jsonl");
    planned::tracked(&f.join("early_mercenaries_juxtapose.ronl"), &f.join("early_mercenaries_juxtapose.oracle.json"), &out).unwrap();
    let rows: BTreeMap<String, serde_json::Value> = std::fs::read_to_string(out).unwrap().lines().map(|s| { let v: serde_json::Value = serde_json::from_str(s).unwrap(); (v["name"].as_str().unwrap().into(), v) }).collect();
    for name in ["Mercenaries", "Juxtapose"] { assert!(rows[name]["faces"][0]["gaps"].as_array().unwrap().is_empty(), "{name}: {}", rows[name]); }
    let merc = rows["Mercenaries"]["faces"][0]["rust"].as_str().unwrap();
    assert!(merc.contains("asker.source"), "{merc}");
    let juxta = rows["Juxtapose"]["faces"][0]["rust"].as_str().unwrap();
    assert!(juxta.contains("CardType::Artifact") && juxta.contains("CardType::Creature"), "{juxta}");
    assert!(juxta.contains("__extreme_pool.contains(&__extreme_candidate)"), "{juxta}");
}
