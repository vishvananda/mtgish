use mtgish_ast::{corpus, ir::*, mtg_types::OracleCard, unified, validate};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const FIXTURE: &str = include_str!("fixtures/reference_remaining30_20261112.ronl");

fn text(name: &str) -> &'static str {
    FIXTURE
        .lines()
        .find(|l| l.contains(&format!("Name: {name:?},")))
        .unwrap()
}
fn lower(text: &str) -> UnifiedCard {
    let source: OracleCard = ron::from_str(text).unwrap();
    let lowered = unified::lower(&source);
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    validate::unified(&lowered.card).unwrap();
    unified::round_trip(&lowered.card).unwrap();
    lowered.card
}
fn cards() -> BTreeMap<String, Card> {
    FIXTURE
        .lines()
        .flat_map(|l| lower(l).faces)
        .map(|c| (c.name.clone(), c))
        .collect()
}
fn nodes<'a>(value: &'a Value, kind: &str, out: &mut Vec<&'a Value>) {
    if value["kind"] == kind {
        out.push(&value["args"]);
    }
    match value {
        Value::Object(fields) => fields.values().for_each(|v| nodes(v, kind, out)),
        Value::Array(values) => values.iter().for_each(|v| nodes(v, kind, out)),
        _ => {}
    }
}
fn found<'a>(value: &'a Value, kind: &str) -> Vec<&'a Value> {
    let mut out = vec![];
    nodes(value, kind, &mut out);
    out
}
fn abilities(card: &Card) -> Value {
    serde_json::to_value(&card.abilities).unwrap()
}

#[test]
fn remaining_packets_and_matching_controls_are_complete_typed_programs() {
    corpus::on_large_stack(|| {
        let cards = cards();
        assert_eq!(FIXTURE.lines().count(), 58);
        assert_eq!(cards.len(), 60); // Hit/Run and Norman Osborn/Green Goblin.
        if let Ok(dir) = std::env::var("MTGISH_REMAINING_REFERENCE_DUMP") {
            std::fs::create_dir_all(&dir).unwrap();
            for (name, card) in cards {
                let file =
                    std::path::Path::new(&dir).join(format!("{}.json", name.replace('/', "_")));
                std::fs::write(file, serde_json::to_vec_pretty(&card).unwrap()).unwrap();
            }
        }
    });
}

#[test]
fn singular_life_loss_quantities_name_the_same_player_as_the_operation() {
    corpus::on_large_stack(|| {
        let cards = cards();
        for name in [
            "Ebonblade Reaper",
            "Personal Incarnation",
            "Skybreen",
            "Liliana, Death Mage",
        ] {
            let value = abilities(&cards[name]);
            assert!(found(&value, "Acting").is_empty(), "{name}: {value}");
            assert!(found(&value, "Prior").is_empty(), "{name}: {value}");
            for loss in found(&value, "LoseLife") {
                for quantity in found(&loss["amount"], "PlayerNumber") {
                    assert_eq!(quantity["player"], loss["players"]["args"], "{name}");
                }
            }
        }
        let lily = abilities(&cards["Liliana, Death Mage"]);
        let loss = found(&lily, "LoseLife");
        let graveyard = found(&loss[1]["amount"], "CountObjects");
        assert!(graveyard[0].to_string().contains("Target"));
    });
}

#[test]
fn plural_life_loss_keeps_its_simultaneous_recipient_relative_quantity() {
    corpus::on_large_stack(|| {
        let value = abilities(&cards()["Scourge of the Skyclaves"]);
        let losses = found(&value, "LoseLife");
        assert_eq!(losses.len(), 1);
        assert_eq!(losses[0]["players"], json!({"kind":"All"}));
        assert_eq!(found(&losses[0]["amount"], "Acting").len(), 1);
        assert!(
            found(&value, "ForEachPlayer").is_empty(),
            "do not turn simultaneous life loss into sequential actions"
        );
    });
}

