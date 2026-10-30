use mtgish_ast::{corpus, ir::*, lower, mtg_types as source, semantics, validate};

fn sources() -> impl Iterator<Item = source::OracleCard> {
    include_str!("fixtures/zones.ronl")
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
    let mut c = source("Counterspell");
    let source::OracleCard::Card { rules, .. } = &mut c else {
        panic!()
    };
    *rules = Some(vec![ron::from_str(rule).unwrap()]);
    lower::card(&c)
}
fn spell(c: &Card) -> (&Announcement, &Program) {
    let Ability::Spell { announcement, body } = &c.abilities[0] else {
        panic!()
    };
    (announcement, body)
}
fn effect(operation: Operation, result: u32) -> Instruction {
    Instruction::Effect {
        result: ResultId(result),
        operation,
    }
}
fn movement(objects: ObjectSet, destination: Destination) -> Operation {
    Operation::Move {
        objects,
        destination,
        entry: EntrySpec::default(),
        face_down: false,
    }
}
fn library(players: PlayerSet) -> ObjectSet {
    ObjectSet::Zone {
        scope: ZoneScope::Players {
            zone: EventZone::Library,
            players,
        },
        kind: ObjectKind::Card,
        filter: ObjectFilter::Any,
    }
}

#[test]
fn real_zone_families_roundtrip_and_missing_nested_semantics_remain_incomplete() {
    let incomplete = ["Exhume", "Extract from Darkness", "Food Chain", "Hinder"];
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
    assert_eq!(complete, 53);
}

#[test]
fn mixed_targets_keep_their_domains_and_source_ordinals() {
    let c = custom("SpellActions(Targeted([TargetPermanent(AnyPermanent), TargetGraveyardCard(AnyCardInAnyGraveyard), TargetSpell(AnySpell)], ActionList([Exile([Permanent(Ref_TargetPermanent)], []), PutGraveyardCardIntoHand(Ref_TargetGraveyardCard), CounterSpell(Ref_TargetSpell)])))").unwrap();
    validate::card(&c).unwrap();
    let (a, p) = spell(&c);
    assert_eq!(
        a.targets.iter().map(|t| t.kind.clone()).collect::<Vec<_>>(),
        vec![
            TargetKind::Object(ObjectDomain::permanent()),
            TargetKind::Object(ObjectDomain::card(EventZone::Graveyard)),
            TargetKind::Object(ObjectDomain::spell())
        ]
    );
    for (index, instruction) in p.instructions.iter().enumerate() {
        let Instruction::Effect {
            operation: Operation::Move { objects, .. } | Operation::Counter { objects, .. },
            ..
        } = instruction
        else {
            panic!()
        };
        assert!(
            matches!(objects, ObjectSet::One(ObjectRef::Target { slot, .. }) if usize::from(slot.0) == index)
        );
    }
    let mut invalid = c.clone();
    let Ability::Spell { body, .. } = &mut invalid.abilities[0] else {
        panic!()
    };
    body.instructions[0] = effect(
        Operation::Destroy {
            objects: ObjectSet::One(ObjectRef::Target {
                slot: Slot(1),
                index: 0,
            }),
            regeneration: true,
        },
        0,
    );
    assert!(validate::card(&invalid)
        .unwrap_err()
        .contains("Battlefield"));
    body_for(&mut invalid).instructions[0] = effect(
        Operation::Counter {
            objects: ObjectSet::One(ObjectRef::Target {
                slot: Slot(0),
                index: 0,
            }),
            destination: Destination::OwnersGraveyard,
        },
        0,
    );
    assert!(validate::card(&invalid).unwrap_err().contains("Stack"));
}
fn body_for(c: &mut Card) -> &mut Program {
    let Ability::Spell { body, .. } = &mut c.abilities[0] else {
        panic!()
    };
    body
}

#[test]
fn countering_is_distinct_from_moving_and_followups_are_unconditional() {
    for (name, destination) in [
        ("Counterspell", Destination::OwnersGraveyard),
        ("Dissipate", Destination::Exile),
        ("Remand", Destination::OwnersHand),
        ("Memory Lapse", Destination::OwnersLibraryTop),
    ] {
        let c = card(name);
        let (a, p) = spell(&c);
        assert_eq!(a.targets[0].kind, TargetKind::Object(ObjectDomain::spell()));
        let Instruction::Effect {
            operation:
                Operation::Counter {
                    destination: actual,
                    ..
                },
            ..
        } = &p.instructions[0]
        else {
            panic!()
        };
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::to_value(destination).unwrap()
        );
    }
    for name in ["Absorb", "Spell Swindle", "Access Denied"] {
        let c = card(name);
        assert!(matches!(
            spell(&c).1.instructions.as_slice(),
            [
                Instruction::Effect {
                    operation: Operation::Counter { .. },
                    ..
                },
                Instruction::Effect { .. }
            ]
        ));
    }
    let c = card("Spell Swindle");
    assert!(
        matches!(&spell(&c).1.instructions[1], Instruction::Effect { operation: Operation::CreateTokens { tokens, .. }, .. } if matches!(&tokens[0].amount, Expr::ObjectProperty { information: Information::CurrentOrLastKnown, object: ObjectRef::Target { slot: Slot(0), .. }, property: Property::ManaValue }))
    );
    let c = card("Swords to Plowshares");
    assert!(
        matches!(&spell(&c).1.instructions[1], Instruction::Effect { operation: Operation::GainLife { players: PlayerSet::One(PlayerRef::ControllerOf(object)), amount: Expr::ObjectProperty { information: Information::CurrentOrLastKnown, .. } }, .. } if matches!(object.as_ref(), ObjectRef::Target { slot: Slot(0), .. }))
    );
}

