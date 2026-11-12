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

#[test]
fn bellowing_fiend_deals_both_parts_simultaneously_with_the_actual_damaged_creatures_controller() {
    let c = card("Bellowing Fiend");
    let trigger = &c["abilities"][1]["args"];
    assert_eq!(trigger["body"]["instructions"].as_array().unwrap().len(), 1);
    let damage = operation(&c, "DealDamageParts");
    assert_eq!(
        *damage,
        json!({"source":{"kind":"Source"},"preventable":true,"parts":[
            {"amount":{"kind":"Constant","args":3},"recipients":{"kind":"Players","args":{"kind":"One","args":{"kind":"ControllerOf","args":{"kind":"Damage","args":"Recipient"}}}}},
            {"amount":{"kind":"Constant","args":3},"recipients":{"kind":"Players","args":{"kind":"One","args":{"kind":"AbilityController"}}}}
        ]})
    );
    assert_eq!(
        trigger["event"]["Damage"]["recipients"],
        json!({"Objects":{"kind":"CardType","args":"Creature"}})
    );
}

fn two_target_damage(name: &str, source: &str, first_ability: usize, optional: bool) {
    let c = card(name);
    let damage = operation(&c, "DealDamageParts");
    let second = if optional {json!({"kind":"Targets","args":1})} else {json!({"kind":"One","args":{"kind":"Target","args":{"slot":1,"index":0}}})};
    assert_eq!(
        *damage,
        json!({"source":{"kind":source},"preventable":true,"parts":[
            {"amount":{"kind":"Constant","args":3},"recipients":{"kind":"Targets","args":0}},
            {"amount":{"kind":"Constant","args":3},"recipients":{"kind":"Objects","args":second}}
        ]})
    );
    let ability = &c["abilities"][first_ability]["args"];
    assert_eq!(
        ability["announcement"]["targets"].as_array().unwrap().len(),
        2
    );
    assert_eq!(ability["body"]["instructions"].as_array().unwrap().len(), 1);
}

#[test]
fn burning_suns_avatar_preserves_two_damage_parts_even_when_both_slots_name_one_object() {
    two_target_damage("Burning Sun's Avatar", "EventSubject", 0, true);
    let c = card("Burning Sun's Avatar");
    let targets = &c["abilities"][0]["args"]["announcement"]["targets"];
    assert_eq!(
        targets[1]["count"],
        json!({"UpTo":{"kind":"Constant","args":1}})
    );
    assert_eq!(
        targets[1]["filter"],
        json!({"kind":"CardType","args":"Creature"})
    );
}

#[test]
fn punish_the_enemy_keeps_both_required_targets_and_separate_damage_parts() {
    two_target_damage("Punish the Enemy", "Source", 0, false);
    let c = card("Punish the Enemy");
    for target in c["abilities"][0]["args"]["announcement"]["targets"]
        .as_array()
        .unwrap()
    {
        assert_eq!(
            target["count"],
            json!({"Exactly":{"kind":"Constant","args":1}})
        );
    }
}

#[test]
fn garruk_gains_target_toughness_independently_of_successful_destruction() {
    let c = card("Garruk, Apex Predator");
    let ability = &c["abilities"][2]["args"];
    let instructions = ability["body"]["instructions"].as_array().unwrap();
    assert_eq!(instructions.len(), 2);
    assert_eq!(
        instructions[0]["args"]["operation"]["kind"],
        json!("Destroy")
    );
    assert_eq!(
        operation(ability, "GainLife")["amount"],
        json!({"kind":"ObjectProperty","args":{
            "object":{"kind":"Target","args":{"slot":0,"index":0}},"property":"Toughness","information":"CurrentOrLastKnown"
        }})
    );
    assert_eq!(
        ability["cost"]["parts"],
        json!([{"Loyalty":{"kind":"Constant","args":-3}}])
    );
}

#[test]
fn nogi_replaces_its_old_creature_types_while_preserving_power_toughness_flying_and_duration() {
    let c = card("Nogi, Draco-Zealot");
    let effect = operation(&c, "CreateContinuousEffect");
    assert_eq!(
        effect["changes"],
        json!([
            {"kind":"CreatureTypes","args":{"mode":"Set","types":{"Listed":["Dragon"]}}},
            {"kind":"SetBasePowerToughness","args":{"power":{"kind":"Constant","args":5},"toughness":{"kind":"Constant","args":5}}},
            {"kind":"GrantKeyword","args":{"kind":"Flying"}}
        ])
    );
    assert_eq!(effect["duration"], json!("EndOfTurn"));
    assert_eq!(
        c["abilities"][1]["args"]["intervening_if"]["args"]["test"]["args"]["value"],
        json!({"kind":"Constant","args":3})
    );
}

#[test]
fn both_reptil_forms_replace_creature_types_with_dinosaur_hero() {
    let c = card("Reptil, Dinomorpher");
    for (index, power, toughness) in [(0, 3, 5), (1, 6, 6)] {
        let effect = operation(&c["abilities"][index], "CreateContinuousEffect");
        assert_eq!(
            effect["changes"][0],
            json!({"kind":"CreatureTypes","args":{"mode":"Set","types":{"Listed":["Dinosaur","Hero"]}}})
        );
        assert_eq!(
            effect["changes"][1],
            json!({"kind":"SetBasePowerToughness","args":{"power":{"kind":"Constant","args":power},"toughness":{"kind":"Constant","args":toughness}}})
        );
        assert_eq!(effect["duration"], json!("EndOfTurn"));
    }
}

#[test]
fn explicit_add_type_source_and_successful_destruction_receipts_remain_distinct() {
    let source=ron::from_str("Card(Name:\"Independent effects control\",Typeline:(Supertypes:[],Cardtypes:[Instant],Subtypes:[]),Rules:[SpellActions(Targeted([TargetPermanent(IsCardtype(Creature))],ActionList([CreatePermanentLayerEffectUntil(Ref_TargetPermanent,[AddCreatureType(Dragon)],UntilEndOfTurn),DestroyPermanent(Ref_TargetPermanent),If(APermanentWasDestroyedThisWay(IsCardtype(Creature)),[GainLife(ToughnessOfCreatureDestroyedThisWay)])])))])").unwrap();
    let c = serde_json::to_value(mtgish_ast::lower::card(&source).unwrap()).unwrap();
    assert_eq!(
        operation(&c, "CreateContinuousEffect")["changes"][0]["args"]["mode"],
        json!("Add")
    );
    let gain = operation(&c, "GainLife");
    assert_eq!(gain["amount"]["kind"], json!("Aggregate"));
    assert_eq!(
        gain["amount"]["args"]["objects"],
        json!({"kind":"Transition","args":{"result":1,"phase":"Before"}})
    );
}
