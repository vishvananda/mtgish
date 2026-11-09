use mtgish_ast::{canonical, human, ir::*, migrate, mtg_types as source, unified, validate};
use serde_json::{json, Value};

fn spell(body: &str) -> String {
    format!("Card{{name:\"Copy test\",properties:{{card_types:[Instant]}},abilities:[Spell{{do:[{body}]}}]}}")
}
fn copy(exceptions: &str) -> String {
    format!("CreateTokens{{players:You,tokens:[{{amount:1,words:CreatingEffect,recipe:Copy{{object:This,exceptions:[{exceptions}]}}}}]}}")
}

#[test]
fn copy_expressions_use_the_creator_but_granted_abilities_are_fresh() {
    let valid = spell(&format!(
        "size=BindValue{{value:4}},{}",
        copy("SetPowerToughness{power:BoundValue(size),toughness:2}")
    ));
    let doc = human::document::read(&valid).unwrap();
    assert!(doc.bindings().iter().any(|b| b.name == "size"));
    assert!(doc.write().unwrap().contains("BoundValue(size)"));
    assert!(human::read(&valid.replace("BoundValue(size)", "BoundValue(missing)")).is_err());
    let leaked=spell(&format!("size=BindValue{{value:4}},{}",copy("AddAbilities([Activated{origin:Ordinary,cost:{parts:[TapSymbol]},do:[DrawCards{players:You,amount:BoundValue(size)}]}])")));
    assert!(human::read(&leaked).is_err());
    human::read(&leaked.replace("BoundValue(size)", "2")).unwrap();
}

#[test]
fn copy_fields_refuse_operand_bags_and_incompatible_type_combinations() {
    for exception in [
        "Integer(42)",
        "Term{family:\"TokenCopyEffect\",form:\"SetPT\",operands:[]}",
        "SetTypes{card_types:[Artifact],subtypes:[Zombie]}",
        "Colors{mode:Set,colors:Listed([Chosen])}",
        "Colors{mode:Set,colors:Listed([Colorless])}",
    ] {
        assert!(
            human::read(&spell(&copy(exception))).is_err(),
            "{exception}"
        );
    }
    human::read(&spell(&copy(
        "SetTypes{card_types:[Creature],subtypes:[Zombie]},Colors{mode:Set,colors:Listed([])}",
    )))
    .unwrap();
}

#[test]
fn copy_color_result_requires_mana_provenance() {
    let effect = copy("Colors{mode:Set,colors:ManaFrom(colors)}");
    human::read(&spell(&format!(
        "colors=AddMana{{players:You,mana:Amount{{color:Blue,amount:1}}}},{effect}"
    )))
    .unwrap();
    assert!(human::read(&spell(&format!(
        "colors=DrawCards{{players:You,amount:1}},{effect}"
    )))
    .is_err());
}

#[test]
fn all_captured_copy_payloads_upgrade_to_fresh_lowering_without_moving_ids() {
    mtgish_ast::corpus::on_large_stack(|| {
        for (old, source) in include_str!("fixtures/copy_payloads_v180.jsonl")
            .lines()
            .zip(include_str!("fixtures/typed_copy.ronl").lines())
        {
            let before: Value = serde_json::from_str(old).unwrap();
            let mapped = migrate::mapped_read_json(old)
                .unwrap_or_else(|e| panic!("{}: {e}", before["faces"][0]["name"]));
            assert_eq!(mapped.source_version, 180);
            let after = serde_json::to_value(&mapped.card).unwrap();
            let source: source::OracleCard = ron::from_str(source).unwrap();
            let fresh = unified::lower(&source).card;
            assert_eq!(
                after,
                serde_json::to_value(fresh).unwrap(),
                "{}",
                before["faces"][0]["name"]
            );
            for (origin, paths) in &mapped.map.sites {
                let id = before.pointer(origin).unwrap();
                for path in paths {
                    assert_eq!(Some(id), after.pointer(path), "{origin} -> {path}");
                }
            }
            assert!(mapped.map.introduced.is_empty());
            validate::unified(&mapped.card).unwrap();
            let text = human::write(&mapped.card).unwrap();
            assert!(!text.contains("TokenCopyEffects"));
            assert!(!text.contains("CopyEffects"));
            let back = human::read(&text).unwrap();
            for (a, b) in mapped.card.faces.iter().zip(back.faces.iter()) {
                assert_eq!(
                    serde_json::to_value(canonical::card(a).unwrap()).unwrap(),
                    serde_json::to_value(canonical::card(b).unwrap()).unwrap()
                );
            }
        }
    });
}