#[test]
fn graveyard_returns_keep_entry_properties_and_new_object_bindings() {
    let c = card("Reanimate");
    assert!(matches!(
        spell(&c).1.instructions.as_slice(),
        [
            Instruction::Effect {
                operation: Operation::Move {
                    destination: Destination::Battlefield(PlayerRef::AbilityController),
                    ..
                },
                ..
            },
            Instruction::Effect {
                operation: Operation::LoseLife {
                    amount: Expr::ObjectProperty {
                        information: Information::CurrentOrLastKnown,
                        ..
                    },
                    ..
                },
                ..
            }
        ]
    ));
    let c = card("Victimize");
    assert!(
        matches!(&spell(&c).1.instructions[1], Instruction::If { condition: Condition::PaymentChosen(_), then, .. } if matches!(then.instructions.as_slice(), [Instruction::Effect { operation: Operation::Move { entry: EntrySpec { tapped: true, .. }, objects: ObjectSet::Targets(_), .. }, .. }]))
    );
    let c = custom("SpellActions(Targeted([TargetGraveyardCard(AnyCardInAnyGraveyard)], ActionList([PutGraveyardCardOntoBattlefield(Ref_TargetGraveyardCard, [EntersTapped, EntersUnderOwnersControl]), TapPermanent(ThePermanentPutOnTheBattlefieldThisWay)])))").unwrap();
    validate::card(&c).unwrap();
    assert!(matches!(
        spell(&c).1.instructions.as_slice(),
        [
            Instruction::Effect {
                result: ResultId(0),
                operation: Operation::Move {
                    destination: Destination::OwnersBattlefield,
                    entry: EntrySpec { tapped: true, .. },
                    ..
                }
            },
            Instruction::Effect {
                operation: Operation::Tap {
                    objects: ObjectSet::One(ObjectRef::Result {
                        result: ResultId(0),
                        phase: ObjectPhase::After,
                        ..
                    }),
                    ..
                },
                ..
            }
        ]
    ));
}

#[test]
fn exile_choices_precede_one_batch_and_transition_bindings_are_scoped() {
    let c = custom("SpellActions(ActionList([Exile([APermanent(AnyPermanent), APermanent(IsCardtype(Creature))], []), GainLife(NumPermanentsExiledThisWay)]))").unwrap();
    validate::card(&c).unwrap();
    let Instruction::Group(group) = &spell(&c).1.instructions[0] else {
        panic!()
    };
    assert!(
        matches!(group.instructions.as_slice(), [Instruction::Effect { operation: Operation::ChooseObjects { count: Count::AsManyAsCan(Expr::Constant(1)), .. }, .. }, Instruction::Effect { operation: Operation::ChooseObjects { .. }, .. }, Instruction::Effect { result: ResultId(2), operation: Operation::Move { objects: ObjectSet::Union(sets), destination: Destination::Exile, .. } }] if sets.len() == 2)
    );
    assert!(
        matches!(&spell(&c).1.instructions[1], Instruction::Effect { operation: Operation::GainLife { amount: Expr::CountObjects(objects), .. }, .. } if matches!(objects.as_ref(), ObjectSet::Transition { result: ResultId(2), phase: ObjectPhase::Before }))
    );
    let mut invalid = c.clone();
    let Instruction::Effect {
        operation:
            Operation::GainLife {
                amount: Expr::CountObjects(objects),
                ..
            },
        ..
    } = &mut body_for(&mut invalid).instructions[1]
    else {
        panic!()
    };
    *objects = Box::new(ObjectSet::Transition {
        result: ResultId(0),
        phase: ObjectPhase::After,
    });
    assert!(validate::card(&invalid)
        .unwrap_err()
        .contains("object-transition"));
    assert!(custom("SpellActions(ActionList([MayAction(Exile([Permanent(ThisPermanent)], [])), GainLife(NumPermanentsExiledThisWay)]))").is_err());
    assert!(custom("SpellActions(ActionList([Exile([EachCardInGraveyards(AnyCardInAnyGraveyard)], []), GainLife(NumPermanentsExiledThisWay)]))").is_err());
    assert!(custom(
        "SpellActions(ActionList([Exile([Permanent(ThisPermanent)], [Limited(UntilItIsNoLongerEnchanted)])]))"
    )
    .is_err());
}

