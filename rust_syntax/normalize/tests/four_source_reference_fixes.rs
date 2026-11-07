use mtgish_ast::{canonical, corpus, ir::*, lower, mtg_types as source, validate};
use serde_json::Value;
fn normalized(mut v: Value) -> Value {
    let abilities: Vec<Ability> = serde_json::from_value(v.clone()).unwrap();
    validate::abilities(&abilities).unwrap();
    canonical::abilities(&mut v).unwrap();
    let abilities: Vec<Ability> = serde_json::from_value(v.clone()).unwrap();
    validate::abilities(&abilities).unwrap();
    let mut twice = v.clone();
    canonical::abilities(&mut twice).unwrap();
    assert_eq!(v, twice);
    v
}
#[test]
fn complete_programs_preserve_unrelated_operands_and_fix_all_four_reported_errors() {
    corpus::on_large_stack(|| {
        let expected: Value =
            serde_json::from_str(include_str!("fixtures/four_source_reference_fixes.json"))
                .unwrap();
        for line in include_str!("fixtures/four_source_reference_fixes.ronl").lines() {
            let source: source::OracleCard = ron::from_str(line).unwrap();
            let c = lower::card(&source).unwrap();
            validate::card(&c).unwrap();
            #[cfg(feature = "cantrip")]
            mtgish_ast::cantrip::module(&c).unwrap_or_else(|error| panic!("{}: {error:?}", c.name));
            let actual = normalized(serde_json::to_value(c.abilities).unwrap());
            let expected = normalized(expected[&c.name].clone());
            if let Ok(dir) = std::env::var("REFERENCE_FIX_EVIDENCE") {
                std::fs::write(
                    format!("{dir}/{}.actual.json", c.name),
                    serde_json::to_vec_pretty(&actual).unwrap(),
                )
                .unwrap();
                std::fs::write(
                    format!("{dir}/{}.expected.json", c.name),
                    serde_json::to_vec_pretty(&expected).unwrap(),
                )
                .unwrap();
            }
            assert_eq!(actual, expected, "{}", c.name);
        }
    });
}
