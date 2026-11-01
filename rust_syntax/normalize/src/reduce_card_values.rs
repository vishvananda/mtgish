// The values and predicates cards use inside other forms: numbers, filters,
// players and objects. Each reads the same parts Forge's `Count$` and `Valid`
// strings do, so a form that names one card still says what it is made of.

/// The colors a `ManaSpent` or `ManaSymbol` term names.
fn term_colors(term: &Operand) -> Option<Vec<ManaColor>> {
    match term {
        Operand::Term { form, operands, .. } if form == "Or" => {
            let mut colors = vec![];
            for operand in operands {
                colors.extend(term_colors(operand)?);
            }
            Some(colors)
        }
        Operand::Term { form, .. } | Operand::Vocabulary { form, .. } => {
            Some(vec![match form.as_str() {
                "ManaProduceW" | "ManaCostW" => ManaColor::White,
                "ManaProduceU" | "ManaCostU" => ManaColor::Blue,
                "ManaProduceB" | "ManaCostB" => ManaColor::Black,
                "ManaProduceR" | "ManaCostR" => ManaColor::Red,
                "ManaProduceG" | "ManaCostG" => ManaColor::Green,
                _ => return None,
            }])
        }
        _ => None,
    }
}

fn mana_of(mana: ManaOccurrence, term: &Operand) -> Option<Expr> {
    Some(Expr::ManaOf {
        mana,
        filter: SpentManaFilter {
            colors: Some(term_colors(term)?),
            snow: false,
        },
    })
}

fn some_exist(left: Expr) -> Condition {
    Condition::Compare {
        left,
        comparison: Comparison::GreaterOrEqual,
        right: Expr::Constant(1),
    }
}

/// The event and window a filter asks of the object it tests.
fn candidate_history(objects: &ObjectSet) -> Option<(ObjectEvent, HistoryWindow)> {
    let ObjectFilter::Condition(condition) = event_filter(objects)? else {
        return None;
    };
    let Condition::Compare {
        left: Expr::EventHistory { event, window, .. },
        ..
    } = *condition
    else {
        return None;
    };
    match *event {
        EventSpec::Object {
            event,
            objects: ObjectFilter::SameObject(ObjectRef::Candidate),
        } => Some((event, window)),
        _ => None,
    }
}

fn damage_to_player(sources: ObjectFilter, player: &PlayerRef) -> EventSpec {
    EventSpec::Damage(DamageEvent {
        source_kind: ObjectKind::Permanent,
        sources,
        recipients: DamageRecipients::Players(PlayerSet::One(player.clone())),
        kind: DamageKind::Any,
        grouping: DamageGrouping::SourceRecipient,
        subject: DamageSubject::Source,
    })
}

fn exiled_with_source() -> ObjectSet {
    ObjectSet::Zone {
        scope: ZoneScope::Shared(EventZone::Exile),
        kind: ObjectKind::Card,
        filter: ObjectFilter::ExiledWith(ObjectRef::Source),
    }
}

