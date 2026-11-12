use mtgish_ast::{corpus, ir::*, lower, mtg_types::OracleCard, unified, validate};
use std::collections::BTreeMap;
fn cards() -> BTreeMap<String, Card> {
    include_str!("fixtures/defending_player_roles_20261112.ronl")
        .lines()
        .flat_map(|line| {
            let source: OracleCard = ron::from_str(line).unwrap();
            let result = unified::lower(&source);
            assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
            validate::unified(&result.card).unwrap();
            unified::round_trip(&result.card).unwrap();
            assert_eq!(
                serde_json::to_value(lower::card(&source).unwrap().abilities).unwrap(),
                serde_json::to_value(&result.card.faces[0].abilities).unwrap()
            );
            result.card.faces
        })
        .map(|c| (c.name.clone(), c))
        .collect()
}
fn states(card: &Card) -> Vec<PlayerState> {
    fn visit(value: &serde_json::Value, out: &mut Vec<PlayerState>) {
        if value["kind"] == "PlayerState" {
            out.push(serde_json::from_value(value["args"]["state"].clone()).unwrap());
        }
        match value {
            serde_json::Value::Object(fields) => fields.values().for_each(|v| visit(v, out)),
            serde_json::Value::Array(values) => values.iter().for_each(|v| visit(v, out)),
            _ => {}
        }
    }
    let mut out = vec![];
    visit(&serde_json::to_value(card).unwrap(), &mut out);
    out
}
#[test]
fn defending_player_uses_the_existing_combat_role_for_each_spell_template() {
    corpus::on_large_stack(|| {
        let cards = cards();
        assert_eq!(cards.len(), 6);
        for name in ["False Orders", "Blaze of Glory", "Yare", "Stand or Fall"] {
            let states = states(&cards[name]);
            assert!(
                states.contains(&PlayerState::PossibleDefender),
                "{name}: {states:?}"
            );
            assert!(!states.contains(&PlayerState::BeingAttacked), "{name}");
        }
    });
}
#[test]
fn personally_attacked_players_keep_the_distinct_state() {
    corpus::on_large_stack(|| {
        let cards = cards();
        for name in ["Commander Liara Portyr", "Suppressor Skyguard"] {
            let states = states(&cards[name]);
            assert!(
                states.contains(&PlayerState::BeingAttacked),
                "{name}: {states:?}"
            );
            assert!(!states.contains(&PlayerState::PossibleDefender), "{name}");
        }
    });
}
