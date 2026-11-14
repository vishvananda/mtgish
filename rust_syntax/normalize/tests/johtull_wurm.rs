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
    for source in include_str!("fixtures/johtull_wurm.ronl").lines() {
        assert!(!source.contains("MaximumOf"), "retain upstream source semantics");
        let fixed = lower(source);
        let path =
            "/faces/0/abilities/0/args/body/instructions/0/args/operation/args/changes/0/args";
        let pump = fixed.pointer(path).unwrap();
        for field in ["power", "toughness"] {
            assert_eq!(pump[field]["kind"], "Multiply");
            let count = &pump[field]["args"][1];
            assert_eq!(count["kind"], "Maximum");
            assert_eq!(count["args"][0], json!({"kind":"Constant","args":0}));
            let count = &count["args"][1];
            assert_eq!(count["kind"], "Subtract");
            assert_eq!(count["args"][0]["kind"], "CountObjects");
            assert_eq!(count["args"][1], json!({"kind":"Constant","args":1}));
        }
        let explicit = source.replace(
            "Minus(TheNumberOfPermanentsOnTheBattlefield(And([IsCardtype(Creature), IsBlockingAttacker(Trigger_ThatCreature)])), Integer(1))",
            "MaximumOf(Integer(0), Minus(TheNumberOfPermanentsOnTheBattlefield(And([IsCardtype(Creature), IsBlockingAttacker(Trigger_ThatCreature)])), Integer(1)))",
        );
        assert_eq!(fixed, lower(&explicit), "explicit clamp is not duplicated");
    }
}

fn pump(effect: &str) -> Value {
    let value = lower(&format!(r#"Card(Name:"Count contract", Typeline:(Supertypes:[],Cardtypes:[Instant],Subtypes:[]), Rules:[SpellActions(ActionList([CreatePermanentLayerEffectUntil(ThisPermanent,[{effect}],UntilEndOfTurn)]))])"#));
    value.pointer("/faces/0/abilities/0/args/body/instructions/0/args/operation/args/changes/0/args").unwrap().clone()
}

#[test]
fn repetition_counts_clamp_before_applying_positive_or_negative_increments() {
    for count in [-3, 0, 1, 4] {
        for (power, toughness) in [(-2, -1), (2, 1)] {
            let got = pump(&format!("AdjustPTForEach({power}, {toughness}, Integer({count}))"));
            assert_eq!(got["power"], json!({"kind":"Constant","args":power * count.max(0)}));
            assert_eq!(got["toughness"], json!({"kind":"Constant","args":toughness * count.max(0)}));
        }
    }
}

#[test]
fn ordinary_signed_adjustments_keep_negative_values() {
    let got = pump("AdjustPT(-2, -1)");
    assert_eq!(got["power"], json!({"kind":"Constant","args":-2}));
    assert_eq!(got["toughness"], json!({"kind":"Constant","args":-1}));
    let got = pump("AdjustPTX(PlusX, MinusX, Integer(-3))");
    assert_eq!(got["power"], json!({"kind":"Constant","args":-3}));
    assert_eq!(got["toughness"], json!({"kind":"Constant","args":3}));
}
