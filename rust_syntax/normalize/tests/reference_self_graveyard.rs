#[path = "support/reference_followup.rs"]
mod fixtures;

use serde_json::{json, Value};

const SELF_SHUFFLERS: &[&str] = &[
    "Dread",
    "Emrakul, the Aeons Torn",
    "Guile",
    "Hostility",
    "Kozilek, Butcher of Truth",
    "Purity",
    "Serra Avatar",
    "Ulamog, the Infinite Gyre",
    "Vigor",
    "Worldspine Wurm",
];

fn card(name: &str) -> Value {
    serde_json::to_value(fixtures::card(name)).unwrap()
}

fn custom(rule: &str) -> Value {
    let source = ron::from_str(&format!("Card(Name:\"Self reference control\",Typeline:(Supertypes:[],Cardtypes:[Creature],Subtypes:[]),CardPT:(Power:1,Toughness:1),Rules:[{rule}])")).unwrap();
    serde_json::to_value(mtgish_ast::lower::card(&source).unwrap()).unwrap()
}

fn graveyard_trigger(card: &Value) -> &Value {
    &card["abilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["kind"] == "Scoped" && a["args"]["zones"] == json!(["Graveyard"]))
        .unwrap()["args"]["ability"]["args"]
}

#[test]
fn named_self_graveyard_triggers_do_not_exclude_token_copies() {
    for name in SELF_SHUFFLERS {
        let c = card(name);
        let event = &graveyard_trigger(&c)["event"]["ZoneChange"];
        assert_eq!(event["from"], Value::Null);
        assert_eq!(event["to"], json!("Graveyard"));
        assert_eq!(
            event["objects"],
            json!({"kind":"All","args":[
                {"kind":"SameObject","args":{"kind":"Source"}},
                {"kind":"OwnedBy","args":{"kind":"All"}}
            ]}),
            "{name}"
        );
    }
}

#[test]
fn ordinary_and_named_card_triggers_keep_their_explicit_card_domain() {
    for filter in ["AnyCard", "IsNamed(NamedCard(\"Other printed name\"))"] {
        let c = custom(&format!("TriggerA(WhenACardIsPutIntoAPlayersGraveyardFromAnywhere({filter},AnyPlayer),ActionList([DrawACard]))"));
        let filters = c["abilities"][0]["args"]["event"]["ZoneChange"]["objects"]["args"]
            .as_array()
            .unwrap();
        assert!(filters.contains(&json!({"kind":"Not","args":{"kind":"Token"}})));
    }
}

fn owners<'a>(value: &'a Value, out: &mut Vec<&'a Value>) {
    match value {
        Value::Object(fields) => {
            if fields.get("kind").and_then(Value::as_str) == Some("OwnerOf") {
                out.push(&fields["args"]);
            }
            for child in fields.values() {
                owners(child, out);
            }
        }
        Value::Array(values) => {
            for child in values {
                owners(child, out);
            }
        }
        _ => {}
    }
}

#[test]
fn whole_graveyard_shuffle_uses_the_original_objects_owner_for_both_steps() {
    for name in [
        "Emrakul, the Aeons Torn",
        "Kozilek, Butcher of Truth",
        "Ulamog, the Infinite Gyre",
    ] {
        let c = card(name);
        let mut refs = vec![];
        owners(&graveyard_trigger(&c)["body"], &mut refs);
        assert_eq!(refs.len(), 2, "{name}");
        assert!(
            refs.iter().all(|v| **v == json!({"kind":"EventSubject"})),
            "{name}: {refs:?}"
        );
    }
}

#[test]
fn shuffling_only_the_source_keeps_its_destination_reference_and_live_zone_guard() {
    for name in [
        "Dread",
        "Guile",
        "Hostility",
        "Purity",
        "Serra Avatar",
        "Vigor",
        "Worldspine Wurm",
    ] {
        let c = card(name);
        let movement =
            &graveyard_trigger(&c)["body"]["instructions"][0]["args"]["operation"]["args"];
        assert_eq!(movement["destination"], json!("OwnersLibraryShuffled"));
        assert_eq!(
            movement["objects"],
            json!({"kind":"One","args":{
                "kind":"Query","args":{"objects":{"kind":"InZone","args":{
                    "objects":{"kind":"One","args":{"kind":"EventDestination"}},
                    "zone":"Graveyard"
                }},"index":0}
            }}),
            "{name}"
        );
    }
}

#[test]
fn an_event_card_owner_cannot_bind_without_a_matching_event() {
    let source = ron::from_str("Card(Name:\"Unbound owner control\",Typeline:(Supertypes:[],Cardtypes:[Sorcery],Subtypes:[]),Rules:[SpellActions(ActionList([PlayerAction(OwnerOfGraveyrdCard(Trigger_ThatGraveyardCard),ShuffleGraveyardIntoLibrary)]))])").unwrap();
    assert!(mtgish_ast::lower::card(&source).is_err());
}

#[test]
fn emrakul_limits_colored_protection_to_spells_without_changing_plain_color_protection() {
    let plain = custom("Protection(FromColor(Colored))");
    assert_eq!(
        plain["abilities"][0]["args"]["args"]["Objects"]["kind"],
        json!("Any")
    );
    let c = card("Emrakul, the Aeons Torn");
    assert_eq!(
        c["abilities"][3]["args"]["args"],
        json!({"Objects":{
            "kind":"Spell","filter":{"kind":"Not","args":{"kind":"Colorless"}}
        }})
    );
}
