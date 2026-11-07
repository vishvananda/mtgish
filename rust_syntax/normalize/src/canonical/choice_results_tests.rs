//! Synthetic branch receipts; no Oracle fixtures or reference aliases.
use crate::{canonical, ir::*, validate};
use serde_json::{json, Value};

fn effect(result: u32, operation: Operation) -> Instruction {
    Instruction::Effect {
        result: ResultId(result),
        operation,
    }
}
fn movement(result: u32, objects: ObjectSet) -> Instruction {
    effect(
        result,
        Operation::Move {
            objects,
            destination: Destination::Exile,
            entry: EntrySpec::default(),
            face_down: false,
            terms: MovementTerms::default(),
        },
    )
}
fn choice(options: Vec<Vec<Instruction>>) -> Instruction {
    Instruction::ChooseOne {
        decider: PlayerRef::AbilityController,
        options: options
            .into_iter()
            .map(|instructions| Program { instructions })
            .collect(),
        offered: None,
    }
}
fn join(result: u32, receipts: &[u32]) -> Instruction {
    effect(
        result,
        Operation::JoinResults {
            results: receipts.iter().copied().map(ResultId).collect(),
        },
    )
}
fn spell(instructions: Vec<Instruction>) -> Value {
    serde_json::to_value(vec![Ability::Spell {
        announcement: Announcement {
            targets: vec![],
            modes: None,
        },
        body: Program { instructions },
    }])
    .unwrap()
}
fn valid(value: &Value) -> Result<(), String> {
    validate::abilities(&serde_json::from_value::<Vec<Ability>>(value.clone()).unwrap())
}
fn norm(mut value: Value) -> Value {
    valid(&value).unwrap();
    canonical::abilities(&mut value).unwrap();
    valid(&value).unwrap();
    let mut twice = value.clone();
    canonical::abilities(&mut twice).unwrap();
    assert_eq!(value, twice);
    value
}
fn picking(pick: u32, moved: u32) -> Vec<Instruction> {
    vec![
        effect(
            pick,
            Operation::ChooseObjects {
                player: PlayerRef::AbilityController,
                objects: ObjectSet::One(ObjectRef::Source),
                count: Count::AsManyAsCan(Expr::Constant(1)),
                constraints: vec![],
            },
        ),
        movement(moved, ObjectSet::Selected(ResultId(pick))),
    ]
}
fn branch_pair(a: u32, b: u32, result: u32) -> Value {
    spell(vec![
        choice(vec![picking(a, a + 1), picking(b, b + 1)]),
        join(result, &[a + 1, b + 1]),
    ])
}
fn steps(value: &Value) -> &Value {
    &value[0]["args"]["body"]["instructions"]
}
fn move_ids(value: &Value) -> Vec<Value> {
    steps(value)[0]["args"]["options"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["instructions"][1]["args"]["result"].clone())
        .collect()
}

#[test]
fn branch_local_selections_and_join_receipts_are_alpha_equivalent_and_bound() {
    let value = norm(branch_pair(7, 9, 30));
    assert_eq!(value, norm(branch_pair(41, 61, 90)));
    assert_eq!(
        steps(&value)[1]["args"]["operation"]["args"]["results"],
        json!(move_ids(&value))
    );
    for option in steps(&value)[0]["args"]["options"].as_array().unwrap() {
        assert_eq!(
            option["instructions"][0]["args"]["result"],
            option["instructions"][1]["args"]["operation"]["args"]["objects"]["args"]
        );
    }
}

#[test]
fn sibling_local_labels_can_repeat_without_becoming_duplicate_definitions() {
    let first = spell(vec![
        choice(vec![picking(7, 8), picking(7, 10)]),
        join(20, &[8, 10]),
    ]);
    let renamed = spell(vec![
        choice(vec![picking(31, 32), picking(41, 42)]),
        join(60, &[32, 42]),
    ]);
    assert_eq!(norm(first), norm(renamed));
}

#[test]
fn repeated_branch_labels_expand_alternative_definitions_preserving_multiplicity() {
    let value = norm(spell(vec![
        choice(vec![picking(7, 8), picking(7, 8)]),
        join(20, &[8, 8]),
    ]));
    let moves = move_ids(&value);
    assert_ne!(moves[0], moves[1]);
    assert_eq!(
        steps(&value)[1]["args"]["operation"]["args"]["results"],
        json!([moves[0], moves[1], moves[0], moves[1]])
    );
}

#[test]
fn branch_outcomes_never_become_ordinary_or_sibling_operands() {
    let reveal = |receipt| {
        effect(
            30,
            Operation::Reveal {
                objects: ObjectSet::Transition {
                    result: ResultId(receipt),
                    phase: ObjectPhase::After,
                },
            },
        )
    };
    for mut value in [
        spell(vec![choice(vec![picking(7, 8), picking(9, 10)]), reveal(8)]),
        spell(vec![choice(vec![picking(7, 8), vec![reveal(8)]])]),
    ] {
        assert!(valid(&value).is_err());
        canonical::abilities(&mut value).unwrap();
        assert!(valid(&value).is_err());
    }
}

