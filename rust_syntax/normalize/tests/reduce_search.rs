//! Library searches, reveals and looks, reflexive triggers and counter moves keep
//! a general shape instead of a row named for the card.
use mtgish_ast::{corpus, ir::*, mtg_types as source, unified, validate};

fn sources() -> impl Iterator<Item = source::OracleCard> {
    include_str!("fixtures/reduce_search.ronl")
        .lines()
        .map(|s| ron::from_str(s).unwrap())
}
fn card(name: &str) -> Card {
    let source = sources()
        .find(|c| {
            corpus::Identity::of(c)
                .unwrap()
                .names
                .iter()
                .any(|n| n == name)
        })
        .unwrap_or_else(|| panic!("{name} is not in the fixture"));
    let lowered = unified::lower(&source);
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    validate::unified(&lowered.card).unwrap();
    lowered
        .card
        .faces
        .into_iter()
        .find(|face| face.name == name)
        .unwrap_or_else(|| panic!("{name} is not a face"))
}
fn collect<'a>(program: &'a Program, out: &mut Vec<&'a Instruction>) {
    for instruction in &program.instructions {
        out.push(instruction);
        match instruction {
            Instruction::Group(p) | Instruction::Repeatable(p) => collect(p, out),
            Instruction::If {
                then, otherwise, ..
            } => {
                collect(then, out);
                collect(otherwise, out);
            }
            Instruction::Optional { body, .. }
            | Instruction::ForEachPlayer { body, .. }
            | Instruction::ForEachObject { body, .. } => collect(body, out),
            _ => {}
        }
    }
}
/// Every instruction of an ability's body, in order, below any nesting.
fn steps(c: &Card, ability: usize) -> Vec<&Instruction> {
    let bodies: Vec<_> = c
        .abilities
        .iter()
        .filter_map(|a| match a {
            Ability::Spell { body, .. }
            | Ability::Activated { body, .. }
            | Ability::Triggered { body, .. } => Some(body),
            _ => None,
        })
        .collect();
    let mut out = vec![];
    collect(bodies[ability], &mut out);
    out
}
fn operations(c: &Card, ability: usize) -> Vec<&Operation> {
    steps(c, ability)
        .into_iter()
        .filter_map(|i| match i {
            Instruction::Effect { operation, .. } => Some(operation),
            _ => None,
        })
        .collect()
}

#[test]
fn no_card_here_keeps_a_primitive() {
    for source in sources() {
        let names = corpus::Identity::of(&source).unwrap().names;
        let lowered = unified::lower(&source);
        assert!(lowered.diagnostics.is_empty(), "{names:?}");
        validate::unified(&lowered.card).unwrap();
        let text = format!("{:?}", lowered.card);
        assert!(!text.contains("Primitive"), "{names:?}: {text}");
    }
}

#[test]
fn a_counter_move_can_divide_among_a_set() {
    let c = card("Forgotten Ancient");
    let ops = operations(&c, 1);
    let Some(Operation::MoveCounters {
        to, kind, amount, ..
    }) = ops.first()
    else {
        panic!("{ops:?}")
    };
    assert!(matches!(
        to,
        ObjectSet::Zone { .. } | mtgish_ast::battlefield_query!(_)
    ));
    assert!(kind.is_some());
    assert!(matches!(amount, MovedCounters::AnyNumber));
}

#[test]
fn goldberry_moves_kinds_it_lacks_and_pays_a_move_as_a_cost() {
    let c = card("Goldberry, River-Daughter");
    assert!(matches!(
        operations(&c, 0).first(),
        Some(Operation::MoveCounters {
            amount: MovedCounters::OneOfEachKindMissing,
            kind: None,
            ..
        })
    ));
    let text = format!("{:?}", operations(&c, 1));
    assert!(
        text.contains("CostPart") || text.contains("MoveCounters"),
        "{text}"
    );
    assert!(text.contains("AtLeast(Constant(1))"), "{text}");
}

