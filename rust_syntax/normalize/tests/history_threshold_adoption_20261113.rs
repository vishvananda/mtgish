use mtgish_ast::{corpus, mtg_types::OracleCard, unified, validate};
use serde_json::{json, Value};

#[test]
fn published_spell_history_existence_thresholds_match_the_existing_controls() {
    corpus::on_large_stack(|| {
        let expected = json!({"kind":"Compare","args":{
            "comparison":"GreaterOrEqual","value":{"kind":"Constant","args":1}}});
        let paths = [
            (
                "Dream Thief",
                "/1/args/body/instructions/0/args/condition/args/test",
            ),
            (
                "Hotheaded Giant",
                "/1/args/body/instructions/0/args/condition/args/args/test",
            ),
            (
                "I Bask in Your Silent Awe",
                "/1/args/intervening_if/args/left/args/args/1/args/condition/args/test",
            ),
            (
                "Laboratory Drudge",
                "/0/args/body/instructions/0/args/condition/args/0/args/test",
            ),
            ("Spider-Man 2099", "/3/args/intervening_if/args/1/args/test"),
        ];
        let mut corrected = 0;
        let mut controls = 0;
        for line in include_str!("fixtures/history_threshold_adoption_20261113.ronl").lines() {
            let source: OracleCard = ron::from_str(line).unwrap();
            let lowered = unified::lower(&source);
            assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
            validate::unified(&lowered.card).unwrap();
            unified::round_trip(&lowered.card).unwrap();
            let card = &lowered.card.faces[0];
            let abilities = serde_json::to_value(&card.abilities).unwrap();
            if let Some((_, path)) = paths.iter().find(|(name, _)| *name == card.name) {
                assert_eq!(abilities.pointer(path), Some(&expected), "{}", card.name);
                corrected += 1;
            } else {
                let mut checks = Vec::new();
                cast_checks(&abilities, &mut checks);
                assert!(!checks.is_empty(), "{}", card.name);
                assert!(
                    checks.iter().all(|test| **test == expected),
                    "{}",
                    card.name
                );
                controls += 1;
            }
        }
        assert_eq!((corrected, controls), (5, 3));
    });
}

fn cast_checks<'a>(value: &'a Value, out: &mut Vec<&'a Value>) {
    if value["kind"] == "Compare"
        && value["args"]["left"]["kind"] == "EventHistory"
        && value["args"]["left"]["args"]["event"]["Spell"]["event"] == "Cast"
    {
        out.push(&value["args"]["test"]);
    }
    match value {
        Value::Array(values) => values.iter().for_each(|v| cast_checks(v, out)),
        Value::Object(values) => values.values().for_each(|v| cast_checks(v, out)),
        _ => (),
    }
}
