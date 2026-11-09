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

#[test]
#[cfg(feature = "cantrip")]
fn unsigned_reflexive_carrier_does_not_silently_change_signed_values() {
    corpus::on_large_stack(|| {
        let line = include_str!("fixtures/oracle_fixes_20261107.ronl")
            .lines()
            .find(|s| s.contains("Name: \"Yuna's Whistle\""))
            .unwrap();
        let source: source::OracleCard = ron::from_str(line).unwrap();
        let mut card = unified::lower(&source).card.faces.remove(0);
        fn signed(body: &mut Program) -> bool {
            for step in &mut body.instructions {
                match step {
                    Instruction::Effect {
                        operation: Operation::BindValue { value },
                        ..
                    } => {
                        *value = Expr::Constant(-1);
                        return true;
                    }
                    Instruction::Group(inner) => {
                        if signed(inner) {
                            return true;
                        }
                    }
                    _ => {}
                }
            }
            false
        }
        let Ability::Spell { body, .. } = &mut card.abilities[0] else {
            panic!()
        };
        assert!(signed(body));
        validate::card(&card).unwrap();
        let error = mtgish_ast::cantrip::module(&card).unwrap_err();
        assert_eq!(error.feature, "value outside generated scope");
    });
}
