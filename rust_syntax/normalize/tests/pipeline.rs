use mtgish_ast::{
    corpus, inspect,
    ir::*,
    mtg_types as source,
    pipeline::{self, source_tree::SourceIndex, DurationInput, ExpandedRecipe, SurfaceAction},
    rewrite_spec::Registry,
    validate,
};
use std::{collections::BTreeSet, path::PathBuf};

fn sources() -> impl Iterator<Item = source::OracleCard> {
    include_str!("fixtures/pipeline.ronl")
        .lines()
        .map(|line| ron::from_str(line).unwrap())
}
fn source(name: &str) -> source::OracleCard {
    sources()
        .find(|s| {
            corpus::Identity::of(s)
                .unwrap()
                .names
                .contains(&name.into())
        })
        .unwrap()
}
fn custom(rule: &str) -> source::OracleCard {
    let mut s = source("Maro");
    let source::OracleCard::Card { rules, .. } = &mut s else {
        panic!()
    };
    *rules = Some(vec![ron::from_str(rule).unwrap()]);
    s
}
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .into()
}
fn scratch(label: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "mtgish-pipeline-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ))
}

#[test]
fn structural_pass_preserves_every_fixture_field_and_layout_through_rust_source_roundtrips() {
    let mut layouts = BTreeSet::new();
    for card in sources() {
        let index = SourceIndex::new(&card);
        layouts.insert(index.layout.clone());
        let expected = inspect::value(&card).unwrap();
        assert_eq!(inspect::value(&index.restore().unwrap()).unwrap(), expected);
        let loaded: SourceIndex =
            serde_json::from_value(serde_json::to_value(&index).unwrap()).unwrap();
        assert_eq!(
            inspect::value(&loaded.restore().unwrap()).unwrap(),
            expected
        );
        for (i, node) in loaded.nodes.iter().enumerate() {
            assert_eq!(node.id.0 as usize, i);
            assert_eq!(loaded.at(&node.id)["variant"], node.form);
            assert!(node.parent.as_ref().is_none_or(|p| p.0 < node.id.0));
        }
        for face in &loaded.faces {
            assert_eq!(
                loaded.original.pointer(&face.pointer).unwrap()["Name"],
                face.name
            );
        }
    }
    assert_eq!(layouts.len(), 18);
}

#[test]
fn unsupported_parents_do_not_hide_nested_actions_or_discard_source_operands() {
    let s=custom("SpellActions(ActionList([ActionForEachPlayerInTurnOrder(AnyPlayer,[PutPermanentIntoItsOwnersHand(ThisPermanent)])]))");
    let intermediate = pipeline::Intermediate::new(&s).unwrap();
    assert!(intermediate.final_card.is_none());
    assert_eq!(intermediate.structural.rewrites.len(), 1);
    assert!(matches!(
        intermediate.structural.rewrites[0].surface,
        SurfaceAction::Move { .. }
    ));
    assert!(intermediate
        .structural
        .source
        .nodes
        .iter()
        .any(|n| n.form == "ActionForEachPlayerInTurnOrder"));
    assert_eq!(
        inspect::value(&intermediate.structural.source.restore().unwrap()).unwrap(),
        inspect::value(&s).unwrap()
    );
    assert!(
        serde_json::from_value::<Card>(serde_json::to_value(intermediate).unwrap()).is_err(),
        "intermediate data is not a semantic Card export"
    );
}

#[test]
fn structural_rewriting_does_not_require_duration_or_reference_resolution() {
    let s=custom("SpellActions(Targeted([TargetPermanent(AnyPermanent)],ActionList([GainControlOfPermanentUntil(Ref_TargetPermanent,UntilItIsNoLongerEnchanted)])))");
    let value = pipeline::Intermediate::new(&s).unwrap();
    assert!(value.final_card.is_none());
    assert!(matches!(
        &value.structural.rewrites[0].expanded,
        ExpandedRecipe::ContinuousEffect {
            duration: DurationInput::Source(source::Expiration::UntilItIsNoLongerEnchanted),
            evaluation: EffectEvaluation::Snapshot,
            ..
        }
    ));
    let s = custom("SpellActions(ActionList([GainControlOfPermanent(Ref_TargetPermanent)]))");
    let value = pipeline::Intermediate::new(&s).unwrap();
    assert_eq!(value.structural.rewrites.len(), 1);
    assert!(value
        .diagnostics
        .iter()
        .any(|d| d.message.contains("target slot")));
}

#[test]
fn binding_reuses_target_domains_actor_scopes_and_result_ids() {
    let s=custom("SpellActions(Targeted([TargetPermanent(AnyPermanent),TargetPlayer(AnyPlayer)],ActionList([PlayerAction(Ref_TargetPlayer,GainControlOfPermanentUntil(Ref_TargetPermanent,UntilEndOfTurn))])))");
    let c = pipeline::finalize(&s).unwrap();
    let Ability::Spell { body, .. } = &c.abilities[0] else {
        panic!()
    };
    assert!(
        matches!(&body.instructions[0],Instruction::Effect {operation:Operation::CreateContinuousEffect {affected:EffectSubjects::Objects(ObjectSet::One(ObjectRef::Target {slot:Slot(0),..})),changes,duration:Duration::EndOfTurn,evaluation:EffectEvaluation::Snapshot,..},..} if matches!(changes.as_slice(),[Change::SetController(PlayerRef::Target {slot:Slot(1),..})]))
    );
    let s = custom(
        "SpellActions(ActionList([GainControlOfAPermanentUntil(AnyPermanent,UntilEndOfTurn)]))",
    );
    let c = pipeline::finalize(&s).unwrap();
    let Ability::Spell { body, .. } = &c.abilities[0] else {
        panic!()
    };
    let Instruction::Group(body) = &body.instructions[0] else {
        panic!()
    };
    assert!(
        matches!(body.instructions.as_slice(),[Instruction::Effect {result,operation:Operation::ChooseObjects {count:Count::AsManyAsCan(Expr::Constant(1)),..}},Instruction::Effect {operation:Operation::CreateContinuousEffect {affected:EffectSubjects::Objects(ObjectSet::Selected(selected)),..},..}] if result.0==selected.0)
    );
}