#[test]
fn an_optional_single_target_population_is_a_target_set() {
    corpus::on_large_stack(|| {
        let cards = cards();
        let Ability::Activated {
            announcement, body, ..
        } = &cards["Liliana, Death Mage"].abilities[0]
        else {
            panic!()
        };
        assert!(matches!(
            announcement.targets[0].count,
            Count::UpTo(Expr::Constant(1))
        ));
        let value = serde_json::to_value(body).unwrap();
        assert_eq!(found(&value, "Move")[0]["objects"]["kind"], "Targets");

        let mandatory = text("Liliana, Death Mage")
            .replace("UptoOneTargetGraveyardCard(", "TargetGraveyardCard(");
        let card = lower(&mandatory);
        let Ability::Activated { body, .. } = &card.faces[0].abilities[0] else {
            panic!()
        };
        let value = serde_json::to_value(body).unwrap();
        assert_eq!(found(&value, "Move")[0]["objects"]["kind"], "One");
    });
}

#[test]
fn cloak_and_one_card_impulse_permissions_use_explicit_after_results() {
    corpus::on_large_stack(|| {
        let cards = cards();
        let coat = abilities(&cards["Cryptic Coat"]);
        assert_eq!(
            found(&coat, "Attach")[0]["host"],
            json!({"kind":"Result","args":{"result":0,"phase":"After","index":0}})
        );
        let molly = abilities(&cards["Molly Hayes, Runaway"]);
        let effects = found(&molly, "CreateContinuousEffect");
        assert!(effects
            .iter()
            .any(|effect| !found(effect, "Result").is_empty()));
        assert!(found(&molly, "Transition").is_empty());
    });
}

#[test]
fn lure_and_equipment_share_the_same_blocking_requirement_direction() {
    corpus::on_large_stack(|| {
        let cards = cards();
        let static_ability = |card: &Card| {
            serde_json::to_value(
                card.abilities
                    .iter()
                    .find(|a| matches!(a, Ability::Static { .. }))
                    .unwrap(),
            )
            .unwrap()
        };
        assert_eq!(
            static_ability(&cards["Nemesis Mask"]),
            static_ability(&cards["Lure"])
        );
    });
}

#[test]
fn explicit_target_pronouns_survive_control_failure_and_delayed_capture() {
    corpus::on_large_stack(|| {
        let value = abilities(&cards()["In Thrall to the Pit"]);
        assert!(found(&value, "Recorded").is_empty());
        assert!(!found(&value, "Target").is_empty());
        assert!(!found(&value, "CreateDelayedTrigger").is_empty());
    });
}

#[test]
fn an_own_graveyard_payment_does_not_repeat_its_zone_filter() {
    corpus::on_large_stack(|| {
        let value = abilities(&cards()["Forbidden Crypt"]);
        let payments = found(&value, "PayCost");
        assert_eq!(payments.len(), 1);
        assert!(found(payments[0], "InZone").is_empty());
        let zones = found(payments[0], "Zone");
        assert_eq!(zones.len(), 1);
        assert_eq!(zones[0]["scope"]["Players"]["zone"], "Graveyard");
        assert_eq!(
            zones[0]["scope"]["Players"]["players"],
            json!({"kind":"One","args":{"kind":"AbilityController"}})
        );
        assert_eq!(payments[0]["optional"], false);
        assert!(!found(&value, "PaymentChosen").is_empty());

        let other = text("Forbidden Crypt").replace(
            "PutACardFromGraveyardIntoHand(InAPlayersGraveyard(SinglePlayer(You)))",
            "PutACardFromGraveyardIntoHand(InAPlayersGraveyard(SinglePlayer(TheActivePlayer)))",
        );
        let value = abilities(&lower(&other).faces[0]);
        let payments = found(&value, "PayCost");
        assert_eq!(
            found(payments[0], "InZone").len(),
            1,
            "a different graveyard scope is a real intersection"
        );
    });
}

#[test]
fn basic_plains_keeps_both_qualifications_without_an_extra_land_test() {
    corpus::on_large_stack(|| {
        let value = abilities(&cards()["Worzel, the Protector"]);
        assert!(found(&value, "Subtype")
            .iter()
            .any(|v| **v == json!("Plains")));
        assert!(found(&value, "Supertype")
            .iter()
            .any(|v| **v == json!("Basic")));
        assert!(!found(&value, "CardType")
            .iter()
            .any(|v| **v == json!("Land")));
        assert!(found(&value, "CardType")
            .iter()
            .any(|v| **v == json!("Planeswalker")));
    });
}

