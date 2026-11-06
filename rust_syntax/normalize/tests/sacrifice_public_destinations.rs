use mtgish_ast::{corpus, lower, mtg_types::OracleCard, validate};
use serde_json::Value;
fn records() -> Vec<OracleCard> {
    include_str!("fixtures/sacrifice_public_destinations.ronl")
        .lines()
        .map(|l| ron::from_str(l).unwrap())
        .collect()
}
fn check_moves(v: &Value, count: &mut usize) {
    if v["kind"] == "Move"
        && v["args"]["objects"]
            == serde_json::json!({"kind":"One","args":{"kind":"EventDestination"}})
    {
        *count += 1;
    }
    match v {
        Value::Object(m) => m.values().for_each(|x| check_moves(x, count)),
        Value::Array(a) => a.iter().for_each(|x| check_moves(x, count)),
        _ => {}
    }
}
#[test]
fn sacrifice_followups_keep_the_actual_first_public_destination() {
    for source in records() {
        let name = corpus::Identity::of(&source).unwrap().names;
        let lowered = mtgish_ast::unified::lower(&source);
        assert!(
            lowered.diagnostics.is_empty(),
            "{name:?}: {:?}",
            lowered.diagnostics
        );
        assert!(
            lowered.blockers.is_empty(),
            "{name:?}: {:?}",
            lowered.blockers
        );
        validate::unified(&lowered.card).unwrap();
        let mut moved = 0;
        check_moves(&serde_json::to_value(&lowered.card).unwrap(), &mut moved);
        assert!(moved > 0, "{name:?}");
    }
}
#[test]
fn a_triggered_card_action_requires_a_card_moving_event() {
    let action = mtgish_ast::mtg_types::Action::PutTriggeredCardOntoBattlefield(vec![]);
    let mut source = records().remove(2);
    // A standalone spell has no causal event from which to obtain this card.
    if let mtgish_ast::mtg_types::OracleCard::Card { rules, .. } = &mut source {
        *rules = Some(vec![mtgish_ast::mtg_types::Rule::SpellActions(Box::new(
            mtgish_ast::mtg_types::Actions::ActionList(vec![action]),
        ))]);
    }
    assert!(lower::card(&source).is_err());
}
