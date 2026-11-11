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

#[path = "support/source_json.rs"]
mod source_json;
#[test]
fn changed_tagged_source_json_and_ron_describe_the_same_complete_records() {
    corpus::on_large_stack(|| {
        source_json::assert_fixture_matches(include_str!("fixtures/reference_seven_20261111.ronl"))
    });
}
