use mtgish_ast::{ir::*, mtg_types::OracleCard, unified, validate};
use serde_json::json;

#[test]
fn life_totals_use_the_general_player_quantity() {
    let source: OracleCard = ron::from_str(r#"Card(Name: "Life read", Typeline: (Supertypes: [], Cardtypes: [Sorcery], Subtypes: []), Rules: [SpellActions(ActionList([DrawNumberCards(LifeTotalOfPlayer(You))]))])"#).unwrap();
    let lowered = unified::lower(&source);
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    validate::unified(&lowered.card).unwrap();
    unified::round_trip(&lowered.card).unwrap();
    let value = serde_json::to_value(&lowered.card).unwrap();
    assert_eq!(
        value
            .pointer("/faces/0/abilities/0/args/body/instructions/0/args/operation/args/amount")
            .unwrap(),
        &json!({"kind":"PlayerNumber", "args":{"player":{"kind":"AbilityController"},"number":"Life"}})
    );
}

#[test]
fn real_life_reading_records_keep_their_scopes_and_roundtrip() {
    for line in include_str!("fixtures/player_life.ronl").lines() {
        let source: OracleCard = ron::from_str(line).unwrap();
        let lowered = unified::lower(&source);
        validate::unified(&lowered.card).unwrap();
        unified::round_trip(&lowered.card).unwrap();
        assert!(!serde_json::to_string(&lowered.card)
            .unwrap()
            .contains("\"PlayerLife\""));
    }
    assert_eq!(
        serde_json::to_value(Expr::player_life(PlayerRef::AbilityController)).unwrap(),
        json!({"kind":"PlayerNumber", "args":{"player":{"kind":"AbilityController"}, "number":"Life"}})
    );
}
