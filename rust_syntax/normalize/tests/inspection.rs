use mtgish_ast::{
    corpus,
    ir::*,
    lower, mtg_types as source,
    semantics::{activation_kind, ActivationKind},
    validate,
};

fn sources() -> impl Iterator<Item = source::OracleCard> {
    include_str!("fixtures/inspection.ronl")
        .lines()
        .map(|s| ron::from_str(s).unwrap())
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
    let mut c = source("Anticipate");
    let source::OracleCard::Card { rules, .. } = &mut c else {
        panic!()
    };
    *rules = Some(vec![ron::from_str(rule).unwrap()]);
    lower::card(&c)
}
fn body(c: &Card) -> &Program {
    c.abilities
        .iter()
        .find_map(|a| match a {
            Ability::Spell { body, .. }
            | Ability::Activated { body, .. }
            | Ability::Triggered { body, .. } => Some(body),
            _ => None,
        })
        .unwrap()
}
fn group(c: &Card) -> &Program {
    let Instruction::Group(p) = &body(c).instructions[0] else {
        panic!()
    };
    p
}
fn operations(c: &Card) -> Vec<&Operation> {
    fn program<'a>(p: &'a Program, out: &mut Vec<&'a Operation>) {
        for i in &p.instructions {
            match i {
                Instruction::Effect { operation, .. } => out.push(operation),
                Instruction::Group(p)
                | Instruction::Optional { body: p, .. }
                | Instruction::Repeatable(p)
                | Instruction::ForEachPlayer { body: p, .. } => program(p, out),
                Instruction::If {
                    then, otherwise, ..
                } => {
                    program(then, out);
                    program(otherwise, out);
                }
                Instruction::ChooseOne { options, .. } => {
                    options.iter().for_each(|option| program(option, out));
                }
            }
        }
    }
    fn ability<'a>(a: &'a Ability, out: &mut Vec<&'a Operation>) {
        match a {
            Ability::Spell { announcement, body }
            | Ability::Activated {
                announcement, body, ..
            }
            | Ability::Triggered {
                announcement, body, ..
            } => {
                program(body, out);
                if let Some(m) = &announcement.modes {
                    for m in &m.modes {
                        program(&m.body, out);
                    }
                }
            }
            Ability::Scoped { ability: a, .. } => ability(a, out),
            Ability::Conditional {
                then, otherwise, ..
            } => {
                for a in then.iter().chain(otherwise) {
                    ability(a, out)
                }
            }
            _ => {}
        }
    }
    let mut out = vec![];
    for a in &c.abilities {
        ability(a, &mut out);
    }
    out
}
fn origin(objects: &ObjectSet) -> Option<u32> {
    match objects {
        ObjectSet::Observed(r) => Some(r.0),
        ObjectSet::InZone { objects, .. }
        | ObjectSet::Matching { objects, .. }
        | ObjectSet::Difference { objects, .. } => origin(objects),
        _ => None,
    }
}
fn effect(operation: Operation, id: u32) -> Instruction {
    Instruction::Effect {
        result: ResultId(id),
        operation,
    }
}

