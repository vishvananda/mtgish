#![cfg(feature = "cantrip")]
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf, sync::OnceLock};

fn rows() -> &'static BTreeMap<String, serde_json::Value> {
    static ROWS: OnceLock<BTreeMap<String, serde_json::Value>> = OnceLock::new();
    ROWS.get_or_init(|| {
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-control-mana.jsonl");
        cantrip::planned::tracked(&fixtures.join("early_control_mana_20261113.ronl"),
            &fixtures.join("early_control_mana_20261113.oracle.json"), &out).unwrap();
        std::fs::read_to_string(out).unwrap().lines().map(|line| {
            let row: serde_json::Value = serde_json::from_str(line).unwrap();
            (row["name"].as_str().unwrap().to_string(), row)
        }).collect()
    })
}
fn generated(name: &str) -> String {
    let face = &rows()[name]["faces"][0];
    assert!(face["gaps"].as_array().unwrap().is_empty(), "{name}: {}", face["gaps"]);
    let code = face["rust"].as_str().unwrap();
    syn::parse_file(code).unwrap();
    code.to_string()
}
#[test]
fn source_control_uses_the_uninterrupted_native_interval() {
    for name in ["Merieke Ri Berit", "Orcish Squatters"] {
        let code = generated(name);
        assert!(code.contains("Duration::WhileSourceControlled"), "{name}: {code}");
        assert!(!code.contains("Duration::AsLongAs("), "{name}: {code}");
    }
}
#[test]
fn compound_land_replacements_have_distinct_verbatim_choice_labels() {
    for (name, clauses) in [
        ("Naked Singularity", vec!["Plains produce {R}", "Islands produce {G}", "Swamps produce {W}", "Mountains produce {U}", "Forests produce {B}"]),
        ("Reality Twist", vec!["Plains produce {R}", "Swamps produce {G}", "Mountains produce {W}", "Forests produce {B}"]),
    ] {
        let code = generated(name);
        for clause in clauses {
            assert!(code.contains(&format!("\"{clause}\"")), "{name}: {code}");
        }
    }
}

#[test]
fn older_native_profiles_keep_the_custom_duration_fallback() {
    use mtgish_ast::{corpus, lower, mtg_types::OracleCard};
    let source: OracleCard = include_str!("fixtures/early_control_mana_20261113.ronl").lines()
        .map(|line| ron::from_str(line).unwrap())
        .find(|card| corpus::Identity::of(card).unwrap().names.iter().any(|n| n == "Orcish Squatters"))
        .unwrap();
    let card = lower::card(&source).unwrap();
    let code = cantrip::planned::without_features(&["source-control-intervals"], || cantrip::module(&card)).unwrap();
    assert!(code.contains("Duration::AsLongAs("), "{code}");
    assert!(!code.contains("Duration::WhileSourceControlled"), "{code}");
}
