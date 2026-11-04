//! The last five cards with a primitive take typed shapes, read from their real records.
use mtgish_ast::{ir::*, lower, mtg_types::OracleCard, unified, validate};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::OnceLock;

fn corpus() -> &'static str {
    static TEXT: OnceLock<String> = OnceLock::new();
    TEXT.get_or_init(|| {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/mtgish.lines.ron");
        std::fs::read_to_string(path).unwrap()
    })
}

/// The validated card, with no primitive or named form left in it.
fn typed(name: &str) -> Card {
    let wanted = format!("Name: {name:?}");
    let line = corpus()
        .lines()
        .find(|line| line.contains(&wanted))
        .unwrap();
    let source: OracleCard = ron::from_str(line).unwrap();
    let card = lower::card(&source).unwrap();
    validate::card(&card).unwrap();
    let forms = unified::forms(&card);
    assert!(forms.named.is_empty(), "{name}: {:?}", forms.named);
    assert!(
        forms.primitives.is_empty(),
        "{name}: {:?}",
        forms.primitives
    );
    card
}

fn json(card: &Card) -> Value {
    serde_json::to_value(card).unwrap()
}

/// The `kind` of every effect in a program, in order, without looking into nested programs.
fn kinds(program: &Program) -> Vec<String> {
    program
        .instructions
        .iter()
        .map(|instruction| match instruction {
            Instruction::Effect { operation, .. } => serde_json::to_value(operation).unwrap()
                ["kind"]
                .as_str()
                .unwrap()
                .to_string(),
            other => serde_json::to_value(other).unwrap()["kind"]
                .as_str()
                .unwrap()
                .to_string(),
        })
        .collect()
}

#[test]
fn market_gnome_triggers_on_exile_while_its_controller_activates_a_craft_ability() {
    let card = typed("Market Gnome");
    let event = card
        .abilities
        .iter()
        .find_map(|ability| match ability {
            Ability::Triggered {
                event: event @ EventSpec::WhileActivating { .. },
                ..
            } => Some(event),
            _ => None,
        })
        .expect("a qualified trigger");
    let EventSpec::WhileActivating {
        event,
        players,
        abilities,
    } = event
    else {
        unreachable!()
    };
    assert!(matches!(
        **event,
        EventSpec::ZoneChange {
            from: Some(EventZone::Battlefield),
            to: EventZone::Exile,
            ..
        }
    ));
    assert!(matches!(
        players,
        PlayerSet::One(PlayerRef::AbilityController)
    ));
    assert!(matches!(
        abilities,
        ObjectFilter::OnStack(StackTrait::Keyword(AbilityKeyword::Craft))
    ));
}