#[test]
fn lifted_conditions_keep_the_outer_object_and_inner_filter_candidates_separate() {
    corpus::on_large_stack(|| {
        let cards = cards();
        let regent = abilities(&cards["Deathbringer Regent"]);
        let ability = &regent[1]["args"];
        assert_eq!(ability["intervening_if"]["kind"], "AllOf");
        let condition = &ability["intervening_if"];
        assert!(found(condition, "Candidate").is_empty());
        let excluding = &found(condition, "Difference")[0]["excluding"];
        assert_eq!(
            excluding,
            &json!({"kind":"One","args":{"kind":"EventSubject"}})
        );
        assert_eq!(found(condition, "Cast").len(), 1);
        let thief = abilities(&cards["Dream Thief"]);
        let conditions = found(&thief, "If");
        assert_eq!(conditions[0]["condition"]["kind"], "PlayerDid");
        assert_eq!(found(&conditions[0]["condition"], "Not").len(), 1);
    });
}

#[test]
fn created_event_subjects_with_one_captured_target_use_that_target() {
    corpus::on_large_stack(|| {
        let cards = cards();
        for name in [
            "Delif's Cube",
            "Delif's Cone",
            "Niko Aris",
            "Barreling Attack",
            "Long River Lurker",
        ] {
            let value = abilities(&cards[name]);
            for trigger in found(&value, "CreateTrigger") {
                assert!(
                    found(&trigger["body"], "EventSubject").is_empty(),
                    "{name}: {trigger}"
                );
                if name=="Niko Aris" {
                    assert_eq!(found(&trigger["body"],"Targets"),vec![&json!(0)]);
                } else {
                    assert!(!found(&trigger["body"], "Target").is_empty(), "{name}");
                }
            }
        }
        let value = abilities(&cards["Long River Lurker"]);
        let cost_objects = found(&value, "CostObjects");
        assert_eq!(cost_objects.len(), 1);
        assert_eq!(cost_objects[0]["view"], "Performed");
        assert_eq!(cost_objects[0]["phase"], "After");
        assert_eq!(cost_objects[0]["component"], json!([0]));
        assert!(!found(&value, "PaymentChosen").is_empty());

        // Matching any creature does not prove that the event concerns the
        // captured target. Its independently established binding must survive.
        let source = text("Delif's Cone").replace(
            "WhenACreatureAttacksAndIsntBlocked(SinglePermanent(Ref_TargetPermanent))",
            "WhenACreatureAttacksAndIsntBlocked(IsCardtype(Creature))",
        );
        let value = abilities(&lower(&source).faces[0]);
        assert!(!found(&found(&value, "CreateTrigger")[0]["body"], "EventSubject").is_empty());
    });
}

#[test]
fn reflexive_payments_inherit_proven_self_objects_but_not_arbitrary_subjects() {
    corpus::on_large_stack(|| {
        let cards = cards();
        for name in [
            "Electro, Assaulting Battery",
            "Passenger Ferry",
            "Resplendent Marshal",
            "Sparktongue Dragon",
            "Cyclops Superconductor",
            "Ambergris, Agent of Balance",
            "Leyline Tyrant",
        ] {
            let value = abilities(&cards[name]);
            let triggers = found(&value, "ReflexiveTrigger");
            assert!(!triggers.is_empty(), "{name}");
            for trigger in triggers {
                assert!(
                    found(trigger, "EventSubject").is_empty(),
                    "{name}: {trigger}"
                );
                assert!(!found(trigger, "Source").is_empty(), "{name}");
            }
        }
        let source = text("Sparktongue Dragon").replace(
            "WhenAPermanentEntersTheBattlefield(SinglePermanent(ThisPermanent))",
            "WhenAPermanentEntersTheBattlefield(IsCardtype(Creature))",
        );
        let value = abilities(&lower(&source).faces[0]);
        assert!(!found(
            &found(&value, "ReflexiveTrigger")[0]["body"],
            "EventSubject"
        )
        .is_empty());
        // Keep the independently matched returning-object frame (Lagrella).
        assert!(lower(text("Lagrella, the Magpie")).faces.len() == 1);
    });
}

#[test]
fn counter_failure_does_not_erase_the_targeted_spells_mana_value() {
    corpus::on_large_stack(|| {
        let cards = cards();
        for name in ["Draining Whelk", "Spell Swindle", "Plasm Capture"] {
            let value = abilities(&cards[name]);
            assert!(found(&value, "Earlier").is_empty(), "{name}");
            let values = found(&value, "ObjectProperty");
            let mana = values
                .iter()
                .find(|v| v["property"] == "ManaValue")
                .unwrap();
            assert_eq!(mana["object"]["kind"], "Target", "{name}");
            assert_eq!(mana["information"], "CurrentOrLastKnown", "{name}");
        }
    });
}

