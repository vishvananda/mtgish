#![cfg(feature = "cantrip")]
//! A surviving blocking creature must not depend on a remaining attacker link.
use std::{collections::BTreeMap, path::PathBuf};

fn rows(features: &[&str], tag: &str) -> BTreeMap<String, serde_json::Value> {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let output = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("blocking-status-{tag}.jsonl"));
    mtgish_ast::cantrip::planned::with_features(features, || {
        mtgish_ast::cantrip::planned::tracked(&fixtures.join("blocking_status.ronl"),
            &fixtures.join("blocking_status.oracle.json"), &output)
    }).unwrap();
    std::fs::read_to_string(output).unwrap().lines().map(|line| {
        let row: serde_json::Value = serde_json::from_str(line).unwrap();
        (row["name"].as_str().unwrap().to_string(), row)
    }).collect()
}

#[test]
fn blocking_and_blocking_alone_use_independent_status_and_old_targets_keep_the_gap() {
    let old = rows(&["lasting-rules"], "old");
    let new = rows(&["lasting-rules", "blocking-status"], "new");
    assert_eq!(new.len(), 9);
    for (name, row) in &new {
        if name == "Lightning Bolt" { continue; }
        let gaps = old[name]["faces"][0]["gaps"].as_array().unwrap();
        assert!(gaps.iter().any(|g| g["feature"] == "blocking status after attacker leaves combat"), "{name}: {gaps:?}");
        assert!(row["faces"][0]["gaps"].as_array().unwrap().is_empty(), "{name}: {row}");
        let module = row["file"].as_str().unwrap();
        let dense: String = module.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(dense.contains("q.is_blocking("), "{name}: {module}");
        if name == "Thijarian Witness" {
            assert!(dense.contains("q.state().blockers.iter().all("), "{module}");
        }
        syn::parse_file(module).unwrap();
    }
    assert_eq!(old["Lightning Bolt"]["file"], new["Lightning Bolt"]["file"]);
}