/// A number another form reads, reduced by what it counts.
fn reduce_value_number(form: &str, operands: &[Operand]) -> Option<Expr> {
    use Operand::{Objects, Player, Players, Result as Run};
    Some(match (form, operands) {
        // A cost's body names no result here, and the game keeps this value.
        ("TheAmountOfManaLostThisWay", []) => Expr::Game(GameValue::ManaLostThisWay),
        ("AManaValueOfAnExiledCard", [Objects(cards)]) => {
            aggregate(cards, Property::ManaValue, Aggregation::AnyOf)
        }
        ("AmountOfManaOfTypeSpentOnCumulativeUpkeep", [term]) => {
            mana_of(ManaOccurrence::SpentOnEventCost, term)?
        }
        ("TheAmountOfColorManaSpentOnX", [term]) => mana_of(ManaOccurrence::SpentOnX, term)?,
        ("PermanentItTargets", [Objects(objects)]) => {
            Expr::CountObjects(Box::new(ObjectSet::Matching {
                objects: Box::new(ObjectSet::TargetsOf(ObjectRef::Affected)),
                filter: event_filter(objects)?,
            }))
        }
        ("TheNumberOfCardTypesItSharesWithAnyExiledCard", [Objects(cards)]) => Expr::SharedValues {
            object: ObjectRef::Affected,
            property: DistinctProperty::CardTypes,
            with: Box::new(cards.clone()),
        },
        ("TheNumberOfCreaturesThatDealtCombatDamageToAPlayer", [Objects(objects), Players(players)]) => {
            Expr::EventHistory {
                event: Box::new(EventSpec::Damage(DamageEvent {
                    source_kind: ObjectKind::Permanent,
                    sources: event_filter(objects)?,
                    recipients: DamageRecipients::Players(players.clone()),
                    kind: DamageKind::Combat,
                    grouping: DamageGrouping::SourceRecipient,
                    subject: DamageSubject::Source,
                })),
                window: HistoryWindow::ThisTurn,
                measure: HistoryMeasure::DistinctSources,
            }
        }
        ("TheNumberOfRepeatedCostsNotPaidByPlayer", [Player(player), Run(result)]) => {
            Expr::RepeatedCostsUnpaid {
                result: result.clone(),
                player: player.clone(),
            }
        }
        _ => return None,
    })
}

/// What a `Comparison` row names for "equal to", or `None` for a form it can't state.
pub fn reduce_comparison_value(name: &str, operands: &[Operand]) -> Option<Expr> {
    use Operand::{Filter, Objects, Player, Players};
    let form = name.strip_prefix("Comparison::")?;
    Some(match (form, operands) {
        ("ANumberOfCardsInAPlayersGraveyard", [Filter(cards), Players(players)]) => {
            Expr::CountObjects(Box::new(zone_set(
                EventZone::Graveyard,
                PlayerSet::One(PlayerRef::OneOf(Box::new(players.clone()))),
                cards.clone(),
            )))
        }
        ("AnyManaValueAmongCardsInPlayersGraveyard", [Filter(cards), Player(owner)]) => aggregate(
            &cards_of(EventZone::Graveyard, owner, cards),
            Property::ManaValue,
            Aggregation::AnyOf,
        ),
        ("AnyManaValueAmongPermanents", [Objects(objects)]) => {
            aggregate(objects, Property::ManaValue, Aggregation::AnyOf)
        }
        _ => return None,
    })
}

/// A predicate on an object, spell or card.
fn reduce_value_filter(form: &str, operands: &[Operand]) -> Option<ObjectFilter> {
    use Operand::{Object, Player, Term};
    let shares_name_with_any = |objects| ObjectFilter::SharesNameWithAny(Box::new(objects));
    Some(match (form, operands) {
        ("TheColorChosenByItsController", []) => ObjectFilter::ChosenBy {
            kind: ValueKind::Color,
            player: PlayerRef::ControllerOf(Box::new(ObjectRef::Candidate)),
        },
        ("SharesANameWithACardSpliceOntoSpell", [Object(spell)]) => {
            shares_name_with_any(ObjectSet::SplicedOnto(spell.clone()))
        }
        ("SharesANameWithASpellCastThisTurn", []) => {
            shares_name_with_any(ObjectSet::FromHistory {
                event: Box::new(EventSpec::Spell {
                    event: SpellEvent::Cast,
                    players: PlayerSet::All,
                    spells: ObjectFilter::Any,
                }),
                window: HistoryWindow::ThisTurn,
            })
        }
        ("SharesANameWithAPermanentThatDealtDamageToPlayerLastTurn", [Player(player)]) => {
            shares_name_with_any(ObjectSet::FromHistory {
                event: Box::new(damage_to_player(ObjectFilter::Any, player)),
                window: HistoryWindow::LastTurn,
            })
        }
        ("ManaCostIsSubsetOfManaPaidForThisAbility", []) => {
            ObjectFilter::ManaCostPayableWith(ManaOccurrence::SpentOnX)
        }
        ("SharesAColorWithAColorOfManaSpendOnActivationCost", []) => {
            ObjectFilter::SharesColorWithMana(ManaOccurrence::SpentOnActivation)
        }
        ("SharesAColorWithAnyManaColorProduced", []) => {
            ObjectFilter::SharesColorWithMana(ManaOccurrence::ProducedByEvent)
        }
        ("IsntTheTargetOfAnAbility", [term]) => ObjectFilter::Not(Box::new(ObjectFilter::TargetedBy(
            Box::new(stack_filter(term)?),
        ))),
        ("WasCastFromAmongCardsPutIntoTheirHandThisTurn", []) => {
            history(ObjectHistory::PutIntoHand, ObjectWindow::ThisTurn)
        }
        // "Landwalk for each type among these" walks what shares a type with them.
        ("IsLandTypeVariable", [Term { form, operands, .. }])
            if form == "AnyBasicLandTypeAmongPermanents" =>
        {
            let [Operand::Objects(lands)] = operands.as_slice() else {
                return None;
            };
            ObjectFilter::SharesWithAny {
                property: DistinctProperty::BasicLandTypes,
                with: Box::new(lands.clone()),
            }
        }
        ("NotAColorInCommanderColorIdentity", []) => {
            ObjectFilter::ColorOutsideCommanderIdentity(PlayerRef::AbilityController)
        }
        ("TargetSpellOrAbility", [Term { form, .. }]) if form == "HasOneOrMoreTargets" => {
            ObjectFilter::TargetCount(Comparison::GreaterOrEqual, Expr::Constant(1))
        }
        _ => return None,
    })
}