#[test]
fn each_counter_kind_offers_its_options() {
    let c = card("Quarry Hauler");
    let Some(Operation::EachCounterKind {
        options, optional, ..
    }) = operations(&c, 0).first().copied()
    else {
        panic!()
    };
    assert!(!optional);
    assert!(matches!(
        options.as_slice(),
        [CounterStep::PutThatKind(_), CounterStep::RemoveThatKind(_)]
    ));
    let c = card("Bribe Taker");
    let Some(Operation::EachCounterKind {
        options, optional, ..
    }) = operations(&c, 0).first().copied()
    else {
        panic!()
    };
    assert!(optional);
    assert!(matches!(
        options.as_slice(),
        [CounterStep::Put(..), CounterStep::PutThatKind(_)]
    ));
}

#[test]
fn a_saga_chapter_may_duplicate_one_counter_on_each_permanent() {
    let text = format!("{:?}", card("The Caves of Androzani"));
    assert!(text.contains("EachObject"), "{text}");
    assert!(text.contains("UpToOne"), "{text}");
}

#[test]
fn a_kind_at_random_is_one_this_permanent_lacks() {
    let c = card("Crystalline Giant");
    assert!(matches!(
        operations(&c, 0).first(),
        Some(Operation::PutChosenCounters {
            choice: CounterChoice::RandomMissing(_),
            ..
        })
    ));
}

#[test]
fn a_card_at_random_is_a_search_with_no_choice() {
    let c = card("Higure, the Still Wind Avatar");
    let Some(Operation::Search { choices, .. }) = operations(&c, 0).first().copied() else {
        panic!()
    };
    assert!(matches!(
        choices[0].constraints.as_slice(),
        [SelectionConstraint::AtRandom]
    ));
    let c = card("Enigma Sphinx Avatar");
    let ops = operations(&c, 0);
    assert!(matches!(
        ops[1],
        Operation::PlayCards {
            action: CardPlayAction::Either,
            cost: PlayCost::Free,
            ..
        }
    ));
    assert!(matches!(
        ops[2],
        Operation::ReorderLibrary {
            end: SequenceEnd::Bottom,
            ..
        }
    ));
    assert!(!ops.iter().any(|o| matches!(o, Operation::Shuffle { .. })));
}

#[test]
fn a_search_reaches_outside_the_game_for_cards_owned() {
    let c = card("Turtles Forever");
    let ops = operations(&c, 0);
    let Some(Operation::Search {
        choices,
        outside,
        either,
        ..
    }) = ops.first().copied()
    else {
        panic!()
    };
    assert!(*outside && *either);
    assert!(matches!(choices[0].count, Count::Exactly(_)));
    assert!(format!("{:?}", choices[0].filter).contains("OwnedBy"));
    // The rest not yet in the library arrive there before the shuffle.
    let shuffle = ops
        .iter()
        .position(|o| matches!(o, Operation::Shuffle { .. }))
        .unwrap();
    assert!(matches!(
        ops[shuffle - 1],
        Operation::Move {
            destination: Destination::OwnersLibraryTop,
            ..
        }
    ));
    let c = card("Invasion of Arcavios");
    let Some(Operation::Search { also, outside, .. }) = operations(&c, 0).first().copied() else {
        panic!()
    };
    assert!(*outside);
    assert_eq!(also.len(), 1);
}

#[test]
fn each_player_library_is_searched_by_one_player() {
    let c = card("Jace, Architect of Thought");
    let text = format!("{:?}", steps(&c, 2)[0]);
    assert!(text.starts_with("ForEachPlayer"), "{text}");
    let Some(Operation::Search {
        player, library, ..
    }) = operations(&c, 2).first().copied()
    else {
        panic!()
    };
    assert!(matches!(player, PlayerRef::AbilityController));
    assert!(matches!(library, PlayerRef::Bound(_)));
}

