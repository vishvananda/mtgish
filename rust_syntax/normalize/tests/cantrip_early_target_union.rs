#![cfg(feature = "cantrip")]
use mtgish_ast::cantrip::planned;
use std::path::PathBuf;

#[test]
fn battlefield_target_union_checks_each_target_and_preserves_additional_cost() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-target-union.jsonl");
    planned::tracked(&fixtures.join("early_target_union.ronl"), &fixtures.join("early_target_union.oracle.json"), &out).unwrap();
    let row: serde_json::Value = serde_json::from_str(std::fs::read_to_string(out).unwrap().trim()).unwrap();
    let face = &row["faces"][0];
    assert!(face["gaps"].as_array().unwrap().is_empty(), "{face}");
    let rust = face["rust"].as_str().unwrap();
    assert!(rust.contains("t_0_raw.get(0).copied().filter(|t| t_0.contains(t))"), "{rust}");
    assert!(rust.contains("t_1_raw.get(0).copied().filter(|t| t_1.contains(t))"), "{rust}");
    assert!(rust.contains("additional_cost: Some(Cost::NONE"), "{rust}");
    assert!(rust.contains(".and(life(3))"), "{rust}");
    assert!(rust.contains("!all.contains(&object)"), "{rust}");
}