#[test]
fn random_graveyard_return_prepares_chance_at_resolution_without_targets() {
    corpus::on_large_stack(|| {
        let value = abilities(&cards()["Haunted Fengraf"]);
        let prep = found(&value, "WithSelections");
        assert_eq!(prep.len(), 1);
        assert_eq!(prep[0]["steps"][0]["chooser"], "Chance");
        assert_eq!(
            prep[0]["steps"][0]["count"],
            json!({"AsManyAsCan":{"kind":"Constant","args":1}})
        );
        assert_eq!(
            prep[0]["steps"][0]["objects"]["args"]["scope"]["Players"]["zone"],
            "Graveyard"
        );
        assert!(found(&value, "ChooseObjects").is_empty());
        assert!(found(&value, "Target").is_empty());
    });
}

#[test]
fn mirrored_block_pairs_name_their_explicit_roles() {
    corpus::on_large_stack(|| {
        let value = abilities(&cards()["No Quarter"]);
        assert!(found(&value, "EventSubject").is_empty());
        let powers = found(&value, "ObjectProperty");
        assert_eq!(
            powers[0]["object"],
            json!({"kind":"BlockPair","args":"Attacker"})
        );
        assert_eq!(
            powers[1]["object"],
            json!({"kind":"BlockPair","args":"Blocker"})
        );
    });
}

#[test]
fn connive_names_its_object_and_that_objects_current_or_last_controller() {
    corpus::on_large_stack(|| {
        let value = abilities(&cards()["Norman Osborn"]);
        let connive = found(&value, "Keyword")
            .into_iter()
            .find(|k| k["action"].get("Connive").is_some())
            .unwrap();
        assert_eq!(
            connive["action"]["Connive"]["object"],
            json!({"kind":"Source"})
        );
        assert_eq!(
            connive["actor"],
            json!({"kind":"One","args":{"kind":"ControllerOf","args":{"kind":"Source"}}})
        );
        let value = abilities(&cards()["Vulshok Factory"]);
        for count in found(&value, "CountCounters") {
            assert_eq!(
                count["recipients"],
                json!({"kind":"Objects","args":{"kind":"One","args":{"kind":"Source"}}})
            );
            assert_eq!(count["information"], "CurrentOrLastKnown");
        }
    });
}

#[test]
fn optional_entry_sequences_share_payment_guards_and_keep_complete_token_results() {
    corpus::on_large_stack(|| {
        let cards = cards();
        for name in [
            "Dawn of the Dead",
            "Ilharg, the Raze-Boar",
            "Mirror Mockery",
            "Hellkite Courser",
            "Planebound Accomplice",
            "Through the Breach",
        ] {
            let value = abilities(&cards[name]);
            assert!(!found(&value, "PayCost").is_empty(), "{name}: {value}");
            assert!(!found(&value, "PaymentChosen").is_empty(), "{name}");
            for trigger in found(&value, "CreateDelayedTrigger") {
                assert!(
                    !found(&trigger["body"], "Result").is_empty()
                        || !found(&trigger["body"], "Transition").is_empty()
                        || !found(&trigger["body"], "CostObjects").is_empty()
                        || !found(&trigger["body"], "Created").is_empty(),
                    "{name}: {trigger}"
                );
            }
        }
        let mirror = abilities(&cards["Mirror Mockery"]);
        let delayed = found(&mirror, "CreateDelayedTrigger");
        let moved = found(&delayed[0]["body"], "Move");
        assert_eq!(
            moved[0]["objects"]["kind"], "Created",
            "token doublers may produce several objects"
        );
        assert!(moved[0]["objects"]["args"].is_number());

        // An optional program with independent work cannot acquire an entry
        // payment guard merely because its first instruction is an entry.
        let source=text("Hellkite Courser").replace(
            "CreatePermanentLayerEffect(ThePermanentPutOnTheBattlefieldThisWay, [AddAbility([Haste])])", "DrawACard");
        let value = abilities(&lower(&source).faces[0]);
        assert!(!found(&value, "Optional").is_empty());
        assert!(found(&value, "PayCost").is_empty());
    });
}