#[test]
fn each_player_reveals_then_one_set_of_steps_reads_every_reveal() {
    let c = card("Dance, Pathetic Marionette");
    let ops = operations(&c, 0);
    assert!(matches!(ops[0], Operation::RevealUntil { .. }));
    assert!(matches!(ops[1], Operation::ChooseObjects { .. }));
    assert!(matches!(
        ops[3],
        Operation::Move {
            destination: Destination::OwnersGraveyard,
            ..
        }
    ));
}

#[test]
fn players_pick_in_order_starting_with_a_seat_and_none_repeats() {
    let c = card("Manifold Insights");
    let each = steps(&c, 0)
        .into_iter()
        .find_map(|i| match i {
            Instruction::ForEachPlayer {
                starting_with,
                body,
                ..
            } => Some((starting_with, body)),
            _ => None,
        })
        .unwrap();
    assert!(each.0.is_some());
    assert!(format!("{:?}", each.1).contains("Unpicked"));
}

#[test]
fn cards_stay_only_when_paid_for() {
    for (name, destination) in [
        ("Sword-Point Diplomacy", "OwnersHand"),
        ("Moonlight Bargain", "OwnersGraveyard"),
    ] {
        let c = card(name);
        let ops = operations(&c, 0);
        let moved = ops.iter().find_map(|o| match o {
            Operation::Move {
                objects: ObjectSet::Unpaid(_),
                destination,
                ..
            } => Some(format!("{destination:?}")),
            _ => None,
        });
        assert_eq!(moved.as_deref(), Some(destination), "{name}");
    }
    let text = format!("{:?}", operations(&card("Sword-Point Diplomacy"), 0));
    assert!(text.contains("FirstWilling"), "{text}");
}

#[test]
fn a_chosen_group_can_divide_between_two_zones() {
    let c = card("Green Sun's Twilight");
    let text = format!("{:?}", operations(&c, 0));
    assert!(text.contains("OneForEach"), "{text}");
    assert!(text.contains("Difference"), "{text}");
}

#[test]
fn each_color_offers_a_card_only_if_a_permanent_has_it() {
    let c = card("Sanar, Innovative First-Year");
    let ops = operations(&c, 0);
    let Some(Operation::ChooseObjects { constraints, .. }) = ops
        .iter()
        .find(|o| matches!(o, Operation::ChooseObjects { .. }))
        .copied()
    else {
        panic!()
    };
    let SelectionConstraint::OneForEach(colors) = &constraints[0] else {
        panic!()
    };
    assert_eq!(colors.len(), 5);
    assert!(format!("{:?}", colors[0]).contains("Condition"));
}

#[test]
fn a_look_repeats_on_the_cards_then_on_top() {
    let c = card("Lim-Dûl's Vault");
    let all = steps(&c, 0);
    assert!(all.iter().any(|i| matches!(i, Instruction::Repeatable(_))));
    let ops = operations(&c, 0);
    let last: Vec<_> = ops.iter().rev().take(3).rev().collect();
    assert!(matches!(last[0], Operation::NoteObjects { .. }));
    assert!(matches!(
        last[1],
        Operation::Shuffle {
            excluding: Some(_),
            ..
        }
    ));
    assert!(matches!(
        last[2],
        Operation::ReorderLibrary {
            end: SequenceEnd::Top,
            ..
        }
    ));
}

#[test]
fn a_card_exiled_face_down_carries_its_counter() {
    let c = card("The Dragon-Kami Reborn");
    let text = format!("{c:?}");
    assert!(text.contains("face_down: true"), "{text}");
    assert!(text.contains("HatchingCounter"), "{text}");
}

