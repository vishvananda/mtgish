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

fn affinity(name: &str, base: Value) {
    let c = card(name);
    let static_effect = &c["abilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| {
            a["kind"] == "Static"
                && a["args"]["affected"]["Objects"]["args"]["scope"] == json!({"Shared":"Stack"})
        })
        .unwrap()["args"];
    let objects = &static_effect["affected"]["Objects"];
    assert_eq!(objects["args"]["kind"], json!("Spell"));
    assert_eq!(
        objects["args"]["filter"],
        json!({"kind":"AllOf","args":[base,
            {"kind":"Cast","args":{"by":{"kind":"One","args":{"kind":"AbilityController"}},"from":null}}
        ]})
    );
    assert!(static_effect["changes"].to_string().contains("Affinity"));
}

#[test]
fn sami_affinity_applies_to_your_cast_spells() {
    affinity("Sami, Wildcat Captain", json!({"kind":"Any"}));
}
#[test]
fn tezzeret_affinity_keeps_creature_or_planeswalker_and_requires_your_cast() {
    affinity(
        "Tezzeret, Master of the Bridge",
        json!({"kind":"AnyOf","args":[{"kind":"CardType","args":"Creature"},{"kind":"CardType","args":"Planeswalker"}]}),
    );
}
#[test]
fn mycosynth_golem_affinity_keeps_both_artifact_and_creature_and_requires_your_cast() {
    affinity(
        "Mycosynth Golem",
        json!({"kind":"AllOf","args":[{"kind":"CardType","args":"Artifact"},{"kind":"CardType","args":"Creature"}]}),
    );
}
#[test]
fn pearl_ear_affinity_keeps_enchantment_and_requires_your_cast() {
    affinity(
        "Pearl-Ear, Imperial Advisor",
        json!({"kind":"CardType","args":"Enchantment"}),
    );
}
#[test]
fn witherbloom_affinity_keeps_instant_or_sorcery_and_requires_your_cast() {
    affinity(
        "Witherbloom, the Balancer",
        json!({"kind":"AnyOf","args":[{"kind":"CardType","args":"Instant"},{"kind":"CardType","args":"Sorcery"}]}),
    );
}

#[test]
fn simic_guildmage_chooses_a_new_host_with_the_current_hosts_controller() {
    let c = card("Simic Guildmage");
    let choose = operation(&c["abilities"][1], "ChooseObjects");
    let aura = json!({"kind":"Target","args":{"slot":0,"index":0}});
    let host = json!({"kind":"AttachedTo","args":aura});
    assert_eq!(
        choose["objects"]["args"]["filter"],
        json!({"kind":"AllOf","args":[
            {"kind":"Not","args":{"kind":"SameObject","args":host}},
            {"kind":"ControlledBy","args":{"kind":"One","args":{"kind":"ControllerOf","args":host}}}
        ]})
    );
    assert_eq!(choose["objects"]["args"]["kind"], json!("Permanent"));
    assert_eq!(
        c["abilities"][1]["args"]["announcement"]["targets"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let attach = operation(&c["abilities"][1], "Attach");
    assert_eq!(attach["attachments"], json!({"kind":"One","args":aura}));
    assert_eq!(
        attach["host"],
        json!({"kind":"Result","args":{"result":0,"phase":"After","index":0}})
    );
    // The separate counter-transfer ability still compares its two creatures.
    let targets = &c["abilities"][0]["args"]["announcement"]["targets"];
    assert_eq!(targets.as_array().unwrap().len(), 2);
    assert!(targets[1]["filter"].to_string().contains("ControllerOf"));
    assert!(!targets[1]["filter"].to_string().contains("AttachedTo"));
}

#[test]
fn sunflare_keeps_separate_target_and_self_damage_with_the_same_elemental_card_count() {
    let c = card("Sunflare Shaman");
    let damage = operation(&c, "DealDamageParts");
    assert_eq!(damage["source"], json!({"kind":"Source"}));
    assert_eq!(damage["preventable"], json!(true));
    let parts = damage["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0]["recipients"], json!({"kind":"Targets","args":0}));
    assert_eq!(
        parts[1]["recipients"],
        json!({"kind":"Objects","args":{"kind":"One","args":{"kind":"Source"}}})
    );
    assert_eq!(parts[0]["amount"], parts[1]["amount"]);
    assert_eq!(
        parts[0]["amount"],
        json!({"kind":"CountObjects","args":{"kind":"Zone","args":{
            "scope":{"Players":{"zone":"Graveyard","players":{"kind":"All"}}},"kind":"Card",
            "filter":{"kind":"AllOf","args":[{"kind":"Subtype","args":"Elemental"},{"kind":"InZone","args":{"Players":{"zone":"Graveyard","players":{"kind":"One","args":{"kind":"AbilityController"}}}}}]}
        }}})
    );
    assert_eq!(
        c["abilities"][0]["args"]["cost"]["parts"][1],
        json!("TapSymbol")
    );
    assert_eq!(
        c["abilities"][0]["args"]["announcement"]["targets"][0]["constraints"],
        json!([])
    );
}

#[test]
fn a_bare_all_spell_affinity_source_keeps_its_explicit_unrestricted_domain() {
    let source=ron::from_str("Card(Name:\"Unrestricted affinity control\",Typeline:(Supertypes:[],Cardtypes:[Enchantment],Subtypes:[]),Rules:[StackSpellsEffect(AnySpell,[Affinity(IsCardtype(Artifact))])])").unwrap();
    let c = serde_json::to_value(mtgish_ast::lower::card(&source).unwrap()).unwrap();
    assert_eq!(
        c["abilities"][0]["args"]["affected"]["Objects"]["args"]["filter"],
        json!({"kind":"Any"})
    );
}

#[test]
fn self_destruct_keeps_its_other_target_constraint_and_disjoint_recipient_union() {
    let c = card("Self-Destruct");
    assert_eq!(
        c["abilities"][0]["args"]["announcement"]["targets"][1]["constraints"],
        json!([{"DifferentFromTarget":0}])
    );
    assert_eq!(
        operation(&c, "DealDamage")["recipients"]["kind"],
        json!("Union")
    );
    assert_eq!(
        operation(&c, "DealDamage")["amount"]["args"]["property"],
        json!("Power")
    );
}