#[test]
fn susan_foreman_looks_in_her_planar_deck_then_planeswalks() {
    let card = typed("Susan Foreman");
    let body = card
        .abilities
        .iter()
        .find_map(|ability| match ability {
            Ability::Replacement { body, .. } => Some(body),
            _ => None,
        })
        .expect("a replacement");
    assert_eq!(kinds(body), ["ModifyEvent", "Group", "Keyword"]);
    let Instruction::Effect {
        operation: Operation::ModifyEvent { change },
        ..
    } = &body.instructions[0]
    else {
        panic!("{:?}", body.instructions[0]);
    };
    assert!(matches!(change, EventChange::Cancel));
    let Instruction::Group(group) = &body.instructions[1] else {
        panic!("{:?}", body.instructions[1]);
    };
    assert_eq!(kinds(group), ["LookAt", "ChooseObjects", "Move", "Move"]);
    let Instruction::Effect {
        operation: Operation::LookAt { objects, .. },
        ..
    } = &group.instructions[0]
    else {
        panic!("{:?}", group.instructions[0]);
    };
    let ObjectSet::Take {
        objects,
        count: Expr::Constant(2),
        from: SequenceEnd::Top,
    } = objects
    else {
        panic!("{objects:?}");
    };
    assert!(matches!(
        **objects,
        ObjectSet::Zone {
            scope: ZoneScope::Players {
                zone: EventZone::PlanarDeck,
                ..
            },
            ..
        }
    ));
    let moved: Vec<_> = group.instructions[2..]
        .iter()
        .map(|instruction| match instruction {
            Instruction::Effect {
                operation: Operation::Move { destination, .. },
                ..
            } => destination.clone(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert!(matches!(
        moved.as_slice(),
        [
            Destination::OwnersPlanarDeck(SequenceEnd::Bottom),
            Destination::OwnersPlanarDeck(SequenceEnd::Top)
        ]
    ));
    let Instruction::Effect {
        operation: Operation::Keyword { action, .. },
        ..
    } = &body.instructions[2]
    else {
        panic!("{:?}", body.instructions[2]);
    };
    assert!(matches!(action, KeywordAction::Planeswalk));
}

#[test]
fn the_celestial_toymaker_counts_stack_objects_by_what_they_caused() {
    let card = typed("The Celestial Toymaker");
    let text = serde_json::to_string(&json(&card)).unwrap();
    for shape in [
        "\"Caused\"",
        "\"Guesses\"",
        "\"GroupsIntoPiles\"",
        "\"DistinctCauses\"",
    ] {
        assert!(text.contains(shape), "{shape}: {text}");
    }
    assert!(text.contains("\"ThisTurn\""), "{text}");
}

#[test]
fn vedalken_squirrel_whacker_keeps_the_roll_and_exchanges_a_result() {
    let card = typed("Vedalken Squirrel-Whacker");
    let body = card
        .abilities
        .iter()
        .find_map(|ability| match ability {
            Ability::Replacement { event, body, .. }
                if matches!(
                    event,
                    ReplacementEvent::Player {
                        event: ReplacedPlayerEvent::RollD6,
                        ..
                    }
                ) =>
            {
                Some(body)
            }
            _ => None,
        })
        .expect("a dice replacement");
    let [Instruction::Effect {
        operation:
            Operation::ModifyEvent {
                change:
                    EventChange::ExchangeDieResult {
                        object, properties, ..
                    },
            },
        ..
    }] = body.instructions.as_slice()
    else {
        panic!("{body:?}");
    };
    assert!(matches!(object, ObjectRef::Source));
    assert!(matches!(
        properties.as_slice(),
        [Property::BasePower, Property::BaseToughness]
    ));
}

/// The two random choices on Davriel's minus ability.
fn davriel_choices() -> (Vec<Program>, Vec<Program>) {
    let card = typed("Davriel, Soul Broker");
    let body = card
        .abilities
        .iter()
        .find_map(|ability| match ability {
            Ability::Activated { body, .. }
                if body
                    .instructions
                    .iter()
                    .any(|i| matches!(i, Instruction::ChooseOne { .. })) =>
            {
                Some(body)
            }
            _ => None,
        })
        .expect("the offers");
    let choices: Vec<_> = body
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            Instruction::ChooseOne {
                options, offered, ..
            } => {
                assert!(matches!(offered, Some(Expr::Constant(3))), "{offered:?}");
                assert_eq!(options.len(), 8);
                Some(options.clone())
            }
            _ => None,
        })
        .collect();
    let [offers, conditions] = <[_; 2]>::try_from(choices).unwrap();
    (offers, conditions)
}

#[test]
fn davriel_offers_eight_typed_programs() {
    let (offers, _) = davriel_choices();
    let kinds: Vec<_> = offers.iter().map(kinds).collect();
    assert_eq!(kinds[0], ["Draw"]);
    assert_eq!(kinds[1], ["Conjure"]);
    assert_eq!(kinds[2].last().unwrap(), "CreatePerpetualEffect");
    for emblem in &kinds[4..] {
        assert_eq!(emblem, &["CreateEmblem"]);
    }
    let text = |n: usize| serde_json::to_string(&offers[n]).unwrap();
    assert!(text(1).contains("Manor Guardian"));
    for shape in ["\"AtRandom\"", "\"OwnersHand\"", "\"InSet\"", "\"Named\""] {
        assert!(text(2).contains(shape), "{shape}: {}", text(2));
    }
    for shape in [
        "\"AtRandom\"",
        "\"Maximum\"",
        "\"ManaValue\"",
        "\"Battlefield\"",
    ] {
        assert!(text(3).contains(shape), "{shape}: {}", text(3));
    }
    assert!(text(5).contains("\"Black\""), "{}", text(5));
    for shape in ["\"Davriel\"", "\"GrantAbility\"", "\"Loyalty\""] {
        assert!(text(6).contains(shape), "{shape}: {}", text(6));
    }
    assert!(text(7).contains("\"Draws\""), "{}", text(7));
}

#[test]
fn davriel_conditions_are_eight_typed_programs() {
    let (_, conditions) = davriel_choices();
    let kinds: Vec<_> = conditions.iter().map(kinds).collect();
    assert_eq!(kinds[0], ["LoseLife"]);
    assert_eq!(kinds[2].len(), 1);
    assert_eq!(kinds[3], ["CreatePerpetualEffect"]);
    for emblem in &kinds[4..] {
        assert_eq!(emblem, &["CreateEmblem"]);
    }
    let text = |n: usize| serde_json::to_string(&conditions[n]).unwrap();
    for shape in [
        "\"If\"",
        "\"Less\"",
        "\"Subtract\"",
        "\"OpponentsOf\"",
        "\"Draw\"",
    ] {
        assert!(text(1).contains(shape), "{shape}: {}", text(1));
    }
    assert!(text(3).contains("\"OthersThan\""), "{}", text(3));
    for shape in ["\"Increase\"", "\"Black\""] {
        assert!(text(5).contains(shape), "{shape}: {}", text(5));
    }
    for shape in ["\"Draws\"", "\"Take\"", "\"Exile\""] {
        assert!(text(6).contains(shape), "{shape}: {}", text(6));
    }
    for shape in ["\"UpkeepBegins\"", "\"CountObjects\"", "\"LoseLife\""] {
        assert!(text(7).contains(shape), "{shape}: {}", text(7));
    }
}

#[test]
fn a_form_with_no_reduction_still_falls_back_to_a_primitive() {
    let actor = PlayerSet::One(PlayerRef::AbilityController);
    let action = mtgish_ast::semantics::reduce_action(actor, "Action::NoSuchForm".into(), vec![]);
    assert!(matches!(action, Operation::Primitive { .. }));
    let event = mtgish_ast::semantics::reduce_event("Trigger::NoSuchForm".into(), vec![]);
    assert!(matches!(event, EventSpec::Primitive { .. }));
}
