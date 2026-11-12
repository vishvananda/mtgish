use mtgish_ast::{corpus, ir::*, lower, mtg_types::OracleCard, unified, validate};
use std::collections::BTreeMap;
const SOURCE: &str = include_str!("fixtures/reference_followups_20261112.ronl");
fn cards() -> BTreeMap<String, Card> {
    SOURCE
        .lines()
        .flat_map(|line| {
            let source: OracleCard = ron::from_str(line).unwrap();
            let result = unified::lower(&source);
            assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
            validate::unified(&result.card).unwrap();
            unified::round_trip(&result.card).unwrap();
            if matches!(source, OracleCard::Card { .. }) {
                assert_eq!(
                    serde_json::to_value(lower::card(&source).unwrap().abilities).unwrap(),
                    serde_json::to_value(&result.card.faces[0].abilities).unwrap()
                );
            }
            result.card.faces
        })
        .map(|c| (c.name.clone(), c))
        .collect()
}
#[test]
fn complete_source_packet_validates_and_round_trips() {
    corpus::on_large_stack(|| {
        let cards = cards();
        assert!(cards.len() >= 74);
        if let Ok(dir) = std::env::var("MTGISH_FOLLOWUPS_DUMP") {
            std::fs::create_dir_all(&dir).unwrap();
            for (name, card) in cards {
                std::fs::write(
                    std::path::Path::new(&dir).join(format!("{}.json", name.replace('/', "_"))),
                    serde_json::to_vec_pretty(&card).unwrap(),
                )
                .unwrap();
            }
        }
    });
}

use serde_json::{json, Value};
fn found<'a>(value: &'a Value, kind: &str) -> Vec<&'a Value> {
    fn visit<'a>(v: &'a Value, kind: &str, out: &mut Vec<&'a Value>) {
        if v["kind"] == kind {
            out.push(&v["args"]);
        }
        match v {
            Value::Object(fields) => fields.values().for_each(|v| visit(v, kind, out)),
            Value::Array(values) => values.iter().for_each(|v| visit(v, kind, out)),
            _ => {}
        }
    }
    let mut out = vec![];
    visit(value, kind, &mut out);
    out
}
fn value(cards: &BTreeMap<String, Card>, name: &str) -> Value {
    serde_json::to_value(&cards[name].abilities).unwrap()
}

#[test]
fn a_later_hand_payment_owns_its_followups() {
    corpus::on_large_stack(|| {
        let c = cards();
        let v = value(&c, "Cauldron Dance");
        let payment = found(&v, "Effect")
            .into_iter()
            .find(|e| {
                e["operation"]["kind"] == "PayCost" && e["operation"]["args"]["optional"] == true
            })
            .unwrap();
        let result = payment["result"].clone();
        let guarded = found(&v, "If")
            .into_iter()
            .find(|i| i["condition"] == json!({"kind":"PaymentChosen","args":result}))
            .unwrap();
        let references = found(&guarded["then"], "CostObjects");
        assert_eq!(
            references.len(),
            2,
            "haste and delayed sacrifice must use the second creature"
        );
        for r in references {
            assert_eq!(
                r,
                &json!({"payment":{"Resolution":result},"component":[0],"view":"Performed","phase":"After"})
            );
        }
        assert!(
            found(&guarded["then"], "Result").is_empty(),
            "the earlier graveyard return cannot supply the second paragraph"
        );
        let haste = found(&v, "CreateContinuousEffect");
        assert_eq!(haste.len(), 2);
        assert_eq!(
            haste[0]["affected"]["Objects"]["args"]["kind"], "Result",
            "the first creature keeps its own receipt"
        );
    });
}

#[test]
fn seizure_damage_and_payment_name_the_enchanted_creatures_controller() {
    corpus::on_large_stack(|| {
        let c = cards();
        let v = value(&c, "Seizures");
        let payer = &found(&v, "PayCost")[0]["player"];
        assert_eq!(
            payer,
            &json!({"kind":"ControllerOf","args":{"kind":"EventSubject"}})
        );
        let damage = found(&v, "DealDamage")[0];
        assert_eq!(
            damage["recipients"],
            json!({"kind":"Players","args":{"kind":"One","args":payer}})
        );
        assert_eq!(
            damage["source"],
            json!({"kind":"Source"}),
            "the Aura still deals the damage"
        );
        let bellower = value(&c, "Woodland Bellower");
        let filter = &found(&bellower, "Search")[0]["choices"][0]["filter"];
        let parts = filter["args"].as_array().unwrap();
        assert!(parts.contains(&json!({"kind":"CardType","args":"Creature"})));
        assert!(parts.contains(&json!({"kind":"Color","args":"Green"})));
        assert!(
            parts.contains(&json!({"kind":"Not","args":{"kind":"Supertype","args":"Legendary"}}))
        );
        assert!(parts.contains(&json!({"kind":"Property","args":{"property":"ManaValue","test":{"kind":"Compare","args":{"comparison":"LessOrEqual","value":{"kind":"Constant","args":3}}}}})));
    });
}

