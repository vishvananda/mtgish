use crate::{canonical, ir::Ability, validate};
use serde_json::{json, Value};

fn players() -> Value {
    json!({"kind":"One","args":{"kind":"AbilityController"}})
}

fn objects() -> Value {
    json!({"kind":"All","args":[{"kind":"CardType","args":"Creature"},
        {"kind":"ControlledBy","args":players()}]})
}

fn zone() -> Value {
    json!({"ZoneChange":{"objects":objects(),"from":null,"to":"Battlefield"}})
}

fn player() -> Value {
    json!({"Player":{"event":"EntersUnderControl","players":players(),"objects":objects()}})
}

fn effect(operation: Value) -> Value {
    json!({"kind":"Effect","args":{"result":7,"operation":operation}})
}

fn draw(who: &str) -> Value {
    effect(
        json!({"kind":"Draw","args":{"players":{"kind":"One","args":{"kind":who}},
        "amount":{"kind":"Constant","args":1}}}),
    )
}

fn trigger(event: Value, instruction: Value) -> Value {
    json!({"kind":"Triggered","args":{"event":event,"intervening_if":null,
        "limits":{"per_turn":null,"total":null},"announcement":{"targets":[],"modes":null},
        "body":{"instructions":[instruction]}}})
}

fn valid(value: &Value) -> bool {
    let abilities: Vec<Ability> = serde_json::from_value(value.clone()).unwrap();
    validate::abilities(&abilities).is_ok()
}

fn canonical(value: Value) -> Value {
    assert!(valid(&value));
    let mut value = value;
    canonical::abilities(&mut value).unwrap();
    assert!(valid(&value));
    let once = value.clone();
    canonical::abilities(&mut value).unwrap();
    assert_eq!(value, once);
    value
}

#[test]
fn controller_restriction_has_one_entry_normal_form_without_duplicate_filters() {
    let a = canonical(json!([trigger(zone(), draw("AbilityController"))]));
    let b = canonical(json!([trigger(player(), draw("AbilityController"))]));
    assert_eq!(a, b);
    let event = &a[0]["args"]["event"]["Player"];
    assert_eq!(event["players"], json!({"kind":"All"}));
    assert_eq!(event["objects"]["args"].as_array().unwrap().len(), 2);
}

#[test]
fn arbitrary_controller_sets_keep_their_original_match_restriction() {
    for players in [
        json!({"kind":"OpponentsOf","args":{"kind":"AbilityController"}}),
        json!({"kind":"One","args":{"kind":"Active"}}),
    ] {
        let filter = json!({"kind":"All","args":[{"kind":"CardType","args":"Land"},
            {"kind":"ControlledBy","args":players}]});
        let zone = json!({"ZoneChange":{"objects":filter,"from":null,"to":"Battlefield"}});
        let player = json!({"Player":{"event":"EntersUnderControl","players":players,
            "objects":{"kind":"CardType","args":"Land"}}});
        assert_eq!(
            canonical(json!([trigger(zone, draw("AbilityController"))])),
            canonical(json!([trigger(player, draw("AbilityController"))]))
        );
    }
}

#[test]
fn recorded_entry_controller_is_not_replaced_by_a_later_controller_query() {
    let mut event = player();
    event["Player"]["players"] = json!({"kind":"All"});
    let recorded = canonical(json!([trigger(event.clone(), draw("EventSubject"))]));
    assert_eq!(
        recorded[0]["args"]["body"]["instructions"][0]["args"]["operation"]["args"]["players"]
            ["args"]["kind"],
        "EventSubject"
    );
    let mut queried = recorded.clone();
    queried[0]["args"]["body"]["instructions"][0]["args"]["operation"]["args"]["players"]["args"] =
        json!({"kind":"ControllerOf","args":{"kind":"EventSubject"}});
    assert_ne!(recorded, canonical(queried));
}

