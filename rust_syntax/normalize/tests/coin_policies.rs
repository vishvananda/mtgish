use mtgish_ast::{
    canonical, corpus,
    human::{self, document::Document},
    ir::*,
    migrate, mtg_types, unified, validate,
};
use serde_json::{json, Value};

#[test]
fn authentic_coin_records_upgrade_to_exact_source_and_preserve_all_binding_origins() {
    corpus::on_large_stack(|| {
        let sources = include_str!("fixtures/coin_policies.ronl").lines();
        let old = include_str!("fixtures/coin_policies_v185.jsonl").lines();
        assert_eq!(sources.clone().count(), 76);
        for (source, old) in sources.zip(old) {
            let source: mtg_types::OracleCard = ron::from_str(source).unwrap();
            let before: Value = serde_json::from_str(old).unwrap();
            let mapped = migrate::mapped_read_json(old)
                .unwrap_or_else(|e| panic!("{}: {e}", before["faces"]));
            let fresh = unified::lower(&source);
            assert!(fresh.diagnostics.is_empty(), "{:?}", fresh.diagnostics);
            let after = serde_json::to_value(&mapped.card).unwrap();
            let fresh_value = serde_json::to_value(&fresh.card).unwrap();
            let mut expected_source = after.clone();
            if fresh.card.faces[0].name == "Negative Zone Portal" {
                // Saved v185 meaning stays Before, including its origin map.
                // Fresh Source now tests the card in exile (2026-11-12 fix).
                let phase = "/faces/0/abilities/0/args/body/instructions/1/args/condition/args/left/args/args/objects/args/phase";
                assert_eq!(after.pointer(phase).unwrap(), "Before");
                assert_eq!(fresh_value.pointer(phase).unwrap(), "After");
                *expected_source.pointer_mut(phase).unwrap() = json!("After");
            }
            assert_eq!(expected_source, fresh_value, "{}", before["faces"]);
            assert!(mapped.map.introduced.is_empty());
            for (from, to) in mapped.map.sites {
                for to in to {
                    assert_eq!(before.pointer(&from), after.pointer(&to), "{from} -> {to}");
                }
            }
            let text = human::write(&fresh.card).unwrap();
            let back = human::read(&text).unwrap();
            assert_eq!(
                serde_json::to_value(canonical::unified(&fresh.card).unwrap()).unwrap(),
                serde_json::to_value(canonical::unified(&back).unwrap()).unwrap()
            );
        }
    });
}
#[test]
fn authentic_named_documents_retain_explicit_names_and_scope_keys() {
    corpus::on_large_stack(|| {
        for old in include_str!("fixtures/coin_documents_v185.jsonl").lines() {
            let before: Value = serde_json::from_str(old).unwrap();
            let doc = Document::from_json(old).unwrap();
            assert_eq!(
                serde_json::to_value(doc.bindings()).unwrap(),
                before["bindings"]
            );
            let back = human::document::read(&doc.write().unwrap()).unwrap();
            let names = |d: &Document| {
                d.bindings()
                    .iter()
                    .map(|b| format!("{:?} {:?} {} {}", b.kind, b.scope_name, b.name, b.explicit))
                    .collect::<Vec<_>>()
            };
            assert_eq!(names(&doc), names(&back));
        }
    });
}
fn card(operation: Operation) -> UnifiedCard {
    let mut c = human::read(
        "Card{name:\"Coin protocol\",properties:{card_types:[Instant]},abilities:[Spell{do:[]}]}",
    )
    .unwrap();
    let Ability::Spell { body, .. } = &mut c.faces[0].abilities[0] else {
        panic!()
    };
    body.instructions.push(Instruction::Effect {
        result: ResultId(0),
        operation,
    });
    c
}
#[test]
fn repetition_is_finite_or_has_a_named_stop_and_counts_are_nonnegative() {
    for protocol in [CoinProtocol::Called, CoinProtocol::Uncalled] {
        for n in [-1, 0, 1] {
            for repetition in [
                CoinRepetition::Count(Expr::Constant(n)),
                CoinRepetition::UpTo {
                    extent: CoinExtent::Count(Expr::Constant(n)),
                    stop: CoinStop::LoseOrPlayerStops,
                },
            ] {
                let c = card(Operation::FlipCoinSeries {
                    players: PlayerSet::All,
                    protocol,
                    repetition,
                });
                assert_eq!(validate::unified(&c).is_ok(), n >= 0);
            }
        }
        for stop in [
            CoinStop::Lose,
            CoinStop::PlayerStops,
            CoinStop::LoseOrPlayerStops,
        ] {
            let c = card(Operation::FlipCoinSeries {
                players: PlayerSet::All,
                protocol,
                repetition: CoinRepetition::Until(stop),
            });
            validate::unified(&c).unwrap();
            let back = human::read(&human::write(&c).unwrap()).unwrap();
            assert_eq!(
                serde_json::to_value(canonical::unified(&c).unwrap()).unwrap(),
                serde_json::to_value(canonical::unified(&back).unwrap()).unwrap()
            );
        }
    }
    let old: Value = serde_json::from_str(
        include_str!("fixtures/coin_policies_v185.jsonl")
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    let mut c = serde_json::to_value(card(Operation::FlipCoin {
        player: PlayerRef::AbilityController,
        protocol: CoinProtocol::Called,
    }))
    .unwrap();
    c["schema_version"] = old["schema_version"].clone();
    c["faces"][0]["schema_version"] = old["schema_version"].clone();
    for (times, each) in [
        (
            json!({"kind":"Constant","args":1}),
            json!({"kind":"One","args":{"kind":"Source"}}),
        ),
        (Value::Null, Value::Null),
    ] {
        c["faces"][0]["abilities"][0]["args"]["body"]["instructions"][0]["args"]["operation"] = json!({"kind":"CoinFlips","args":{"actor":{"kind":"All"},"times":times,"each":each,"until_lost":false,"may_stop":false,"called":false}});
        let expected = if times.is_null() {
            "finite extent or an explicit stop"
        } else {
            "both a count and per-object extent"
        };
        let error = migrate::read_json(&c.to_string()).unwrap_err();
        assert!(error.contains(expected), "{error}");
    }
}

#[test]
fn scoped_legacy_observations_do_not_confuse_reused_ids_or_future_flips() {
    let mut c = card(Operation::FlipCoin {
        player: PlayerRef::AbilityController,
        protocol: CoinProtocol::Called,
    });
    let branch = |outcome| Instruction::If {
        condition: Condition::CoinFlip {
            result: ResultId(0),
            outcome,
        },
        then: Program::empty(),
        otherwise: Program::empty(),
    };
    let Ability::Spell { body, .. } = &mut c.faces[0].abilities[0] else {
        panic!()
    };
    body.instructions.push(branch(CoinOutcome::Heads));
    let mut second = c.faces[0].abilities[0].clone();
    let Ability::Spell { body, .. } = &mut second else {
        panic!()
    };
    body.instructions[1] = branch(CoinOutcome::Won);
    c.faces[0].abilities.push(second);
    // Loading a current document preserves its explicitly called protocol,
    // even with a face observation that the current backend cannot supply.
    let current = human::read(&human::write(&c).unwrap()).unwrap();
    assert!(format!("{current:?}").contains("protocol: Called"));
    let mut old = serde_json::to_value(&c).unwrap();
    old["schema_version"] = json!(185);
    old["faces"][0]["schema_version"] = json!(185);
    for ability in old["faces"][0]["abilities"].as_array_mut().unwrap() {
        ability["args"]["body"]["instructions"][0]["args"]["operation"]["args"]
            .as_object_mut()
            .unwrap()
            .remove("protocol");
    }
    let upgraded = migrate::read_json(&old.to_string()).unwrap();
    let operation = |i: usize| match &upgraded.faces[0].abilities[i] {
        Ability::Spell { body, .. } => &body.instructions[0],
        _ => panic!(),
    };
    assert!(matches!(
        operation(0),
        Instruction::Effect {
            operation: Operation::FlipCoin {
                protocol: CoinProtocol::Uncalled,
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        operation(1),
        Instruction::Effect {
            operation: Operation::FlipCoin {
                protocol: CoinProtocol::Called,
                ..
            },
            ..
        }
    ));
}

#[cfg(feature = "cantrip")]
#[test]
fn generator_uses_explicit_protocol_without_printed_text_and_reports_unavailable_channels() {
    for (protocol, expected, other) in [
        (
            CoinProtocol::Called,
            "g.flip_coin(",
            "g.flip_coin_for_heads(",
        ),
        (
            CoinProtocol::Uncalled,
            "g.flip_coin_for_heads(",
            "g.flip_coin(",
        ),
    ] {
        let mut c = card(Operation::FlipCoin {
            player: PlayerRef::AbilityController,
            protocol,
        });
        let output = mtgish_ast::cantrip::module(&c.faces[0]).unwrap();
        assert!(output.contains(expected), "{output}");
        assert!(!output.contains(other), "{output}");
        let Ability::Spell { body, .. } = &mut c.faces[0].abilities[0] else {
            panic!()
        };
        body.instructions.push(Instruction::If {
            condition: Condition::CoinFlip {
                result: ResultId(0),
                outcome: if protocol == CoinProtocol::Called {
                    CoinOutcome::Heads
                } else {
                    CoinOutcome::Won
                },
            },
            then: Program {
                instructions: vec![Instruction::Effect {
                    result: ResultId(1),
                    operation: Operation::Draw {
                        players: PlayerSet::One(PlayerRef::AbilityController),
                        amount: Expr::Constant(1),
                    },
                }],
            },
            otherwise: Program::empty(),
        });
        validate::unified(&c).unwrap();
        assert!(!mtgish_ast::cantrip::gaps(&c.faces[0]).is_empty());
    }
}

#[cfg(feature = "cantrip")]
#[test]
#[ignore = "writes generator comparison evidence; set COIN_GENERATOR_OUTPUT"]
fn all_coin_records_generate_with_the_current_explicit_policies() {
    let output = std::env::var("COIN_GENERATOR_OUTPUT").unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let cache = mtgish_ast::cantrip::pr_cache::default_dir();
    let n = mtgish_ast::cantrip::planned::lane_rows(
        &root.join("tests/fixtures/coin_policies.ronl"),
        &root.join("../../data/oracle.json"),
        std::path::Path::new(&output),
        cache.as_deref(),
    )
    .unwrap();
    assert_eq!(n, 76);
}

#[test]
fn implicit_program_tally_is_distinct_from_a_required_receipt() {
    for prefix in ["", "DrawCards{players:You,amount:1},"] {
        let code=format!("Card{{name:\"Coin scope\",properties:{{card_types:[Instant]}},abilities:[Spell{{do:[{prefix}DrawCards{{players:You,amount:CountCoinOutcomes{{source:Latest,outcome:Won}}}}]}}]}}");
        human::read(&code).unwrap();
    }
}

#[test]
fn standalone_source_programs_use_the_same_scoped_protocol_inference() {
    let actions: mtg_types::Actions = ron::from_str(
        "ActionList([FlipCoins(Integer(2)),DrawNumberCards(TheNumberOfCoinsThatCameUpHeads)])",
    )
    .unwrap();
    let operand = mtgish_ast::lower::operand(&mtgish_ast::inspect::value(&actions).unwrap());
    let Operand::Program(body) = operand else {
        panic!("expected a fully lowered program")
    };
    assert!(matches!(
        &body.instructions[0],
        Instruction::Effect {
            operation: Operation::FlipCoins {
                protocol: CoinProtocol::Uncalled,
                ..
            },
            ..
        }
    ));
}