#[test]
fn move_expansion_preserves_exile_incarnations_entry_flags_and_followup_results() {
    let s=custom("SpellActions(Targeted([TargetPermanent(AnyPermanent)],ActionList([Exile([Permanent(Ref_TargetPermanent)],[]),PutExiledCardOntoBattlefield(TheCardExiledThisWay,[EntersUnderOwnersControl,EntersTapped]),TapPermanent(ThePermanentPutOnTheBattlefieldThisWay)])))");
    let c = pipeline::finalize(&s).unwrap();
    validate::card(&c).unwrap();
    let Ability::Spell { body, .. } = &c.abilities[0] else {
        panic!()
    };
    assert!(matches!(
        &body.instructions[1],
        Instruction::Effect {
            operation: Operation::Move {
                objects: ObjectSet::InZone {
                    zone: EventZone::Exile,
                    ..
                },
                destination: Destination::OwnersBattlefield,
                entry: EntrySpec { tapped: true, .. },
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        &body.instructions[2],
        Instruction::Effect {
            operation: Operation::Tap {
                objects: ObjectSet::One(ObjectRef::Result {
                    result: ResultId(1),
                    phase: ObjectPhase::After,
                    ..
                }),
                ..
            },
            ..
        }
    ));
}

#[test]
fn rewrite_registry_rejects_unconsumed_fields_duplicate_sources_and_invalid_recipe_shapes() {
    let original = serde_json::to_value(pipeline::registry()).unwrap();
    for change in 0..4 {
        let mut value = original.clone();
        match change {
            0 => value["rules"][0]["arity"] = 3.into(),
            1 => {
                let duplicate = value["rules"][0].clone();
                value["rules"].as_array_mut().unwrap().push(duplicate);
            }
            2 => value["rules"][0]["destination"] = "OwnersHand".into(),
            _ => value["rules"][0]["misspelled_field"] = true.into(),
        }
        assert!(Registry::parse(&value.to_string()).is_err());
    }
}

#[test]
fn full_tree_report_and_fixture_selection_are_deterministic_and_separate_from_native_coverage() {
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pipeline.ronl");
    let report = pipeline::report::Report::check(&input, &root(), None).unwrap();
    assert_eq!(report.total, report.structurally_preserved);
    assert!(report.semantic_complete < report.total);
    assert!(report.source_forms["Action"].len() > report.rewrite_rules.len());
    assert_eq!(report.fixtures.covered, report.fixtures.universe);
    assert_eq!(
        pipeline::report::select_fixtures(&report.records).selected_lines,
        report.fixtures.selected_lines
    );
    let selected = scratch("fixtures.ronl");
    let count = pipeline::report::write_fixtures(&input, &report, &selected).unwrap();
    assert_eq!(count, report.fixtures.selected_lines.len());
    let subset = pipeline::report::Report::check(&selected, &root(), None).unwrap();
    let full: BTreeSet<_> = report
        .records
        .iter()
        .flat_map(|r| r.fixture_features.iter())
        .collect();
    let chosen: BTreeSet<_> = subset
        .records
        .iter()
        .flat_map(|r| r.fixture_features.iter())
        .collect();
    assert_eq!(full, chosen);
    let mut stale = report;
    stale.corpus_sha256 = "stale".into();
    assert!(pipeline::report::write_fixtures(&input, &stale, &selected).is_err());
    std::fs::remove_file(selected).unwrap();
}

#[test]
fn stage_recipe_roundtrips_and_final_export_stays_atomic_on_unresolved_data() {
    let s = custom("SpellActions(ActionList([GainControlOfPermanent(ThisPermanent)]))");
    let value = pipeline::Intermediate::new(&s).unwrap();
    let loaded: pipeline::Intermediate =
        ron::from_str(&ron::ser::to_string(&value).unwrap()).unwrap();
    assert_eq!(
        inspect::value(&loaded.structural.source.restore().unwrap()).unwrap(),
        inspect::value(&s).unwrap()
    );
    assert!(loaded.final_card.is_some());
    let rebound = pipeline::bind(&loaded.structural).unwrap();
    assert_eq!(
        serde_json::to_value(rebound).unwrap(),
        serde_json::to_value(loaded.final_card.as_ref().unwrap()).unwrap()
    );
    let mut altered = loaded.structural.clone();
    let ExpandedRecipe::ContinuousEffect { duration, .. } = &mut altered.rewrites[0].expanded
    else {
        panic!()
    };
    *duration = DurationInput::Source(source::Expiration::UntilEndOfTurn);
    assert!(
        pipeline::bind(&altered).is_err(),
        "a saved artifact cannot change generated semantics without changing its source"
    );
    altered = loaded.structural.clone();
    altered.registries.insert(
        "rust_syntax/normalize/rewrites/actions.json5".into(),
        "another registry".into(),
    );
    assert!(pipeline::bind(&altered).is_err());
    let input = scratch("input.ronl");
    let output = scratch("output.ronl");
    let bad=custom("SpellActions(ActionList([GainControlOfPermanentUntil(ThisPermanent,UntilItIsNoLongerEnchanted)]))");
    std::fs::write(&input, ron::ser::to_string(&bad).unwrap() + "\n").unwrap();
    std::fs::write(&output, "previous").unwrap();
    assert!(mtgish_ast::report::export_ron(&input, &output).is_err());
    assert_eq!(std::fs::read_to_string(&output).unwrap(), "previous");
    std::fs::remove_file(input).unwrap();
    std::fs::remove_file(output).unwrap();
}

#[test]
fn automatically_selected_corpus_fixtures_cover_the_registry_and_preserve_partial_and_final_forms()
{
    let (mut rules, mut cards, mut count) = (BTreeSet::new(), vec![], 0);
    let fixtures = root().join("rust_syntax/normalize/tests/fixtures/pipeline_generated.ronl");
    mtgish_ast::corpus::map_cards(
        &fixtures,
        |_, source| {
            let intermediate = pipeline::Intermediate::new(&source).unwrap();
            let native = pipeline::finalize(&source);
            match (&intermediate.final_card, native) {
                (Some(expected), Ok(actual)) => assert_eq!(
                    serde_json::to_value(expected).unwrap(),
                    serde_json::to_value(actual).unwrap()
                ),
                (None, Err(errors)) => assert_eq!(
                    serde_json::to_value(&intermediate.diagnostics).unwrap(),
                    serde_json::to_value(errors).unwrap()
                ),
                _ => panic!("prepared and ordinary lowering disagree"),
            }

            let loaded: pipeline::Intermediate =
                serde_json::from_value(serde_json::to_value(&intermediate).unwrap()).unwrap();
            assert_eq!(
                inspect::value(&loaded.structural.source.restore().unwrap()).unwrap(),
                inspect::value(&source).unwrap()
            );
            let structural = &loaded.structural;
            let used: Vec<String> = (structural.rewrites.iter().map(|r| r.rule.clone()))
                .chain(structural.numbers.iter().map(|r| r.rule.clone()))
                .chain(structural.selectors.iter().map(|r| r.rule.clone()))
                .chain(structural.programs.iter().map(|r| r.rule.clone()))
                .chain(structural.mechanics.iter().map(|r| r.rule.clone()))
                .chain(structural.operands.iter().map(|r| r.rule.clone()))
                .collect();
            if loaded.final_card.is_none() {
                assert!(!loaded.diagnostics.is_empty());
            }
            Ok((used, loaded.final_card))
        },
        |_, (used, card)| {
            rules.extend(used);
            cards.extend(card);
            count += 1;
            Ok(())
        },
    )
    .unwrap();
    let native = cards.len();
    for card in cards {
        let bytes = bincode::encode_to_vec(&card, bincode::config::standard()).unwrap();
        let (decoded, used): (Card, usize) =
            bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
        assert_eq!(used, bytes.len());
        validate::card(&decoded).unwrap();
        assert_eq!(
            serde_json::to_value(decoded).unwrap(),
            serde_json::to_value(card).unwrap()
        );
    }
    assert_eq!(
        rules,
        pipeline::registry()
            .rules
            .iter()
            .map(|r| r.id.clone())
            .chain(
                pipeline::numbers::registry()
                    .rules
                    .iter()
                    .map(|r| r.id.clone())
            )
            .chain(
                pipeline::selectors::registry()
                    .rules
                    .iter()
                    .map(|r| r.id.clone())
            )
            .chain(
                pipeline::programs::registry()
                    .rules
                    .iter()
                    .map(|r| r.id.clone())
            )
            .chain(
                pipeline::operands::registry()
                    .rules
                    .iter()
                    .map(|r| r.id.clone())
            )
            .chain(
                pipeline::mechanics::registry()
                    .rules
                    .iter()
                    .map(|r| r.id.clone())
            )
            .collect()
    );
    assert_eq!(
        count,
        include_str!("fixtures/pipeline_generated.ronl")
            .lines()
            .count()
    );
    assert!(native > 0 && native < count);
}

#[test]
fn saved_artifact_cli_rebinds_instead_of_trusting_cached_output_and_preserves_failed_destinations()
{
    let source = custom("SpellActions(ActionList([GainControlOfPermanent(ThisPermanent)]))");
    let mut intermediate = pipeline::Intermediate::new(&source).unwrap();
    intermediate.final_card.as_mut().unwrap().name = "untrusted cached name".into();
    let input = scratch("intermediate.json");
    let output = scratch("bound.ronl");
    std::fs::write(&input, serde_json::to_vec(&intermediate).unwrap()).unwrap();
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_ast"))
        .args(["pipeline-export"])
        .arg(&input)
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    let text = std::fs::read_to_string(&output).unwrap();
    let card: Card = ron::from_str(text.trim()).unwrap();
    assert_eq!(card.name, "Maro");
    intermediate.structural.registries.insert(
        "rust_syntax/normalize/rewrites/actions.json5".into(),
        "stale".into(),
    );
    std::fs::write(&input, serde_json::to_vec(&intermediate).unwrap()).unwrap();
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_ast"))
        .args(["pipeline-export"])
        .arg(&input)
        .arg(&output)
        .output()
        .unwrap();
    assert!(!status.status.success());
    assert_eq!(std::fs::read_to_string(&output).unwrap(), text);
    std::fs::remove_file(input).unwrap();
    std::fs::remove_file(output).unwrap();
}

#[test]
fn shared_quantities_compose_arithmetic_and_keep_actor_and_event_verbs() {
    let source=custom("SpellActions(Targeted([TargetPlayer(AnyPlayer)],ActionList([PlayerAction(Ref_TargetPlayer,GainLifeAndLifeForEach(Integer(2),Integer(3),Integer(4))),DiscardNumberCardsAtRandom(Integer(2)),EachPlayerAction(AnyPlayer,MillACard)])))");
    let card = pipeline::finalize(&source).unwrap();
    let Ability::Spell { body, .. } = &card.abilities[0] else {
        panic!()
    };
    assert_eq!(body.instructions.len(), 3);
    assert!(matches!(
        &body.instructions[0],
        Instruction::Effect {
            operation: Operation::GainLife {
                players: PlayerSet::One(PlayerRef::Target { slot: Slot(0), .. }),
                amount: Expr::Constant(14)
            },
            ..
        }
    ));
    assert!(matches!(
        &body.instructions[1],
        Instruction::Effect {
            operation: Operation::Discard {
                amount: Expr::Constant(2),
                random: true,
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        &body.instructions[2],
        Instruction::Effect {
            operation: Operation::Mill {
                players: PlayerSet::All,
                amount: Expr::Constant(1)
            },
            ..
        }
    ));
}

#[test]
fn shared_cardinalities_preserve_resolution_choices_and_simultaneous_sacrifice() {
    let source=custom("SpellActions(ActionList([UntapUptoNumberPermanents(Integer(2),AnyPermanent),DestroyAPermanentNoRegen(AnyPermanent),EachPlayerAction(AnyPlayer,SacrificeNumberPermanents(Integer(3),AnyPermanent))]))");
    let card = pipeline::finalize(&source).unwrap();
    let Ability::Spell { body, .. } = &card.abilities[0] else {
        panic!()
    };
    let Instruction::Group(untap) = &body.instructions[0] else {
        panic!()
    };
    assert!(matches!(untap.instructions.as_slice(),[
        Instruction::Effect {result,operation:Operation::ChooseObjects {count:Count::UpTo(Expr::Constant(2)),..}},
        Instruction::Effect {operation:Operation::Tap {objects:ObjectSet::Selected(selected),tapped:false},..}
    ] if result.0==selected.0));
    let Instruction::Group(destroy) = &body.instructions[1] else {
        panic!()
    };
    assert!(matches!(
        destroy.instructions.as_slice(),
        [
            Instruction::Effect {
                operation: Operation::ChooseObjects {
                    count: Count::AsManyAsCan(Expr::Constant(1)),
                    ..
                },
                ..
            },
            Instruction::Effect {
                operation: Operation::Destroy {
                    regeneration: false,
                    ..
                },
                ..
            }
        ]
    ));
    assert!(matches!(
        &body.instructions[2],
        Instruction::Effect {
            operation: Operation::Sacrifice {
                players: PlayerSet::All,
                choose: Some(Count::AsManyAsCan(Expr::Constant(3))),
                ..
            },
            ..
        }
    ));
}

#[test]
fn effect_templates_keep_snapshot_characteristics_separate_from_live_rule_subjects() {
    let source=custom("SpellActions(ActionList([CreateEachPermanentLayerEffectUntil(AnyPermanent,[AdjustPT(1,2)],UntilEndOfTurn),CreateEachPermanentRuleEffectUntil(AnyPermanent,[CantAttack],UntilEndOfTurn)]))");
    let card = pipeline::finalize(&source).unwrap();
    let Ability::Spell { body, .. } = &card.abilities[0] else {
        panic!()
    };
    assert!(matches!(
        &body.instructions[0],
        Instruction::Effect {
            operation: Operation::CreateContinuousEffect {
                evaluation: EffectEvaluation::Snapshot,
                duration: Duration::EndOfTurn,
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        &body.instructions[1],
        Instruction::Effect {
            operation: Operation::CreateContinuousEffect {
                evaluation: EffectEvaluation::LiveObjects,
                duration: Duration::EndOfTurn,
                ..
            },
            ..
        }
    ));
}

#[test]
fn registry_accounts_for_fields_inside_arithmetic_and_selection_templates() {
    let value = serde_json::to_value(pipeline::registry()).unwrap();
    for form in ["GainLifeAndLifeForEach", "UntapUptoNumberPermanents"] {
        let mut bad = value.clone();
        let row = bad["rules"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| r["source"] == form)
            .unwrap();
        if form == "GainLifeAndLifeForEach" {
            row["amount"]["right"]["right"]["argument"] = 1.into();
        } else {
            row["selection"]["amount"]["argument"] = 1.into();
        }
        assert!(Registry::parse(&bad.to_string())
            .unwrap_err()
            .contains("reused argument"));
    }
}

#[test]
fn candidate_preparation_groups_real_rust_signatures_without_claiming_semantic_equivalence() {
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pipeline.ronl");
    let report = pipeline::report::Report::check(&input, &root(), None).unwrap();
    let schema = std::fs::read_to_string(root().join("rust_syntax/src/mtg_types.rs")).unwrap();
    let candidates =
        pipeline::candidates::Candidates::from_report(&report, &schema, "Action").unwrap();
    assert_eq!(
        candidates.observed_forms,
        report.source_forms["Action"].len()
    );
    assert_eq!(
        candidates.occurrences,
        report.source_forms["Action"].values().sum::<usize>()
    );
    assert!(candidates.groups.len() < candidates.observed_forms);
    for group in &candidates.groups {
        for c in &group.candidates {
            assert_eq!(c.arity, group.fields.len());
            assert!(c.record_count > 0);
            assert!(!c.example_lines.is_empty());
        }
    }
    let numbers =
        pipeline::candidates::Candidates::from_report(&report, &schema, "GameNumber").unwrap();
    assert!(numbers.groups.iter().any(|g| g.fields == ["i32"]));
    assert!(numbers
        .groups
        .iter()
        .flat_map(|g| &g.candidates)
        .all(|c| c.record_count > 0 && !c.example_lines.is_empty()));
    assert!(
        pipeline::candidates::Candidates::from_report(&report, &(schema + "\n"), "Action").is_err()
    );
}

#[test]
fn fixture_features_distinguish_operand_positions_and_actor_scopes() {
    let input = scratch("position-scopes.ronl");
    let sources=[
        custom("SpellActions(ActionList([GainLifeForEach(Integer(1),X_From_Casting)]))"),
        custom("SpellActions(ActionList([PlayerAction(You,GainLifeForEach(X_From_Casting,Integer(1)))]))"),
    ];
    std::fs::write(
        &input,
        sources
            .iter()
            .map(|s| ron::ser::to_string(s).unwrap() + "\n")
            .collect::<String>(),
    )
    .unwrap();
    let report = pipeline::report::Report::check(&input, &root(), None).unwrap();
    let first = &report.records[0].fixture_features;
    let second = &report.records[1].fixture_features;
    assert!(
        first.contains("rule:action.GainLifeForEach/argument:1/operand:GameNumber::X_From_Casting")
    );
    assert!(!second
        .contains("rule:action.GainLifeForEach/argument:1/operand:GameNumber::X_From_Casting"));
    assert!(second
        .contains("rule:action.GainLifeForEach/argument:0/operand:GameNumber::X_From_Casting"));
    assert!(second.contains("rule:action.GainLifeForEach/scope:Action::PlayerAction"));
    assert!(!first.contains("rule:action.GainLifeForEach/scope:Action::PlayerAction"));
    assert_eq!(report.fixtures.selected_lines.len(), 2);
    std::fs::remove_file(input).unwrap();
}

#[test]
fn indexed_fixture_selection_matches_reference_greedy_selection_including_score_updates_and_ties() {
    use pipeline::report::{select_fixtures, Record};
    use std::cmp::Reverse;
    let mut seed = 17u64;
    let mut records = vec![];
    for i in 0..200 {
        let mut features = BTreeSet::new();
        for feature in 0..80 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            if seed >> 60 < 3 {
                features.insert(format!("feature:{feature}"));
            }
        }
        records.push(Record {
            line: i + 1,
            identity: corpus::Identity {
                layout: "Card".into(),
                names: vec![],
                source_sha256: String::new(),
            },
            nodes: 20 + i % 5,
            faces: 1,
            rewrites: 1,
            number_rewrites: 0,
            selector_rewrites: 0,
            program_rewrites: 0,
            operand_rewrites: 0,
            mechanic_rewrites: 0,
            semantic_complete: i % 3 == 0,
            named: Default::default(),
            source_forms: BTreeSet::new(),
            diagnostics: vec![],
            remaining_registry_forms: Default::default(),
            fixture_features: features,
        });
    }
    let mut uncovered: BTreeSet<_> = records
        .iter()
        .flat_map(|r| r.fixture_features.iter().cloned())
        .collect();
    let universe = uncovered.len();
    let mut expected = vec![];
    while !uncovered.is_empty() {
        let r = records
            .iter()
            .max_by_key(|r| {
                (
                    r.fixture_features.intersection(&uncovered).count(),
                    r.semantic_complete,
                    Reverse(r.nodes),
                    Reverse(r.line),
                )
            })
            .unwrap();
        expected.push(r.line);
        for f in &r.fixture_features {
            uncovered.remove(f);
        }
    }
    let selected = select_fixtures(&records);
    assert_eq!(selected.selected_lines, expected);
    assert_eq!(selected.covered, universe);
    assert!(select_fixtures(&[]).selected_lines.is_empty());
}

fn numeric_amount(card: &Card, index: usize) -> &Expr {
    let Ability::Spell { body, .. } = &card.abilities[0] else {
        panic!()
    };
    let Instruction::Effect {
        operation: Operation::GainLife { amount, .. },
        ..
    } = &body.instructions[index]
    else {
        panic!()
    };
    amount
}

#[test]
fn numeric_consolidation_traverses_unsupported_actions_without_binding_them() {
    let source =
        custom("SpellActions(ActionList([DiscardNumberCardsDrawnThisTurn(Twice(NumCardsInPlayersLibrary(You)))]))");
    let stage = pipeline::Intermediate::new(&source).unwrap();
    assert!(stage.structural.rewrites.is_empty());
    assert_eq!(stage.structural.numbers.len(), 2);
    assert!(stage.final_card.is_none());
    assert!(
        matches!(&stage.structural.numbers[0].expression,pipeline::Quantity::Multiply(a,b) if matches!(a.as_ref(),pipeline::Quantity::Constant(2)) && matches!(b.as_ref(),pipeline::Quantity::Count(pipeline::NumberQuery::PrivateCards {zone:EventZone::Library,..})))
    );
    assert_eq!(
        inspect::value(&stage.structural.source.restore().unwrap()).unwrap(),
        inspect::value(&source).unwrap()
    );
}

#[test]
fn unknown_numeric_leaves_survive_formula_expansion_and_block_final_export() {
    let source =
        custom("SpellActions(ActionList([GainLife(Plus(Integer(2),PlayersChosenNumber(You)))]))");
    let stage = pipeline::Intermediate::new(&source).unwrap();
    assert_eq!(stage.structural.numbers.len(), 2);
    assert!(
        matches!(&stage.structural.numbers[0].expression,pipeline::Quantity::Add(a,b) if matches!(a.as_ref(),pipeline::Quantity::Constant(2)) && matches!(b.as_ref(),pipeline::Quantity::Source(source::GameNumber::PlayersChosenNumber(_))))
    );
    let loaded: pipeline::Intermediate =
        serde_json::from_value(serde_json::to_value(&stage).unwrap()).unwrap();
    assert!(loaded.final_card.is_none());
    assert!(pipeline::bind(&loaded.structural)
        .unwrap_err()
        .iter()
        .any(|d| d.source_type == "GameNumber" && d.form == "PlayersChosenNumber"));
}

#[test]
fn numeric_binding_preserves_target_domains_zone_membership_and_last_known_properties() {
    let source=custom("SpellActions(Targeted([TargetPermanent(AnyPermanent),TargetPlayer(AnyPlayer)],ActionList([GainLife(Plus(PowerOfPermanent(Ref_TargetPermanent),NumCardsInPlayersLibrary(Ref_TargetPlayer))),GainLife(TheNumberOfPermanentsOnTheBattlefield(SinglePermanent(Ref_TargetPermanent))),GainLife(TotalPowerOfPermanents(SinglePermanent(Ref_TargetPermanent)))])))");
    let card = pipeline::finalize(&source).unwrap();
    let Expr::Add(power, count) = numeric_amount(&card, 0) else {
        panic!()
    };
    assert!(matches!(
        power.as_ref(),
        Expr::ObjectProperty {
            object: ObjectRef::Target { slot: Slot(0), .. },
            property: Property::Power,
            information: Information::CurrentOrLastKnown
        }
    ));
    assert!(
        matches!(count.as_ref(),Expr::CountObjects(objects) if matches!(objects.as_ref(),ObjectSet::Zone {scope:ZoneScope::Players {zone:EventZone::Library,players:PlayerSet::One(PlayerRef::Target {slot:Slot(1),..})},kind:ObjectKind::Card,..}))
    );
    assert!(
        matches!(numeric_amount(&card,1),Expr::CountObjects(objects) if matches!(objects.as_ref(),ObjectSet::InZone {zone:EventZone::Battlefield,..}))
    );
    assert!(
        matches!(numeric_amount(&card,2),Expr::Aggregate {objects,property:Property::Power,aggregation:Aggregation::Sum} if matches!(objects.as_ref(),ObjectSet::One(ObjectRef::Target {slot:Slot(0),..})))
    );
}

#[test]
fn arithmetic_recipes_preserve_operand_order_and_explicit_rounding_without_eager_evaluation() {
    let source=custom("SpellActions(ActionList([GainLife(Plus3(Integer(1),HalfRoundedUp(Integer(3)),Minus(Integer(8),Integer(5))))]))");
    let card = pipeline::finalize(&source).unwrap();
    let Expr::Add(left, right) = numeric_amount(&card, 0) else {
        panic!()
    };
    assert!(
        matches!(right.as_ref(),Expr::Subtract(a,b) if matches!(a.as_ref(),Expr::Constant(8)) && matches!(b.as_ref(),Expr::Constant(5)))
    );
    let Expr::Add(one, divide) = left.as_ref() else {
        panic!()
    };
    assert!(matches!(one.as_ref(), Expr::Constant(1)));
    assert!(
        matches!(divide.as_ref(),Expr::Divide {numerator,denominator,rounding:Rounding::Up} if matches!(numerator.as_ref(),Expr::Constant(3)) && matches!(denominator.as_ref(),Expr::Constant(2)))
    );
    let loaded: Card = ron::from_str(&ron::ser::to_string(&card).unwrap()).unwrap();
    validate::card(&loaded).unwrap();
}

#[test]
fn query_recipes_preserve_exclusions_distinct_values_and_cross_zone_aggregates() {
    let source=custom("SpellActions(Targeted([TargetGraveyardCard(AnyCardInAnyGraveyard)],ActionList([GainLife(TheNumberOfOtherCardsInPlayersGraveyard(Ref_TargetGraveyardCard,AnyCard,You)),GainLife(TheGreatestPowerAmongPermanentsAndCardsInPlayersGraveyard(AnyPermanent,AnyCard,You)),GainLife(NumberOfBasicLandTypesAmongPermanents(AnyPermanent))])))");
    let card = pipeline::finalize(&source).unwrap();
    let Expr::CountObjects(objects) = numeric_amount(&card, 0) else {
        panic!()
    };
    assert!(
        matches!(objects.as_ref(),ObjectSet::Zone {scope:ZoneScope::Players {zone:EventZone::Graveyard,..},filter:ObjectFilter::All(filters),..} if matches!(filters.as_slice(),[ObjectFilter::Any,ObjectFilter::Not(except)] if matches!(except.as_ref(),ObjectFilter::SameObject(ObjectRef::Target {slot:Slot(0),..}))))
    );
    assert!(
        matches!(numeric_amount(&card,1),Expr::Aggregate {objects,property:Property::Power,aggregation:Aggregation::Maximum} if matches!(objects.as_ref(),ObjectSet::Union(zones) if zones.len()==2))
    );
    assert!(matches!(
        numeric_amount(&card, 2),
        Expr::CountDistinct {
            property: DistinctProperty::BasicLandTypes,
            ..
        }
    ));
}

#[test]
fn counter_queries_keep_recipient_domains_kinds_and_information_policy() {
    let source=custom("SpellActions(ActionList([GainLife(TheTotalNumberOfCountersOfTypeAmongPermanents(PTCounter(1,1),AnyPermanent)),GainLife(TheTotalNumberOfCountersAmongPlayers(AnyPlayer))]))");
    let card = pipeline::finalize(&source).unwrap();
    assert!(
        matches!(numeric_amount(&card,0),Expr::CountCounters {recipients,kind:Some(mtgish_ast::counter::CounterKind::PowerToughness(1,1)),information:Information::CurrentOrLastKnown} if matches!(recipients.as_ref(),Recipients::Objects(_)))
    );
    assert!(
        matches!(numeric_amount(&card,1),Expr::CountCounters {recipients,kind:None,..} if matches!(recipients.as_ref(),Recipients::Players(PlayerSet::All)))
    );
}

#[test]
fn numeric_registry_rejects_field_loss_reuse_unknown_fields_and_empty_unions() {
    let original = serde_json::to_value(pipeline::numbers::registry()).unwrap();
    for case in 0..4 {
        let mut value = original.clone();
        match case {
            0 => value["rules"][0]["expression"] = serde_json::json!({"kind":"Constant","value":0}),
            1 => value["rules"][1]["expression"]["right"]["argument"] = 0.into(),
            2 => value["rules"][0]["expression"]["discarded_field"] = true.into(),
            _ => {
                value["rules"][0]["expression"] =
                    serde_json::json!({"kind":"Count","query":{"kind":"Union","queries":[]}})
            }
        }
        assert!(mtgish_ast::number_spec::Registry::parse(&value.to_string()).is_err());
    }
}

#[test]
fn saved_numeric_artifacts_verify_expressions_source_keys_and_registry_fingerprints() {
    let source = custom("SpellActions(ActionList([GainLife(NumCardsInPlayersLibrary(You))]))");
    let stage = pipeline::Intermediate::new(&source).unwrap();
    assert_eq!(stage.format, "mtgish.pipeline.intermediate.v8");
    let mut changed = stage.structural.clone();
    changed.numbers[0].expression = pipeline::Quantity::Constant(999);
    assert!(pipeline::bind(&changed).is_err());
    changed = stage.structural.clone();
    changed.numbers[0].source_key = "another expression".into();
    assert!(pipeline::bind(&changed).is_err());
    changed = stage.structural.clone();
    changed.registries.insert(
        "rust_syntax/normalize/rewrites/numbers.json5".into(),
        "another registry".into(),
    );
    assert!(pipeline::bind(&changed).is_err());
    let rebound = pipeline::bind(&stage.structural).unwrap();
    assert_eq!(
        serde_json::to_value(rebound).unwrap(),
        serde_json::to_value(stage.final_card.unwrap()).unwrap()
    );
}

#[test]
fn shared_numeric_templates_bind_freshly_in_each_ability_scope() {
    let mut source=custom("SpellActions(Targeted([TargetPermanent(AnyPermanent)],ActionList([GainLife(PowerOfPermanent(Ref_TargetPermanent))])))");
    let source::OracleCard::Card { rules, .. } = &mut source else {
        panic!()
    };
    rules.as_mut().unwrap().push(
        ron::from_str(
            "SpellActions(ActionList([GainLife(PowerOfPermanent(Ref_TargetPermanent))]))",
        )
        .unwrap(),
    );
    let stage = pipeline::Intermediate::new(&source).unwrap();
    assert_eq!(stage.structural.numbers.len(), 2);
    assert_eq!(
        stage.structural.numbers[0].source_key,
        stage.structural.numbers[1].source_key
    );
    assert!(stage.final_card.is_none());
    assert!(stage
        .diagnostics
        .iter()
        .any(|d| d.message.contains("target slot")));
}

#[test]
fn ordinary_lowering_handles_nested_legacy_abilities_on_the_default_stack() {
    let text = include_str!("fixtures/pipeline_nested_abilities.ronl").trim();
    let source: source::OracleCard = ron::from_str(text).unwrap();
    match pipeline::finalize(&source) {
        Ok(card) => validate::card(&card).unwrap(),
        Err(errors) => assert!(!errors.is_empty()),
    }
}

#[test]
fn selectors_consolidate_beneath_unknown_parents_and_preserve_unknown_leaves() {
    use pipeline::selectors::{Predicate as P, SelectorSource};
    let source=custom("SpellActions(ActionList([SetLifeTotal(TheNumberOfPermanentsOnTheBattlefield(And([IsCreatureType(Elf),HasntDealtCombatDamageThisGame])))]))");
    let stage = pipeline::Intermediate::new(&source).unwrap();
    assert!(stage.final_card.is_none());
    let root = stage
        .structural
        .selectors
        .iter()
        .find(|r| r.rule == "selector.Permanents.And")
        .unwrap();
    assert!(
        matches!(&root.predicate,P::All(parts) if matches!(parts.as_slice(),[P::Vocabulary(_),P::Source(SelectorSource::Permanents(source::Permanents::HasntDealtCombatDamageThisGame))]))
    );
    let source=custom("SpellActions(Targeted([TargetPermanent(And([IsCardtype(Creature),HasntDealtCombatDamageThisGame]))],ActionList([TapPermanent(Ref_TargetPermanent)])))");
    let stage = pipeline::Intermediate::new(&source).unwrap();
    assert!(stage.final_card.is_none());
    assert!(stage
        .diagnostics
        .iter()
        .any(|d| d.form == "HasntDealtCombatDamageThisGame"));
}

#[test]
fn equivalent_selector_vocabulary_uses_one_predicate_tree_across_six_source_domains() {
    use pipeline::selectors::{self, SelectorSource};
    let text = "And([IsCardtype(Creature),IsCreatureType(Elf)])";
    let values = [
        SelectorSource::Permanents(ron::from_str(text).unwrap()),
        SelectorSource::Cards(ron::from_str(text).unwrap()),
        SelectorSource::Spells(ron::from_str(text).unwrap()),
        SelectorSource::CardsInGraveyards(ron::from_str(text).unwrap()),
        SelectorSource::CardsInHand(ron::from_str(text).unwrap()),
        SelectorSource::CardsInExile(ron::from_str(text).unwrap()),
    ];
    let predicates: Vec<_> = values
        .iter()
        .map(|v| serde_json::to_value(selectors::rewrite(v.as_ref()).unwrap()).unwrap())
        .collect();
    assert!(predicates.windows(2).all(|w| w[0] == w[1]));
    let keys: BTreeSet<_> = values
        .iter()
        .map(|v| selectors::source_key(v.as_ref()))
        .collect();
    assert_eq!(
        keys.len(),
        6,
        "source identities remain domain-specific for later binding"
    );
}

#[test]
fn selectors_keep_permanent_spell_and_graveyard_target_domains() {
    let source=custom("SpellActions(Targeted([TargetPermanent(And([IsCardtype(Creature),IsCreatureType(Elf)])),TargetSpell(And([IsCardtype(Creature),IsCreatureType(Elf)])),TargetGraveyardCard(And([IsCardtype(Creature),IsCreatureType(Elf)]))],ActionList([TapPermanent(Ref_TargetPermanent),CounterSpell(Ref_TargetSpell),PutGraveyardCardIntoHand(Ref_TargetGraveyardCard)])))");
    let card = pipeline::finalize(&source).unwrap();
    let Ability::Spell { announcement, .. } = &card.abilities[0] else {
        panic!()
    };
    let targets = &announcement.targets;
    let actual: Vec<_> = targets.iter().map(|t| &t.kind).collect();
    assert!(
        matches!(actual.as_slice(),[TargetKind::Object(a),TargetKind::Object(b),TargetKind::Object(c)] if a==&ObjectDomain::permanent() && b==&ObjectDomain::spell() && c==&ObjectDomain::card(EventZone::Graveyard))
    );
    assert_eq!(
        serde_json::to_value(&targets[0].filter).unwrap(),
        serde_json::to_value(&targets[1].filter).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&targets[1].filter).unwrap(),
        serde_json::to_value(&targets[2].filter).unwrap()
    );
}

#[test]
fn predicate_properties_bind_the_candidate_without_rebinding_explicit_targets() {
    let source=custom("SpellActions(Targeted([TargetPermanent(AnyPermanent),TargetPermanent(And([Other(Ref_TargetPermanent),PowerIs(EqualTo(ToughnessOfPermanent(Self_It)))]))],ActionList([TapPermanent(Ref_TargetPermanent2)])))");
    let card = pipeline::finalize(&source).unwrap();
    let Ability::Spell { announcement, .. } = &card.abilities[0] else {
        panic!()
    };
    let targets = &announcement.targets;
    let ObjectFilter::All(filters) = &targets[1].filter else {
        panic!()
    };
    assert!(
        matches!(&filters[0],ObjectFilter::Not(f) if matches!(f.as_ref(),ObjectFilter::SameObject(ObjectRef::Target {slot:Slot(0),..})))
    );
    assert!(matches!(
        &filters[1],
        ObjectFilter::Property {
            property: Property::Power,
            comparison: Comparison::Equal,
            value: Expr::ObjectProperty {
                object: ObjectRef::Candidate,
                property: Property::Toughness,
                ..
            }
        }
    ));
}

#[test]
fn zone_predicates_preserve_incoming_scope_instead_of_inventing_a_candidate() {
    let source=custom("SpellActions(Targeted([TargetGraveyardCard(InAPlayersGraveyard(SinglePlayer(ControllerOfPermanent(Self_It))))],ActionList([PutGraveyardCardIntoHand(Ref_TargetGraveyardCard)])))");
    let errors = pipeline::finalize(&source).unwrap_err();
    assert!(errors
        .iter()
        .any(|d| d.source_type == "Permanent" && d.form == "Self_It"));
    let source=custom("SpellActions(Targeted([TargetGraveyardCard(InAPlayersGraveyard(SinglePlayer(You)))],ActionList([PutGraveyardCardIntoHand(Ref_TargetGraveyardCard)])))");
    pipeline::finalize(&source).unwrap();
}

#[test]
fn selector_negation_counter_kinds_and_base_properties_remain_distinct() {
    let source=custom("SpellActions(Targeted([TargetPermanent(And([IsNonLandType(Forest),HasNumberCountersOfType(GreaterThan(Integer(2)),PTCounter(1,1)),BaseToughnessIs(EqualTo(Integer(3)))]))],ActionList([TapPermanent(Ref_TargetPermanent)])))");
    let card = pipeline::finalize(&source).unwrap();
    let Ability::Spell { announcement, .. } = &card.abilities[0] else {
        panic!()
    };
    let targets = &announcement.targets;
    let ObjectFilter::All(filters) = &targets[0].filter else {
        panic!()
    };
    assert!(
        matches!(&filters[0],ObjectFilter::Not(inner) if matches!(inner.as_ref(),ObjectFilter::Subtype(source::SubType::Forest)))
    );
    assert!(matches!(
        &filters[1],
        ObjectFilter::Counters {
            kind: Some(mtgish_ast::counter::CounterKind::PowerToughness(1, 1)),
            comparison: Comparison::Greater,
            value: Expr::Constant(2)
        }
    ));
    assert!(matches!(
        &filters[2],
        ObjectFilter::Property {
            property: Property::BaseToughness,
            comparison: Comparison::Equal,
            value: Expr::Constant(3)
        }
    ));
}

#[test]
fn shared_selector_templates_do_not_capture_another_abilitys_target_scope() {
    let mut source=custom("SpellActions(Targeted([TargetPermanent(AnyPermanent),TargetPermanent(Other(Ref_TargetPermanent))],ActionList([TapPermanent(Ref_TargetPermanent2)])))");
    let source::OracleCard::Card { rules, .. } = &mut source else {
        panic!()
    };
    rules.as_mut().unwrap().push(ron::from_str("SpellActions(Targeted([TargetPermanent(Other(Ref_TargetPermanent))],ActionList([TapPermanent(Ref_TargetPermanent)])))").unwrap());
    let stage = pipeline::Intermediate::new(&source).unwrap();
    let refs: Vec<_> = stage
        .structural
        .selectors
        .iter()
        .filter(|r| r.rule == "selector.Permanents.Other")
        .map(|r| &r.source_key)
        .collect();
    assert_eq!(refs.len(), 2);
    assert_eq!(refs[0], refs[1]);
    assert!(stage.final_card.is_none());
    assert!(stage
        .diagnostics
        .iter()
        .any(|d| d.message.contains("target slot")));
}

#[test]
fn selector_registry_accounts_for_nested_fields_domains_and_vocabulary_constants() {
    let original = serde_json::to_value(pipeline::selectors::registry()).unwrap();
    for case in 0..5 {
        let mut value = original.clone();
        match case {
            0 => {
                value["rules"][0]["predicate"] = serde_json::json!({"kind":"Literal","value":"Any"})
            }
            1 => {
                value["rules"][0]["predicate"] = serde_json::json!({"kind":"All","predicates":[{"kind":"Source","argument":0},{"kind":"Source","argument":0}]})
            }
            2 => value["rules"][0]["domain"] = "Cards".into(),
            3 => {
                value["rules"][0]["predicate"] = serde_json::json!({"kind":"Vocabulary","vocabulary":"CardType","argument":0,"value":"Creature"})
            }
            _ => {
                value["rules"][0]["predicate"] = serde_json::json!({"kind":"Vocabulary","vocabulary":"CardType","value":"Not a Rust constant"})
            }
        }
        assert!(mtgish_ast::selector_spec::Registry::parse(&value.to_string()).is_err());
    }
}

#[test]
fn selector_artifacts_and_saved_reports_require_all_registry_fingerprints() {
    let source=custom("SpellActions(Targeted([TargetPermanent(IsCardtype(Creature))],ActionList([TapPermanent(Ref_TargetPermanent)])))");
    let stage = pipeline::Intermediate::new(&source).unwrap();
    assert_eq!(stage.structural.registries.len(), 7);
    let loaded: pipeline::Intermediate =
        ron::from_str(&ron::ser::to_string(&stage).unwrap()).unwrap();
    assert!(pipeline::bind(&loaded.structural).is_ok());
    for case in 0..3 {
        let mut changed = stage.structural.clone();
        match case {
            0 => {
                changed.selectors[0].predicate = pipeline::selectors::Predicate::Literal(
                    pipeline::selectors::PredicateLiteral::Any,
                )
            }
            1 => changed.selectors[0].source_key = "a different source".into(),
            _ => {
                changed
                    .registries
                    .remove("rust_syntax/normalize/rewrites/selectors.json5");
            }
        }
        assert!(pipeline::bind(&changed).is_err());
    }
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pipeline.ronl");
    let mut report = pipeline::report::Report::check(&input, &root(), None).unwrap();
    report
        .inputs
        .remove("rust_syntax/normalize/rewrites/selectors.json5");
    assert!(
        pipeline::report::write_fixtures(&input, &report, &scratch("stale-selectors.ronl"))
            .is_err()
    );
    let schema = std::fs::read_to_string(root().join("rust_syntax/src/mtg_types.rs")).unwrap();
    assert!(pipeline::candidates::Candidates::from_report(&report, &schema, "Permanents").is_err());
}

#[test]
fn program_structure_consolidates_even_when_a_nested_instruction_is_unknown() {
    use pipeline::programs::InstructionRecipe as R;
    let source = custom(
        "SpellActions(ActionList([MayActions([DrawACard,DiscardNumberCardsDrawnThisTurn(Integer(7))])]))",
    );
    let stage = pipeline::Intermediate::new(&source).unwrap();
    assert!(stage.final_card.is_none());
    let recipe = &stage
        .structural
        .programs
        .iter()
        .find(|r| r.rule == "program.Action.MayActions")
        .unwrap()
        .recipe;
    assert!(
        matches!(recipe,R::Optional {body,..} if matches!(body.as_ref(),R::Sequence(items) if matches!(items.as_slice(),[R::Effect(_),R::Source(_)])))
    );
    assert!(stage
        .diagnostics
        .iter()
        .any(|d| d.form == "DiscardNumberCardsDrawnThisTurn"));
}

#[test]
fn optional_multi_instruction_recipes_keep_one_decider_and_do_not_bind_player_as_you() {
    let source=custom("SpellActions(Targeted([TargetPlayer(AnyPlayer)],ActionList([PlayerMayActions(Ref_TargetPlayer,[DrawACard,GainLife(Integer(2)),PlayerAction(You,DrawACard)])])))");
    let card = pipeline::finalize(&source).unwrap();
    let Ability::Spell { body, .. } = &card.abilities[0] else {
        panic!()
    };
    let Instruction::Optional {
        decider: PlayerRef::Target { slot: Slot(0), .. },
        body,
    } = &body.instructions[0]
    else {
        panic!()
    };
    assert!(matches!(
        body.instructions.as_slice(),
        [
            Instruction::Effect {
                operation: Operation::Draw {
                    players: PlayerSet::One(PlayerRef::Target { slot: Slot(0), .. }),
                    ..
                },
                ..
            },
            Instruction::Effect {
                operation: Operation::GainLife {
                    players: PlayerSet::One(PlayerRef::Target { slot: Slot(0), .. }),
                    ..
                },
                ..
            },
            Instruction::Effect {
                operation: Operation::Draw {
                    players: PlayerSet::One(PlayerRef::AbilityController),
                    ..
                },
                ..
            }
        ]
    ));
}

#[test]
fn program_actor_batches_preserve_single_operations_and_reject_implicit_serialization() {
    let source=custom("SpellActions(ActionList([EachPlayerAction(AnyPlayer,SacrificeAPermanent(AnyPermanent)),EachPlayerAction(AnyPlayer,DrawACard)]))");
    let card = pipeline::finalize(&source).unwrap();
    let Ability::Spell { body, .. } = &card.abilities[0] else {
        panic!()
    };
    assert!(matches!(
        body.instructions.as_slice(),
        [
            Instruction::Effect {
                operation: Operation::Sacrifice {
                    players: PlayerSet::All,
                    choose: Some(Count::AsManyAsCan(Expr::Constant(1))),
                    ..
                },
                ..
            },
            Instruction::Effect {
                operation: Operation::Draw {
                    players: PlayerSet::All,
                    ..
                },
                ..
            }
        ]
    ));
    let source = custom(
        "SpellActions(ActionList([EachPlayerAction(AnyPlayer,TapPermanent(ThisPermanent))]))",
    );
    let card = pipeline::finalize(&source).unwrap();
    let Ability::Spell { body, .. } = &card.abilities[0] else {
        panic!()
    };
    assert!(matches!(
        body.instructions.as_slice(),
        [Instruction::ForEachPlayer {
            players: PlayerSet::All,
            ..
        }]
    ));
    let source =
        custom("SpellActions(ActionList([EachPlayerAction(AnyPlayer,MayAction(DrawACard))]))");
    let card = pipeline::finalize(&source).unwrap();
    let Ability::Spell { body, .. } = &card.abilities[0] else {
        panic!()
    };
    let [Instruction::ForEachPlayer { body, .. }] = body.instructions.as_slice() else {
        panic!()
    };
    assert!(matches!(
        body.instructions.as_slice(),
        [Instruction::Optional {
            decider: PlayerRef::Bound(_),
            ..
        }]
    ));
}

#[test]
fn program_conditionals_cannot_leak_movement_results() {
    for action in [
        "IfElse(ValueIs(Integer(1),EqualTo(Integer(1))),[PutGraveyardCardOntoBattlefield(Ref_TargetGraveyardCard,[])],[])",
    ] {
        let source=custom(&format!("SpellActions(Targeted([TargetGraveyardCard(AnyCardInAnyGraveyard)],ActionList([{action},TapPermanent(ThePermanentPutOnTheBattlefieldThisWay)])))"));
        assert!(pipeline::finalize(&source).unwrap_err().iter().any(|d|d.message.contains("movement reference")));
    }
    let source=custom("SpellActions(Targeted([TargetGraveyardCard(AnyCardInAnyGraveyard),TargetPlayer(AnyPlayer)],ActionList([PlayerAction(Ref_TargetPlayer,PutGraveyardCardOntoBattlefield(Ref_TargetGraveyardCard,[])),TapPermanent(ThePermanentPutOnTheBattlefieldThisWay)])))");
    let card = pipeline::finalize(&source).unwrap();
    let Ability::Spell { body, .. } = &card.abilities[0] else {
        panic!()
    };
    assert!(matches!(
        &body.instructions[1],
        Instruction::Effect {
            operation: Operation::Tap {
                objects: ObjectSet::One(ObjectRef::Result {
                    result: ResultId(0),
                    phase: ObjectPhase::After,
                    ..
                }),
                ..
            },
            ..
        }
    ));
}

#[test]
fn program_payment_retains_explicit_payer_and_unless_uses_the_payment_result() {
    let source=custom("SpellActions(Targeted([TargetPlayer(AnyPlayer)],ActionList([PlayerMayCost(Ref_TargetPlayer,PayLife(Integer(2))),Unless(CostWasPaid,[DrawACard])])))");
    let card = pipeline::finalize(&source).unwrap();
    let Ability::Spell { body, .. } = &card.abilities[0] else {
        panic!()
    };
    assert!(matches!(
        &body.instructions[0],
        Instruction::Effect {
            result: ResultId(0),
            operation: Operation::PayCost {
                player: PlayerRef::Target { slot: Slot(0), .. },
                optional: true,
                ..
            }
        }
    ));
    assert!(
        matches!(&body.instructions[1],Instruction::If {condition:Condition::Not(c),otherwise:Program {instructions},..} if instructions.is_empty() && matches!(c.as_ref(),Condition::PaymentChosen(ResultId(0))))
    );
}

#[test]
fn program_modes_preserve_announcement_targets_and_reset_result_and_reference_scopes() {
    let source=custom("SpellActions(Modal_ChooseOneOrBoth([Targeted([TargetPermanent(AnyPermanent)],ActionList([TapPermanent(Ref_TargetPermanent)])),Targeted([TargetPlayer(AnyPlayer)],ActionList([PlayerAction(Ref_TargetPlayer,DrawACard)]))]))");
    let card = pipeline::finalize(&source).unwrap();
    let Ability::Spell { announcement, body } = &card.abilities[0] else {
        panic!()
    };
    assert!(body.instructions.is_empty());
    assert!(announcement.targets.is_empty());
    let modes = announcement.modes.as_ref().unwrap();
    assert!(matches!(
        (&modes.minimum, &modes.maximum, modes.repeat),
        (&Expr::Constant(1), &Expr::Constant(2), false)
    ));
    assert_eq!(modes.modes[0].announcement.targets[0].slot.0, 0);
    assert_eq!(modes.modes[1].announcement.targets[0].slot.0, 0);
    assert!(matches!(
        &modes.modes[0].body.instructions[0],
        Instruction::Effect {
            result: ResultId(0),
            ..
        }
    ));
    assert!(matches!(
        &modes.modes[1].body.instructions[0],
        Instruction::Effect {
            result: ResultId(0),
            ..
        }
    ));
    let source=custom("SpellActions(Modal_ChooseOne([Targeted([TargetPermanent(AnyPermanent)],ActionList([TapPermanent(Ref_TargetPermanent)])),ActionList([TapPermanent(Ref_TargetPermanent)])]))");
    assert!(pipeline::finalize(&source)
        .unwrap_err()
        .iter()
        .any(|d| d.path.contains("modes[1]") && d.message.contains("target slot")));
}

#[test]
fn program_registry_checks_field_accounting() {
    let original = serde_json::to_value(pipeline::programs::registry()).unwrap();
    for case in 0..3 {
        let mut value = original.clone();
        match case {
            0 => value["rules"][0]["arity"] = 3.into(),
            1 => value["rules"][0]["recipe"]["body"]["argument"] = 0.into(),
            _ => value["rules"][0]["recipe"]["kind"] = "Optional".into(),
        }
        if case == 2 {
            value["rules"][0]["domain"] = "Actions".into();
        }
        assert!(mtgish_ast::program_spec::Registry::parse(&value.to_string()).is_err());
    }
}

#[test]
fn saved_program_recipes_verify_semantics_provenance_and_the_fourth_registry() {
    let source = custom("SpellActions(ActionList([MayAction(DrawACard)]))");
    let stage = pipeline::Intermediate::new(&source).unwrap();
    let loaded: pipeline::Intermediate =
        serde_json::from_value(serde_json::to_value(&stage).unwrap()).unwrap();
    assert_eq!(loaded.format, "mtgish.pipeline.intermediate.v8");
    assert!(pipeline::bind(&loaded.structural).is_ok());
    for case in 0..3 {
        let mut changed = loaded.structural.clone();
        match case {
            0 => {
                changed.programs[0].recipe = pipeline::programs::InstructionRecipe::Sequence(vec![])
            }
            1 => changed.programs[0].source_key = "another source".into(),
            _ => {
                changed
                    .registries
                    .remove("rust_syntax/normalize/rewrites/programs.json5");
            }
        }
        assert!(pipeline::bind(&changed).is_err());
    }
}

#[test]
fn cost_recipes_keep_conjunction_order_and_explicit_announced_alternatives() {
    let source=custom("Activated(And([PayMana([ManaCostGeneric(2)]),Or([PayLife(Integer(3)),SacrificeAPermanent(AnyPermanent)]),DiscardACardAtRandom]),ActionList([DrawACard]))");
    let card = pipeline::finalize(&source).unwrap();
    let Ability::Activated { cost, .. } = &card.abilities[0] else {
        panic!()
    };
    assert!(
        matches!(cost.parts.as_slice(),[CostPart::Mana(mana),CostPart::Choice(options),CostPart::Discard {count:Expr::Constant(1),random:true,..}] if matches!(mana.as_slice(),[ManaSymbol::Generic(2)]) && matches!(options.as_slice(),[Cost {parts:a},Cost {parts:b}] if matches!(a.as_slice(),[CostPart::Life(Expr::Constant(3))]) && matches!(b.as_slice(),[CostPart::Sacrifice {choose:Some(Count::Exactly(Expr::Constant(1))),..}])))
    );
    let binary = bincode::encode_to_vec(&card, bincode::config::standard()).unwrap();
    let (loaded, used): (Card, usize) =
        bincode::decode_from_slice(&binary, bincode::config::standard()).unwrap();
    assert_eq!(used, binary.len());
    validate::card(&loaded).unwrap();
}

#[test]
fn cost_selection_remains_strict_while_effect_selection_does_as_much_as_possible() {
    let source=custom("Activated(SacrificeNumberPermanents(Integer(2),AnyPermanent),ActionList([SacrificeNumberPermanents(Integer(2),AnyPermanent)]))");
    let card = pipeline::finalize(&source).unwrap();
    let Ability::Activated { cost, body, .. } = &card.abilities[0] else {
        panic!()
    };
    assert!(matches!(
        cost.parts.as_slice(),
        [CostPart::Sacrifice {
            choose: Some(Count::Exactly(Expr::Constant(2))),
            ..
        }]
    ));
    assert!(matches!(
        &body.instructions[0],
        Instruction::Effect {
            operation: Operation::Sacrifice {
                choose: Some(Count::AsManyAsCan(Expr::Constant(2))),
                ..
            },
            ..
        }
    ));
    let source=custom("Activated(PutNumberPermanentsIntoOwnersHand(Integer(2),AnyPermanent),ActionList([DrawACard]))");
    let card = pipeline::finalize(&source).unwrap();
    let Ability::Activated { cost, .. } = &card.abilities[0] else {
        panic!()
    };
    assert!(
        matches!(cost.parts.as_slice(),[CostPart::Move {selections,destination:Destination::OwnersHand,..}] if matches!(selections.as_slice(),[CostObjectSelection {choose:Some(Count::Exactly(Expr::Constant(2))),..}]))
    );
}

#[test]
fn cost_consolidation_retains_unknown_children_and_symbol_scope_failures() {
    use pipeline::operands::CostRecipe as R;
    let source=custom("Activated(And([PayLifeForEach(Integer(2),Integer(3)),PayItsSuspendCost]),ActionList([DrawACard]))");
    let stage = pipeline::Intermediate::new(&source).unwrap();
    let root = stage
        .structural
        .operands
        .iter()
        .find(|r| r.rule == "operand.Cost.And")
        .unwrap();
    assert!(
        matches!(&root.recipe,pipeline::operands::OperandRecipe::Cost(R::All(items)) if matches!(items.as_slice(),[R::Amount {..},R::Source(source::Cost::PayItsSuspendCost)]))
    );
    assert!(stage.final_card.is_none());
    assert!(stage
        .diagnostics
        .iter()
        .any(|d| d.form == "PayItsSuspendCost"));
    for cost in ["TapPermanent(Ref_TargetPermanent)"] {
        let source = custom(&format!("Activated({cost},ActionList([DrawACard]))"));
        let stage = pipeline::Intermediate::new(&source).unwrap();
        assert!(stage.final_card.is_none());
        assert!(stage
            .diagnostics
            .iter()
            .any(|d| d.source_type == "Cost" || d.message.contains("target slot")));
        assert!(!stage.structural.operands.is_empty());
    }
}

#[test]
fn resolving_counter_cost_uses_the_payer_and_payment_choice_has_lexical_scope() {
    let source=custom("SpellActions(Targeted([TargetPlayer(AnyPlayer)],ActionList([PlayerMayCost(Ref_TargetPlayer,RemoveCounters([ACounterOfTypeFromAPermanent(PTCounter(1,1),AnyPermanent)])),If(CostWasPaid,[DrawACard])])))");
    let card = pipeline::finalize(&source).unwrap();
    let Ability::Spell { body, .. } = &card.abilities[0] else {
        panic!()
    };
    let Instruction::Effect {
        result,
        operation:
            Operation::PayCost {
                player: PlayerRef::Target { slot: Slot(0), .. },
                cost,
                ..
            },
    } = &body.instructions[0]
    else {
        panic!()
    };
    assert!(
        matches!(cost.parts.as_slice(),[CostPart::RemoveCounters {removals}] if matches!(removals.as_slice(),[CounterRemoval {player:PlayerRef::Target {slot:Slot(0),..},selection:Some(SelectionChoice {player:PlayerRef::Target {slot:Slot(0),..},count:Count::Exactly(Expr::Constant(1))}),..}]))
    );
    assert!(
        matches!(&body.instructions[1],Instruction::If {condition:Condition::PaymentChosen(paid),..} if paid.0==result.0)
    );
    let source=custom("SpellActions(ActionList([MayAction(MayCost(PayLife(Integer(1)))),If(CostWasPaid,[DrawACard])]))");
    assert!(pipeline::finalize(&source)
        .unwrap_err()
        .iter()
        .any(|d| d.message.contains("payment reference")));
}

#[test]
fn cost_mana_and_shared_arithmetic_are_preserved_without_reducing_symbols_to_value() {
    let source=custom("Activated(And([ItsManaCost,PayMana([ManaCostW,ManaCostU,ManaCostGeneric(2)]),PayLifeForEach(Integer(2),Integer(3))]),ActionList([DrawACard]))");
    let card = pipeline::finalize(&source).unwrap();
    let Ability::Activated { cost, .. } = &card.abilities[0] else {
        panic!()
    };
    assert!(
        matches!(cost.parts.as_slice(),[CostPart::ManaCostOf(ObjectRef::Source),CostPart::Mana(mana),CostPart::Life(Expr::Constant(6))] if matches!(mana.as_slice(),[ManaSymbol::Color(ManaColor::White),ManaSymbol::Color(ManaColor::Blue),ManaSymbol::Generic(2)]))
    );
}

#[test]
fn condition_templates_preserve_quantified_player_bindings_and_source_zone_checks() {
    let source=custom("SpellActions(ActionList([If(And([IsAPlayersTurn(Opponent),APlayerPassesFilter(AnyPlayer,LifeTotalIs(GreaterThan(Integer(4)))),ThisCardIsInExileOrOnTheBattlefield]),[DrawACard])]))");
    let card = pipeline::finalize(&source).unwrap();
    let Ability::Spell { body, .. } = &card.abilities[0] else {
        panic!()
    };
    let Instruction::If {
        condition: Condition::All(conditions),
        ..
    } = &body.instructions[0]
    else {
        panic!()
    };
    assert!(
        matches!(&conditions[0],Condition::ExistsPlayer {binding:Binding(0),condition,..} if matches!(condition.as_ref(),Condition::PlayerIsActive(PlayerRef::Bound(Binding(0)))))
    );
    assert!(
        matches!(&conditions[1],Condition::ExistsPlayer {binding:Binding(0),condition,..} if matches!(condition.as_ref(),Condition::Compare {left:Expr::PlayerLife(PlayerRef::Bound(Binding(0))),comparison:Comparison::Greater,right:Expr::Constant(4)}))
    );
    assert!(
        matches!(&conditions[2],Condition::ObjectMatches {object:ObjectRef::Source,filter:ObjectFilter::Either(zones)} if matches!(zones.as_slice(),[ObjectFilter::InZone(ZoneScope::Shared(EventZone::Exile)),ObjectFilter::InZone(ZoneScope::Shared(EventZone::Battlefield))]))
    );
}

#[test]
fn condition_numeric_and_predicate_children_expand_without_announcing_new_targets() {
    let source=custom("SpellActions(Targeted([TargetGraveyardCard(AnyCardInAnyGraveyard)],ActionList([If(And([GraveyardCardPassesFilter(Ref_TargetGraveyardCard,IsCardtype(Creature)),ValueIs(NumCardsInPlayersLibrary(You),GreaterThan(Integer(2)))]),[DrawACard])])))");
    let card = pipeline::finalize(&source).unwrap();
    let Ability::Spell { announcement, body } = &card.abilities[0] else {
        panic!()
    };
    assert_eq!(announcement.targets.len(), 1);
    let Instruction::If {
        condition: Condition::All(conditions),
        ..
    } = &body.instructions[0]
    else {
        panic!()
    };
    assert!(matches!(
        &conditions[0],
        Condition::ObjectMatches {
            object: ObjectRef::Target { slot: Slot(0), .. },
            filter: ObjectFilter::CardType(source::CardType::Creature)
        }
    ));
    assert!(
        matches!(&conditions[1],Condition::Compare {left:Expr::CountObjects(objects),comparison:Comparison::Greater,right:Expr::Constant(2)} if matches!(objects.as_ref(),ObjectSet::Zone {scope:ZoneScope::Players {zone:EventZone::Library,..},..}))
    );
    let stage = pipeline::Intermediate::new(&source).unwrap();
    assert!(stage.structural.operands.iter().any(|r| matches!(
        &r.recipe,
        pipeline::operands::OperandRecipe::Condition(pipeline::operands::CheckRecipe::Compare {
            left: pipeline::Quantity::Count(_),
            ..
        })
    )));
}

#[test]
fn cost_and_condition_registry_rejects_nested_field_loss_reuse_and_wrong_domains() {
    let original = serde_json::to_value(pipeline::operands::registry()).unwrap();
    for case in 0..4 {
        let mut value = original.clone();
        match case {
            0 => value["rules"][0]["arity"] = 2.into(),
            1 => {
                let row = value["rules"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|r| r["source"] == "PayLifeForEach")
                    .unwrap();
                row["recipe"]["amount"]["right"]["argument"] = 0.into();
            }
            2 => value["rules"][0]["domain"] = "Condition".into(),
            _ => value["rules"][0]["recipe"]["ignored_field"] = true.into(),
        }
        assert!(mtgish_ast::operand_spec::Registry::parse(&value.to_string()).is_err());
    }
}

#[test]
fn saved_operand_artifacts_validate_recipe_source_and_registry_before_binding() {
    let source=custom("Activated(PayLifeForEach(Integer(2),Integer(3)),ActionList([If(IsPlayersTurn(You),[DrawACard])]))");
    let stage = pipeline::Intermediate::new(&source).unwrap();
    let loaded: pipeline::Intermediate =
        ron::from_str(&ron::ser::to_string(&stage).unwrap()).unwrap();
    assert_eq!(loaded.format, "mtgish.pipeline.intermediate.v8");
    assert_eq!(loaded.structural.registries.len(), 7);
    assert!(pipeline::bind(&loaded.structural).is_ok());
    for case in 0..3 {
        let mut changed = loaded.structural.clone();
        match case {
            0 => {
                changed.operands[0].recipe = pipeline::operands::OperandRecipe::Cost(
                    pipeline::operands::CostRecipe::All(vec![]),
                )
            }
            1 => changed.operands[0].source_key = "another operand".into(),
            _ => {
                changed
                    .registries
                    .remove("rust_syntax/normalize/rewrites/operands.json5");
            }
        }
        assert!(pipeline::bind(&changed).is_err());
    }
}

fn activation_cost(card: &Card) -> &Cost {
    let Ability::Activated { cost, .. } = &card.abilities[0] else {
        panic!()
    };
    cost
}

#[test]
fn object_tapping_costs_and_tap_untap_symbols_remain_distinct() {
    let source=custom("Activated(And([TapPermanent(ThisPermanent),UntapPermanent(ThisPermanent),TapAPermanent(AnyPermanent),UntapNumberPermanents(Integer(2),AnyPermanent)]),ActionList([DrawACard]))");
    let card = pipeline::finalize(&source).unwrap();
    assert!(matches!(activation_cost(&card).parts.as_slice(),[
        CostPart::TapSymbol,
        CostPart::ChangeTapState {tapped:false,symbol:true,selections:symbol},
        CostPart::ChangeTapState {tapped:true,symbol:false,selections:tap},
        CostPart::ChangeTapState {tapped:false,symbol:false,selections:untap},
    ] if matches!(symbol.as_slice(),[CostObjectSelection {objects:ObjectSet::One(ObjectRef::Source),choose:None}]) && matches!(tap.as_slice(),[CostObjectSelection {choose:Some(Count::Exactly(Expr::Constant(1))),..}]) && matches!(untap.as_slice(),[CostObjectSelection {choose:Some(Count::Exactly(Expr::Constant(2))),..}])));
    let source = custom("Activated(TapPermanent(HostPermanent),ActionList([DrawACard]))");
    let card = pipeline::finalize(&source).unwrap();
    assert!(
        matches!(activation_cost(&card).parts.as_slice(),[CostPart::ChangeTapState {tapped:true,symbol:false,selections}] if matches!(selections.as_slice(),[CostObjectSelection {objects:ObjectSet::One(ObjectRef::AttachedTo(_)),choose:None}]))
    );
}

#[test]
fn exact_any_and_all_object_payments_preserve_cardinality_without_announcing_targets() {
    let source=custom("Activated(And([TapNumberPermanents(ValueX,AnyPermanent),TapAnyNumberOfPermanents(AnyPermanent),UntapEachPermanent(AnyPermanent),PutAPermanentIntoItsOwnersHand(AnyPermanent)]),ActionList([DrawACard]))");
    let card = pipeline::finalize(&source).unwrap();
    let Ability::Activated {
        cost, announcement, ..
    } = &card.abilities[0]
    else {
        panic!()
    };
    assert!(announcement.targets.is_empty());
    assert!(matches!(cost.parts.as_slice(),[
        CostPart::ChangeTapState {selections:x,..},CostPart::ChangeTapState {selections:any,..},CostPart::ChangeTapState {tapped:false,selections:all,..},CostPart::Move {selections:ret,destination:Destination::OwnersHand,..}
    ] if matches!(x.as_slice(),[CostObjectSelection {choose:Some(Count::Exactly(Expr::AnnouncedX)),..}]) && matches!(any.as_slice(),[CostObjectSelection {choose:Some(Count::AnyNumber),..}]) && matches!(all.as_slice(),[CostObjectSelection {choose:None,..}]) && matches!(ret.as_slice(),[CostObjectSelection {choose:Some(Count::Exactly(Expr::Constant(1))),..}])));
}

#[test]
fn tap_payment_validation_rejects_effect_counts_wrong_zones_and_invalid_symbol_subjects() {
    let source = custom("Activated(TapAPermanent(AnyPermanent),ActionList([DrawACard]))");
    let baseline = pipeline::finalize(&source).unwrap();
    for case in 0..4 {
        let mut card = baseline.clone();
        let Ability::Activated { cost, .. } = &mut card.abilities[0] else {
            panic!()
        };
        let CostPart::ChangeTapState {
            selections, symbol, ..
        } = &mut cost.parts[0]
        else {
            panic!()
        };
        let expected = match case {
            0 => {
                selections[0].choose = Some(Count::AsManyAsCan(Expr::Constant(1)));
                "payable in full"
            }
            1 => {
                selections[0].objects = ObjectSet::Zone {
                    scope: ZoneScope::Players {
                        zone: EventZone::Graveyard,
                        players: PlayerSet::All,
                    },
                    kind: ObjectKind::Card,
                    filter: ObjectFilter::Any,
                };
                "Battlefield"
            }
            2 => {
                *symbol = true;
                "implicit source"
            }
            _ => {
                selections.clear();
                "no object selections"
            }
        };
        assert!(validate::card(&card).unwrap_err().contains(expected));
    }
    let source = custom("Activated(UntapPermanent(ThisPermanent),ActionList([DrawACard]))");
    let mut card = pipeline::finalize(&source).unwrap();
    let Ability::Activated { cost, .. } = &mut card.abilities[0] else {
        panic!()
    };
    let CostPart::ChangeTapState { selections, .. } = &mut cost.parts[0] else {
        panic!()
    };
    selections[0].objects = ObjectSet::One(ObjectRef::AttachedTo(Box::new(ObjectRef::Source)));
    assert!(validate::card(&card)
        .unwrap_err()
        .contains("implicit source"));
}

#[test]
fn resolving_object_payments_keep_their_actor_and_payment_aliases_require_a_binding() {
    let source=custom("SpellActions(Targeted([TargetPlayer(AnyPlayer),TargetPermanent(AnyPermanent)],ActionList([PlayerMayCost(Ref_TargetPlayer,TapPermanent(Ref_TargetPermanent)),If(CostWasPaid,[DrawACard])])))");
    let card = pipeline::finalize(&source).unwrap();
    let Ability::Spell { body, .. } = &card.abilities[0] else {
        panic!()
    };
    assert!(
        matches!(&body.instructions[0],Instruction::Effect {operation:Operation::PayCost {player:PlayerRef::Target {slot:Slot(0),..},cost,..},..} if matches!(cost.parts.as_slice(),[CostPart::ChangeTapState {symbol:false,tapped:true,selections}] if matches!(selections.as_slice(),[CostObjectSelection {objects:ObjectSet::One(ObjectRef::Target {slot:Slot(1),..}),choose:None}])))
    );
    let source = custom(
        "Activated(TapAPermanent(AnyPermanent),ActionList([TapPermanent(ThePermanentTappedThisWay)]))",
    );
    let card = pipeline::finalize(&source).unwrap();
    validate::card(&card).unwrap();
    let unbound = custom("SpellActions(ActionList([TapPermanent(ThePermanentTappedThisWay)]))");
    assert!(pipeline::finalize(&unbound).is_err());
}

#[test]
fn tap_costs_preserve_mana_ability_classification_and_granted_symbol_host_scope() {
    let source = custom(
        "Activated(TapAPermanent(IsCardtype(Creature)),ActionList([AddMana(ManaProduceW)]))",
    );
    let card = pipeline::finalize(&source).unwrap();
    let Ability::Activated {
        cost,
        announcement,
        body,
        ..
    } = &card.abilities[0]
    else {
        panic!()
    };
    assert_eq!(
        mtgish_ast::semantics::activation_kind(cost, announcement, body),
        mtgish_ast::semantics::ActivationKind::Mana
    );
    let source=custom("SpellActions(ActionList([CreatePermanentLayerEffectUntil(ThisPermanent,[AddAbility([Ward(UntapPermanent(ThisPermanent))])],UntilEndOfTurn)]))");
    let card = pipeline::finalize(&source).unwrap();
    let Ability::Spell { body, .. } = &card.abilities[0] else {
        panic!()
    };
    assert!(
        matches!(&body.instructions[0],Instruction::Effect {operation:Operation::CreateContinuousEffect {changes,..},..} if matches!(changes.as_slice(),[Change::GrantKeyword(Keyword::Ward(cost))] if matches!(cost.parts.as_slice(),[CostPart::ChangeTapState {tapped:false,symbol:true,selections}] if matches!(selections.as_slice(),[CostObjectSelection {objects:ObjectSet::One(ObjectRef::KeywordHost),choose:None}]))))
    );
}

#[test]
fn schema_46_costs_roundtrip_and_reject_previous_versions_at_validation() {
    let source=custom("Activated(And([UntapPermanent(ThisPermanent),TapAPermanent(AnyPermanent)]),ActionList([DrawACard]))");
    let card = pipeline::finalize(&source).unwrap();
    assert_eq!(card.schema_version, 139);
    let json = serde_json::to_value(&card).unwrap();
    let bytes = bincode::encode_to_vec(&card, bincode::config::standard()).unwrap();
    let (binary, used): (Card, usize) =
        bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
    assert_eq!(used, bytes.len());
    for loaded in [
        binary,
        ron::from_str(&ron::ser::to_string(&card).unwrap()).unwrap(),
        serde_json::from_value(json.clone()).unwrap(),
    ] {
        validate::card(&loaded).unwrap();
        assert_eq!(serde_json::to_value(loaded).unwrap(), json);
    }
    let mut previous = card;
    previous.schema_version = 23;
    assert!(validate::card(&previous).is_err());
}

#[test]
fn real_object_payment_cards_roundtrip_without_dropping_other_missing_abilities() {
    let mut complete = 0;
    for line in include_str!("fixtures/object_payments.ronl").lines() {
        let source: source::OracleCard = ron::from_str(line).unwrap();
        let name = corpus::Identity::of(&source).unwrap().names.remove(0);
        let card =
            pipeline::finalize(&source).unwrap_or_else(|errors| panic!("{name}: {errors:?}"));
        let expected = serde_json::to_value(&card).unwrap();
        let bytes = bincode::encode_to_vec(&card, bincode::config::standard()).unwrap();
        let (binary, used): (Card, usize) =
            bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
        assert_eq!(used, bytes.len());
        for loaded in [
            binary,
            ron::from_str(&ron::ser::to_string(&card).unwrap()).unwrap(),
            serde_json::from_value(expected.clone()).unwrap(),
        ] {
            validate::card(&loaded).unwrap();
            assert_eq!(serde_json::to_value(loaded).unwrap(), expected, "{name}");
        }
        if name == "Pili-Pala" || name == "Heritage Druid" {
            let tap = card
                .abilities
                .iter()
                .filter_map(|a| match a {
                    Ability::Activated { cost, .. } => Some(cost),
                    _ => None,
                })
                .flat_map(|cost| &cost.parts)
                .find(|p| matches!(p, CostPart::ChangeTapState { .. }))
                .unwrap();
            let CostPart::ChangeTapState {
                symbol,
                tapped,
                selections,
            } = tap
            else {
                panic!()
            };
            if name == "Pili-Pala" {
                assert!(*symbol && !*tapped);
            } else {
                assert!(!*symbol && *tapped);
                assert!(matches!(
                    selections.as_slice(),
                    [CostObjectSelection {
                        choose: Some(Count::Exactly(Expr::Constant(3))),
                        ..
                    }]
                ));
            }
        }
        complete += 1;
    }
    assert_eq!(complete, 24);
}

#[test]
fn appended_tap_payment_keeps_existing_cost_binary_discriminants() {
    assert_eq!(
        bincode::encode_to_vec(CostPart::TapSymbol, bincode::config::standard()).unwrap(),
        vec![5]
    );
    assert_eq!(
        bincode::encode_to_vec(CostPart::Mana(vec![]), bincode::config::standard()).unwrap(),
        vec![4, 0]
    );
    let part = CostPart::ChangeTapState {
        selections: vec![CostObjectSelection {
            objects: ObjectSet::One(ObjectRef::Source),
            choose: None,
        }],
        tapped: false,
        symbol: true,
    };
    let bytes = bincode::encode_to_vec(&part, bincode::config::standard()).unwrap();
    assert_eq!(bytes[0], 12);
    let (loaded, used): (CostPart, usize) =
        bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
    assert_eq!(used, bytes.len());
    assert_eq!(
        serde_json::to_value(loaded).unwrap(),
        serde_json::to_value(part).unwrap()
    );
}

fn ability_json(rule: &str) -> String {
    let card = pipeline::finalize(&custom(rule)).unwrap();
    serde_json::to_string(&card.abilities[0]).unwrap()
}

#[test]
fn a_search_keeps_every_zone_it_looks_through_and_whether_each_is_optional() {
    let text = ability_json("SpellActions(ActionList([SearchLibraryAndOrGraveyard([FindAGenericCard,PutFoundCardsIntoHand,Shuffle])]))");
    assert!(
        text.contains(r#""also":["Graveyard"],"either":true"#),
        "{text}"
    );
    let text = ability_json("SpellActions(ActionList([SearchLibraryAndGraveyardAndHand([FindAGenericCard,PutFoundCardsIntoHand,Shuffle])]))");
    assert!(
        text.contains(r#""also":["Graveyard","Hand"],"either":false"#),
        "{text}"
    );
    let text = ability_json("SpellActions(ActionList([SearchLibrary([FindAGenericCard,PutFoundCardsIntoHand,Shuffle])]))");
    assert!(text.contains(r#""also":[],"either":false"#), "{text}");
}

#[test]
fn a_die_result_reads_the_roll_in_its_scope() {
    let text = ability_json("SpellActions(ActionList([RollAD20,GainLife(TheDiceResult)]))");
    assert!(text.contains("DieResult"), "{text}");
    assert!(pipeline::finalize(&custom(
        "SpellActions(ActionList([GainLife(TheDiceResult)]))"
    ))
    .is_err());
}

#[test]
fn a_next_spell_trigger_ends_once_it_has_triggered() {
    let text = ability_json("SpellActions(ActionList([CreateFutureTrigger(WhenPlayerCastsTheirNextSpellThisTurn(You,AnySpell),ActionList([DrawACard]))]))");
    assert!(text.contains(r#""once":true"#), "{text}");
    assert!(text.contains("EndOfTurn"), "{text}");
}

#[test]
fn cant_attack_unless_asks_about_each_defending_player() {
    let text = ability_json("PermanentRuleEffect(ThisPermanent,[CantAttackUnlessDefendingPlayer(PlayerPassesFilter(DefendingPlayer,ControlsA(IsLandType(Island))))])");
    assert!(text.contains("PlayersAndPlaneswalkers"), "{text}");
    assert!(text.contains("Matching"), "{text}");
}

#[test]
fn entwine_branches_between_one_mode_and_both() {
    let card = pipeline::finalize(&custom("SpellActions(Modal_ChooseOne_Entwine(PayMana([ManaCostG]),[ActionList([DrawACard]),ActionList([GainLife(Integer(2))])]))")).unwrap();
    let Ability::CostBranch {
        keyword,
        paid,
        unpaid,
        ..
    } = &card.abilities[0]
    else {
        panic!()
    };
    assert_eq!(keyword, "Entwine");
    let modes = |ability: &Ability| {
        let Ability::Spell { announcement, .. } = ability else {
            panic!()
        };
        let selection = announcement.modes.as_ref().unwrap();
        matches!(selection.minimum, Expr::Constant(n) if n == selection.modes.len() as i64)
    };
    assert!(modes(paid));
    assert!(!modes(unpaid));
}

#[test]
fn an_exile_paid_as_a_cost_is_read_in_exile_and_a_linked_exile_is_read_by_source() {
    let paid = ability_json("TriggerA(WhenAPermanentEntersTheBattlefield(SinglePermanent(ThisPermanent)),ActionList([MayCost(Exile([TheTopCardOfLibrary],[])),If(CostWasPaid,[PutExiledCardOntoBattlefield(TheCardExiledThisWay,[EntersNormally])])]))");
    assert!(paid.contains(r#""kind":"CostObjects""#), "{paid}");
    assert!(
        paid.contains(r#""view":"Performed","phase":"After""#),
        "{paid}"
    );

    let linked = ability_json("TriggerA(WhenAPermanentLeavesTheBattlefield(SinglePermanent(ThisPermanent)),ActionList([PutExiledCardsIntoOwnersHand(TheExiledCards)]))");
    assert!(linked.contains(r#""ExiledWith""#), "{linked}");

    // "This way" means this scope, so it never reaches another ability's exile.
    let strict = custom("TriggerA(WhenAPermanentLeavesTheBattlefield(SinglePermanent(ThisPermanent)),ActionList([PutExiledCardsIntoOwnersHand(TheCardsExiledThisWay)]))");
    assert!(pipeline::finalize(&strict).is_err());
}

#[test]
fn a_manifested_or_single_created_permanent_is_the_permanent_put_there_this_way() {
    let manifested = ability_json("SpellActions(ActionList([ManifestTheTopCardOfPlayersLibrary(You),PutCounters([ACounterOfTypeOnPermanent(PTCounter(+1,+1),ThePermanentPutOnTheBattlefieldThisWay)])]))");
    assert!(manifested.contains(r#""kind":"Result""#), "{manifested}");

    let two = custom("SpellActions(ActionList([CreateTokens([NumberTokens(Integer(2),TokenWithPT(PT(1,1),Colors([Black]),[],[Creature],SubtypesList([Horror]),[]))],[]),PutCounters([ACounterOfTypeOnPermanent(PTCounter(+1,+1),ThePermanentPutOnTheBattlefieldThisWay)])]))");
    assert!(pipeline::finalize(&two).is_err());
}

#[test]
fn abilities_are_targets_on_the_stack_with_their_filter_kept() {
    let text = ability_json("SpellActions(Targeted([TargetAbility(And([ActivatedAbility,ControlledByAPlayer(SinglePlayer(You))]))],ActionList([CounterAbility(Ref_TargetAbility)])))");
    assert!(
        text.contains(r#""kind":{"Object":{"kind":"Ability","zones":["Stack"]}}"#)
            || text.contains(r#""kind":"Ability""#),
        "{text}"
    );
    assert!(text.contains(r#""kind":"ControlledBy""#), "{text}");
}

#[test]
fn removing_counters_as_a_cost_is_counted_from_that_cost() {
    let text = ability_json("Activated(RemoveCounters([NumberCountersOfTypeFromPermanent(Integer(3),ChargeCounter,ThisPermanent)]),ActionList([DrawNumberCards(TheNumberOfCountersRemovedThisWay)]))");
    assert!(text.contains("PaidCountersRemoved"), "{text}");
}

#[test]
fn set_card_type_replaces_and_except_for_negates() {
    let set = ability_json("PermanentLayerEffect(ThisPermanent,[SetCardtype(Land)])");
    assert!(set.contains("SetCardTypes"), "{set}");
    assert!(!set.contains("AddCardType"), "{set}");
    let except = ability_json(
        "EachPermanentRuleEffect(And([ExceptFor(IsToken),IsCardtype(Creature)]),[CantAttack])",
    );
    assert!(except.contains(r#""Not""#), "{except}");
}

#[test]
fn a_returned_graveyard_card_is_chosen_once_and_moved_to_its_owners_hand() {
    let text = ability_json(
        "SpellActions(ActionList([PutACardFromGraveyardIntoHand(IsCardtype(Creature))]))",
    );
    assert!(text.contains("ChooseObjects"), "{text}");
    assert!(text.contains("OwnersHand"), "{text}");
}

#[test]
fn a_target_that_decides_its_mode_announces_no_mode_and_a_shuffle_is_one_destination() {
    let card = pipeline::finalize(&custom("SpellActions(Targeted_Modal([Targeted([TargetSpell(AnySpell)],ActionList([ReturnSpellToOwnersHand(Ref_TargetSpell)])),Targeted([TargetPermanent(AnyPermanent)],ActionList([ShufflePermanentIntoLibrary(Ref_TargetPermanent)]))]))")).unwrap();
    let Ability::Spell { announcement, .. } = &card.abilities[0] else {
        panic!()
    };
    let modes = announcement.modes.as_ref().unwrap();
    assert!(modes.implied);
    assert_eq!(modes.modes.len(), 2);
    let text = serde_json::to_string(&modes.modes).unwrap();
    assert!(text.contains("OwnersHand"), "{text}");
    assert!(text.contains("OwnersLibraryShuffled"), "{text}");
}

#[test]
fn counts_and_primitive_rule_families_keep_their_operands() {
    let text = ability_json("SpellActions(Targeted([OneOrTwoTargetPermanents(AnyPermanent)],ActionList([DestroyPermanent(Ref_TargetPermanent)])))");
    assert!(text.contains(r#""Between""#), "{text}");
    let text = ability_json(
        "Activated(TapPermanent(ThisPermanent),ActionList([InvestigateTimes(Integer(2))]))",
    );
    assert!(text.contains(r#""action":"Investigate""#), "{text}");
    assert!(
        text.contains(r#""Integer":2"#) || text.contains("2"),
        "{text}"
    );
}

#[test]
fn this_way_histories_read_the_action_that_did_it() {
    let text =
        ability_json("SpellActions(ActionList([LoseLife(Integer(3)),GainLife(LifeLostThisWay)]))");
    assert!(text.contains(r#""Performed""#), "{text}");
    let text = ability_json("SpellActions(Targeted([TargetPermanent(AnyPermanent)],ActionList([DestroyPermanent(Ref_TargetPermanent),If(APermanentWasDestroyedThisWay(IsCardtype(Creature)),[DrawACard])])))");
    assert!(text.contains(r#""Transition""#), "{text}");
    let unbound = custom("SpellActions(ActionList([GainLife(LifeLostThisWay)]))");
    assert!(pipeline::finalize(&unbound).is_err());
}

#[test]
fn any_player_may_pay_is_offered_and_the_payer_pays_for_themselves() {
    let text = ability_json("SpellActions(ActionList([AnyPlayerMayCost(AnyPlayer,SacrificeAPermanent(IsCardtype(Land))),If(CostWasPaid,[DrawACard])]))");
    assert!(text.contains("FirstWilling"), "{text}");
    assert!(text.contains(r#""optional":true"#), "{text}");
}

#[test]
fn a_replacement_from_anywhere_works_in_every_zone() {
    let text = ability_json("AsPutIntoAGraveyardFromAnywhere(ThisCard,[ExileItInstead])");
    assert!(text.contains("Everywhere"), "{text}");
    assert!(text.contains("SetDestination"), "{text}");
    assert!(text.contains(r#""Graveyard""#), "{text}");
}

#[test]
fn a_cost_reduction_floors_at_zero_with_a_maximum() {
    let text = ability_json("CastEffect(ReduceCastingCostX([CostReduceX],MaximumOf(Integer(0),Minus(StartingLifeTotalOfPlayer(You),LifeTotalOfPlayer(You)))))");
    assert!(text.contains(r#""Maximum""#), "{text}");
}

#[test]
fn free_and_flash_are_one_permission() {
    let text = ability_json("CastEffect(MayCastWithoutPayingAndAsThoughItHadFlashIf(PlayerPassesFilter(You,IsTheMonarch)))");
    assert!(text.contains("AlternativeCostWithFlash"), "{text}");
    assert!(!text.contains(r#""Flash""#), "{text}");
}

#[test]
fn any_other_color_leaves_colorless_mana_alone() {
    let text = ability_json("SpellActions(ActionList([ChooseAColor(AnyColor),CreateReplaceWouldProduceManaUntil(TappingAPermanentWouldProduceMana(IsCardtype(Land)),[WouldProduceMana_ReplaceColorWithTheChosenColor],UntilEndOfTurn)]))");
    assert!(text.contains("ReplaceColoredMana"), "{text}");
}

#[test]
fn a_card_put_from_hand_as_a_cost_comes_from_the_payers_hand() {
    let text = ability_json("SpellActions(ActionList([MayCost(PutACardFromHandOnBattlefield(IsCardtype(Land),[EntersTapped])),If(CostWasPaid,[DrawACard])]))");
    assert!(text.contains(r#""Move""#), "{text}");
    assert!(text.contains(r#""Hand""#), "{text}");
    assert!(text.contains(r#""optional":true"#), "{text}");
}

#[test]
fn split_damage_is_one_batch_and_each_part_keeps_its_target() {
    let text = ability_json("SpellActions(Targeted([TargetPermanent(IsCardtype(Creature)),TargetPermanent(IsCardtype(Land))],ActionList([SpellDealsMultipleDamage(ThisSpell,[DamageToRecipients(Integer(2),Permanent(Ref_TargetPermanent1)),DamageToRecipients(Integer(3),Permanent(Ref_TargetPermanent2))])])))");
    assert!(text.contains("DealDamageParts"), "{text}");
    assert!(!text.contains(r#""DealDamage""#), "{text}");
    let parts = text.split(r#""recipients""#).skip(1).collect::<Vec<_>>();
    assert_eq!(parts.len(), 2, "{text}");
    assert_ne!(
        parts[0].split(r#""amount""#).next(),
        parts[1].split(r#""amount""#).next(),
        "{text}"
    );
}

#[test]
fn each_source_reads_itself_for_its_own_damage() {
    let text = ability_json("SpellActions(ActionList([EachPermanentDealsDamage(IsCardtype(Creature),PowerOfPermanent(EachablePermanent),Permanent(EachablePermanent))]))");
    assert!(text.contains("DealDamageFromEach"), "{text}");
    assert!(text.contains("Affected"), "{text}");
}

#[test]
fn shuffling_a_graveyard_in_moves_every_card_then_shuffles() {
    let text = ability_json("SpellActions(ActionList([ShuffleHandAndGraveyardIntoLibrary]))");
    assert!(text.contains(r#""Move""#), "{text}");
    assert!(text.contains(r#""Shuffle""#), "{text}");
    assert!(
        text.contains(r#""Hand""#) && text.contains(r#""Graveyard""#),
        "{text}"
    );
}

#[test]
fn a_later_filter_reads_the_targets_and_target_players_are_a_set() {
    let text = ability_json("SpellActions(Targeted([UptoNumberTargetPermanents(Integer(3),IsCardtype(Creature))],ActionList([DestroyEachPermanent(And([Ref_TargetPermanents,IsCreatureType(Wall)]))])))");
    assert!(text.contains("InSet"), "{text}");
    let text = ability_json("SpellActions(Targeted([AnyNumberOfTargetPlayers(AnyPlayer)],ActionList([EachPlayerAction(Ref_TargetPlayers,DrawACard)])))");
    assert!(text.contains("Targets"), "{text}");
}

#[test]
fn losing_a_card_type_and_gaining_a_supertype_are_changes() {
    let text = ability_json(
        "PermanentLayerEffect(ThisPermanent,[RemoveCardtype(Creature),AddSupertype(Legendary)])",
    );
    assert!(text.contains("RemoveCardType"), "{text}");
    assert!(text.contains("AddSupertype"), "{text}");
}

#[test]
fn x_paid_during_resolution_is_read_from_that_payment() {
    let text = ability_json("SpellActions(ActionList([MayCost(PayManaAnyX([ManaCostX])),If(CostWasPaid,[GainLife(ValueX)])]))");
    assert!(
        text.contains(r#""ManaWithX":{"mana":["X"],"x":null}"#),
        "{text}"
    );
    assert!(text.contains("BoundValue"), "{text}");
    assert!(!text.contains("AnnouncedX"), "{text}");
}

#[test]
fn a_damage_cost_keeps_its_recipient_bound() {
    let text = ability_json("SpellActions(ActionList([MayCost(HavePermanentDealDamage(ThisPermanent,Integer(2),Player(You))),If(CostWasPaid,[DrawACard])]))");
    assert!(text.contains("DealDamage"), "{text}");
    assert!(
        text.contains("\"recipients\":{\"kind\":\"Players\""),
        "{text}"
    );
}

#[test]
fn group_targets_keep_their_shared_requirement() {
    let text = ability_json("SpellActions(Targeted([UptoNumberTargetGroupGraveyardCards(Integer(2),AnyCardInAnyGraveyard,ShareAGraveyard)],ActionList([Exile([EachCardInGraveyards(Ref_TargetGraveyardCards)],[])])))");
    assert!(text.contains("Owners"), "{text}");
    let text = ability_json("SpellActions(Targeted([NumberTargetGroupPermanents(Integer(2),IsCardtype(Creature),ControlledByDifferentPlayers)],ActionList([DestroyEachPermanent(Ref_TargetPermanents)])))");
    assert!(text.contains("DifferentControllers"), "{text}");
}

#[test]
fn a_shared_name_is_read_from_the_named_object() {
    let text = ability_json("SpellActions(Targeted([TargetPermanent(AnyPermanent)],ActionList([DestroyEachPermanent(SharesANameWithPermanent(Ref_TargetPermanent))])))");
    assert!(text.contains("SharesNameWith"), "{text}");
}

#[test]
fn choosing_both_depends_on_the_condition() {
    let text = ability_json("SpellActions(Modal_ChooseOneOrBothIf(PlayerPassesFilter(You,LifeTotalIs(EqualTo(Integer(13)))),[ActionList([DrawACard]),ActionList([GainLife(Integer(1))])]))");
    assert!(text.contains("Conditional"), "{text}");
}

#[test]
fn a_block_event_names_each_creature_of_the_pair() {
    let text = ability_json("TriggerA(WhenACreatureBecomesBlockedByACreature(SinglePermanent(ThisPermanent),IsCardtype(Creature)),ActionList([DestroyPermanent(Trigger_TheBlockingCreature)]))");
    assert!(text.contains("Blocker"), "{text}");
    let text = ability_json("TriggerA(WhenACreatureBlocksACreature(SinglePermanent(ThisPermanent),IsCardtype(Creature)),ActionList([DestroyPermanent(Trigger_ThatOtherPermanent)]))");
    assert!(text.contains("BlockPair"), "{text}");
    assert!(text.contains("Other"), "{text}");
}

#[test]
fn a_player_who_refuses_the_cost_takes_the_fallback() {
    let text = ability_json("SpellActions(ActionList([EachPlayerMayCostOrFallback(Opponent,[DiscardACardOfType(AnyCard)],LoseLife(Integer(3)))]))");
    assert!(text.contains("ForEachPlayer"), "{text}");
    assert!(text.contains("PaymentChosen"), "{text}");
    assert!(text.contains("Not"), "{text}");
}

#[test]
fn token_copies_of_an_exiled_card_and_of_each_permanent() {
    let text = ability_json("SpellActions(ActionList([Exile([TheTopCardOfLibrary],[]),CreateTokens([TokenCopyOfExiledCard(TheCardExiledThisWay,NoTokenCopyEffects)],[])]))");
    assert!(text.contains("Copy"), "{text}");
    let text = ability_json("SpellActions(ActionList([CreateTokens([TokenCopyOfEachPermanent(IsCardtype(Creature),NoTokenCopyEffects)],[])]))");
    assert!(text.contains("CopyEach"), "{text}");
}

#[test]
fn the_chosen_color_is_a_change_of_its_own() {
    let text = ability_json("PermanentLayerEffect(ThisPermanent,[SetColor(TheChosenColor)])");
    assert!(text.contains("ChosenColor"), "{text}");
}

#[test]
fn the_revealed_card_and_the_exiled_card_are_read_after_their_action() {
    let text = ability_json("SpellActions(ActionList([RevealTopCardOfLibrary,PutTopOfLibraryInHand,LoseLife(ManaValueOfTheCardRevealedThisWay)]))");
    assert!(text.contains("ManaValue"), "{text}");
    let text = ability_json("SpellActions(ActionList([Exile([TheTopCardOfLibrary],[]),If(ExiledCardPassesFilter(TheCardExiledThisWay,IsCardtype(Land)),[DrawACard])]))");
    assert!(text.contains("ObjectMatches"), "{text}");
}

#[test]
fn a_top_card_moves_anywhere_and_any_number_of_cards_are_chosen() {
    let text = ability_json("SpellActions(ActionList([RevealTopCardOfLibrary,PutTopOfLibraryOnBattlefield([EntersTapped])]))");
    assert!(text.contains("Battlefield"), "{text}");
    let text = ability_json("SpellActions(ActionList([DiscardAnyNumberOfCards,DrawNumberCards(NumCardsDiscardedThisWay)]))");
    assert!(text.contains("DiscardObjects"), "{text}");
    assert!(text.contains("AnyNumber"), "{text}");
}

#[test]
fn exiling_until_a_card_reads_only_the_card_that_stopped_it() {
    let text = ability_json("SpellActions(ActionList([Exile([CardsFromTheTopOfLibraryUntilACardOfTypeIsExiled(IsNonCardtype(Land))],[]),MayAction(PutExiledCardIntoOwnersHand(TheSpecificCardExiledThisWay))]))");
    assert!(text.contains("RevealUntil"), "{text}");
    assert!(text.contains("Matching"), "{text}");
}

#[test]
fn a_boon_is_a_trigger_that_lasts_until_used_once() {
    let text = ability_json("SpellActions(ActionList([GetOneTimeBoon(WhenAPlayerCastsASpell(SinglePlayer(You),IsCardtype(Creature)),ActionList([DrawACard]))]))");
    assert!(text.contains("Indefinite"), "{text}");
    assert!(text.contains("\"once\":true"), "{text}");
}

#[test]
fn players_act_starting_with_the_named_player() {
    let text = ability_json(
        "SpellActions(ActionList([EachPlayerStartingWithAction(AnyPlayer,You,[DrawACard])]))",
    );
    assert!(text.contains("starting_with"), "{text}");
    assert!(!text.contains("\"starting_with\":null"), "{text}");
}

#[test]
fn an_optional_additional_cost_branches_the_spell() {
    let text = ability_json("SpellActions(AdditionalCost_Modal([AdditionalCost(PayLife(Integer(2)),ActionList([DrawNumberCards(Integer(2))])),NoAdditionalCost(ActionList([DrawACard]))]))");
    assert!(text.contains("CostBranch"), "{text}");
}

#[test]
fn damage_read_from_each_recipient_is_worked_out_per_player() {
    let text = ability_json("SpellActions(ActionList([SpellDealsDamage(ThisSpell,LifeTotalOfPlayer(DealsDamage_ThatPlayer),EachPlayer(AnyPlayer))]))");
    assert!(text.contains("ForEachPlayer"), "{text}");
    let text = ability_json("SpellActions(Targeted([TargetPlayer(AnyPlayer)],ActionList([SpellDealsDamage(ThisSpell,TheNumberOfCardsInPlayersHand(DealsDamage_ThatPlayer),Player(Ref_TargetPlayer))])))");
    assert!(!text.contains("ForEachPlayer"), "{text}");
}

#[test]
fn defending_player_is_the_attacker_s_unless_it_limits_an_activation() {
    let text = ability_json("If(PlayerPassesFilter(DefendingPlayer,ControlsA(IsCardtype(Artifact))),[PermanentRuleEffect(ThisPermanent,[CantBeBlocked])])");
    assert!(text.contains("DefendingPlayerOf"), "{text}");
}

#[test]
fn another_player_acting_can_be_a_cost() {
    let text = ability_json("SpellActions(Targeted([TargetPlayer(Opponent)],ActionList([MayCost(HavePlayerTakeAction(Ref_TargetPlayer,DrawACard)),If(CostWasPaid,[GainLife(Integer(2))])])))");
    assert!(text.contains("PlayerAction"), "{text}");
    assert!(text.contains("PaymentChosen"), "{text}");
}

#[test]
fn a_chosen_ability_is_a_change_after_the_choice() {
    let text = ability_json("SpellActions(ActionList([ChooseAnAbility([Flying,Haste]),CreatePermanentLayerEffectUntil(ThisPermanent,[AddAbilityVariable(TheChosenAbility)],UntilEndOfTurn)]))");
    assert!(text.contains(r#""kind":"Ability","count":1"#), "{text}");
    assert!(text.contains("ChosenAbility"), "{text}");
}

#[test]
fn discarding_a_chosen_card_and_returning_a_chosen_card() {
    let text = ability_json("SpellActions(Targeted([TargetPlayer(AnyPlayer)],ActionList([PlayerAction(Ref_TargetPlayer,RevealHand),ChooseACardFromPlayersRevealedHand(AnyCard,Ref_TargetPlayer),PlayerAction(Ref_TargetPlayer,DiscardCard(TheCardInHandChosenThisWay))])))");
    assert!(text.contains("DiscardObjects"), "{text}");
    let text = ability_json("SpellActions(ActionList([ReturnACardFromGraveyardToBattlefield(IsCardtype(Creature),[EntersTapped])]))");
    assert!(text.contains("ChooseObjects"), "{text}");
}

#[test]
fn modes_can_exclude_those_already_chosen() {
    let text = ability_json("SpellActions(Modal_ChooseOneThatHasntBeenChosenThisTurn([ActionList([DrawACard]),ActionList([GainLife(Integer(1))])]))");
    assert!(text.contains("ThisTurn"), "{text}");
}

#[test]
fn revealed_cards_are_counted_and_tokens_are_made_per_player() {
    let text = ability_json("SpellActions(ActionList([RevealAnyNumberOfCardsOfTypeFromHand(AnyCard),GainLife(TheNumberOfCardsInHandRevealedThisWay)]))");
    assert!(text.contains("CountObjects"), "{text}");
    let text = ability_json("SpellActions(ActionList([CreateTokensForEachPlayer(Opponent,[NumberTokens(Integer(1),TreasureToken)],[])]))");
    assert!(text.contains("ForEachPlayer"), "{text}");
}

#[test]
fn a_prevention_shield_can_remove_counters_for_the_damage_prevented() {
    let text = ability_json("PreventDamage(DamageWouldBeDealtToRecipient(Permanent(ThisPermanent)),[PreventThatDamage,RemoveCounters([NumberCountersOfTypeFromPermanent(TheAmountOfDamagePreventedThisWay,PTCounter(1,1),ThisPermanent)])])");
    assert!(text.contains("RemoveCounters"), "{text}");
}

#[test]
fn x_can_be_defined_by_the_cost_itself() {
    let text =
        ability_json("Encore(PayManaX([ManaCostX],ManaValueOfGraveyardCard(ThisGraveyardCard)))");
    assert!(text.contains("ManaWithX"), "{text}");
}

#[test]
fn escalate_charges_each_mode_past_the_first() {
    let text = ability_json("SpellActions(Modal_ChooseOneOrMore_Escalate(PayMana([ManaCostGeneric(1)]),[ActionList([DrawACard]),ActionList([GainLife(Integer(2))])]))");
    assert!(text.contains("surcharge"), "{text}");
    assert!(!text.contains("\"surcharge\":null"), "{text}");
}

#[test]
fn any_other_target_excludes_one_object() {
    let text = ability_json("Activated(TapPermanent(ThisPermanent),Targeted([AnyTargetExceptPermanent(ThisPermanent)],ActionList([PermanentDealsDamage(ThisPermanent,Integer(1),Ref_AnyTarget)])))");
    assert!(text.contains("DamageRecipient"), "{text}");
    assert!(text.contains("SameObject"), "{text}");
}

#[test]
fn a_damage_recipient_can_be_tested_as_a_permanent() {
    let text = ability_json("SpellActions(Targeted([AnyTarget],ActionList([SpellDealsDamage(ThisSpell,Integer(3),Ref_AnyTarget),If(PermanentPassesFilter(AnyTargetAsAPermanent,IsCardtype(Creature)),[DrawACard])])))");
    assert!(text.contains("RecipientObject"), "{text}");
}

#[test]
fn another_player_can_choose_a_target() {
    let text = ability_json("TriggerA(WhenACreatureDealsCombatDamageToAPlayer(SinglePermanent(ThisPermanent),AnyPlayer),Targeted([TargetPermanentOfPlayersChoice(IsCardtype(Land),Trigger_ThatPlayer)],ActionList([DestroyPermanent(Ref_TargetPermanent)])))");
    assert!(text.contains("\"chooser\":{"), "{text}");
}

#[test]
fn a_group_event_names_its_objects_and_a_targeting_event_its_source() {
    let text = ability_json("TriggerA(WhenAPlayerAttacksWithANumberOfCreatures(SinglePlayer(You),GreaterThanOrEqualTo(Integer(2)),IsCardtype(Creature)),ActionList([PutCounters([ACounterOfTypeOnEachPermanent(PTCounter(+1,+1),Trigger_ThoseCreatures)])]))");
    assert!(text.contains("EventGroup"), "{text}");
    let text = ability_json("TriggerA(WhenAPermanentBecomesTheTargetOfASpellOrAbility(SinglePermanent(ThisPermanent),ControlledByAPlayer(Opponent)),ActionList([PlayerMayCost(ControllerOfSpellOrAbility(Trigger_ThatSpellOrAbility),PayMana([ManaCostGeneric(2)])),Unless(CostWasPaid,[CounterSpellOrAbility(Trigger_ThatSpellOrAbility)])]))");
    assert_eq!(text.matches("EventStackObject").count(), 2, "{text}");
}

#[test]
fn an_effect_can_last_until_a_player_pays() {
    let text = ability_json("Activated(TapPermanent(ThisPermanent),ActionList([CreatePermanentLayerEffectUntil(ThisPermanent,[AddCardtype(Enchantment)],UntilPlayerPaysMana(You,[ManaCostW]))]))");
    assert!(text.contains("UntilPaid"), "{text}");
}

#[test]
fn a_process_repeats_a_set_number_of_times() {
    let text = ability_json("SpellActions(ActionList([RepeatableActionsNumTimes(Plus(Integer(1),ValueX),[DrawACard])]))");
    assert!(text.contains("Times"), "{text}");
}

#[test]
fn damage_can_go_to_what_a_creature_attacks() {
    let text = ability_json("TriggerA(WhenACreatureAttacks(SinglePermanent(ThisPermanent)),ActionList([PermanentDealsDamage(ThisPermanent,Integer(1),PlayerOrPlaneswalkerPermanentIsAttacking(Trigger_ThatCreature))]))");
    assert!(text.contains("AttackedBy"), "{text}");
}

#[test]
fn up_to_one_card_from_each_graveyard_is_one_slot_with_distinct_owners() {
    let text = ability_json("SpellActions(Targeted([UptoOneTargetGraveyardCardInEachPlayersGraveyard(IsCardtype(Creature),AnyPlayer)],ActionList([DrawACard])))");
    assert!(text.contains("DifferentOwners"), "{text}");
}

#[test]
fn all_matching_cards_move_from_graveyard_or_hand() {
    let text = ability_json(
        "SpellActions(ActionList([ReturnEachCardFromGraveyardToHand(IsCardtype(Enchantment))]))",
    );
    assert!(text.contains("OwnersHand"), "{text}");
    let text = ability_json("SpellActions(ActionList([DiscardCards(IsNonCardtype(Land))]))");
    assert!(text.contains("DiscardObjects"), "{text}");
}

#[test]
fn the_top_card_of_a_library_can_be_tested() {
    let text = ability_json("If(TopCardOfPlayersLibraryPassesFilter(You,IsCardtype(Creature)),[PermanentLayerEffect(ThisPermanent,[AdjustPT(+1,+1)])])");
    assert!(text.contains("ObjectMatches"), "{text}");
}

#[test]
fn nonattacking_and_exact_power_toughness_are_plain_filters() {
    let text = ability_json("SpellActions(Targeted([TargetPermanent(And([IsntAttacking,PowerAndToughnessIsEqualTo(PT(1,1))]))],ActionList([DestroyPermanent(Ref_TargetPermanent)])))");
    assert!(text.contains("Not"), "{text}");
    assert!(text.contains("Toughness"), "{text}");
}

#[test]
fn sharing_a_property_with_an_object_keeps_the_property() {
    let text = ability_json("SpellActions(Targeted([TargetPermanent(AnyPermanent)],ActionList([DestroyEachPermanent(SharesACardtypeWithPermanent(Ref_TargetPermanent))])))");
    assert!(text.contains("Shares"), "{text}");
    assert!(text.contains("CardTypes"), "{text}");
}

#[test]
fn a_name_can_be_shared_with_any_of_a_set() {
    let text = ability_json("SpellActions(ActionList([DestroyEachPermanent(SharesANameWithAPermanent(IsCardtype(Land)))]))");
    assert!(text.contains("SharesNameWithAny"), "{text}");
}

#[test]
fn a_fixed_number_of_players_or_any_targets_share_a_slot() {
    let text = ability_json("SpellActions(Targeted([NumberTargetPlayers(Integer(2),AnyPlayer)],ActionList([DrawACard])))");
    assert!(text.contains("Exactly"), "{text}");
    let text = ability_json("SpellActions(Targeted([TargetPermanentEachPlayerControls(AnyPermanent,Opponent)],ActionList([DrawACard])))");
    assert!(text.contains("CountPlayers"), "{text}");
    assert!(text.contains("DifferentControllers"), "{text}");
}

#[test]
fn a_card_can_come_from_a_hand_or_a_graveyard() {
    let text = ability_json("SpellActions(ActionList([PutACardFromHandOrGraveyardOnBattlefield(IsCardtype(Land),You,[EntersNormally])]))");
    assert!(text.contains("Union"), "{text}");
}

#[test]
fn a_cost_shaped_like_an_action_is_that_action_by_the_payer() {
    let text = ability_json("Activated(DestroyPermanent(ThisPermanent),ActionList([DrawACard]))");
    assert!(text.contains("PlayerAction"), "{text}");
}

#[test]
fn a_card_can_go_a_set_depth_into_its_owners_library() {
    let text = ability_json("TriggerA(WhenAPermanentDies(SinglePermanent(ThisPermanent)),ActionList([PutGraveyardCardInOwnersLibraryNthFromTheTop(ThisGraveyardCard,Integer(2))]))");
    assert!(text.contains("OwnersLibraryFromTop"), "{text}");
}

#[test]
fn for_each_opponent_keeps_the_actor_and_names_that_player() {
    let text = ability_json("SpellActions(ActionList([ActionForEachPlayer(Opponent,[DestroyEachPermanent(ControlledByAPlayer(SinglePlayer(EachablePlayer)))])]))");
    assert!(text.contains("ForEachPlayer"), "{text}");
}

#[test]
fn life_lost_is_the_event_amount() {
    let text = ability_json(
        "TriggerA(WhenAPlayerLosesLife(Opponent),ActionList([GainLife(Trigger_AmountOfLifeLost)]))",
    );
    assert!(text.contains("EventAmount"), "{text}");
}

#[test]
fn a_chosen_hand_card_can_go_to_the_bottom_of_the_library() {
    let text = ability_json("SpellActions(ActionList([PutACardFromHandOnBottomOfLibrary]))");
    assert!(text.contains("OwnersLibraryBottom"), "{text}");
}

#[test]
fn for_each_object_binds_the_object_for_its_body() {
    let text = ability_json("SpellActions(ActionList([ActionForEachPermanent(IsCardtype(Creature),[DestroyPermanent(EachablePermanent)])]))");
    assert!(text.contains("ForEachObject"), "{text}");
    assert!(text.contains("Bound"), "{text}");
}

#[test]
fn a_chosen_permanent_is_read_from_its_choice() {
    let text = ability_json("SpellActions(ActionList([ChooseAPermanent(IsCardtype(Creature)),DestroyPermanent(ThePermanentChosenThisWay)]))");
    assert!(text.contains("ChooseObjects"), "{text}");
    assert!(text.contains("\"Result\""), "{text}");
}

#[test]
fn modes_can_require_different_targets() {
    let text = ability_json("SpellActions(Modal_ChooseOneOrMore_DifferentTargets([ActionList([DrawACard]),ActionList([GainLife(Integer(2))])]))");
    assert!(text.contains("\"distinct_targets\":true"), "{text}");
}

#[test]
fn an_optional_exile_stays_readable_afterwards() {
    let text = ability_json("SpellActions(ActionList([MayAction(Exile([TheTopCardOfLibrary],[])),CreatePlayerEffectUntil(You,[MayPlayExiledCard(TheCardExiledThisWay)],UntilEndOfTurn)]))");
    assert!(text.contains("Optional"), "{text}");
    assert!(text.contains("\"Result\""), "{text}");
}

#[test]
fn a_move_inside_a_cost_stays_readable_afterwards() {
    let text = ability_json("SpellActions(ActionList([MayCost(PutACardFromHandOnBattlefield(IsCardtype(Creature),[EntersNormally])),If(CostWasPaid,[DestroyPermanent(ThePermanentPutOnTheBattlefieldThisWay)])]))");
    assert!(text.contains("CostObjects"), "{text}");
}

#[test]
fn a_row_that_moves_nothing_keeps_earlier_antecedents() {
    let text = ability_json("SpellActions(ActionList([Exile([TheTopCardOfLibrary],[]),ChooseACardName(AnyOracleCard),PutExiledCardIntoOwnersHand(TheCardExiledThisWay)]))");
    assert!(text.contains("OwnersHand"), "{text}");
}

#[test]
fn entry_flags_can_depend_on_the_entering_object() {
    let text = ability_json("SpellActions(Targeted([TargetGraveyardCard(AnyCardInAnyGraveyard)],ActionList([PutGraveyardCardOntoBattlefield(Ref_TargetGraveyardCard,[IfPassesFilter(IsCardtype(Land),[EntersTapped])])])))");
    assert!(text.contains("When"), "{text}");
}

#[test]
fn a_chosen_parity_compares_against_the_choice() {
    let text = ability_json("SpellActions(ActionList([ChooseEvenOrOdd,Exile([EachPermanent(ManaValueIs(TheChosenQuality))],[])]))");
    assert!(text.contains("SameParity"), "{text}");
    assert!(text.contains("ChosenParity"), "{text}");
}

#[test]
fn that_player_on_a_graveyard_trigger_is_the_cards_owner() {
    let text = ability_json("TriggerA(WhenAPermanentIsPutIntoAPlayersGraveyard(IsNonToken,AnyPlayer),ActionList([PlayerAction(Trigger_ThatPlayer,LoseLife(Integer(1)))]))");
    assert!(text.contains("OwnerOf"), "{text}");
    assert!(text.contains("EventSubject"), "{text}");
}

#[test]
fn a_two_object_event_names_its_second_object() {
    let text = ability_json("TriggerA(WhenAPermanentBecomesAttachedToAPermanent(SinglePermanent(ThisPermanent),IsCardtype(Creature)),ActionList([TapPermanent(Trigger_ThatOtherPermanent)]))");
    assert!(text.contains("EventOther"), "{text}");
}

#[test]
fn hand_cards_exiled_this_way_count_only_the_hand() {
    let text = ability_json("SpellActions(ActionList([Exile([PlayersHand(You)],[]),DrawNumberCards(NumHandCardsExiledThisWay)]))");
    assert!(text.contains("\"Hand\""), "{text}");
}

#[test]
fn each_player_revealing_their_top_card_is_one_observation() {
    let text = ability_json("SpellActions(ActionList([EachPlayerAction(AnyPlayer,RevealTopCardOfLibrary),GainLife(NumLibraryCardsRevealedThisWay(IsCardtype(Land)))]))");
    assert!(text.contains("LibraryTops"), "{text}");
    assert!(text.contains("Observed"), "{text}");
}

#[test]
fn a_seeked_card_can_be_named_by_a_later_step() {
    let text = ability_json("SpellActions(ActionList([SeekACard(IsCardtype(Creature)),PutCardFromHandOnBattlefield(TheCardSeekedThisWay,[EntersNormally])]))");
    assert!(text.contains("\"Result\""), "{text}");
}

#[test]
fn a_noted_set_is_read_from_the_snapshot() {
    let text = ability_json("SpellActions(ActionList([CreatePermanentsList(IsCardtype(Creature)),DestroyEachPermanent(ThePermanentsList)]))");
    assert!(text.contains("NoteObjects"), "{text}");
    assert!(text.contains("Selected"), "{text}");
}

#[test]
fn search_steps_can_branch_on_a_condition() {
    let text = ability_json("SpellActions(ActionList([SearchLibrary([FindACardOfType(IsCardtype(Land)),IfElse(ValueIs(Integer(1),EqualTo(Integer(1))),[PutFoundCardsIntoHand],[PutFoundCardsOntoBattlefield([EntersTapped])]),Shuffle])]))");
    assert!(text.contains("\"If\""), "{text}");
}

#[test]
fn an_attacked_player_is_the_events_other_player() {
    let text = ability_json("TriggerA(WhenAPlayerAttacksAPlayer(AnyPlayer,Opponent),ActionList([PlayerAction(Trigger_ThatPlayer,DrawACard),PlayerAction(Trigger_ThatOtherPlayer,LoseLife(Integer(1)))]))");
    assert!(text.contains("EventOtherPlayer"), "{text}");
    assert!(text.contains("EventSubject"), "{text}");
}

#[test]
fn an_optional_destroy_is_checked_in_the_graveyard() {
    let text = ability_json("TriggerA(WhenAPermanentEntersTheBattlefield(SinglePermanent(ThisPermanent)),Targeted([TargetPermanent(IsCardtype(Artifact))],ActionList([MayAction(DestroyPermanent(Ref_TargetPermanent)),If(DestroyedPermanentIsPutInAGraveyardThisWay,[DrawACard])])))");
    assert!(text.contains("\"Graveyard\""), "{text}");
    assert!(text.contains("\"After\""), "{text}");
}

#[test]
fn a_search_can_match_a_targeted_graveyard_cards_name() {
    let text = ability_json("SpellActions(Targeted([TargetGraveyardCard(AnyCardInAnyGraveyard)],ActionList([SearchPlayersLibraryAndGraveyardAndHand(OwnerOfGraveyrdCard(Ref_TargetGraveyardCard),[FindAnyNumberOfCardsOfType(SharesANameWithGraveyardCard(Ref_TargetGraveyardCard)),ExileFoundCards])])))");
    assert!(text.contains("SharesNameWith"), "{text}");
}

#[test]
fn a_random_move_is_picked_by_the_game() {
    let text = ability_json(
        "SpellActions(ActionList([PutACardFromGraveyardIntoHandAtRandom(IsCardtype(Creature))]))",
    );
    assert!(text.contains("AtRandom"), "{text}");
    assert!(text.contains("OwnersHand"), "{text}");
}

#[test]
fn a_random_bottom_order_is_part_of_the_destination() {
    let text = ability_json("SpellActions(ActionList([ReturnEachExiledCardToBottomOfOwnersLibraryRandomOrder(AnyExiledCard)]))");
    assert!(text.contains("OwnersLibraryBottomRandom"), "{text}");
}

#[test]
fn a_whole_hand_moves_as_one_set() {
    let text = ability_json("SpellActions(ActionList([ShuffleHandIntoLibrary]))");
    assert!(text.contains("\"Hand\""), "{text}");
    assert!(text.contains("OwnersLibraryShuffled"), "{text}");
}

#[test]
fn an_nth_position_keeps_its_number() {
    let text = ability_json(
        "SpellActions(ActionList([PutSpellInOwnersLibraryNthFromTheTop(ThisSpell,Integer(7))]))",
    );
    assert!(text.contains("OwnersLibraryFromTop"), "{text}");
    assert!(text.contains('7'), "{text}");
}

#[test]
fn a_first_each_turn_event_wraps_its_plain_event() {
    let text = ability_json("TriggerA(WhenAPlayerGainsLifeForTheFirstTimeEachTurn(SinglePlayer(You)),ActionList([DrawACard]))");
    assert!(text.contains("FirstEachTurn"), "{text}");
}

#[test]
fn cards_moved_this_way_are_counted_from_the_move() {
    let text = ability_json("SpellActions(Targeted([TargetPermanent(AnyPermanent)],ActionList([ShufflePermanentIntoLibrary(Ref_TargetPermanent),DrawNumberCards(NumCardsShuffledIntoLibraryThisWay)])))");
    assert!(text.contains("Transition"), "{text}");
}

#[test]
fn a_missing_antecedent_stays_an_error() {
    let source =
        custom("SpellActions(ActionList([DrawNumberCards(NumCardsPutIntoLibraryThisWay)]))");
    assert!(pipeline::finalize(&source).is_err());
}

#[test]
fn each_players_choice_reads_as_a_set_after_the_loop() {
    let text = ability_json("SpellActions(ActionList([EachPlayerAction(AnyPlayer,ChooseAPermanent(And([IsCardtype(Creature),ControlledByAPlayer(SinglePlayer(ActionPlayer))]))),DestroyEachPermanent(And([IsCardtype(Creature),ExceptFor(ThePermanentsChosenThisWay)]))]))");
    assert!(text.contains("ForEachPlayer"), "{text}");
    assert!(text.contains("InSet"), "{text}");
    assert!(text.contains("Selected"), "{text}");
}

#[test]
fn a_loops_choice_is_never_one_object() {
    let source = custom("SpellActions(ActionList([EachPlayerAction(AnyPlayer,ChooseAPermanent(ControlledByAPlayer(SinglePlayer(ActionPlayer)))),DestroyPermanent(ThePermanentChosenThisWay)]))");
    assert!(pipeline::finalize(&source).is_err());
}

#[test]
fn the_rest_of_a_choice_are_what_it_offered_and_left() {
    let text = ability_json("SpellActions(ActionList([EachPlayerAction(AnyPlayer,ChooseNumberPermanents(Integer(3),And([IsNonCardtype(Land),ControlledByAPlayer(SinglePlayer(ActionPlayer))]))),DestroyEachPermanent(ThePermanentsNotChosenThisWay)]))");
    assert!(text.contains("Unselected"), "{text}");
}

#[test]
fn one_pick_per_filter_is_a_constraint_over_one_domain() {
    let text = ability_json("SpellActions(ActionList([EachPlayerAction(AnyPlayer,ChooseMultiplePermanentsAmoungPermanents([IsCardtype(Artifact),IsCardtype(Creature)],ControlledByAPlayer(SinglePlayer(EachablePlayer)))),SacrificeEachPermanent(ThePermanentsNotChosenThisWay)]))");
    assert!(text.contains("OneForEach"), "{text}");
    assert!(text.contains("Unselected"), "{text}");
}

#[test]
fn one_chooser_picks_for_each_player() {
    let text = ability_json("SpellActions(ActionList([ChooseACardInEachPlayersGraveyard(IsCardtype(Creature),AnyPlayer),ReturnEachGraveyardCardToBattlefield(TheChosenGraveyardCards,[EntersUnderPlayersControl(You)])]))");
    assert!(text.contains("ForEachPlayer"), "{text}");
    assert!(text.contains("Selected"), "{text}");
}

#[test]
fn a_choice_made_as_a_cost_is_read_when_it_was_paid() {
    let text = ability_json("SpellActions(ActionList([MayCost(ChooseACardInPlayersGraveyard(IsCardtype(Creature),You)),If(CostWasPaid,[PutGraveyardCardOntoBattlefield(TheChosenGraveyardCard,[])])]))");
    assert!(text.contains("ChooseObjects"), "{text}");
    assert!(text.contains("\"Result\""), "{text}");
}

#[test]
fn a_delayed_event_matches_any_event_and_reads_its_creators_scope() {
    let text = ability_json("SpellActions(Targeted([TargetPermanent(AnyPermanent)],ActionList([CreateFutureTrigger(Or([WhenPermanentLeavesTheBattlefield(ThisPermanent),WhenPermanentBecomesUntapped(ThisPermanent)]),ActionList([DestroyPermanent(Ref_TargetPermanent)]))])))");
    assert!(text.contains("CreateDelayedTrigger"), "{text}");
    assert!(text.contains("Either"), "{text}");
}

#[test]
fn a_next_event_this_turn_is_a_one_shot_trigger() {
    let text = ability_json("SpellActions(ActionList([CreateFutureTrigger(WhenAPlayerNextAttacksThisTurn(SinglePlayer(You)),ActionList([DrawACard]))]))");
    assert!(text.contains("CreateTrigger"), "{text}");
    assert!(text.contains("\"once\":true"), "{text}");
}

#[test]
fn the_extra_turn_created_is_where_the_delayed_trigger_starts() {
    let text = ability_json("SpellActions(ActionList([TakeAnExtraTurn,CreateFutureTrigger(AtTheBeginningOfTheEndStepOfTheExtraTurnCreatedThisWay,ActionList([LoseTheGame]))]))");
    assert!(text.contains("ExtraTurn"), "{text}");
    let source = custom("SpellActions(ActionList([CreateFutureTrigger(AtTheBeginningOfTheEndStepOfTheExtraTurnCreatedThisWay,ActionList([LoseTheGame]))]))");
    assert!(pipeline::finalize(&source).is_err());
}

#[test]
fn modal_options_are_data_on_one_selection() {
    let random = ability_json("SpellActions(Modal_ChooseOneAtRandom([ActionList([DrawACard]),ActionList([GainLife(Integer(1))])]))");
    assert!(random.contains("\"random\":true"), "{random}");
    let chooser = ability_json("SpellActions(Modal_APlayerChoosesOne(Opponent,[ActionList([DrawACard]),ActionList([GainLife(Integer(1))])]))");
    assert!(chooser.contains("\"chooser\":{"), "{chooser}");
    let paired = ability_json("SpellActions(Modal_IfElse(PlayerPassesFilter(You,ControlsA(IsCreatureType(Faerie))),ActionList([DrawACard]),ActionList([GainLife(Integer(1))])))");
    assert!(paired.contains("\"implied\":true"), "{paired}");
    assert!(paired.contains("\"Not\""), "{paired}");
}

#[test]
fn a_separator_splits_what_is_left_and_the_chooser_takes_a_pile() {
    let text = ability_json("SpellActions(ActionList([RevealTheTopNumberCardsOfLibrary(Integer(5),[APlayerSeparatesThoseCardsIntoTwoPiles(Opponent),PutAPileIntoHand,PutTheRemainingCardsIntoGraveyard])]))");
    assert!(text.contains("SeparatePiles"), "{text}");
    assert!(text.contains("OneOf"), "{text}");
    assert!(text.contains("OwnersGraveyard"), "{text}");
}

#[test]
fn an_opponents_choice_among_revealed_cards_leaves_the_rest() {
    let text = ability_json("SpellActions(ActionList([RevealTheTopNumberCardsOfLibrary(Integer(3),[APlayerChoosesAGenericCard(Opponent),PutTheChosenCardIntoGraveyard,PutTheRemainingCardsIntoHand])]))");
    assert!(text.contains("OneOf"), "{text}");
    assert!(text.contains("Difference"), "{text}");
}

#[test]
fn a_damage_rider_is_data_on_one_operation() {
    let target = "Targeted([TargetPermanent(IsCardtype(Creature))],ActionList([";
    let excess = ability_json(&format!("SpellActions({target}SpellDealsDamageExcessReplacable(ThisSpell,Integer(4),Permanent(Ref_TargetPermanent),[WouldDealExcessDamage_DealExcessToPlayerInstead(ControllerOfPermanent(Ref_TargetPermanent))])])))"));
    assert!(excess.contains("ExcessTo"), "{excess}");
    let even = ability_json("SpellActions(Targeted([AnyNumberOfAnyTargets],ActionList([SpellDealsDamageDividedAmongRecipientsRoundedDown(ThisSpell,ValueX,Ref_AnyTargets)])))");
    assert!(even.contains("DividedEvenly"), "{even}");
    let scaled = ability_json("SpellActions(ActionList([SpellDealsDamageForEach(ThisSpell,Integer(2),EachPlayer(Opponent),TheNumberOfPermanentsOnTheBattlefield(IsCardtype(Creature)))]))");
    assert!(scaled.contains("Multiply"), "{scaled}");
}

#[test]
fn damage_history_is_one_query_over_windows_and_kinds() {
    let distinct = ability_json("SpellActions(ActionList([If(PlayerPassesFilter(You,WasDealtCombatDamageByNumPermanentsThisTurn(GreaterThanOrEqualTo(Integer(3)),IsCardtype(Creature))),[DrawACard])]))");
    assert!(distinct.contains("DistinctSources"), "{distinct}");
    let game = ability_json("SpellActions(ActionList([If(PlayerPassesFilter(You,WasDealtDamageByPermanentThisGame(ThisPermanent)),[DrawACard])]))");
    assert!(game.contains("ThisGame"), "{game}");
    let combat = ability_json("SpellActions(ActionList([If(PlayerPassesFilter(You,WasDealtDamageByPermanentThisCombat(ThisPermanent)),[DrawACard])]))");
    assert!(combat.contains("ThisCombat"), "{combat}");
}

#[test]
fn excess_and_recipients_read_back_from_the_damage_that_dealt_them() {
    let excess = ability_json("SpellActions(Targeted([AnyTarget],ActionList([SpellDealsDamage(ThisSpell,Integer(5),Ref_AnyTarget),If(ExcessDamageWasDealtToAPermanentThisWay(AnyPermanent),[DrawACard])])))");
    assert!(excess.contains("ExcessDamage"), "{excess}");
    let player = ability_json("SpellActions(Targeted([AnyTarget],ActionList([SpellDealsDamage(ThisSpell,Integer(2),Ref_AnyTarget),If(APlayerWasDealtDamageThisWay(AnyPlayer),[DrawACard])])))");
    assert!(player.contains("DamagedBy"), "{player}");
    let source = custom("SpellActions(ActionList([If(ExcessDamageWasDealtThisWay,[DrawACard])]))");
    assert!(pipeline::finalize(&source).is_err());
}

#[test]
fn an_action_player_is_whoever_acts() {
    let cost = ability_json("SpellActions(Targeted([TargetPlayer(AnyPlayer)],ActionList([PlayerMayCost(Ref_TargetPlayer,HaveSpellDealDamage(ThisSpell,Integer(4),Player(ActionPlayer)))])))");
    assert!(cost.contains("PlayerAction"), "{cost}");
    assert!(!cost.contains("Primitive"), "{cost}");
    let recipient = "\"recipients\":{\"kind\":\"Players\",\"args\":{\"kind\":\"One\",\"args\":{\"kind\":\"Target\"";
    assert!(cost.contains(recipient), "{cost}");
    let bare = ability_json(
        "SpellActions(ActionList([LoseLife(HalfRoundedUp(LifeTotalOfPlayer(ActionPlayer)))]))",
    );
    assert!(bare.contains("AbilityController"), "{bare}");
    let batch = ability_json("SpellActions(ActionList([EachPlayerAction(Opponent,MillNumberCards(HalfRoundedUp(NumCardsInPlayersLibrary(ActionPlayer))))]))");
    assert!(batch.contains("ForEachPlayer"), "{batch}");
    let chosen = ability_json("SpellActions(ActionList([APlayerAction(Opponent,DrawNumberCards(LifeTotalOfPlayer(ActionPlayer)))]))");
    assert!(chosen.contains("OneOf"), "{chosen}");
    assert!(!chosen.contains("Primitive"), "{chosen}");
}

#[test]
fn tokens_counted_per_player_loop_over_the_players() {
    let text = ability_json("SpellActions(ActionList([EachPlayerCreatesTokens(AnyPlayer,[NumberTokens(TheNumberOfPermanentsOnTheBattlefield(ControlledByAPlayer(SinglePlayer(ActionPlayer))),TreasureToken)],[])]))");
    assert!(text.contains("ForEachPlayer"), "{text}");
}

#[test]
fn a_cost_paid_on_resolution_is_the_action_this_way_reads() {
    let discard = ability_json("SpellActions(ActionList([MayCost(DiscardAnyNumberOfCardsOfType(IsCardtype(Land))),If(CostWasPaid,[DrawNumberCards(NumCardsDiscardedThisWay)])]))");
    assert!(!discard.contains("Primitive"), "{discard}");
    assert!(discard.contains("Discard"), "{discard}");
    assert!(discard.contains("Transition"), "{discard}");
    let announced = ability_json("Activated(Exile([APermanent(And([IsCardtype(Creature),ControlledByAPlayer(SinglePlayer(You))]))],[]),ActionList([AddManaRepeated(AnyManaColor,ManaValueOfTheCardExiledThisWay)]))");
    assert!(announced.contains("CostObjects"), "{announced}");
}

#[test]
fn a_hand_pick_is_a_reveal_a_choice_and_a_fate() {
    let exile = ability_json("SpellActions(Targeted([TargetPlayer(Opponent)],ActionList([PlayerAction(Ref_TargetPlayer,RevealHandAndPlayerChoosesACardToExile(You,AnyCard)),If(ACardWasExiledThisWay(AnyCard),[DrawACard])])))");
    assert!(!exile.contains("Primitive"), "{exile}");
    assert!(exile.contains("Reveal"), "{exile}");
    assert!(exile.contains("ChooseObjects"), "{exile}");
    assert!(
        exile.contains("\"destination\":{\"kind\":\"Exile\"")
            || exile.contains("\"destination\":\"Exile\""),
        "{exile}"
    );
    let partial = ability_json("SpellActions(Targeted([TargetPlayer(Opponent)],ActionList([PlayerAction(Ref_TargetPlayer,RevealANumberOfCardsFromHandAndPlayerChoosesACardToDiscard(Integer(2),You))])))");
    assert_eq!(partial.matches("ChooseObjects").count(), 2, "{partial}");
    assert!(partial.contains("Selected"), "{partial}");
    let look = ability_json("SpellActions(Targeted([TargetPlayer(Opponent)],ActionList([LookAtPlayersHandAndChooseACardToDiscard(Ref_TargetPlayer,AnyCard)])))");
    assert!(look.contains("LookAt"), "{look}");
    assert!(!look.contains("Reveal"), "{look}");
}

#[test]
fn a_flip_is_a_result_branches_read() {
    let both = ability_json(
        "SpellActions(ActionList([FlipACoin_OnWinAndLose([DrawACard],[LoseLife(Integer(1))])]))",
    );
    assert!(both.contains("FlipCoin"), "{both}");
    assert!(both.contains("\"outcome\":\"Won\""), "{both}");
    assert!(!both.contains("Primitive"), "{both}");
    let lost = ability_json("SpellActions(ActionList([FlipACoin_OnLose([DrawACard])]))");
    assert!(lost.contains("\"outcome\":\"Lost\""), "{lost}");
    let reflexive = ability_json("SpellActions(ActionList([Reflexive_FlipACoin_WhenYouWin_WhenYouLose(ActionList([DrawACard]),ActionList([LoseLife(Integer(1))]))]))");
    assert_eq!(
        reflexive.matches("ReflexiveTrigger").count(),
        2,
        "{reflexive}"
    );
}

#[test]
fn an_opening_hand_action_is_its_own_ability() {
    let text = ability_json("IfCardIsInOpeningHand([MayCost(RevealCardFromHand(ThisCardInHand)),If(CostWasPaid,[CreateFutureTrigger(AtTheBeginningOfTheFirstUpkeep,ActionList([SetLifeTotal(Integer(26))]))])])");
    assert!(text.contains("OpeningHand"), "{text}");
    assert!(!text.contains("Primitive"), "{text}");
}

#[test]
fn a_token_oracle_names_is_kept_by_name() {
    let named = ability_json(
        "SpellActions(ActionList([CreateTokens([OracleToken(\"Mutavault\")],[EntersTapped])]))",
    );
    assert!(named.contains("Named"), "{named}");
    assert!(named.contains("Mutavault"), "{named}");
}

#[test]
fn a_populated_token_and_an_investigation_leave_their_antecedents_readable() {
    let populated = ability_json("SpellActions(ActionList([Populate,CreateEachPermanentRuleEffectUntil(TheTokensCreatedThisWay,[CantBlock],UntilEndOfTurn)]))");
    assert!(populated.contains("Created"), "{populated}");
    let kept = ability_json("SpellActions(Targeted([TargetPermanent(IsCardtype(Creature))],ActionList([Exile([Permanent(Ref_TargetPermanent)],[]),Investigate,PutExiledCardOntoBattlefield(TheCardExiledThisWay,[EntersNormally])])))");
    assert!(kept.contains("\"kind\":\"Result\""), "{kept}");
}

#[test]
fn a_target_says_how_many_who_chooses_and_what_it_avoids() {
    let other = ability_json("SpellActions(Targeted([AnyTarget,AnyOtherTarget(Ref_AnyTarget1)],ActionList([SpellDealsMultipleDamage(ThisSpell,[DamageToRecipients(Integer(2),Ref_AnyTarget1),DamageToRecipients(Integer(1),Ref_AnyTarget2)])])))");
    assert!(other.contains("DifferentFromTarget"), "{other}");
    assert!(!other.contains("Primitive"), "{other}");
    let chosen = ability_json("SpellActions(Targeted([TargetPermanentOfAPlayersChoiceTheyControl(IsCardtype(Creature),Opponent)],ActionList([DestroyPermanent(Ref_TargetPermanent)])))");
    assert!(
        chosen.contains("\"chooser\":{\"kind\":\"OneOf\""),
        "{chosen}"
    );
    assert!(chosen.contains("ControlledBy"), "{chosen}");
    let random = ability_json("SpellActions(Targeted([TargetPlayerAtRandom(AnyPlayer)],ActionList([PlayerAction(Ref_TargetPlayer,DiscardACard)])))");
    assert!(random.contains("AtRandom"), "{random}");
    let colors = ability_json("SpellActions(Targeted([UptoOneTargetGraveyardCardOfEachColor(InAPlayersGraveyard(SinglePlayer(You)))],ActionList([PutEachGraveyardCardIntoHand(Ref_TargetGraveyardCards)])))");
    assert!(colors.contains("OneForEach"), "{colors}");
}

#[test]
fn moves_name_the_owners_choice_and_the_depth_under_cards() {
    let choice = ability_json("SpellActions(Targeted([TargetSpell(AnySpell)],ActionList([ReturnSpellToTopOrBottomOfLibrary(Ref_TargetSpell)])))");
    assert!(choice.contains("OwnersLibraryTopOrBottom"), "{choice}");
    let under = ability_json("SpellActions(Targeted([TargetPermanent(IsCardtype(Creature))],ActionList([PutPermanentInOwnersLibraryBeneathNumberCards(Ref_TargetPermanent,Integer(2))])))");
    assert!(under.contains("OwnersLibraryFromTop"), "{under}");
    assert!(under.contains("\"kind\":\"Add\""), "{under}");
}

#[test]
fn counters_leave_exiled_cards_as_they_leave_permanents() {
    let text = ability_json("SpellActions(ActionList([Exile([TheTopNumberCardsOfLibrary(Integer(1))],[ExilesWithNumberCounters(Integer(3),TimeCounter)]),RemoveCounters([ACounterOfTypeFromCardInExile(TimeCounter,TheCardExiledThisWay)])]))");
    assert!(text.contains("RemoveCounters"), "{text}");
    assert!(!text.contains("Primitive"), "{text}");
}

#[test]
fn a_reflexive_discard_reads_what_was_discarded() {
    let plain = ability_json("SpellActions(ActionList([Reflexive_Discard_WhenYouDo(ACardOfType(IsNonCardtype(Land)),ActionList([DrawACard]))]))");
    assert!(plain.contains("ReflexiveTrigger"), "{plain}");
    assert!(!plain.contains("Primitive"), "{plain}");
    let filtered = ability_json("SpellActions(ActionList([Reflexive_Discard_WhenACardOfTypeIsDiscardedThisWay(ACard,IsCardtype(Creature),ActionList([DrawACard]))]))");
    assert!(filtered.contains("\"condition\":{"), "{filtered}");
    assert!(filtered.contains("Creature"), "{filtered}");
}

#[test]
fn a_seek_is_a_random_pick_moved_to_hand() {
    let text = ability_json("SpellActions(ActionList([SeekACard(AnyCard)]))");
    assert!(!text.contains("Primitive"), "{text}");
    assert!(text.contains("AtRandom"), "{text}");
    assert!(text.contains("OwnersHand"), "{text}");
    let many = ability_json("SpellActions(ActionList([SeekNumberCards(Integer(2),AnyCard)]))");
    assert!(many.contains("AsManyAsCan"), "{many}");
    assert!(!many.contains("Primitive"), "{many}");
}

#[test]
fn each_player_who_did_reads_their_share_of_the_batch() {
    let refused = ability_json("SpellActions(ActionList([EachPlayerAction(Opponent,SacrificeAPermanent(IsCardtype(Creature))),EachPlayerAction(DidntSacrificeAPermanentThisWay(AnyPermanent),DiscardACard)]))");
    assert!(!refused.contains("Primitive"), "{refused}");
    assert!(refused.contains("MovedBy"), "{refused}");
    assert!(refused.contains("Intersection"), "{refused}");
    assert!(refused.contains("OpponentsOf"), "{refused}");
    let took = ability_json("SpellActions(ActionList([EachPlayerMayAction(AnyPlayer,DrawACard),EachPlayerAction(PlayerWhoTookActionThisWay,GainLife(Integer(1)))]))");
    assert!(took.contains("PlayerTook"), "{took}");
    let declined = ability_json("SpellActions(ActionList([EachPlayerMayAction(Opponent,DrawACard),EachPlayerAction(DidntTakeAction,LoseLife(Integer(1)))]))");
    assert!(declined.contains("PlayerTook"), "{declined}");
    assert!(declined.contains("Intersection"), "{declined}");
    let counted = ability_json("SpellActions(ActionList([EachPlayerAction(AnyPlayer,DiscardHand),EachPlayerAction(AnyPlayer,DrawNumberCards(TheNumberOfCardsDiscardedByPlayerThisWay(EachablePlayer)))]))");
    assert!(counted.contains("MovedBy"), "{counted}");
    assert!(!counted.contains("Primitive"), "{counted}");
}

#[test]
fn a_loop_keeps_what_it_made_and_a_payment_reads_its_amount() {
    let paid = ability_json(
        "SpellActions(ActionList([PayAnyAmountOfMana,DrawNumberCards(AmountOfManaPaidThisWay)]))",
    );
    assert!(paid.contains("Performed"), "{paid}");
    let found = ability_json("SpellActions(ActionList([SearchLibrary([FindACardOfType(IsCardtype(Creature)),RevealFoundCards,If(ACardOfTypeWasFoundThisWay(IsCardtype(Land)),[PutFoundCardsIntoHand])])]))");
    assert!(found.contains("Selected"), "{found}");
    assert!(found.contains("Compare"), "{found}");
    let choice = ability_json("SpellActions(ActionList([EachPlayerMakesAVillainousChoice(Opponent,[[DrawACard],[PlayerAction(EachablePlayer,DiscardACard)]])]))");
    assert!(choice.contains("ForEachPlayer"), "{choice}");
    assert!(choice.contains("ChooseOne"), "{choice}");
    assert!(!choice.contains("Primitive"), "{choice}");
}

#[test]
fn each_permanent_can_act_through_its_controller() {
    let text = ability_json("SpellActions(ActionList([ActionForEachPermanentByController(IsCardtype(Creature),[DrawACard])]))");
    assert!(text.contains("ForEachObject"), "{text}");
    assert!(text.contains("ControllerOf"), "{text}");
    assert!(!text.contains("Primitive"), "{text}");
}

#[test]
fn zone_history_reads_this_turn_and_where_a_card_came_from() {
    let back = ability_json("SpellActions(ActionList([ReturnEachCardFromGraveyardToHandThatWasPutThereFromBattlefieldThisTurn(IsCardtype(Creature))]))");
    assert!(back.contains("ArrivedThisTurn"), "{back}");
    assert!(back.contains("Battlefield"), "{back}");
    assert!(!back.contains("Primitive"), "{back}");
    let attacked = ability_json("TriggerI(AtTheBeginningOfAPlayersEndStep(SinglePlayer(You)),PlayerPassesFilter(You,AttackedWithACreatureThisTurn(AnyPermanent)),ActionList([DrawACard]))");
    assert!(attacked.contains("Attacks"), "{attacked}");
    assert!(attacked.contains("EventHistory"), "{attacked}");
}

#[test]
fn a_duration_can_name_the_extra_turn_this_way_made() {
    let text = ability_json("SpellActions(ActionList([TakeAnExtraTurn,CreateGameEffect(DuringTheExtraTurnCreatedThisWay,CreaturesCantBlock)]))");
    assert!(text.contains("ExtraTurn"), "{text}");
}

#[test]
fn a_target_player_can_carry_a_condition() {
    let text = ability_json("SpellActions(Targeted([TargetPlayerAtTime(Opponent,ControlsMorePermanentThanPlayer(You,IsCardtype(Creature)))],ActionList([DrawACard])))");
    assert!(text.contains("Intersection"), "{text}");
    assert!(text.contains("OpponentsOf"), "{text}");
}

#[test]
fn an_action_reads_its_own_actor_library_and_battlefield() {
    let text = ability_json("TriggerA(AtTheBeginningOfAPlayersDrawStep(AnyPlayer),ActionList([PlayerAction(Trigger_ThatPlayer,Exile([TheTopCardOfLibrary],[])),If(ACardWasExiledThisWay(IsCardtype(Land)),[PlayerAction(Trigger_ThatPlayer,PutExiledCardOntoBattlefield(TheCardExiledThisWay,[EntersNormally]))])]))");
    assert!(!text.contains("AbilityController"), "{text}");
    let text = ability_json(
        "SpellActions(ActionList([EachPlayerAction(Opponent,Exile([TheTopCardOfLibrary],[]))]))",
    );
    assert!(text.contains("Bound"), "{text}");
}

#[test]
fn a_life_loss_is_replaced_for_the_player_losing_it() {
    let text = ability_json("ReplaceWouldLoseLife(APlayerWouldLoseLife(Opponent),[LoseLife(Twice(WouldLoseLife_ThatMuch))])");
    assert!(text.contains("EventSubject"), "{text}");
    assert!(
        !text.contains(
            "\"players\":{\"kind\":\"One\",\"args\":{\"kind\":\"AbilityController\"}},\"amount\""
        ),
        "{text}"
    );
}

#[test]
fn a_difference_is_never_negative() {
    let text = ability_json(
        "SpellActions(ActionList([GainLife(DifferenceBetween(Integer(1),Integer(4)))]))",
    );
    assert!(text.contains("Maximum"), "{text}");
    assert_eq!(text.matches("Subtract").count(), 2, "{text}");
}

#[test]
fn counters_removed_from_among_permanents_are_spread_by_the_player() {
    let text = ability_json("Activated(RemoveCounters([NumberCountersFromAmongPermanents(Integer(3), ControlledByAPlayer(SinglePlayer(You)))]),ActionList([DrawACard]))");
    assert!(text.contains("AmongThem"), "{text}");
}

#[test]
fn a_replacement_reveals_and_shuffles_the_card_it_saves() {
    let text = ability_json(
        "AsPutIntoAGraveyardFromAnywhere(ThisCard, [RevealItAndShuffleItIntoLibraryInstead])",
    );
    assert!(text.contains("Reveal"), "{text}");
    assert!(text.contains("OwnersLibraryShuffled"), "{text}");
}

#[test]
fn a_search_finds_a_card_named_after_each_chosen_permanent() {
    let text = ability_json("SpellActions(ActionList([ChooseNumberPermanents(Integer(5), ControlledByAPlayer(SinglePlayer(You))), SearchLibrary([FindAnyNumberOfGroupCards(AnyCardInLibrary, SharesANameWithEachPermanent(ThePermanentsChosenThisWay)), PutFoundCardsOntoBattlefield([EntersTapped]), Shuffle])]))");
    assert!(text.contains("NamedAfterOneEach"), "{text}");
}

#[test]
fn a_timed_entry_replacement_changes_the_entering_object() {
    let text = ability_json("SpellActions(ActionList([CreateReplaceWouldEnterUntil(APermanentWouldEnterTheBattlefield(IsCardtype(Creature)), [EntersWithACounter(PTCounter(+1, +1))], UntilEndOfTurn)]))");
    assert!(text.contains("AddCounters"), "{text}");
    let text = ability_json("SpellActions(ActionList([CreateReplaceWouldEnterUntil(APermanentWouldEnterTheBattlefield(IsCardtype(Creature)), [EntersTapped], UntilEndOfTurn)]))");
    assert!(text.contains("SetTapped"), "{text}");
}

#[test]
fn a_move_keeps_how_the_object_enters() {
    let text = ability_json("TriggerA(WhenAPermanentDies(SinglePermanent(ThisPermanent)), ActionList([ReturnDeadGraveyardCardToBattlefield([EntersFaceDown, EntersUnderPlayersControl(You)])]))");
    assert!(text.contains("FaceDown"), "{text}");
    let text = ability_json("SpellActions(ActionList([Exile([Permanent(ThisPermanent)], []), PutExiledCardOntoBattlefield(TheCardExiledThisWay, [EntersUnderAPlayersControl(Opponent)])]))");
    assert!(text.contains("OneOf"), "{text}");
}

#[test]
fn a_cost_reveals_any_number_of_cards_and_counts_them() {
    let text = ability_json("AsPermanentEnters(ThisPermanent, [MayCost(RevealAnyNumberOfCardsOfTypeFromHand(IsCardtype(Artifact))), If(CostWasPaid, [EntersWithNumberCounters(TheNumberOfCardsInHandRevealedThisWay, PTCounter(+1, +1))])])");
    assert!(text.contains("AnyNumber"), "{text}");
    assert!(text.contains("CostObjects"), "{text}");
}

#[test]
fn a_token_can_copy_any_object_or_one_of_a_set() {
    let text = ability_json("SpellActions(Targeted([TargetGraveyardCard(IsCardtype(Creature))], ActionList([CreateTokens([TokenCopyOfGraveyardCard(Ref_TargetGraveyardCard, NoTokenCopyEffects)], [])])))");
    assert!(text.contains(r#""Copy":{"object""#), "{text}");
    let text = ability_json("SpellActions(ActionList([CreateTokens([TokenCopyOfAPermanent(ControlledByAPlayer(SinglePlayer(You)), NoTokenCopyEffects)], [])]))");
    assert!(text.contains("CopyOne"), "{text}");
    let text = ability_json("SpellActions(ActionList([CreateTokens([TokenCopyOfACardAtRandom(IsCardtype(Creature))], [])]))");
    assert!(text.contains("CopyAtRandom"), "{text}");
}

#[test]
fn a_countered_spell_sent_to_exile_is_what_was_exiled() {
    let text = ability_json("SpellActions(Targeted([TargetSpell(AnySpell)], ActionList([CounterSpellIntoExile(Ref_TargetSpell), CreatePlayerEffectUntil(You, [MayCastExiledCard(TheCardExiledThisWay)], UntilEndOfTurn)])))");
    assert!(text.contains(r#""phase":"After""#), "{text}");
}

#[test]
fn either_branch_leaves_one_result() {
    let text = ability_json("SpellActions(ActionList([IfElse(FreerunningCostWasPaid, [Exile([TheTopNumberCardsOfLibrary(ValueX)], [])], [Exile([TheTopNumberCardsOfLibrary(Integer(2))], [])]), CreatePlayerEffectUntil(You, [MayPlayExiledCards(TheCardsExiledThisWay)], UntilEndOfTurn)]))");
    assert!(text.contains("JoinResults"), "{text}");
    let text = ability_json("SpellActions(ActionList([ChooseAnAction([[DestroyEachPermanentNoRegen(IsColor(Green))], [DestroyEachPermanentNoRegen(IsColor(White))]]), LoseLifeForEach(Integer(2), NumPermanentsDestroyedThisWay(IsCardtype(Creature)))]))");
    assert!(text.contains("JoinResults"), "{text}");
}

#[test]
fn a_mana_rider_names_the_spell_it_paid_for() {
    let text = ability_json("Activated(TapPermanent(ThisPermanent), ActionList([AddManaWithModifiers(ManaProduceR, TriggerSpentOnSpell(IsColor(Red), ActionList([CopySpellAndMayChooseNewTargets(Trigger_ThatSpell)])))]))");
    assert!(text.contains("TriggerSpentOnSpell"), "{text}");
    assert!(text.contains("EventSubject"), "{text}");
}

#[test]
fn a_play_permission_keeps_its_terms() {
    let text = ability_json("PlayerEffect(You, [MayCastASpellFromGraveyardOnceEachPlayersTurn(IsCardtype(Creature), You)])");
    assert!(text.contains(r#""EachTurn":1"#), "{text}");
    assert!(text.contains(r#""during":{"kind":"One""#), "{text}");
    let text = ability_json("SpellActions(ActionList([Exile([TheTopCardOfLibrary], []), CreatePlayerEffectUntil(You, [MayPlayOneCardFromAmongExiled(TheCardsExiledThisWay)], UntilEndOfTurn)]))");
    assert!(text.contains(r#""Total":1"#), "{text}");
    assert!(text.contains(r#""from":"Exile""#), "{text}");
    let text = ability_json("PlayerEffect(You, [MayCastSpellsFromGraveyardForAdditionalCostWithEffect(IsCardtype(Creature), You, Forage, [EntersWithACounter(FinalityCounter)])])");
    assert!(text.contains(r#""Additional":{"kind":"Cost""#), "{text}");
    assert!(text.contains("EntersWithACounter"), "{text}");
}

#[test]
fn each_payer_is_offered_and_the_refusers_are_counted() {
    let text = ability_json("SpellActions(ActionList([EachPlayerMustCost(Opponent, DiscardACard), DrawNumberCards(TheNumberOfPlayersThatDidntPayCost)]))");
    assert!(text.contains("PayCost"), "{text}");
    assert!(text.contains("Intersection"), "{text}");
    assert!(text.contains("PlayerTook"), "{text}");
}

#[test]
fn a_chosen_power_change_is_read_where_it_applies() {
    let text = ability_json("Activated(PayMana([ManaCostGeneric(1)]), ActionList([ChoosePTMod([PTMod(+1, -1), PTMod(-1, +1)]), CreatePermanentLayerEffectUntil(ThisPermanent, [AdjustPT_TheChosenPTMod], UntilEndOfTurn)]))");
    assert!(text.contains("ChoosePowerToughness"), "{text}");
    assert!(text.contains("ChosenPowerToughness"), "{text}");
}

#[test]
fn ordered_dice_keep_their_positions() {
    let text = ability_json("SpellActions(ActionList([RollTwoD8AndChooseAnOrder, DrawNumberCards(TheFirstDiceResult), GainLife(TheSecondDiceResult)]))");
    assert!(text.contains(r#""DieResultAt""#), "{text}");
    assert!(text.contains(r#""index":1"#), "{text}");
}

#[test]
fn an_inner_target_is_not_the_outer_one() {
    let text = ability_json("TriggerA(WhenAPermanentEntersTheBattlefield(SinglePermanent(ThisPermanent)), Targeted([TargetPermanent(IsCardtype(Creature))], ActionList([TapPermanent(Ref_TargetPermanent), CreateFutureTrigger(AtTheBeginningOfTheNextEndStep, Targeted([TargetPermanent(IsCardtype(Land))], ActionList([PermanentDealsDamage(Ref_TargetPermanent, Integer(3), Permanent(RefOuter_TargetPermanent))])))])))");
    assert!(text.contains("CreateTrigger"), "{text}");
    assert!(text.contains(r#""once":true"#), "{text}");
    let source = text.find(r#""source":{"kind":"Target""#).expect(&text);
    assert!(text[source..].contains(r#""slot":1"#), "{text}");
}

#[test]
fn what_a_card_made_is_read_from_its_other_abilities() {
    let text = ability_json("TriggerA(WhenAPermanentLeavesTheBattlefield(SinglePermanent(ThisPermanent)), ActionList([Exile([EachPermanent(TheCreatedTokens)], [])]))");
    assert!(text.contains("MadeBy"), "{text}");
    let text = ability_json("TriggerA(WhenAPermanentDies(PutOntoBattlefieldByPermanent(ThisPermanent)), ActionList([PutPermanentIntoItsOwnersHand(ThisPermanent)]))");
    assert!(text.contains("MadeBy"), "{text}");
    let text = ability_json("TriggerA(WhenAPermanentLeavesTheBattlefield(SinglePermanent(ThisPermanent)), ActionList([Exile([Permanent(ThePermanentPutOnTheBattlefieldThisWay)], [])]))");
    assert!(text.contains("MadeHere"), "{text}");
}

#[test]
fn a_cost_paying_twice_reads_both_payments() {
    let text = ability_json("Activated(And([TapPermanent(ThisPermanent), And([SacrificePermanent(ThisPermanent), SacrificeAnyNumberOfPermanents(IsCardtype(Creature))])]), ActionList([DrawNumberCards(TheNumberOfPermanentsSacrificedThisWay)]))");
    assert!(text.contains("Union"), "{text}");
}

#[test]
fn a_mode_reads_what_the_ability_already_did() {
    let text = ability_json("SpellActions(ActionList([Reflexive_Exile_WhenYouDo([ACardInGraveyards(AnyCardInAnyGraveyard)], [], Modal_ChooseOne([ActionList([GainLife(ManaValueOfExiled(TheCardExiledThisWay))]), ActionList([DrawACard])]))]))");
    assert!(
        text.contains(r#""object":{"kind":"Result","args":{"result":1"#),
        "{text}"
    );
}

#[test]
fn numbered_any_targets_are_read_by_position() {
    let text = ability_json("SpellActions(Targeted([NumberAnyTargets(Integer(2))], ActionList([SpellDealsDamage(ThisSpell, Integer(3), Ref_AnyTarget1), SpellDealsDamage(ThisSpell, Integer(3), Ref_AnyTarget2)])))");
    assert!(text.contains(r#""index":1"#), "{text}");
}

#[test]
fn reflexive_triggers_name_their_base_and_who_does_it() {
    let text = ability_json("SpellActions(ActionList([Reflexive_Discard_Draw_WhenYouDo(ACard, ACard, ActionList([GainLife(Integer(1))]))]))");
    assert!(text.contains("ReflexiveTrigger"), "{text}");
    assert!(text.contains("Discard") && text.contains("Draw"), "{text}");
    let text = ability_json("SpellActions(ActionList([Reflexive_RollDice_WhenYouDo(D20, ActionList([GainLife(TheDiceResult)]))]))");
    assert!(text.contains("RollDie"), "{text}");
    let text = ability_json("SpellActions(ActionList([Reflexive_PlayerSacrifices_WhenTheyDo(You, [APermanent(IsCardtype(Creature))], ActionList([DrawACard]))]))");
    assert!(text.contains("ReflexiveTrigger"), "{text}");
}

#[test]
fn a_set_acted_on_this_way_is_walked_one_object_at_a_time() {
    let text = ability_json("SpellActions(Targeted([TargetPermanent(IsCardtype(Creature))], ActionList([DestroyPermanent(Ref_TargetPermanent), ActionForEachPermanentDestroyedThisWay([PlayerAction(ControllerOfPermanent(EachablePermanent), DrawACard)])])))");
    assert!(text.contains("ForEachObject"), "{text}");
    assert!(text.contains(r#""kind":"Transition""#), "{text}");
}

#[test]
fn a_trigger_made_for_good_or_once_keeps_how_long_it_lasts() {
    let text = ability_json("SpellActions(ActionList([CreateTrigger(WhenACreatureAttacks(SinglePermanent(ThisPermanent)), ActionList([DrawACard]))]))");
    assert!(text.contains(r#""once":false"#), "{text}");
    let text = ability_json("SpellActions(ActionList([CreateTriggerOnce(UntilEndOfTurn, WhenACreatureAttacks(SinglePermanent(ThisPermanent)), ActionList([DrawACard]))]))");
    assert!(text.contains(r#""once":true"#), "{text}");
}

#[test]
fn what_was_revealed_this_way_is_tested_as_a_set() {
    let text = ability_json("SpellActions(ActionList([RevealTheTopNumberCardsOfLibrary(Integer(1), []), If(NoCardsOfTypeWereRevealedThisWay(IsCardtype(Land)), [DrawACard])]))");
    assert!(text.contains("Matching"), "{text}");
}

#[test]
fn what_an_object_did_this_turn_is_asked_of_that_object() {
    let text = ability_json("SpellActions(Targeted([TargetPermanent(And([IsCardtype(Creature),BlockedAttackerThisTurn(ThisPermanent)]))],ActionList([DestroyPermanent(Ref_TargetPermanent)])))");
    assert!(text.contains("EventHistory"), "{text}");
    assert!(text.contains("Candidate"), "{text}");
    let text = ability_json("SpellActions(Targeted([TargetPermanent(WasCastFromAPlayersGraveyard(SinglePlayer(You)))],ActionList([DestroyPermanent(Ref_TargetPermanent)])))");
    assert!(text.contains("Cast"), "{text}");
}

#[test]
fn each_player_keeps_one_of_each_and_loses_the_rest() {
    let text = ability_json("SpellActions(ActionList([EachPlayerAction(AnyPlayer,ChooseAPartyFromAmongPermanents(And([IsCardtype(Creature),ControlledByPlayer(EachablePlayer)]))),EachPlayerAction(AnyPlayer,SacrificeEachPermanent(ThePermanentsNotChosenThisWay))]))");
    assert!(text.contains("OneForEach"), "{text}");
    assert!(text.contains("Unselected"), "{text}");
    let text = ability_json("SpellActions(ActionList([EachPlayerAction(AnyPlayer,RevealHand),EachPlayerAction(AnyPlayer,ChooseACardInHandOfEachColor),EachPlayerAction(AnyPlayer,DiscardCards(And([TheCardsInHandNotChosenThisWay,IsNonCardtype(Land)])))]))");
    assert!(text.contains("Unselected"), "{text}");
}

#[test]
fn a_pick_is_read_by_who_made_it() {
    let text = ability_json("SpellActions(ActionList([EachPlayerAction(Opponent,ChooseAPermanent(And([IsCardtype(Creature),ControlledByPlayer(EachablePlayer)]))),EachPlayerAction(Opponent,Exile([Permanent(ThePermanentChosenByPlayerThisWay(EachablePlayer))],[]))]))");
    assert!(text.contains("ChosenBy"), "{text}");
    let text = ability_json("SpellActions(ActionList([ChooseAPermanent(IsCardtype(Creature)),EachPlayerAction(Opponent,ChooseAPermanent(And([IsCardtype(Creature),ControlledByPlayer(EachablePlayer)]))),Exile([EachPermanent(And([IsCardtype(Creature),NotChosenByAPlayerThisWay(AnyPlayer)]))],[])]))");
    assert_eq!(text.matches("Selected").count(), 2, "{text}");
}

#[test]
fn a_reveal_is_read_by_later_filters() {
    let text = ability_json("SpellActions(ActionList([EachPlayerRevealsTheTopNumberCardsOfLibrary(AnyPlayer,Integer(5)),EachPlayerAction(AnyPlayer,PutCardsInLibraryOntoTheBattlefield(And([IsCardtype(Land),RevealedFromPlayersLibraryThisWay(EachablePlayer)]),[EntersTapped])),EachPlayerAction(AnyPlayer,Exile([EachCardInLibrary(RevealedFromPlayersLibraryThisWay(EachablePlayer))],[]))]))");
    assert!(text.contains("Observed"), "{text}");
    assert!(text.contains("OwnedBy"), "{text}");
    let text = ability_json("SpellActions(ActionList([RevealACardOfTypeFromHand(IsCardtype(Creature)),SearchLibrary([FindACardOfType(SharesANameWithTheCardRevealedThisWay),RevealFoundCards,PutFoundCardsIntoHand,Shuffle])]))");
    assert!(text.contains("SharesNameWithAny"), "{text}");
    let text = ability_json("SpellActions(ActionList([DrawACard,RevealTheCardDrawnThisWay,Unless(ACardWasRevealedThisWay(IsCardtype(Land)),[DiscardTheCardDrawnThisWay])]))");
    assert!(text.contains("DiscardObjects"), "{text}");
}

#[test]
fn a_tap_change_and_goading_read_what_they_touched() {
    let text = ability_json("SpellActions(ActionList([UntapEachPermanent(IsCardtype(Creature)),CreateEachPermanentLayerEffectUntil(WasUntappedThisWay,[AdjustPT(+1,+1)],UntilEndOfTurn)]))");
    assert!(text.contains("Affected"), "{text}");
    let text = ability_json("SpellActions(ActionList([GoadEachCreature(IsCardtype(Creature)),CreateEachPermanentRuleEffectUntil(WasGoadedThisWay,[CantBlock],UntilEndOfTurn)]))");
    assert!(text.contains("Affected"), "{text}");
}

#[test]
fn an_exile_until_splits_what_it_found_from_the_rest() {
    let text = ability_json("SpellActions(ActionList([Exile([CardsFromTheTopOfLibraryUntilACardOfTypeIsExiled(IsCardtype(Land))],[]),PutExiledCardOntoBattlefield(TheExiledCardFoundThisWay,[EntersNormally]),PutEachExiledCardOnTheBottomOfTheirOwnersLibraryInARandomOrder(TheNonSpecificCardsExiledThisWay)]))");
    assert!(text.contains("Not"), "{text}");
}

#[test]
fn each_source_can_aim_at_its_own_controller() {
    let text = ability_json("SpellActions(ActionList([EachPermanentDealsDamage(IsCardtype(Enchantment),Integer(2),Player(ControllerOfPermanent(ActionPermanent)))]))");
    assert!(text.contains("ForEachObject"), "{text}");
}

#[test]
fn a_granted_ability_names_who_granted_it() {
    let text = ability_json("PermanentLayerEffect(HostPermanent,[AddAbility([Activated(PayMana([ManaCostGeneric(7)]),ActionList([SacrificePermanent(ThePermanentThatGrantedThisAbility)]))])])");
    assert!(text.contains("Granter"), "{text}");
}

#[test]
fn a_player_static_binds_each_player_it_affects() {
    let text = ability_json("EachPlayerEffect(Opponent,[CantCastSpells(ManaValueIs(GreaterThan(TheNumberOfPermanentsOnTheBattlefield(And([IsCardtype(Land),ControlledByAPlayer(SinglePlayer(EachablePlayer))])))))])");
    assert!(text.contains("Affected"), "{text}");
}

#[test]
fn a_card_replaces_its_discard_from_hand() {
    let text = ability_json("ReplaceWouldDiscard(ASpellOrAbilityWouldCausePlayerToDiscardCard(ControlledByAPlayer(Opponent),You,ThisCardInHand),[PutItOntoTheBattlefieldInstead([EntersNormally])])");
    assert!(text.contains("Everywhere"), "{text}");
}

#[test]
fn may_costs_read_what_they_drew_and_milled() {
    let text = ability_json("TriggerA(WhenACreatureAttacks(SinglePermanent(ThisPermanent)),ActionList([MayCost(DrawNumberCards(Integer(2))),If(CostWasPaid,[DiscardNumberCards(NumberOfCardsDrawnThisWay)])]))");
    assert!(text.contains("CostObjects"), "{text}");
    let text = ability_json("SpellActions(ActionList([MayCost(MillNumberCards(Integer(2))),If(CostWasPaid,[MayAction(PutACardOfTypeMilledThisWayIntoHand(IsCardtype(Creature)))])]))");
    assert!(text.contains("CostObjects"), "{text}");
}

#[test]
fn history_numbers_ask_about_ordinary_events() {
    let text = ability_json("SpellActions(ActionList([GainLife(NumberOfPermanentsSacrificedByPlayerThisTurn(AnyPermanent,You))]))");
    assert!(text.contains("EventHistory"), "{text}");
    assert!(text.contains("\"event\":\"Sacrifices\""), "{text}");
    let text = ability_json("SpellActions(ActionList([GainLife(TheNumberOfCountersPutOnPermanentsThisTurn(AnyPermanent))]))");
    assert!(text.contains("TotalAmount"), "{text}");
}

#[test]
fn a_player_history_reads_that_player() {
    let text = ability_json("If(PlayerPassesFilter(You,DidntAttackWithCreaturesThisTurn),[PlayerEffect(You,[CantGainLife])])");
    assert!(text.contains("\"event\":\"Attacks\""), "{text}");
    assert!(text.contains("Not"), "{text}");
}

#[test]
fn every_player_passing_counts_those_who_pass() {
    let text = ability_json("TriggerI(AtTheBeginningOfAPlayersUpkeep(SinglePlayer(You)),AllPlayersPassFilter(AnyPlayer,LifeTotalIs(LessThanOrEqualTo(Integer(10)))),ActionList([DrawACard]))");
    assert_eq!(text.matches("CountPlayers").count(), 2, "{text}");
}

#[test]
fn trigger_rows_bind_what_their_bodies_read() {
    let exile = ability_json("TriggerA(WhenAPermanentIsExiled(AnyPermanent),ActionList([ReturnAnExiledCardToBattlefield(SingleExiledCard(Trigger_ThatExiledCard),[])]))");
    assert!(exile.contains("EventDestination"), "{exile}");
    let spell = ability_json("TriggerA(WhenAPermanentBecomesTheTargetOfASpell(SinglePermanent(ThisPermanent),AnySpell),ActionList([PermanentDealsDamage(ThisPermanent,Integer(2),Player(ControllerOfSpell(Trigger_ThatSpell)))]))");
    assert!(spell.contains("EventStackObject"), "{spell}");
}
