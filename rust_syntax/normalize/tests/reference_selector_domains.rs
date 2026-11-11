#[path = "support/reference_followup.rs"]
mod fixtures;

use serde_json::{json, Value};

fn card(name: &str) -> Value {
    serde_json::to_value(fixtures::card(name)).unwrap()
}

fn operation<'a>(value: &'a Value, kind: &str) -> &'a Value {
    fn find<'a>(v: &'a Value, kind: &str) -> Option<&'a Value> {
        match v {
            Value::Object(fields) => {
                if fields.get("kind").and_then(Value::as_str) == Some(kind) {
                    return fields.get("args");
                }
                fields.values().find_map(|v| find(v, kind))
            }
            Value::Array(values) => values.iter().find_map(|v| find(v, kind)),
            _ => None,
        }
    }
    find(value, kind).unwrap_or_else(|| panic!("missing {kind}"))
}

fn target(index: u32) -> Value {
    json!({"kind":"Target","args":{"slot":index,"index":0}})
}

fn other_color_creature() -> Value {
    json!({"kind":"AllOf","args":[
        {"kind":"Not","args":{"kind":"SameObject","args":target(0)}},
        {"kind":"CardType","args":"Creature"},
        {"kind":"Shares","args":{"property":"Colors","with":target(0)}}
    ]})
}

#[test]
fn protection_radiance_restricts_only_the_other_recipients_to_creatures() {
    let c = card("Bathe in Light");
    let continuous = operation(&c, "CreateContinuousEffect");
    assert_eq!(
        continuous["affected"]["Objects"]["args"]["filter"],
        json!({"kind":"AnyOf","args":[
            {"kind":"SameObject","args":target(0)}, other_color_creature()
        ]})
    );
    assert_eq!(continuous["duration"], json!("EndOfTurn"));
    assert_eq!(
        continuous["changes"],
        json!([{"kind":"GrantKeyword","args":{"kind":"Protection","args":{"Objects":{"kind":"Any","filter":{"kind":"Chosen","args":"Color"}}}}}])
    );
    assert_eq!(
        c["abilities"][0]["args"]["announcement"]["targets"][0]["filter"],
        json!({"kind":"CardType","args":"Creature"})
    );
}

#[test]
fn prevention_radiance_preserves_the_target_once_and_the_one_damage_per_recipient_limit() {
    let c = card("Wojek Apothecary");
    let replacement = operation(&c, "CreateReplacement");
    assert_eq!(
        replacement["event"]["Damage"]["recipients"]["Objects"],
        json!({"kind":"AnyOf","args":[
            {"kind":"SameObject","args":target(0)},other_color_creature()
        ]})
    );
    assert_eq!(
        replacement["limit"],
        json!({"DamageToEach":{"kind":"Constant","args":1}})
    );
    assert_eq!(replacement["duration"], json!("EndOfTurn"));
    assert_eq!(
        replacement["event"]["Damage"]["sources"],
        json!({"kind":"Any"})
    );
}

