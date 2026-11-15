#[path = "support/legacy_entries.rs"]
mod legacy_entries;
use mtgish_ast::{canonical, corpus, ir::*, mtg_types as source, unified, validate};
use serde_json::Value;

fn normalized(mut value: Value) -> Value {
    legacy_entries::json(&mut value);
    let abilities: Vec<Ability> = serde_json::from_value(value.clone()).unwrap();
    validate::abilities(&abilities).unwrap();
    canonical::abilities(&mut value).unwrap();
    let abilities: Vec<Ability> = serde_json::from_value(value.clone()).unwrap();
    validate::abilities(&abilities).unwrap();
    let mut twice = value.clone();
    canonical::abilities(&mut twice).unwrap();
    assert_eq!(value, twice);
    value
}

#[test]
fn reported_references_preserve_the_complete_corrected_program() {
    corpus::on_large_stack(|| {
        let expected: Value =
            serde_json::from_str(include_str!("fixtures/oracle_fixes_20261107.json")).unwrap();
        let mut mismatches = Vec::new();
        for line in include_str!("fixtures/oracle_fixes_20261107.ronl").lines() {
            let source: source::OracleCard = ron::from_str(line).unwrap();
            let lowered = unified::lower(&source);
            assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
            for card in lowered.card.faces {
                validate::card(&card).unwrap_or_else(|e| panic!("{}: {e}", card.name));
                let actual = normalized(serde_json::to_value(&card.abilities).unwrap());
                let expected = normalized(expected[&card.name].clone());
                if actual != expected {
                    if let Ok(dir) = std::env::var("ORACLE_FIX_EVIDENCE") {
                        std::fs::write(
                            format!("{dir}/{}.actual.json", card.name),
                            serde_json::to_vec_pretty(&actual).unwrap(),
                        )
                        .unwrap();
                        std::fs::write(
                            format!("{dir}/{}.expected.json", card.name),
                            serde_json::to_vec_pretty(&expected).unwrap(),
                        )
                        .unwrap();
                    }
                    mismatches.push(card.name);
                }
            }
        }
        assert!(mismatches.is_empty(), "{mismatches:?}");
    });
}
