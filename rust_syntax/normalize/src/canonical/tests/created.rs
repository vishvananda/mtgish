//! A future trigger's subject is distinct from the creator's matched event.
use crate::{canonical, ir::Ability, validate};
use serde_json::{json, Value};

fn spell_event() -> Value {
    json!({"Spell":{"event":"Cast","players":{"kind":"One","args":{"kind":"AbilityController"}},"spells":{"kind":"Any"}}})
}

fn self_event() -> Value {
    json!({"Object":{"event":"Attacks","objects":{"kind":"SameObject","args":{"kind":"Source"}}}})
}

fn damage_event(subject: &str) -> Value {
    json!({"Damage":{"source_kind":"Permanent","sources":{"kind":"Any"},
        "recipients":{"Objects":{"kind":"Any"}},"kind":"Any",
        "grouping":"SourceRecipient","subject":subject}})
}

fn effect(result: u32, operation: Value) -> Value {
    json!({"kind":"Effect","args":{"result":result,"operation":operation}})
}

fn created(event: Value, instructions: Vec<Value>) -> Value {
    json!({"kind":"CreateTrigger","args":{"event":event,"duration":"EndOfTurn",
        "announcement":{"targets":[],"modes":null},"once":true,"intervening_if":null,
        "body":{"instructions":instructions}}})
}

fn ability(event: Value, operation: Value) -> Value {
    json!([{"kind":"Triggered","args":{"event":event,"limits":{"per_turn":null,"total":null},
        "intervening_if":null,"announcement":{"targets":[],"modes":null},
        "body":{"instructions":[effect(2,operation)]}}}])
}

fn copy(object: Value) -> Value {
    json!({"kind":"CopyOnStack","args":{"actor":{"kind":"One","args":{"kind":"AbilityController"}},
        "object":object,"times":{"kind":"Constant","args":1},"targets":"May","effects":[],"card":null}})
}

fn valid(value: &Value) -> bool {
    let abilities: Vec<Ability> = serde_json::from_value(value.clone()).unwrap();
    validate::abilities(&abilities).is_ok()
}

#[test]
fn future_spell_subject_is_not_the_creating_sources_own_subject() {
    let mut value = ability(
        self_event(),
        created(
            spell_event(),
            vec![effect(0, copy(json!({"kind":"EventSubject"})))],
        ),
    );
    assert!(valid(&value));
    canonical::abilities(&mut value).unwrap();
    assert!(valid(&value));
    let object = value.pointer("/0/args/body/instructions/0/args/operation/args/body/instructions/0/args/operation/args/object").unwrap();
    assert_eq!(object, &json!({"kind":"EventSubject"}));
}

#[test]
fn a_new_event_never_repairs_an_invalid_creator_damage_role_read() {
    let mut value = ability(
        damage_event("Source"),
        created(
            spell_event(),
            vec![effect(0, copy(json!({"kind":"Damage","args":"Source"})))],
        ),
    );
    assert!(!valid(&value));
    canonical::abilities(&mut value).unwrap();
    assert!(!valid(&value));
}

#[test]
fn the_new_damage_subject_uses_its_own_role() {
    let reveal = json!({"kind":"Reveal","args":{"objects":{"kind":"One","args":{"kind":"Damage","args":"Recipient"}}}});
    let mut value = ability(
        damage_event("Source"),
        created(damage_event("Recipient"), vec![effect(0, reveal)]),
    );
    assert!(valid(&value));
    canonical::abilities(&mut value).unwrap();
    assert!(valid(&value));
    let object = value.pointer("/0/args/body/instructions/0/args/operation/args/body/instructions/0/args/operation/args/objects/args").unwrap();
    assert_eq!(object, &json!({"kind":"EventSubject"}));
}

#[test]
fn nested_created_events_restore_the_enclosing_reference_frame() {
    let inner = created(
        spell_event(),
        vec![effect(0, copy(json!({"kind":"EventSubject"})))],
    );
    let mut value = ability(self_event(), created(self_event(), vec![effect(1, inner)]));
    assert!(valid(&value));
    canonical::abilities(&mut value).unwrap();
    assert!(valid(&value));
    let object = value.pointer("/0/args/body/instructions/0/args/operation/args/body/instructions/0/args/operation/args/body/instructions/0/args/operation/args/object").unwrap();
    assert_eq!(object, &json!({"kind":"EventSubject"}));
    let once = value.clone();
    canonical::abilities(&mut value).unwrap();
    assert_eq!(value, once);
}

#[test]
fn a_following_instruction_uses_the_creator_event_again() {
    let mut value = ability(
        damage_event("Source"),
        created(
            spell_event(),
            vec![effect(0, copy(json!({"kind":"EventSubject"})))],
        ),
    );
    value[0]["args"]["body"]["instructions"].as_array_mut().unwrap().push(effect(3,
        json!({"kind":"Reveal","args":{"objects":{"kind":"One","args":{"kind":"Damage","args":"Source"}}}})));
    assert!(valid(&value));
    canonical::abilities(&mut value).unwrap();
    assert!(valid(&value));
    assert_eq!(
        value.pointer("/0/args/body/instructions/1/args/operation/args/objects/args"),
        Some(&json!({"kind":"EventSubject"}))
    );
}

#[test]
fn a_future_spell_does_not_inherit_the_creators_move_destination() {
    let event = json!({"ZoneChange":{"objects":{"kind":"SameObject","args":{"kind":"Source"}},
        "from":"Battlefield","to":"Exile"}});
    let object = json!({"kind":"Query","args":{"objects":{"kind":"InZone","args":{
        "objects":{"kind":"One","args":{"kind":"EventDestination"}},"zone":"Exile"}},"index":0}});
    let mut value = ability(
        event,
        created(
            spell_event(),
            vec![effect(
                0,
                json!({"kind":"Reveal","args":{"objects":{"kind":"One","args":object}}}),
            )],
        ),
    );
    assert!(!valid(&value));
    canonical::abilities(&mut value).unwrap();
    assert!(!valid(&value));
}

#[test]
fn sibling_created_triggers_have_independent_target_definitions() {
    fn targeted(kind: &str, slot: u16) -> Value {
        let target = serde_json::to_value(
            crate::targets::TargetSelector::creature().into_spec(crate::ir::Slot(slot)),
        )
        .unwrap();
        let mut op = created(
            spell_event(),
            vec![effect(
                0,
                json!({"kind":"Reveal","args":{"objects":{"kind":"Targets","args":slot}}}),
            )],
        );
        if kind == "ReflexiveTrigger" {
            op = json!({"kind":"ReflexiveTrigger","args":{"performed":null,"condition":null,"announcement":{"targets":[target],"modes":null},"body":op["args"]["body"]}});
        } else {
            op["args"]["announcement"]["targets"] = json!([target]);
        }
        op
    }
    for kind in ["CreateTrigger", "ReflexiveTrigger"] {
        let normalize = |a, b| {
            let mut value = ability(self_event(), targeted(kind, a));
            value[0]["args"]["body"]["instructions"]
                .as_array_mut()
                .unwrap()
                .push(effect(8, targeted(kind, b)));
            assert!(valid(&value));
            canonical::abilities(&mut value).unwrap();
            assert!(valid(&value));
            value
        };
        assert_eq!(normalize(17, 17), normalize(23, 91), "{kind}");
    }
}