#[test]
fn every_printed_each_player_vote_has_an_ordered_voting_loop() {
    corpus::on_large_stack(|| {
        let c = cards();
        for name in [
            "Lieutenants of the Guard",
            "Coercive Portal",
            "Council Guardian",
            "Messenger Jays",
            "Orchard Elemental",
            "Magister of Worth",
            "Tyrant's Choice",
            "Plea for Power",
            "Emissary Green",
            "Galadriel, Elven-Queen",
            "Tivit, Seller of Secrets",
        ] {
            let v = value(&c, name);
            let loops = found(&v, "ForEachPlayer");
            let voting = loops
                .iter()
                .filter(|l| !found(&l["body"], "Vote").is_empty())
                .collect::<Vec<_>>();
            assert!(!voting.is_empty(), "{name}");
            for l in voting {
                assert_eq!(l["players"], json!({"kind":"All"}), "{name}");
                assert_eq!(
                    l["starting_with"],
                    json!({"kind":"AbilityController"}),
                    "{name}"
                );
                for vote in found(&l["body"], "Vote") {
                    assert_eq!(
                        vote["actor"],
                        json!({"kind":"One","args":{"kind":"Bound","args":l["binding"]}}),
                        "{name}"
                    );
                }
            }
        }
        let v = value(&c, "Lieutenants of the Guard");
        assert_eq!(
            found(&v, "Vote")[0]["ballot"],
            json!({"Words":["Strength","Numbers"]})
        );
        assert!(found(&v, "Votes").contains(&&json!({"Word":"Strength"})));
        assert!(!v.to_string().contains("Feather"));
    });
}

#[test]
fn artifact_creature_animations_preserve_existing_card_types() {
    corpus::on_large_stack(|| {
        let c = cards();
        for name in [
            "Start Your Engines",
            "Armed and Armored",
            "The Thanos-Copter",
        ] {
            let v = value(&c, name);
            assert!(
                found(&v, "AddCardType").contains(&&json!("Artifact")),
                "{name}"
            );
            assert!(
                found(&v, "AddCardType").contains(&&json!("Creature")),
                "{name}"
            );
            assert!(
                found(&v, "SetCardTypes").is_empty(),
                "{name}: a land Vehicle must stay a land"
            );
        }
        for (name, card) in c {
            let v = serde_json::to_value(card.abilities).unwrap();
            for types in found(&v, "SetCardTypes") {
                assert_ne!(
                    types,
                    &json!(["Artifact", "Creature"]),
                    "{name}: CR 205.1b retains prior types"
                );
            }
        }
    });
}

#[test]
fn required_sacrifice_and_optional_hand_entry_guard_their_followups() {
    corpus::on_large_stack(|| {
        let c = cards();
        let v = value(&c, "Grab the Reins");
        assert!(
            found(&v, "Sacrifice").is_empty(),
            "the damage-mode sacrifice is a required payment"
        );
        let costs = found(&v, "CostObjects");
        assert!(!costs.is_empty());
        for r in costs {
            assert_eq!(r["view"], "Inputs");
            assert_eq!(r["phase"], "Before");
        }
        for name in [
            "Surprise Deployment",
            "Ilharg, the Raze-Boar",
            "Meek Attack",
            "Planebound Accomplice",
        ] {
            let v = value(&c, name);
            if name == "Surprise Deployment" {
                assert!(found(&v, "Optional").is_empty(), "one typed entry payment");
            }
            let guarded = found(&v, "If")
                .into_iter()
                .filter(|i| {
                    i["condition"]["kind"] == "PaymentChosen"
                        && !found(&i["then"], "CreateDelayedTrigger").is_empty()
                })
                .collect::<Vec<_>>();
            assert!(!guarded.is_empty(), "{name}");
            for guard in guarded {
                let receipts = found(&guard["then"], "CostObjects");
                assert!(!receipts.is_empty(), "{name}");
                for r in receipts {
                    assert_eq!(
                        r["payment"],
                        json!({"Resolution":guard["condition"]["args"]}),
                        "{name}"
                    );
                    assert_eq!(r["view"], "Performed");
                    assert_eq!(r["phase"], "After");
                }
            }
        }
        let v = value(&c, "Fling");
        assert!(
            found(&v, "CostObjects")
                .iter()
                .any(|r| r["payment"].get("Casting").is_some()),
            "casting and resolution costs keep separate namespaces"
        );
    });
}