#[test]
fn later_enclosing_definition_shadows_an_earlier_branch_label() {
    let value = norm(spell(vec![
        choice(vec![picking(7, 8), picking(9, 10)]),
        movement(8, ObjectSet::One(ObjectRef::Source)),
        join(20, &[8, 10]),
    ]));
    let expected = json!([steps(&value)[1]["args"]["result"], move_ids(&value)[1]]);
    assert_eq!(
        steps(&value)[2]["args"]["operation"]["args"]["results"],
        expected
    );
}

#[test]
fn choices_keep_genuine_outer_captures() {
    let captured = |outer, a, b, result| {
        let gain = |id| {
            effect(
                id,
                Operation::GainLife {
                    players: PlayerSet::One(PlayerRef::AbilityController),
                    amount: Expr::BoundValue(ResultId(outer)),
                },
            )
        };
        spell(vec![
            effect(
                outer,
                Operation::BindValue {
                    value: Expr::Constant(3),
                },
            ),
            choice(vec![
                vec![gain(a), movement(a + 1, ObjectSet::One(ObjectRef::Source))],
                vec![gain(b), movement(b + 1, ObjectSet::One(ObjectRef::Source))],
            ]),
            join(result, &[a + 1, b + 1]),
        ])
    };
    assert_eq!(norm(captured(1, 7, 9, 30)), norm(captured(40, 50, 60, 80)));
}

#[test]
fn duplicate_definitions_within_one_option_remain_invalid() {
    let mut value = spell(vec![
        choice(vec![
            vec![
                movement(7, ObjectSet::One(ObjectRef::Source)),
                movement(7, ObjectSet::One(ObjectRef::Source)),
            ],
            picking(9, 10),
        ]),
        join(20, &[7, 10]),
    ]);
    assert!(valid(&value).is_err());
    canonical::abilities(&mut value).unwrap();
    assert!(valid(&value).unwrap_err().contains("duplicate result"));
}

#[test]
fn nested_choice_join_outcomes_are_exported_only_to_the_enclosing_join() {
    let value = norm(spell(vec![
        choice(vec![
            vec![
                choice(vec![
                    vec![movement(7, ObjectSet::One(ObjectRef::Source))],
                    vec![movement(9, ObjectSet::One(ObjectRef::Source))],
                ]),
                join(20, &[7, 9]),
            ],
            vec![movement(30, ObjectSet::One(ObjectRef::Source))],
        ]),
        join(40, &[20, 30]),
    ]));
    let options = &steps(&value)[0]["args"]["options"];
    let inner = &options[0]["instructions"];
    let inner_moves: Vec<_> = inner[0]["args"]["options"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["instructions"][0]["args"]["result"].clone())
        .collect();
    assert_eq!(
        inner[1]["args"]["operation"]["args"]["results"],
        json!(inner_moves)
    );
    assert_eq!(
        steps(&value)[1]["args"]["operation"]["args"]["results"],
        json!([
            inner[1]["args"]["result"],
            options[1]["instructions"][0]["args"]["result"]
        ])
    );
}

#[test]
fn nearest_granted_ability_branch_beats_an_enclosing_ordinary_label() {
    let inner = Ability::Activated {
        origin: ActivationOrigin::Ordinary,
        cost: Cost { parts: vec![] },
        limits: ActivationLimits::unrestricted(),
        announcement: Announcement {
            targets: vec![],
            modes: None,
        },
        body: Program {
            instructions: vec![
                choice(vec![
                    vec![movement(7, ObjectSet::One(ObjectRef::Source))],
                    vec![movement(9, ObjectSet::One(ObjectRef::Source))],
                ]),
                join(20, &[7, 9]),
            ],
        },
    };
    let value = norm(spell(vec![
        movement(7, ObjectSet::One(ObjectRef::Source)),
        effect(
            50,
            Operation::CreateContinuousEffect {
                source: ObjectRef::Source,
                affected: EffectSubjects::Objects(ObjectSet::One(ObjectRef::Source)),
                changes: vec![Change::GrantAbility(Box::new(inner))],
                duration: Duration::EndOfTurn,
                evaluation: EffectEvaluation::Snapshot,
            },
        ),
    ]));
    fn activated(value: &Value) -> Option<&Value> {
        if value["kind"] == "Activated" {
            return Some(value);
        }
        match value {
            Value::Object(fields) => fields.values().find_map(activated),
            Value::Array(values) => values.iter().find_map(activated),
            _ => None,
        }
    }
    let body = &activated(&value).unwrap()["args"]["body"]["instructions"];
    let moves: Vec<_> = body[0]["args"]["options"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["instructions"][0]["args"]["result"].clone())
        .collect();
    assert_eq!(
        body[1]["args"]["operation"]["args"]["results"],
        json!(moves)
    );
    assert_ne!(moves[0], steps(&value)[0]["args"]["result"]);
}

#[test]
fn quadratic_shared_label_expansion_is_refused_without_truncation() {
    let options = (0..300)
        .map(|_| vec![movement(7, ObjectSet::One(ObjectRef::Source))])
        .collect();
    let mut value = spell(vec![choice(options), join(20, &vec![7; 256])]);
    valid(&value).unwrap();
    let original = value.clone();
    assert!(canonical::abilities(&mut value)
        .unwrap_err()
        .contains("work limit"));
    assert_eq!(
        value, original,
        "preflight must refuse before mutating the input"
    );
}
