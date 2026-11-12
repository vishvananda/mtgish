use mtgish_ast::{corpus, ir::*, mtg_types::OracleCard, unified, validate};
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn cards() -> BTreeMap<String, Card> {
    let mut cards = BTreeMap::new();
    for line in include_str!("fixtures/reference_consistency_20261112.ronl").lines() {
        let source: OracleCard = ron::from_str(line).unwrap();
        let lowered = unified::lower(&source);
        validate::unified(&lowered.card).unwrap();
        unified::round_trip(&lowered.card).unwrap();
        for card in lowered.card.faces {
            cards.insert(card.name.clone(), card);
        }
    }
    cards
}
fn nodes<'a>(value: &'a Value, kind: &str, out: &mut Vec<&'a Value>) {
    if value["kind"] == kind {
        out.push(&value["args"]);
    }
    match value {
        Value::Object(m) => m.values().for_each(|v| nodes(v, kind, out)),
        Value::Array(a) => a.iter().for_each(|v| nodes(v, kind, out)),
        _ => {}
    }
}
fn found<'a>(value: &'a Value, kind: &str) -> Vec<&'a Value> {
    let mut out = vec![];
    nodes(value, kind, &mut out);
    out
}

#[path = "support/source_json.rs"]
mod source_json;
#[test]
fn all_changed_parser_records_match_the_tagged_source_corpus() {
    corpus::on_large_stack(|| {
        source_json::assert_fixture_matches(include_str!(
            "fixtures/reference_consistency_20261112.ronl"
        ));
    });
}

#[test]
fn opponent_sacrifice_has_only_the_printed_filter() {
    corpus::on_large_stack(|| {
        let c = cards();
        let Ability::Triggered {
            event:
                EventSpec::Player {
                    event: PlayerEvent::Sacrifices,
                    players,
                    objects: Some(objects),
                },
            ..
        } = c["Bloodroot Apothecary"].abilities.last().unwrap()
        else {
            panic!()
        };
        assert!(matches!(
            players,
            PlayerSet::OpponentsOf(PlayerRef::AbilityController)
        ));
        let filter = serde_json::to_value(objects).unwrap();
        assert!(found(&filter, "Not")
            .iter()
            .any(|f| **f == json!({"kind":"CardType","args":"Creature"})));
        assert!(!found(&filter, "Token").is_empty());
        assert!(
            found(&filter, "SameObject").is_empty(),
            "no unprinted source exclusion"
        );
    });
}

#[test]
fn other_in_the_haunt_body_excludes_source_on_both_event_branches() {
    corpus::on_large_stack(|| {
        let c = cards();
        let Ability::Triggered {
            event: EventSpec::AnyOf(events),
            body,
            ..
        } = &c["Graven Dominator"].abilities[2]
        else {
            panic!("{:?}", c["Graven Dominator"].abilities)
        };
        assert_eq!(events.len(), 2);
        let body = serde_json::to_value(body).unwrap();
        let exclusions = found(&body, "Not");
        assert!(exclusions.contains(&&json!({"kind":"SameObject","args":{"kind":"Source"}})));
        assert!(!exclusions.contains(&&json!({"kind":"SameObject","args":{"kind":"EventSubject"}})));
    });
}

#[test]
fn animation_reads_its_target_at_resolution_for_spell_and_activated_wording() {
    corpus::on_large_stack(|| {
        let c = cards();
        for name in [
            "Karn's Touch",
            "Karn, Silver Golem",
            "Sydri, Galvanic Genius",
            "Toymaker",
        ] {
            let abilities = serde_json::to_value(&c[name].abilities).unwrap();
            let effects = found(&abilities, "CreateContinuousEffect");
            let effect = effects
                .iter()
                .find(|effect| !found(effect, "SetBasePowerToughness").is_empty())
                .unwrap();
            assert_eq!(effect["evaluation"], "Snapshot", "{name}");
            let properties = found(effect, "ObjectProperty");
            assert_eq!(properties.len(), 2, "{name}");
            for property in properties {
                assert_eq!(
                    property["object"],
                    json!({"kind":"Target","args":{"slot":0,"index":0}}),
                    "{name}"
                );
                assert_eq!(property["property"], "ManaValue", "{name}");
                assert_eq!(property["information"], "CurrentOrLastKnown", "{name}");
            }
        }
    });
}

#[test]
fn self_enters_or_attacks_uses_source_independently_of_the_body_template() {
    corpus::on_large_stack(|| {
        let c = cards();
        for name in [
            "Korvold, Fae-Cursed King",
            "Sephiroth, Fabled SOLDIER",
            "Hei Bai, Spirit of Balance",
        ] {
            let ability = c[name]
                .abilities
                .iter()
                .find(|a| {
                    matches!(
                        a,
                        Ability::Triggered {
                            event: EventSpec::AnyOf(_),
                            ..
                        }
                    )
                })
                .unwrap();
            let Ability::Triggered {
                body, announcement, ..
            } = ability
            else {
                unreachable!()
            };
            let value = serde_json::to_value((announcement, body)).unwrap();
            assert!(!found(&value, "Source").is_empty(), "{name}: {value}");
            assert!(found(&value, "EventSubject").is_empty(), "{name}: {value}");
        }
    });
}
