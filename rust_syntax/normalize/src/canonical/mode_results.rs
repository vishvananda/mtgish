//! Synthetic IR only: mode-local labels are not definition identities.
use crate::{
    canonical,
    ir::{Ability, Destination, EntrySpec, MovementTerms, ObjectRef, ObjectSet, Operation},
    validate,
};
use serde_json::{json, Value};

fn effect(result: u32, operation: Value) -> Value {
    json!({"kind":"Effect","args":{"result":result,"operation":operation}})
}

fn bind(result: u32) -> Value {
    effect(
        result,
        json!({"kind":"BindValue","args":{"value":{"kind":"Constant","args":3}}}),
    )
}

fn read(result: u32, receipt: u32) -> Value {
    effect(
        result,
        json!({"kind":"GainLife","args":{
        "players":{"kind":"One","args":{"kind":"AbilityController"}},
        "amount":{"kind":"BoundValue","args":receipt}}}),
    )
}

fn modal(bodies: Vec<Vec<Value>>, common: Vec<Value>) -> Value {
    let modes: Vec<_> = bodies
        .into_iter()
        .map(|instructions| {
            json!({
                "announcement":{"targets":[],"modes":null},
                "body":{"instructions":instructions},"weight":1,"requires":null
            })
        })
        .collect();
    json!([{"kind":"Spell","args":{
        "announcement":{"targets":[],"modes":{
            "minimum":{"kind":"Constant","args":1},
            "maximum":{"kind":"Constant","args":1},
            "repeat":false,"modes":modes,"implied":false,"unchosen":null,
            "surcharge":null,"distinct_targets":false,"chooser":null,
            "random":false,"budget":null,"may_decline":false}},
        "body":{"instructions":common}}}])
}

fn valid(value: &Value) -> Result<(), String> {
    let abilities: Vec<Ability> = serde_json::from_value(value.clone()).unwrap();
    validate::abilities(&abilities)
}

fn norm(mut value: Value) -> Value {
    valid(&value).unwrap();
    canonical::abilities(&mut value).unwrap();
    valid(&value).unwrap();
    let mut twice = value.clone();
    canonical::abilities(&mut twice).unwrap();
    assert_eq!(value, twice, "canonicalization must be idempotent");
    value
}

#[test]
fn reused_and_fresh_unused_mode_labels_are_alpha_equivalent() {
    let reused = norm(modal(vec![vec![bind(17)], vec![bind(17)]], vec![]));
    let fresh = norm(modal(vec![vec![bind(23)], vec![bind(91)]], vec![]));
    assert_eq!(reused, fresh);
    let modes = &reused[0]["args"]["announcement"]["modes"]["modes"];
    assert_eq!(
        modes[0]["body"]["instructions"][0]["args"]["result"],
        u32::MAX
    );
    assert_eq!(
        modes[1]["body"]["instructions"][0]["args"]["result"],
        u32::MAX - 1
    );
}

#[test]
fn each_used_mode_local_read_resolves_its_own_definition() {
    assert_eq!(
        norm(modal(
            vec![vec![bind(7), read(8, 7)], vec![bind(7), read(8, 7)]],
            vec![]
        )),
        norm(modal(
            vec![vec![bind(40), read(41, 40)], vec![bind(50), read(51, 50)]],
            vec![]
        ))
    );
}

#[test]
fn invalid_separate_common_body_does_not_alias_mode_definitions() {
    // A modal spell cannot also have a common body. Keep this malformed IR
    // refused, while checking that canonicalization cannot repair it by alias.
    let mut reused = modal(vec![vec![bind(7)]], vec![bind(7), read(8, 7)]);
    let mut fresh = modal(vec![vec![bind(80)]], vec![bind(90), read(91, 90)]);
    assert!(valid(&reused).is_err());
    canonical::abilities(&mut reused).unwrap();
    canonical::abilities(&mut fresh).unwrap();
    assert_eq!(reused, fresh);
    assert!(valid(&reused).is_err());
}

#[test]
fn mode_definitions_cannot_supply_sibling_or_common_reads() {
    for value in [
        modal(vec![vec![bind(7)], vec![read(8, 7)]], vec![]),
        modal(vec![vec![bind(7)]], vec![read(8, 7)]),
    ] {
        assert!(valid(&value).is_err());
    }
}

#[test]
fn same_scope_duplicate_definitions_remain_invalid() {
    let mut value = modal(vec![vec![bind(7), bind(7), read(8, 7)]], vec![]);
    assert!(valid(&value).is_err());
    canonical::abilities(&mut value).unwrap();
    assert!(valid(&value).unwrap_err().contains("duplicate result"));
}

#[test]
fn immediate_optional_receipt_exports_survive_mode_scope() {
    let body = |id| {
        let movement = serde_json::to_value(Operation::Move {
            objects: ObjectSet::One(ObjectRef::Source),
            destination: Destination::Exile,
            entry: EntrySpec::default(),
            face_down: false,
            terms: MovementTerms::default(),
        })
        .unwrap();
        vec![
            json!({"kind":"Optional","args":{"decider":{"kind":"AbilityController"},
            "body":{"instructions":[effect(id, movement)]}}}),
            effect(
                id + 1,
                json!({"kind":"Reveal","args":{"objects":{
            "kind":"Transition","args":{"result":id,"phase":"After"}}}}),
            ),
        ]
    };
    assert_eq!(
        norm(modal(vec![body(7), body(7)], vec![])),
        norm(modal(vec![body(20), body(40)], vec![]))
    );
}

#[test]
fn mode_bearing_created_trigger_retains_genuine_outer_capture() {
    let captured = |outer, creator, local| {
        let nested = modal(
            vec![
                vec![read(80, outer), bind(local)],
                vec![read(81, outer), bind(local)],
            ],
            vec![],
        );
        let mut value = modal(vec![], vec![]);
        value[0]["args"]["announcement"]["modes"] = Value::Null;
        value[0]["args"]["body"]["instructions"] = json!([
            bind(outer),
            effect(
                creator,
                json!({"kind":"CreateTrigger","args":{
                "event":{"Turn":{"point":"UpkeepBegins","players":{"kind":"One","args":{"kind":"AbilityController"}}}},
                "duration":"EndOfTurn","once":false,"intervening_if":null,
                "announcement":nested[0]["args"]["announcement"],
                "body":{"instructions":[]}}})
            )
        ]);
        value
    };
    let value = norm(captured(7, 8, 9));
    assert_eq!(value, norm(captured(17, 18, 19)));
    assert_eq!(
        value[0]["args"]["body"]["instructions"][0]["args"]["result"],
        0
    );
}
