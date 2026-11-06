#[path = "support/reference_followup.rs"]
mod fixtures;
use fixtures::card;
use mtgish_ast::{corpus, lower, mtg_types as source};
use serde_json::{json, Value};

fn operation<'a>(value: &'a Value, kind: &str) -> &'a Value {
    fn visit<'a>(value: &'a Value, kind: &str, found: &mut Vec<&'a Value>) {
        match value {
            Value::Object(fields) => {
                if fields.get("kind").and_then(Value::as_str) == Some(kind) {
                    found.push(&fields["args"]);
                }
                for child in fields.values() {
                    visit(child, kind, found);
                }
            }
            Value::Array(values) => {
                for child in values {
                    visit(child, kind, found);
                }
            }
            _ => {}
        }
    }
    let mut found = Vec::new();
    visit(value, kind, &mut found);
    assert_eq!(found.len(), 1, "expected one {kind}: {value}");
    found[0]
}

fn target() -> Value {
    json!({"kind":"Target","args":{"slot":0,"index":0}})
}

#[test]
fn arcane_artisan_gives_the_copy_to_its_target_player() {
    let c = serde_json::to_value(card("Arcane Artisan")).unwrap();
    let create = operation(&c, "CreateTokens");
    let draw = operation(&c, "Draw");
    let recipient = json!({"kind":"One","args":target()});
    assert_eq!(create["players"], recipient);
    assert_eq!(draw["players"], recipient);
}

#[test]
fn basilica_bell_haunt_discards_opponents_cards_but_gains_its_controllers_life() {
    let c = serde_json::to_value(card("Basilica Bell-Haunt")).unwrap();
    assert_eq!(
        operation(&c, "Discard")["players"],
        json!({"kind":"OpponentsOf","args":{"kind":"AbilityController"}})
    );
    assert_eq!(
        operation(&c, "GainLife")["players"],
        json!({"kind":"One","args":{"kind":"AbilityController"}})
    );
}

#[test]
fn crash_landing_damages_the_target_creature() {
    let c = serde_json::to_value(card("Crash Landing")).unwrap();
    let damage = operation(&c, "DealDamage");
    assert_eq!(damage["source"], json!({"kind":"Source"}));
    assert_eq!(
        damage["recipients"],
        json!({"kind":"Objects","args":{"kind":"One","args":target()}})
    );
}

#[test]
fn counterspell_damage_goes_to_the_target_spells_controller() {
    for name in ["Essence Backlash", "Ionize"] {
        let c = serde_json::to_value(card(name)).unwrap();
        let damage = operation(&c, "DealDamage");
        assert_eq!(damage["source"], json!({"kind":"Source"}));
        assert_eq!(
            damage["recipients"],
            json!({"kind":"Players","args":{"kind":"One","args":{"kind":"ControllerOf","args":target()}}}),
            "{name}"
        );
    }
}

#[test]
fn mindswipe_asks_the_target_spells_controller_to_pay_its_cast_x() {
    let c = serde_json::to_value(card("Mindswipe")).unwrap();
    let payment = operation(&c, "PayCost");
    assert_eq!(
        payment["player"],
        json!({"kind":"ControllerOf","args":target()})
    );
    assert_eq!(payment["cost"]["parts"], json!([{"Mana":["X"]}]));
    assert_eq!(
        operation(&c, "DealDamage")["amount"],
        json!({"kind":"AnnouncedX"})
    );
}

#[test]
fn gutter_grime_tokens_count_the_granter_for_both_characteristics() {
    let c = serde_json::to_value(card("Gutter Grime")).unwrap();
    let create = operation(&c, "CreateTokens");
    let change =
        &create["tokens"][0]["recipe"]["Defined"]["abilities"][0]["args"]["changes"][0]["args"];
    for characteristic in ["power", "toughness"] {
        assert_eq!(change[characteristic]["kind"], "CountCounters");
        assert_eq!(
            change[characteristic]["args"]["recipients"],
            json!({"kind":"Objects","args":{"kind":"One","args":{"kind":"Granter"}}})
        );
    }
}

