#![cfg(feature = "cantrip")]
use mtgish_ast::cantrip::planned;
use std::{collections::BTreeMap, path::PathBuf};

#[test]
fn history_candidate_is_not_shadowed_and_aura_filter_is_preserved() {
    let f = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-history-gauntlets.jsonl");
    planned::tracked(&f.join("early_history_gauntlets.ronl"), &f.join("early_history_gauntlets.oracle.json"), &out).unwrap();
    let rows: BTreeMap<String, serde_json::Value> = std::fs::read_to_string(out).unwrap().lines().map(|s| {
        let v: serde_json::Value = serde_json::from_str(s).unwrap();
        (v["name"].as_str().unwrap().into(), v)
    }).collect();
    for name in ["Halls of Mist", "Gauntlets of Chaos"] {
        assert!(rows[name]["faces"][0]["gaps"].as_array().unwrap().is_empty(), "{name}: {}", rows[name]);
    }
    let halls = rows["Halls of Mist"]["faces"][0]["rust"].as_str().unwrap();
    assert!(halls.contains("let __history_candidate = attacker") && halls.contains("attacker == __history_candidate"), "{halls}");
    let gauntlets = rows["Gauntlets of Chaos"]["faces"][0]["rust"].as_str().unwrap();
    assert!(gauntlets.contains("Subtype::Aura"), "{gauntlets}");
}