#[test]
fn copy_migration_rejects_mixed_epochs_unknown_fields_and_wrong_families() {
    let original: Value = serde_json::from_str(
        include_str!("fixtures/copy_payloads_v180.jsonl")
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    let mut mixed = original.clone();
    mixed["faces"][0]["schema_version"] = json!(181);
    assert!(migrate::read_json(&mixed.to_string()).is_err());
    let mut extra = original.clone();
    extra["surprise"] = json!(true);
    assert!(migrate::read_json(&extra.to_string()).is_err());
    fn forge(v: &mut Value) -> bool {
        match v {
            Value::Object(fields)
                if fields
                    .get("family")
                    .and_then(Value::as_str)
                    .is_some_and(|s| matches!(s, "CopyEffects" | "TokenCopyEffects")) =>
            {
                fields.insert("family".into(), json!("InventedCopyEffects"));
                true
            }
            Value::Object(fields) => fields.values_mut().any(forge),
            Value::Array(values) => values.iter_mut().any(forge),
            _ => false,
        }
    }
    let mut wrong = original;
    assert!(forge(&mut wrong));
    assert!(migrate::read_json(&wrong.to_string()).is_err());
}

#[test]
fn source_copy_forms_must_belong_to_their_declared_family() {
    use Operand as O;
    let exception = O::Vocabulary {
        family: "CopyEffect".into(),
        form: "RemoveThisAbility".into(),
    };
    assert!(mtgish_ast::copying::from_source(&[exception]).is_err());
    let exception = O::Vocabulary {
        family: "TokenCopyEffect".into(),
        form: "RemoveThisAbility".into(),
    };
    mtgish_ast::copying::from_source(std::slice::from_ref(&exception)).unwrap();
    let wrapped = O::Term {
        family: "CopyEffects".into(),
        form: "CopyEffects".into(),
        operands: vec![exception],
    };
    assert!(mtgish_ast::copying::from_source(&[wrapped]).is_err());
}

#[test]
fn shared_ptx_values_are_expanded_into_both_expressions() {
    use Operand as O;
    let x = O::Term {
        family: "PTXValue".into(),
        form: "X".into(),
        operands: vec![],
    };
    let input = O::Term {
        family: "CopyEffect".into(),
        form: "SetPT".into(),
        operands: vec![O::Term {
            family: "PT".into(),
            form: "PTX".into(),
            operands: vec![x.clone(), x, O::Number(Expr::BoundValue(ResultId(3)))],
        }],
    };
    let result = mtgish_ast::copying::from_source(&[input]).unwrap();
    assert!(matches!(
        result.as_slice(),
        [CopyException::SetPowerToughness {
            power: Expr::BoundValue(ResultId(3)),
            toughness: Expr::BoundValue(ResultId(3))
        }]
    ));
}

#[test]
fn named_schema_180_documents_retain_every_binding_through_the_payload_moves() {
    mtgish_ast::corpus::on_large_stack(|| {
        for source in include_str!("fixtures/copy_documents_v180.jsonl").lines() {
            let old: Value = serde_json::from_str(source).unwrap();
            let document = human::document::Document::from_json(source)
                .unwrap_or_else(|e| panic!("{}: {e}", old["card"]["faces"][0]["name"]));
            assert_eq!(document.card().schema_version, SCHEMA_VERSION);
            assert_eq!(
                serde_json::to_value(document.bindings()).unwrap(),
                old["bindings"]
            );
            let expected = migrate::read_json(&old["card"].to_string()).unwrap();
            assert_eq!(
                serde_json::to_value(document.card()).unwrap(),
                serde_json::to_value(expected).unwrap()
            );
            let output = document.write().unwrap();
            for binding in document.bindings().iter().filter(|b| b.explicit) {
                assert!(output.contains(&binding.name));
            }
            let saved = serde_json::to_string(&document).unwrap();
            assert_eq!(
                human::document::Document::from_json(&saved)
                    .unwrap()
                    .write()
                    .unwrap(),
                output
            );
        }
    });
}

#[test]
fn copy_migration_clones_shared_reads_without_aliasing_reused_labels_in_a_quote() {
    let source = include_str!("fixtures/copy_documents_v180.jsonl")
        .lines()
        .find(|line| line.contains("Nested PTX"))
        .unwrap();
    let before: Value = serde_json::from_str(source).unwrap();
    let mapped = migrate::mapped_read_json(&before["card"].to_string()).unwrap();
    assert_eq!(
        mapped
            .map
            .sites
            .values()
            .filter(|paths| paths.len() == 2)
            .count(),
        2
    );
    let document = human::document::Document::from_json(source).unwrap();
    let code = document.write().unwrap();
    assert_eq!(code.matches("BoundValue(outer_size)").count(), 2);
    assert_eq!(code.matches("BoundValue(inner_size)").count(), 2);
    let outer = before["bindings"]
        .as_array()
        .unwrap()
        .iter()
        .position(|b| b["name"] == "outer_size")
        .unwrap();
    let inner = before["bindings"]
        .as_array()
        .unwrap()
        .iter()
        .position(|b| b["name"] == "inner_size")
        .unwrap();
    let mut forged = before;
    let use_ = forged["uses"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|u| u["binding"] == inner && u["path"].as_str().unwrap().ends_with("/args"))
        .unwrap();
    use_["binding"] = json!(outer);
    assert!(human::document::Document::from_json(&forged.to_string()).is_err());
}

#[test]
#[ignore = "explicit full before/current normalized corpora"]
fn full_copy_upgrade_matches_every_source_record_and_preserves_origins() {
    use std::io::BufRead;
    mtgish_ast::corpus::on_large_stack(|| {
        let before = std::fs::File::open(std::env::var("COPY_BEFORE_CORPUS").unwrap()).unwrap();
        let after = std::fs::File::open(std::env::var("COPY_CURRENT_CORPUS").unwrap()).unwrap();
        let mut old = std::io::BufReader::new(before).lines();
        let mut new = std::io::BufReader::new(after).lines();
        let mut count = 0;
        loop {
            match (old.next(), new.next()) {
                (None, None) => break,
                (Some(a), Some(b)) => {
                    let original = a.unwrap();
                    let before: Value = serde_json::from_str(&original).unwrap();
                    let mapped = migrate::mapped_read_json(&original)
                        .unwrap_or_else(|e| panic!("{}: {e}", before["faces"][0]["name"]));
                    let actual = serde_json::to_value(&mapped.card).unwrap();
                    let expected: Value = serde_json::from_str(&b.unwrap()).unwrap();
                    assert_eq!(actual, expected, "{}", before["faces"][0]["name"]);
                    for (path, targets) in &mapped.map.sites {
                        for target in targets {
                            assert_eq!(
                                before.pointer(path),
                                actual.pointer(target),
                                "{path} -> {target}"
                            );
                        }
                    }
                    count += 1;
                }
                _ => panic!("corpus record counts differ"),
            }
        }
        assert_eq!(count, 33529);
        println!("Upgraded all {count} records with exact fresh-source output and complete typed origins");
    });
}
