use mtgish_ast::{lower, mtg_types::OracleCard, unified, validate};
use serde_json::json;

#[test]
fn quagnoth_follows_the_discarded_card_without_a_graveyard_guard() {
    let line = include_str!("fixtures/discard_return.ronl").trim();
    let source: OracleCard = ron::from_str(line).unwrap();
    let lowered = unified::lower(&source);
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    validate::unified(&lowered.card).unwrap();
    unified::round_trip(&lowered.card).unwrap();
    let mut fixed = serde_json::to_value(&lowered.card).unwrap();
    let path = "/faces/0/abilities/2/args/ability/args/body/instructions/0/args/operation/args/objects/args";
    assert_eq!(fixed.pointer(path), Some(&json!({"kind":"Discarded"})), "{fixed}");
    // Compare the whole original reference. Only the returned object changes;
    // the trigger, controller, destination, keywords and every other operand stay.
    let old: OracleCard = ron::from_str(&line.replace("ReturnDiscardedCardToHand",
        "ReturnGraveyardCardToHand(Trigger_ThatGraveyardCard)")).unwrap();
    let old = serde_json::to_value(unified::lower(&old).card).unwrap();
    assert_eq!(old.pointer(&format!("{path}/args/objects/args/zone")), Some(&json!("Graveyard")));
    *fixed.pointer_mut(path).unwrap() = old.pointer(path).unwrap().clone();
    assert_eq!(fixed, old, "only the unprinted zone restriction may change");
}

#[test]
fn discarded_return_requires_a_discard_event() {
    let source: OracleCard = ron::from_str(r#"Card(Name: "Invalid", Typeline: (Supertypes: [], Cardtypes: [Instant], Subtypes: []), Rules: [SpellActions(ActionList([ReturnDiscardedCardToHand]))])"#).unwrap();
    assert!(lower::card(&source).is_err());
}