#[test]
fn each_exile_in_turn_keeps_whose_it_was() {
    let c = card("Struggle for Sanity");
    let ops = operations(&c, 0);
    let moves: Vec<_> = ops
        .iter()
        .filter(|o| matches!(o, Operation::Move { .. }))
        .collect();
    assert_eq!(moves.len(), 4);
    let text = format!("{:?}", moves[2]);
    let other = format!("{:?}", moves[3]);
    assert!(text.contains("Transition"), "{text}");
    assert!(other.contains("Transition"), "{other}");
    assert_ne!(
        text.replace("OwnersHand", ""),
        other.replace("OwnersGraveyard", "")
    );
}

#[test]
fn a_chosen_card_from_a_graveyard_top_leaves_the_other_one() {
    let c = card("Phyrexian Grimoire");
    let ops = operations(&c, 0);
    assert!(matches!(
        ops[2],
        Operation::Move {
            objects: ObjectSet::Unselected(_),
            destination: Destination::OwnersHand,
            ..
        }
    ));
}

#[test]
fn a_chooser_offers_cards_and_another_may_cast_one() {
    let c = card("Extract Brain");
    let ops = operations(&c, 0);
    assert!(matches!(ops[1], Operation::LookAt { .. }));
    assert!(matches!(
        ops[2],
        Operation::PlayCards {
            action: CardPlayAction::CastSpell,
            cost: PlayCost::Free,
            ..
        }
    ));
}

#[test]
fn a_greatest_reveal_reads_one_winner() {
    let c = card("Psychic Battle");
    let text = format!("{:?}", steps(&c, 0));
    assert!(text.contains("LibraryTops"), "{text}");
    assert!(text.contains("HadExtreme"), "{text}");
    assert!(text.contains("Extreme"), "{text}");
}

#[test]
fn a_reflexive_trigger_may_have_no_base_action() {
    let c = card("Gimli, Mournful Avenger");
    let reflexive = operations(&c, 0)
        .into_iter()
        .find(|o| matches!(o, Operation::ReflexiveTrigger { .. }))
        .unwrap();
    let Operation::ReflexiveTrigger { performed, .. } = reflexive else {
        unreachable!()
    };
    assert!(performed.is_none());
}

#[test]
fn each_draw_has_its_own_reflexive_trigger() {
    let c = card("Explosion of Riches");
    let ops = operations(&c, 0);
    let performed: Vec<_> = ops
        .iter()
        .filter_map(|o| match o {
            Operation::ReflexiveTrigger { performed, .. } => Some(performed),
            _ => None,
        })
        .collect();
    assert_eq!(performed.len(), 2);
    let results: Vec<_> = performed.iter().filter_map(|p| p.as_ref()).collect();
    assert_eq!(results.len(), 2);
    assert_ne!(results[0].0, results[1].0);
}

#[test]
fn the_cards_just_looked_at_go_onto_the_battlefield_reflexively() {
    for (name, ability, any_number) in [
        ("Vivien's Invocation", 0, false),
        ("Gilgamesh, Master-at-Arms", 0, true),
    ] {
        let c = card(name);
        let ops = operations(&c, ability);
        let Some(Operation::ChooseObjects { count, .. }) = ops
            .iter()
            .find(|o| matches!(o, Operation::ChooseObjects { .. }))
            .copied()
        else {
            panic!("{name}")
        };
        assert_eq!(matches!(count, Count::AnyNumber), any_number, "{name}");
        assert!(ops.iter().any(|o| matches!(
            o,
            Operation::ReflexiveTrigger {
                performed: Some(_),
                ..
            }
        )));
    }
}

#[test]
fn a_cast_with_an_extra_cost_records_the_spell_for_its_reflexive_trigger() {
    let c = card("Ogre Battlecaster");
    let ops = operations(&c, 0);
    assert!(matches!(
        ops[0],
        Operation::PlayWith {
            terms: PlayTerms {
                cost: PlayCost::Additional(_),
                into_exile: true,
                ..
            },
            ..
        }
    ));
    assert!(matches!(ops[1], Operation::ReflexiveTrigger { .. }));
}
