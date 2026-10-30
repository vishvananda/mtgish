use mtgish_ast::{
    corpus,
    ir::*,
    lower, mtg_types as source,
    semantics::{activation_kind, ActivationKind},
    validate,
};

fn sources() -> impl Iterator<Item = source::OracleCard> {
    include_str!("fixtures/library.ronl")
        .lines()
        .map(|l| ron::from_str(l).unwrap())
}
fn source(name: &str) -> source::OracleCard {
    sources()
        .find(|c| {
            corpus::Identity::of(c)
                .unwrap()
                .names
                .iter()
                .any(|n| n == name)
        })
        .unwrap()
}
fn card(name: &str) -> Card {
    let c = lower::card(&source(name)).unwrap_or_else(|e| panic!("{name}: {e:?}"));
    validate::card(&c).unwrap();
    c
}
fn custom(rule: &str) -> Result<Card, Vec<lower::Diagnostic>> {
    let mut c = source("Demonic Tutor");
    let source::OracleCard::Card { rules, .. } = &mut c else {
        panic!()
    };
    *rules = Some(vec![ron::from_str(rule).unwrap()]);
    lower::card(&c)
}
fn spell(c: &Card) -> &Program {
    c.abilities
        .iter()
        .find_map(|a| match a {
            Ability::Spell { body, .. } => Some(body),
            _ => None,
        })
        .unwrap()
}
fn group(c: &Card) -> &Program {
    let Instruction::Group(p) = &spell(c).instructions[0] else {
        panic!()
    };
    p
}
fn search(c: &Card) -> (&PlayerRef, &PlayerRef, &[SearchChoice]) {
    let Instruction::Effect {
        operation:
            Operation::Search {
                player,
                library,
                choices,
                ..
            },
        ..
    } = &group(c).instructions[0]
    else {
        panic!()
    };
    (player, library, choices)
}
fn effect(operation: Operation, id: u32) -> Instruction {
    Instruction::Effect {
        result: ResultId(id),
        operation,
    }
}

#[test]
fn real_library_families_roundtrip_and_advanced_steps_remain_diagnostics() {
    let incomplete = [
        "Archdruid's Charm",
        "Beseech the Mirror",
        "Caravan Vigil",
        "Claim Jumper",
        "Cultivator Colossus",
        "Green Sun's Zenith",
        "Land Grant",
        "Liliana Vess",
        "Plunge into Darkness",
        "Praetor's Grasp",
        "Snooping Newsie",
        "Tooth and Nail",
        "Urza's Saga",
        "Varragoth, Bloodsky Sire",
        "Wishclaw Talisman",
        "Woe Strider",
    ];
    let mut complete = 0;
    for source in sources() {
        let name = corpus::Identity::of(&source).unwrap().names.remove(0);
        if incomplete.contains(&name.as_str()) {
            assert!(lower::card(&source).is_err(), "{name}");
            continue;
        }
        let c = lower::card(&source).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let expected = serde_json::to_value(&c).unwrap();
        let bytes = bincode::encode_to_vec(&c, bincode::config::standard()).unwrap();
        let (binary, used): (Card, usize) =
            bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
        assert_eq!(used, bytes.len());
        for loaded in [
            binary,
            ron::from_str(&ron::ser::to_string(&c).unwrap()).unwrap(),
            serde_json::from_value(expected.clone()).unwrap(),
        ] {
            validate::card(&loaded).unwrap();
            assert_eq!(serde_json::to_value(loaded).unwrap(), expected, "{name}");
        }
        complete += 1;
    }
    assert_eq!(complete, 74);
}

