//! Published source-valid programs must stay valid through normalization.
use mtgish_ast::{canonical, ir::Card, validate};
use serde_json::Value;
#[test]
fn immediate_loop_and_payment_receipts_remain_available() {
    for line in include_str!("fixtures/canonical_immediate_results.jsonl").lines() {
        let record: Value = serde_json::from_str(line).unwrap();
        for face in record["faces"].as_array().unwrap() {
            let mut card: Card = serde_json::from_value(face.clone()).unwrap();
            // These published programs use only unchanged forms. Keep the
            // historical fixture intact and validate its operands at this schema.
            card.schema_version = mtgish_ast::ir::SCHEMA_VERSION;
            validate::card(&card).unwrap();
            let normal = canonical::card(&card).unwrap();
            validate::card(&normal).unwrap_or_else(|e| panic!("{}: {e}", card.name));
            assert_eq!(
                serde_json::to_value(&normal).unwrap(),
                serde_json::to_value(canonical::card(&normal).unwrap()).unwrap()
            );
        }
    }
}
