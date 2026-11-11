use mtgish_ast::canonical;
use serde_json::json;

fn effect(result: u32, objects: serde_json::Value) -> serde_json::Value {
    json!({"kind":"Effect","args":{"result":result,"operation":{"kind":"Destroy","args":{"objects":objects,"regeneration":true}}}})
}

#[test]
fn renumbers_ids_in_definition_order_and_sorts_commutative_lists() {
    let target =
        |slot: u32| json!({"kind":"One","args":{"kind":"Target","args":{"slot":slot,"index":0}}});
    let spec = |slot: u32, filter: serde_json::Value| {
        json!({"slot":slot,"kind":"Player","count":{"Exactly":{"kind":"Constant","args":1}},"filter":filter,
               "players":null,"distribute":null,"constraints":[],"chooser":null})
    };
    let mut value = json!([{"kind":"Spell","args":{
        "announcement":{"targets":[spec(5, json!({"kind":"AllOf","args":[
            {"kind":"Tapped"},{"kind":"CardType","args":"Creature"}]}))],"modes":null},
        "body":{"instructions":[effect(7, target(5)), effect(9, json!({"kind":"Selected","args":7}))]}}}]);
    canonical::abilities(&mut value).unwrap();
    let args = &value[0]["args"];
    assert_eq!(args["announcement"]["targets"][0]["slot"], 0);
    assert_eq!(args["body"]["instructions"][0]["args"]["result"], 0);
    assert_eq!(
        args["body"]["instructions"][0]["args"]["operation"]["args"]["objects"],
        target(0)
    );
    assert_eq!(
        args["announcement"]["targets"][0]["filter"]["args"][0]["kind"],
        "CardType"
    );
}

fn destroy(objects: serde_json::Value) -> serde_json::Value {
    let mut value = json!([{"kind":"Spell","args":{
        "announcement":{"targets":[],"modes":null},
        "body":{"instructions":[effect(0, objects)]}}}]);
    canonical::abilities(&mut value).unwrap();
    value[0]["args"]["body"]["instructions"][0]["args"]["operation"]["args"]["objects"].clone()
}

#[test]
fn all_drops_any_and_keeps_one_when_nothing_else_remains() {
    let filter = json!({"kind":"AllOf","args":[{"kind":"Any"},{"kind":"Tapped"}]});
    let reduced = destroy(
        json!({"kind":"Zone","args":{"scope":{"Shared":"Battlefield"},"kind":"Permanent","filter":filter}}),
    );
    assert_eq!(reduced["args"]["filter"], json!({"kind":"Tapped"}));
    let filter = json!({"kind":"AllOf","args":[{"kind":"Any"},{"kind":"Any"}]});
    let reduced = destroy(
        json!({"kind":"Zone","args":{"scope":{"Shared":"Battlefield"},"kind":"Permanent","filter":filter}}),
    );
    assert_eq!(reduced["args"]["filter"], json!({"kind":"Any"}));
}

#[test]
fn one_object_set_is_the_same_object() {
    let source = json!({"kind":"Source"});
    let filter = json!({"kind":"InSet","args":{"kind":"One","args":source}});
    let reduced = destroy(
        json!({"kind":"Zone","args":{"scope":{"Shared":"Battlefield"},"kind":"Permanent","filter":filter}}),
    );
    assert_eq!(
        reduced["args"]["filter"],
        json!({"kind":"SameObject","args":source})
    );
}

#[test]
fn zone_scope_absorbs_its_filter() {
    let yours = json!({"kind":"One","args":{"kind":"AbilityController"}});
    let zone =
        |players: serde_json::Value| json!({"Players":{"zone":"Graveyard","players":players}});
    let in_zone = json!({"kind":"InZone","args":zone(yours.clone())});
    let filter = json!({"kind":"AllOf","args":[{"kind":"Tapped"},in_zone.clone()]});
    let set = |filter: serde_json::Value| json!({"kind":"Zone","args":{"scope":zone(json!({"kind":"All"})),"kind":"Card","filter":filter}});
    let reduced = destroy(set(filter));
    assert_eq!(reduced["args"]["scope"], zone(yours.clone()));
    assert_eq!(reduced["args"]["filter"], json!({"kind":"Tapped"}));
    let reduced = destroy(set(in_zone));
    assert_eq!(reduced["args"]["scope"], zone(yours));
    assert_eq!(reduced["args"]["filter"], json!({"kind":"Any"}));
}