#[test]
fn stated_qualities_and_optional_quantities_keep_different_search_requirements() {
    let c = card("Demonic Tutor");
    assert!(matches!(
        search(&c).2,
        [SearchChoice {
            filter: ObjectFilter::Any,
            count: Count::AsManyAsCan(Expr::Constant(1)),
            ..
        }]
    ));
    assert!(!group(&c).instructions.iter().any(|i| matches!(
        i,
        Instruction::Effect {
            operation: Operation::Reveal { .. },
            ..
        }
    )));
    let c = card("Worldly Tutor");
    assert!(matches!(
        search(&c).2,
        [SearchChoice {
            filter: ObjectFilter::CardType(source::CardType::Creature),
            count: Count::UpTo(Expr::Constant(1)),
            ..
        }]
    ));
    assert!(group(&c).instructions.iter().any(|i| matches!(
        i,
        Instruction::Effect {
            operation: Operation::Reveal { .. },
            ..
        }
    )));
    let c = card("Denying Wind");
    assert!(matches!(
        search(&c).2,
        [SearchChoice {
            filter: ObjectFilter::Any,
            count: Count::UpTo(Expr::Constant(7)),
            ..
        }]
    ));
    let c = custom("SpellActions(ActionList([SearchLibrary([FindNumberGenericCards(Integer(3)), PutFoundCardsIntoHand, Shuffle])]))").unwrap();
    assert!(matches!(
        search(&c).2,
        [SearchChoice {
            count: Count::AsManyAsCan(Expr::Constant(3)),
            ..
        }]
    ));
}

#[test]
fn searcher_library_owner_and_battlefield_controller_remain_distinct() {
    let c = card("Bribery");
    assert!(matches!(search(&c).0, PlayerRef::AbilityController));
    assert!(matches!(
        search(&c).1,
        PlayerRef::Target {
            slot: Slot(0),
            index: 0
        }
    ));
    assert!(group(&c).instructions.iter().any(|i| matches!(
        i,
        Instruction::Effect {
            operation: Operation::Move {
                destination: Destination::Battlefield(PlayerRef::AbilityController),
                ..
            },
            ..
        }
    )));
    let c = card("Path to Exile");
    let [Instruction::Effect {
        operation:
            Operation::Move {
                destination: Destination::Exile,
                ..
            },
        ..
    }, Instruction::Optional { decider, body }] = spell(&c).instructions.as_slice()
    else {
        panic!()
    };
    assert!(
        matches!(decider,PlayerRef::ControllerOf(object) if matches!(object.as_ref(),ObjectRef::Target { slot:Slot(0), .. }))
    );
    let [Instruction::Group(p)] = body.instructions.as_slice() else {
        panic!()
    };
    let Instruction::Effect {
        operation: Operation::Search {
            player, library, ..
        },
        ..
    } = &p.instructions[0]
    else {
        panic!()
    };
    assert_eq!(
        serde_json::to_value(player).unwrap(),
        serde_json::to_value(decider).unwrap()
    );
    assert_eq!(
        serde_json::to_value(library).unwrap(),
        serde_json::to_value(decider).unwrap()
    );
}

#[test]
fn multiple_find_clauses_share_one_episode_and_set_aside_cards_stay_in_the_library() {
    for (name, n) in [("Firemind's Foresight", 3), ("Krosan Verge", 2)] {
        let c = card(name);
        let p = if name == "Krosan Verge" {
            c.abilities
                .iter()
                .find_map(|a| match a {
                    Ability::Activated { body, .. } => match body.instructions.first() {
                        Some(Instruction::Group(p)) => Some(p),
                        _ => None,
                    },
                    _ => None,
                })
                .unwrap()
        } else {
            group(&c)
        };
        assert!(
            matches!(&p.instructions[0],Instruction::Effect { operation: Operation::Search { choices,.. },.. } if choices.len()==n)
        );
        assert_eq!(
            p.instructions
                .iter()
                .filter(|i| matches!(
                    i,
                    Instruction::Effect {
                        operation: Operation::Search { .. },
                        ..
                    }
                ))
                .count(),
            1
        );
    }
    let c = card("Worldly Tutor");
    assert!(matches!(
        group(&c).instructions.as_slice(),
        [
            Instruction::Effect {
                result: ResultId(0),
                operation: Operation::Search { .. }
            },
            Instruction::Effect {
                operation: Operation::Reveal {
                    objects: ObjectSet::Selected(ResultId(0))
                },
                ..
            },
            Instruction::Effect {
                operation: Operation::Shuffle {
                    excluding: Some(ObjectSet::Selected(ResultId(0))),
                    ..
                },
                ..
            },
            Instruction::Effect {
                operation: Operation::ReorderLibrary {
                    objects: ObjectSet::Selected(ResultId(0)),
                    end: SequenceEnd::Top,
                    order: CardOrder::Chosen,
                    ..
                },
                ..
            }
        ]
    ));
    let c = card("Begin the Invasion");
    assert!(
        matches!(search(&c).2,[SearchChoice {constraints,count:Count::UpTo(Expr::AnnouncedX),..}] if matches!(constraints.as_slice(),[SelectionConstraint::DifferentNames]))
    );
}

