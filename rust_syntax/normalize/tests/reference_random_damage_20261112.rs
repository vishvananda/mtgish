use mtgish_ast::{corpus, ir::*, lower, mtg_types::OracleCard, unified, validate};
use std::collections::BTreeMap;

const SOURCE: &str = include_str!("fixtures/reference_random_damage_20261112.ronl");
fn lowered(text: &str) -> UnifiedCard {
    let source: OracleCard = ron::from_str(text).unwrap();
    let result = unified::lower(&source);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    validate::unified(&result.card).unwrap();
    unified::round_trip(&result.card).unwrap();
    if matches!(source, OracleCard::Card { .. }) {
        let strict = lower::card(&source).unwrap();
        assert_eq!(
            serde_json::to_value(&strict.abilities).unwrap(),
            serde_json::to_value(&result.card.faces[0].abilities).unwrap()
        );
    }
    result.card
}
fn cards() -> BTreeMap<String, Card> {
    SOURCE
        .lines()
        .flat_map(|l| lowered(l).faces)
        .map(|c| (c.name.clone(), c))
        .collect()
}
fn custom(rule: &str) -> Card {
    lowered(&format!("Card(Name:\"Control\",Typeline:(Supertypes:[],Cardtypes:[Instant],Subtypes:[]),Rules:[{rule}])"))
        .faces.remove(0)
}
fn steps(card: &Card) -> Vec<SelectionStep> {
    use mtgish_ast::visit::{Visitor, Walk};
    struct Selections(Vec<SelectionStep>);
    impl Visitor for Selections {
        fn operation(&mut self, op: &Operation) -> bool {
            if let Operation::WithSelections { steps, .. } = op {
                self.0.extend(
                    steps
                        .iter()
                        .filter(|s| matches!(s.chooser, SelectionChooser::Chance))
                        .cloned(),
                );
            }
            true
        }
    }
    let mut visitor = Selections(vec![]);
    card.walk(&mut visitor);
    visitor.0
}

#[test]
fn complete_source_packet_validates_and_round_trips() {
    corpus::on_large_stack(|| {
        let cards = cards();
        assert_eq!(SOURCE.lines().count(), 23);
        assert_eq!(cards.len(), 23);
        if let Ok(dir) = std::env::var("MTGISH_RANDOM_DAMAGE_DUMP") {
            std::fs::create_dir_all(&dir).unwrap();
            for (name, card) in cards {
                std::fs::write(
                    std::path::Path::new(&dir).join(format!("{name}.json")),
                    serde_json::to_vec_pretty(&card).unwrap(),
                )
                .unwrap();
            }
        }
    });
}

#[test]
fn random_returns_share_resolution_time_cardinality_and_pool_contracts() {
    corpus::on_large_stack(|| {
        let cards = cards();
        for name in [
            "Haunted Fengraf",
            "Fossil Find",
            "Surreal Memoir",
            "Trash Bin",
            "The Deck of Many Things",
            "Charmbreaker Devils",
            "Exalted Flamer of Tzeentch",
            "Ghoulraiser",
            "Viconia, Drow Apostate",
            "Woodland Sleuth",
            "Nephalia",
            "Make a Wish",
            "Reap the Past",
        ] {
            let steps = steps(&cards[name]);
            assert_eq!(steps.len(), 1, "{name}");
            let step = &steps[0];
            assert!(
                matches!(step.count, Count::AsManyAsCan(_)),
                "{name}: {step:?}"
            );
            assert!(step.constraints.is_empty(), "{name}");
            let ObjectSet::Zone { scope, filter, .. } = &step.objects else {
                panic!("{name}")
            };
            assert!(
                matches!(
                    scope,
                    ZoneScope::Players {
                        zone: EventZone::Graveyard,
                        players: PlayerSet::One(PlayerRef::AbilityController)
                    }
                ),
                "{name}: {scope:?}"
            );
            if [
                "Fossil Find",
                "Trash Bin",
                "The Deck of Many Things",
                "Nephalia",
                "Make a Wish",
                "Reap the Past",
            ]
            .contains(&name)
            {
                assert!(matches!(filter, ObjectFilter::Any), "{name}: {filter:?}");
            }
            if !["Make a Wish", "Reap the Past"].contains(&name) {
                assert!(
                    matches!(step.count, Count::AsManyAsCan(Expr::Constant(1))),
                    "{name}"
                );
            }
        }
    });
}

#[test]
fn a_random_library_match_keeps_its_top_card_boundary_and_type_filter() {
    corpus::on_large_stack(|| {
        let cards = cards();
        let steps = steps(&cards["Chronicler of Worship"]);
        assert_eq!(steps.len(), 1);
        assert!(matches!(
            steps[0].count,
            Count::AsManyAsCan(Expr::Constant(1))
        ));
        let ObjectSet::Matching {
            objects,
            filter: ObjectFilter::Subtype(subtype),
        } = &steps[0].objects
        else {
            panic!()
        };
        assert_eq!(format!("{subtype:?}"), "Shrine");
        let ObjectSet::Take {
            objects,
            count: Expr::Constant(7),
            from: SequenceEnd::Top,
        } = objects.as_ref()
        else {
            panic!()
        };
        assert!(matches!(
            objects.as_ref(),
            ObjectSet::Zone {
                scope: ZoneScope::Players {
                    zone: EventZone::Library,
                    players: PlayerSet::One(PlayerRef::AbilityController)
                },
                ..
            }
        ));
    });
}