#[test]
fn divided_open_count_is_bounded_by_the_total() {
    let total = json!({"kind":"Constant","args":5});
    let mut value = json!([{"kind":"Spell","args":{
        "announcement":{"targets":[{"slot":0,"kind":"DamageRecipient","count":"AnyNumber",
            "filter":{"kind":"Any"},"players":null,"distribute":total,"constraints":[],"chooser":null}],
            "modes":null},
        "body":{"instructions":[]}}}]);
    canonical::abilities(&mut value).unwrap();
    assert_eq!(
        value[0]["args"]["announcement"]["targets"][0]["count"],
        json!({"UpTo":total})
    );
}

#[test]
fn damage_role_of_the_subject_is_the_event_subject() {
    let role = |subject: &str, named: &str| {
        let mut value = json!([{"kind":"Triggered","args":{
            "event":{"Damage":{"source_kind":"Any","sources":{"kind":"Any"},
                "recipients":{"Players":{"kind":"All"}},"kind":"Any",
                "grouping":"SourceRecipient","subject":subject}},
            "announcement":{"targets":[],"modes":null},
            "body":{"instructions":[effect(0, json!({"kind":"One","args":{"kind":"Damage","args":named}}))]}}}]);
        canonical::abilities(&mut value).unwrap();
        value[0]["args"]["body"]["instructions"][0]["args"]["operation"]["args"]["objects"]["args"]
            .clone()
    };
    assert_eq!(role("Source", "Source"), json!({"kind":"EventSubject"}));
    assert_eq!(
        role("Source", "Recipient"),
        json!({"kind":"Damage","args":"Recipient"})
    );
}

#[test]
fn reduced_form_keeps_ids_and_order() {
    let filter = json!({"kind":"AllOf","args":[{"kind":"Tapped"},{"kind":"Any"},{"kind":"CardType","args":"Creature"}]});
    let mut value = json!([{"kind":"Spell","args":{
        "announcement":{"targets":[],"modes":null},
        "body":{"instructions":[effect(7, json!({"kind":"Zone","args":{"scope":{"Shared":"Battlefield"},"kind":"Permanent","filter":filter}}))]}}}]);
    canonical::reduced(&mut value).unwrap();
    let instruction = &value[0]["args"]["body"]["instructions"][0]["args"];
    assert_eq!(instruction["result"], 7);
    let items = &instruction["operation"]["args"]["objects"]["args"]["filter"]["args"];
    assert_eq!(
        items,
        &json!([{"kind":"Tapped"},{"kind":"CardType","args":"Creature"}])
    );
}

#[test]
fn permanent_types_narrow_nothing_on_the_battlefield() {
    let six: Vec<serde_json::Value> = [
        "Land",
        "Artifact",
        "Battle",
        "Creature",
        "Enchantment",
        "Planeswalker",
    ]
    .iter()
    .map(|name| json!({"kind":"CardType","args":name}))
    .collect();
    let either = json!({"kind":"AnyOf","args":six});
    let reduced = destroy(
        json!({"kind":"Zone","args":{"scope":{"Shared":"Battlefield"},"kind":"Permanent","filter":either}}),
    );
    assert_eq!(reduced["args"]["filter"], json!({"kind":"Any"}));
    let filter = json!({"kind":"AllOf","args":[{"kind":"Tapped"},either]});
    let reduced = destroy(
        json!({"kind":"Zone","args":{"scope":{"Shared":"Battlefield"},"kind":"Permanent","filter":filter}}),
    );
    assert_eq!(reduced["args"]["filter"], json!({"kind":"Tapped"}));
}