#[test]
fn split_placement_choices_preserve_original_found_identities_and_entry_flags() {
    let c = card("Cultivate");
    let p = group(&c);
    let Instruction::Effect {
        result: found,
        operation: Operation::Search { choices, .. },
    } = &p.instructions[0]
    else {
        panic!()
    };
    assert!(matches!(choices[0].count, Count::UpTo(Expr::Constant(2))));
    let moves = p
        .instructions
        .iter()
        .filter_map(|i| match i {
            Instruction::Effect {
                operation:
                    Operation::Move {
                        objects,
                        destination,
                        entry,
                        ..
                    },
                ..
            } => Some((objects, destination, entry)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(moves.len(), 2);
    assert!(
        matches!(moves[0],(ObjectSet::Selected(chosen),Destination::Battlefield(_),EntrySpec {tapped:true,..}) if chosen.0!=found.0)
    );
    assert!(
        matches!(moves[1],(ObjectSet::Selected(rest),Destination::OwnersHand,_) if rest.0==found.0)
    );
    // Once a found card moves it has a new identity; the original selection
    // cannot move it twice or retrieve it after an unrelated replacement move.
}

#[test]
fn search_results_have_typed_status_and_zone_scope() {
    let c=custom("SpellActions(ActionList([SearchTheTopNumberCardsOfLibrary(Integer(4), [FindAGenericCard, PutFoundCardsIntoHand, ShuffleLibraryIfSearched])]))").unwrap();
    validate::card(&c).unwrap();
    assert!(matches!(
        &group(&c).instructions[0],
        Instruction::Effect {
            operation: Operation::Search {
                top: Some(Expr::Constant(4)),
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        group(&c).instructions.last().unwrap(),
        Instruction::If {
            condition: Condition::Searched(ResultId(0)),
            ..
        }
    ));
    let mut c = custom("SpellActions(ActionList([SearchLibrary([FindAGenericCard])]))").unwrap();
    let Ability::Spell { body, .. } = &mut c.abilities[0] else {
        panic!()
    };
    body.instructions.push(effect(
        Operation::Tap {
            objects: ObjectSet::Selected(ResultId(0)),
            tapped: true,
        },
        1,
    ));
    assert!(validate::card(&c).unwrap_err().contains("Battlefield"));
    let mut c = card("Demonic Tutor");
    let Ability::Spell { body, .. } = &mut c.abilities[0] else {
        panic!()
    };
    body.instructions = vec![
        effect(
            Operation::ChooseObjects {
                constraints: vec![],
                player: PlayerRef::AbilityController,
                objects: ObjectSet::Battlefield(ObjectFilter::Any),
                count: Count::UpTo(Expr::Constant(1)),
            },
            0,
        ),
        Instruction::If {
            condition: Condition::Searched(ResultId(0)),
            then: Program::empty(),
            otherwise: Program::empty(),
        },
    ];
    assert!(validate::card(&c).unwrap_err().contains("search result"));
}

#[test]
fn separated_unshuffled_searches_are_detected_across_optional_branches() {
    assert!(custom("SpellActions(ActionList([SearchLibrary([FindAGenericCard, PutFoundCardsIntoHand]), SearchLibrary([FindAGenericCard, PutFoundCardsIntoHand, Shuffle])]))").is_err());
    assert!(custom("SpellActions(ActionList([MayAction(SearchLibrary([FindAGenericCard, PutFoundCardsIntoHand])), SearchLibrary([FindAGenericCard, PutFoundCardsIntoHand, Shuffle])]))").is_err());
    for first in ["Shuffle", "ShuffleLibraryIfSearched"] {
        let c=custom(&format!("SpellActions(ActionList([SearchLibrary([FindAGenericCard, PutFoundCardsIntoHand, {first}]), SearchLibrary([FindAGenericCard, PutFoundCardsIntoHand, Shuffle])]))")).unwrap();
        validate::card(&c).unwrap();
    }
}

#[test]
fn typecycling_reuses_search_but_retains_keyword_identity_and_canonical_payment() {
    let c = card("Pale Recluse");
    let mut count = 0;
    for a in &c.abilities {
        let Ability::Scoped { zones, ability, .. } = a else {
            continue;
        };
        let Ability::Activated {
            origin: ActivationOrigin::Cycling {
                search: Some(filter),
            },
            cost,
            announcement,
            body,
            ..
        } = ability.as_ref()
        else {
            continue;
        };
        count += 1;
        assert_eq!(zones, &[EventZone::Hand]);
        assert!(matches!(
            cost.parts.last(),
            Some(CostPart::Discard {
                filter: ObjectFilter::SameObject(ObjectRef::Source),
                ..
            })
        ));
        assert_eq!(
            activation_kind(cost, announcement, body),
            ActivationKind::Stack
        );
        let [Instruction::Group(p)] = body.instructions.as_slice() else {
            panic!()
        };
        let Instruction::Effect {
            operation: Operation::Search { choices, .. },
            ..
        } = &p.instructions[0]
        else {
            panic!()
        };
        assert_eq!(
            serde_json::to_value(&choices[0].filter).unwrap(),
            serde_json::to_value(filter).unwrap()
        );
        assert!(matches!(choices[0].count, Count::UpTo(Expr::Constant(1))));
        assert!(matches!(
            &p.instructions[1],
            Instruction::Effect {
                operation: Operation::Reveal { .. },
                ..
            }
        ));
    }
    assert_eq!(count, 2);
}

#[test]
fn library_observation_and_reordering_do_not_masquerade_as_zone_movement() {
    let cost = Cost { parts: vec![] };
    let ann = Announcement::empty();
    let player = PlayerRef::AbilityController;
    let mana = effect(
        Operation::AddMana {
            players: PlayerSet::One(player.clone()),
            mana: ManaProduction::Amount {
                color: ManaColor::Green,
                amount: Expr::Constant(1),
            },
            restriction: None,
        },
        99,
    );
    let kind = |mut ops: Vec<Instruction>| {
        ops.push(mana.clone());
        activation_kind(&cost, &ann, &Program { instructions: ops })
    };
    assert_eq!(
        kind(vec![effect(
            Operation::Scry {
                players: PlayerSet::One(player.clone()),
                amount: Expr::Constant(2)
            },
            0
        )]),
        ActivationKind::Mana
    );
    assert_eq!(
        kind(vec![effect(
            Operation::Surveil {
                players: PlayerSet::One(player.clone()),
                amount: Expr::Constant(2)
            },
            0
        )]),
        ActivationKind::Stack
    );
    assert_eq!(
        kind(vec![effect(
            Operation::Surveil {
                players: PlayerSet::One(player.clone()),
                amount: Expr::Constant(0)
            },
            0
        )]),
        ActivationKind::Mana
    );
    assert_eq!(
        kind(vec![effect(
            Operation::Shuffle {
                players: PlayerSet::One(player.clone()),
                excluding: None
            },
            0
        )]),
        ActivationKind::Mana
    );
    let search = effect(
        Operation::Search {
            player: player.clone(),
            library: player,
            top: None,
            choices: vec![SearchChoice {
                filter: ObjectFilter::Any,
                count: Count::AsManyAsCan(Expr::Constant(1)),
                constraints: vec![],
            }],
        },
        0,
    );
    assert_eq!(kind(vec![search.clone()]), ActivationKind::Mana);
    assert_eq!(
        kind(vec![
            search,
            effect(
                Operation::Move {
                    objects: ObjectSet::Selected(ResultId(0)),
                    destination: Destination::OwnersHand,
                    entry: EntrySpec::default(),
                    face_down: false
                },
                1
            )
        ]),
        ActivationKind::Stack
    );
    let c = custom("SpellActions(ActionList([Scry(Integer(0)),Surveil(Integer(0))]))").unwrap();
    assert_eq!(spell(&c).instructions.len(), 2);
}

#[test]
fn a_kicked_spell_announces_each_branch_on_its_own() {
    let c = card("Grow from the Ashes");
    let [Ability::CostBranch {
        keyword,
        paid,
        unpaid,
        ..
    }] = &c.abilities[..]
    else {
        panic!("{:?}", c.abilities);
    };
    assert_eq!(keyword, "Kicker");
    assert!(matches!(**paid, Ability::Spell { .. }));
    assert!(matches!(**unpaid, Ability::Spell { .. }));
}
