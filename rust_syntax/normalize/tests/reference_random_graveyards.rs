#[path = "support/reference_followup.rs"]
mod fixtures;

use serde_json::{json, Value};

fn operation<'a>(value: &'a Value, kind: &str) -> &'a Value {
    match value {
        Value::Object(fields) => {
            if fields.get("kind").and_then(Value::as_str) == Some(kind) {
                return &fields["args"];
            }
            for child in fields.values() {
                if contains(child, kind) {
                    return operation(child, kind);
                }
            }
        }
        Value::Array(values) => {
            for child in values {
                if contains(child, kind) {
                    return operation(child, kind);
                }
            }
        }
        _ => {}
    }
    panic!("missing {kind}: {value}")
}

fn contains(value: &Value, kind: &str) -> bool {
    match value {
        Value::Object(fields) => {
            fields.get("kind").and_then(Value::as_str) == Some(kind)
                || fields.values().any(|v| contains(v, kind))
        }
        Value::Array(values) => values.iter().any(|v| contains(v, kind)),
        _ => false,
    }
}

fn custom(rule: &str) -> Value {
    let source = ron::from_str(&format!("Card(Name:\"Random graveyard control\",Typeline:(Supertypes:[],Cardtypes:[Instant],Subtypes:[]),Rules:[{rule}])")).unwrap();
    serde_json::to_value(mtgish_ast::lower::card(&source).unwrap()).unwrap()
}

#[test]
fn random_returns_use_your_graveyard_and_keep_type_randomness_and_actual_receipt() {
    for (name, kind) in [
        ("Haunted Fengraf", "Creature"),
        ("Surreal Memoir", "Instant"),
    ] {
        let card = serde_json::to_value(fixtures::card(name)).unwrap();
        let choose = operation(&card, "ChooseObjects");
        assert_eq!(
            choose["objects"]["args"]["scope"],
            json!({"Players":{
                "zone":"Graveyard","players":{"kind":"One","args":{"kind":"AbilityController"}}
            }})
        );
        assert_eq!(
            choose["objects"]["args"]["filter"],
            json!({"kind":"CardType","args":kind})
        );
        assert_eq!(
            choose["count"],
            json!({"AsManyAsCan":{"kind":"Constant","args":1}})
        );
        assert_eq!(choose["constraints"], json!(["AtRandom"]));
        let movement = operation(&card, "Move");
        assert_eq!(movement["objects"], json!({"kind":"Selected","args":0}));
        assert_eq!(movement["destination"], json!("OwnersHand"));
    }
}

#[test]
fn a_named_actor_randomly_returns_from_that_actors_graveyard() {
    let card = custom("SpellActions(Targeted([TargetPlayer(Opponent)],ActionList([PlayerAction(Ref_TargetPlayer,PutACardFromGraveyardIntoHandAtRandom(IsCardtype(Creature)))])))");
    let target = json!({"kind":"Target","args":{"slot":0,"index":0}});
    let choose = operation(&card, "ChooseObjects");
    assert_eq!(choose["player"], target);
    assert_eq!(
        choose["objects"]["args"]["scope"],
        json!({"Players":{
            "zone":"Graveyard","players":{"kind":"One","args":target}
        }})
    );
}

#[test]
fn unrestricted_return_recipe_keeps_all_graveyards() {
    let card =
        custom("SpellActions(ActionList([ReturnGraveyardCardsToHand(IsCardtype(Creature))]))");
    let movement = operation(&card, "Move");
    assert_eq!(
        movement["objects"]["args"]["scope"],
        json!({"Players":{
            "zone":"Graveyard","players":{"kind":"All"}
        }})
    );
}
