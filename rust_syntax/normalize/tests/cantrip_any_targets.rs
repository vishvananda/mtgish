#![cfg(feature = "cantrip")]
//! Target cardinality uses TargetSpec's sentinel; the selection helper's u32::MAX means COUNTED to TargetSpec.
use mtgish_ast::cantrip;
use std::path::PathBuf;

#[test]
fn any_number_of_targets_is_not_an_x_counted_slot() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let output = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("any-target-counts.jsonl");
    cantrip::planned::tracked(&fixtures.join("any_targets.ronl"), &fixtures.join("any_targets.oracle.json"), &output).unwrap();
    let mut tested = 0;
    for line in std::fs::read_to_string(output).unwrap().lines() {
        let row: serde_json::Value = serde_json::from_str(line).unwrap();
        let name = row["name"].as_str().unwrap();
        let file = row["file"].as_str().unwrap_or_else(|| row["faces"][0]["rust"].as_str().unwrap());
        if name == "Miasma Demon" {
            assert!(file.contains("TargetSpec::COUNTED"), "{file}");
        } else {
            assert!(file.contains("TargetSpec::ANY_NUMBER"), "{name}: {file}");
            assert!(!file.contains("cantrip_mechanics::ANY_NUMBER"), "{name}: {file}");
            assert!(!file.contains("TargetSpec::COUNTED"), "{name}: {file}");
            assert!(row["faces"][0]["gaps"].as_array().unwrap().is_empty(), "{name}: {row}");
            tested += 1;
        }
    }
    assert_eq!(tested, 5);
}
