use mtgish_ast::{
    ir::SCHEMA_VERSION,
    mtg_types::OracleCard,
    oracle_export::{compare, Request},
    unified,
};
use serde_json::{json, Value};

fn source() -> OracleCard {
    ron::from_str(r#"Card(Name:"Synthetic body",Typeline:(Supertypes:[],Cardtypes:[Creature],Subtypes:[]),CardPT:(Power:1,Toughness:1))"#).unwrap()
}
fn expected() -> Value {
    json!({"schema_version":SCHEMA_VERSION,"name":"Synthetic body","characteristics":{
        "supertypes":[],"card_types":["Creature"],"subtypes":[],"mana_cost":null,
        "color_indicator":[],"power_toughness":[1,1],"loyalty":null,"defense":null},"abilities":[]})
}
fn request(status: &str, value: Option<Value>) -> Request {
    Request {
        identity: json!({"provider_id":"synthetic","record_index":0,"face_index":0}),
        record_names: vec!["Synthetic body".into()],
        name: "Synthetic body".into(),
        grammar_status: status.into(),
        expected: value,
    }
}
#[test]
fn compares_fresh_source_lowering_with_exported_native_face() {
    let report = compare(&[source()], &[request("unique", Some(expected()))]).unwrap();
    assert!(report.passed);
    assert!(report.complete_agreement);
    assert_eq!(report.counts["equal"], 1);
}
#[test]
fn reports_a_changed_operand_and_its_path() {
    let mut value = expected();
    value["characteristics"]["power_toughness"][0] = json!(2);
    let report = compare(&[source()], &[request("unique", Some(value))]).unwrap();
    assert!(!report.passed);
    assert_eq!(report.counts["mismatch"], 1);
    assert_eq!(
        report.findings[0]["difference"]["path"],
        "/characteristics/power_toughness/0"
    );
}
#[test]
fn never_joins_on_face_name_alone_or_selects_the_first_source_occurrence() {
    let mut other = request("unique", Some(expected()));
    other.record_names.push("Other face".into());
    let report = compare(&[source()], &[other]).unwrap();
    assert_eq!(report.counts["mtgish_missing"], 1);
    let report = compare(
        &[source(), source()],
        &[request("unique", Some(expected()))],
    )
    .unwrap();
    assert!(!report.passed);
    assert_eq!(report.counts["identity_collision"], 1);
}
#[test]
fn unsupported_on_both_sides_never_counts_as_agreement() {
    let report = compare(&[], &[request("refused", None)]).unwrap();
    assert!(!report.passed);
    assert!(!report.complete_agreement);
    assert_eq!(report.counts.get("equal"), None);
}
#[test]
fn preserves_ambiguity_and_resource_limits_without_selecting_a_reading() {
    for (status, expected_status) in [
        ("ambiguous", "grammar_ambiguous"),
        ("limit", "grammar_limit"),
    ] {
        let report = compare(&[source()], &[request(status, None)]).unwrap();
        assert!(!report.passed);
        assert_eq!(report.counts[expected_status], 1);
    }
}
#[test]
fn rejects_wrong_schema_and_unknown_fields_in_exported_values() {
    for value in [
        {
            let mut v = expected();
            v["schema_version"] = json!(SCHEMA_VERSION + 1);
            v
        },
        {
            let mut v = expected();
            v["unrecognized_field"] = json!(true);
            v
        },
    ] {
        let report = compare(&[source()], &[request("unique", Some(value))]).unwrap();
        assert!(!report.passed);
        assert_eq!(report.counts["invalid_export"], 1);
    }
}
#[test]
fn equivalent_alternate_encodings_remain_visible_and_require_review() {
    let source:OracleCard=ron::from_str(r#"Card(Name:"Synthetic body",Typeline:(Supertypes:[],Cardtypes:[Sorcery],Subtypes:[]),Rules:[SpellActions(ActionList([DrawNumberCards(Integer(1))]))])"#).unwrap();
    let mut expected = serde_json::to_value(&unified::lower(&source).card.faces[0]).unwrap();
    fn rename(value: &mut Value) -> usize {
        match value {
            Value::Object(fields) => fields
                .iter_mut()
                .map(|(name, v)| {
                    if name == "result" && v.is_number() {
                        *v = json!(42);
                        1
                    } else {
                        rename(v)
                    }
                })
                .sum(),
            Value::Array(items) => items.iter_mut().map(rename).sum(),
            _ => 0,
        }
    }
    assert_eq!(rename(&mut expected), 1);
    let report = compare(&[source], &[request("unique", Some(expected))]).unwrap();
    assert!(!report.passed);
    assert_eq!(report.counts["canonical_only"], 1);
    assert_eq!(report.findings[0]["canonical_equal"], true);
}

#[test]
fn presentation_is_not_a_meaning_mismatch_or_an_exact_agreement() {
    let source: OracleCard = ron::from_str(r#"Card(Name:"Synthetic body",Typeline:(Supertypes:[],Cardtypes:[Creature],Subtypes:[]),CardPT:(Power:1,Toughness:1),Rules:[Flying])"#).unwrap();
    let mut expected = serde_json::to_value(&unified::lower(&source).card.faces[0]).unwrap();
    expected["abilities"][0] = json!({"kind":"WithPresentation","args":{
        "label":"Example", "reminder":false,
        "ability":{"kind":"Keyword","args":{"kind":"WithReminder","args":{
            "keyword":{"kind":"Flying"},"reminder":true}}}}});
    let report = compare(&[source], &[request("unique", Some(expected))]).unwrap();
    assert_eq!(report.counts["presentation_only"], 1);
    assert_eq!(report.findings[0]["semantic_equal"], true);
    assert_eq!(report.findings[0]["exact_equal"], false);
    assert!(
        !report.passed,
        "presentation loss cannot pass the exact gate"
    );
}