#[test]
fn kinship_controls_have_two_choices_and_an_explicit_eligibility_test() {
    corpus::on_large_stack(|| {
        let cards = cards();
        for name in ["Wandering Graybeard", "Kithkin Zephyrnaut"] {
            let value = abilities(&cards[name]);
            assert!(found(&value, "Optional").is_empty());
            let costs = found(&value, "PayCost");
            assert_eq!(costs.len(), 2, "{name}: {value}");
            assert!(costs.iter().all(|c| c["optional"] == true));
            assert_eq!(found(&value, "PaymentChosen").len(), 2);
            assert_eq!(found(&value, "Shares").len(), 1);
            assert_eq!(found(&value, "LookAt").len(), 1);
            assert_eq!(found(&value, "Reveal").len(), 1);
        }
    });
}

#[test]
fn compulsory_sacrifice_information_uses_input_before_in_both_damage_families() {
    corpus::on_large_stack(|| {
        let cards = cards();
        for name in ["Hit", "Rupture"] {
            let value = abilities(&cards[name]);
            let paid = found(&value, "PayCost");
            assert_eq!(paid.len(), 1);
            assert_eq!(paid[0]["optional"], false);
            assert_eq!(found(&value, "PaymentChosen").len(), 1);
            let receipts = found(&found(&value, "DealDamage")[0]["amount"], "CostObjects");
            assert_eq!(receipts.len(), 1, "{name}: {value}");
            assert_eq!(
                receipts[0]["view"], "Inputs",
                "a replacement does not erase what was paid"
            );
            assert_eq!(receipts[0]["phase"], "Before");
            assert_eq!(receipts[0]["component"], json!([0]));
            assert!(found(&value, "Transition").is_empty());
        }
        // Additional casting costs still use their own namespace.
        let fling = abilities(&cards["Fling"]);
        assert_eq!(
            found(&fling, "CostObjects")[0]["payment"],
            json!({"Casting":{"ability":0}})
        );
    });
}

#[test]
fn self_sacrifice_power_names_the_original_object_with_last_known_information() {
    corpus::on_large_stack(|| {
        let cards = cards();
        let value = abilities(&cards["Myriad Construct"]);
        let created = found(&value, "CreateTokens");
        assert_eq!(created.len(), 1);
        let power = found(&created[0]["tokens"][0]["amount"], "ObjectProperty");
        assert_eq!(power.len(), 1);
        assert_eq!(power[0]["object"], json!({"kind":"EventSubject"}));
        assert_eq!(power[0]["information"], "CurrentOrLastKnown");
        assert!(found(&created[0]["tokens"][0]["amount"], "Transition").is_empty());
    });
}

#[test]
fn duplicate_after_zone_change_uses_recorded_definition_including_perpetual_changes() {
    corpus::on_large_stack(|| {
        let cards = cards();
        for name in ["Veteran Ghoulcaller", "Kithkin Brinefarer"] {
            let value = abilities(&cards[name]);
            let conjured = found(&value, "Conjure");
            assert_eq!(conjured.len(), 1);
            assert_eq!(
                conjured[0]["card"],
                json!({"DuplicateLastKnown":{"kind":"EventSubject"}})
            );
        }
        let value = abilities(&cards["Veteran Ghoulcaller"]);
        assert_eq!(found(&value, "InZone").len(), 1);
        assert!(
            found(&value, "OwnedBy").is_empty(),
            "the event names the actual graveyard"
        );
    });
}

#[test]
fn one_static_ability_keeps_keyword_requirement_and_pump_in_printed_order() {
    corpus::on_large_stack(|| {
        let cards = cards();
        let value = abilities(&cards["Skeletal Swarming"]);
        let statics = found(&value, "Static");
        assert_eq!(statics.len(), 1);
        let changes = statics[0]["changes"].as_array().unwrap();
        assert_eq!(
            changes
                .iter()
                .map(|c| c["kind"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["GrantKeyword", "Rule", "Pump"]
        );
        assert_eq!(
            changes[1]["args"]["RequireCombat"]["action"],
            json!({"Attack":"Any"})
        );
    });
}

#[path = "support/source_json.rs"]
mod source_json;

#[test]
fn current_source_json_and_ron_match_the_complete_family_packet() {
    corpus::on_large_stack(|| {
        source_json::assert_fixture_matches(FIXTURE);
        source_json::assert_fixture_matches(include_str!(
            "fixtures/reference_remaining30_source_20261112.ronl"
        ));
    });
}