#[test]
fn entry_destination_reads_and_invalid_player_reads_keep_zone_change_roles() {
    let reveal = effect(json!({"kind":"Reveal","args":{"objects":{
        "kind":"One","args":{"kind":"EventDestination"}}}}));
    let value = canonical(json!([trigger(zone(), reveal.clone())]));
    assert!(value[0]["args"]["event"]["ZoneChange"].is_object());
    let mut invalid_player = json!([trigger(zone(), draw("EventSubject"))]);
    assert!(!valid(&invalid_player));
    canonical::abilities(&mut invalid_player).unwrap();
    assert!(!valid(&invalid_player));
    let mut invalid_destination = json!([trigger(player(), reveal)]);
    assert!(!valid(&invalid_destination));
    canonical::abilities(&mut invalid_destination).unwrap();
    assert!(!valid(&invalid_destination));
}

#[test]
fn a_controller_selector_cannot_acquire_a_new_object_candidate_binding() {
    let mut event = player();
    event["Player"]["players"] = json!({"kind":"One","args":{
        "kind":"ControllerOf","args":{"kind":"Candidate"}}});
    let mut value = json!([trigger(event, draw("AbilityController"))]);
    assert!(!valid(&value));
    canonical::abilities(&mut value).unwrap();
    assert!(!valid(&value));
}

#[test]
fn entry_wrappers_preserve_grouping_and_filter_conditions() {
    for wrap in ["Batch", "Filtered", "FirstEachTurn", "NthEachTurn"] {
        let wrapped = |event: Value| match wrap {
            "Batch" => json!({"Batch":{"event":event,"per":"Whole","count":null,"including":null}}),
            "Filtered" => json!({"Filtered":{"event":event,"condition":{"kind":"PlayerIsActive",
                "args":{"kind":"AbilityController"}}}}),
            "NthEachTurn" => {
                json!({"NthEachTurn":{"event":event,"comparison":"Equal","value":{"kind":"Constant","args":2}}})
            }
            _ => json!({"FirstEachTurn":event}),
        };
        assert_eq!(
            canonical(json!([trigger(wrapped(zone()), draw("AbilityController"))])),
            canonical(json!([trigger(
                wrapped(player()),
                draw("AbilityController")
            )]))
        );
    }
}

#[test]
fn a_future_player_trigger_does_not_prevent_normalizing_its_creators_frame() {
    let future = effect(json!({"kind":"CreateTrigger","args":{"event":player(),
        "duration":"EndOfTurn","announcement":{"targets":[],"modes":null},"once":true,
        "intervening_if":null,"body":{"instructions":[draw("EventSubject")]}}}));
    let value = canonical(json!([trigger(zone(), future)]));
    assert!(value[0]["args"]["event"]["Player"].is_object());
    let future = &value[0]["args"]["body"]["instructions"][0]["args"]["operation"]["args"];
    assert!(future["event"]["Player"].is_object());
    assert_eq!(
        future["body"]["instructions"][0]["args"]["operation"]["args"]["players"]["args"]["kind"],
        "EventSubject"
    );
}

#[test]
fn origins_uncontrolled_entries_and_ordered_reduction_remain_distinct() {
    let base = json!([trigger(zone(), draw("AbilityController"))]);
    let mut from_hand = base.clone();
    from_hand[0]["args"]["event"]["ZoneChange"]["from"] = json!("Hand");
    assert!(canonical(from_hand)[0]["args"]["event"]["ZoneChange"].is_object());
    let mut uncontrolled = base.clone();
    uncontrolled[0]["args"]["event"]["ZoneChange"]["objects"] =
        json!({"kind":"CardType","args":"Creature"});
    assert!(canonical(uncontrolled)[0]["args"]["event"]["ZoneChange"].is_object());
    for mut value in [base, json!([trigger(player(), draw("AbilityController"))])] {
        let original = value.clone();
        canonical::reduced(&mut value).unwrap();
        assert_eq!(value, original);
    }
}
