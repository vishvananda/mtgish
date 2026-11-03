use super::{normalized, spell};
use crate::{canonical, ir::*, validate};

fn nested(outer: u32, inner: u32, predicate: u32) -> Ability {
    let draw = |binding, result| Instruction::Effect {
        result: ResultId(result),
        operation: Operation::Draw {
            players: PlayerSet::One(PlayerRef::Bound(Binding(binding))),
            amount: Expr::Constant(1),
        },
    };
    spell(vec![
        Instruction::ForEachPlayer {
            players: PlayerSet::Matching {
                binding: Binding(predicate),
                condition: Box::new(Condition::PlayerIsActive(PlayerRef::Bound(Binding(
                    predicate,
                )))),
            },
            binding: Binding(outer),
            starting_with: None,
            in_chosen_direction: false,
            body: Program {
                instructions: vec![Instruction::ForEachPlayer {
                    players: PlayerSet::OthersThan(PlayerRef::Bound(Binding(outer))),
                    binding: Binding(inner),
                    starting_with: Some(PlayerRef::Bound(Binding(outer))),
                    in_chosen_direction: false,
                    body: Program {
                        instructions: vec![draw(outer, 1), draw(inner, 2)],
                    },
                }],
            },
        },
        Instruction::ForEachObject {
            objects: ObjectSet::Battlefield(ObjectFilter::Any),
            binding: Binding(outer),
            body: Program {
                instructions: vec![Instruction::ForEachObject {
                    objects: ObjectSet::One(ObjectRef::Bound(Binding(outer))),
                    binding: Binding(inner),
                    body: Program {
                        instructions: vec![Instruction::Effect {
                            result: ResultId(3),
                            operation: Operation::Tap {
                                objects: ObjectSet::One(ObjectRef::Bound(Binding(inner))),
                                tapped: true,
                            },
                        }],
                    },
                }],
            },
        },
    ])
}

#[test]
fn predicate_and_loop_bindings_are_lexical_and_alpha_equivalent() {
    let original = nested(7, 19, 91);
    validate::validate_ability(&original).unwrap();
    let normalized = normalized(&original);
    validate::abilities(&normalized).unwrap();
    let mut value = serde_json::to_value(&normalized).unwrap();
    let instructions = &value[0]["args"]["body"]["instructions"];
    for index in [0, 1] {
        assert_eq!(instructions[index]["args"]["binding"], 0);
        assert_eq!(
            instructions[index]["args"]["body"]["instructions"][0]["args"]["binding"],
            1
        );
    }
    assert_eq!(instructions[0]["args"]["players"]["args"]["binding"], 0);
    let once = value.clone();
    canonical::abilities(&mut value).unwrap();
    assert_eq!(value, once);
    assert_eq!(
        value,
        serde_json::to_value(super::normalized(&nested(81, 2, 6))).unwrap()
    );
}

#[test]
fn reduced_bindings_preserve_lowering_ids() {
    let mut value = serde_json::to_value(vec![nested(7, 19, 91)]).unwrap();
    let original = value.clone();
    canonical::reduced(&mut value).unwrap();
    assert_eq!(value, original);
}
