use mtgish_ast::{corpus, counter::CounterKind, ir::*, lower, mtg_types::OracleCard, validate};

fn card(name: &str) -> Card {
    let source = include_str!("fixtures/families.ronl")
        .lines()
        .find_map(|line| {
            let value: OracleCard = ron::from_str(line).unwrap();
            corpus::Identity::of(&value)
                .unwrap()
                .names
                .iter()
                .any(|n| n == name)
                .then_some(value)
        })
        .unwrap();
    let card = lower::card(&source).unwrap_or_else(|errors| panic!("{name}: {errors:?}"));
    validate::card(&card).unwrap();
    card
}

#[test]
fn real_card_families_lower_and_roundtrip_without_escape_forms() {
    for line in include_str!("fixtures/families.ronl").lines() {
        let source: OracleCard = ron::from_str(line).unwrap();
        let card = lower::card(&source).unwrap_or_else(|errors| {
            panic!(
                "{:?}: {errors:?}",
                corpus::Identity::of(&source).unwrap().names
            )
        });
        validate::card(&card).unwrap();
        let json = serde_json::to_value(&card).unwrap();
        let from_json: Card = serde_json::from_value(json.clone()).unwrap();
        let from_ron: Card = ron::from_str(&ron::ser::to_string(&card).unwrap()).unwrap();
        let bytes = bincode::encode_to_vec(&card, bincode::config::standard()).unwrap();
        let (from_binary, read): (Card, usize) =
            bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
        assert_eq!(read, bytes.len());
        for decoded in [from_json, from_ron, from_binary] {
            validate::card(&decoded).unwrap();
            assert_eq!(serde_json::to_value(decoded).unwrap(), json);
        }
    }
}

#[test]
fn incremental_counters_use_three_distinct_targets_in_one_batch() {
    for (name, sign) in [("Incremental Growth", 1), ("Incremental Blight", -1)] {
        let card = card(name);
        let Ability::Spell { announcement, body } = &card.abilities[0] else {
            panic!()
        };
        assert_eq!(announcement.targets.len(), 3);
        assert_eq!(body.instructions.len(), 1);
        let Instruction::Effect {
            operation: Operation::PutCounters { placements },
            ..
        } = &body.instructions[0]
        else {
            panic!()
        };
        assert_eq!(placements.len(), 3);
        for (index, placement) in placements.iter().enumerate() {
            assert!(
                matches!(placement.kind,CounterKind::PowerToughness(p,t) if p==sign && t==sign)
            );
            assert!(matches!(placement.amount,Expr::Constant(n) if n==index as i64+1));
            assert!(placement.selection.is_none());
            assert!(
                matches!(&placement.recipients,Recipients::Objects(ObjectSet::One(ObjectRef::Target{slot:Slot(slot),index:0})) if *slot==index as u16)
            );
        }
    }
}

#[test]
fn static_definition_retains_live_queries_and_resolving_effect_snapshots() {
    let ram = card("Gatebreaker Ram");
    let Ability::Static { changes, .. } = &ram.abilities[0] else {
        panic!()
    };
    assert!(matches!(
        &changes[0],
        Change::Pump {
            power: Expr::CountObjects(_),
            toughness: Expr::CountObjects(_)
        }
    ));
    assert!(matches!(
        &ram.abilities[1],
        Ability::Conditional {
            condition: Condition::Compare {
                left: Expr::CountObjects(_),
                comparison: Comparison::GreaterOrEqual,
                right: Expr::Constant(2)
            },
            ..
        }
    ));
    let aristocrat = card("Vampire Aristocrat");
    let Ability::Activated { cost, body, .. } = &aristocrat.abilities[0] else {
        panic!()
    };
    assert!(matches!(
        &cost.parts[0],
        CostPart::Sacrifice {
            choose: Some(Count::Exactly(Expr::Constant(1))),
            ..
        }
    ));
    assert!(matches!(
        &body.instructions[0],
        Instruction::Effect {
            operation: Operation::CreateContinuousEffect {
                evaluation: EffectEvaluation::Snapshot,
                ..
            },
            ..
        }
    ));
}

#[test]
fn loyalty_delta_and_intervening_condition_are_preserved() {
    let jace = card("Jace Beleren");
    for (ability, delta) in jace.abilities.iter().zip([2, -1, -10]) {
        let Ability::Activated { cost, limits, .. } = ability else {
            panic!()
        };
        assert!(
            matches!(cost.parts.as_slice(),[CostPart::Loyalty(Expr::Constant(n))] if *n==delta)
        );
        assert!(limits.sorcery);
        // The loyalty cost carries the shared per-permanent rule. It must not
        // become a separate once-per-turn allowance for each printed ability.
        assert!(!limits.once_each_turn);
    }
    let vampires = card("Bleak Coven Vampires");
    assert!(matches!(
        &vampires.abilities[0],
        Ability::Triggered {
            intervening_if: Some(Condition::Compare {
                comparison: Comparison::GreaterOrEqual,
                right: Expr::Constant(3),
                ..
            }),
            ..
        }
    ));
}

#[test]
fn state_triggers_do_not_invent_an_event_subject() {
    let mut armodon = card("Endangered Armodon");
    let Ability::Triggered { event, body, .. } = &mut armodon.abilities[0] else {
        panic!()
    };
    assert!(matches!(event, EventSpec::State { .. }));
    let Instruction::Effect {
        operation: Operation::Sacrifice { objects, .. },
        ..
    } = &mut body.instructions[0]
    else {
        panic!()
    };
    *objects = ObjectSet::One(ObjectRef::EventSubject);
    assert!(validate::card(&armodon)
        .unwrap_err()
        .contains("event subject"));
}

#[test]
fn affected_object_is_scoped_to_characteristic_changes() {
    let mut anthem = card("Glorious Anthem");
    let Ability::Static { changes, .. } = &mut anthem.abilities[0] else {
        panic!()
    };
    changes.push(Change::SetPower(Expr::ObjectProperty {
        object: ObjectRef::Affected,
        property: Property::Toughness,
        information: Information::CurrentOrLastKnown,
    }));
    validate::card(&anthem).unwrap();
    let Ability::Static { affected, .. } = &mut anthem.abilities[0] else {
        panic!()
    };
    *affected = (ObjectSet::One(ObjectRef::Affected)).into();
    assert!(validate::card(&anthem)
        .unwrap_err()
        .contains("affected object"));
}

#[test]
fn distributed_counters_need_distribution_semantics_before_counting_complete() {
    let source:OracleCard=ron::from_str("Card(Name:\"Distributed counter fixture\",Typeline:(Supertypes:[],Cardtypes:[Sorcery],Subtypes:[]),Rules:[SpellActions(ActionList([PutCounters([DistributedCounters(PTCounter(1,1))])]))])").unwrap();
    let errors = lower::card(&source).unwrap_err();
    assert!(errors
        .iter()
        .any(|e| e.source_type == "PutCountersAction" && e.form == "DistributedCounters"));
}
