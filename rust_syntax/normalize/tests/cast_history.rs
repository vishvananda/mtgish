use mtgish_ast::{corpus, ir::*, lower, mtg_types as source, validate};

fn sources() -> impl Iterator<Item = source::OracleCard> {
    include_str!("fixtures/cast_history.ronl")
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
fn custom(rule: &str) -> Result<Card, Vec<lower::Diagnostic>> {
    let mut c = source("Ancient Grudge");
    let source::OracleCard::Card { rules, .. } = &mut c else {
        panic!()
    };
    *rules = Some(vec![ron::from_str(rule).unwrap()]);
    lower::card(&c)
}
fn casting(c: &Card) -> &CastingRule {
    c.abilities
        .iter()
        .find_map(|a| {
            if let Ability::Casting(rule) = a {
                Some(rule)
            } else {
                None
            }
        })
        .unwrap()
}
fn intervening(c: &Card) -> &Condition {
    c.abilities
        .iter()
        .find_map(|a| {
            if let Ability::Triggered {
                intervening_if: Some(condition),
                ..
            } = a
            {
                Some(condition)
            } else {
                None
            }
        })
        .unwrap()
}
fn spell(c: &Card) -> &Program {
    c.abilities
        .iter()
        .find_map(|a| {
            if let Ability::Spell { body, .. } = a {
                Some(body)
            } else {
                None
            }
        })
        .unwrap()
}
fn spent_filter(c: &Card) -> &Expr {
    let Condition::Compare { left, .. } = intervening(c) else {
        panic!("expected a directly bound spent-mana condition")
    };
    left
}

#[test]
fn flashback_is_permission_and_stack_exit_replacement_with_an_alternative_cost() {
    let c = card("Ancient Grudge");
    assert!(matches!(casting(&c),CastingRule::Permission {
        origin:CastingOrigin::Flashback,
        player:PlayerRef::OwnerOf(_),
        from:ZoneScope::Players {zone:EventZone::Graveyard,players:PlayerSet::One(PlayerRef::OwnerOf(_))},
        require:ObjectFilter::AnyOf(types),
        cost:Cost {parts},leave_stack:Some(Destination::Exile),
    } if types.len()==2 && matches!(parts.as_slice(),[CostPart::Mana(m)] if matches!(m.as_slice(),[ManaSymbol::Color(ManaColor::Green)]))));
    assert!(
        matches!(
            spell(&c).instructions.as_slice(),
            [Instruction::Effect {
                operation: Operation::Destroy { .. },
                ..
            }]
        ),
        "flashback exile must also apply when countered/bounced; it is not appended to resolution"
    );
    let c = card("Acorn Harvest");
    assert!(
        matches!(casting(&c),CastingRule::Permission {cost:Cost {parts},..} if matches!(parts.as_slice(),[CostPart::Mana(_),CostPart::Life(Expr::Constant(3))]))
    );
    let c = card("Dread Return");
    assert!(
        matches!(casting(&c),CastingRule::Permission {cost:Cost {parts},..} if matches!(parts.as_slice(),[CostPart::Sacrifice {choose:Some(Count::Exactly(Expr::Constant(3))),..}]))
    );
    let c = card("Increasing Confusion");
    assert!(
        matches!(casting(&c),CastingRule::Permission {cost:Cost {parts},..} if matches!(parts.as_slice(),[CostPart::Mana(m)] if matches!(m.as_slice(),[ManaSymbol::X,ManaSymbol::Color(ManaColor::Blue)])))
    );
}

#[test]
fn granted_flashback_uses_the_receiving_cards_mana_symbols_and_a_snapshot_cohort() {
    for name in ["Snapcaster Mage", "Past in Flames"] {
        let c = card(name);
        let body = c
            .abilities
            .iter()
            .find_map(|a| match a {
                Ability::Spell { body, .. } | Ability::Triggered { body, .. } => Some(body),
                _ => None,
            })
            .unwrap();
        let Instruction::Effect {
            operation:
                Operation::CreateContinuousEffect {
                    affected,
                    changes,
                    duration: Duration::EndOfTurn,
                    evaluation: EffectEvaluation::Snapshot,
                    ..
                },
            ..
        } = &body.instructions[0]
        else {
            panic!()
        };
        if name == "Snapcaster Mage" {
            assert!(matches!(
                affected,
                EffectSubjects::Objects(ObjectSet::InZone {
                    zone: EventZone::Graveyard,
                    ..
                })
            ));
        } else {
            assert!(matches!(
                affected,
                EffectSubjects::Objects(ObjectSet::Zone {
                    scope: ZoneScope::Players {
                        zone: EventZone::Graveyard,
                        ..
                    },
                    ..
                })
            ));
        }
        let [Change::GrantAbility(ability)] = changes.as_slice() else {
            panic!()
        };
        assert!(matches!(ability.as_ref(),Ability::Casting(CastingRule::Permission {cost:Cost {parts},..}) if matches!(parts.as_slice(),[CostPart::ManaCostOf(ObjectRef::Source)])),"mana cost comes from each receiving card, including X/colored symbols and an unpayable absent cost");
    }
}

#[test]
fn entry_queries_keep_actual_cast_origin_distinct_from_battlefield_entry_origin() {
    let c = card("Coal Stoker");
    assert!(matches!(
        intervening(&c),
        Condition::ObjectMatches {
            object: ObjectRef::EventSubject,
            filter: ObjectFilter::Cast {
                by: Some(PlayerSet::One(PlayerRef::AbilityController)),
                from: Some(ZoneScope::Players {
                    zone: EventZone::Hand,
                    players: PlayerSet::One(PlayerRef::AbilityController)
                })
            }
        }
    ));
    let c = card("Archfiend's Vessel");
    let Condition::ObjectMatches {
        filter: ObjectFilter::AnyOf(filters),
        ..
    } = intervening(&c)
    else {
        panic!()
    };
    assert!(matches!(
        filters.as_slice(),
        [
            ObjectFilter::EnteredFrom(ZoneScope::Players {
                zone: EventZone::Graveyard,
                ..
            }),
            ObjectFilter::Cast {
                from: Some(ZoneScope::Players {
                    zone: EventZone::Graveyard,
                    ..
                }),
                ..
            }
        ]
    ));
    let c = custom(
        "SpellActions(ActionList([If(CastSpellPassesFilter(WasCastFromTheirHand),[DrawACard])]))",
    )
    .unwrap();
    validate::card(&c).unwrap();
    assert!(
        matches!(&spell(&c).instructions[0],Instruction::If {condition:Condition::ObjectMatches {object:ObjectRef::Source,filter:ObjectFilter::Cast {from:Some(ZoneScope::Players {players:PlayerSet::One(PlayerRef::CasterOf(object)),..}),..}},..} if matches!(object.as_ref(),ObjectRef::Candidate)),
        "their hand means the recorded caster's hand, even after control changes"
    );
}

#[test]
fn spent_mana_queries_use_actual_types_and_count_repeated_requirements() {
    let c=custom("SpellActions(ActionList([If(CastSpellPassesFilter(ManaWasSpentToCastIt([ManaProduceW,ManaProduceW,ManaProduceR])),[DrawACard])]))").unwrap();
    validate::card(&c).unwrap();
    let Instruction::If {
        condition: Condition::AllOf(filters),
        ..
    } = &spell(&c).instructions[0]
    else {
        panic!()
    };
    assert_eq!(filters.len(), 2);
    assert!(
        matches!(&filters[0],Condition::Compare { left:Expr::ManaSpent {object:ObjectRef::Source,filter:SpentManaFilter {colors:Some(colors),snow:false},aggregation:SpentManaAggregation::Count}, test: NumberTest::Compare { comparison: Comparison::GreaterOrEqual, value: Expr::Constant(2) } } if colors==&vec![ManaColor::White])
    );
    let c = card("Clockwork Servant");
    assert!(matches!(
        spent_filter(&c),
        Expr::ManaSpent {
            aggregation: SpentManaAggregation::MaximumPerColor,
            ..
        }
    ));
    let c = card("Steel Exemplar");
    let Ability::Replacement { body, .. } = &c.abilities[1] else {
        panic!()
    };
    let Instruction::If {
        condition: Condition::Not(condition),
        ..
    } = &body.instructions[0]
    else {
        panic!()
    };
    assert!(matches!(
        condition.as_ref(),
        Condition::Compare { left: Expr::ManaSpent {
                object: ObjectRef::EventSubject,
                aggregation: SpentManaAggregation::DistinctColors,
                ..
            }, .. }
    ));
    let c=custom("SpellActions(ActionList([If(CastSpellPassesFilter(NoColoredManaWasSpentToCastIt),[DrawACard])]))").unwrap();
    validate::card(&c).unwrap();
    let Instruction::If {
        condition,
        ..
    } = &spell(&c).instructions[0]
    else {
        panic!()
    };
    assert!(
        matches!(condition,Condition::Compare { left:Expr::ManaSpent {object:ObjectRef::Source,filter:SpentManaFilter {colors:Some(colors),..},..}, test: NumberTest::Compare { comparison: Comparison::Equal, value: Expr::Constant(0) } } if colors.len()==5 && !colors.contains(&ManaColor::Colorless))
    );
}

#[test]
fn snow_mana_and_recorded_amounts_are_independent_of_mana_cost_and_color() {
    let c=custom("TriggerI(WhenAPermanentEntersTheBattlefield(SinglePermanent(ThisPermanent)),EnteringPermanentPassesFilter(SnowManaWasSpentToCastIt),ActionList([DrawACard]))").unwrap();
    validate::card(&c).unwrap();
    assert!(matches!(
        spent_filter(&c),
        Expr::ManaSpent {
            filter: SpentManaFilter {
                colors: None,
                snow: true
            },
            aggregation: SpentManaAggregation::Count,
            ..
        }
    ));
    let c=custom("SpellActions(Targeted([TargetSpell(AnySpell)],ActionList([AddManaRepeated(ManaProduceC,AmountOfManaSpentToCastSpell(Ref_TargetSpell))])))").unwrap();
    validate::card(&c).unwrap();
    let json = serde_json::to_value(&c).unwrap().to_string();
    assert!(json.contains("ManaSpent") && json.contains("Target") && !json.contains("ManaValue"));
}

#[test]
fn entry_conditions_reuse_queries_without_rebinding_nested_candidates() {
    let c = card("Deathbringer Regent");
    let Condition::AllOf(filters) = intervening(&c) else { panic!() };
    let condition = &filters[1];
    assert!(
        matches!(condition,Condition::Compare { left:Expr::CountObjects(objects), .. } if matches!(objects.as_ref(),ObjectSet::Difference {excluding,..} if matches!(excluding.as_ref(),ObjectSet::One(ObjectRef::EventSubject))))
    );
}

#[test]
fn new_metadata_and_granted_definitions_preserve_validation_scopes() {
    let mut c = card("Ancient Grudge");
    let a = c
        .abilities
        .iter_mut()
        .find(|a| matches!(a, Ability::Casting(_)))
        .unwrap();
    let Ability::Casting(CastingRule::Permission { leave_stack, .. }) = a else {
        panic!()
    };
    *leave_stack = None;
    assert!(validate::card(&c).is_err());
    let mut c = card("Snapcaster Mage");
    let Ability::Triggered { body, .. } = &mut c.abilities[1] else {
        panic!()
    };
    let Instruction::Effect {
        operation: Operation::CreateContinuousEffect { changes, .. },
        ..
    } = &mut body.instructions[0]
    else {
        panic!()
    };
    let Change::GrantAbility(ability) = &mut changes[0] else {
        panic!()
    };
    let Ability::Casting(CastingRule::Permission { cost, .. }) = ability.as_mut() else {
        panic!()
    };
    cost.parts = vec![CostPart::ManaCostOf(ObjectRef::Target {
        slot: TargetSlot(0),
        index: 0,
    })];
    assert!(
        validate::card(&c).is_err(),
        "a new ability definition must not see the granting trigger's targets"
    );
    assert!(custom("Activated(PayMana([ManaCostGeneric(1)]),ActionList([If(EnteringPermanentPassesFilter(ItWasCast),[DrawACard])]))").is_err(),"entry alias requires an entry event or an actual prior movement result");
    let mut c = card("Ancient Grudge");
    let Ability::Spell { body, .. } = &mut c.abilities[0] else {
        panic!()
    };
    body.instructions = vec![Instruction::Effect {
        result: ResultId(0),
        operation: Operation::Draw {
            players: PlayerSet::One(PlayerRef::AbilityController),
            amount: Expr::ManaSpent {
                object: ObjectRef::Candidate,
                filter: SpentManaFilter {
                    colors: None,
                    snow: false,
                },
                aggregation: SpentManaAggregation::Count,
            },
        },
    }];
    assert!(
        validate::card(&c).is_err(),
        "candidate references cannot escape their filter"
    );
}

#[test]
fn spell_event_filters_bind_the_cast_spell_and_source_spell_bodies_keep_the_source() {
    let c=custom("TriggerI(WhenAPlayerCastsASpell(AnyPlayer,AnySpell),CastSpellPassesFilter(ManaWasSpentToCastIt([ManaProduceU])),ActionList([DrawACard]))").unwrap();
    validate::card(&c).unwrap();
    assert!(matches!(
        intervening(&c),
        Condition::Compare { left: Expr::ManaSpent { object: ObjectRef::EventSubject, .. }, .. }
    ));
    let c = card("Increasing Confusion");
    assert!(matches!(
        &spell(&c).instructions[0],
        Instruction::If {
            condition: Condition::ObjectMatches {
                object: ObjectRef::Source,
                filter: ObjectFilter::Cast {
                    from: Some(ZoneScope::Players {
                        zone: EventZone::Graveyard,
                        players: PlayerSet::All
                    }),
                    ..
                }
            },
            ..
        }
    ));
}

#[test]
fn cast_history_families_roundtrip_and_keep_unimplemented_nested_semantics() {
    let incomplete: [&str; 0] = [];
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
    assert_eq!(complete, 69);
    let rapid = card("Rapid Augmenter");
    assert!(rapid.abilities.iter().any(|a|matches!(a,Ability::Triggered {event:EventSpec::ZoneChange {objects:ObjectFilter::AllOf(outer),..},..} if outer.iter().any(|f|matches!(f,ObjectFilter::AllOf(inner) if inner.iter().any(|f|matches!(f,ObjectFilter::Property { property:Property::BasePower, test: NumberTest::Compare { comparison: Comparison::Equal, value: Expr::Constant(1) } })))))));
}
