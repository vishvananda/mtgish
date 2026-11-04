use mtgish_ast::{change_lists, ir::*, mtg_types::OracleCard, semantics, unified, validate};
use serde_json::{json, Value};
#[path = "support/query_aliases.rs"]
mod query_aliases;
fn migration(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            if let Some(stamp) = fields.get_mut("schema_version") {
                *stamp = json!(SCHEMA_VERSION);
            }
            fields.values_mut().for_each(migration);
        }
        Value::Array(values) => {
            for mut value in std::mem::take(values) {
                migration(&mut value);
                if value["kind"] == "OnlyAbilities" {
                    values.push(json!({"kind":"RemoveAllAbilities"}));
                    for ability in value["args"].as_array().unwrap() {
                        values.push(if ability["kind"] == "Keyword" {
                            json!({"kind":"GrantKeyword","args":ability["args"]})
                        } else {
                            json!({"kind":"GrantAbility","args":ability})
                        });
                    }
                } else {
                    values.push(value);
                }
            }
        }
        _ => {}
    }
}
#[test]
fn complete_old_producer_records_expand_only_inside_their_original_change_lists() {
    let sources: Vec<_> = include_str!("fixtures/ability_aliases.ronl")
        .lines()
        .collect();
    let before: Vec<_> = include_str!("fixtures/ability_expansion_before.jsonl")
        .lines()
        .collect();
    assert_eq!((sources.len(), before.len()), (11, 11));
    for (line, before) in sources.into_iter().zip(before) {
        let source: OracleCard = ron::from_str(line).unwrap();
        let actual = unified::lower(&source);
        assert!(actual.diagnostics.is_empty(), "{:?}", actual.diagnostics);
        let mut expected: Value = serde_json::from_str(before).unwrap();
        migration(&mut expected);
        query_aliases::movement_defaults(&mut expected);
        assert_eq!(
            serde_json::to_value(&actual.card).unwrap(),
            expected,
            "{line}"
        );
        validate::unified(&actual.card).unwrap();
        unified::round_trip(&actual.card).unwrap();
    }
}
#[test]
fn replacing_abilities_is_an_ordered_list_in_every_producer_family() {
    for family in ["LayerEffect", "StaticLayerEffect", "SpellEffect"] {
        let changes = semantics::reduce_changes(
            format!("{family}::AddAbilityAndLoseAllOtherAbilities"),
            vec![
                Operand::Ability(Box::new(Ability::Keyword(Keyword::Flying))),
                Operand::Ability(Box::new(Ability::Keyword(Keyword::Haste))),
            ],
        );
        assert!(matches!(
            changes.as_slice(),
            [
                Change::RemoveAllAbilities,
                Change::GrantKeyword(Keyword::Flying),
                Change::GrantKeyword(Keyword::Haste)
            ]
        ));
    }
    assert!(matches!(
        change_lists::replace_abilities(vec![]).as_slice(),
        [Change::RemoveAllAbilities]
    ));
    for operands in [vec![], vec![Operand::Text("not an ability".into())]] {
        assert!(matches!(
            semantics::reduce_changes(
                "LayerEffect::AddAbilityAndLoseAllOtherAbilities".into(),
                operands
            )
            .as_slice(),
            [Change::Primitive { .. }]
        ));
    }
}
#[test]
fn entry_projection_retains_only_grants_after_the_last_direct_loss_and_keeps_barriers() {
    let nested = Change::Copiable(vec![
        Change::RemoveAllAbilities,
        Change::GrantKeyword(Keyword::Flying),
    ]);
    let continuing = Change::Continuing {
        changes: vec![Change::RemoveAllAbilities],
        until: Duration::EndOfTurn,
    };
    let changes = vec![
        Change::GrantKeyword(Keyword::Flying),
        nested.clone(),
        continuing.clone(),
        Change::RemoveAllAbilities,
        Change::GrantKeyword(Keyword::Vigilance),
        Change::RemoveAllAbilities,
        Change::GrantKeyword(Keyword::Haste),
    ];
    let retained: Vec<_> = change_lists::entering_changes(&changes).collect();
    assert!(matches!(
        retained.as_slice(),
        [
            Change::Copiable(_),
            Change::Continuing { .. },
            Change::RemoveAllAbilities,
            Change::GrantKeyword(Keyword::Haste)
        ]
    ));
    let barriers = vec![Change::GrantKeyword(Keyword::Flying), nested, continuing];
    assert_eq!(change_lists::entering_changes(&barriers).count(), 3);
}
fn static_ability(changes: Vec<Change>, kind: StaticKind) -> Ability {
    Ability::Static {
        kind,
        affected: EffectSubjects::Objects(ObjectSet::One(ObjectRef::Source)),
        changes,
    }
}
fn base_card() -> Card {
    let source: OracleCard = ron::from_str(
        include_str!("fixtures/ability_aliases.ronl")
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    unified::lower(&source).card.faces.remove(0)
}
#[test]
fn ordinary_replacement_cannot_smuggle_an_intrinsic_definition_but_copiable_text_can() {
    let definition = static_ability(
        vec![Change::SetBasePowerToughness {
            power: Expr::Constant(2),
            toughness: Expr::Constant(3),
        }],
        StaticKind::CharacteristicDefining {
            keyword: None,
            contributes_to_color_identity: false,
        },
    );
    let replacement = change_lists::replace_abilities(vec![definition]);
    let mut card = base_card();
    card.abilities = vec![static_ability(replacement.clone(), StaticKind::Ordinary)];
    assert!(validate::card(&card)
        .unwrap_err()
        .contains("not an intrinsic characteristic definition"));
    card.abilities = vec![static_ability(
        vec![Change::Copiable(replacement)],
        StaticKind::Ordinary,
    )];
    validate::card(&card).unwrap();
}
#[test]
fn nested_ability_replacements_keep_ability_scopes_and_enchant_restrictions() {
    let hosts = EnchantHosts::Permanents(ObjectFilter::All(vec![
        ObjectFilter::CardType(mtgish_ast::mtg_types::CardType::Creature),
        ObjectFilter::Not(Box::new(ObjectFilter::SameObject(ObjectRef::Source))),
    ]));
    let enchant = Ability::Enchant { hosts };
    let inner = static_ability(
        change_lists::replace_abilities(vec![enchant.clone(), Ability::Keyword(Keyword::Flying)]),
        StaticKind::Ordinary,
    );
    let outer = change_lists::replace_abilities(vec![inner]);
    let mut card = base_card();
    card.abilities = vec![static_ability(outer, StaticKind::Ordinary)];
    validate::card(&card).unwrap();
    let value = serde_json::to_value(&card).unwrap();
    assert_eq!(
        value["abilities"][0]["args"]["changes"][1]["args"]["args"]["changes"][0]["kind"],
        "RemoveAllAbilities"
    );
    // Copiable text preserves the same typed enchant restriction; it is not
    // pulled out into an unrelated entry specification or surrounding scope.
    card.abilities = vec![static_ability(
        vec![Change::Copiable(change_lists::replace_abilities(vec![
            enchant,
        ]))],
        StaticKind::Ordinary,
    )];
    validate::card(&card).unwrap();
    let value = serde_json::to_value(&card).unwrap();
    assert_eq!(
        value["abilities"][0]["args"]["changes"][0]["args"][1]["args"]["args"]["hosts"]
            ["Permanents"]["kind"],
        "All"
    );
}
#[test]
fn canonical_change_binary_tags_require_the_new_schema() {
    let bytes = bincode::encode_to_vec(
        Change::GrantAbilityIfAbsent(Box::new(Ability::Keyword(Keyword::Flying))),
        bincode::config::standard(),
    )
    .unwrap();
    // Schema 174 removes OnlyAbilities before this binary discriminant.
    assert_eq!(bytes[0], 42);
}

#[cfg(feature = "cantrip")]
#[test]
fn every_supported_replacement_record_still_generates_a_complete_rust_module() {
    let mut modules = 0;
    let mut held = 0;
    for line in include_str!("fixtures/ability_aliases.ronl").lines() {
        let source: OracleCard = ron::from_str(line).unwrap();
        let card = unified::lower(&source).card;
        match mtgish_ast::cantrip::whole_module(&source, &card) {
            Ok(code) => {
                syn::parse_file(&code).unwrap();
                modules += 1;
            }
            Err(_) => {
                assert!(!mtgish_ast::cantrip::whole_gaps(&source, &card).is_empty());
                held += 1;
            }
        }
    }
    assert_eq!((modules, held), (7, 4));
}