#[test]
fn followups_use_the_printed_targets_without_extra_constraints_or_success_filters() {
    corpus::on_large_stack(|| {
        let c = cards();
        let v = value(&c, "Spirit Flare");
        assert_eq!(
            v[0]["args"]["announcement"]["targets"][1]["constraints"],
            json!([])
        );
        assert!(!found(
            &v[0]["args"]["announcement"]["targets"][1]["filter"],
            "SameObject"
        )
        .contains(&&json!({"kind":"Target","args":{"slot":0,"index":0}})));
        for name in ["Hideous Taskmaster", "In the Darkness Bind Them"] {
            let v = value(&c, name);
            let taps = found(&v, "SetTapped");
            assert!(!taps.is_empty(), "{name}");
            for tap in taps {
                assert_eq!(
                    tap["objects"],
                    json!({"kind":"Targets","args":0}),
                    "{name}: failed control changes still untap legal targets"
                );
            }
            assert!(found(&v, "Affected").is_empty(), "{name}");
        }
        let v = value(&c, "Uchuulon");
        assert!(!v.to_string().contains("CommonProperty"));
        assert!(found(&v, "CostObjects")
            .iter()
            .all(|r| r["view"] == "Performed" || r["view"] == "Inputs"));
    });
}

#[test]
fn spell_history_and_single_card_values_have_one_producer_form() {
    corpus::on_large_stack(|| {
        let c = cards();
        for name in [
            "Dream Thief",
            "Hotheaded Giant",
            "Soul Reap",
            "Patrician's Scorn",
            "Talara's Battalion",
            "Mage Duel",
        ] {
            let v = value(&c, name);
            assert!(found(&v, "PlayerDid").is_empty(), "{name}");
            assert!(!found(&v, "EventHistory").is_empty(), "{name}");
        }
        for name in [
            "Audacious Swap",
            "Bioplasm",
            "Dimensional Infiltrator",
            "Kianne, Dean of Substance",
            "Mind's Dilation",
            "Omen Machine",
            "Bruse Tarl, Roving Rancher",
            "Rundvelt Hordemaster",
            "Pep, Raucous Raider",
        ] {
            let v = value(&c, name);
            assert!(
                found(&v, "ObjectMatches")
                    .iter()
                    .any(|c| c["object"]["kind"] == "Result"
                        && c["object"]["args"]["phase"] == "After"),
                "{name}"
            );
        }
        let v = value(&c, "Daredevil, Fearless Fighter");
        assert!(found(&v, "Aggregate").is_empty());
        assert!(found(&v, "ObjectProperty")
            .iter()
            .any(|e| e["object"]["kind"] == "Result" && e["property"] == "ManaValue"));
        let v = value(&c, "Satya, Aetherflux Genius");
        assert!(
            found(&v, "Aggregate")
                .iter()
                .any(|e| e["objects"]["kind"] == "Created" && e["aggregation"] == "Sum"),
            "token doubling keeps the whole created batch"
        );
        assert!(found(&v, "Sacrifice")
            .iter()
            .any(|e| e["objects"]["kind"] == "Created"));
    });
}

#[test]
fn arithmetic_and_pure_prevention_use_stable_operand_order() {
    corpus::on_large_stack(|| {
        let c = cards();
        for name in [
            "Chalice of Life",
            "Doctor Strange, Surgeon",
            "Leyline of Hope",
            "Speaker of the Heavens",
            "Ayli, Eternal Pilgrim",
        ] {
            let v = value(&c, name);
            let sums = found(&v, "Add");
            assert!(
                sums.iter()
                    .any(|a| a[0]["kind"] == "StartingLife" && a[1]["kind"] == "Constant"),
                "{name}"
            );
        }
        for name in [
            "Elvish Scout",
            "Maze of Ith",
            "Cephalid Illusionist",
            "Energy Arc",
            "Soratami Cloud Chariot",
            "Statecraft",
        ] {
            let v = value(&c, name);
            let event = if name == "Statecraft" {
                &v[0]["args"]["event"]
            } else {
                &found(&v, "CreateReplacement")[0]["event"]
            };
            let branches = event["Either"].as_array().unwrap();
            assert!(
                branches[0]["Damage"]["recipients"].get("Objects").is_some(),
                "{name}"
            );
            assert_eq!(branches[1]["Damage"]["recipients"], "Any", "{name}");
        }
    });
}

