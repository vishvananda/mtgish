#[path = "support/legacy_entries.rs"]
mod legacy_entries;
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
fn pool() -> ObjectSet {
    ObjectSet::battlefield(ObjectFilter::Any)
}
fn zone(
    cards: ObjectSet,
    count: Option<Count>,
    random: bool,
    destination: Destination,
    entry: Vec<Operand>,
    group: Option<Operand>,
    face_down: Option<FaceDownAs>,
) -> Operation {
    Operation::DraftChangeZone {
        actor: you(),
        cards,
        count,
        random,
        destination,
        entry,
        group: group.map(Box::new),
        face_down,
    }
}
fn prepared(operation: Operation) -> Operation {
    let mut p = Program {
        instructions: vec![Instruction::Effect {
            result: ResultId(4),
            operation,
        }],
    };
    movement::normalize_program(&mut p);
    let Instruction::Effect { result, operation } = p.instructions.remove(0) else {
        unreachable!()
    };
    assert_eq!(result.0, 4);
    operation
}
fn flag(family: &str, form: &str) -> Operand {
    Operand::Vocabulary {
        family: family.into(),
        form: form.into(),
    }
}
fn card(body: Program) -> Card {
    let source: OracleCard=ron::from_str(r#"Card(Name: "Transfer Control", Typeline: (Supertypes: [], Cardtypes: [Sorcery], Subtypes: []), Rules: [SpellActions(ActionList([DrawACard]))])"#).unwrap();
    let mut card=mtgish_ast::lower::card(&source).unwrap();
    card.abilities=vec![Ability::Spell {announcement:Announcement::empty(),body}];
    card
}
#[cfg(feature = "cantrip")]
#[test]
fn graveyard_moves_use_the_zone_verb_only_for_proven_nonpermanents() {
    let exile = || ObjectSet::Zone {
        scope: ZoneScope::Shared(EventZone::Exile),
        kind: ObjectKind::Card,
        filter: ObjectFilter::Any,
    };
    let emit = |objects| mtgish_ast::cantrip::module(&card(Program {
        instructions: vec![Instruction::Effect {
            result: ResultId(0),
            operation: Operation::Move {
                objects,
                destination: Destination::OwnersGraveyard,
                entry: EntrySpec::default(),
                face_down: false,
                terms: MovementTerms::default(),
            },
        }, Instruction::Effect {
            result: ResultId(1),
            operation: Operation::GainLife {
                players: you(),
                amount: Expr::CountObjects(Box::new(ObjectSet::Transition {
                    result: ResultId(0), phase: ObjectPhase::After,
                })),
            },
        }],
    })).unwrap();
    for objects in [
        exile(),
        ObjectSet::Matching { objects: Box::new(exile()), filter: ObjectFilter::Any },
        ObjectSet::InZone { objects: Box::new(ObjectSet::AnyZone(ObjectFilter::Any)), zone: EventZone::Exile },
        ObjectSet::Union(vec![exile(), ObjectSet::Zone {
            scope: ZoneScope::Players { zone: EventZone::Hand, players: you() },
            kind: ObjectKind::Card,
            filter: ObjectFilter::Any,
        }]),
    ] {
        let generated = emit(objects);
        assert!(generated.contains("g.put_into_graveyard("), "{generated}");
        assert!(!generated.contains("g.move_together("), "{generated}");
        assert!(generated.contains("ZoneKind::Graveyard"), "replacement destinations must not count: {generated}");
    }
    for objects in [
        pool(),
        ObjectSet::AnyZone(ObjectFilter::Any),
        ObjectSet::Union(vec![exile(), pool()]),
    ] {
        let generated = emit(objects);
        assert!(generated.contains("g.move_together("), "{generated}");
        assert!(!generated.contains("g.put_into_graveyard("), "{generated}");
    }
}
#[test]
fn draft_forms_decode_old_captures_but_cannot_reach_validated_records() {
    for line in include_str!("fixtures/movement_transfer_before.ronl").lines() {
        let mut old:UnifiedCard=legacy_entries::ron(line);
        old.schema_version=SCHEMA_VERSION;
        for face in &mut old.faces {face.schema_version=SCHEMA_VERSION;}
        assert!(old.faces.iter().any(|face|validate::card(face).is_err()));
        movement::normalize_unified(&mut old);
        for face in &old.faces {validate::card(face).unwrap_or_else(|e|panic!("{}: {e}",face.name));}
    }
    let raw=card(Program {instructions:vec![Instruction::Effect {result:ResultId(0),operation:Operation::Tap {objects:ObjectSet::DraftChosen {chooser:you(),objects:Box::new(pool()),count:Count::Exactly(Expr::Constant(1))},tapped:true}}]});
    assert!(validate::card(&raw).unwrap_err().contains("draft selector"));
    validate::card(&movement::normalize_card(raw)).unwrap();
}
#[cfg(feature="cantrip")]
#[test]
fn an_optimized_search_cannot_discard_order_or_group_terms() {
    use mtgish_ast::mtg_types::{CardType,SubType,SuperType};
    let make=|terms:MovementTerms|card(Program {instructions:vec![
        Instruction::Effect {result:ResultId(0),operation:Operation::Search {player:PlayerRef::AbilityController,library:PlayerRef::AbilityController,top:None,choices:vec![SearchChoice {filter:ObjectFilter::All(vec![ObjectFilter::Supertype(SuperType::Basic),ObjectFilter::Subtype(SubType::Plains)]),count:Count::UpTo(Expr::Constant(1)),constraints:vec![]}],also:vec![],either:false,outside:false}},
        Instruction::Effect {result:ResultId(1),operation:Operation::Reveal {objects:ObjectSet::Selected(ResultId(0))}},
        Instruction::Effect {result:ResultId(2),operation:Operation::Move {objects:ObjectSet::Selected(ResultId(0)),destination:Destination::OwnersHand,entry:EntrySpec::default(),face_down:false,terms}},
        Instruction::Effect {result:ResultId(3),operation:Operation::Shuffle {players:you(),excluding:None}},
    ]});
    let plain=make(MovementTerms::default());
    validate::card(&plain).unwrap();
    assert!(mtgish_ast::cantrip::module(&plain).unwrap().contains("library::find_to_hand"));
    for terms in [MovementTerms {qualifiers:vec![SelectionConstraint::DifferentNames],..Default::default()},MovementTerms {order:MovementOrder::Random,..Default::default()}] {
        let rich=make(terms);
        validate::card(&rich).unwrap();
        assert!(mtgish_ast::cantrip::module(&rich).is_err(),"a recipe discarded terms");
    }
    // Tokens remain eligible in a live type domain; a card-only projection
    // would drop artifact/creature types held only by tokens.
    let domain=CardTypeDomain::AmongObjects(ObjectSet::battlefield(ObjectFilter::CardType(CardType::Creature)));
    let action=Operation::WithSelections {steps:vec![SelectionStep {binding:SelectionId(0),chooser:SelectionChooser::Players(you()),objects:pool(),count:Count::AnyNumber,constraints:vec![SelectionConstraint::OneForEachCardType(domain)]}],action:Box::new(Operation::Move {objects:ObjectSet::Prepared(SelectionId(0)),destination:Destination::OwnersHand,entry:EntrySpec::default(),face_down:false,terms:MovementTerms::default()})};
    let generated=mtgish_ast::cantrip::module(&card(Program {instructions:vec![Instruction::Effect {result:ResultId(0),operation:action}]})).unwrap();
    assert!(generated.contains("q.state().battlefield()") && generated.contains("q.chars(o).is(t)"));
}
#[test]
fn every_captured_zone_change_normalizes_without_a_legacy_operation_and_roundtrips() {
    struct Count(usize);
    impl Visitor for Count {
        fn operation(&mut self, operation: &Operation) -> bool {
            if matches!(operation, Operation::DraftChangeZone { .. }) {
                self.0 += 1;
            }
            true
        }
    }
    let mut count = Count(0);
    let mut records = 0;
    for line in include_str!("fixtures/movement_transfer.ronl").lines() {
        let source: OracleCard = ron::from_str(line).unwrap();
        let mut lowered = mtgish_ast::unified::lower(&source);
        assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
        lowered.card.walk(&mut count);
        for face in &lowered.card.faces {
            validate::card(face).unwrap_or_else(|e| panic!("{}: {e}", face.name));
        }
        let before = serde_json::to_value(&lowered.card).unwrap();
        movement::normalize_unified(&mut lowered.card);
        assert_eq!(before, serde_json::to_value(&lowered.card).unwrap());
        let bytes = bincode::encode_to_vec(&lowered.card, bincode::config::standard()).unwrap();
        let (round, used): (UnifiedCard, usize) =
            bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
        assert_eq!(used, bytes.len());
        assert_eq!(before, serde_json::to_value(round).unwrap());
        records += 1;
    }
    assert_eq!(records, 121);
    assert_eq!(count.0, 0);
}
#[test]
fn random_subset_and_random_placement_remain_different_operations() {
    let selected = prepared(zone(
        pool(),
        Some(Count::Exactly(Expr::Constant(2))),
        true,
        Destination::OwnersLibraryTop,
        vec![],
        None,
        None,
    ));
    let Operation::WithSelections { steps, action } = selected else {
        panic!("{selected:?}")
    };
    assert!(matches!(steps[0].chooser, SelectionChooser::Chance));
    assert!(matches!(steps[0].count, Count::Exactly(Expr::Constant(2))));
    let Operation::Move { terms, .. } = action.as_ref() else {
        panic!("{action:?}")
    };
    assert!(terms.is_default());
    let ordered = prepared(zone(
        pool(),
        None,
        true,
        Destination::OwnersLibraryTop,
        vec![],
        None,
        None,
    ));
    let Operation::Move { terms, .. } = ordered else {
        panic!("{ordered:?}")
    };
    assert!(matches!(terms.order, MovementOrder::Random));
}
#[test]
fn a_stated_placement_chooser_is_independent_of_selecting_the_objects() {
    let operation = prepared(zone(
        pool(),
        None,
        false,
        Destination::OwnersLibraryTopOrBottom,
        vec![],
        None,
        None,
    ));
    let Operation::Move { terms, .. } = operation else {
        panic!("{operation:?}")
    };
    assert!(matches!(
        terms.placement,
        MovementPlacement::By(PlayerSet::One(PlayerRef::AbilityController))
    ));
    let operation = prepared(zone(
        pool(),
        None,
        false,
        Destination::OwnersHand,
        vec![],
        None,
        None,
    ));
    let Operation::Move { terms, .. } = operation else {
        panic!("{operation:?}")
    };
    assert!(terms.is_default());
}
#[test]
fn exile_visibility_piles_counters_and_battlefield_shape_are_not_flattened() {
    let operation = prepared(zone(
        pool(),
        None,
        false,
        Destination::Exile,
        vec![flag("ExileFlag", "ExilesIntoShuffledFaceDownPile")],
        None,
        None,
    ));
    let Operation::Move {
        entry,
        face_down: true,
        ..
    } = operation
    else {
        panic!("{operation:?}")
    };
    assert!(matches!(
        entry.states.as_slice(),
        [EntryState::ExilePile { shuffled: true }]
    ));
    let operation = prepared(zone(
        pool(),
        None,
        false,
        Destination::Exile,
        vec![flag("CounterType", "SilverCounter")],
        None,
        None,
    ));
    let Operation::Move {
        entry,
        face_down: false,
        ..
    } = operation
    else {
        panic!("{operation:?}")
    };
    assert!(matches!(
        entry.counters.as_slice(),
        [(
            mtgish_ast::counter::CounterKind::Named(
                mtgish_ast::counter::CounterName::SilverCounter
            ),
            Expr::Constant(1)
        )]
    ));
    let shape = FaceDownAs {
        power_toughness: Box::new(Operand::Text("a stated nondefault shape".into())),
        creature_type: None,
        artifact: true,
    };
    let operation = prepared(zone(
        pool(),
        None,
        false,
        Destination::OwnersBattlefield,
        vec![],
        None,
        Some(shape),
    ));
    let Operation::Move {
        entry,
        face_down: false,
        ..
    } = operation
    else {
        panic!("{operation:?}")
    };
    assert!(matches!(
        entry.states.as_slice(),
        [EntryState::FaceDownAs(FaceDownAs { artifact: true, .. })]
    ));
}
#[test]
fn forced_name_classes_type_domains_and_unselected_group_qualifications_remain_distinct() {
    for (form, expected) in [
        ("OneAndAllSameName", 0),
        ("OnePerCardType", 1),
        ("OnePerPermanentType", 2),
    ] {
        let operation = prepared(zone(
            pool(),
            Some(Count::AnyNumber),
            false,
            Destination::OwnersHand,
            vec![],
            Some(flag("GroupFilter", form)),
            None,
        ));
        let Operation::WithSelections { steps, .. } = operation else {
            panic!("{operation:?}")
        };
        match expected {
            0 => assert!(matches!(
                steps[0].constraints.as_slice(),
                [SelectionConstraint::AllWithChosenName]
            )),
            1 => assert!(matches!(
                steps[0].constraints.as_slice(),
                [SelectionConstraint::OneForEachCardType(CardTypeDomain::All)]
            )),
            _ => assert!(matches!(
                steps[0].constraints.as_slice(),
                [SelectionConstraint::OneForEachCardType(
                    CardTypeDomain::Permanent
                )]
            )),
        }
    }
    let operation = prepared(zone(
        pool(),
        None,
        false,
        Destination::OwnersHand,
        vec![],
        Some(flag("GroupFilter", "DifferentNames")),
        None,
    ));
    let Operation::Move { terms, .. } = operation else {
        panic!("{operation:?}")
    };
    assert!(matches!(
        terms.qualifiers.as_slice(),
        [SelectionConstraint::DifferentNames]
    ));
}
