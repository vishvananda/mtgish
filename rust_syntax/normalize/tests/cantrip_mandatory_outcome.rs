#![cfg(feature = "cantrip")]
//! Compulsory effect-shaped pseudo-costs report the actual result, not a choice to attempt it.
use mtgish_ast::cantrip::planned;
use std::{collections::BTreeMap, path::PathBuf, sync::OnceLock};

fn generated() -> &'static BTreeMap<String, String> {
    static GENERATED: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    GENERATED.get_or_init(|| {
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let output = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("mandatory-outcome.jsonl");
        planned::tracked(&fixtures.join("mandatory_outcome.ronl"), &fixtures.join("mandatory_outcome.oracle.json"), &output).unwrap();
        std::fs::read_to_string(output).unwrap().lines().map(|line| {
            let row: serde_json::Value = serde_json::from_str(line).unwrap();
            (row["name"].as_str().unwrap().to_string(), row["file"].as_str().unwrap().to_string())
        }).collect()
    })
}

#[test]
fn cosmic_horror_tests_the_actual_destruction_before_dealing_damage() {
    let code = &generated()["Cosmic Horror"];
    assert!(code.contains("g.destroy_all("), "{code}");
    assert!(!code.contains("(true as i64)"), "a mandatory destruction can fail: {code}");
}

#[test]
fn puppet_master_tests_the_return_to_hand_before_offering_payment() {
    let code = &generated()["Puppet Master"];
    assert!(code.contains("g.return_to_hand("), "{code}");
    assert!(!code.contains("(true as i64)"), "the dead card can be gone or its return replaced: {code}");
    assert!(code.contains("ZoneKind::Hand"), "an actual return to hand matters, not movement to another zone: {code}");
}

#[test]
fn infinite_authority_only_schedules_its_counter_after_its_own_destruction() {
    let code = &generated()["Infinite Authority"];
    assert!(code.contains("g.destroy_all("), "{code}");
    assert!(!code.contains("(true as i64)"), "a creature dying earlier does not satisfy this destruction: {code}");
}

#[test]
fn compulsory_effect_receipts_are_distinct_from_real_payment_choices_in_the_ir() {
    for (line, real_payments) in include_str!("fixtures/mandatory_outcome.ronl").lines().zip([1, 1, 0]) {
        let source = ron::from_str(line).unwrap();
        let card = mtgish_ast::lower::card(&source).unwrap();
        let ir = serde_json::to_string(&card).unwrap();
        assert_eq!(ir.matches("PaymentChosen").count(), real_payments, "{ir}");
        if ir.contains("Puppet Master") {
            assert!(ir.contains("InZone") && ir.contains("Hand"), "{ir}");
        }
    }
}
