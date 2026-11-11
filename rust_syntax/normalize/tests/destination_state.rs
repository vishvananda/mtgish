use mtgish_ast::{human, ir::*, migrate, validate};
use serde_json::{json, Value};
fn card(operation: Value) -> Value {
    let mut c=serde_json::to_value(human::read("Card{name:\"Arrival test\",properties:{card_types:[Instant]},abilities:[Spell{do:[]}]}").unwrap()).unwrap();
    c["faces"][0]["abilities"][0]["args"]["body"]["instructions"] =
        json!([{"kind":"Effect","args":{"result":0,"operation":operation}}]);
    c
}
fn moving(destination: Value) -> Value {
    json!({"kind":"Move","args":{"objects":{"kind":"One","args":{"kind":"Source"}},"destination":destination,"qualifiers":[]}})
}
fn checked(operation: Value) -> Result<UnifiedCard, String> {
    migrate::read_json(&card(operation).to_string())
}
fn bf(state: BattlefieldState) -> Value {
    serde_json::to_value(MovementDestination::Battlefield {
        controller: EntryController::Owner,
        arrival: BattlefieldArrival {
            states: vec![state],
            ..Default::default()
        },
    })
    .unwrap()
}
#[test]
fn permanent_arrival_state_cannot_follow_a_card_into_a_hand_or_library() {
    for state in [
        BattlefieldState::FaceDown,
        BattlefieldState::Transformed,
        BattlefieldState::Attacking {
            player: None,
            or_planeswalker: false,
        },
        BattlefieldState::AttachedTo(ObjectRef::Source),
        BattlefieldState::CopyOf {
            objects: ObjectSet::One(ObjectRef::Source),
            exceptions: vec![],
            until: None,
        },
    ] {
        checked(moving(bf(state.clone()))).unwrap();
        for destination in [
            json!({"OwnersHand":{"counters":[],"states":[state]}}),
            json!({"Library":{"position":"Top","order":"DestinationDefault","arrival":{"counters":[],"states":[state]}}}),
        ] {
            assert!(checked(moving(destination)).is_err());
        }
    }
    assert!(checked(moving(json!({"OwnersHand":{"counters":[],"states":[{"When":{"filter":{"kind":"Any"},"entry":{"counters":[],"states":["FaceDown"]}}}]}}))).is_err());
}
#[test]
fn exile_visibility_piles_and_return_terms_do_not_become_permanent_state() {
    for state in [
        ExileState::ExilePile { shuffled: true },
        ExileState::ExiledFor(Duration::EndOfTurn),
    ] {
        checked(moving(
            json!({"Exile":{"visibility":"FaceUp","arrival":{"counters":[],"states":[state]}}}),
        ))
        .unwrap();
        assert!(checked(moving(json!({"Battlefield":{"controller":"Owner","arrival":{"controller":null,"tapped":false,"counters":[],"states":[state]}}}))).is_err());
    }
    checked(moving(
        serde_json::to_value(MovementDestination::Exile {
            visibility: ExileVisibility::FaceDown,
            arrival: ExileArrival::default(),
        })
        .unwrap(),
    ))
    .unwrap();
}
#[test]
fn specialized_actions_use_their_destination_for_arrival_validation() {
    let invalid = json!({"OwnersHand":{"tapped":true,"counters":[],"states":[]}});
    assert!(checked(json!({"kind":"MoveTopCard","args":{"actor":{"kind":"One","args":{"kind":"AbilityController"}},"cards":{"kind":"Any"},"reveal":false,"destination":invalid}})).is_err());
    assert!(checked(json!({"kind":"CounterWith","args":{"object":{"kind":"Source"},"when":null,"destination":{"Exile":{"visibility":"FaceUp","arrival":{"tapped":true,"counters":[],"states":[]}}}}})).is_err());
    let entry = json!({"controller":null,"tapped":false,"counters":[],"states":[{"ExilePile":{"shuffled":false}}]});
    for operation in [
        json!({"kind":"Populate","args":{"actor":{"kind":"One","args":{"kind":"AbilityController"}},"entry":entry}}),
        json!({"kind":"Meld","args":{"objects":{"kind":"One","args":{"kind":"Source"}},"into":"Meld result","entry":entry}}),
        json!({"kind":"CreateTokens","args":{"players":{"kind":"One","args":{"kind":"AbilityController"}},"tokens":[],"entry":entry}}),
    ] {
        assert!(checked(operation).is_err());
    }
}
#[test]
fn specialized_hidden_exile_and_delay_remain_valid() {
    let destination = MovementDestination::Exile {
        visibility: ExileVisibility::FaceDown,
        arrival: ExileArrival::default(),
    };
    for operation in [
        Operation::Counter { objects: ObjectSet::One(ObjectRef::Source), destination_when: None, destination: destination.clone() },
        Operation::MoveTopCard {
            actor: PlayerSet::One(PlayerRef::AbilityController),
            cards: ObjectFilter::Any,
            reveal: false,
            destination,
        },
    ] {
        checked(serde_json::to_value(operation).unwrap()).unwrap();
    }
    let base="Card{name:\"Conjure test\",properties:{card_types:[Instant]},abilities:[Spell{do:[Conjure{actor:You,card:Named(\"Grizzly Bears\"),count:1,zone:Exile,entry:{states:[FaceDown]}}]}]}";
    human::read(base).unwrap();
    assert!(human::read(&base.replace("zone:Exile", "zone:Hand")).is_err());
    assert!(human::read(&base.replace("entry:{states:[FaceDown]}", "spot:Some(Top)")).is_err());
    let delay = include_str!("fixtures/entry_payloads_v179.jsonl")
        .lines()
        .find(|line| line.contains("\"Delay\""))
        .unwrap();
    migrate::read_json(delay).unwrap();
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

#[test]
fn conditional_hidden_exile_is_valid_native_destination_state() {
    let destination = MovementDestination::Exile {
        visibility: ExileVisibility::FaceUp,
        arrival: ExileArrival {
            states: vec![ExileState::When {
                filter: ObjectFilter::Any,
                entry: Box::new(ExileArrival {
                    states: vec![ExileState::FaceDown],
                    ..Default::default()
                }),
            }],
            ..Default::default()
        },
    };
    let c = checked(moving(serde_json::to_value(destination).unwrap())).unwrap();
    let code = human::write(&c).unwrap();
    human::read(&code).unwrap();
}
