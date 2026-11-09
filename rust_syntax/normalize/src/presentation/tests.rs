//! Presentation survives exact IR round trips without becoming a new rules scope.
use crate::{canonical, ir::*, presentation, validate};
use serde_json::{json, Value};

fn draw() -> Ability {
    Ability::Spell {
        announcement: Announcement {
            targets: vec![],
            modes: None,
        },
        body: Program {
            instructions: vec![Instruction::Effect {
                result: ResultId(17),
                operation: Operation::Draw {
                    players: PlayerSet::One(PlayerRef::AbilityController),
                    amount: Expr::Constant(1),
                },
            }],
        },
    }
}

fn presented(ability: Ability, label: Option<&str>, reminder: bool) -> Ability {
    Ability::WithPresentation {
        label: label.map(str::to_owned),
        reminder,
        ability: Box::new(ability),
    }
}

fn value(ability: &Ability) -> Value {
    serde_json::to_value(vec![ability]).unwrap()
}

#[test]
fn labels_and_reminder_flags_roundtrip_through_typed_json_ron_and_binary() {
    for ability in [
        presented(draw(), Some("Test ability"), false),
        presented(draw(), None, true),
        presented(draw(), Some("Test flavor!"), true),
        Ability::Keyword(Keyword::WithReminder {
            keyword: Box::new(Keyword::Flying),
            reminder: true,
        }),
    ] {
        validate::validate_ability(&ability).unwrap();
        let encoded = serde_json::to_value(&ability).unwrap();
        let decoded: Ability = serde_json::from_value(encoded.clone()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), encoded);
        let decoded: Ability = ron::from_str(&ron::to_string(&ability).unwrap()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), encoded);
        let bytes = bincode::encode_to_vec(&ability, bincode::config::standard()).unwrap();
        let (decoded, used): (Ability, usize) =
            bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
        assert_eq!(used, bytes.len());
        assert_eq!(serde_json::to_value(decoded).unwrap(), encoded);
    }
}

#[test]
fn ordinary_canonicalization_preserves_presentation_and_meaning_erasure_is_explicit() {
    let mut plain = value(&draw());
    let mut labeled = value(&presented(draw(), Some("Test label"), false));
    canonical::abilities(&mut plain).unwrap();
    canonical::abilities(&mut labeled).unwrap();
    assert_ne!(plain, labeled);
    assert_eq!(labeled[0]["args"]["label"], "Test label");
    assert_eq!(labeled[0]["args"]["ability"], plain[0]);
    let preserved = labeled.clone();
    canonical::abilities(&mut labeled).unwrap();
    assert_eq!(labeled, preserved);
    canonical::semantic_abilities(&mut labeled).unwrap();
    assert_eq!(labeled, plain);
}

#[test]
fn presentation_does_not_change_a_trigger_subject_or_result_scope() {
    let mut plain = json!([{"kind":"Triggered","args":{
        "event":{"Object":{"event":"Attacks","objects":{"kind":"SameObject","args":{"kind":"Source"}}}},
        "body":{"instructions":[{"kind":"Effect","args":{"result":9,"operation":{"kind":"Destroy","args":{
            "objects":{"kind":"One","args":{"kind":"EventSubject"}},"regeneration":true}}}}]}}}]);
    let mut labeled = json!([{"kind":"WithPresentation","args":{
        "label":"Test label", "reminder":false,"ability":plain[0]}}]);
    canonical::abilities(&mut plain).unwrap();
    canonical::abilities(&mut labeled).unwrap();
    assert_eq!(labeled[0]["args"]["ability"], plain[0]);
    assert_eq!(
        plain[0]["args"]["body"]["instructions"][0]["args"]["operation"]["args"]["objects"]["args"]
            ["kind"],
        "Source"
    );
}

#[test]
fn nested_grants_keep_labels_and_each_keyword_instance_has_its_own_flag() {
    let keywords = Ability::Static {
        kind: StaticKind::Ordinary,
        affected: EffectSubjects::Objects(ObjectSet::One(ObjectRef::Source)),
        changes: vec![
            Change::GrantKeyword(Keyword::WithReminder {
                keyword: Box::new(Keyword::Flying),
                reminder: true,
            }),
            Change::GrantKeyword(Keyword::Flying),
            Change::GrantAbility(Box::new(presented(draw(), Some("Nested label"), false))),
        ],
    };
    validate::validate_ability(&keywords).unwrap();
    let mut exact = value(&keywords);
    canonical::abilities(&mut exact).unwrap();
    assert_eq!(
        exact[0]["args"]["changes"][0]["args"]["args"]["reminder"],
        true
    );
    assert_eq!(exact[0]["args"]["changes"][1]["args"]["kind"], "Flying");
    let mut semantic = exact.clone();
    canonical::semantic_abilities(&mut semantic).unwrap();
    let changes = semantic[0]["args"]["changes"].as_array().unwrap();
    assert_eq!(changes.len(), 3);
    assert_eq!(changes[0], changes[1]);
    assert_eq!(changes[2]["args"]["kind"], "Spell");
    assert_ne!(semantic, exact);
}

#[test]
fn an_invalid_inner_operand_is_still_refused() {
    let mut inner = draw();
    if let Ability::Spell { body, .. } = &mut inner {
        body.instructions = vec![Instruction::Effect {
            result: ResultId(0),
            operation: Operation::Destroy {
                objects: ObjectSet::One(ObjectRef::Target {
                    slot: TargetSlot(7),
                    index: 0,
                }),
                regeneration: true,
            },
        }];
    }
    let reason = validate::validate_ability(&inner).unwrap_err();
    assert_eq!(
        validate::validate_ability(&presented(inner, Some("Test label"), false)),
        Err(reason)
    );
}

