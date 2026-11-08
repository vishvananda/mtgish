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
fn complete_return_followup_programs_match_reviewed_group_and_snapshot_repairs() {
    corpus::on_large_stack(|| {
        let expected: Value = serde_json::from_str(include_str!(
            "fixtures/return_followup_reference_fixes.json"
        ))
        .unwrap();
        for line in include_str!("fixtures/return_followup_reference_fixes.ronl").lines() {
            let source: source::OracleCard = ron::from_str(line).unwrap();
            let c = lower::card(&source).unwrap();
            validate::card(&c).unwrap();
            #[cfg(feature = "cantrip")]
            {
                let code = mtgish_ast::cantrip::module(&c);
                if matches!(
                    c.name.as_str(),
                    "Sundering Sentinel" | "Teysa of the Ghost Council"
                ) {
                    assert!(code.as_ref().unwrap_err().feature.contains("Intensity"));
                } else {
                    code.as_ref()
                        .unwrap_or_else(|error| panic!("{}: {error:?}", c.name));
                }
                if let Ok(dir) = std::env::var("REFERENCE_FIX_EVIDENCE") {
                    match code {
                        Ok(code) => std::fs::write(format!("{dir}/{}.rs", c.name), code).unwrap(),
                        Err(error) => std::fs::write(
                            format!("{dir}/{}.gap.json", c.name),
                            serde_json::to_vec_pretty(&error).unwrap(),
                        )
                        .unwrap(),
                    }
                }
            }
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
