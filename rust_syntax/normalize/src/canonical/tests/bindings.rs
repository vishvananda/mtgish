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
            objects: ObjectSet::battlefield(ObjectFilter::Any),
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

#[test]
fn player_and_object_binding_domains_are_independent() {
    let make = |player, object| {
        spell(vec![Instruction::ForEachPlayer {
            players: PlayerSet::All,
            binding: Binding(player),
            starting_with: None,
            in_chosen_direction: false,
            body: Program {
                instructions: vec![Instruction::ForEachObject {
                    objects: ObjectSet::One(ObjectRef::Source),
                    binding: Binding(object),
                    body: Program {
                        instructions: vec![
                            Instruction::Effect {
                                result: ResultId(5),
                                operation: Operation::Draw {
                                    players: PlayerSet::One(PlayerRef::Bound(Binding(player))),
                                    amount: Expr::Constant(1),
                                },
                            },
                            Instruction::Effect {
                                result: ResultId(6),
                                operation: Operation::Reveal {
                                    objects: ObjectSet::One(ObjectRef::Bound(Binding(object))),
                                },
                            },
                        ],
                    },
                }],
            },
        }])
    };
    let a = make(0, 0);
    let b = make(7, 91);
    validate::validate_ability(&a).unwrap();
    validate::validate_ability(&b).unwrap();
    let a = normalized(&a);
    let b = normalized(&b);
    validate::abilities(&a).unwrap();
    validate::abilities(&b).unwrap();
    assert_eq!(
        serde_json::to_value(a).unwrap(),
        serde_json::to_value(b).unwrap()
    );
}

#[test]
fn object_binding_shadowing_preserves_inputs_and_restores_the_outer_label() {
    fn nested(depth: u32, label: u32, distinct: bool) -> Instruction {
        Instruction::ForEachObject {
            objects: if depth == 2 {
                ObjectSet::One(ObjectRef::Source)
            } else {
                ObjectSet::One(ObjectRef::Bound(Binding(if distinct {
                    label - 1
                } else {
                    label
                })))
            },
            binding: Binding(label),
            body: Program {
                instructions: if depth == 0 {
                    vec![Instruction::Effect {
                        result: ResultId(9),
                        operation: Operation::Reveal {
                            objects: ObjectSet::One(ObjectRef::Bound(Binding(label))),
                        },
                    }]
                } else {
                    vec![
                        nested(
                            depth - 1,
                            if distinct { label + 1 } else { label },
                            distinct,
                        ),
                        Instruction::Effect {
                            result: ResultId(depth),
                            operation: Operation::Reveal {
                                objects: ObjectSet::One(ObjectRef::Bound(Binding(label))),
                            },
                        },
                    ]
                },
            },
        }
    }
    let a = spell(vec![nested(2, 7, false)]);
    let b = spell(vec![nested(2, 30, true)]);
    validate::validate_ability(&a).unwrap();
    validate::validate_ability(&b).unwrap();
    let a = normalized(&a);
    let b = normalized(&b);
    validate::abilities(&a).unwrap();
    validate::abilities(&b).unwrap();
    assert_eq!(
        serde_json::to_value(a).unwrap(),
        serde_json::to_value(b).unwrap()
    );
}

#[test]
fn activation_modifiers_resolve_announced_targets_before_renumbering() {
    fn activated(slot: u16) -> Ability {
        serde_json::from_value(serde_json::json!({"kind":"Activated","args":{
            "origin":"Ordinary","cost":{"parts":[]},
            "limits":{"once":false,"once_each_turn":false,"sorcery":false,
                "condition":null,"activators":null,"keyword":null,"modifiers":[
                    {"kind":"ActivationRule","args":{"Cost":{"condition":null,"modifier":{"Reduce":{
                        "reductions":[{"color":null,"amount":{"kind":"CountCounters","args":{
                            "recipients":{"kind":"Objects","args":{"kind":"One","args":{"kind":"Target","args":{"slot":slot,"index":0}}}},
                            "kind":{"PowerToughness":[1,1]},"information":"CurrentOrLastKnown"}}}],"spill_into_generic":true
                    }}}}}
                ]},
            "announcement":{"targets":[{"slot":slot,"kind":{"Object":{"kind":"Permanent","zones":["Battlefield"]}},
                "count":{"Exactly":{"kind":"Constant","args":1}},"filter":{"kind":"Any"},"players":null,"distribute":null,"constraints":[],"chooser":null}],"modes":null},
            "body":{"instructions":[]}
        }})).unwrap()
    }
    let a = activated(17);
    validate::validate_ability(&a).unwrap();
    let normalized = normalized(&a);
    validate::abilities(&normalized).unwrap();
    let mut other = serde_json::to_value(vec![activated(91)]).unwrap();
    canonical::abilities(&mut other).unwrap();
    assert_eq!(serde_json::to_value(normalized).unwrap(), other);
}
