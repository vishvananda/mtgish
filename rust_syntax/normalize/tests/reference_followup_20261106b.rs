//! Independent complete expected programs supplied by the Oracle investigation.
use mtgish_ast::{canonical, lower, mtg_types::OracleCard, validate};
use serde_json::Value;
#[test]
fn new_reference_operands_match_the_complete_reviewed_programs() {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/reference_followup_expected.json")).unwrap();
    for line in include_str!("fixtures/reference_followup_20261106b.ronl").lines() {
        let source: OracleCard = ron::from_str(line).unwrap();
        let card = lower::card(&source).unwrap();
        validate::card(&card).unwrap();
        let case = cases.iter().find(|c| c["name"] == card.name).unwrap();
        let mut expected = case["expected"].clone();
        let mut historical = case["historical"].clone();
        let mut actual = serde_json::to_value(&card.abilities).unwrap();
        canonical::abilities(&mut expected).unwrap();
        canonical::abilities(&mut historical).unwrap();
        canonical::abilities(&mut actual).unwrap();
        assert_eq!(actual, expected, "{}", card.name);
        assert_ne!(actual, historical, "{}: old defect", card.name);
    }
}