/// A condition, under its whole row name: a `Players` row tests the player first.
fn reduce_value_condition(name: &str, operands: &[Operand]) -> Option<Condition> {
    use Operand::{Comparison as Than, Filter, Object, Objects, Player, Players, Term};
    let (family, form) = name.split_once("::")?;
    Some(match (family, form, operands) {
        ("Players", "AllowedToDrawCards", [Player(player)]) => Condition::PlayerState {
            player: player.clone(),
            state: PlayerState::CanDraw,
        },
        ("Players", "HasACardInHand", [Player(player), Term { form, operands, .. }])
            if form == "SingleCardInHand" =>
        {
            let [Object(card)] = operands.as_slice() else {
                return None;
            };
            Condition::ObjectMatches {
                object: card.clone(),
                filter: ObjectFilter::InZone(ZoneScope::Players {
                    zone: EventZone::Hand,
                    players: PlayerSet::One(player.clone()),
                }),
            }
        }
        ("Condition", "ThereAreNumberNamesAmongUnlockedDoorsOfPermanents", [Than(comparison, value), Objects(rooms)]) => {
            Condition::Compare {
                left: distinct(rooms, DistinctProperty::UnlockedDoorNames),
                comparison: comparison.clone(),
                right: value.clone(),
            }
        }
        ("Condition", "CombatDamageWasDealtByACreature", [Objects(creatures)]) => {
            some_exist(Expr::CountObjects(Box::new(ObjectSet::Matching {
                objects: Box::new(ObjectSet::Damage(DamageSubject::Source)),
                filter: event_filter(creatures)?,
            })))
        }
        // The spell the enclosing spell effect changes.
        ("Condition", "CastByAPlayer", [Players(players)]) => Condition::ObjectMatches {
            object: ObjectRef::Scoped,
            filter: ObjectFilter::Cast {
                by: Some(players.clone()),
                from: None,
            },
        },
        // "Whenever a player attacks": only that player's creatures attack this combat.
        ("Condition", "AttackingPlayerPassesFilter", [Term { form, operands, .. }])
            if form == "AttackedPlayerOrPlaneswalkerTheyControl" =>
        {
            let [Player(defender)] = operands.as_slice() else {
                return None;
            };
            some_exist(Expr::EventHistory {
                event: Box::new(EventSpec::Attack {
                    attackers: ObjectFilter::Any,
                    by: None,
                    defenders: AttackDefenders::PlayersAndPlaneswalkers(PlayerSet::One(
                        defender.clone(),
                    )),
                }),
                window: HistoryWindow::ThisCombat,
                measure: HistoryMeasure::Occurrences,
            })
        }
        // The copy `ChooseCopyFromCopies` picked.
        ("Condition", "CopiedCardPassesFilter", [Filter(filter)]) => Condition::ObjectMatches {
            object: ObjectRef::Chosen,
            filter: filter.clone(),
        },
        // Counted as events, so what attacked and has since left still counts.
        ("Condition", "NumCombatPermanentsPassFilter", [Than(comparison, value), Objects(objects), Objects(combat)]) => {
            let (event, window) = candidate_history(combat)?;
            Condition::Compare {
                left: Expr::EventHistory {
                    event: Box::new(EventSpec::Object {
                        event,
                        objects: event_filter(objects)?,
                    }),
                    window,
                    measure: HistoryMeasure::Occurrences,
                },
                comparison: comparison.clone(),
                right: value.clone(),
            }
        }
        // The event already says who activated it.
        ("Condition", "PlayerPaidANumberOfCountersToActivateIt", [Player(_), Than(comparison, value), kind]) => {
            Condition::Compare {
                left: Expr::CountersPaid {
                    object: ObjectRef::EventStackObject,
                    kind: counter_kind(kind)?,
                },
                comparison: comparison.clone(),
                right: value.clone(),
            }
        }
        _ => return None,
    })
}

