use mtgish_ast::{corpus, ir::*, lower, mtg_types as source, semantics::event_bindings, validate};

fn sources() -> impl Iterator<Item = source::OracleCard> {
    include_str!("fixtures/damage.ronl")
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
    let mut c = source("Coastal Piracy");
    let source::OracleCard::Card { rules, .. } = &mut c else {
        panic!()
    };
    *rules = Some(vec![ron::from_str(rule).unwrap()]);
    lower::card(&c)
}
fn trigger(c: &Card) -> (&EventSpec, &Announcement, &Program) {
    c.abilities
        .iter()
        .find_map(|a| match a {
            Ability::Triggered {
                event,
                announcement,
                body,
                ..
            } => Some((event, announcement, body)),
            _ => None,
        })
        .unwrap()
}
fn damage(c: &Card) -> &DamageEvent {
    let EventSpec::Damage(d) = trigger(c).0 else {
        panic!()
    };
    d
}
fn body_mut(c: &mut Card) -> &mut Program {
    c.abilities
        .iter_mut()
        .find_map(|a| match a {
            Ability::Triggered { body, .. } => Some(body),
            _ => None,
        })
        .unwrap()
}
fn effect(operation: Operation) -> Instruction {
    Instruction::Effect {
        result: ResultId(0),
        operation,
    }
}

