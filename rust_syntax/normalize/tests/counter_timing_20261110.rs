#[path = "support/fresh_producer.rs"]
mod fresh_producer;
use mtgish_ast::{canonical, corpus, ir::*, mtg_types::OracleCard, unified, validate};
use serde_json::Value;

fn cards(text: &str) -> Vec<Card> {
    text.lines()
        .flat_map(|line| {
            let source: OracleCard = ron::from_str(line).unwrap();
            unified::lower(&source).card.faces
        })
        .collect()
}
fn normalized(abilities: &[Ability]) -> Value {
    validate::abilities(abilities).unwrap();
    let mut value = serde_json::to_value(abilities).unwrap();
    canonical::abilities(&mut value).unwrap();
    value
}
#[test]
fn six_complete_shared_verb_programs_match_independent_batch_proposals() {
    corpus::on_large_stack(|| {
        let expected: Value = serde_json::from_str(include_str!(
            "fixtures/counter_timing_20261110_expected.json"
        ))
        .unwrap();
        let cards = cards(include_str!("fixtures/counter_timing_20261110.ronl"));
        assert_eq!(cards.len(), 6);
        for card in cards {
            validate::card(&card).unwrap_or_else(|e| panic!("{}: {e}", card.name));
            let want: Vec<Ability> = serde_json::from_value(expected[&card.name].clone()).unwrap();
            fresh_producer::assert_named("counter_timing_20261110_expected.json", &card.name,
                &normalized(&want), &normalized(&card.abilities));
            let program = card
                .abilities
                .iter()
                .find_map(|a| match a {
                    Ability::Spell { body, .. }
                    | Ability::Activated { body, .. }
                    | Ability::Triggered { body, .. } => Some(body),
                    _ => None,
                })
                .unwrap();
            let [Instruction::Effect {
                operation: Operation::PutCounters { placements },
                ..
            }] = program.instructions.as_slice()
            else {
                panic!("{}: {program:?}", card.name)
            };
            assert_eq!(placements.len(), 2, "{}", card.name);
            assert!(placements
                .iter()
                .all(|p| matches!(p.amount, Expr::Constant(1)) && p.selection.is_none()));
        }
    });
}
#[test]
fn overlapping_recipients_preserve_two_contributions_inside_one_batch() {
    corpus::on_large_stack(|| {
        let mut card = cards(include_str!("fixtures/counter_timing_20261110.ronl"))
            .into_iter()
            .find(|c| c.name == "River Heralds' Boon")
            .unwrap();
        let Ability::Spell { body, .. } = &mut card.abilities[0] else {
            panic!()
        };
        let Instruction::Effect {
            operation: Operation::PutCounters { placements },
            ..
        } = &mut body.instructions[0]
        else {
            panic!()
        };
        placements[1].recipients = placements[0].recipients.clone();
        let value = normalized(&card.abilities);
        assert_eq!(
            value[0]["args"]["body"]["instructions"][0]["args"]["operation"]["args"]["placements"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    });
}
#[test]
fn explicit_sequencing_is_not_a_canonical_alias_for_one_batch() {
    let placement = CounterPlacement {
        recipients: Recipients::Objects(ObjectSet::One(ObjectRef::Source)),
        kind: mtgish_ast::counter::CounterKind::PowerToughness(1, 1),
        amount: Expr::Constant(1),
        selection: None,
    };
    let ability = Ability::Spell {
        announcement: Announcement {
            targets: vec![],
            modes: None,
        },
        body: Program {
            instructions: vec![
                Instruction::Effect {
                    result: ResultId(0),
                    operation: Operation::PutCounters {
                        placements: vec![placement.clone()],
                    },
                },
                Instruction::Effect {
                    result: ResultId(1),
                    operation: Operation::PutCounters {
                        placements: vec![placement],
                    },
                },
            ],
        },
    };
    assert_eq!(
        normalized(&[ability])[0]["args"]["body"]["instructions"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}
#[test]
fn existing_batch_and_then_controls_remain_native_valid() {
    corpus::on_large_stack(|| {
        let controls = cards(include_str!(
            "fixtures/counter_timing_controls_20261110.ronl"
        ));
        assert_eq!(controls.len(), 3);
        for card in controls {
            validate::card(&card).unwrap_or_else(|e| panic!("{}: {e}", card.name));
        }
    });
}
