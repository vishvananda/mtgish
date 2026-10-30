use mtgish_ast::{corpus, ir::*, lower, mtg_types as source, semantics, validate};

fn sources() -> impl Iterator<Item = source::OracleCard> {
    include_str!("fixtures/delayed.ronl")
        .lines()
        .map(|s| ron::from_str(s).unwrap())
}
fn source(name: &str) -> source::OracleCard {
    sources()
        .find(|c| {
            corpus::Identity::of(c)
                .unwrap()
                .names
                .contains(&name.into())
        })
        .unwrap()
}
fn card(name: &str) -> Card {
    let c = lower::card(&source(name)).unwrap_or_else(|e| panic!("{name}: {e:?}"));
    validate::card(&c).unwrap();
    c
}
fn program(c: &Card) -> &Program {
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
fn delayed(p: &Program) -> &Operation {
    for i in &p.instructions {
        match i {
            Instruction::Effect {
                operation: op @ Operation::CreateDelayedTrigger { .. },
                ..
            } => return op,
            Instruction::If { then, .. } => return delayed(then),
            Instruction::Group(p) | Instruction::Optional { body: p, .. } => return delayed(p),
            _ => {}
        }
    }
    panic!("no delayed trigger")
}
fn spell(body: Program) -> Card {
    let mut c = card("Cloudshift");
    c.abilities = vec![Ability::Spell {
        announcement: Announcement {
            targets: vec![],
            modes: None,
        },
        body,
    }];
    c
}
fn scheduled(body: Program) -> Instruction {
    Instruction::Effect {
        result: ResultId(10),
        operation: Operation::CreateDelayedTrigger {
            event: EventSpec::Turn {
                point: TurnPoint::EndStepBegins,
                players: PlayerSet::All,
            },
            start: DelayedStart::Immediately,
            expires: None,
            intervening_if: None,
            body,
        },
    }
}

fn custom(rule: &str) -> Card {
    let mut s = source("Cloudshift");
    let source::OracleCard::Card { rules, .. } = &mut s else {
        panic!()
    };
    *rules = Some(vec![ron::from_str(rule).unwrap()]);
    let c = lower::card(&s).unwrap_or_else(|e| panic!("{e:?}"));
    validate::card(&c).unwrap();
    c
}

#[test]
fn bounded_schedules_and_intervening_checks_remain_distinct() {
    for (trigger, expected) in [
        ("AtNextEndOfCombatThisTurn", "EndOfTurn"),
        ("AtTheEndOfThisCombat", "EndOfCombat"),
        ("AtTheBeginningOfTheNextCombatPhaseThisTurn", "EndOfTurn"),
        ("AtTheBeginningOfTheNextMainPhaseThisTurn", "EndOfTurn"),
    ] {
        let c = custom(&format!(
            "SpellActions(ActionList([CreateFutureTrigger({trigger}, ActionList([DrawACard]))]))"
        ));
        let Operation::CreateDelayedTrigger {
            expires: Some(expires),
            ..
        } = delayed(program(&c))
        else {
            panic!()
        };
        assert_eq!(serde_json::to_value(expires).unwrap(), expected);
    }
    let c=custom("SpellActions(Targeted([TargetPermanent(AnyPermanent)], ActionList([Exile([Permanent(Ref_TargetPermanent)], []), CreateFutureTriggerI(AtTheBeginningOfTheNextEndStep, TheCardExiledThisWayIsStillExiled, ActionList([PutExiledCardIntoOwnersHand(TheCardExiledThisWay)]))])))");
    let Operation::CreateDelayedTrigger {
        intervening_if:
            Some(Condition::Compare {
                left: Expr::CountObjects(objects),
                ..
            }),
        ..
    } = delayed(program(&c))
    else {
        panic!()
    };
    assert!(matches!(
        objects.as_ref(),
        ObjectSet::InZone {
            zone: EventZone::Exile,
            ..
        }
    ));
}

#[test]
fn turn_schedules_preserve_creation_relative_timing_and_expiration() {
    for (name, point) in [
        ("Waylay", "CleanupStepBegins"),
        ("Clockwork Beetle", "CombatEnds"),
        ("Flickerwisp", "EndStepBegins"),
        ("Mana Drain", "MainPhaseBegins"),
    ] {
        let c = card(name);
        let Operation::CreateDelayedTrigger {
            event: EventSpec::Turn { point: p, .. },
            expires,
            ..
        } = delayed(program(&c))
        else {
            panic!()
        };
        assert_eq!(serde_json::to_value(p).unwrap(), point);
        assert!(expires.is_none());
    }
    let c = card("Mishra's Bauble");
    assert!(matches!(
        delayed(program(&c)),
        Operation::CreateDelayedTrigger {
            start: DelayedStart::NextTurn,
            ..
        }
    ));
    let c = card("Suppress");
    assert!(matches!(
        delayed(program(&c)),
        Operation::CreateDelayedTrigger {
            start: DelayedStart::PlayersNextTurn(PlayerRef::Target { .. }),
            ..
        }
    ));
}

#[test]
fn returned_objects_use_exact_captured_post_exile_incarnations() {
    let c = card("Flickerwisp");
    let p = program(&c);
    let Instruction::Effect { result: exile, .. } = &p.instructions[0] else {
        panic!()
    };
    let Operation::CreateDelayedTrigger { body, .. } = delayed(p) else {
        panic!()
    };
    let Instruction::Effect {
        operation:
            Operation::Move {
                objects:
                    ObjectSet::InZone {
                        objects,
                        zone: EventZone::Exile,
                    },
                destination: Destination::OwnersBattlefield,
                ..
            },
        ..
    } = &body.instructions[0]
    else {
        panic!()
    };
    assert!(
        matches!(objects.as_ref(),ObjectSet::One(ObjectRef::Result {result,phase:ObjectPhase::After,index:0}) if result.0==exile.0)
    );
    let c = card("Ghostway");
    let Operation::CreateDelayedTrigger { body, .. } = delayed(program(&c)) else {
        panic!()
    };
    assert!(
        matches!(&body.instructions[0],Instruction::Effect {operation:Operation::Move {objects:ObjectSet::InZone {objects,..},..},..} if matches!(objects.as_ref(),ObjectSet::Transition {phase:ObjectPhase::After,..}))
    );
    // Immediate and delayed returns share Move; owner and controller differ.
    let c = card("Cloudshift");
    assert!(matches!(
        &program(&c).instructions[1],
        Instruction::Effect {
            operation: Operation::Move {
                destination: Destination::Battlefield(PlayerRef::AbilityController),
                ..
            },
            ..
        }
    ));
}

#[test]
fn creator_event_bindings_survive_without_becoming_future_turn_bindings() {
    let c = card("Clockwork Beetle");
    let Operation::CreateDelayedTrigger { body, .. } = delayed(program(&c)) else {
        panic!()
    };
    let value = serde_json::to_value(body).unwrap().to_string();
    assert!(value.contains("EventSubject"));
    let c = card("Horn of Plenty");
    let Operation::CreateDelayedTrigger { body, .. } = delayed(program(&c)) else {
        panic!()
    };
    assert!(matches!(
        &body.instructions[0],
        Instruction::Effect {
            operation: Operation::Draw {
                players: PlayerSet::One(PlayerRef::EventSubject),
                ..
            },
            ..
        }
    ));
    let bad = spell(Program {
        instructions: vec![scheduled(Program {
            instructions: vec![Instruction::Effect {
                result: ResultId(0),
                operation: Operation::Draw {
                    players: PlayerSet::One(PlayerRef::EventSubject),
                    amount: Expr::Constant(1),
                },
            }],
        })],
    });
    assert!(
        validate::card(&bad).is_err(),
        "a future turn must not invent a missing creator player binding"
    );
}

#[test]
fn captured_target_properties_remain_resolution_time_expressions() {
    let c = card("Mana Drain");
    let Operation::CreateDelayedTrigger { body, .. } = delayed(program(&c)) else {
        panic!()
    };
    let value = serde_json::to_value(body).unwrap().to_string();
    assert!(
        value.contains("ObjectProperty")
            && value.contains("ManaValue")
            && value.contains("CurrentOrLastKnown")
            && value.contains("Target")
    );
    let c = card("Otherworldly Journey");
    let Operation::CreateDelayedTrigger { body, .. } = delayed(program(&c)) else {
        panic!()
    };
    assert!(
        matches!(&body.instructions[0],Instruction::Effect {operation:Operation::Move {entry:EntrySpec {counters,..},..},..} if counters.len()==1),
        "return-with-counter retains the entry counter, rather than adding it after entry"
    );
}

#[test]
fn deferred_results_do_not_escape_and_new_results_do_not_overwrite_creator_results() {
    let c = card("Waylay");
    let p = program(&c);
    let Instruction::Effect {
        result: created, ..
    } = &p.instructions[0]
    else {
        panic!()
    };
    let Operation::CreateDelayedTrigger { body, .. } = delayed(p) else {
        panic!()
    };
    assert!(serde_json::to_value(body)
        .unwrap()
        .to_string()
        .contains("Created"));
    let mut c = c.clone();
    let Ability::Spell { body, .. } = &mut c.abilities[0] else {
        panic!()
    };
    body.instructions.push(Instruction::Effect {
        result: ResultId(99),
        operation: Operation::Reveal {
            objects: ObjectSet::Transition {
                result: ResultId(created.0 + 1),
                phase: ObjectPhase::After,
            },
        },
    });
    assert!(
        validate::card(&c).is_err(),
        "the creator cannot read the deferred movement's result"
    );
}

#[test]
fn scheduling_mana_is_not_immediate_mana_and_future_library_work_is_not_immediate_movement() {
    let future_mana = Program {
        instructions: vec![Instruction::Effect {
            result: ResultId(0),
            operation: Operation::AddMana {
                players: PlayerSet::One(PlayerRef::AbilityController),
                mana: ManaProduction::Amount {
                    color: ManaColor::Blue,
                    amount: Expr::Constant(1),
                },
                restriction: None,
            },
        }],
    };
    let c = spell(Program {
        instructions: vec![scheduled(future_mana)],
    });
    let p = program(&c);
    assert_eq!(
        semantics::activation_kind(
            &Cost { parts: vec![] },
            &Announcement {
                targets: vec![],
                modes: None
            },
            p
        ),
        semantics::ActivationKind::Stack
    );
    let mut p = p.clone();
    p.instructions.insert(
        0,
        Instruction::Effect {
            result: ResultId(1),
            operation: Operation::AddMana {
                players: PlayerSet::One(PlayerRef::AbilityController),
                mana: ManaProduction::Amount {
                    color: ManaColor::Blue,
                    amount: Expr::Constant(1),
                },
                restriction: None,
            },
        },
    );
    let Instruction::Effect {
        operation: Operation::CreateDelayedTrigger { body, .. },
        ..
    } = &mut p.instructions[1]
    else {
        panic!()
    };
    body.instructions = vec![Instruction::Effect {
        result: ResultId(0),
        operation: Operation::Draw {
            players: PlayerSet::One(PlayerRef::AbilityController),
            amount: Expr::Constant(1),
        },
    }];
    assert_eq!(
        semantics::activation_kind(
            &Cost { parts: vec![] },
            &Announcement {
                targets: vec![],
                modes: None
            },
            &p
        ),
        semantics::ActivationKind::Mana
    );
}

#[test]
fn deferred_program_validation_retains_scope_and_rejects_unrepresented_future_event_bindings() {
    let mut c = spell(Program {
        instructions: vec![scheduled(Program::empty())],
    });
    let Ability::Spell { body, .. } = &mut c.abilities[0] else {
        panic!()
    };
    let Instruction::Effect {
        operation: Operation::CreateDelayedTrigger { event, .. },
        ..
    } = &mut body.instructions[0]
    else {
        panic!()
    };
    *event = EventSpec::Object {
        event: ObjectEvent::BecomesTapped,
        objects: ObjectFilter::Any,
    };
    assert!(validate::card(&c)
        .unwrap_err()
        .contains("separate captured/event scope"));
}

#[test]
fn delayed_families_roundtrip_without_dropping_other_card_gaps() {
    let incomplete = [
        "Abomination",
        "Alchemist's Gambit",
        "Aleatory",
        "Alora, Cheerful Assassin",
        "Arcane Denial",
        "Armor of Thorns",
        "Astral Drift",
        "Astral Slide",
        "Bounty of the Hunt",
        "Chancellor of the Tangle",
        "Come Back Wrong",
        "Dazzling Beauty",
        "Faramir, Prince of Ithilien",
        "Feldon of the Third Path",
        "Feral Encounter",
        "Fortune, Loyal Steed",
        "Gaze of the Gorgon",
        "Grinning Totem",
        "Heat Shimmer",
        "Kavaron Harrier",
        "Kiki-Jiki, Mirror Breaker",
        "Meandering Towershell",
        "Pact of Negation",
        "Pygmy Hippo",
        "Scattering Stroke",
        "Slaughter Pact",
        "Splinter Twin",
        "Summoner's Pact",
        "Tatsumasa, the Dragon's Fang",
        "Thassa, Deep-Dwelling",
        "Urza's Bauble",
        "Vivien's Stampede",
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
    assert_eq!(complete, 21);
}