#[test]
fn empty_nested_and_unbounded_presentation_is_rejected() {
    assert!(
        validate::validate_ability(&presented(Ability::Keyword(Keyword::Flying), None, true))
            .is_err()
    );
    for label in ["", " leading", "trailing ", "two\nlines", "label — clause"] {
        assert!(validate::validate_ability(&presented(draw(), Some(label), false)).is_err());
    }
    assert!(
        validate::validate_ability(&presented(draw(), Some(&"x".repeat(1025)), false)).is_err()
    );
    let nested = presented(
        presented(draw(), Some("Inner"), false),
        Some("Outer"),
        false,
    );
    assert!(validate::validate_ability(&nested).is_err());
    let wrapped = Keyword::WithReminder {
        keyword: Box::new(Keyword::Flying),
        reminder: true,
    };
    let nested = Keyword::WithReminder {
        keyword: Box::new(wrapped),
        reminder: true,
    };
    assert!(validate::validate_ability(&Ability::Keyword(nested)).is_err());
}

#[test]
fn clearing_a_flag_or_label_restores_the_plain_canonical_form() {
    for (mut annotated, mut plain) in [
        (value(&presented(draw(), None, false)), value(&draw())),
        (
            value(&Ability::Keyword(Keyword::WithReminder {
                keyword: Box::new(Keyword::Flying),
                reminder: false,
            })),
            value(&Ability::Keyword(Keyword::Flying)),
        ),
    ] {
        let typed: Vec<Ability> = serde_json::from_value(annotated.clone()).unwrap();
        validate::validate_ability(&typed[0]).unwrap();
        canonical::reduced(&mut annotated).unwrap();
        canonical::reduced(&mut plain).unwrap();
        assert_eq!(annotated, plain);
    }
}

#[test]
fn a_defining_keyword_keeps_its_rules_and_reminder_when_labeled() {
    let ability = Ability::Static {
        kind: StaticKind::CharacteristicDefining {
            keyword: Some(Keyword::WithReminder {
                keyword: Box::new(Keyword::Devoid),
                reminder: true,
            }),
            contributes_to_color_identity: false,
        },
        affected: EffectSubjects::Objects(ObjectSet::One(ObjectRef::Source)),
        changes: vec![Change::Colors {
            mode: ChangeMode::Set,
            colors: vec![],
        }],
    };
    let ability = presented(ability, Some("Test label"), false);
    validate::validate_ability(&ability).unwrap();
    let mut exact = value(&ability);
    let mut semantic = exact.clone();
    canonical::abilities(&mut exact).unwrap();
    canonical::semantic_abilities(&mut semantic).unwrap();
    assert_eq!(
        semantic[0]["args"]["kind"]["CharacteristicDefining"]["keyword"]["kind"],
        "Devoid"
    );
    assert_eq!(
        semantic[0]["args"]["changes"],
        exact[0]["args"]["ability"]["args"]["changes"]
    );
}

#[test]
fn a_labeled_additional_cost_keeps_its_payment_namespace() {
    let cost = Ability::Casting(CastingRule::AdditionalCost {
        cost: Cost {
            parts: vec![CostPart::Sacrifice {
                objects: ObjectSet::battlefield(ObjectFilter::CardType(
                    crate::mtg_types::CardType::Creature,
                )),
                choose: Some(Count::Exactly(Expr::Constant(1))),
            }],
        },
        optional: false,
        repetition: CostRepetition::Once,
    });
    let mut spell = draw();
    if let Ability::Spell { body, .. } = &mut spell {
        if let Instruction::Effect {
            operation: Operation::Draw { amount, .. },
            ..
        } = &mut body.instructions[0]
        {
            *amount = Expr::CountObjects(Box::new(ObjectSet::CostObjects {
                payment: PaymentRef::Casting { ability: 0 },
                component: vec![0],
                view: CostObjectView::Inputs,
                phase: ObjectPhase::Before,
            }));
        }
    }
    let mut card = Card {
        schema_version: SCHEMA_VERSION,
        name: "Fixture Card".into(),
        characteristics: Characteristics {
            supertypes: vec![],
            card_types: vec![crate::mtg_types::CardType::Sorcery],
            subtypes: vec![],
            mana_cost: None,
            color_indicator: vec![],
            power_toughness: None,
            loyalty: None,
            defense: None,
        },
        abilities: vec![cost, spell],
    };
    validate::card(&card).unwrap();
    let cost = card.abilities.remove(0);
    card.abilities
        .insert(0, presented(cost, Some("Test label"), false));
    // Reinsert rather than renumbering either the declaration or its reader.
    assert!(crate::cost_history::casting_cost(&card.abilities[0]).is_some());
    validate::abilities(&card.abilities).unwrap();
    validate::card(&card).unwrap();
}

#[test]
fn erasure_only_visits_typed_metadata_and_preserves_opaque_lookalikes() {
    let mut input = value(&draw());
    let opaque = json!({"WithReminder":{"keyword":"Flying","reminder":true}});
    input[0]["args"]["untyped"] = opaque.clone();
    presentation::erase_abilities(&mut input).unwrap();
    assert_eq!(input[0]["args"]["untyped"], opaque);
}

#[test]
fn erasure_has_a_depth_bound_and_failure_does_not_mutate_the_input() {
    let mut ability = json!({"kind":"Keyword","args":"Flying"});
    for _ in 0..300 {
        ability = json!({"kind":"WithPresentation","args":{"label":"Test","reminder":false,"ability":ability}});
    }
    let mut input = json!([ability]);
    let original = input.clone();
    assert_eq!(
        presentation::erase_abilities(&mut input),
        Err("presentation walk limit".into())
    );
    assert_eq!(input, original);
}
