use mtgish_ast::{corpus, ir::*, lower, mtg_types as source, validate};

fn sources() -> impl Iterator<Item = source::OracleCard> {
    include_str!("fixtures/players.ronl")
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
    let mut c = source("Silence");
    let source::OracleCard::Card { rules, .. } = &mut c else {
        panic!()
    };
    *rules = Some(vec![ron::from_str(rule).unwrap()]);
    lower::card(&c)
}
fn rules(card: &Card) -> Vec<&PlayerRule> {
    fn visit<'a>(abilities: &'a [Ability], out: &mut Vec<&'a PlayerRule>) {
        for a in abilities {
            match a {
                Ability::Static { changes, .. } => out.extend(changes.iter().filter_map(|c| {
                    if let Change::PlayerRule(r) = c {
                        Some(r)
                    } else {
                        None
                    }
                })),
                Ability::Conditional {
                    then, otherwise, ..
                } => {
                    visit(then, out);
                    visit(otherwise, out);
                }
                _ => {}
            }
        }
    }
    let mut out = vec![];
    visit(&card.abilities, &mut out);
    out
}
fn body(c: &Card) -> &Program {
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

#[test]
fn real_player_families_roundtrip_and_unsupported_nested_mechanics_stay_incomplete() {
    let incomplete = ["Valakut Exploration", "Muldrotha, the Gravetide"];
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
    assert_eq!(complete, 49);
}

#[test]
fn static_and_temporary_player_rules_share_effects_and_keep_their_scope() {
    let curse = card("Curse of Exhaustion");
    assert!(matches!(&curse.abilities[1], Ability::Static {
        affected: EffectSubjects::Players(PlayerSet::One(PlayerRef::EnchantedBy(source))), changes, ..
    } if matches!(**source, ObjectRef::Source) && matches!(changes.as_slice(), [Change::PlayerRule(PlayerRule::LimitPerTurn { action: PlayerAction::CastSpell { .. }, amount: Expr::Constant(1) })])));
    let mut silence = card("Silence");
    let Ability::Spell { body, .. } = &mut silence.abilities[0] else {
        panic!()
    };
    let Instruction::Effect {
        operation:
            Operation::CreateContinuousEffect {
                affected,
                evaluation,
                duration,
                changes,
                ..
            },
        ..
    } = &mut body.instructions[0]
    else {
        panic!()
    };
    assert!(matches!(
        affected,
        EffectSubjects::Players(PlayerSet::OpponentsOf(PlayerRef::AbilityController))
    ));
    assert!(matches!(duration, Duration::EndOfTurn));
    assert!(matches!(evaluation, EffectEvaluation::LiveObjects));
    assert!(matches!(
        changes.as_slice(),
        [Change::PlayerRule(PlayerRule::Prohibit(
            PlayerAction::CastSpell {
                from: None,
                filter: ObjectFilter::Any
            }
        ))]
    ));
    *evaluation = EffectEvaluation::Snapshot;
    assert!(validate::card(&silence)
        .unwrap_err()
        .contains("live subject queries"));
    let grid = card("Defense Grid");
    assert!(matches!(
        &grid.abilities[0],
        Ability::Static {
            affected: EffectSubjects::Players(PlayerSet::OthersThan(PlayerRef::Active)),
            ..
        }
    ));
}

#[test]
fn casting_modifiers_keep_colored_spill_floors_target_predicates_and_nonmana_costs() {
    let edge = card("Edgewalker");
    assert!(
        matches!(rules(&edge).as_slice(), [PlayerRule::SpellCost { modifier: SpellCostModifier::Reduce { reductions, spill_into_generic: false }, .. }] if reductions.len()==2 && reductions.iter().all(|r| r.color.is_some() && matches!(r.amount, Expr::Constant(1))))
    );
    let jet = card("Jet Medallion");
    assert!(
        matches!(rules(&jet).as_slice(), [PlayerRule::SpellCost { spells: ObjectFilter::Color(ManaColor::Black), modifier: SpellCostModifier::Reduce { reductions, spill_into_generic: true } }] if matches!(reductions.as_slice(), [ManaReduction { color: None, amount: Expr::Constant(1) }]))
    );
    let sphere = card("Trinisphere");
    assert!(matches!(&sphere.abilities[0], Ability::Conditional { .. }));
    assert!(matches!(
        rules(&sphere).as_slice(),
        [PlayerRule::SpellCost {
            modifier: SpellCostModifier::Minimum(Expr::Constant(3)),
            ..
        }]
    ));
    let terror = card("Terror of the Peaks");
    assert!(
        matches!(rules(&terror).as_slice(), [PlayerRule::SpellCost { spells: ObjectFilter::All(filters), modifier: SpellCostModifier::Increase(Cost { parts }) }] if filters.iter().any(|f| matches!(f,ObjectFilter::TargetsAny(targets) if matches!(targets.as_ref(), Recipients::Objects(ObjectSet::One(ObjectRef::Source))))) && matches!(parts.as_slice(), [CostPart::Life(Expr::Constant(3))]))
    );
}

#[test]
fn land_allowances_zone_permissions_and_hand_restrictions_remain_independent() {
    let azusa = card("Azusa, Lost but Seeking");
    assert!(matches!(
        rules(&azusa).as_slice(),
        [PlayerRule::AdditionalLandPlays(Count::Exactly(
            Expr::Constant(2)
        ))]
    ));
    let crucible = card("Crucible of Worlds");
    assert!(matches!(
        rules(&crucible).as_slice(),
        [PlayerRule::PlayPermission {
            cards: ObjectSet::Zone {
                scope: ZoneScope::Players {
                    zone: EventZone::Graveyard,
                    players: PlayerSet::One(PlayerRef::Affected)
                },
                ..
            },
            from: EventZone::Graveyard,
            action: CardPlayAction::PlayLand,
            ..
        }]
    ));
    let future = card("Future Sight");
    let permissions = rules(&future);
    assert!(permissions.iter().any(|r| matches!(
        r,
        PlayerRule::PlayPermission {
            from: EventZone::Library,
            action: CardPlayAction::CastSpell,
            cards: ObjectSet::Take {
                count: Expr::Constant(1),
                from: SequenceEnd::Top,
                ..
            },
            ..
        }
    )));
    assert!(!permissions.iter().any(|r| matches!(
        r,
        PlayerRule::SpellTiming { .. } | PlayerRule::AdditionalLandPlays(_)
    )));
    let frenzy = card("Experimental Frenzy");
    assert!(rules(&frenzy).iter().any(|r| matches!(
        r,
        PlayerRule::Prohibit(PlayerAction::CastSpell {
            from: Some(ZoneScope::Players {
                zone: EventZone::Hand,
                players: PlayerSet::One(PlayerRef::Affected)
            }),
            ..
        })
    )));
    assert!(rules(&frenzy).iter().any(|r| matches!(
        r,
        PlayerRule::Prohibit(PlayerAction::PlayLand {
            from: Some(ZoneScope::Players {
                zone: EventZone::Hand,
                players: PlayerSet::One(PlayerRef::Affected)
            }),
            ..
        })
    )));
    // A singular use permission needs its own usage accounting.
    assert!(custom("PlayerEffect(You,[MayCastASpellFromGraveyard(AnySpell,You)])").is_err());
}

#[test]
fn visibility_is_private_or_public_and_player_bindings_do_not_escape() {
    let oracle = card("Oracle of Mul Daya");
    assert!(rules(&oracle).iter().any(|r| matches!(
        r,
        PlayerRule::Visibility {
            audience: VisibilityAudience::Everyone,
            objects: ObjectSet::Take {
                count: Expr::Constant(1),
                ..
            }
        }
    )));
    let private = custom("PlayerEffect(You,[MayLookAtTopCardOfLibraryAnyTime])").unwrap();
    validate::card(&private).unwrap();
    assert!(matches!(
        rules(&private).as_slice(),
        [PlayerRule::Visibility {
            audience: VisibilityAudience::AffectedPlayer,
            ..
        }]
    ));
    let mut c = card("Telepathy");
    let Ability::Static { affected, .. } = &mut c.abilities[0] else {
        panic!()
    };
    *affected = EffectSubjects::Players(PlayerSet::One(PlayerRef::Affected));
    assert!(validate::card(&c).unwrap_err().contains("affected player"));
    let mut c = card("Silence");
    let Ability::Spell { body, .. } = &mut c.abilities[0] else {
        panic!()
    };
    body.instructions.push(Instruction::Effect {
        result: ResultId(99),
        operation: Operation::Draw {
            players: PlayerSet::One(PlayerRef::Affected),
            amount: Expr::Constant(1),
        },
    });
    assert!(validate::card(&c).unwrap_err().contains("affected player"));
}

#[test]
fn exact_exiled_incarnations_are_not_live_permissions_for_future_exiled_cards() {
    let c = card("Reckless Impulse");
    let [Instruction::Effect {
        result,
        operation:
            Operation::Move {
                destination: Destination::Exile,
                ..
            },
    }, Instruction::Effect {
        operation:
            Operation::CreateContinuousEffect {
                changes,
                duration: Duration::UntilEndOfNextTurn(PlayerRef::AbilityController),
                ..
            },
        ..
    }] = body(&c).instructions.as_slice()
    else {
        panic!()
    };
    assert!(
        matches!(changes.as_slice(), [Change::PlayerRule(PlayerRule::PlayPermission { cards: ObjectSet::Transition { result: bound, phase: ObjectPhase::After }, from: EventZone::Exile, action: CardPlayAction::Either, .. })] if bound.0 == result.0)
    );
    let broken = custom("SpellActions(ActionList([CreatePlayerEffectUntil(You,[MayPlayExiledCards(TheCardsExiledThisWay)],UntilEndOfTurn)]))").unwrap_err();
    assert!(broken.iter().any(|e| e.message.contains("no movement")));
}

#[test]
fn player_rules_reject_wrong_subjects_and_keep_life_hand_size_and_skip_semantics() {
    let angel = card("Platinum Angel");
    assert!(matches!(
        rules(&angel).as_slice(),
        [
            PlayerRule::Prohibit(PlayerAction::LoseGame { reason: None }),
            PlayerRule::Prohibit(PlayerAction::WinGame)
        ] | [
            PlayerRule::Prohibit(PlayerAction::WinGame),
            PlayerRule::Prohibit(PlayerAction::LoseGame { reason: None })
        ]
    ));
    let emperor = card("Platinum Emperion");
    assert!(matches!(
        rules(&emperor).as_slice(),
        [PlayerRule::LifeTotalLocked]
    ));
    let thought = card("Thought Eater");
    assert!(matches!(
        rules(&thought).as_slice(),
        [PlayerRule::HandSize(HandSizeRule::Adjust(Expr::Constant(
            -3
        )))]
    ));
    let stasis = card("Stasis");
    assert!(rules(&stasis)
        .iter()
        .any(|r| matches!(r, PlayerRule::Skip(TurnSection::Untap))));
    let mut c = card("Reliquary Tower");
    let Ability::Static { affected, .. } = &mut c.abilities[0] else {
        panic!()
    };
    *affected = EffectSubjects::Objects(ObjectSet::One(ObjectRef::Source));
    assert!(validate::card(&c)
        .unwrap_err()
        .contains("object/player subjects"));
    let mut c = card("Reliquary Tower");
    let Ability::Static { changes, .. } = &mut c.abilities[0] else {
        panic!()
    };
    changes.clear();
    assert!(validate::card(&c)
        .unwrap_err()
        .contains("empty continuous effect"));
}

#[test]
fn defending_player_comes_from_the_attacking_event_subject() {
    let c = custom(
        "TriggerA(WhenACreatureAttacks(SinglePermanent(ThisPermanent)), ActionList([PlayerAction(Trigger_DefendingPlayer, LoseLife(Integer(1)))]))",
    )
    .unwrap();
    validate::card(&c).unwrap();
    let json = serde_json::to_string(&c).unwrap();
    assert!(json.contains("DefendingPlayerOf"), "{json}");
    assert!(custom(
        "SpellActions(ActionList([PlayerAction(Trigger_DefendingPlayer, LoseLife(Integer(1)))]))"
    )
    .is_err());
}
