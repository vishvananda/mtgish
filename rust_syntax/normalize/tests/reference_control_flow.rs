#[path = "support/reference_followup.rs"]
mod fixtures;

use serde_json::{json, Value};

fn card(name: &str) -> Value {
    serde_json::to_value(fixtures::card(name)).unwrap()
}

fn target(slot: u32) -> Value {
    json!({"kind":"Target","args":{"slot":slot,"index":0}})
}

#[test]
fn bridgeworks_requires_the_pumped_creature_and_makes_only_its_fight_partner_optional() {
    let c = card("Bridgeworks Battle");
    let spell = &c["abilities"][0]["args"];
    let targets = &spell["announcement"]["targets"];
    assert_eq!(
        targets[0]["count"],
        json!({"Exactly":{"kind":"Constant","args":1}})
    );
    assert_eq!(
        targets[1]["count"],
        json!({"UpTo":{"kind":"Constant","args":1}})
    );
    let instructions = &spell["body"]["instructions"];
    assert_eq!(
        instructions[0]["args"]["operation"]["args"]["affected"],
        json!({"Objects":{"kind":"One","args":target(0)}})
    );
    let fight = &instructions[1]["args"]["operation"]["args"];
    assert_eq!(fight["action"], json!("Fight"));
    assert_eq!(fight["object"], target(0));
    assert_eq!(fight["other"], target(1));
}

#[test]
fn resurgence_adds_both_the_combat_and_the_following_main_phase() {
    let c = card("Resurgence");
    let instructions = &c["abilities"][0]["args"]["body"]["instructions"];
    assert_eq!(
        instructions[1]["args"]["operation"]["args"]["action"],
        json!({"AdditionalPhase":"CombatAndMain"})
    );
    let buff = &instructions[0]["args"]["operation"]["args"];
    assert_eq!(buff["duration"], json!("EndOfTurn"));
    assert_eq!(
        buff["changes"],
        json!([
            {"kind":"GrantKeyword","args":{"kind":"FirstStrike"}},
            {"kind":"GrantKeyword","args":{"kind":"Vigilance"}}
        ])
    );
}

#[test]
fn titan_hunter_checks_that_no_creature_died_and_damages_the_end_step_player() {
    let c = card("Titan Hunter");
    let trigger = &c["abilities"][0]["args"];
    let condition = &trigger["intervening_if"];
    assert_eq!(condition["kind"], json!("Compare"));
    let compare = &condition["args"];
    assert_eq!(compare["comparison"], json!("Equal"));
    assert_eq!(compare["right"], json!({"kind":"Constant","args":0}));
    assert_eq!(
        compare["left"]["args"]["event"],
        json!({"ZoneChange":{
            "objects":{"kind":"CardType","args":"Creature"},"from":"Battlefield","to":"Graveyard"
        }})
    );
    let damage = &trigger["body"]["instructions"][0]["args"]["operation"]["args"];
    assert_eq!(damage["source"], json!({"kind":"Source"}));
    assert_eq!(
        damage["recipients"],
        json!({"kind":"Players","args":{"kind":"One","args":{"kind":"EventSubject"}}})
    );
    assert_eq!(damage["amount"], json!({"kind":"Constant","args":4}));
}

#[test]
fn satya_sacrifices_the_created_token_only_when_payment_is_declined() {
    let c = card("Satya, Aetherflux Genius");
    let instructions = &c["abilities"][2]["args"]["body"]["instructions"];
    let delayed = &instructions[2]["args"]["operation"]["args"]["body"]["instructions"];
    let payment = &delayed[0]["args"];
    assert_eq!(payment["operation"]["kind"], json!("PayCost"));
    assert_eq!(payment["operation"]["args"]["optional"], json!(true));
    assert_eq!(
        delayed[1]["args"]["condition"],
        json!({"kind":"Not","args":{
            "kind":"PaymentChosen","args":payment["result"]
        }})
    );
    let sacrifice = &delayed[1]["args"]["then"]["instructions"][0]["args"]["operation"];
    assert_eq!(sacrifice["kind"], json!("Sacrifice"));
    assert_eq!(
        sacrifice["args"]["objects"],
        json!({"kind":"Created","args":0})
    );
}
