use crate::{canonical, ir::Ability, validate};
use serde_json::{json, Value};

fn events() -> Vec<Value> {
    vec![
        json!({"Object":{"event":"Attacks","objects":{"kind":"CardType","args":"Creature"}}}),
        json!({"ZoneChange":{"objects":{"kind":"CardType","args":"Artifact"},"from":null,"to":"Battlefield"}}),
    ]
}

fn trigger(events: Vec<Value>) -> Value {
    json!({"kind":"Triggered","args":{"event":{"AnyOf":events},"intervening_if":null,
        "limits":{"per_turn":null,"total":null},"announcement":{"targets":[],"modes":null},
        "body":{"instructions":[{"kind":"Effect","args":{"result":1,"operation":{
            "kind":"Draw","args":{"players":{"kind":"One","args":{"kind":"AbilityController"}},"amount":{"kind":"Constant","args":1}}}}}]}}})
}

fn contexts(events: Vec<Value>) -> Vec<Value> {
    let ability = trigger(events.clone());
    vec![
        json!([ability]),
        json!([{"kind":"Static","args":{"kind":"Ordinary","affected":{"Objects":{"kind":"Zone","args":{"scope":{"Shared":"Battlefield"},"kind":"Permanent","filter":{"kind":"Any"}}}},"changes":[{"kind":"GrantAbility","args":ability}]}}]),
        json!([{"kind":"Replacement","args":{"active":"SourceEntering","event":{"Enter":{"objects":{"kind":"SameObject","args":{"kind":"Source"}}}},"body":{"instructions":[{"kind":"Effect","args":{"result":0,"operation":{"kind":"ModifyEvent","args":{"change":{"Entry":{"CopyOf":{"objects":{"kind":"Zone","args":{"scope":{"Shared":"Battlefield"},"kind":"Permanent","filter":{"kind":"CardType","args":"Creature"}}},"exceptions":[{"kind":"AddAbilities","args":[ability]}],"until":null}}}}}}}]}}}]),
        json!([{"kind":"Spell","args":{"announcement":{"targets":[],"modes":null},"body":{"instructions":[{"kind":"Effect","args":{"result":0,"operation":{"kind":"CreateTrigger","args":{
            "event":{"AnyOf":events},"duration":"EndOfTurn","announcement":{"targets":[],"modes":null},"once":false,"intervening_if":null,
            "body":ability["args"]["body"]}}}}]}}}]),
    ]
}

fn valid(value: &Value) {
    let abilities: Vec<Ability> = serde_json::from_value(value.clone()).unwrap();
    validate::abilities(&abilities).unwrap();
}

#[test]
fn event_order_is_canonical_inside_every_ability_container() {
    let original = events();
    let reversed: Vec<_> = original.iter().rev().cloned().collect();
    for (mut a, mut b) in contexts(original).into_iter().zip(contexts(reversed)) {
        valid(&a);
        valid(&b);
        canonical::abilities(&mut a).unwrap();
        canonical::abilities(&mut b).unwrap();
        valid(&a);
        valid(&b);
        assert_eq!(a, b);
        let once = a.clone();
        canonical::abilities(&mut a).unwrap();
        assert_eq!(a, once);
    }
}

#[test]
fn reduced_event_order_preserves_the_given_spelling_order() {
    let reversed = events().into_iter().rev().collect();
    for mut value in contexts(events()).into_iter().chain(contexts(reversed)) {
        let original = value.clone();
        canonical::reduced(&mut value).unwrap();
        assert_eq!(value, original);
    }
}

#[test]
fn local_bindings_remain_canonical_when_event_alternatives_move() {
    let original: Vec<_> = events()
        .into_iter()
        .enumerate()
        .map(|(index, event)| {
            let binding = index + 7;
            json!({"Filtered":{"event":event,"condition":{"kind":"ExistsPlayer","args":{
                "players":{"kind":"All"},"binding":binding,
                "condition":{"kind":"PlayerIsActive","args":{"kind":"Bound","args":binding}}
            }}}})
        })
        .collect();
    let reversed: Vec<_> = original.iter().rev().cloned().collect();
    for (mut a, mut b) in contexts(original).into_iter().zip(contexts(reversed)) {
        valid(&a);
        valid(&b);
        canonical::abilities(&mut a).unwrap();
        canonical::abilities(&mut b).unwrap();
        valid(&a);
        valid(&b);
        assert_eq!(a, b);
        let once = a.clone();
        canonical::abilities(&mut a).unwrap();
        assert_eq!(a, once);
    }
}
