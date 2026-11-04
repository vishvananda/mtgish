#![cfg(feature = "cantrip")]
//! Whole printed cards use the native P/T counter metadata only when the target has it.
use mtgish_ast::cantrip::planned;
use std::{collections::BTreeMap, path::PathBuf};

fn rows(features: &[&str], tag: &str) -> BTreeMap<String, serde_json::Value> {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let output = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("pt-counters-{tag}.jsonl"));
    planned::with_features(features, || planned::tracked(&fixtures.join("pt_counters.ronl"), &fixtures.join("pt_counters.oracle.json"), &output)).unwrap();
    std::fs::read_to_string(output).unwrap().lines().map(|s| {
        let row: serde_json::Value = serde_json::from_str(s).unwrap();
        (row["name"].as_str().unwrap().to_string(), row)
    }).collect()
}

#[test]
fn nonstandard_counters_keep_their_distinct_kinds_and_old_targets_keep_the_gap() {
    let old = rows(&[], "old");
    let new = rows(&["pt-counters"], "new");
    for (name, kinds) in [
        ("Armor Thrull", &["Plus1Plus2"][..]), ("Balduvian Hydra", &["Plus1Plus0"][..]),
        ("Clockwork Avian", &["Plus1Plus0"][..]), ("Dwarven Armorer", &["Plus0Plus1", "Plus1Plus0"][..]),
        ("Dwarven Armory", &["Plus2Plus2"][..]), ("Ebon Praetor", &["Minus2Minus2", "Plus1Plus0"][..]),
        ("Essence Flare", &["Plus0Minus1"][..]), ("Lesser Werewolf", &["Plus0Minus1"][..]),
        ("Living Armor", &["Plus0Plus1"][..]), ("Spirit Shackle", &["Plus0Minus2"][..]),
    ] {
        let held = old[name]["faces"][0]["rust"].as_str().unwrap();
        let enabled = new[name]["faces"][0]["rust"].as_str().unwrap();
        assert!(held.contains("power/toughness counter other than ±1/±1"), "{name}: {held}");
        assert!(!enabled.contains("power/toughness counter other than ±1/±1"), "{name}: {enabled}");
        for kind in kinds { assert!(enabled.contains(&format!("CounterKind::{kind}")), "{name}: {enabled}"); }
        assert!(new[name]["faces"][0]["gaps"].as_array().unwrap().is_empty(), "{name}: {}", new[name]);
    }
}