#[test]
fn real_damage_families_roundtrip_and_nested_gaps_stay_incomplete() {
    let incomplete = [
        "Aegar, the Freezing Flame",
        "Aisha of Sparks and Smoke",
        "Anowon, the Ruin Thief",
        "Barbed Shocker",
        "Breeches, Brazen Plunderer",
        "Curious Obsession",
        "Dread",
        "Fall of Cair Andros",
        "Fathom Fleet Swordjack",
        "Fblthp, Impossibly Lost",
        "Firesong and Sunspeaker",
        "Giant Shark",
        "Gonti's Machinations",
        "Kaito, Dancing Shadow",
        "Maarika, Brutal Gladiator",
        "Malcolm, Keen-Eyed Navigator",
        "Neheb, Dreadhorde Champion",
        "Norn's Decree",
        "Sakashima's Will",
        "Screaming Nemesis",
        "Tandem Lookout",
        "The Gitrog, Ravenous Ride",
        "Vengeful Ancestor",
        "Obelisk Spider",
        "Lost Vale",
        "Vault of Catlacan",
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
    assert_eq!(complete, 59);
}

#[test]
fn single_source_recipient_and_batch_triggers_keep_different_grouping_contracts() {
    let c = card("Coastal Piracy");
    assert!(matches!(
        damage(&c),
        DamageEvent {
            grouping: DamageGrouping::SourceRecipient,
            kind: DamageKind::Combat,
            source_kind: ObjectKind::Permanent,
            recipients: DamageRecipients::Players(PlayerSet::OpponentsOf(_)),
            ..
        }
    ));
    let c = card("Blaze Commando");
    assert!(matches!(
        damage(&c),
        DamageEvent {
            grouping: DamageGrouping::Source,
            source_kind: ObjectKind::Spell,
            recipients: DamageRecipients::Any,
            ..
        }
    ));
    let c = card("Boros Reckoner");
    assert!(matches!(
        damage(&c),
        DamageEvent {
            grouping: DamageGrouping::Recipient,
            source_kind: ObjectKind::Any,
            subject: DamageSubject::Recipient,
            ..
        }
    ));
    let c = custom("TriggerA(WhenAnyNumberOfCreaturesDealCombatDamageToAPlayer(AnyPermanent, AnyPlayer), ActionList([PlayerAction(Trigger_ThatPlayer, DrawNumberCards(Trigger_AmountOfCreatures))]))").unwrap();
    validate::card(&c).unwrap();
    assert!(matches!(
        damage(&c),
        DamageEvent {
            grouping: DamageGrouping::Recipient,
            ..
        }
    ));
    assert!(
        matches!(trigger(&c).2.instructions.as_slice(), [Instruction::Effect { operation: Operation::Draw { players: PlayerSet::One(PlayerRef::EventSubject), amount: Expr::CountObjects(objects) }, .. }] if matches!(objects.as_ref(), ObjectSet::Damage(DamageSubject::Source)))
    );
    let c = custom("TriggerA(WhenAnyNumberOfPermanentsDealDamageToAnyNumberOfPlayers(AnyPermanent, AnyPlayer), ActionList([EachPlayerAction(Trigger_ThosePlayers, DrawACard)]))").unwrap();
    validate::card(&c).unwrap();
    assert!(matches!(
        damage(&c),
        DamageEvent {
            grouping: DamageGrouping::Batch,
            ..
        }
    ));
    assert!(matches!(
        trigger(&c).2.instructions.as_slice(),
        [Instruction::Effect {
            operation: Operation::Draw {
                players: PlayerSet::DamageRecipients,
                ..
            },
            ..
        }]
    ));
}

#[test]
fn incoming_amounts_sources_and_damaged_objects_have_distinct_references() {
    let c = custom("TriggerA(WhenACreatureDealsCombatDamageToAPermanent(AnyPermanent, AnyPermanent), ActionList([PlayerAction(Trigger_ControllerOfThatPermanent, DrawACard)]))").unwrap();
    validate::card(&c).unwrap();
    assert!(
        matches!(trigger(&c).2.instructions.as_slice(), [Instruction::Effect { operation: Operation::Draw { players: PlayerSet::One(PlayerRef::ControllerOf(object)), .. }, .. }] if matches!(object.as_ref(), ObjectRef::Damage(DamageSubject::Recipient)))
    );
    let c = card("Boros Reckoner");
    assert!(matches!(
        trigger(&c).2.instructions.as_slice(),
        [Instruction::Effect {
            operation: Operation::DealDamage {
                source: ObjectRef::Damage(DamageSubject::Recipient),
                amount: Expr::EventAmount,
                ..
            },
            ..
        }]
    ));
    let bindings = event_bindings(trigger(&c).0);
    assert!(bindings.amount && bindings.damage_recipient);
    assert!(!bindings.damage_source);
    let c = card("Pious Warrior");
    assert!(matches!(
        damage(&c),
        DamageEvent {
            kind: DamageKind::Combat,
            grouping: DamageGrouping::Recipient,
            ..
        }
    ));
    assert!(matches!(
        trigger(&c).2.instructions.as_slice(),
        [Instruction::Effect {
            operation: Operation::GainLife {
                amount: Expr::EventAmount,
                ..
            },
            ..
        }]
    ));
    let c = card("Chandra's Spitfire");
    assert!(matches!(
        damage(&c),
        DamageEvent {
            kind: DamageKind::Noncombat,
            recipients: DamageRecipients::Players(_),
            ..
        }
    ));
    let c = card("Bellowing Fiend");
    assert!(
        matches!(&trigger(&c).2.instructions[0], Instruction::Effect { operation: Operation::DealDamage { recipients: Recipients::Players(PlayerSet::One(PlayerRef::ControllerOf(object))), .. }, .. } if matches!(object.as_ref(), ObjectRef::Damage(DamageSubject::Recipient)))
    );
    let c = card("Voracious Cobra");
    assert!(matches!(
        trigger(&c).2.instructions.as_slice(),
        [Instruction::Effect {
            operation: Operation::Destroy {
                objects: ObjectSet::One(ObjectRef::Damage(DamageSubject::Recipient)),
                ..
            },
            ..
        }]
    ));
    let c = card("Tephraderm");
    let Ability::Triggered { body, .. } = &c.abilities[0] else {
        panic!()
    };
    assert!(matches!(
        body.instructions.as_slice(),
        [Instruction::Effect {
            operation: Operation::DealDamage {
                source: ObjectRef::Source,
                recipients: Recipients::Objects(ObjectSet::One(ObjectRef::EventSubject)),
                ..
            },
            ..
        }]
    ));
}

#[test]
fn amount_thresholds_filter_at_the_event_without_becoming_resolution_guards() {
    let c = card("Dragonborn Champion");
    assert!(
        matches!(trigger(&c).0, EventSpec::Filtered { event, condition: Condition::Compare { left: Expr::EventAmount, comparison: Comparison::GreaterOrEqual, right: Expr::Constant(5) } } if matches!(event.as_ref(), EventSpec::Damage(DamageEvent { grouping: DamageGrouping::SourceRecipient, .. })))
    );
    let Ability::Triggered { intervening_if, .. } = c
        .abilities
        .iter()
        .find(|a| matches!(a, Ability::Triggered { .. }))
        .unwrap()
    else {
        panic!()
    };
    assert!(intervening_if.is_none());
    let c = card("Reaper of Sheoldred");
    assert!(
        matches!(trigger(&c).2.instructions.as_slice(), [Instruction::Effect { operation: Operation::PutCounters { placements }, .. }] if matches!(&placements[0].recipients, Recipients::Players(PlayerSet::One(PlayerRef::ControllerOf(object))) if matches!(object.as_ref(), ObjectRef::Damage(DamageSubject::Source))))
    );
}

#[test]
fn aggregate_damage_cannot_supply_singular_references_or_leak_event_bindings() {
    let c = card("Cryoshatter");
    assert!(matches!(trigger(&c).0, EventSpec::Either(events) if events.len() == 2));
    assert!(matches!(
        trigger(&c).2.instructions.as_slice(),
        [Instruction::Effect {
            operation: Operation::Destroy {
                objects: ObjectSet::One(ObjectRef::EventSubject),
                ..
            },
            ..
        }]
    ));
    let mut c = custom("TriggerA(WhenAnyNumberOfCreaturesDealCombatDamageToAPlayer(AnyPermanent, AnyPlayer), ActionList([DrawACard]))").unwrap();
    body_mut(&mut c).instructions = vec![effect(Operation::Destroy {
        objects: ObjectSet::One(ObjectRef::Damage(DamageSubject::Source)),
        regeneration: true,
    })];
    assert!(validate::card(&c).unwrap_err().contains("singular object"));
    body_mut(&mut c).instructions = vec![effect(Operation::Destroy {
        objects: ObjectSet::Damage(DamageSubject::Source),
        regeneration: true,
    })];
    validate::card(&c).unwrap();
    let Ability::Triggered { event, .. } = &mut c.abilities[0] else {
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
        .contains("object collection"));
    assert!(custom("TriggerA(WhenAnyNumberOfCreaturesDealCombatDamageToAnyNumberOfPlayers(AnyPermanent, AnyPlayer), ActionList([PlayerAction(Trigger_ThatPlayer, DrawACard)]))").is_err());
    let mut c = card("Coastal Piracy");
    c.abilities.push(Ability::Spell {
        announcement: Announcement::empty(),
        body: Program {
            instructions: vec![effect(Operation::Draw {
                players: PlayerSet::One(PlayerRef::AbilityController),
                amount: Expr::EventAmount,
            })],
        },
    });
    assert!(validate::card(&c).unwrap_err().contains("event amount"));
}

#[test]
fn restricted_damage_targets_and_multi_player_effects_keep_their_predicates_and_batch() {
    let c = card("Mogg Maniac");
    let [target] = trigger(&c).1.targets.as_slice() else {
        panic!()
    };
    assert_eq!(target.kind, TargetKind::DamageRecipient);
    assert!(matches!(
        &target.filter,
        ObjectFilter::CardType(source::CardType::Planeswalker)
    ));
    assert!(matches!(target.players, Some(PlayerSet::OpponentsOf(_))));
    assert!(matches!(
        trigger(&c).2.instructions.as_slice(),
        [Instruction::Effect {
            operation: Operation::DealDamage {
                recipients: Recipients::Targets(Slot(0)),
                ..
            },
            ..
        }]
    ));
    let c = card("Flumph");
    assert!(
        matches!(trigger(&c).2.instructions.as_slice(), [Instruction::Effect { operation: Operation::Draw { players: PlayerSet::Union(sets), .. }, .. }] if sets.len() == 2)
    );
    let c = card("Silverclad Ferocidons");
    assert!(matches!(
        trigger(&c).2.instructions.as_slice(),
        [Instruction::Effect {
            operation: Operation::Sacrifice {
                players: PlayerSet::OpponentsOf(_),
                choose: Some(Count::AsManyAsCan(Expr::Constant(1))),
                ..
            },
            ..
        }]
    ));
}

#[test]
fn ambiguous_legacy_roles_and_first_occurrence_history_remain_explicit_gaps() {
    let c = card("Greatbow Doyen");
    assert!(
        matches!(trigger(&c).2.instructions.as_slice(), [Instruction::Effect { operation: Operation::DealDamage { source: ObjectRef::Damage(DamageSubject::Source), recipients: Recipients::Players(PlayerSet::One(PlayerRef::ControllerOf(object))), .. }, .. }] if matches!(object.as_ref(), ObjectRef::Damage(DamageSubject::Recipient)))
    );
    let c = card("Vraska, Swarm's Eminence");
    assert!(
        matches!(trigger(&c).2.instructions.as_slice(), [Instruction::Effect { operation: Operation::PutCounters { placements }, .. }] if matches!(placements[0].recipients, Recipients::Objects(ObjectSet::One(ObjectRef::EventSubject))))
    );
    assert!(custom("TriggerA(WhenAPermanentDealsDamageToAPermanent(AnyPermanent, AnyPermanent), ActionList([DestroyPermanent(Trigger_ThatPermanent)]))").is_err());
    let c = custom("TriggerA(WhenAPermanentDealsDamageToAPermanent(AnyPermanent, AnyPermanent), ActionList([DestroyPermanent(Trigger_ThatOtherPermanent)]))").unwrap();
    validate::card(&c).unwrap();
    assert!(custom("TriggerA(WhenACreatureDealsCombatDamageToAPlayerForTheFirstTimeEachTurn(AnyPermanent, AnyPlayer), ActionList([DrawACard]))").is_err());
    assert!(custom(
        "TriggerA(WhenAPermanentIsDealtExcessDamage(AnyPermanent), ActionList([DrawACard]))"
    )
    .is_err());
}

#[test]
fn prevention_is_a_replacement_on_a_damage_event() {
    let lower = |rule: &str| {
        let line = format!(
            r#"Card(Name: "Test", Typeline: (Supertypes: [], Cardtypes: [Instant], Subtypes: []), ManaCost: [], Rules: [{rule}])"#
        );
        let source: source::OracleCard = ron::from_str(&line).unwrap();
        let card = lower::card(&source).unwrap();
        validate::card(&card).unwrap();
        serde_json::to_string(&card).unwrap()
    };
    let fog = lower(
        "SpellActions(ActionList([CreatePreventDamageUntil(CombatDamageWouldBeDealt, [PreventThatDamage], UntilEndOfTurn)]))",
    );
    assert!(fog.contains(r#""kind":"CreateReplacement""#), "{fog}");
    assert!(fog.contains(r#""limit":"UntilExpired""#), "{fog}");
    assert!(fog.contains(r#""kind":"Combat""#), "{fog}");
    assert!(fog.contains(r#""kind":"ModifyEvent""#), "{fog}");
    let salve = lower(
        "SpellActions(Targeted([AnyTarget], ActionList([CreateFuturePreventDamage(NextAmountOfDamageThatWouldBeDealtThisTurnToRecipient(Integer(3), Ref_AnyTarget), [PreventThatDamage])])))",
    );
    assert!(salve.contains("TotalDamage"), "{salve}");
    assert!(salve.contains(r#""duration":"EndOfTurn""#), "{salve}");
}
