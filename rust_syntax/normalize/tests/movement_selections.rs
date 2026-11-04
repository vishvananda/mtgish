#[path = "support/generated_locals.rs"]
mod generated_locals;
use mtgish_ast::{
    ir::*,
    movement,
    mtg_types::OracleCard,
    validate,
    visit::{Visitor, Walk},
};
fn you() -> PlayerSet {
    PlayerSet::One(PlayerRef::AbilityController)
}
fn field() -> ObjectSet {
    ObjectSet::battlefield(ObjectFilter::Any)
}
fn picked(objects: ObjectSet) -> ObjectSet {
    ObjectSet::Chosen {
        chooser: you(),
        objects: Box::new(objects),
        count: Count::Exactly(Expr::Constant(1)),
    }
}
fn effect(result: u32, operation: Operation) -> Instruction {
    Instruction::Effect {
        result: ResultId(result),
        operation,
    }
}
fn tap(objects: ObjectSet) -> Operation {
    Operation::Tap {
        objects,
        tapped: true,
    }
}
fn program(instructions: Vec<Instruction>) -> Program {
    Program { instructions }
}
fn card(body: Program) -> Card {
    let source:OracleCard=ron::from_str(r#"Card(Name: "Preparation", Typeline: (Supertypes: [], Cardtypes: [Sorcery], Subtypes: []), Rules: [SpellActions(ActionList([DrawACard]))])"#).unwrap();
    let mut card = mtgish_ast::lower::card(&source).unwrap();
    card.abilities = vec![Ability::Spell {
        announcement: Announcement::empty(),
        body,
    }];
    card
}
fn action(instruction: &Instruction) -> &Operation {
    let Instruction::Effect { operation, .. } = instruction else {
        panic!("{instruction:?}")
    };
    operation
}
#[test]
fn nested_choices_are_prepared_in_dependency_order_with_distinct_result_provenance() {
    let mut p = program(vec![effect(
        0,
        tap(picked(ObjectSet::Random {
            objects: Box::new(field()),
            count: Expr::Constant(3),
        })),
    )]);
    movement::normalize_program(&mut p);
    let Operation::WithSelections { steps, action } = action(&p.instructions[0]) else {
        panic!("{p:?}")
    };
    assert_eq!(steps.len(), 2);
    assert!(matches!(steps[0].chooser, SelectionChooser::Chance));
    assert!(matches!(
        steps[1].objects,
        ObjectSet::Prepared(SelectionId(0))
    ));
    assert!(matches!(
        **action,
        Operation::Tap {
            objects: ObjectSet::Prepared(SelectionId(1)),
            ..
        }
    ));
    assert!(matches!(
        p.instructions[0],
        Instruction::Effect {
            result: ResultId(0),
            ..
        }
    ));
    validate::card(&card(p.clone())).unwrap();
    let before = serde_json::to_value(&p).unwrap();
    movement::normalize_program(&mut p);
    assert_eq!(before, serde_json::to_value(&p).unwrap());
}
#[test]
fn preparation_stays_at_the_nearest_nested_operation_and_future_program() {
    let delayed = Operation::CreateDelayedTrigger {
        event: EventSpec::Turn {
            point: TurnPoint::EndStepBegins,
            players: PlayerSet::All,
        },
        start: DelayedStart::Immediately,
        expires: None,
        intervening_if: None,
        body: program(vec![effect(7, tap(picked(field())))]),
    };
    let mut p = program(vec![effect(
        0,
        Operation::InOrder(vec![
            Operation::Draw {
                players: you(),
                amount: Expr::Constant(1),
            },
            tap(picked(field())),
            delayed,
        ]),
    )]);
    movement::normalize_program(&mut p);
    let Operation::InOrder(ops) = action(&p.instructions[0]) else {
        panic!("{p:?}")
    };
    assert!(matches!(ops[0], Operation::Draw { .. }));
    assert!(matches!(ops[1], Operation::WithSelections { .. }));
    let Operation::CreateDelayedTrigger { body, .. } = &ops[2] else {
        panic!()
    };
    assert!(matches!(
        action(&body.instructions[0]),
        Operation::WithSelections { .. }
    ));
    validate::card(&card(p)).unwrap();
}
#[test]
fn costs_targets_and_filter_predicates_do_not_become_resolution_picks() {
    let random = ObjectSet::Random {
        objects: Box::new(field()),
        count: Expr::Constant(2),
    };
    let mut p = program(vec![
        effect(
            1,
            Operation::PayCost {
                player: PlayerRef::AbilityController,
                cost: Cost {
                    parts: vec![CostPart::Sacrifice {
                        objects: random.clone(),
                        choose: Some(Count::Exactly(Expr::Constant(2))),
                    }],
                },
                optional: false,
            },
        ),
        effect(
            2,
            tap(ObjectSet::battlefield(ObjectFilter::InSet(Box::new(
                random,
            )))),
        ),
    ]);
    let before = serde_json::to_value(&p).unwrap();
    movement::normalize_program(&mut p);
    assert_eq!(before, serde_json::to_value(&p).unwrap());
}
#[test]
fn local_selection_bindings_cannot_escape_or_forward_reference_and_duplicates_reject() {
    let step = SelectionStep {
        binding: SelectionId(0),
        chooser: SelectionChooser::Players(you()),
        objects: field(),
        count: Count::Exactly(Expr::Constant(1)),
        constraints: vec![],
    };
    let with = |steps| Operation::WithSelections {
        steps,
        action: Box::new(tap(ObjectSet::Prepared(SelectionId(0)))),
    };
    validate::card(&card(program(vec![effect(0, with(vec![step.clone()]))]))).unwrap();
    let bad = card(program(vec![
        effect(0, with(vec![step.clone()])),
        effect(1, tap(ObjectSet::Prepared(SelectionId(0)))),
    ]));
    assert!(validate::card(&bad)
        .unwrap_err()
        .contains("outside its preparation scope"));
    let mut forward = step.clone();
    forward.objects = ObjectSet::Prepared(SelectionId(0));
    assert!(validate::card(&card(program(vec![effect(0, with(vec![forward]))]))).is_err());
    assert!(validate::card(&card(program(vec![effect(
        0,
        with(vec![step.clone(), step])
    )])))
    .unwrap_err()
    .contains("duplicate local selection"));
}
#[test]
fn prepared_pool_zone_guards_survive_scope_and_codec_roundtrips() {
    let operation = Operation::WithSelections {
        steps: vec![SelectionStep {
            binding: SelectionId(5),
            chooser: SelectionChooser::Chance,
            objects: ObjectSet::Zone {
                scope: ZoneScope::Players {
                    players: you(),
                    zone: EventZone::Graveyard,
                },
                kind: ObjectKind::Card,
                filter: ObjectFilter::Any,
            },
            count: Count::AsManyAsCan(Expr::Constant(1)),
            constraints: vec![],
        }],
        action: Box::new(tap(ObjectSet::Prepared(SelectionId(5)))),
    };
    let c = card(program(vec![effect(0, operation)]));
    assert!(validate::card(&c).unwrap_err().contains("Battlefield"));
    let bytes = bincode::encode_to_vec(&c, bincode::config::standard()).unwrap();
    let (decoded, used): (Card, usize) =
        bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
    assert_eq!(used, bytes.len());
    assert_eq!(
        serde_json::to_value(c).unwrap(),
        serde_json::to_value(decoded).unwrap()
    );
}
#[test]
fn all_captured_producer_selectors_have_lexical_execution_sites_and_stable_normalization() {
    struct Counts {
        steps: usize,
        prepared: usize,
    }
    impl Visitor for Counts {
        fn operation(&mut self, op: &Operation) -> bool {
            if let Operation::WithSelections { steps, .. } = op {
                self.steps += steps.len();
            }
            true
        }
        fn object_set(&mut self, objects: &ObjectSet) -> bool {
            if matches!(objects, ObjectSet::Prepared(_)) {
                self.prepared += 1;
            }
            true
        }
    }
    let mut counts = Counts {
        steps: 0,
        prepared: 0,
    };
    for line in include_str!("fixtures/movement_selections.ronl").lines() {
        let source: OracleCard = ron::from_str(line).unwrap();
        let mut lowered = mtgish_ast::unified::lower(&source);
        assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
        lowered.card.walk(&mut counts);
        let before = serde_json::to_value(&lowered.card).unwrap();
        movement::normalize_unified(&mut lowered.card);
        assert_eq!(before, serde_json::to_value(&lowered.card).unwrap());
    }
    assert_eq!(counts.steps, 41);
    assert_eq!(counts.prepared, 41);
}
#[cfg(feature = "cantrip")]
#[test]
fn generated_choices_stay_inside_repetitions_and_after_prior_sibling_effects() {
    let body = program(vec![
        effect(
            0,
            Operation::Draw {
                players: you(),
                amount: Expr::Constant(1),
            },
        ),
        Instruction::Times {
            amount: Expr::Constant(2),
            body: program(vec![effect(1, tap(picked(field())))]),
        },
    ]);
    let c = movement::normalize_card(card(body));
    validate::card(&c).unwrap();
    let code = mtgish_ast::cantrip::module(&c).unwrap();
    syn::parse_file(&code).unwrap();
    assert!(
        code.find("draw(g")
            .unwrap_or_else(|| code.find("draw(").unwrap())
            < code.find("selected_").unwrap(),
        "{code}"
    );
    let choice = code.find("cantrip_mechanics::choose::select").unwrap();
    let repeat = code[..choice].rfind("for ").unwrap();
    assert!(repeat < choice, "{code}");
    assert!(!code.contains("1000000"));
}
#[test]
fn complete_captured_old_records_migrate_only_by_lexical_selection_elaboration_and_schema() {
    let old = include_str!("fixtures/movement_selections_before.ronl").lines();
    let source = include_str!("fixtures/movement_selections.ronl").lines();
    for (before, line) in old.zip(source) {
        let mut expected: UnifiedCard = ron::from_str(before).unwrap();
        expected.schema_version = SCHEMA_VERSION;
        for f in &mut expected.faces {
            f.schema_version = SCHEMA_VERSION;
        }
        movement::normalize_unified(&mut expected);
        let source: OracleCard = ron::from_str(line).unwrap();
        let actual = mtgish_ast::unified::lower(&source);
        assert!(actual.diagnostics.is_empty());
        assert_eq!(
            serde_json::to_value(expected).unwrap(),
            serde_json::to_value(actual.card).unwrap()
        );
    }
}
#[test]
fn granted_ability_local_choices_stay_inside_their_independent_program() {
    let grant = Operation::CreateContinuousEffect {
        source: ObjectRef::Source,
        affected: EffectSubjects::Objects(ObjectSet::One(ObjectRef::Source)),
        changes: vec![Change::GrantAbility(Box::new(Ability::Spell {
            announcement: Announcement::empty(),
            body: program(vec![effect(0, tap(picked(field())))]),
        }))],
        duration: Duration::EndOfTurn,
        evaluation: EffectEvaluation::Snapshot,
    };
    let mut p = program(vec![effect(0, grant)]);
    movement::normalize_program(&mut p);
    let Operation::CreateContinuousEffect { changes, .. } = action(&p.instructions[0]) else {
        panic!("{p:?}")
    };
    let Change::GrantAbility(a) = &changes[0] else {
        panic!()
    };
    let Ability::Spell { body, .. } = a.as_ref() else {
        panic!()
    };
    assert!(matches!(
        action(&body.instructions[0]),
        Operation::WithSelections { .. }
    ));
}
#[cfg(feature = "cantrip")]
#[test]
fn per_player_choices_are_gathered_before_one_simultaneous_move() {
    let binding = Binding(0);
    let player = PlayerSet::One(PlayerRef::Bound(binding.clone()));
    let selected = ObjectSet::Chosen {
        chooser: player.clone(),
        objects: Box::new(ObjectSet::Zone {
            scope: ZoneScope::Players {
                players: player,
                zone: EventZone::Hand,
            },
            kind: ObjectKind::Card,
            filter: ObjectFilter::Any,
        }),
        count: Count::Exactly(Expr::Constant(1)),
    };
    let p = program(vec![Instruction::ForEachPlayer {
        players: PlayerSet::All,
        binding,
        starting_with: None,
        in_chosen_direction: false,
        body: program(vec![effect(
            0,
            Operation::Move {
                objects: selected,
                destination: Destination::Exile,
                entry: EntrySpec::default(),
                face_down: false,
            },
        )]),
    }]);
    let c = movement::normalize_card(card(p));
    validate::card(&c).unwrap();
    let code = mtgish_ast::cantrip::module(&c).unwrap();
    syn::parse_file(&code).unwrap();
    assert_eq!(code.matches("g.exile_all(").count(), 1, "{code}");
    let choice = code.find("choose::select").unwrap();
    let moved = code.find("g.exile_all(").unwrap();
    assert!(code[..choice].contains("for "), "{code}");
    assert!(choice < moved, "{code}");
    assert!(code[choice..moved].contains("gathered_"), "{code}");
}
#[test]
fn simultaneous_members_prepare_all_choices_before_mutation_without_aliasing_bindings() {
    let mut p = program(vec![effect(
        0,
        Operation::Together(vec![
            tap(picked(field())),
            Operation::Destroy {
                objects: picked(field()),
                regeneration: true,
            },
        ]),
    )]);
    movement::normalize_program(&mut p);
    let Operation::WithSelections { steps, action } = action(&p.instructions[0]) else {
        panic!("{p:?}")
    };
    assert_eq!(steps.len(), 2);
    assert_eq!((steps[0].binding.0, steps[1].binding.0), (0, 1));
    let Operation::Together(members) = action.as_ref() else {
        panic!()
    };
    assert!(matches!(
        members.as_slice(),
        [
            Operation::Tap {
                objects: ObjectSet::Prepared(SelectionId(0)),
                ..
            },
            Operation::Destroy {
                objects: ObjectSet::Prepared(SelectionId(1)),
                ..
            }
        ]
    ));
    validate::card(&card(p.clone())).unwrap();
    let before = serde_json::to_value(&p).unwrap();
    movement::normalize_program(&mut p);
    assert_eq!(before, serde_json::to_value(&p).unwrap());
}
#[cfg(feature = "cantrip")]
#[test]
fn reveal_then_random_discard_is_explicitly_ordered_and_samples_after_reveal() {
    let c = card(program(vec![effect(
        0,
        Operation::Together(vec![
            Operation::Reveal { objects: field() },
            Operation::DiscardObjects {
                objects: ObjectSet::Random {
                    objects: Box::new(field()),
                    count: Expr::Constant(1),
                },
            },
        ]),
    )]));
    let c = movement::normalize_card(c);
    let Ability::Spell { body, .. } = &c.abilities[0] else {
        panic!()
    };
    let Operation::InOrder(clauses) = action(&body.instructions[0]) else {
        panic!("{body:?}")
    };
    assert!(matches!(clauses[1], Operation::WithSelections { .. }));
    let code = mtgish_ast::cantrip::module(&c).unwrap();
    syn::parse_file(&code).unwrap();
    let reveal = code.find("g.reveal(").unwrap();
    let random = code.find("library::random_of").unwrap();
    let discard = code.find("g.discard").unwrap();
    assert!(reveal < random && random < discard, "{code}");
}
#[cfg(feature = "cantrip")]
#[test]
fn a_movement_result_keeps_pre_action_choice_and_new_identity_in_separate_views() {
    let c = movement::normalize_card(card(program(vec![
        effect(
            0,
            Operation::Move {
                objects: picked(field()),
                destination: Destination::Exile,
                entry: EntrySpec::default(),
                face_down: false,
            },
        ),
        effect(
            1,
            Operation::Draw {
                players: you(),
                amount: Expr::CountObjects(Box::new(ObjectSet::Earlier {
                    what: EarlierObjects::Selected,
                    by: None,
                })),
            },
        ),
        effect(
            2,
            Operation::Draw {
                players: you(),
                amount: Expr::CountObjects(Box::new(ObjectSet::Named(ResultId(0)))),
            },
        ),
    ])));
    validate::card(&c).unwrap();
    let code = mtgish_ast::cantrip::module(&c).unwrap();
    syn::parse_file(&code).unwrap();
    let draws: Vec<_> = code
        .lines()
        .filter(|line| line.contains("let amount_"))
        .collect();
    assert_eq!(draws.len(), 2, "{code}");
    assert!(
        draws[0].contains("selected_") && draws[1].contains("moved_"),
        "{code}"
    );
}
#[test]
fn test_shape_comparison_changes_only_scratch_counters_and_keeps_payload_slots() {
    assert!(generated_locals::contains(
        "(carried_4_0,) = (selected_3.clone(),);",
        "(carried_5_0,) = (selected_4.clone(),);"
    ));
    assert!(!generated_locals::contains(
        "carried_4_1.clone()",
        "carried_5_0.clone()"
    ));
    assert!(!generated_locals::contains("bound_4", "bound_3"));
}
#[cfg(feature = "cantrip")]
#[test]
fn player_gathering_uses_prepared_pool_ownership_and_retains_choice_views() {
    let make = |pool: ObjectSet| {
        let player = PlayerRef::Bound(Binding(0));
        let p = program(vec![
            Instruction::ForEachPlayer {
                players: PlayerSet::All,
                binding: Binding(0),
                starting_with: None,
                in_chosen_direction: false,
                body: program(vec![effect(
                    0,
                    Operation::Move {
                        objects: ObjectSet::Chosen {
                            chooser: PlayerSet::One(player.clone()),
                            objects: Box::new(pool),
                            count: Count::Exactly(Expr::Constant(1)),
                        },
                        destination: Destination::Battlefield(player),
                        entry: EntrySpec::default(),
                        face_down: false,
                    },
                )]),
            },
            effect(
                1,
                Operation::Draw {
                    players: you(),
                    amount: Expr::CountObjects(Box::new(ObjectSet::Earlier {
                        what: EarlierObjects::Selected,
                        by: None,
                    })),
                },
            ),
        ]);
        movement::normalize_card(card(p))
    };
    let own = make(ObjectSet::Zone {
        scope: ZoneScope::Players {
            players: PlayerSet::One(PlayerRef::Bound(Binding(0))),
            zone: EventZone::Hand,
        },
        kind: ObjectKind::Card,
        filter: ObjectFilter::Any,
    });
    validate::card(&own).unwrap();
    let code = mtgish_ast::cantrip::module(&own).unwrap();
    syn::parse_file(&code).unwrap();
    assert_eq!(
        code.matches("g.put_all_onto_battlefield_as(").count(),
        1,
        "{code}"
    );
    assert!(
        code.lines()
            .any(|line| line.contains("let amount_") && line.contains("implicit_")),
        "{code}"
    );
    let foreign = make(field());
    assert!(mtgish_ast::cantrip::gaps(&foreign).iter().any(|g| g
        .feature
        .contains("per-player move under each player's control")));
}
#[cfg(feature = "cantrip")]
#[test]
fn player_local_entry_values_remain_a_batching_gap_instead_of_leaking_the_binding() {
    let who = PlayerSet::One(PlayerRef::Bound(Binding(0)));
    let entry = EntrySpec {
        tapped: false,
        states: vec![],
        counters: vec![(
            mtgish_ast::counter::CounterKind::Named(
                mtgish_ast::counter::CounterName::ChargeCounter,
            ),
            Expr::CountObjects(Box::new(ObjectSet::Chosen {
                chooser: who.clone(),
                objects: Box::new(field()),
                count: Count::Exactly(Expr::Constant(1)),
            })),
        )],
    };
    let p = program(vec![Instruction::ForEachPlayer {
        players: PlayerSet::All,
        binding: Binding(0),
        starting_with: None,
        in_chosen_direction: false,
        body: program(vec![effect(
            0,
            Operation::Move {
                objects: ObjectSet::Zone {
                    scope: ZoneScope::Players {
                        players: who,
                        zone: EventZone::Hand,
                    },
                    kind: ObjectKind::Card,
                    filter: ObjectFilter::Any,
                },
                destination: Destination::OwnersBattlefield,
                entry,
                face_down: false,
            },
        )]),
    }]);
    let c = movement::normalize_card(card(p));
    validate::card(&c).unwrap();
    assert!(mtgish_ast::cantrip::gaps(&c)
        .iter()
        .any(|gap| gap.feature == "per-player entry or destination with a local selection"));
}
#[cfg(feature = "cantrip")]
#[test]
fn optional_chooser_exports_the_choice_view_with_the_action_result() {
    let chooser = PlayerRef::OneOf(Box::new(PlayerSet::OpponentsOf(
        PlayerRef::AbilityController,
    )));
    let objects = ObjectSet::Chosen {
        chooser: PlayerSet::One(chooser),
        objects: Box::new(field()),
        count: Count::Exactly(Expr::Constant(1)),
    };
    let c = movement::normalize_card(card(program(vec![
        effect(
            0,
            Operation::Move {
                objects,
                destination: Destination::Exile,
                entry: EntrySpec::default(),
                face_down: false,
            },
        ),
        effect(
            1,
            Operation::Draw {
                players: you(),
                amount: Expr::CountObjects(Box::new(ObjectSet::Earlier {
                    what: EarlierObjects::Selected,
                    by: None,
                })),
            },
        ),
    ])));
    validate::card(&c).unwrap();
    let code = mtgish_ast::cantrip::module(&c).unwrap();
    syn::parse_file(&code).unwrap();
    let line = code
        .lines()
        .find(|line| line.contains("let amount_"))
        .unwrap();
    assert!(line.contains("carried_"), "{code}");
}
#[cfg(feature = "cantrip")]
#[test]
fn deferred_programs_capture_prepared_values_and_implicit_choice_views() {
    let delayed = |objects| Operation::CreateDelayedTrigger {
        event: EventSpec::Turn {
            point: TurnPoint::EndStepBegins,
            players: PlayerSet::All,
        },
        start: DelayedStart::Immediately,
        expires: None,
        intervening_if: None,
        body: program(vec![effect(1, tap(objects))]),
    };
    for explicit in [false, true] {
        let p = if explicit {
            program(vec![effect(
                0,
                Operation::WithSelections {
                    steps: vec![SelectionStep {
                        binding: SelectionId(0),
                        chooser: SelectionChooser::Players(you()),
                        objects: field(),
                        count: Count::Exactly(Expr::Constant(1)),
                        constraints: vec![],
                    }],
                    action: Box::new(delayed(ObjectSet::Prepared(SelectionId(0)))),
                },
            )])
        } else {
            program(vec![
                effect(0, tap(picked(field()))),
                effect(
                    2,
                    delayed(ObjectSet::Earlier {
                        what: EarlierObjects::Selected,
                        by: None,
                    }),
                ),
            ])
        };
        let c = movement::normalize_card(card(p));
        validate::card(&c).unwrap();
        let code = mtgish_ast::cantrip::module(&c).unwrap();
        syn::parse_file(&code).unwrap();
        assert!(code.contains("about(q, item, 0)"), "{code}");
        let delayed_fn = code
            .lines()
            .find(|line| line.contains("fn delayed_") && line.contains("&mut dyn Game"))
            .unwrap();
        let start = code.find(delayed_fn).unwrap();
        let end = code[start..].find("Ok(())").map(|i| i + start).unwrap();
        assert!(
            !code[start..end].contains("selected_"),
            "maker local escaped: {code}"
        );
    }
}
#[cfg(feature = "cantrip")]
#[test]
fn player_walked_choices_export_an_aggregate_pre_action_view() {
    let who = PlayerRef::Bound(Binding(0));
    let chosen = ObjectSet::Chosen {
        chooser: PlayerSet::One(who.clone()),
        objects: Box::new(ObjectSet::Zone {
            scope: ZoneScope::Players {
                players: PlayerSet::One(who),
                zone: EventZone::Hand,
            },
            kind: ObjectKind::Card,
            filter: ObjectFilter::Any,
        }),
        count: Count::Exactly(Expr::Constant(1)),
    };
    let p = program(vec![
        Instruction::ForEachPlayer {
            players: PlayerSet::All,
            binding: Binding(0),
            starting_with: None,
            in_chosen_direction: false,
            body: program(vec![effect(
                0,
                Operation::LookAt {
                    player: PlayerRef::AbilityController,
                    objects: chosen,
                },
            )]),
        },
        effect(
            1,
            Operation::Draw {
                players: you(),
                amount: Expr::CountObjects(Box::new(ObjectSet::Earlier {
                    what: EarlierObjects::Selected,
                    by: None,
                })),
            },
        ),
    ]);
    let c = movement::normalize_card(card(p));
    validate::card(&c).unwrap();
    let code = mtgish_ast::cantrip::module(&c).unwrap();
    syn::parse_file(&code).unwrap();
    let amount = code
        .lines()
        .find(|line| line.contains("let amount_"))
        .unwrap();
    assert!(amount.contains("all_"), "{code}");
}
#[cfg(feature = "cantrip")]
#[test]
fn independent_abilities_reset_implicit_views_before_explicit_choices_reuse_result_ids() {
    let activated = |body| Ability::Activated {
        origin: ActivationOrigin::Ordinary,
        cost: Cost { parts: vec![] },
        limits: ActivationLimits::unrestricted(),
        announcement: Announcement::empty(),
        body,
    };
    let mut c = card(Program::empty());
    c.characteristics.card_types = vec![mtgish_ast::mtg_types::CardType::Artifact];
    c.abilities = vec![
        activated(program(vec![effect(0, tap(picked(field())))])),
        activated(program(vec![
            effect(
                0,
                Operation::ChooseObjects {
                    player: PlayerRef::AbilityController,
                    objects: field(),
                    count: Count::Exactly(Expr::Constant(1)),
                    constraints: vec![],
                },
            ),
            effect(
                1,
                Operation::Draw {
                    players: you(),
                    amount: Expr::CountObjects(Box::new(ObjectSet::Earlier {
                        what: EarlierObjects::Selected,
                        by: None,
                    })),
                },
            ),
        ])),
    ];
    let c = movement::normalize_card(c);
    validate::card(&c).unwrap();
    let code = mtgish_ast::cantrip::module(&c).unwrap();
    syn::parse_file(&code).unwrap();
    let second = &code[code.find("fn ability_1(").unwrap()..];
    let choice = second.find("let selected_").unwrap();
    let base = second[choice + 4..].split("_player").next().unwrap();
    let amount = second
        .lines()
        .find(|line| line.contains("let amount_"))
        .unwrap();
    assert!(amount.contains(&format!("{base}.clone()")), "{code}");
}
#[cfg(feature = "cantrip")]
#[test]
fn an_individual_players_implicit_choices_use_pre_action_rows() {
    let who = PlayerRef::Bound(Binding(0));
    let pool = ObjectSet::Zone {
        scope: ZoneScope::Players {
            players: PlayerSet::One(who.clone()),
            zone: EventZone::Hand,
        },
        kind: ObjectKind::Card,
        filter: ObjectFilter::Any,
    };
    let p = program(vec![
        Instruction::ForEachPlayer {
            players: PlayerSet::All,
            binding: Binding(0),
            starting_with: None,
            in_chosen_direction: false,
            body: program(vec![effect(
                0,
                Operation::Move {
                    objects: ObjectSet::Chosen {
                        chooser: PlayerSet::One(who),
                        objects: Box::new(pool),
                        count: Count::Exactly(Expr::Constant(1)),
                    },
                    destination: Destination::Exile,
                    entry: EntrySpec::default(),
                    face_down: false,
                },
            )]),
        },
        Instruction::ForEachPlayer {
            players: PlayerSet::All,
            binding: Binding(1),
            starting_with: None,
            in_chosen_direction: false,
            body: program(vec![effect(
                1,
                Operation::Draw {
                    players: PlayerSet::One(PlayerRef::Bound(Binding(1))),
                    amount: Expr::CountObjects(Box::new(ObjectSet::Earlier {
                        what: EarlierObjects::Selected,
                        by: Some(PlayerSet::One(PlayerRef::Bound(Binding(1)))),
                    })),
                },
            )]),
        },
    ]);
    let c = movement::normalize_card(card(p));
    validate::card(&c).unwrap();
    let code = mtgish_ast::cantrip::module(&c).unwrap();
    syn::parse_file(&code).unwrap();
    let amount = code
        .lines()
        .find(|line| line.contains("let amount_"))
        .unwrap();
    assert!(amount.contains("implicit_each_"), "{code}");
}
