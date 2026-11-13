#![cfg(feature = "cantrip")]
use mtgish_ast::cantrip::planned;
use std::{collections::BTreeMap, path::PathBuf};

#[test]
fn discard_is_limited_to_cards_actually_drawn_and_still_in_hand() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-draw-discard.jsonl");
    planned::tracked(&fixtures.join("early_draw_discard.ronl"), &fixtures.join("early_draw_discard.oracle.json"), &out).unwrap();
    let rows: BTreeMap<String, serde_json::Value> = std::fs::read_to_string(out).unwrap().lines().map(|s| {
        let v: serde_json::Value = serde_json::from_str(s).unwrap();
        (v["name"].as_str().unwrap().into(), v)
    }).collect();
    for name in ["Krovikan Sorcerer", "Casting of Bones", "Soldevi Sage"] {
        let face = &rows[name]["faces"][0];
        assert!(face["gaps"].as_array().unwrap().is_empty(), "{name}: {face}");
        let rust = face["rust"].as_str().unwrap();
        assert!(rust.contains("Event::Drew") && rust.contains("Zone::hand(player)"), "{name}: {rust}");
        assert!(rust.contains("Purpose::Discard, drawn"), "{name}: {rust}");
    }
}
