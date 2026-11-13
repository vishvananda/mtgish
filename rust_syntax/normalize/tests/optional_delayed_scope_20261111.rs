//! Guard placement is observable: a declined choice must not register a cleanup
//! trigger. This locks the accepted representation, not a full engine scenario.
#[path = "support/fresh_producer.rs"]
mod fresh_producer;
use mtgish_ast::{canonical, corpus, mtg_types::OracleCard, unified, validate};
use serde_json::{Map, Value};

fn registrations(value: &Value, guarded: bool, output: &mut Vec<bool>) {
    if value["kind"] == "If" {
        let accepts = value["args"]["condition"]["kind"] == "PaymentChosen";
        registrations(&value["args"]["then"], guarded || accepts, output);
        registrations(&value["args"]["otherwise"], guarded, output);
        return;
    }
    if value["kind"] == "Optional" {
        registrations(&value["args"]["body"], true, output);
        return;
    }
    if value["kind"] == "CreateDelayedTrigger" {
        output.push(guarded);
        return;
    }
    match value {
        Value::Object(map) => {
            for child in map.values() {
                registrations(child, guarded, output);
            }
        }
        Value::Array(values) => {
            for child in values {
                registrations(child, guarded, output);
            }
        }
        _ => {}
    }
}

#[test]
fn optional_dependent_delays_are_guarded_and_unconditional_controls_remain_independent() {
    corpus::on_large_stack(|| {
        let expected: Map<String, Value> = serde_json::from_str(include_str!(
            "fixtures/optional_delayed_scope_20261111_expected.json"
        ))
        .unwrap();
        let mut seen = 0;
        for line in include_str!("fixtures/optional_delayed_scope_20261111.ronl").lines() {
            let source: OracleCard = ron::from_str(line).unwrap();
            let lowered = unified::lower(&source);
            assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
            let card = &lowered.card.faces[0];
            validate::card(card).unwrap();
            let mut actual = serde_json::to_value(&card.abilities).unwrap();
            let mut wanted = expected[&card.name].clone();
            canonical::abilities(&mut actual).unwrap();
            canonical::abilities(&mut wanted).unwrap();
            fresh_producer::assert_named("optional_delayed_scope_20261111_expected.json", &card.name, &wanted, &actual);
            let mut guards = Vec::new();
            registrations(&actual, false, &mut guards);
            let optional = !matches!(
                card.name.as_str(),
                "Feldon of the Third Path" | "Shallow Grave"
            );
            assert_eq!(guards, vec![optional], "{}", card.name);
            seen += 1;
        }
        assert_eq!(seen, 7);
    });
}
