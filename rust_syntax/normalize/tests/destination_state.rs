use mtgish_ast::{human, ir::*, validate};

fn validate_action(operation: Operation) -> Result<(), String> {
    let mut card = human::read(
        "Card{name:\"Arrival test\",properties:{card_types:[Instant]},abilities:[Spell{do:[]}]}",
    )
    .unwrap();
    let Ability::Spell { body, .. } = &mut card.faces[0].abilities[0] else {
        unreachable!()
    };
    body.instructions.push(Instruction::Effect {
        result: ResultId(0),
        operation,
    });
    validate::unified(&card)
}
fn objects() -> ObjectSet {
    ObjectSet::One(ObjectRef::Source)
}
fn actor() -> PlayerSet {
    PlayerSet::One(PlayerRef::AbilityController)
}
fn entry(state: EntryState) -> EntrySpec {
    EntrySpec {
        states: vec![state],
        ..Default::default()
    }
}
fn moving(destination: Destination, entry: EntrySpec) -> Operation {
    Operation::Move {
        objects: objects(),
        destination,
        entry,
        face_down: false,
        terms: Default::default(),
    }
}

#[test]
fn permanent_arrival_state_cannot_follow_a_card_into_a_hand_or_library() {
    for state in [
        EntryState::FaceDown,
        EntryState::Transformed,
        EntryState::Attacking {
            player: None,
            or_planeswalker: false,
        },
        EntryState::AttachedTo(ObjectRef::Source),
        EntryState::CopyOf {
            objects: objects(),
            exceptions: vec![],
            until: None,
        },
    ] {
        validate_action(moving(Destination::OwnersBattlefield, entry(state.clone()))).unwrap();
        for destination in [
            Destination::OwnersHand,
            Destination::OwnersGraveyard,
            Destination::OwnersLibraryTop,
            Destination::Exile,
        ] {
            let error = validate_action(moving(destination, entry(state.clone()))).unwrap_err();
            assert!(error.contains("battlefield destination"), "{error}");
        }
    }
    let nested = entry(EntryState::When {
        filter: ObjectFilter::Any,
        entry: Box::new(entry(EntryState::FaceDown)),
    });
    assert!(validate_action(moving(Destination::OwnersHand, nested)).is_err());
}

#[test]
fn exile_visibility_piles_and_return_terms_do_not_become_permanent_state() {
    for state in [
        EntryState::ExilePile { shuffled: true },
        EntryState::ExiledFor(Duration::EndOfTurn),
    ] {
        validate_action(moving(Destination::Exile, entry(state.clone()))).unwrap();
        assert!(
            validate_action(moving(Destination::OwnersBattlefield, entry(state)))
                .unwrap_err()
                .contains("exile destination")
        );
    }
    let mut operation = moving(Destination::Exile, EntrySpec::default());
    let Operation::Move { face_down, .. } = &mut operation else {
        unreachable!()
    };
    *face_down = true;
    validate_action(operation).unwrap();
}

#[test]
fn specialized_actions_use_their_destination_for_arrival_validation() {
    let tapped = EntrySpec {
        tapped: true,
        ..Default::default()
    };
    assert!(validate_action(Operation::MoveTopCard {
        actor: actor(),
        cards: ObjectFilter::Any,
        reveal: false,
        destination: Destination::OwnersHand,
        entry: tapped.clone()
    })
    .is_err());
    assert!(validate_action(Operation::CounterWith {
        object: ObjectRef::Source,
        when: None,
        destination: Destination::Exile,
        entry: tapped
    })
    .is_err());
    assert!(validate_action(Operation::Populate {
        actor: actor(),
        entry: entry(EntryState::ExilePile { shuffled: false })
    })
    .is_err());
    assert!(validate_action(Operation::Meld {
        objects: objects(),
        into: "Meld result".into(),
        entry: entry(EntryState::ExilePile { shuffled: false })
    })
    .is_err());
    assert!(validate_action(Operation::CreateTokens {
        players: actor(),
        tokens: vec![TokenSpec {
            amount: Expr::Constant(1),
            words: TokenWords::CreatingEffect,
            recipe: TokenRecipe::Named("Clue Token".into())
        }],
        entry: entry(EntryState::ExilePile { shuffled: false })
    })
    .is_err());
}

#[test]
fn specialized_hidden_exile_and_delay_remain_valid() {
    validate_action(Operation::CounterWith {
        object: ObjectRef::Source,
        when: None,
        destination: Destination::Exile,
        entry: entry(EntryState::FaceDown),
    })
    .unwrap();
    validate_action(Operation::MoveTopCard {
        actor: actor(),
        cards: ObjectFilter::Any,
        reveal: false,
        destination: Destination::Exile,
        entry: entry(EntryState::FaceDown),
    })
    .unwrap();
    let base = "Card{name:\"Conjure test\",properties:{card_types:[Instant]},abilities:[Spell{do:[Conjure{actor:You,card:Named(\"Grizzly Bears\"),count:1,zone:Exile,entry:{states:[FaceDown]}}]}]}";
    human::read(base).unwrap();
    assert!(human::read(&base.replace("zone:Exile", "zone:Hand")).is_err());
    assert!(human::read(&base.replace("entry:{states:[FaceDown]}", "spot:Some(Top)")).is_err());
    let delay = include_str!("fixtures/entry_payloads_v179.jsonl")
        .lines()
        .find(|line| line.contains("\"Delay\""))
        .unwrap();
    mtgish_ast::migrate::read_json(delay).unwrap();
}

#[test]
#[ignore = "explicit normalized corpus"]
fn all_current_arrival_states_remain_valid_for_their_destinations() {
    use std::io::BufRead;
    mtgish_ast::corpus::on_large_stack(|| {
        let input = std::fs::File::open(std::env::var("AST_DESTINATION_CORPUS").unwrap()).unwrap();
        let mut count = 0;
        for line in std::io::BufReader::new(input).lines() {
            let card: UnifiedCard = serde_json::from_str(&line.unwrap()).unwrap();
            validate::unified(&card).unwrap_or_else(|e| {
                panic!(
                    "{:?}: {e}",
                    card.faces.iter().map(|f| &f.name).collect::<Vec<_>>()
                )
            });
            count += 1;
        }
        assert_eq!(count, 33529);
    });
}