#[test]
fn ordered_queries_require_a_single_private_zone_and_shared_zones_keep_their_scope() {
    let c = card("Phyrexian Furnace");
    let Ability::Activated { body, .. } = &c.abilities[0] else {
        panic!()
    };
    assert!(
        matches!(&body.instructions[0], Instruction::Effect { operation: Operation::Move { objects: ObjectSet::One(ObjectRef::Query { objects, .. }), .. }, .. } if matches!(objects.as_ref(), ObjectSet::Take { from: SequenceEnd::Bottom, .. }))
    );
    let mut c = card("Counterspell");
    let objects = ObjectSet::Take {
        objects: Box::new(library(PlayerSet::One(PlayerRef::AbilityController))),
        count: Expr::Constant(3),
        from: SequenceEnd::Top,
    };
    body_for(&mut c).instructions = vec![effect(movement(objects, Destination::Exile), 0)];
    validate::card(&c).unwrap();
    let Instruction::Effect {
        operation:
            Operation::Move {
                objects: ObjectSet::Take { objects, .. },
                ..
            },
        ..
    } = &mut body_for(&mut c).instructions[0]
    else {
        panic!()
    };
    *objects = Box::new(library(PlayerSet::All));
    assert!(validate::card(&c).unwrap_err().contains("one ordered zone"));
    body_for(&mut c).instructions = vec![effect(
        movement(
            ObjectSet::Zone {
                scope: ZoneScope::Shared(EventZone::Graveyard),
                kind: ObjectKind::Card,
                filter: ObjectFilter::Any,
            },
            Destination::Exile,
        ),
        0,
    )];
    assert!(validate::card(&c).unwrap_err().contains("wrong scope"));
    body_for(&mut c).instructions = vec![effect(
        movement(
            ObjectSet::Zone {
                scope: ZoneScope::Shared(EventZone::Exile),
                kind: ObjectKind::Card,
                filter: ObjectFilter::Any,
            },
            Destination::OwnersHand,
        ),
        0,
    )];
    validate::card(&c).unwrap();
}

#[test]
fn library_movement_classification_follows_queries_selections_and_transition_phases() {
    use semantics::{activation_kind, ActivationKind};
    let cost = Cost { parts: vec![] };
    let announcement = Announcement {
        targets: vec![],
        modes: None,
    };
    let mana = effect(
        Operation::AddMana {
            players: PlayerSet::One(PlayerRef::AbilityController),
            mana: ManaProduction::Amount {
                color: ManaColor::Green,
                amount: Expr::Constant(1),
            },
            restriction: None,
        },
        9,
    );
    let query = library(PlayerSet::One(PlayerRef::AbilityController));
    let choose = effect(
        Operation::ChooseObjects {
            constraints: vec![],
            player: PlayerRef::AbilityController,
            objects: query.clone(),
            count: Count::UpTo(Expr::Constant(1)),
        },
        0,
    );
    let kind = |mut instructions: Vec<Instruction>| {
        instructions.push(mana.clone());
        activation_kind(&cost, &announcement, &Program { instructions })
    };
    assert_eq!(kind(vec![choose.clone()]), ActivationKind::Mana);
    assert_eq!(
        kind(vec![effect(movement(query, Destination::Exile), 0)]),
        ActivationKind::Stack
    );
    assert_eq!(
        kind(vec![
            choose,
            effect(
                movement(ObjectSet::Selected(ResultId(0)), Destination::Exile),
                1
            )
        ]),
        ActivationKind::Stack
    );
    assert_eq!(
        kind(vec![effect(
            Operation::GainLife {
                players: PlayerSet::One(PlayerRef::AbilityController),
                amount: Expr::CountObjects(Box::new(library(PlayerSet::All)))
            },
            0
        )]),
        ActivationKind::Mana
    );
    assert_eq!(
        kind(vec![
            effect(
                movement(
                    ObjectSet::One(ObjectRef::Source),
                    Destination::OwnersLibraryTop
                ),
                0
            ),
            effect(
                movement(
                    ObjectSet::Transition {
                        result: ResultId(0),
                        phase: ObjectPhase::After
                    },
                    Destination::Exile
                ),
                1
            )
        ]),
        ActivationKind::Stack
    );
}

#[test]
fn a_graveyard_card_from_an_event_stays_in_the_graveyard() {
    let json = serde_json::to_string(&card("Necropotence")).unwrap();
    assert!(
        json.contains(r#""objects":{"kind":"One","args":{"kind":"Discarded"}},"zone":"Graveyard""#),
        "{json}"
    );
}