#[test]
fn perpetual_death_changes_follow_the_graveyard_incarnation() {
    corpus::on_large_stack(|| {
        let c = cards();
        let expected = json!({"kind":"One","args":{"kind":"Query","args":{"objects":{"kind":"InZone","args":{"objects":{"kind":"One","args":{"kind":"EventDestination"}},"zone":"Graveyard"}},"index":0}}});
        for name in ["Forgeborn Phoenix", "Hook Horror", "Consumed by History"] {
            let v = value(&c, name);
            let changes = found(&v, "CreatePerpetualEffect");
            assert_eq!(changes.len(), 1, "{name}");
            assert_eq!(
                changes[0]["objects"], expected,
                "{name}: old battlefield object is not the dead card"
            );
            assert!(
                found(&v, "Term").is_empty(),
                "{name}: use typed perpetual changes"
            );
        }
        let v = value(&c, "Forgeborn Phoenix");
        let effect = found(&v, "CreatePerpetualEffect")[0];
        let moves = found(&effect["changes"], "Move");
        assert_eq!(moves.len(), 1);
        assert_eq!(
            moves[0]["objects"],
            json!({"kind":"One","args":{"kind":"Source"}}),
            "the granted graveyard ability has its own source frame"
        );
    });
}

fn source_card(text: &str) -> Value {
    let source: OracleCard = ron::from_str(text).unwrap();
    let result = unified::lower(&source);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    validate::unified(&result.card).unwrap();
    serde_json::to_value(&result.card.faces[0].abilities).unwrap()
}

#[test]
fn multiple_cards_and_a_meldable_permanent_do_not_become_one_card() {
    corpus::on_large_stack(|| {
        let source = SOURCE
            .lines()
            .find(|l| l.contains("Name: \"Bioplasm\""))
            .unwrap();
        for objects in [
            "TheTopNumberCardsOfLibrary(Integer(2))",
            "Permanent(ThisPermanent)",
        ] {
            let v = source_card(&source.replace("TheTopCardOfLibrary", objects));
            let condition = &found(&v, "If")[0]["condition"];
            assert_eq!(condition["kind"], "Compare", "{objects}");
            assert_eq!(found(condition, "CountObjects").len(), 1, "{objects}");
            assert!(
                found(condition, "Result").is_empty(),
                "a melded permanent can leave two cards"
            );
        }
    });
}

#[test]
fn a_new_ordinary_entry_supersedes_a_paid_entry_without_mixing_receipts() {
    corpus::on_large_stack(|| {
        let v = source_card(
            r#"Card(Name: "Sequential Entry Control", Typeline: (Supertypes: [], Cardtypes: [Sorcery], Subtypes: []), Rules: [SpellActions(ActionList([
            PutACardFromHandOnBattlefield(IsCardtype(Creature), [EntersNormally]),
            CreatePermanentLayerEffect(ThePermanentPutOnTheBattlefieldThisWay, [AddAbility([Haste])]),
            MayCost(PutACardFromHandOnBattlefield(IsCardtype(Creature), [EntersNormally])),
            If(CostWasPaid, [CreatePermanentLayerEffect(ThePermanentPutOnTheBattlefieldThisWay, [AddAbility([Flying])])]),
            PutACardFromHandOnBattlefield(IsCardtype(Creature), [EntersNormally]),
            CreatePermanentLayerEffect(ThePermanentPutOnTheBattlefieldThisWay, [AddAbility([Trample])])
        ]))])"#,
        );
        let effects = found(&v, "CreateContinuousEffect");
        assert_eq!(effects.len(), 3);
        assert!(found(&effects[0]["affected"], "CostObjects").is_empty());
        assert_eq!(found(&effects[1]["affected"], "CostObjects").len(), 1);
        assert!(found(&effects[2]["affected"], "CostObjects").is_empty());
        let first = found(&effects[0]["affected"], "Result");
        let last = found(&effects[2]["affected"], "Result");
        assert_eq!(first.len(), 1);
        assert_eq!(last.len(), 1);
        assert_ne!(first[0]["result"], last[0]["result"]);
    });
}