#[test]
fn what_enters_the_battlefield_is_a_permanent() {
    let six: Vec<serde_json::Value> = [
        "Artifact",
        "Battle",
        "Creature",
        "Enchantment",
        "Land",
        "Planeswalker",
    ]
    .iter()
    .map(|name| json!({"kind":"CardType","args":name}))
    .collect();
    let mut value = json!([{"kind":"Triggered","args":{
        "event":{"ZoneChange":{"objects":{"kind":"AllOf","args":[{"kind":"Tapped"},{"kind":"AnyOf","args":six}]},
            "from":null,"to":"Battlefield"}},
        "announcement":{"targets":[],"modes":null},"body":{"instructions":[]}}}]);
    canonical::abilities(&mut value).unwrap();
    assert_eq!(
        value[0]["args"]["event"]["ZoneChange"]["objects"],
        json!({"kind":"Tapped"})
    );
}

fn triggered(event: serde_json::Value, objects: serde_json::Value) -> serde_json::Value {
    json!([{"kind":"Triggered","args":{"event":event,"intervening_if":null,"limits":{"per_turn":null,"total":null},
        "announcement":{"targets":[],"modes":null},
        "body":{"instructions":[effect(0, json!({"kind":"One","args":objects}))]}}}])
}

#[test]
fn a_trigger_on_the_source_calls_its_subject_the_source() {
    let source = json!({"kind":"SameObject","args":{"kind":"Source"}});
    let event = json!({"Object":{"event":"Attacks","objects":source}});
    let mut subject = triggered(event.clone(), json!({"kind":"EventSubject"}));
    let mut named = triggered(event, json!({"kind":"Source"}));
    canonical::abilities(&mut subject).unwrap();
    canonical::abilities(&mut named).unwrap();
    assert_eq!(subject, named);
}

#[test]
fn another_creature_keeps_its_event_subject() {
    let event =
        json!({"Object":{"event":"Attacks","objects":{"kind":"CardType","args":"Creature"}}});
    let mut subject = triggered(event.clone(), json!({"kind":"EventSubject"}));
    let mut named = triggered(event, json!({"kind":"Source"}));
    canonical::abilities(&mut subject).unwrap();
    canonical::abilities(&mut named).unwrap();
    assert_ne!(subject, named);
}

#[test]
fn groups_and_adjacent_statics_flatten() {
    let flat = json!([{"kind":"Spell","args":{"announcement":{"targets":[],"modes":null},
        "body":{"instructions":[effect(0, json!({"kind":"One","args":{"kind":"Source"}}))]}}}]);
    let mut grouped = flat.clone();
    let body = grouped[0]["args"]["body"].clone();
    grouped[0]["args"]["body"]["instructions"] = json!([{"kind":"Group","args":body}]);
    canonical::abilities(&mut grouped).unwrap();
    let mut flat = flat;
    canonical::abilities(&mut flat).unwrap();
    assert_eq!(grouped, flat);
}

#[test]
fn a_shared_body_merges_triggers_into_either() {
    let trigger = |event: serde_json::Value| {
        json!({"kind":"Triggered","args":{"event":event,"intervening_if":null,"limits":{"once_each_turn":false,"max_each_turn":null,"not_on_stack":false},
               "announcement":{"targets":[],"modes":null},"body":{"instructions":[]}}})
    };
    let event = |name: &str| json!({"Object":{"event":name,"objects":{"kind":"SameObject","args":{"kind":"Source"}}}});
    let mut split = json!([trigger(event("Attacks")), trigger(event("Blocks"))]);
    let mut joined = json!([trigger(
        json!({"AnyOf":[event("Blocks"), event("Attacks")]})
    )]);
    canonical::abilities(&mut split).unwrap();
    canonical::abilities(&mut joined).unwrap();
    assert_eq!(split, joined);
}

#[test]
fn numbers_nobody_reads_do_not_count() {
    let spell = |first: u32, second: u32| {
        json!([{"kind":"Spell","args":{"announcement":{"targets":[],"modes":null},"body":{"instructions":[
            effect(first, json!({"kind":"One","args":{"kind":"Source"}})),
            effect(second, json!({"kind":"Selected","args":first})),
        ]}}}])
    };
    let (mut a, mut b) = (spell(0, 1), spell(7, 2));
    canonical::abilities(&mut a).unwrap();
    canonical::abilities(&mut b).unwrap();
    assert_eq!(a, b);
    assert_eq!(
        a[0]["args"]["body"]["instructions"][1]["args"]["result"],
        u32::MAX
    );
}