#[test]
fn fated_clash_grants_indestructible_to_either_target_before_destroying_all_creatures() {
    let c = card("Fated Clash");
    let continuous = operation(&c, "CreateContinuousEffect");
    assert_eq!(
        continuous["affected"]["Objects"]["args"]["filter"],
        json!({"kind":"AnyOf","args":[
            {"kind":"SameObject","args":target(0)},{"kind":"SameObject","args":target(1)}
        ]})
    );
    assert_eq!(
        continuous["changes"],
        json!([{"kind":"GrantKeyword","args":{"kind":"Indestructible"}}])
    );
    let spell = &c["abilities"][1]["args"];
    assert_eq!(
        spell["announcement"]["targets"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        spell["announcement"]["targets"][0]["filter"]["args"][1],
        json!({"kind":"ControlledBy","args":{"kind":"One","args":{"kind":"AbilityController"}}})
    );
    assert_eq!(
        spell["announcement"]["targets"][1]["filter"]["args"][1],
        json!({"kind":"ControlledBy","args":{"kind":"OpponentsOf","args":{"kind":"AbilityController"}}})
    );
    let instructions = spell["body"]["instructions"].as_array().unwrap();
    assert_eq!(
        instructions[0]["args"]["operation"]["kind"],
        json!("CreateContinuousEffect")
    );
    assert_eq!(
        instructions[1]["args"]["operation"]["kind"],
        json!("Destroy")
    );
    assert_eq!(
        operation(&c, "Destroy")["objects"]["args"]["filter"],
        json!({"kind":"CardType","args":"Creature"})
    );
}

#[test]
fn rukarumel_stack_type_change_requires_your_control_and_keeps_other_zone_scopes() {
    let c = card("Rukarumel, Biologist");
    let stack = &c["abilities"][2]["args"]["affected"]["Objects"];
    assert_eq!(stack["args"]["scope"], json!({"Shared":"Stack"}));
    assert_eq!(stack["args"]["kind"], json!("Spell"));
    assert_eq!(
        stack["args"]["filter"],
        json!({"kind":"AllOf","args":[
            {"kind":"CardType","args":"Creature"},{"kind":"ControlledBy","args":{"kind":"One","args":{"kind":"AbilityController"}}}
        ]})
    );
    let cards = &c["abilities"][3]["args"]["affected"]["Objects"]["args"];
    assert_eq!(
        cards["args"][1],
        json!({"kind":"OwnedBy","args":{"kind":"One","args":{"kind":"AbilityController"}}})
    );
    assert_eq!(
        cards["args"][2],
        json!({"kind":"Not","args":{"kind":"InZone","args":{"Shared":"Battlefield"}}})
    );
    assert_eq!(
        c["abilities"][1]["args"]["affected"]["Objects"]["args"]["filter"]["kind"],
        json!("AnyOf")
    );
}

#[test]
fn soul_diviner_removes_one_unspecified_counter_only_from_your_four_permanent_types() {
    let c = card("Soul Diviner");
    let removal = &c["abilities"][0]["args"]["cost"]["parts"][1]["RemoveCounters"]["removals"][0];
    assert_eq!(
        removal["recipients"]["args"]["args"]["filter"],
        json!({"kind":"AllOf","args":[
            {"kind":"AnyOf","args":[{"kind":"CardType","args":"Artifact"},{"kind":"CardType","args":"Creature"},{"kind":"CardType","args":"Land"},{"kind":"CardType","args":"Planeswalker"}]},
            {"kind":"ControlledBy","args":{"kind":"One","args":{"kind":"AbilityController"}}}
        ]})
    );
    assert_eq!(removal["kind"], Value::Null);
    assert_eq!(
        removal["amount"],
        json!({"Count":{"Exactly":{"kind":"Constant","args":1}}})
    );
    assert_eq!(
        c["abilities"][0]["args"]["cost"]["parts"][0],
        json!("TapSymbol")
    );
    assert_eq!(
        operation(&c, "Draw")["amount"],
        json!({"kind":"Constant","args":1})
    );
}

#[test]
fn soulcleaver_another_excludes_only_the_equipment_and_keeps_the_equipped_recipient() {
    let c = card("Tarrian's Soulcleaver");
    let event = &c["abilities"][1]["args"]["event"]["ZoneChange"];
    assert_eq!(
        event["objects"]["args"][0],
        json!({"kind":"AllOf","args":[
            {"kind":"Not","args":{"kind":"SameObject","args":{"kind":"Source"}}},
            {"kind":"AnyOf","args":[{"kind":"CardType","args":"Artifact"},{"kind":"CardType","args":"Creature"}]}
        ]})
    );
    assert_eq!(event["from"], json!("Battlefield"));
    assert_eq!(event["to"], json!("Graveyard"));
    let placement = &operation(&c, "PutCounters")["placements"][0];
    assert_eq!(
        placement["recipients"],
        json!({"kind":"Objects","args":{"kind":"One","args":{"kind":"AttachedTo","args":{"kind":"Source"}}}})
    );
    assert_eq!(placement["kind"], json!({"PowerToughness":[1,1]}));
}

#[test]
fn tranquil_domain_destroys_non_aura_enchantments_and_keeps_regeneration() {
    let c = card("Tranquil Domain");
    let destroy = operation(&c, "Destroy");
    assert_eq!(
        destroy["objects"]["args"]["filter"],
        json!({"kind":"AllOf","args":[
            {"kind":"Not","args":{"kind":"Subtype","args":"Aura"}},{"kind":"CardType","args":"Enchantment"}
        ]})
    );
    assert_eq!(destroy["regeneration"], json!(true));
}
