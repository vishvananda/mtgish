#[path = "support/fresh_producer.rs"]
mod fresh_producer;
use mtgish_ast::{canonical, corpus, human, ir::*, migrate, mtg_types, unified, validate};
use serde_json::Value;

fn card(action: &str) -> String {
    format!("Card{{name:\"Keyword values\",properties:{{card_types:[Instant]}},abilities:[Spell{{do:[{action}]}}]}}")
}

#[test]
fn actions_expose_only_their_parameters_and_keep_counted_forms_distinct() {
    for action in [
        "Keyword{actor:You,action:Fight{first:This,second:This}}",
        "Keyword{actor:You,action:Amass{amount:3,creature_type:Zombie}}",
        "Keyword{actor:You,action:Investigate{times:None}}",
        "Keyword{actor:You,action:Investigate{times:Some(1)}}",
        "KeywordEach{actor:You,action:Goad{until:Some(EndOfTurn)},objects:One(This)}",
    ] {
        let parsed = human::read(&card(action)).unwrap_or_else(|e| panic!("{action}: {e}"));
        let encoded = bincode::encode_to_vec(&parsed, bincode::config::standard()).unwrap();
        let (decoded, used): (UnifiedCard, _) =
            bincode::decode_from_slice(&encoded, bincode::config::standard()).unwrap();
        assert_eq!(used, encoded.len());
        assert_eq!(
            serde_json::to_value(parsed).unwrap(),
            serde_json::to_value(decoded).unwrap()
        );
    }
    for wrong in [
        "Keyword{actor:You,action:Fight{first:This}}",
        "Keyword{actor:You,action:Fight{first:This,second:This,amount:2}}",
        "Keyword{actor:You,action:Amass{amount:3,creature_type:InventedCreature}}",
        "Keyword{actor:You,action:Fight,object:Some(This),other:Some(This)}",
        "KeywordEach{actor:You,action:Fight,objects:One(This)}",
        "KeywordEach{actor:You,action:Transform{until:Some(EndOfTurn)},objects:One(This)}",
    ] {
        assert!(human::read(&card(wrong)).is_err(), "{wrong}");
    }
    let once = human::read(&card("Keyword{actor:You,action:Investigate{times:None}}")).unwrap();
    let counted = human::read(&card(
        "Keyword{actor:You,action:Investigate{times:Some(1)}}",
    ))
    .unwrap();
    assert_ne!(
        serde_json::to_value(once).unwrap(),
        serde_json::to_value(counted).unwrap()
    );
    let error = human::read(&card(
        "Keyword{actor:You,action:Fight{first:Target{slot:t99,index:0},second:This}}",
    ))
    .unwrap_err();
    assert!(error.contains("unbound target slot 99"), "{error}");
}

#[test]
fn every_captured_parameter_shape_upgrades_to_exact_fresh_source() {
    corpus::on_large_stack(|| {
        let sources: Vec<mtg_types::OracleCard> = include_str!("fixtures/keyword_payloads.ronl")
            .lines()
            .map(|l| ron::from_str(l).unwrap())
            .collect();
        let mut count = 0;
        for line in include_str!("fixtures/keyword_payloads_v182.jsonl").lines() {
            let old: Value = serde_json::from_str(line).unwrap();
            let name = old["faces"][0]["name"].as_str().unwrap();
            let source = sources
                .iter()
                .find(|s| {
                    corpus::Identity::of(s)
                        .unwrap()
                        .names
                        .iter()
                        .any(|n| n == name)
                })
                .unwrap();
            let mapped = migrate::mapped_read_json(line).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(mapped.source_version, 182);
            assert!(mapped.map.introduced.is_empty());
            let current = serde_json::to_value(&mapped.card).unwrap();
            fresh_producer::assert_matches("keyword_payloads_v182.jsonl", &current, &serde_json::to_value(unified::lower(source).card).unwrap());
            for (origin, targets) in &mapped.map.sites {
                for target in targets {
                    assert_eq!(
                        old.pointer(origin),
                        current.pointer(target),
                        "{name}: {origin} -> {target}"
                    );
                }
            }
            let text = human::write(&mapped.card).unwrap();
            let back = human::read(&text).unwrap_or_else(|e| panic!("{name}: {e}\n{text}"));
            assert_eq!(
                serde_json::to_value(canonical::mapped_unified(&back).unwrap().card).unwrap(),
                serde_json::to_value(canonical::mapped_unified(&mapped.card).unwrap().card)
                    .unwrap(),
                "{name}"
            );
            count += 1;
        }
        assert_eq!(count, 151);
    });
}

