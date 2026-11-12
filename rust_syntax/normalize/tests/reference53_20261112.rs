use mtgish_ast::{canonical, corpus, mtg_types::OracleCard, unified, validate};
use serde_json::Value;
use std::collections::BTreeMap;
fn cards() -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for line in include_str!("fixtures/reference53_20261112.ronl").lines() {
        let source: OracleCard = ron::from_str(line).unwrap();
        let lowered = unified::lower(&source);
        assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
        for face in lowered.card.faces {
            validate::card(&face).unwrap_or_else(|e| panic!("{}: {e}", face.name));
            let mut abilities = serde_json::to_value(&face.abilities).unwrap();
            canonical::abilities(&mut abilities).unwrap();
            if let Ok(dir) = std::env::var("MTGISH_REFERENCE53_DIR") {
                let path = std::path::Path::new(&dir)
                    .join(format!("{}.json", face.name.replace('/', "_")));
                std::fs::write(path, serde_json::to_vec_pretty(&abilities).unwrap()).unwrap();
            }
            out.insert(face.name, abilities);
        }
    }
    out
}
fn nodes<'a>(value: &'a Value, kind: &str, out: &mut Vec<&'a Value>) {
    if value["kind"] == kind {
        out.push(&value["args"]);
    }
    match value {
        Value::Object(m) => {
            for v in m.values() {
                nodes(v, kind, out);
            }
        }
        Value::Array(a) => {
            for v in a {
                nodes(v, kind, out);
            }
        }
        _ => {}
    }
}
fn found<'a>(value: &'a Value, kind: &str) -> Vec<&'a Value> {
    let mut out = Vec::new();
    nodes(value, kind, &mut out);
    out
}
#[test]
fn join_forces_uses_every_players_total_in_each_players_followup() {
    corpus::on_large_stack(|| {
        let c = cards();
        for name in [
            "Collective Voyage",
            "Minds Aglow",
            "Shared Trauma",
            "Alliance of Arms",
        ] {
            assert_eq!(found(&c[name], "PerformedTotal").len(), 1, "{name}");
            assert!(
                found(&c[name], "Performed").is_empty(),
                "{name}: each player's own payment is not the shared total"
            );
        }
    });
}
#[test]
fn except_first_draw_means_the_first_draw_in_its_owners_draw_step_only() {
    corpus::on_large_stack(|| {
        let c = cards();
        for name in [
            "Leela, Sevateem Warrior",
            "Xyris, the Writhing Storm",
            "Orcish Bowmasters",
        ] {
            assert_eq!(found(&c[name], "FirstDrawInOwnDrawStep").len(), 1, "{name}");
            assert!(found(&c[name], "NthEachTurn").is_empty(), "{name}");
            assert!(
                !c[name].to_string().contains("\"Window\":\"DrawStep\""),
                "{name}: draws outside draw steps still trigger"
            );
        }
    });
}
#[test]
fn perpetual_returns_preserve_all_outputs_and_return_payments_keep_their_cost_receipt() {
    corpus::on_large_stack(|| {
        let c = cards();
        for name in [
            "Paths of Tuinvale",
            "Veko, Death's Doorkeeper",
            "Candela, Aegis of Adagia",
        ] {
            let operations = found(&c[name], "CreatePerpetualEffect");
            assert!(!operations.is_empty(), "{name}");
            for op in operations {
                assert_eq!(op["objects"]["kind"], "Transition", "{name}");
                assert_eq!(op["objects"]["args"]["phase"], "After", "{name}");
            }
        }
        let alora = found(&c["Alora, Cheerful Scout"], "CreatePerpetualEffect");
        assert_eq!(alora[0]["objects"]["kind"], "CostObjects");
        assert!(!found(&c["Alora, Cheerful Scout"], "PaymentChosen").is_empty());
        let veko = found(&c["Veko, Death's Doorkeeper"], "CreatePerpetualEffect");
        let types = found(veko[0], "CreatureTypes");
        assert_eq!(types[0]["mode"], "Set");
    });
}
#[path = "support/source_json.rs"]
mod source_json;
#[test]
fn current_tagged_and_ron_source_agree_for_the_packet() {
    corpus::on_large_stack(|| {
        source_json::assert_fixture_matches(include_str!(
            "fixtures/reference53_source_20261112.ronl"
        ))
    });
}

#[test]
fn complete_programs_change_only_the_reviewed_meanings_and_normal_forms() {
    corpus::on_large_stack(|| {
        let expected: BTreeMap<String, Value> =
            serde_json::from_str(include_str!("fixtures/reference53_20261112_expected.json"))
                .unwrap();
        let current = cards();
        assert_eq!(expected.len(), 53);
        for (name, wanted) in expected {
            assert_eq!(current[&name], wanted, "{name}");
        }
    });
}

#[test]
fn is_characteristic_applies_on_entry_and_card_tests_read_the_exiled_state() {
    corpus::on_large_stack(|| {
        let c = cards();
        for name in [
            "Portal to Phyrexia",
            "Ascent of the Worthy",
            "Ashiok, Nightmare Weaver",
            "Chainer, Dementia Master",
            "Grave Betrayal",
            "Liliana, Death's Majesty",
            "Lorcan, Warlock Collector",
            "Sorin, Vengeful Bloodlord",
        ] {
            let moves = found(&c[name], "Move");
            assert!(
                moves
                    .iter()
                    .any(|m| m["destination"]["Battlefield"]["arrival"]["states"]
                        .as_array()
                        .is_some_and(|states| states.iter().any(|s| s.get("Changes").is_some()))),
                "{name}"
            );
        }
        let event = &c["Omen Machine"][1]["args"]["body"]["instructions"][1]["args"]["condition"];
        let reads = found(event, "Transition");
        assert_eq!(reads.len(), 1);
        assert_eq!(reads[0]["phase"], "After");
    });
}

#[test]
fn scheme_damage_binds_each_recipient_and_self_return_names_its_own_cost() {
    corpus::on_large_stack(|| {
        let c = cards();
        let damage = &c["Know Naught but Fire"][0]["args"]["body"]["instructions"][0];
        assert_eq!(damage["kind"], "ForEachPlayer");
        let binding = &damage["args"]["binding"];
        for player in found(damage, "Bound") {
            assert_eq!(player, binding);
        }
        assert!(found(damage, "Acting").is_empty());
        let body = &c["Lamplight Phoenix"][1]["args"]["body"];
        let objects = found(body, "CostObjects");
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0]["component"], serde_json::json!([0]));
        assert_eq!(objects[0]["phase"], "After");
        assert_eq!(objects[0]["view"], "Performed");
        assert!(!found(body, "PaymentChosen").is_empty());
        // Evidence is its own keyword payment, not the returned card's receipt.
        assert!(body.to_string().contains("CollectEvidence"));
    });
}
