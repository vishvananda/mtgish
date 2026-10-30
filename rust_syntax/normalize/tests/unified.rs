use mtgish_ast::{
    ir::{Ability, Named, Operand},
    lower, mtg_types as source, unified, validate,
};

fn parse(card: &str) -> source::OracleCard {
    ron::from_str(card).unwrap()
}

fn lines(name: &str) -> impl Iterator<Item = source::OracleCard> {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name),
    )
    .unwrap()
    .lines()
    .filter(|line| !line.trim().is_empty())
    .map(parse)
    .collect::<Vec<_>>()
    .into_iter()
}

/// Strict lowering of a large fixture needs more than a test thread's 2 MiB.
fn on_large_stack(test: impl FnOnce() + Send) {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(256 << 20)
            .spawn_scoped(scope, test)
            .unwrap()
            .join()
            .unwrap()
    })
}

#[test]
fn every_fixture_record_lowers_validates_and_round_trips() {
    let mut records = 0;
    for name in ["pipeline_generated.ronl", "cast_history.ronl"] {
        for card in lines(name) {
            let lowered = unified::lower(&card);
            validate::unified(&lowered.card).unwrap();
            unified::round_trip(&lowered.card).unwrap();
            assert!(!lowered.card.faces.is_empty() || !lowered.card.data.is_empty());
            records += 1;
        }
    }
    assert!(records > 0);
}

#[test]
fn strict_validation_rejects_named_forms() {
    on_large_stack(|| {
        let mut card = lines("pipeline_generated.ronl")
            .find_map(|candidate| lower::card(&candidate).ok())
            .expect("a complete fixture");
        validate::card(&card).unwrap();
        card.abilities.push(Ability::Named(Named {
            source_type: "Rule".into(),
            form: "Test".into(),
            operands: vec![],
        }));
        assert!(validate::card(&card).unwrap_err().contains("Rule::Test"));
    });
}

#[test]
fn complete_cards_contain_no_named_forms() {
    on_large_stack(|| {
        for card in lines("pipeline_generated.ronl") {
            if let Ok(strict) = lower::card(&card) {
                let lowered = unified::lower(&card);
                assert!(lowered.diagnostics.is_empty());
                assert!(unified::named_forms(&lowered.card).is_empty());
                assert_eq!(
                    serde_json::to_value(&lowered.card.faces[0]).unwrap(),
                    serde_json::to_value(&strict).unwrap(),
                );
            }
        }
    });
}

#[test]
fn a_named_operand_never_holds_an_unbound_reference() {
    let reference = serde_json::json!({"enum": "Permanent", "variant": "Ref_TargetPermanent", "args": null});
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
    assert!(lowered.faces.iter().all(|f| !f.characteristics.card_types.is_empty()));

    let side = |n: &str| {
        format!(r#"(Name:"{n}",Typeline:(Supertypes:[],Cardtypes:[Creature],Subtypes:[]),Rules:[],CardPT:(Power:1,Toughness:1))"#)
    };
    let flip = parse(&format!(
        "Flip(ManaCost:[ManaCostGeneric(2)],Unflipped:{},Flipped:{})",
        side("Up"),
        side("Down")
    ));
    let lowered = unified::lower(&flip).card;
    assert_eq!(lowered.faces.len(), 2);
    assert!(lowered.faces.iter().all(|f| f.characteristics.mana_cost.is_some()));
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
