#![cfg(feature = "cantrip")]
use mtgish_ast::cantrip::planned;
use std::{collections::BTreeMap, path::PathBuf};

fn rows(features: &[&str], tag: &str) -> BTreeMap<String, serde_json::Value> {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let output = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("cost-receipts-{tag}.jsonl"));
    planned::with_features(features, || planned::tracked(&fixtures.join("cost_receipts.ronl"),
        &fixtures.join("cost_receipts.oracle.json"), &output)).unwrap();
    std::fs::read_to_string(output).unwrap().lines().map(|line| {
        let row: serde_json::Value = serde_json::from_str(line).unwrap();
        (row["name"].as_str().unwrap().to_string(), row)
    }).collect()
}

#[test]
fn chosen_sacrifices_use_their_cost_receipt_and_old_profiles_report_the_gap() {
    let old = rows(&[], "old");
    let new = rows(&["cost-object-receipts"], "new");
    for (name, expected) in [
        ("Priest of Yawgmoth", "q.mana_ability_cost_objects(0, CostPart::Permanents)"),
        ("Sacrifice", "q.history().spell_cost_objects(item.object, CostOrigin::PrintedAdditional, 0, CostPart::Permanents)"),
    ] {
        assert!(!old[name]["faces"][0]["gaps"].as_array().unwrap().is_empty(), "{name}");
        assert!(new[name]["faces"][0]["gaps"].as_array().unwrap().is_empty(), "{name}: {}", new[name]);
        let module = new[name]["file"].as_str().unwrap();
        let dense: String = module.chars().filter(|c| !c.is_whitespace() && *c != ',').collect();
        let expected: String = expected.chars().filter(|c| !c.is_whitespace() && *c != ',').collect();
        assert!(dense.contains(&expected), "{name}: {module}");
        assert!(!module.contains("rposition(|r|"));
        assert!(!module.contains("q.events().iter().rev().find_map"));
        syn::parse_file(module).unwrap();
    }
}
