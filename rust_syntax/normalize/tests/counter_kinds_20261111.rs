use mtgish_ast::{canonical, corpus, ir::Ability, mtg_types::OracleCard, unified, validate};
use serde_json::Value;
fn normalized(abilities: &[Ability]) -> Value {
    validate::abilities(abilities).unwrap();
    let mut value = serde_json::to_value(abilities).unwrap();
    canonical::abilities(&mut value).unwrap();
    value
}
fn batches(value: &Value) -> usize {
    match value {
        Value::Object(map) => {
            usize::from(
                value["kind"] == "PutCounters"
                    && value["args"]["placements"]
                        .as_array()
                        .is_some_and(|p| p.len() == 2),
            ) + map.values().map(batches).sum::<usize>()
        }
        Value::Array(values) => values.iter().map(batches).sum(),
        _ => 0,
    }
}
#[test]
fn three_repairs_and_five_controls_keep_one_batch_and_complete_program_operands() {
    corpus::on_large_stack(|| {
        let expected: Value = serde_json::from_str(include_str!(
            "fixtures/counter_kinds_20261111_expected.json"
        ))
        .unwrap();
        let mut seen = 0;
        for line in include_str!("fixtures/counter_kinds_20261111.ronl").lines() {
            let source: OracleCard = ron::from_str(line).unwrap();
            let result = unified::lower(&source);
            assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
            let card = &result.card.faces[0];
            validate::card(card).unwrap();
            let wanted: Vec<Ability> =
                serde_json::from_value(expected[&card.name].clone()).unwrap();
            let actual = normalized(&card.abilities);
            assert_eq!(actual, normalized(&wanted), "{}", card.name);
            assert_eq!(batches(&actual), 1, "{}", card.name);
            seen += 1;
        }
        assert_eq!(seen, 8);
    });
}

#[path = "support/source_json.rs"]
mod source_json;
#[test]
fn tagged_source_and_ron_keep_the_same_eight_complete_cards() {
    corpus::on_large_stack(|| {
        source_json::assert_fixture_matches(include_str!("fixtures/counter_kinds_20261111.ronl"))
    });
}
