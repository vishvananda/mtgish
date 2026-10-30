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
    let s=custom("SpellActions(ActionList([FlipACoin_OnWin([PutPermanentIntoItsOwnersHand(ThisPermanent)])]))");
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
        .any(|n| n.form == "FlipACoin_OnWin"));
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
    let s=custom("SpellActions(Targeted([TargetPermanent(AnyPermanent)],ActionList([GainControlOfPermanentUntil(Ref_TargetPermanent,UntilNextUpkeep(You))])))");
    let value = pipeline::Intermediate::new(&s).unwrap();
    assert!(value.final_card.is_none());
    assert!(matches!(
        &value.structural.rewrites[0].expanded,
        ExpandedRecipe::ContinuousEffect {
            duration: DurationInput::Source(source::Expiration::UntilNextUpkeep(_)),
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
    let bad=custom("SpellActions(ActionList([GainControlOfPermanentUntil(ThisPermanent,UntilNextUpkeep(You))]))");
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
        custom("SpellActions(ActionList([SetLifeTotal(Twice(NumCardsInPlayersLibrary(You)))]))");
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
    let source =
        custom("SpellActions(ActionList([MayActions([DrawACard,SetLifeTotal(Integer(7))])]))");
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
    assert!(stage.diagnostics.iter().any(|d| d.form == "SetLifeTotal"));
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
    assert!(pipeline::finalize(&source)
        .unwrap_err()
        .iter()
        .any(|d| d.message.contains("iteration or batch policy")));
    let source =
        custom("SpellActions(ActionList([EachPlayerAction(AnyPlayer,MayAction(DrawACard))]))");
    assert!(pipeline::finalize(&source)
        .unwrap_err()
        .iter()
        .any(|d| d.message.contains("one decider")));
}

#[test]
fn program_optionals_and_conditionals_cannot_leak_movement_results() {
    for action in [
        "MayAction(PutGraveyardCardOntoBattlefield(Ref_TargetGraveyardCard,[]))",
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
fn program_registry_checks_field_accounting_and_rejects_unreviewed_batch_optional_policy() {
    let original = serde_json::to_value(pipeline::programs::registry()).unwrap();
    for case in 0..4 {
        let mut value = original.clone();
        match case {
            0 => value["rules"][0]["arity"] = 3.into(),
            1 => value["rules"][0]["recipe"]["body"]["argument"] = 0.into(),
            2 => value["rules"][0]["recipe"]["kind"] = "Optional".into(),
            _ => {
                value["rules"][0]["recipe"] = serde_json::json!({"kind":"Optional","actor":{"kind":"Batch","argument":0},"body":{"kind":"Action","argument":1}})
            }
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
    let source=custom("Activated(And([PayLifeForEach(Integer(2),Integer(3)),PayEnergy(Integer(1))]),ActionList([DrawACard]))");
    let stage = pipeline::Intermediate::new(&source).unwrap();
    let root = stage
        .structural
        .operands
        .iter()
        .find(|r| r.rule == "operand.Cost.And")
        .unwrap();
    assert!(
        matches!(&root.recipe,pipeline::operands::OperandRecipe::Cost(R::All(items)) if matches!(items.as_slice(),[R::Amount {..},R::Source(source::Cost::PayEnergy(..))]))
    );
    assert!(stage.final_card.is_none());
    assert!(stage.diagnostics.iter().any(|d| d.form == "PayEnergy"));
    for cost in [
        "TapPermanent(Ref_TargetPermanent)",
        "PayManaX([ManaCostGeneric(1)],Integer(2))",
    ] {
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
fn schema_24_costs_roundtrip_and_reject_previous_versions_at_validation() {
    let source=custom("Activated(And([UntapPermanent(ThisPermanent),TapAPermanent(AnyPermanent)]),ActionList([DrawACard]))");
    let card = pipeline::finalize(&source).unwrap();
    assert_eq!(card.schema_version, 24);
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
        if ["Birchlore Rangers", "Cryptic Gateway", "Leech Bonder"].contains(&name.as_str()) {
            assert!(pipeline::finalize(&source).is_err(), "{name}");
            continue;
        }
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
    assert_eq!(complete, 21);
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
