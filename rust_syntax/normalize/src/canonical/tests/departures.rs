//! Event-subject and destination identities agree only in the post-event view.
use crate::{canonical, ir::Ability, validate};
use serde_json::{json, Value};

fn query(zone: &str) -> Value {
    json!({"kind":"Query","args":{"objects":{"kind":"InZone","args":{
        "objects":{"kind":"One","args":{"kind":"EventDestination"}},"zone":zone}},"index":0}})
}

fn event(from: Option<&str>, to: &str, own: bool) -> Value {
    let objects = if own {
        json!({"kind":"SameObject","args":{"kind":"Source"}})
    } else {
        json!({"kind":"Any"})
    };
    json!({"ZoneChange":{"objects":objects,"from":from,"to":to}})
}

fn effect(result: u32, object: Value) -> Value {
    json!({"kind":"Effect","args":{"result":result,"operation":{
        "kind":"Reveal","args":{"objects":{"kind":"One","args":object}}}}})
}

fn ability(event: Value, object: Value) -> Value {
    json!([{"kind":"Triggered","args":{"event":event,
        "limits":{"per_turn":null,"total":null},"announcement":{"targets":[],"modes":null},
        "intervening_if":null,"body":{"instructions":[effect(4,object)]}}}])
}

fn normalized(mut value: Value) -> Value {
    let typed: Vec<Ability> = serde_json::from_value(value.clone()).unwrap();
    validate::abilities(&typed).unwrap();
    canonical::abilities(&mut value).unwrap();
    let typed: Vec<Ability> = serde_json::from_value(value.clone()).unwrap();
    validate::abilities(&typed).unwrap();
    let once = value.clone();
    canonical::abilities(&mut value).unwrap();
    assert_eq!(once, value);
    value
}

fn object(value: &Value) -> &Value {
    &value[0]["args"]["body"]["instructions"][0]["args"]["operation"]["args"]["objects"]["args"]
}

#[test]
fn battlefield_departures_never_alias_the_new_card_to_the_departed_permanent() {
    for own in [true, false] {
        for to in ["Graveyard", "Exile", "Hand", "Library"] {
            let header = event(Some("Battlefield"), to, own);
            let new = normalized(ability(header.clone(), query(to)));
            let old = normalized(ability(header, json!({"kind":"EventSubject"})));
            assert_eq!(object(&new), &query(to));
            assert_ne!(new, old, "{to}, own={own}");
        }
    }
}

#[test]
fn explicit_nonbattlefield_departures_keep_old_and_new_objects_distinct() {
    let value = normalized(ability(
        event(Some("Library"), "Graveyard", false),
        query("Graveyard"),
    ));
    assert_eq!(object(&value), &query("Graveyard"));
}

#[test]
fn even_post_event_identity_keeps_current_zone_availability() {
    for (from, to, own) in [
        (Some("Hand"), "Battlefield", false),
        (None, "Graveyard", false),
        (Some("Library"), "Graveyard", true),
    ] {
        let header = event(from, to, own);
        let new = normalized(ability(header.clone(), query(to)));
        let old = normalized(ability(header, json!({"kind":"EventSubject"})));
        assert_eq!(object(&new), &query(to));
        assert_ne!(new, old);
    }
}

#[test]
fn wrapped_and_alternative_events_keep_destination_availability() {
    let old = event(Some("Battlefield"), "Graveyard", false);
    let new = event(None, "Graveyard", false);
    for header in [
        json!({"FirstEachTurn":old.clone()}),
        json!({"AnyOf":[old,new.clone()]}),
        json!({"FirstEachTurn":new}),
    ] {
        let value = normalized(ability(header, query("Graveyard")));
        assert_eq!(object(&value), &query("Graveyard"));
    }
}

#[test]
fn repeated_current_zone_checks_reduce_without_erasing_the_query() {
    for zone in ["Graveyard", "Exile", "Hand", "Library"] {
        let reference = query(zone);
        let mut value = ability(event(Some("Battlefield"), zone, false), reference.clone());
        let objects = &mut value[0]["args"]["body"]["instructions"][0]["args"]["operation"]["args"]
            ["objects"];
        *objects = json!({"kind":"InZone","args":{"objects":objects.clone(),"zone":zone}});
        let value = normalized(value);
        assert_eq!(object(&value), &reference);
        let mut different = ability(event(Some("Battlefield"), zone, false), reference);
        let objects = &mut different[0]["args"]["body"]["instructions"][0]["args"]["operation"]
            ["args"]["objects"];
        *objects = json!({"kind":"InZone","args":{"objects":objects.clone(),"zone":"Battlefield"}});
        let value = normalized(different);
        assert_eq!(
            value[0]["args"]["body"]["instructions"][0]["args"]["operation"]["args"]["objects"]
                ["kind"],
            "InZone"
        );
    }
}

#[test]
fn created_trigger_destination_does_not_alias_its_own_departed_subject() {
    let operation = json!({"kind":"CreateTrigger","args":{
        "event":event(Some("Battlefield"),"Graveyard",true),"duration":"EndOfTurn",
        "announcement":{"targets":[],"modes":null},"once":true,"intervening_if":null,
        "body":{"instructions":[effect(0,query("Graveyard"))]}}});
    let mut value = ability(event(None, "Battlefield", true), json!({"kind":"Source"}));
    value[0]["args"]["body"]["instructions"][0]["args"]["operation"] = operation;
    let value = normalized(value);
    assert_eq!(value.pointer("/0/args/body/instructions/0/args/operation/args/body/instructions/0/args/operation/args/objects/args"), Some(&query("Graveyard")));
}
