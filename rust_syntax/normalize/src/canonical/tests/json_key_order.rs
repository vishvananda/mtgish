use crate::{canonical, ir::Ability, validate};
use serde_json::{json, Value};

fn reverse_fields(value: &Value) -> Value {
    match value {
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .rev()
                .map(|(k, v)| (k.clone(), reverse_fields(v)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(reverse_fields).collect()),
        scalar => scalar.clone(),
    }
}
fn normalize(mut value: Value) -> Value {
    let typed: Vec<Ability> = serde_json::from_value(value.clone()).unwrap();
    validate::abilities(&typed).unwrap();
    canonical::abilities(&mut value).unwrap();
    let once = value.clone();
    canonical::abilities(&mut value).unwrap();
    assert_eq!(value, once);
    value
}
fn trigger(event: Value) -> Value {
    json!({"kind":"Triggered","args":{"event":event,"intervening_if":null,
        "limits":{"per_turn":null,"total":null},"announcement":{"targets":[],"modes":null},
        "body":{"instructions":[{"kind":"Effect","args":{"result":5,"operation":{"kind":"Draw","args":{
            "players":{"kind":"One","args":{"kind":"AbilityController"}},"amount":{"kind":"Constant","args":1}}}}}]}}})
}
#[test]
fn commutative_keys_ignore_recursive_object_insertion_order() {
    let a = json!({"Object":{"event":"Attacks","objects":{"kind":"CardType","args":"Creature"}}});
    let b = json!({"ZoneChange":{"objects":{"kind":"CardType","args":"Artifact"},"from":null,"to":"Battlefield"}});
    let filter = json!({"kind":"All","args":[{"kind":"State","args":"Enchanted"},
        {"kind":"CardType","args":"Creature"},{"kind":"ControlledBy","args":{"kind":"One","args":{"kind":"AbilityController"}}}]});
    let query = |filter| {
        json!({"kind":"Objects","args":{"kind":"Zone","args":{
        "scope":{"Shared":"Battlefield"},"kind":"Permanent","filter":filter}}})
    };
    let static_ability = json!({"kind":"Static","args":{"kind":"Ordinary","affected":{"Objects":{"kind":"Zone","args":{
        "scope":{"Shared":"Battlefield"},"kind":"Permanent","filter":filter}}},"changes":[{"kind":"Pump","args":{"power":{"kind":"Constant","args":1},"toughness":{"kind":"Constant","args":1}}}]}});
    let union_damage = json!({"kind":"Spell","args":{"announcement":{"targets":[],"modes":null},"body":{"instructions":[
        {"kind":"Effect","args":{"result":0,"operation":{"kind":"DealDamage","args":{"source":{"kind":"Source"},
        "recipients":{"kind":"Union","args":[query(json!({"kind":"CardType","args":"Creature"})),query(json!({"kind":"State","args":"Enchanted"}))]},
        "amount":{"kind":"Constant","args":1},"preventable":true}}}}
    ]}}});
    for value in [
        json!([static_ability]),
        json!([trigger(json!({"Either":[a.clone(),b.clone()]}))]),
        json!([trigger(a), trigger(b)]),
        json!([union_damage]),
    ] {
        let reversed = reverse_fields(&value);
        assert_ne!(
            value.to_string(),
            reversed.to_string(),
            "preserve_order must be enabled"
        );
        assert_eq!(normalize(value), normalize(reversed));
    }
}
#[test]
fn comparison_key_retains_array_and_scalar_values() {
    let a: Value =
        serde_json::from_str(r#"{"z":[1,true,null,"x"],"a":{"kind":"Constant","args":2}}"#)
            .unwrap();
    assert_eq!(
        super::super::stable_json_key(&a),
        super::super::stable_json_key(&reverse_fields(&a))
    );
    let mut b = a.clone();
    b["z"].as_array_mut().unwrap().reverse();
    assert_ne!(
        super::super::stable_json_key(&a),
        super::super::stable_json_key(&b)
    );
}
#[test]
fn ordered_instructions_remain_distinct() {
    let mut a = json!([{"kind":"Spell","args":{"announcement":{"targets":[],"modes":null},"body":{"instructions":[
        {"kind":"Effect","args":{"result":1,"operation":{"kind":"GainLife","args":{"players":{"kind":"One","args":{"kind":"AbilityController"}},"amount":{"kind":"Constant","args":1}}}}},
        {"kind":"Effect","args":{"result":2,"operation":{"kind":"LoseLife","args":{"players":{"kind":"One","args":{"kind":"AbilityController"}},"amount":{"kind":"Constant","args":2}}}}}
    ]}}}]);
    let first = normalize(a.clone());
    a[0]["args"]["body"]["instructions"]
        .as_array_mut()
        .unwrap()
        .reverse();
    assert_ne!(first, normalize(a));
}
