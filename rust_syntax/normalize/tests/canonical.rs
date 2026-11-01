use mtgish_ast::canonical;
use serde_json::json;

fn effect(result: u32, objects: serde_json::Value) -> serde_json::Value {
    json!({"kind":"Effect","args":{"result":result,"operation":{"kind":"Destroy","args":{"objects":objects,"regeneration":true}}}})
}

#[test]
fn renumbers_ids_in_definition_order_and_sorts_commutative_lists() {
    let target = |slot: u32| json!({"kind":"One","args":{"kind":"Target","args":{"slot":slot,"index":0}}});
    let spec = |slot: u32, filter: serde_json::Value| {
        json!({"slot":slot,"kind":"Player","count":{"Exactly":{"kind":"Constant","args":1}},"filter":filter,
               "players":null,"distribute":null,"constraints":[],"chooser":null})
    };
    let mut value = json!([{"kind":"Spell","args":{
        "announcement":{"targets":[spec(5, json!({"kind":"All","args":[
            {"kind":"Tapped"},{"kind":"CardType","args":"Creature"}]}))],"modes":null},
        "body":{"instructions":[effect(7, target(5))]}}}]);
    canonical::abilities(&mut value).unwrap();
    let args = &value[0]["args"];
    assert_eq!(args["announcement"]["targets"][0]["slot"], 0);
    assert_eq!(args["body"]["instructions"][0]["args"]["result"], 0);
    assert_eq!(args["body"]["instructions"][0]["args"]["operation"]["args"]["objects"], target(0));
    assert_eq!(args["announcement"]["targets"][0]["filter"]["args"][0]["kind"], "CardType");
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
    let filter = json!({"kind":"All","args":[{"kind":"Any"},{"kind":"Tapped"}]});
    let reduced = destroy(json!({"kind":"Battlefield","args":filter}));
    assert_eq!(reduced["args"], json!({"kind":"Tapped"}));
    let filter = json!({"kind":"All","args":[{"kind":"Any"},{"kind":"Any"}]});
    let reduced = destroy(json!({"kind":"Battlefield","args":filter}));
    assert_eq!(reduced["args"], json!({"kind":"Any"}));
}

#[test]
fn one_object_set_is_the_same_object() {
    let source = json!({"kind":"Source"});
    let filter = json!({"kind":"InSet","args":{"kind":"One","args":source}});
    let reduced = destroy(json!({"kind":"Battlefield","args":filter}));
    assert_eq!(reduced["args"], json!({"kind":"SameObject","args":source}));
}

#[test]
fn zone_scope_absorbs_its_filter() {
    let yours = json!({"kind":"One","args":{"kind":"AbilityController"}});
    let zone = |players: serde_json::Value| json!({"Players":{"zone":"Graveyard","players":players}});
    let in_zone = json!({"kind":"InZone","args":zone(yours.clone())});
    let filter = json!({"kind":"All","args":[{"kind":"Tapped"},in_zone.clone()]});
    let set = |filter: serde_json::Value| {
        json!({"kind":"Zone","args":{"scope":zone(json!({"kind":"All"})),"kind":"Card","filter":filter}})
    };
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
    assert_eq!(value[0]["args"]["announcement"]["targets"][0]["count"], json!({"UpTo":total}));
}
