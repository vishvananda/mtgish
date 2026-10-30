use mtgish_ast::{
    corpus,
    ir::{Ability, EventSpec, Named, Operand},
    lower, mtg_types as source, unified, validate,
};

fn parse(card: &str) -> source::OracleCard {
    ron::from_str(card).unwrap()
}

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

#[test]
fn every_fixture_record_lowers_validates_and_round_trips() {
    for name in ["pipeline_generated.ronl", "cast_history.ronl"] {
        let (mut records, mut complete) = (0, 0);
        corpus::map_cards(
            &fixture(name),
            |_, card| {
                let lowered = unified::lower(&card);
                validate::unified(&lowered.card).unwrap();
                unified::round_trip(&lowered.card).unwrap();
                assert!(!lowered.card.faces.is_empty() || !lowered.card.data.is_empty());
                let strict = lower::card(&card).ok();
                if let Some(strict) = &strict {
                    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
                    assert!(unified::named_forms(&lowered.card).is_empty());
                    assert_eq!(
                        serde_json::to_value(&lowered.card.faces[0]).unwrap(),
                        serde_json::to_value(strict).unwrap(),
                    );
                }
                Ok(strict.is_some())
            },
            |_, is_complete| {
                records += 1;
                complete += usize::from(is_complete);
                Ok(())
            },
        )
        .unwrap();
        assert!(records > 0);
        assert!(complete > 0);
    }
}

#[test]
fn strict_validation_rejects_named_forms() {
    let card = parse(
        r#"Card(Name:"Test",Typeline:(Supertypes:[],Cardtypes:[Creature],Subtypes:[]),Rules:[],CardPT:(Power:1,Toughness:1))"#,
    );
    let mut card = lower::card(&card).unwrap();
    validate::card(&card).unwrap();
    card.abilities.push(Ability::Named(Named {
        source_type: "Rule".into(),
        form: "Test".into(),
        operands: vec![],
    }));
    assert!(validate::card(&card).unwrap_err().contains("Rule::Test"));
}

#[test]
fn a_named_operand_never_holds_an_unbound_reference() {
    let reference =
        serde_json::json!({"enum": "Permanent", "variant": "Ref_TargetPermanent", "args": null});
    assert!(matches!(lower::operand(&reference), Operand::Form(_)));
}

