use mtgish_ast::{canonical, ir::Ability, semantics, validate};
use serde_json::{json, Value};

fn manifest(count: u32, hand: bool, cloak: bool) -> Value {
    let source = if hand {
        json!({"Cards":{"kind":"Chosen","args":{
            "chooser":{"kind":"One","args":{"kind":"AbilityController"}},
            "objects":{"kind":"Zone","args":{"scope":{"Players":{
                "zone":"Hand","players":{"kind":"One","args":{"kind":"AbilityController"}}
            }},"kind":"Card","filter":{"kind":"Any"}}},
            "count":{"Exactly":{"kind":"Constant","args":count}}
        }}})
    } else {
        json!({"TopOfLibrary":{"player":{"kind":"AbilityController"},
            "count":{"kind":"Constant","args":count}}})
    };
    json!({"kind":"Manifest","args":{
        "actor":{"kind":"One","args":{"kind":"AbilityController"}},
        "source":source,"cloak":cloak,"entry":[]
    }})
}

fn card(operation: Value, objects: Value) -> Value {
    json!([{"kind":"Spell","args":{
        "announcement":{"targets":[],"modes":null},
        "body":{"instructions":[
            {"kind":"Effect","args":{"result":17,"operation":operation}},
            {"kind":"Effect","args":{"result":23,"operation":{
                "kind":"Tap","args":{"objects":objects,"tapped":true}
            }}}
        ]}
    }}])
}

fn checked(value: &Value) {
    let typed: Vec<Ability> = serde_json::from_value(value.clone()).unwrap();
    validate::abilities(&typed).unwrap();
    let mut canonical = value.clone();
    canonical::abilities(&mut canonical).unwrap();
    let typed: Vec<Ability> = serde_json::from_value(canonical.clone()).unwrap();
    validate::abilities(&typed).unwrap();
    let once = canonical.clone();
    canonical::abilities(&mut canonical).unwrap();
    assert_eq!(canonical, once);
}

#[test]
fn all_manifest_sources_keep_the_reviewed_transition_contract() {
    let baseline = serde_json::from_value(manifest(1, false, false)).unwrap();
    let expected = semantics::yields(&baseline);
    assert!(expected.is_some());
    for hand in [false, true] {
        for cloak in [false, true] {
            for count in [0, 1, 2] {
                let operation = manifest(count, hand, cloak);
                let typed = serde_json::from_value(operation.clone()).unwrap();
                assert_eq!(semantics::yields(&typed), expected);
                checked(&card(
                    operation,
                    json!({"kind":"Transition","args":{
                        "result":17,"phase":"After"
                    }}),
                ));
            }
        }
    }
}

#[test]
fn a_single_chosen_hand_card_can_be_referenced_after_manifest_or_cloak() {
    for cloak in [false, true] {
        checked(&card(
            manifest(1, true, cloak),
            json!({"kind":"One","args":{
                "kind":"Result","args":{"result":17,"phase":"After","index":0}
            }}),
        ));
    }
}

#[test]
fn result_references_still_require_their_definition() {
    let value = card(
        manifest(1, true, false),
        json!({"kind":"Transition","args":{
            "result":99,"phase":"After"
        }}),
    );
    let typed: Vec<Ability> = serde_json::from_value(value).unwrap();
    assert!(validate::abilities(&typed).is_err());
}
