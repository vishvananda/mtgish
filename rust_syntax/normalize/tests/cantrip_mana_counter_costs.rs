#![cfg(feature = "cantrip")]
use mtgish_ast::cantrip::planned;
use std::{collections::BTreeMap, path::PathBuf};

fn rows(features: &[&str], tag: &str) -> BTreeMap<String, serde_json::Value> {
    rows_from("mana_counter_costs", features, tag)
}

fn rows_from(stem: &str, features: &[&str], tag: &str) -> BTreeMap<String, serde_json::Value> {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let output = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("mana-counters-{tag}.jsonl"));
    planned::with_features(features, || planned::tracked(&fixtures.join(format!("{stem}.ronl")),
        &fixtures.join(format!("{stem}.oracle.json")), &output)).unwrap();
    std::fs::read_to_string(output).unwrap().lines().map(|line| {
        let row: serde_json::Value = serde_json::from_str(line).unwrap();
        (row["name"].as_str().unwrap().to_string(), row)
    }).collect()
}

#[test]
fn variable_mana_combinations_keep_the_full_amount_across_color_question_chunks() {
    let rows = rows_from("mana_counter_combinations", &["mana-counter-costs"], "combinations-new");
    assert_eq!(rows.len(), 4);
    for (name, row) in rows {
        assert!(row["faces"][0]["gaps"].as_array().unwrap().is_empty(), "{name}: {row}");
        let module = row["file"].as_str().unwrap();
        let dense: String = module.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(dense.contains("mana_ability_x().unwrap_or(0)"), "{name}: {module}");
        if matches!(name.as_str(), "Calciform Pools" | "Wizard's Rockets") {
            assert!(dense.contains(".min(u32::from(u8::MAX))asu8"), "{name}: {module}");
        }
        assert!(!dense.contains(".min(u8::MAX.into())asu8"), "{name}: {module}");
        assert!(!dense.contains("q.obj(asker.source).x"), "{name}: {module}");
        syn::parse_file(module).unwrap();
    }
}

#[test]
fn old_targets_keep_the_gap_while_new_targets_use_the_activation_x_and_exact_counter_cost() {
    let old = rows(&[], "old");
    let new = rows(&["mana-counter-costs"], "new");
    assert_eq!(new.len(), 10);
    for (name, row) in new {
        assert!(!old[&name]["faces"][0]["gaps"].as_array().unwrap().is_empty(), "{name}");
        assert!(row["faces"][0]["gaps"].as_array().unwrap().is_empty(), "{name}: {row}");
        let module = row["file"].as_str().unwrap();
        assert!(module.contains("mana_ability_x()"), "{name}: {module}");
        assert!(module.contains(".x_is_mana_amount()"), "{name}: {module}");
        assert!(module.contains("VariableCost::RemoveAnyNumberOfCounters"), "{name}: {module}");
        let dense: String = module.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(!dense.contains("q.obj(asker.source).x"), "{name}: {module}");
        syn::parse_file(module).unwrap();
    }
}
