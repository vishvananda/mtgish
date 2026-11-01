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
