use mtgish_ast::{mtg_types::OracleCard, unified, validate};
use serde_json::{json, Value};

fn lower(text: &str) -> Value {
    let source: OracleCard = ron::from_str(text.trim()).unwrap();
    let lowered = unified::lower(&source);
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    validate::unified(&lowered.card).unwrap();
    unified::round_trip(&lowered.card).unwrap();
    serde_json::to_value(lowered.card).unwrap()
}

#[test]
fn blockers_beyond_the_first_cannot_be_negative() {
    for (before, after) in include_str!("fixtures/johtull_wurm_before.ronl")
        .lines()
        .zip(include_str!("fixtures/johtull_wurm.ronl").lines())
    {
        let mut expected = lower(before);
        let fixed = lower(after);
        let path =
            "/faces/0/abilities/0/args/body/instructions/0/args/operation/args/changes/0/args";
        let pump = expected.pointer_mut(path).unwrap();
        for field in ["power", "toughness"] {
            assert_eq!(pump[field]["kind"], "Multiply");
            let count = pump[field]["args"][1].take();
            assert_eq!(count["kind"], "Subtract");
            assert_eq!(count["args"][0]["kind"], "CountObjects");
            assert_eq!(count["args"][1], json!({"kind":"Constant","args":1}));
            pump[field]["args"][1] =
                json!({"kind":"Maximum","args":[{"kind":"Constant","args":0}, count]});
        }
        assert_eq!(
            fixed, expected,
            "only the count of blockers beyond the first changes"
        );
    }
}