/// A set of objects, named by what happened to them.
fn reduce_value_objects(form: &str, operands: &[Operand]) -> Option<ObjectSet> {
    use Operand::Objects;
    Some(match (form, operands) {
        ("WasExiledThisWay", []) => ObjectSet::Earlier {
            what: EarlierObjects::Exiled,
            by: None,
        },
        ("InThePermanentPileChosenThisWay", []) => {
            ObjectSet::Battlefield(ObjectFilter::InPile(None))
        }
        ("CreatureOrPlaneswalkerChosenAtRandom", [Objects(objects)]) => ObjectSet::Random {
            objects: Box::new(objects.clone()),
            count: Expr::Constant(1),
        },
        _ => return None,
    })
}

/// One object, picked out of a set.
fn reduce_value_object(form: &str, operands: &[Operand]) -> Option<ObjectRef> {
    use Operand::Objects;
    Some(match (form, operands) {
        ("TheOtherExiledCard", [Objects(named)]) => ObjectRef::TheOther {
            among: Box::new(exiled_with_source()),
            excluding: Box::new(named.clone()),
        },
        ("TopCardOfExiledPile", []) => ObjectRef::AtEnd {
            objects: Box::new(exiled_with_source()),
            end: SequenceEnd::Top,
        },
        _ => return None,
    })
}

/// A player, read off what happened earlier.
fn reduce_value_player(form: &str, operands: &[Operand]) -> Option<PlayerRef> {
    use Operand::{Filter, Object, Player};
    Some(match (form, operands) {
        ("ControllerOfLastSpellThatDealtDamageToPlayerThisTurn", [Filter(spells), Player(player)]) => {
            let EventSpec::Damage(damage) = damage_to_player(spells.clone(), player) else {
                return None;
            };
            PlayerRef::ControllerOf(Box::new(ObjectRef::AtEnd {
                objects: Box::new(ObjectSet::FromHistory {
                    event: Box::new(EventSpec::Damage(DamageEvent {
                        source_kind: ObjectKind::Spell,
                        ..damage
                    })),
                    window: HistoryWindow::ThisTurn,
                }),
                end: SequenceEnd::Bottom,
            }))
        }
        (
            "ControllerOfPermanentTheLastTimeItWasBlockedByPermanentThisTurn",
            [Object(blocked), Object(blocker)],
        ) => PlayerRef::AsOf {
            moment: Box::new(Moment::LastEvent {
                event: Box::new(EventSpec::Pair {
                    event: PairEvent::BecomesBlockedBy,
                    objects: ObjectFilter::SameObject(blocked.clone()),
                    others: ObjectFilter::SameObject(blocker.clone()),
                }),
                window: HistoryWindow::ThisTurn,
            }),
            player: Box::new(PlayerRef::ControllerOf(Box::new(blocked.clone()))),
        },
        _ => return None,
    })
}