#[test]
fn equivalent_random_return_source_templates_produce_identical_programs() {
    for filters in [
        ["IsCardtype(Creature)", "IsCardtype(Creature)"],
        ["InAPlayersGraveyard(SinglePlayer(You))", "AnyCard"],
    ] {
        let cards = [
            "PutACardFromGraveyardIntoHandAtRandom",
            "ReturnACardFromGraveyardToHandAtRandom",
        ]
        .into_iter()
        .zip(filters)
        .map(|(form, filter)| custom(&format!("SpellActions(ActionList([{form}({filter})]))")))
        .collect::<Vec<_>>();
        assert_eq!(
            serde_json::to_value(&cards[0].abilities).unwrap(),
            serde_json::to_value(&cards[1].abilities).unwrap()
        );
    }
}

#[test]
fn a_pool_filter_for_a_different_player_is_not_redundant() {
    let card = custom("SpellActions(Targeted([TargetPlayer(Opponent)],ActionList([PlayerAction(Ref_TargetPlayer,PutACardFromGraveyardIntoHandAtRandom(InAPlayersGraveyard(SinglePlayer(You))))])))");
    let step = &steps(&card)[0];
    let ObjectSet::Zone { scope, filter, .. } = &step.objects else {
        panic!()
    };
    assert!(matches!(
        scope,
        ZoneScope::Players {
            players: PlayerSet::One(PlayerRef::Target { .. }),
            ..
        }
    ));
    assert!(matches!(
        filter,
        ObjectFilter::InZone(ZoneScope::Players {
            players: PlayerSet::One(PlayerRef::AbilityController),
            ..
        })
    ));
}

fn trigger_destroy(body: &Program) -> (&EventSpec, &ObjectRef) {
    let Instruction::Effect {
        operation: Operation::CreateTrigger { event, body, .. },
        ..
    } = &body.instructions[0]
    else {
        panic!("{body:?}")
    };
    let Instruction::Effect {
        operation:
            Operation::Destroy {
                objects: ObjectSet::One(object),
                ..
            },
        ..
    } = &body.instructions[0]
    else {
        panic!("{body:?}")
    };
    (event, object)
}

#[test]
fn phage_avatar_destroys_the_damaged_creature_and_keeps_its_other_ability() {
    corpus::on_large_stack(|| {
        let cards = cards();
        let abilities = &cards["Phage the Untouchable Avatar"].abilities;
        let Ability::Activated { body, .. } = &abilities[0] else {
            panic!()
        };
        assert!(matches!(
            trigger_destroy(body).1,
            ObjectRef::Damage(DamageSubject::Source)
        ));
        let Ability::Activated { body, .. } = &abilities[1] else {
            panic!()
        };
        let (event, destroyed) = trigger_destroy(body);
        assert!(matches!(
            event,
            EventSpec::Damage(DamageEvent {
                sources: ObjectFilter::SameObject(ObjectRef::Target { .. }),
                recipients: DamageRecipients::Objects(_),
                ..
            })
        ));
        assert!(
            matches!(destroyed, ObjectRef::Damage(DamageSubject::Recipient)),
            "{destroyed:?}"
        );
        let Ability::Triggered { body, .. } = &cards["Phage the Untouchable"].abilities[1] else {
            panic!()
        };
        let Instruction::Effect {
            operation:
                Operation::Destroy {
                    objects: ObjectSet::One(ObjectRef::Damage(DamageSubject::Recipient)),
                    ..
                },
            ..
        } = &body.instructions[0]
        else {
            panic!("{body:?}")
        };
    });
}

#[test]
fn a_created_trigger_does_not_inherit_its_creators_damage_roles() {
    let card = custom("TriggerA(WhenACreatureDealsCombatDamageToAPermanent(SinglePermanent(ThisPermanent),IsCardtype(Creature)),ActionList([CreateTriggerUntil(WhenAPermanentDies(IsCardtype(Creature)),ActionList([DestroyPermanent(Trigger_ThatPermanent)]),UntilEndOfTurn)]))");
    let Ability::Triggered { body, .. } = &card.abilities[0] else {
        panic!()
    };
    assert!(matches!(trigger_destroy(body).1, ObjectRef::EventSubject));
}

#[test]
fn the_created_damage_source_and_recipient_keep_distinct_references() {
    let card = custom("SpellActions(Targeted([TargetPermanent(IsCardtype(Creature))],ActionList([CreateTriggerUntil(WhenACreatureDealsCombatDamageToAPermanent(SinglePermanent(Ref_TargetPermanent),IsCardtype(Creature)),ActionList([DestroyPermanent(Trigger_ThatCreature),DestroyPermanent(Trigger_ThatPermanent)]),UntilEndOfTurn)])))");
    let Ability::Spell { body, .. } = &card.abilities[0] else {
        panic!()
    };
    let Instruction::Effect {
        operation: Operation::CreateTrigger { body, .. },
        ..
    } = &body.instructions[0]
    else {
        panic!()
    };
    for (instruction, role) in body
        .instructions
        .iter()
        .zip([DamageSubject::Source, DamageSubject::Recipient])
    {
        let Instruction::Effect {
            operation:
                Operation::Destroy {
                    objects: ObjectSet::One(ObjectRef::Damage(actual)),
                    ..
                },
            ..
        } = instruction
        else {
            panic!("{instruction:?}")
        };
        assert_eq!(*actual, role);
    }
}
