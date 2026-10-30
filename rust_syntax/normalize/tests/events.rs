use mtgish_ast::{corpus, ir::*, lower, mtg_types as source, semantics::event_bindings, validate};

fn sources() -> impl Iterator<Item = source::OracleCard> {
    include_str!("fixtures/events.ronl")
        .lines()
        .map(|line| ron::from_str(line).unwrap())
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
    let card = lower::card(&source(name)).unwrap_or_else(|e| panic!("{name}: {e:?}"));
    validate::card(&card).unwrap();
    card
}
fn trigger(card: &Card) -> &Ability {
    card.abilities
        .iter()
        .find(|a| matches!(a, Ability::Triggered { .. }))
        .unwrap()
}

#[test]
fn real_event_families_roundtrip_without_losing_nested_behavior() {
    let mut complete = 0;
    for source in sources() {
        let name = &corpus::Identity::of(&source).unwrap().names[0];
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
    assert_eq!(complete, 33);
}

#[test]
fn turn_events_bind_the_turn_player_and_keep_intervening_conditions() {
    let c = card("Dictate of Kruphix");
    let Ability::Triggered { event, body, .. } = trigger(&c) else {
        panic!()
    };
    assert!(matches!(
        event,
        EventSpec::Turn {
            point: TurnPoint::DrawStepBegins,
            players: PlayerSet::All
        }
    ));
    assert!(matches!(
        body.instructions.as_slice(),
        [Instruction::Effect {
            operation: Operation::Draw {
                players: PlayerSet::One(PlayerRef::EventSubject),
                amount: Expr::Constant(1)
            },
            ..
        }]
    ));
    let c = card("Howling Mine");
    assert!(
        matches!(trigger(&c), Ability::Triggered { intervening_if: Some(Condition::ObjectMatches {
        object: ObjectRef::Source, filter: ObjectFilter::Not(filter),
    }), .. } if matches!(**filter, ObjectFilter::Tapped))
    );

    let c = card("Furnace Punisher");
    let Ability::Triggered { body, .. } = trigger(&c) else {
        panic!()
    };
    assert!(
        matches!(body.instructions.as_slice(), [Instruction::If { condition: Condition::Not(_), then, otherwise }]
        if otherwise.instructions.is_empty() && matches!(then.instructions.as_slice(), [Instruction::Effect { operation: Operation::DealDamage {
            recipients: Recipients::Players(PlayerSet::One(PlayerRef::EventSubject)), ..
        }, .. }]))
    );
}

#[test]
fn alternative_events_keep_one_ability_and_casting_is_distinct_from_copying() {
    let c = card("Archmage Emeritus");
    let [Ability::Triggered {
        event: EventSpec::Either(events),
        body,
        ..
    }] = c.abilities.as_slice()
    else {
        panic!()
    };
    assert!(matches!(
        events.as_slice(),
        [
            EventSpec::Spell {
                event: SpellEvent::Cast,
                ..
            },
            EventSpec::Spell {
                event: SpellEvent::Copy,
                ..
            }
        ]
    ));
    assert!(event_bindings(&EventSpec::Either(events.clone())).spell);
    assert_eq!(body.instructions.len(), 1);

    let c = card("Aang and Katara");
    let [Ability::Triggered {
        event: EventSpec::Either(events),
        body,
        ..
    }] = c.abilities.as_slice()
    else {
        panic!()
    };
    assert_eq!(events.len(), 2);
    assert!(
        matches!(body.instructions.as_slice(), [Instruction::Effect { operation: Operation::CreateTokens { tokens, .. }, .. }]
        if matches!(&tokens[0].amount, Expr::CountObjects(_)))
    );
}

#[test]
fn event_only_filters_are_not_promoted_to_intervening_if() {
    let c = card("Brazen Blademaster");
    let Ability::Triggered {
        event,
        intervening_if,
        body,
        ..
    } = trigger(&c)
    else {
        panic!()
    };
    assert!(
        matches!(event, EventSpec::Filtered { event, condition: Condition::Compare { comparison: Comparison::GreaterOrEqual, right: Expr::Constant(2), .. } }
        if matches!(**event, EventSpec::Object { event: ObjectEvent::Attacks, .. }))
    );
    assert!(intervening_if.is_none());
    assert!(matches!(
        body.instructions.as_slice(),
        [Instruction::Effect {
            operation: Operation::CreateContinuousEffect {
                affected: EffectSubjects::Objects(ObjectSet::One(ObjectRef::EventSubject)),
                ..
            },
            ..
        }]
    ));
}

#[test]
fn references_require_bindings_in_every_alternative_and_nested_abilities_start_fresh() {
    let mut c = card("Dictate of Kruphix");
    let Ability::Triggered { event, .. } = &mut c.abilities[1] else {
        panic!()
    };
    *event = EventSpec::Either(vec![
        event.clone(),
        EventSpec::Object {
            event: ObjectEvent::Attacks,
            objects: ObjectFilter::Any,
        },
    ]);
    assert!(validate::card(&c)
        .unwrap_err()
        .contains("does not bind a player"));

    let mut c = card("Up the Beanstalk");
    let Ability::Triggered { event, body, .. } = &mut c.abilities[0] else {
        panic!()
    };
    let bindings = event_bindings(event);
    assert!(bindings.object);
    assert!(!bindings.player && !bindings.spell && !bindings.permanent && !bindings.amount);
    body.instructions = vec![Instruction::Effect {
        result: ResultId(0),
        operation: Operation::Draw {
            players: PlayerSet::One(PlayerRef::EventSubject),
            amount: Expr::Constant(1),
        },
    }];
    assert!(validate::card(&c)
        .unwrap_err()
        .contains("does not bind a player"));

    let mut c = card("Borderland Marauder");
    let Ability::Triggered { body, .. } = &mut c.abilities[0] else {
        panic!()
    };
    body.instructions = vec![Instruction::Effect {
        result: ResultId(0),
        operation: Operation::CreateContinuousEffect {
            source: ObjectRef::Source,
            affected: EffectSubjects::Objects(ObjectSet::One(ObjectRef::Source)),
            changes: vec![Change::GrantAbility(Box::new(Ability::Activated {
                origin: ActivationOrigin::Ordinary,
                cost: Cost { parts: vec![] },
                limits: ActivationLimits::unrestricted(),
                announcement: Announcement::empty(),
                body: Program {
                    instructions: vec![Instruction::Effect {
                        result: ResultId(0),
                        operation: Operation::Destroy {
                            objects: ObjectSet::One(ObjectRef::EventSubject),
                            regeneration: true,
                        },
                    }],
                },
            }))],
            duration: Duration::EndOfTurn,
            evaluation: EffectEvaluation::Snapshot,
        },
    }];
    assert!(validate::card(&c).unwrap_err().contains("event subject"));
}

#[test]
fn modal_programs_inherit_event_bindings_and_event_values_do_not_leak_to_other_abilities() {
    let mut c = source("Borderland Marauder");
    let source::OracleCard::Card {
        rules: Some(rules), ..
    } = &mut c
    else {
        panic!()
    };
    let source::Rule::TriggerA(_, actions) = &mut rules[0] else {
        panic!()
    };
    **actions = source::Actions::Modal_ChooseOne(vec![
        source::Actions::ActionList(vec![source::Action::DestroyPermanent(Box::new(
            source::Permanent::Trigger_ThatCreature,
        ))]),
        source::Actions::ActionList(vec![source::Action::UntapPermanent(Box::new(
            source::Permanent::Trigger_ThatCreature,
        ))]),
    ]);
    let normalized = lower::card(&c).unwrap();
    validate::card(&normalized).unwrap();
    let Ability::Triggered {
        announcement, body, ..
    } = trigger(&normalized)
    else {
        panic!()
    };
    assert!(body.instructions.is_empty());
    assert_eq!(announcement.modes.as_ref().unwrap().modes.len(), 2);

    let source::OracleCard::Card {
        rules: Some(rules), ..
    } = &mut c
    else {
        panic!()
    };
    rules.push(source::Rule::Activated(
        Box::new(source::Cost::PayMana(vec![])),
        Box::new(source::Actions::ActionList(vec![
            source::Action::DestroyPermanent(Box::new(source::Permanent::Trigger_ThatCreature)),
        ])),
    ));
    assert!(lower::card(&c).is_err());
}

#[test]
fn departures_keep_graveyard_and_leaves_battlefield_semantics_distinct() {
    let c = card("Thragtusk");
    assert!(matches!(
        &c.abilities[1],
        Ability::Triggered {
            event: EventSpec::Object {
                event: ObjectEvent::LeavesBattlefield,
                ..
            },
            ..
        }
    ));
    let c = card("Hatching Plans");
    assert!(matches!(
        trigger(&c),
        Ability::Triggered {
            event: EventSpec::ZoneChange {
                from: Some(EventZone::Battlefield),
                to: EventZone::Graveyard,
                ..
            },
            ..
        }
    ));
}