#[test]
fn layout_faces_are_ordinary_cards() {
    let front = r#"(Name:"Front",Typeline:(Supertypes:[],Cardtypes:[Creature],Subtypes:[]),ManaCost:[ManaCostGeneric(1)],Rules:[],CardPT:(Power:1,Toughness:1))"#;
    let back = r#"(Name:"Back",Typeline:(Supertypes:[],Cardtypes:[Creature],Subtypes:[]),Rules:[],CardPT:(Power:2,Toughness:2))"#;
    let card = parse(&format!("Transforming(FrontFace:{front},BackFace:{back})"));
    let lowered = unified::lower(&card);
    assert_eq!(lowered.card.layout, "Transforming");
    let mut names: Vec<_> = lowered.card.faces.iter().map(|f| f.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Back", "Front"]);
    assert!(unified::named_forms(&lowered.card).is_empty());
}

#[test]
fn rooms_share_their_typeline_and_flips_share_their_mana_cost() {
    let door = |n: &str| format!(r#"(Name:"{n}",Rules:[],ManaCost:[ManaCostGeneric(1)])"#);
    let typeline = "(Supertypes:[],Cardtypes:[Enchantment],Subtypes:[])";
    let room = parse(&format!(
        "Room(Typeline:{typeline},LeftDoor:{},RightDoor:{})",
        door("Left"),
        door("Right")
    ));
    let lowered = unified::lower(&room).card;
    assert_eq!(lowered.faces.len(), 2);
    assert!(lowered
        .faces
        .iter()
        .all(|f| !f.characteristics.card_types.is_empty()));

    let side = |n: &str| {
        format!(
            r#"(Name:"{n}",Typeline:(Supertypes:[],Cardtypes:[Creature],Subtypes:[]),Rules:[],CardPT:(Power:1,Toughness:1))"#
        )
    };
    let flip = parse(&format!(
        "Flip(ManaCost:[ManaCostGeneric(2)],Unflipped:{},Flipped:{})",
        side("Up"),
        side("Down")
    ));
    let lowered = unified::lower(&flip).card;
    assert_eq!(lowered.faces.len(), 2);
    assert!(lowered
        .faces
        .iter()
        .all(|f| f.characteristics.mana_cost.is_some()));
}

#[test]
fn layout_fields_outside_a_face_are_kept_as_data() {
    let piece = parse(
        r#"MeldPiece(Name:"Piece",Typeline:(Supertypes:[],Cardtypes:[Creature],Subtypes:[]),Rules:[],CardPT:(Power:1,Toughness:1),MeldsInto:"Whole")"#,
    );
    let lowered = unified::lower(&piece).card;
    assert_eq!(lowered.faces.len(), 1);
    assert_eq!(lowered.data[0].name, "MeldsInto");
    assert!(matches!(&lowered.data[0].value, Operand::Text(t) if t == "Whole"));
}

#[test]
fn a_primitive_binds_its_operands_in_scope_and_is_complete() {
    let card = parse(
        r#"Card(Name:"Test",Typeline:(Supertypes:[],Cardtypes:[Sorcery],Subtypes:[]),Rules:[SpellActions(ActionList([Investigate]))])"#,
    );
    let strict = lower::card(&card).unwrap();
    validate::card(&strict).unwrap();
    let forms = unified::forms(&strict);
    assert!(forms.named.is_empty());
    assert_eq!(forms.primitives["Action::Investigate"], 1);
}

#[test]
fn every_primitive_row_is_a_real_source_form() {
    let schema = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/mtg_types.rs"),
    )
    .unwrap();
    let syntax = syn::parse_file(&schema).unwrap();
    let rows = &mtgish_ast::primitive_spec::registry().rules;
    assert!(!rows.is_empty());
    for row in rows {
        assert!(mtgish_ast::primitive_spec::lookup(&row.source).is_some());
        let (ty, form) = row.source.split_once("::").unwrap();
        assert!(
            syntax.items.iter().any(|item| matches!(item,
                syn::Item::Enum(e) if e.ident == ty
                    && e.variants.iter().any(|v| v.ident == form))),
            "{} is not a source form",
            row.source
        );
        assert!(!row.text.trim().is_empty());
    }
}

#[test]
fn trigger_primitives_are_counted_for_the_audit() {
    let card = parse(
        r#"Card(Name:"Test",Typeline:(Supertypes:[],Cardtypes:[Enchantment],Subtypes:[]),Rules:[TriggerA(WhenChaosEnsues,ActionList([DrawACard]))])"#,
    );
    let strict = lower::card(&card).unwrap();
    assert_eq!(
        unified::forms(&strict).primitives["Trigger::WhenChaosEnsues"],
        1
    );
}

/// Occurrence history and damage have structured semantics that a name would hide.
/// See `ambiguous_legacy_roles_and_first_occurrence_history_remain_explicit_gaps`.
#[test]
fn primitive_rows_exclude_history_and_damage_events() {
    for row in &mtgish_ast::primitive_spec::registry().rules {
        if row.source.starts_with("Trigger::") {
            for word in ["Damage", "FirstTime", "FirstUpkeep"] {
                assert!(!row.source.contains(word), "{}", row.source);
            }
        }
    }
}

#[test]
fn saga_chapters_are_one_trigger_per_number() {
    let source = parse(
        r#"Card(Name: "Test Saga", Typeline: (Supertypes: [], Cardtypes: [Enchantment], Subtypes: [Saga]), ManaCost: [], Rules: [SagaChapters([SagaChapter([1, 2], ActionList([DrawACard])), SagaChapter([3], ActionList([DrawACard]))])])"#,
    );
    let card = lower::card(&source).unwrap();
    validate::card(&card).unwrap();
    let [Ability::Saga { chapters }] = &card.abilities[..] else {
        panic!("{:?}", card.abilities);
    };
    let numbers: Vec<u32> = chapters
        .iter()
        .map(|chapter| match chapter {
            Ability::Triggered {
                event: EventSpec::SagaChapter(number),
                ..
            } => *number,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(numbers, [1, 2, 3]);
}

#[test]
fn primitives_in_every_family_bind_their_operands() {
    let lowered = |rule: &str| {
        let source = parse(&format!(
            r#"Card(Name: "Test", Typeline: (Supertypes: [], Cardtypes: [Creature], Subtypes: []), ManaCost: [], Rules: [{rule}])"#
        ));
        lower::card(&source).map(|card| {
            validate::card(&card).unwrap();
            serde_json::to_string(&card).unwrap()
        })
    };
    for (rule, name) in [
        (
            "Activated(PayEnergy(Integer(1)), ActionList([DrawACard]))",
            "Cost::PayEnergy",
        ),
        (
            "TriggerA(WhenAPlayerSetsASchemeInMotion(SinglePlayer(You), SingleScheme(ThisScheme)), ActionList([DrawACard]))",
            "Trigger::WhenAPlayerSetsASchemeInMotion",
        ),
    ] {
        let json = lowered(rule).unwrap();
        assert!(json.contains(name), "{json}");
    }
    // A term must still bind what it refers to, unlike a named form.
    assert!(lowered(
        "TriggerA(WhenAPermanentBecomesTheTargetOfASpellOrAbility(SinglePermanent(ThisPermanent), TargetsPermanent(Ref_TargetPermanent)), ActionList([DrawACard]))"
    )
    .is_err());
}

#[test]
fn player_predicates_in_a_set_bind_the_tested_player() {
    let source = parse(
        r#"Card(Name: "Test", Typeline: (Supertypes: [], Cardtypes: [Sorcery], Subtypes: []), ManaCost: [], Rules: [SpellActions(ActionList([EachPlayerAction(And([Opponent, IsTheMonarch]), DrawACard)]))])"#,
    );
    let card = lower::card(&source).unwrap();
    validate::card(&card).unwrap();
    let json = serde_json::to_string(&card).unwrap();
    assert!(json.contains("Intersection"), "{json}");
    assert!(json.contains("Players::IsTheMonarch"), "{json}");
}

#[test]
fn history_counts_ordinary_events_and_dead_references_read_the_event_subject() {
    let lowered = |rule: &str| {
        let source = parse(&format!(
            r#"Card(Name: "Test", Typeline: (Supertypes: [], Cardtypes: [Creature], Subtypes: []), ManaCost: [], Rules: [{rule}])"#
        ));
        let card = lower::card(&source).unwrap();
        validate::card(&card).unwrap();
        serde_json::to_string(&card).unwrap()
    };
    let morbid = lowered(
        "SpellActions(ActionList([If(ACreatureOrPlaneswalkerDiedThisTurn(IsCardtype(Creature)), [DrawACard])]))",
    );
    assert!(morbid.contains("EventHistory"), "{morbid}");
    assert!(morbid.contains(r#""window":"ThisTurn""#), "{morbid}");
    assert!(morbid.contains(r#""measure":"Occurrences""#), "{morbid}");
    let blood = lowered(
        "TriggerA(WhenAPermanentDies(SinglePermanent(ThisPermanent)), ActionList([DeadPermanentDealsDamage(Integer(1), Player(You))]))",
    );
    assert!(
        blood.contains(r#""source":{"kind":"EventSubject"}"#),
        "{blood}"
    );
    let bad = parse(
        r#"Card(Name: "Test", Typeline: (Supertypes: [], Cardtypes: [Instant], Subtypes: []), ManaCost: [], Rules: [SpellActions(ActionList([DeadPermanentDealsDamage(Integer(1), Player(You))]))])"#,
    );
    assert!(lower::card(&bad).is_err());
}

#[test]
fn a_created_trigger_keeps_its_duration_and_announcement() {
    let source = parse(
        r#"Card(Name: "Test", Typeline: (Supertypes: [], Cardtypes: [Instant], Subtypes: []), ManaCost: [], Rules: [SpellActions(ActionList([CreateTriggerUntil(WhenAPermanentEntersTheBattlefield(ControlledByAPlayer(SinglePlayer(You))), ActionList([DrawACard]), UntilEndOfTurn)]))])"#,
    );
    let card = lower::card(&source).unwrap();
    validate::card(&card).unwrap();
    let json = serde_json::to_string(&card).unwrap();
    assert!(json.contains("CreateTrigger"), "{json}");
    assert!(json.contains(r#""duration":"EndOfTurn""#), "{json}");
}

#[test]
fn a_reflexive_form_is_its_base_action_then_a_trigger_with_its_own_target() {
    let source = parse(
        r#"Card(Name: "Test", Typeline: (Supertypes: [], Cardtypes: [Instant], Subtypes: []), ManaCost: [], Rules: [SpellActions(ActionList([MayAction(Reflexive_Sacrifice_WhenYouDo([APermanent(IsCardtype(Creature))], Targeted([AnyTarget], ActionList([DealDamage(Permanent(ThisPermanent), PowerOfTheSacrificedCreature, Ref_AnyTarget)]))))]))])"#,
    );
    let card = lower::card(&source).unwrap();
    validate::card(&card).unwrap();
    let json = serde_json::to_string(&card).unwrap();
    assert!(json.contains("ReflexiveTrigger"), "{json}");
    assert!(json.contains(r#""kind":"Sacrifice""#), "{json}");
    assert!(json.contains(r#""kind":"Transition""#), "{json}");

    let source = parse(
        r#"Card(Name: "Test", Typeline: (Supertypes: [], Cardtypes: [Instant], Subtypes: []), ManaCost: [], Rules: [SpellActions(ActionList([MayAction(Reflexive_PayMana_WhenYouDo([ManaCostGeneric(2)], ActionList([DrawACard])))]))])"#,
    );
    let card = lower::card(&source).unwrap();
    validate::card(&card).unwrap();
    assert!(serde_json::to_string(&card).unwrap().contains("PayCost"));
}

#[test]
fn exile_until_returns_in_one_effect_and_durations_keep_their_object() {
    let source = parse(
        r#"Card(Name: "Test", Typeline: (Supertypes: [], Cardtypes: [Instant], Subtypes: []), ManaCost: [], Rules: [SpellActions(Targeted([TargetPermanent(AnyPermanent)], ActionList([Exile([Permanent(Ref_TargetPermanent)], [Limited(UntilPermanentLeavesBattlefield(ThisPermanent))])])))])"#,
    );
    let card = lower::card(&source).unwrap();
    validate::card(&card).unwrap();
    let json = serde_json::to_string(&card).unwrap();
    assert!(json.contains("ReturnAfter"), "{json}");
    assert!(json.contains("WhileOnBattlefield"), "{json}");
}