#[test]
fn declining_an_entry_preserves_the_earlier_creature() {
    corpus::on_large_stack(|| {
        let c = cards();
        let v = value(&c, "Evershrike");
        let earlier = found(&v, "Effect")
            .into_iter()
            .find(|e| {
                e["operation"]["kind"] == "Move"
                    && e["operation"]["args"]["destination"]
                        .get("Battlefield")
                        .is_some()
            })
            .unwrap();
        let exile = found(&v, "Move")
            .into_iter()
            .find(|m| m["destination"].get("Exile").is_some())
            .unwrap();
        assert_eq!(
            exile["objects"],
            json!({"kind":"One","args":{"kind":"Result","args":{"result":earlier["result"],"phase":"After","index":0}}}),
            "the unchosen Aura cannot replace Evershrike"
        );
        let v = source_card(
            r#"Card(Name: "Declined Entry Control", Typeline: (Supertypes: [], Cardtypes: [Sorcery], Subtypes: []), Rules: [SpellActions(ActionList([
            MayCost(PutACardFromHandOnBattlefield(IsCardtype(Creature), [EntersNormally])),
            MayCost(PutACardFromHandOnBattlefield(IsEnchantmentType(Aura), [EntersNormally])),
            Unless(CostWasPaid, [CreatePermanentLayerEffect(ThePermanentPutOnTheBattlefieldThisWay, [AddAbility([Haste])])])
        ]))])"#,
        );
        let effect = found(&v, "CreateContinuousEffect")[0];
        let receipt = found(&effect["affected"], "CostObjects");
        assert_eq!(receipt.len(), 1);
        assert_eq!(
            receipt[0]["payment"],
            json!({"Resolution":0}),
            "declining a later entry restores an earlier paid receipt too"
        );
    });
}

#[test]
fn a_single_card_has_the_same_form_after_top_bottom_target_and_choice_moves() {
    corpus::on_large_stack(|| {
        let c = cards();
        for name in [
            "Cellar Door",
            "Grenzo, Dungeon Warden",
            "Arcane Artisan",
            "Agatha's Soul Cauldron",
            "Vengeful Rebirth",
            "Scavenging Ooze",
            "Graveyard Shovel",
            "Armored Skyhunter",
        ] {
            let v = value(&c, name);
            assert!(
                found(&v, "ObjectMatches")
                    .iter()
                    .any(|c| c["object"]["kind"] == "Result"
                        && c["object"]["args"]["phase"] == "After"),
                "{name}"
            );
        }
        for name in ["Alaundo the Seer", "Reviving Vapors", "Torrent Sculptor"] {
            let v = value(&c, name);
            assert!(
                found(&v, "ObjectProperty")
                    .iter()
                    .any(|c| c["object"]["kind"] == "Result" && c["property"] == "ManaValue"),
                "{name}"
            );
        }
    });
}

#[test]
fn one_original_permanent_and_one_requested_mill_keep_their_distinct_proofs() {
    corpus::on_large_stack(|| {
        let c = cards();
        let v = value(&c, "Make Yourself Useful");
        assert!(found(&v, "Aggregate").is_empty());
        assert!(found(&v, "ObjectProperty")
            .iter()
            .any(|p| p["object"]["kind"] == "Result"
                && p["object"]["args"]["phase"] == "Before"
                && p["property"] == "Toughness"));
        for name in ["Heed the Mists", "Infernal Genesis", "Mindshrieker"] {
            let v = value(&c, name);
            assert!(
                found(&v, "Aggregate")
                    .iter()
                    .any(|p| p["objects"]["kind"] == "Transition"
                        && p["objects"]["args"]["phase"] == "After"),
                "{name}: Bruvac can increase the actual mill"
            );
        }
        for name in [
            "Dracoplasm",
            "Reign of the Pit",
            "Shadowgrange Archfiend",
            "Stitcher Geralf",
            "The Kenriths' Royal Funeral",
        ] {
            assert!(
                !found(&value(&c, name), "Aggregate").is_empty(),
                "{name}: a real population keeps its aggregate"
            );
        }
    });
}
