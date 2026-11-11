#[path = "support/reference_followup.rs"]
mod fixtures;

use serde_json::{json, Value};

fn card(name: &str) -> Value {
    serde_json::to_value(fixtures::card(name)).unwrap()
}

fn history(value: &Value) -> &Value {
    fn find(value: &Value) -> Option<&Value> {
        match value {
            Value::Object(fields) => {
                if fields.get("kind").and_then(Value::as_str) == Some("EventHistory") {
                    return fields.get("args");
                }
                fields.values().find_map(find)
            }
            Value::Array(values) => values.iter().find_map(find),
            _ => None,
        }
    }
    find(value).unwrap()
}

#[test]
fn heralds_reveille_keeps_permanent_exploration_controller_and_this_turn_without_current_types() {
    let c = card("Herald's Reveille");
    let h = history(&c);
    assert_eq!(h["event"]["Object"]["event"], json!("Explores"));
    assert_eq!(
        h["event"]["Object"]["objects"],
        json!({"kind":"AllOf","args":[
            {"kind":"Any"},{"kind":"ControlledBy","args":{"kind":"One","args":{"kind":"AbilityController"}}}
        ]})
    );
    assert_eq!(h["window"], json!("ThisTurn"));
    assert_eq!(h["measure"], json!("Occurrences"));
}

#[test]
fn balrog_counts_sacrificed_permanents_without_retesting_their_card_types() {
    let c = card("The Balrog, Durin's Bane");
    let h = history(&c);
    assert_eq!(
        h["event"],
        json!({"Player":{
            "event":"Sacrifices","players":{"kind":"All"},"objects":{"kind":"Any"}
        }})
    );
    assert_eq!(h["window"], json!("ThisTurn"));
    assert_eq!(h["measure"], json!("Occurrences"));
    assert_eq!(
        c["abilities"][0]["args"]["ReduceCost"][0]["color"],
        Value::Null
    );
}

#[test]
fn yuna_keeps_battlefield_origin_control_other_source_and_departing_counters() {
    let c = card("Yuna, Grand Summoner");
    let t = &c["abilities"][1]["args"];
    let e = &t["event"]["ZoneChange"];
    assert_eq!(e["from"], json!("Battlefield"));
    assert_eq!(e["to"], json!("Graveyard"));
    assert_eq!(
        e["objects"]["args"][0],
        json!({"kind":"AllOf","args":[
            {"kind":"Not","args":{"kind":"SameObject","args":{"kind":"Source"}}},
            {"kind":"Any"},
            {"kind":"ControlledBy","args":{"kind":"One","args":{"kind":"AbilityController"}}}
        ]})
    );
    assert_eq!(
        t["intervening_if"]["args"]["object"],
        json!({"kind":"EventSubject"})
    );
    assert_eq!(
        t["intervening_if"]["args"]["filter"]["kind"],
        json!("Counters")
    );
}

#[test]
fn it_that_betrays_keeps_opponent_nontoken_and_existing_source_exclusion() {
    let c = card("It That Betrays");
    let e = &c["abilities"][1]["args"]["event"]["Player"];
    assert_eq!(e["event"], json!("Sacrifices"));
    assert_eq!(
        e["players"],
        json!({"kind":"OpponentsOf","args":{"kind":"AbilityController"}})
    );
    assert_eq!(
        e["objects"],
        json!({"kind":"AllOf","args":[
            {"kind":"Not","args":{"kind":"SameObject","args":{"kind":"Source"}}},
            {"kind":"AllOf","args":[{"kind":"Not","args":{"kind":"Token"}},{"kind":"Any"}]}
        ]})
    );
}

#[test]
fn sawblade_counts_the_same_permanent_sacrifices_for_its_condition_and_damage() {
    let c = card("Sawblade Skinripper");
    let trigger = &c["abilities"][2]["args"];
    let condition_history = history(&trigger["intervening_if"]);
    let damage_history = history(&trigger["body"]);
    assert_eq!(condition_history, damage_history);
    assert_eq!(
        condition_history["event"],
        json!({"Player":{
            "event":"Sacrifices","players":{"kind":"One","args":{"kind":"AbilityController"}},
            "objects":{"kind":"Any"}
        }})
    );
    assert_eq!(condition_history["window"], json!("ThisTurn"));
    assert_eq!(condition_history["measure"], json!("Occurrences"));
    assert_eq!(
        trigger["intervening_if"]["args"]["test"]["args"]["comparison"],
        json!("GreaterOrEqual")
    );
    assert_eq!(
        trigger["intervening_if"]["args"]["test"]["args"]["value"],
        json!({"kind":"Constant","args":1})
    );
    assert_eq!(
        trigger["announcement"]["targets"][0]["kind"],
        json!("DamageRecipient")
    );
}

fn custom(event: &str) -> Value {
    let source = ron::from_str(&format!("Card(Name:\"Event domain control\",Typeline:(Supertypes:[],Cardtypes:[Instant],Subtypes:[]),Rules:[TriggerA({event},ActionList([GainLife(Integer(1))]))])")).unwrap();
    serde_json::to_value(mtgish_ast::lower::card(&source).unwrap()).unwrap()
}

#[test]
fn explicit_card_type_qualifications_remain_in_all_three_event_families() {
    let types = "Or([IsCardtype(Artifact),IsCardtype(Battle),IsCardtype(Creature),IsCardtype(Enchantment),IsCardtype(Land),IsCardtype(Planeswalker)])";
    for (event, outer, path) in [
        (format!("WhenAPermanentExplores({types})"), "Object", false),
        (
            format!("WhenAPlayerSacrificesAPermanent(AnyPlayer,{types})"),
            "Player",
            false,
        ),
        (
            format!("WhenAPermanentIsPutIntoAPlayersGraveyard({types},AnyPlayer)"),
            "ZoneChange",
            true,
        ),
    ] {
        let c = custom(&event);
        let mut filter = &c["abilities"][0]["args"]["event"][outer]["objects"];
        if path {
            filter = &filter["args"][0];
        }
        assert_eq!(filter["kind"], json!("AnyOf"));
        assert_eq!(filter["args"].as_array().unwrap().len(), 6);
    }
}

#[test]
fn from_anywhere_permanent_cards_keep_their_type_domain() {
    let c = custom("WhenACardIsPutIntoAPlayersGraveyardFromAnywhere(IsPermanent,AnyPlayer)");
    let event = &c["abilities"][0]["args"]["event"]["ZoneChange"];
    assert_eq!(event["from"], Value::Null);
    assert_eq!(event["objects"]["args"][0]["kind"], json!("AnyOf"));
    assert_eq!(
        event["objects"]["args"][0]["args"]
            .as_array()
            .unwrap()
            .len(),
        6
    );
    assert_eq!(
        event["objects"]["args"][2],
        json!({"kind":"Not","args":{"kind":"Token"}})
    );
}
