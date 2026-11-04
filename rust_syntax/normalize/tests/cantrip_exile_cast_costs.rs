#![cfg(feature = "cantrip")]
//! Ordinary casting permissions retain printed mana costs in both ExileCast constructors.
use std::{collections::BTreeSet, path::PathBuf};

#[test]
fn exile_permissions_explicitly_retain_the_normal_casting_cost() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let output = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("exile-cast-costs.jsonl");
    mtgish_ast::cantrip::planned::tracked(&fixtures.join("exile_cast_costs.ronl"),
        &fixtures.join("exile_cast_costs.oracle.json"), &output).unwrap();
    let mut names = BTreeSet::new();
    for line in std::fs::read_to_string(output).unwrap().lines() {
        let row: serde_json::Value = serde_json::from_str(line).unwrap();
        let name = row["name"].as_str().unwrap();
        assert!(row["faces"][0]["gaps"].as_array().unwrap().is_empty(), "{name}: {row}");
        let module = row["file"].as_str().unwrap();
        let dense: String = module.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(dense.contains("ExileCast{"), "{name}: {module}");
        assert!(dense.contains("instead:None"), "{name}: {module}");
        assert!(!dense.contains("instead:Some("), "{name}: {module}");
        syn::parse_file(module).unwrap();
        names.insert(name.to_string());
    }
    assert_eq!(names, BTreeSet::from(["Ian Malcolm, Chaotician".to_string(),
        "Misthollow Griffin".to_string(), "Eternal Scourge".to_string()]));
}