#[test]
fn real_inspection_programs_roundtrip_and_unimplemented_nested_programs_remain_diagnostics() {
    let incomplete = [
        "Fact or Fiction",
        "Atris, Oracle of Half-Truths",
        "Fortune's Favor",
        "Velomachus Lorehold",
        "Svella, Ice Shaper",
        "Break Out",
        "Birthing Ritual",
        "Lim-Dûl's Vault",
        "Moonlight Bargain",
        "Hide in Plain Sight",
        "Orcish Librarian",
        "Planar Genesis",
        "Zimone's Experiment",
    ];
    let mut complete = 0;
    for s in sources() {
        let name = corpus::Identity::of(&s).unwrap().names.remove(0);
        if incomplete.contains(&name.as_str()) {
            assert!(lower::card(&s).is_err(), "{name}");
            continue;
        }
        let c = lower::card(&s).unwrap_or_else(|e| panic!("{name}: {e:?}"));
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
    assert_eq!(complete, 40);
}

#[test]
fn later_choices_and_the_rest_use_one_captured_cohort_and_actual_placement_results() {
    let c = card("Telling Time");
    let p = group(&c);
    assert!(matches!(
        &p.instructions[0],
        Instruction::Effect {
            result: ResultId(0),
            operation: Operation::LookAt {
                objects: ObjectSet::Take {
                    count: Expr::Constant(3),
                    ..
                },
                ..
            }
        }
    ));
    let Instruction::Effect {
        operation:
            Operation::ChooseObjects {
                objects: first,
                count: Count::AsManyAsCan(Expr::Constant(1)),
                ..
            },
        ..
    } = &p.instructions[1]
    else {
        panic!()
    };
    assert_eq!(origin(first), Some(0));
    let Instruction::Effect {
        result: moved,
        operation:
            Operation::Move {
                destination: Destination::OwnersHand,
                ..
            },
    } = &p.instructions[2]
    else {
        panic!()
    };
    let Instruction::Effect {
        operation: Operation::ChooseObjects {
            objects: second, ..
        },
        ..
    } = &p.instructions[3]
    else {
        panic!()
    };
    assert_eq!(origin(second), Some(0));
    assert!(
        matches!(second,ObjectSet::InZone {objects,..} if matches!(objects.as_ref(),ObjectSet::Difference {excluding,..} if matches!(excluding.as_ref(),ObjectSet::Union(sets) if matches!(sets.as_slice(),[ObjectSet::Transition {result,phase:ObjectPhase::Before}] if result.0==moved.0))))
    );
    let Instruction::Effect {
        result: reordered,
        operation:
            Operation::ReorderLibrary {
                end: SequenceEnd::Top,
                ..
            },
    } = &p.instructions[4]
    else {
        panic!()
    };
    let Instruction::Effect {
        operation:
            Operation::ReorderLibrary {
                objects: rest,
                end: SequenceEnd::Bottom,
                ..
            },
        ..
    } = &p.instructions[5]
    else {
        panic!()
    };
    assert_eq!(origin(rest), Some(0));
    assert!(
        matches!(rest,ObjectSet::InZone {objects,..} if matches!(objects.as_ref(),ObjectSet::Difference {excluding,..} if matches!(excluding.as_ref(),ObjectSet::Union(sets) if sets.iter().any(|s|matches!(s,ObjectSet::Transition {result,phase:ObjectPhase::Before} if result.0==reordered.0)))))
    );
}

#[test]
fn optional_filtered_placements_keep_revelation_legality_entry_flags_and_group_budgets() {
    let c = card("Commune with Nature");
    let ops = operations(&c);
    assert!(matches!(
        ops.as_slice(),
        [
            Operation::LookAt { .. },
            Operation::ChooseObjects {
                count: Count::UpTo(Expr::Constant(1)),
                objects: ObjectSet::Matching {
                    filter: ObjectFilter::CardType(source::CardType::Creature),
                    ..
                },
                ..
            },
            Operation::Reveal { .. },
            Operation::Move {
                destination: Destination::OwnersHand,
                ..
            },
            Operation::ReorderLibrary {
                order: CardOrder::Chosen,
                ..
            }
        ]
    ));
    let c = card("Elvish Rejuvenator");
    assert!(operations(&c).iter().any(|op|matches!(op,Operation::ChooseObjects {constraints,..} if matches!(constraints.as_slice(),[SelectionConstraint::CanMoveTo {destination:Destination::Battlefield(PlayerRef::AbilityController),entry:EntrySpec {tapped:true,..},face_down:false}]))));
    let c = card("Collected Company");
    assert!(operations(&c).iter().any(|op| matches!(
        op,
        Operation::ChooseObjects {
            count: Count::UpTo(Expr::Constant(2)),
            ..
        }
    )));
    let c = card("Ao, the Dawn Sky");
    assert!(operations(&c).iter().any(|op|matches!(op,Operation::ChooseObjects {constraints,..} if constraints.iter().any(|c|matches!(c,SelectionConstraint::TotalProperty {property:Property::ManaValue,comparison:Comparison::LessOrEqual,amount:Expr::Constant(4)})))));
}

#[test]
fn several_type_clauses_choose_distinct_cards_before_one_reveal_and_one_move() {
    let c = card("Gift of the Gargantuan");
    let ops = operations(&c);
    let [Operation::LookAt { .. }, Operation::ChooseObjects {
        count: Count::UpTo(Expr::Constant(1)),
        ..
    }, Operation::ChooseObjects { objects, .. }, Operation::Reveal { .. }, Operation::Move {
        objects: placed,
        destination: Destination::OwnersHand,
        ..
    }, Operation::ReorderLibrary { .. }] = ops.as_slice()
    else {
        panic!()
    };
    assert!(
        matches!(objects,ObjectSet::Matching {objects,..} if matches!(objects.as_ref(),ObjectSet::Difference {excluding,..} if matches!(excluding.as_ref(),ObjectSet::Union(sets) if matches!(sets.as_slice(),[ObjectSet::Selected(ResultId(1))]))))
    );
    assert!(
        matches!(placed,ObjectSet::Union(sets) if matches!(sets.as_slice(),[ObjectSet::Selected(ResultId(1)),ObjectSet::Selected(ResultId(2))]))
    );
}

#[test]
fn viewer_library_owner_and_ordering_player_remain_distinct_and_other_zones_share_look_at() {
    let c = card("Tahngarth's Glare");
    let ops = operations(&c);
    assert!(matches!(ops.as_slice(),[
        Operation::LookAt {player:PlayerRef::AbilityController,objects:ObjectSet::Take {objects:theirs,..}},
        Operation::ReorderLibrary {player:PlayerRef::AbilityController,library:PlayerRef::Target {slot:Slot(0),..},..},
        Operation::LookAt {player:PlayerRef::Target {slot:Slot(0),..},objects:ObjectSet::Take {objects:ours,..}},
        Operation::ReorderLibrary {player:PlayerRef::Target {slot:Slot(0),..},library:PlayerRef::AbilityController,..}
    ] if matches!(theirs.as_ref(),ObjectSet::Zone {scope:ZoneScope::Players {players:PlayerSet::One(PlayerRef::Target {slot:Slot(0),..}),zone:EventZone::Library},..}) && matches!(ours.as_ref(),ObjectSet::Zone {scope:ZoneScope::Players {players:PlayerSet::One(PlayerRef::AbilityController),zone:EventZone::Library},..})));
    let c=custom("SpellActions(Targeted([TargetPlayer(AnyPlayer)],ActionList([LookAtPlayersHand(Ref_TargetPlayer)])))").unwrap();
    validate::card(&c).unwrap();
    assert!(matches!(
        operations(&c).as_slice(),
        [Operation::LookAt {
            player: PlayerRef::AbilityController,
            objects: ObjectSet::Zone {
                scope: ZoneScope::Players {
                    zone: EventZone::Hand,
                    players: PlayerSet::One(PlayerRef::Target { slot: Slot(0), .. })
                },
                ..
            }
        }]
    ));
}

#[test]
fn hand_status_uses_actual_movements_and_conditional_remainders_do_not_leak_branch_results() {
    let c = card("Blossom Prancer");
    let p = body(&c);
    let Instruction::Group(inspect) = &p.instructions[0] else {
        panic!()
    };
    let moved = inspect
        .instructions
        .iter()
        .find_map(|i| {
            if let Instruction::Effect {
                result,
                operation:
                    Operation::Move {
                        destination: Destination::OwnersHand,
                        ..
                    },
            } = i
            {
                Some(result.0)
            } else {
                None
            }
        })
        .unwrap();
    assert!(
        matches!(&p.instructions[1],Instruction::If {condition:Condition::Not(condition),..} if matches!(condition.as_ref(),Condition::Compare {left:Expr::CountObjects(objects),comparison:Comparison::Greater,right:Expr::Constant(0)} if matches!(objects.as_ref(),ObjectSet::Matching {objects,..} if matches!(objects.as_ref(),ObjectSet::Union(sets) if matches!(sets.as_slice(),[ObjectSet::Transition {result,phase:ObjectPhase::Before}] if result.0==moved)))))
    );
    let c=custom("SpellActions(ActionList([LookAtTheTopNumberCardsOfLibrary(Integer(3),[If(IsPlayersTurn(You),[PutAGenericCardIntoHand]),PutTheRemainingCardsIntoGraveyard])]))").unwrap();
    validate::card(&c).unwrap();
    let Instruction::If {
        then, otherwise, ..
    } = &group(&c).instructions[1]
    else {
        panic!()
    };
    assert_eq!(then.instructions.len(), 3);
    assert_eq!(otherwise.instructions.len(), 1);
    assert!(custom("SpellActions(ActionList([LookAtTheTopNumberCardsOfLibrary(Integer(3),[If(IsPlayersTurn(You),[PutAGenericCardIntoHand]),PutTheRemainingCardsIntoGraveyard]),Unless(ACardWasPutIntoHandThisWay(AnyCard),[DrawACard])]))").is_err());
}

#[test]
fn observation_results_keep_zone_order_and_result_kind_invariants() {
    let mut c = card("Index");
    let Ability::Spell { body, .. } = &mut c.abilities[0] else {
        panic!()
    };
    let Instruction::Group(p) = &mut body.instructions[0] else {
        panic!()
    };
    p.instructions.push(effect(
        Operation::CreateRegenerationShields {
            objects: ObjectSet::Observed(ResultId(0)),
        },
        99,
    ));
    assert!(validate::card(&c).unwrap_err().contains("Battlefield"));
    let mut c = card("Index");
    let Ability::Spell { body, .. } = &mut c.abilities[0] else {
        panic!()
    };
    let Instruction::Group(p) = &mut body.instructions[0] else {
        panic!()
    };
    let Instruction::Effect {
        operation: Operation::ReorderLibrary { objects, .. },
        ..
    } = &mut p.instructions[1]
    else {
        panic!()
    };
    *objects = ObjectSet::Observed(ResultId(1));
    assert!(validate::card(&c).unwrap_err().contains("LookAt"));
    let mut c = card("Index");
    let Ability::Spell { body, .. } = &mut c.abilities[0] else {
        panic!()
    };
    let Instruction::Group(p) = &mut body.instructions[0] else {
        panic!()
    };
    let Instruction::Effect {
        operation: Operation::ReorderLibrary { objects, .. },
        ..
    } = &mut p.instructions[1]
    else {
        panic!()
    };
    *objects = ObjectSet::Transition {
        result: ResultId(0),
        phase: ObjectPhase::Before,
    };
    assert!(validate::card(&c).unwrap_err().contains("transition"));
    let mut c=custom("SpellActions(Targeted([TargetPlayer(AnyPlayer)],ActionList([LookAtPlayersHand(Ref_TargetPlayer)])))").unwrap();
    let Ability::Spell { body, .. } = &mut c.abilities[0] else {
        panic!()
    };
    body.instructions.push(effect(
        Operation::Reveal {
            objects: ObjectSet::One(ObjectRef::Query {
                objects: Box::new(ObjectSet::Observed(ResultId(0))),
                index: 0,
            }),
        },
        99,
    ));
    assert!(validate::card(&c).unwrap_err().contains("ordered"));
}

#[test]
fn looking_and_reordering_do_not_disqualify_mana_abilities_but_moving_inspected_cards_does() {
    let mana = effect(
        Operation::AddMana {
            players: PlayerSet::One(PlayerRef::AbilityController),
            mana: ManaProduction::Amount {
                color: ManaColor::Green,
                amount: Expr::Constant(1),
            },
            restriction: None,
        },
        99,
    );
    let classify = |c: Card| {
        let mut p = body(&c).clone();
        p.instructions.push(mana.clone());
        activation_kind(&Cost { parts: vec![] }, &Announcement::empty(), &p)
    };
    assert_eq!(classify(card("Index")), ActivationKind::Mana);
    assert_eq!(classify(card("Anticipate")), ActivationKind::Stack);
    let c=custom("SpellActions(ActionList([LookAtTheTopNumberCardsOfLibrary(Integer(2),[PutAGenericCardOnTopOfLibrary,PutTheRemainingCardsOnTopOfLibraryInAnyOrder])]))").unwrap();
    assert_eq!(classify(c), ActivationKind::Mana);
}