#[test]
fn the_masters_opponent_chooses_but_the_ability_controller_creates() {
    let c = serde_json::to_value(card("The Master, Gallifrey's End")).unwrap();
    assert_eq!(
        operation(&c, "ChooseAction")["actor"],
        json!({"kind":"One","args":{"kind":"Chosen"}})
    );
    assert_eq!(
        operation(&c, "LoseLife")["players"],
        json!({"kind":"One","args":{"kind":"Chosen"}})
    );
    assert_eq!(
        operation(&c, "CreateTokens")["players"],
        json!({"kind":"One","args":{"kind":"AbilityController"}})
    );
}

#[test]
fn toralfs_hammer_is_returned_but_the_equipped_creature_deals_the_damage() {
    for name in ["Toralf's Hammer", "Razor Boomerang"] {
        let c = serde_json::to_value(card(name)).unwrap();
        assert_eq!(operation(&c, "DealDamage")["source"], json!({"kind":"Source"}), "{name}");
        assert_eq!(operation(&c, "Move")["objects"], json!({"kind":"One","args":{"kind":"Granter"}}), "{name}");
    }
}

#[test]
fn curse_of_the_werefox_keeps_the_original_creature_as_the_fighter() {
    let c = serde_json::to_value(card("Curse of the Werefox")).unwrap();
    let fight = operation(&c, "Keyword");
    assert_eq!(fight["action"], "Fight");
    assert_eq!(fight["object"], target());
    assert_eq!(
        fight["other"],
        json!({"kind":"Target","args":{"slot":1,"index":0}})
    );
}

fn custom(rule: &str) -> source::OracleCard {
    corpus::on_large_stack(|| {
        let fixture = include_str!("fixtures/reference_followup.ronl")
            .lines()
            .find(|line| line.contains("Name: \"Curse of the Werefox\""))
            .unwrap();
        let mut c: source::OracleCard = ron::from_str(fixture).unwrap();
        let source::OracleCard::Card { rules, .. } = &mut c else {
            panic!()
        };
        *rules = Some(vec![ron::from_str(rule).unwrap()]);
        c
    })
}

#[test]
fn an_attached_tokens_host_can_be_a_different_outer_target() {
    let c = custom("SpellActions(Targeted([TargetPermanent(IsCardtype(Creature)), TargetPermanent(IsCardtype(Creature))], ActionList([Reflexive_CreateTokens_WhenYouDo([MonsterRoleToken], [EntersAttachedToPermanent(Ref_TargetPermanent2)], Targeted([UptoOneTargetPermanent(IsCardtype(Creature))], ActionList([Fight(ThePermanentAttachedToThisWay, Ref_TargetPermanent)])))])))");
    let c = serde_json::to_value(lower::card(&c).unwrap()).unwrap();
    assert_eq!(
        operation(&c, "Keyword")["object"],
        json!({"kind":"Target","args":{"slot":1,"index":0}})
    );
    assert_eq!(
        operation(&c, "Keyword")["other"],
        json!({"kind":"Target","args":{"slot":2,"index":0}})
    );
}

#[test]
fn creating_a_token_without_a_host_does_not_bind_an_attachment_host() {
    let c = custom("SpellActions(Targeted([TargetPermanent(IsCardtype(Creature))], ActionList([CreateTokens([MonsterRoleToken], []), Fight(ThePermanentAttachedToThisWay, Ref_TargetPermanent)])))");
    assert!(lower::card(&c).is_err());
}

#[test]
fn an_outer_attachment_host_is_not_inherited_by_a_tokens_own_ability() {
    let c = custom("SpellActions(Targeted([TargetPermanent(IsCardtype(Creature))], ActionList([CreateTokens([MonsterRoleToken], [EntersAttachedToPermanent(Ref_TargetPermanent)]), CreateTokens([TokenWithPT(PT(1, 1), Colors([Green]), [], [Creature], SubtypesList([Saproling]), [Activated(TapPermanent(ThisPermanent), ActionList([Fight(ThePermanentAttachedToThisWay, ThisPermanent)]))])], [])])))");
    assert!(lower::card(&c).is_err());
}