#[test]
fn captured_names_and_scopes_survive_parameter_relocation() {
    corpus::on_large_stack(|| {
        let mut count = 0;
        for old in include_str!("fixtures/keyword_documents_v182.jsonl").lines() {
            let value: Value = serde_json::from_str(old).unwrap();
            let name = &value["card"]["faces"][0]["name"];
            let document =
                human::document::Document::from_json(old).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(
                serde_json::to_value(document.bindings()).unwrap(),
                value["bindings"],
                "{name}"
            );
            let saved = serde_json::to_string(&document).unwrap();
            let text = document.write().unwrap();
            assert_eq!(
                human::document::Document::from_json(&saved)
                    .unwrap()
                    .write()
                    .unwrap(),
                text
            );
            let back = human::document::read(&text).unwrap();
            assert_eq!(
                serde_json::to_value(canonical::mapped_unified(back.card()).unwrap().card).unwrap(),
                serde_json::to_value(canonical::mapped_unified(document.card()).unwrap().card)
                    .unwrap(),
                "{name}"
            );
            count += 1;
        }
        assert_eq!(count, 151);
    });
}

#[test]
fn invalid_legacy_shapes_and_primitive_escapes_are_refused() {
    let mut old: Value = serde_json::from_str(
        include_str!("fixtures/keyword_payloads_v182.jsonl")
            .lines()
            .find(|l| l.contains("\"Fight\""))
            .unwrap(),
    )
    .unwrap();
    fn corrupt(value: &mut Value) -> bool {
        if value["kind"] == "Keyword" && value["args"]["action"] == "Fight" {
            value["args"]["other"] = Value::Null;
            return true;
        }
        match value {
            Value::Array(items) => items.iter_mut().any(corrupt),
            Value::Object(fields) => fields.values_mut().any(corrupt),
            _ => false,
        }
    }
    assert!(corrupt(&mut old));
    assert!(migrate::read_json(&old.to_string())
        .unwrap_err()
        .contains("invalid parameters"));
    let mut card = human::read(&card("Keyword{actor:You,action:Investigate{times:None}}")).unwrap();
    let Ability::Spell { body, .. } = &mut card.faces[0].abilities[0] else {
        panic!()
    };
    let Instruction::Effect { operation, .. } = &mut body.instructions[0] else {
        panic!()
    };
    *operation = Operation::Primitive {
        actor: PlayerSet::One(PlayerRef::AbilityController),
        name: "Action::Fight".into(),
        operands: vec![],
    };
    assert!(validate::unified(&card)
        .unwrap_err()
        .contains("typed keyword action"));
}

#[test]
fn a_source_subtype_must_use_the_declared_creature_type_family() {
    let operation = mtgish_ast::semantics::reduce_action(
        PlayerSet::One(PlayerRef::AbilityController),
        "Action::Amass".into(),
        vec![
            Operand::Number(Expr::Constant(3)),
            Operand::Vocabulary {
                family: "Color".into(),
                form: "Zombie".into(),
            },
        ],
    );
    let mut card = human::read(&card("Keyword{actor:You,action:Investigate{times:None}}")).unwrap();
    let Ability::Spell { body, .. } = &mut card.faces[0].abilities[0] else {
        panic!()
    };
    let Instruction::Effect {
        operation: slot, ..
    } = &mut body.instructions[0]
    else {
        panic!()
    };
    *slot = operation;
    assert!(validate::unified(&card)
        .unwrap_err()
        .contains("typed keyword action"));
}

#[test]
fn optional_adapt_and_monstrosity_subjects_retain_explicit_legacy_intent() {
    for old in include_str!("fixtures/keyword_subject_documents_v182.jsonl").lines() {
        let before: Value = serde_json::from_str(old).unwrap();
        let document = human::document::Document::from_json(old).unwrap();
        assert_eq!(
            serde_json::to_value(document.bindings()).unwrap(),
            before["bindings"]
        );
        let old_operation = &before["card"]["faces"][0]["abilities"][0]["args"]["body"]
            ["instructions"][0]["args"]["operation"]["args"];
        let action = old_operation["action"].as_str().unwrap();
        let after = serde_json::to_value(document.card()).unwrap();
        assert_eq!(
            after["faces"][0]["abilities"][0]["args"]["body"]["instructions"][0]["args"]
                ["operation"]["args"]["action"][action]["subject"],
            old_operation["object"]
        );
        let text = document.write().unwrap();
        assert!(text.contains("other_creature"));
        let parsed = human::document::read(&text).unwrap();
        assert_eq!(
            serde_json::to_value(canonical::mapped_unified(parsed.card()).unwrap().card).unwrap(),
            serde_json::to_value(canonical::mapped_unified(document.card()).unwrap().card).unwrap()
        );
        #[cfg(feature = "cantrip")]
        assert!(format!(
            "{:?}",
            mtgish_ast::cantrip::module(&document.card().faces[0]).unwrap_err()
        )
        .contains(&format!(
            "{} of another object",
            action.to_ascii_lowercase()
        )));
    }
}
