use crate::{canonical, ir::Ability, validate};
use serde_json::{json, Value};
fn norm(mut v: Value) -> Value {
    let a: Vec<Ability> = serde_json::from_value(v.clone()).unwrap();
    validate::abilities(&a).unwrap();
    canonical::abilities(&mut v).unwrap();
    let a: Vec<Ability> = serde_json::from_value(v.clone()).unwrap();
    validate::abilities(&a).unwrap();
    let mut twice = v.clone();
    canonical::abilities(&mut twice).unwrap();
    assert_eq!(v, twice);
    v
}
fn pair() -> Value {
    serde_json::from_str(include_str!("result_scope_pair.json")).unwrap()
}
#[test]
fn delayed_local_read_does_not_make_its_creators_result_live() {
    let p = pair();
    let a = norm(p["collision_input"].clone());
    let b = norm(p["fresh_unused_outer_definition"].clone());
    assert_eq!(a, b);
    assert_eq!(
        a[0]["args"]["body"]["instructions"][0]["args"]["result"],
        u32::MAX - 1
    );
}
fn effect(id: u32, op: Value) -> Value {
    json!({"kind":"Effect","args":{"result":id,"operation":op}})
}
fn capture(outer: u32, creator: u32) -> Value {
    let mut p = pair()["collision_input"].clone();
    let steps = p[0]["args"]["body"]["instructions"].as_array_mut().unwrap();
    steps[0]["args"]["result"] = json!(creator);
    steps[0]["args"]["operation"]["args"]["body"]["instructions"] = json!([effect(
        80,
        json!({"kind":"GainLife","args":{"players":{"kind":"One","args":{"kind":"AbilityController"}},"amount":{"kind":"BoundValue","args":outer}}})
    )]);
    steps.insert(
        0,
        effect(
            outer,
            json!({"kind":"BindValue","args":{"value":{"kind":"Constant","args":3}}}),
        ),
    );
    p
}
#[test]
fn delayed_body_retains_the_preceding_outer_capture() {
    let a = norm(capture(7, 8));
    assert_eq!(a, norm(capture(17, 18)));
    assert_eq!(a[0]["args"]["body"]["instructions"][0]["args"]["result"], 0);
}
#[test]
fn sibling_branches_resolve_their_own_definitions() {
    let mut p = pair()["collision_input"].clone();
    let body = p[0]["args"]["body"]["instructions"][0]["args"]["operation"]["args"]["body"].clone();
    let unused = effect(
        1,
        json!({"kind":"BindValue","args":{"value":{"kind":"Constant","args":4}}}),
    );
    p[0]["args"]["body"]["instructions"] = json!([{"kind":"If","args":{"condition":{"kind":"Compare","args":{"left":{"kind":"Constant","args":1},"comparison":"Equal","right":{"kind":"Constant","args":1}}},"then":{"instructions":[unused]},"otherwise":body}}]);
    let first = norm(p.clone());
    p[0]["args"]["body"]["instructions"][0]["args"]["then"]["instructions"][0]["args"]["result"] =
        json!(99);
    assert_eq!(first, norm(p));
}
#[test]
fn optional_numeric_locals_do_not_shadow_a_later_payment_receipt() {
    let mut p = pair()["collision_input"].clone();
    p[0]["args"]["body"]["instructions"] = json!([
        {"kind":"Optional","args":{"decider":{"kind":"AbilityController"},"body":{"instructions":[effect(9,json!({"kind":"BindValue","args":{"value":{"kind":"Constant","args":2}}}))]}}},
        effect(9,json!({"kind":"PayCost","args":{"player":{"kind":"AbilityController"},"cost":{"parts":[{"Life":{"kind":"Constant","args":1}}]},"optional":true}})),
        {"kind":"If","args":{"condition":{"kind":"PaymentChosen","args":9},"then":{"instructions":[]},"otherwise":{"instructions":[]}}}
    ]);
    let normal = norm(p);
    let instructions = &normal[0]["args"]["body"]["instructions"];
    assert_ne!(
        instructions[0]["args"]["body"]["instructions"][0]["args"]["result"],
        instructions[1]["args"]["result"]
    );
    assert_eq!(
        instructions[1]["args"]["result"],
        instructions[2]["args"]["condition"]["args"]
    );
}

#[test]
fn payment_action_parts_have_private_definitions_and_publish_receipts_at_the_end() {
    fn payment(first: u32, last: u32) -> Value {
        let move_to = |destination: &str| {
            json!({"kind":"Move","args":{
            "objects":{"kind":"One","args":{"kind":"Source"}},"destination":destination,
            "entry":crate::ir::EntrySpec::default(),"face_down":false,"terms":crate::ir::MovementTerms::default()}})
        };
        let part = |id, destination| json!({"PlayerAction":{"player":{"kind":"AbilityController"},"body":{"instructions":[effect(id,move_to(destination))]}}});
        json!([{"kind":"Spell","args":{"announcement":{"targets":[],"modes":null},"body":{"instructions":[
            effect(500,json!({"kind":"PayCost","args":{"player":{"kind":"AbilityController"},"optional":false,"cost":{"parts":[part(first,"OwnersHand"),part(last,"OwnersGraveyard")]}}})),
            effect(501,json!({"kind":"NoteObjects","args":{"objects":{"kind":"Transition","args":{"result":last,"phase":"After"}}}}))
        ]}}}])
    }
    let a = norm(payment(17, 17));
    let b = norm(payment(23, 91));
    assert_eq!(a, b);
    let parts =
        &a[0]["args"]["body"]["instructions"][0]["args"]["operation"]["args"]["cost"]["parts"];
    assert_eq!(
        parts[0]["PlayerAction"]["body"]["instructions"][0]["args"]["result"],
        u32::MAX
    );
    assert_eq!(
        parts[1]["PlayerAction"]["body"]["instructions"][0]["args"]["result"],
        0
    );
}

#[test]
fn after_event_program_results_do_not_leak_into_the_replacement_body() {
    fn replacement(inner: u32, outer: u32) -> Value {
        let define = |id| {
            effect(
                id,
                json!({"kind":"BindValue","args":{"value":{"kind":"Constant","args":2}}}),
            )
        };
        let use_value = |id| {
            effect(
                500,
                json!({"kind":"GainLife","args":{
            "players":{"kind":"One","args":{"kind":"AbilityController"}},
            "amount":{"kind":"BoundValue","args":id}}}),
            )
        };
        json!([{"kind":"Spell","args":{"announcement":{"targets":[],"modes":null},"body":{"instructions":[
            effect(501, json!({"kind":"CreateReplacement","args":{
                "event":{"Enter":{"objects":{"kind":"Any"}}},"limit":"UntilExpired","duration":"EndOfTurn",
                "body":{"instructions":[
                    effect(502,json!({"kind":"ModifyEvent","args":{"change":{"Then":{"instructions":[define(inner),use_value(inner)]}}}})),
                    define(outer),use_value(outer)
                ]}
            }}))
        ]}}}])
    }
    assert_eq!(norm(replacement(17, 17)), norm(replacement(23, 91)));
}
