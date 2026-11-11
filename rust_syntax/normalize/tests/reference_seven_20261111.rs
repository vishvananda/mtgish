use mtgish_ast::{canonical, corpus, ir::*, mtg_types::OracleCard, unified, validate};
use serde_json::{Map, Value};

fn normalized(abilities: &[Ability]) -> Value {
    validate::abilities(abilities).unwrap();
    let mut value = serde_json::to_value(abilities).unwrap();
    canonical::abilities(&mut value).unwrap();
    value
}

#[test]
fn chosen_player_definitions_are_cdas_and_curse_keeps_the_triggering_player() {
    corpus::on_large_stack(|| {
        let expected: Map<String, Value> = serde_json::from_str(include_str!(
            "fixtures/reference_seven_20261111_expected.json"
        ))
        .unwrap();
        let mut seen = 0;
        for line in include_str!("fixtures/reference_seven_20261111.ronl").lines() {
            let source: OracleCard = ron::from_str(line).unwrap();
            let lowered = unified::lower(&source);
            assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
            let card = &lowered.card.faces[0];
            validate::card(card).unwrap();
            let wanted: Vec<Ability> =
                serde_json::from_value(expected[&card.name].clone()).unwrap();
            assert_eq!(
                normalized(&card.abilities),
                normalized(&wanted),
                "{}",
                card.name
            );
            if card.name == "Trespasser's Curse" {
                let Ability::Triggered { event, body, .. } = &card.abilities[1] else {
                    panic!("missing Curse trigger")
                };
                assert!(matches!(
                    event,
                    EventSpec::Player {
                        event: PlayerEvent::EntersUnderControl,
                        ..
                    }
                ));
                assert!(matches!(
                    &body.instructions[0],
                    Instruction::Effect {
                        operation: Operation::LoseLife {
                            players: PlayerSet::One(PlayerRef::EventSubject),
                            ..
                        },
                        ..
                    }
                ));
                assert!(matches!(
                    &body.instructions[1],
                    Instruction::Effect {
                        operation: Operation::GainLife {
                            players: PlayerSet::One(PlayerRef::AbilityController),
                            ..
                        },
                        ..
                    }
                ));
            } else {
                assert!(card.abilities.iter().any(|ability| matches!(
                    ability,
                    Ability::Static {
                        kind: StaticKind::CharacteristicDefining { .. },
                        ..
                    }
                )));
                assert!(!card.abilities.iter().any(|ability| matches!(
                    ability,
                    Ability::Static {
                        kind: StaticKind::Ordinary,
                        ..
                    }
                )));
            }
            seen += 1;
        }
        assert_eq!(seen, 7);
    });
}

#[test]
fn changed_tagged_source_json_and_ron_describe_the_same_complete_records() {
    corpus::on_large_stack(|| {
        use std::io::BufRead;
        let mut source = std::collections::BTreeMap::new();
        for line in include_str!("fixtures/reference_seven_20261111.ronl").lines() {
            let card: OracleCard = ron::from_str(line).unwrap();
            let name = corpus::Identity::of(&card).unwrap().names.remove(0);
            source.insert(name, serde_json::to_value(card).unwrap());
        }
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/mtgish.lines.json");
        let mut checked = 0;
        for line in std::io::BufReader::new(std::fs::File::open(path).unwrap()).lines() {
            let line = line.unwrap();
            if !source
                .keys()
                .any(|name| line.contains(&format!("\"Name\":{name:?}")))
            {
                continue;
            }
            let tagged: Value = serde_json::from_str(&line).unwrap();
            let name = tagged["Name"].as_str().unwrap().to_owned();
            let value = untag_source(tagged);
            assert_eq!(value, source[&name], "{name}");
            let _: OracleCard = serde_json::from_value(value).unwrap();
            checked += 1;
        }
        assert_eq!(checked, 7);
    });
}

// The browser's legacy Source JSON uses _Type/args tags; the Rust default
// feature set uses external Serde tags. Compare both encodings structurally.
fn untag_source(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.into_iter().map(untag_source).collect()),
        Value::Object(mut fields) => {
            let tag = fields.keys().find(|key| key.starts_with('_')).cloned();
            if let Some(tag) = tag {
                let variant = fields.remove(&tag).unwrap().as_str().unwrap().to_owned();
                if fields.is_empty() {
                    return Value::String(variant);
                }
                let payload = if fields.len() == 1 && fields.contains_key("args") {
                    untag_source(fields.remove("args").unwrap())
                } else {
                    untag_source(Value::Object(fields))
                };
                Value::Object([(variant, payload)].into_iter().collect())
            } else {
                Value::Object(
                    fields
                        .into_iter()
                        .map(|(key, value)| (key, untag_source(value)))
                        .collect(),
                )
            }
        }
        other => other,
    }
}
