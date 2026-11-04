// Conditions on players and the turn, and players a row names. "Did this
// happen to them in that window" is one shape, as Forge's `PlayerCountPropertyX`.

fn at_least(left: Expr, amount: i64) -> Condition {
    Condition::Compare {
        left,
        comparison: Comparison::GreaterOrEqual,
        right: Expr::Constant(amount),
    }
}

fn exactly(left: Expr, amount: i64) -> Condition {
    Condition::Compare {
        left,
        comparison: Comparison::Equal,
        right: Expr::Constant(amount),
    }
}

fn compared(left: Expr, comparison: &Comparison, right: &Expr) -> Condition {
    Condition::Compare {
        left,
        comparison: comparison.clone(),
        right: right.clone(),
    }
}

fn negated(condition: Condition) -> Condition {
    Condition::Not(Box::new(condition))
}

fn did(player: &PlayerRef, event: PlayerHistory, window: ObjectWindow) -> Condition {
    Condition::PlayerDid {
        player: player.clone(),
        event,
        window,
    }
}

fn count_of(objects: ObjectSet) -> Expr {
    Expr::CountObjects(Box::new(objects))
}

fn controlled_by(player: &PlayerRef, filter: ObjectFilter) -> ObjectSet {
    ObjectSet::battlefield(ObjectFilter::All(vec![
        filter,
        ObjectFilter::ControlledBy(PlayerSet::One(player.clone())),
    ]))
}

fn filtered(objects: &ObjectSet, filter: ObjectFilter) -> ObjectSet {
    ObjectSet::Matching {
        objects: Box::new(objects.clone()),
        filter,
    }
}

fn permanents_of(player: &PlayerRef, filter: ObjectFilter) -> Expr {
    Expr::PlayerNumber {
        player: player.clone(),
        number: PlayerNumber::Permanents(Box::new(filter)),
    }
}

fn player_state(player: &PlayerRef, state: PlayerState) -> Condition {
    Condition::PlayerState {
        player: player.clone(),
        state,
    }
}

fn player_check(form: &str, player: &PlayerRef, operands: &[Operand]) -> Option<Condition> {
    use DistinctProperty as D;
    use Operand::{Comparison as Than, Filter, Object, Objects, Player, Players};
    use PlayerHistory as H;
    use ObjectWindow as W;
    let last_turn = || W::LastTurnOf(player.clone());
    let filter_of = |objects: &ObjectSet| event_filter(objects);
    let set = |filter: &ObjectFilter| Box::new(filter.clone());
    Some(match (form, operands) {
        ("AttackedWithCreaturesWithTotalPowerThisCombat", [Than(comparison, value)]) => did(
            player,
            H::AttackedWithTotalPower(comparison.clone(), value.clone()),
            W::ThisCombat,
        ),
        ("AttackedByCreatureThisTurn", [Object(object)]) => did(
            player,
            H::AttackedBy(Box::new(ObjectFilter::SameObject(object.clone()))),
            W::ThisTurn,
        ),
        ("AttackedByACreatureThisTurn", [Objects(objects)]) => {
            did(player, H::AttackedBy(Box::new(filter_of(objects)?)), W::ThisTurn)
        }
        ("AttackedWithCreatureThisTurn", [Object(object)]) => did(
            player,
            H::AttackedWith(Box::new(ObjectFilter::SameObject(object.clone()))),
            W::ThisTurn,
        ),
        ("AttackedPlayerDuringTheirLastTurn", [Player(other)]) => {
            did(player, H::AttackedPlayer(other.clone()), last_turn())
        }
        ("AttackedAPlaneswalkerThisTurn", [Objects(objects)]) => {
            did(player, H::AttackedDefender(Box::new(filter_of(objects)?)), W::ThisTurn)
        }
        ("WasntAttackedByCreatureDuringPlayersLastCombat", [Object(object), Player(other)]) => {
            negated(did(
                player,
                H::AttackedBy(Box::new(ObjectFilter::SameObject(object.clone()))),
                W::LastCombatOf(other.clone()),
            ))
        }
        ("CastASpellDuringTheirLastTurn", [Filter(spells)]) => {
            did(player, H::CastSpell(set(spells)), last_turn())
        }
        ("CastASpellSincePlayersLastTurnEnded", [Filter(spells), Player(other)]) => {
            did(player, H::CastSpell(set(spells)), W::SinceLastTurnOf(other.clone()))
        }
        ("CastASpellFromAGraveyardThisTurn", [Filter(spells), Players(owners)]) => did(
            player,
            H::CastSpell(Box::new(ObjectFilter::All(vec![
                spells.clone(),
                ObjectFilter::Cast {
                    by: None,
                    from: Some(ZoneScope::Players {
                        zone: EventZone::Graveyard,
                        players: owners.clone(),
                    }),
                },
            ]))),
            W::ThisTurn,
        ),
        ("CastASpellFromAnywhereOtherThanTheirHandThisTurn", [Filter(spells)]) => did(
            player,
            H::CastSpell(Box::new(ObjectFilter::All(vec![
                spells.clone(),
                not(cast_from(EventZone::Hand)),
            ]))),
            W::ThisTurn,
        ),
        ("PlayedALandFromAnywhereOtherThanTheirHandThisTurn", []) => {
            did(player, H::PlayedLandNotFromHand, W::ThisTurn)
        }
        ("DidntPlayACardFromExileThisTurn", [Filter(cards)]) => {
            negated(did(player, H::PlayedFromExile(set(cards)), W::ThisTurn))
        }
        ("PutAPermanentOnBattleDuringTheirLastTurn", [Objects(objects)])
        | ("HadAPermanentEnterTheBattlefieldUnderTheirControlLastTurn", [Objects(objects)]) => {
            did(player, H::PermanentEntered(Box::new(filter_of(objects)?)), last_turn())
        }
        ("HasActivatedAnAbilityOfAGraveyardCardThisTurn", [Players(_)]) => {
            did(player, H::ActivatedGraveyardAbility, W::ThisTurn)
        }
        ("HasNotActivatedAnExhaustAbilityThisTurn", []) => {
            negated(did(player, H::ActivatedExhaust, W::ThisTurn))
        }
        ("HasntBeenDealtCombatDamageSinceTheirLastTurn", []) => negated(did(
            player,
            H::WasDealtCombatDamage,
            W::SinceLastTurnOf(player.clone()),
        )),
        ("PaidOrLostAnAmountOfEnergyThisTurn", [Than(comparison, value)]) => did(
            player,
            H::PaidOrLostEnergy(comparison.clone(), value.clone()),
            W::ThisTurn,
        ),
        ("CycledANumberOfCardsThisTurn", [Than(comparison, value), Filter(cards)]) => did(
            player,
            H::Cycled(comparison.clone(), value.clone(), set(cards)),
            W::ThisTurn,
        ),
        ("CycledANumberOfCardsThisGame", [Than(comparison, value), Filter(cards)]) => did(
            player,
            H::Cycled(comparison.clone(), value.clone(), set(cards)),
            W::ThisGame,
        ),
        ("RolledADiceValueThisTurn", [Than(comparison, value)]) => did(
            player,
            H::RolledDie(comparison.clone(), value.clone()),
            W::ThisTurn,
        ),
        ("WasTheMonarchAsTheTurnBegan", []) => did(player, H::WasMonarch, W::ThisTurn),
        ("HasWaterEarthFireAndAirBendedThisTurn", []) => {
            did(player, H::BentAllFour, W::ThisTurn)
        }
        ("HasBeenTemptedByTheRingNumberTimes", [Than(comparison, value)]) => compared(
            Expr::PlayerNumber {
                player: player.clone(),
                number: PlayerNumber::RingTemptations,
            },
            comparison,
            value,
        ),
        ("SpeedIs", [Than(comparison, value)]) => compared(
            Expr::PlayerNumber {
                player: player.clone(),
                number: PlayerNumber::Speed,
            },
            comparison,
            value,
        ),
        ("HasMaxSpeed", []) => player_state(player, PlayerState::MaxSpeed),
        ("DoesntHaveMaxSpeed", []) => negated(player_state(player, PlayerState::MaxSpeed)),
        ("IsNotTheMonarch", []) => negated(player_state(player, PlayerState::Monarch)),
        ("IsntBeingAttacked", []) => negated(player_state(player, PlayerState::BeingAttacked)),
        ("ReceivedAVote", []) => player_state(player, PlayerState::ReceivedAVote),
        ("DidntReceiveAVote", []) => negated(player_state(player, PlayerState::ReceivedAVote)),
        ("HasABoon", []) => player_state(player, PlayerState::HasBoon),
        ("IsAttackedByPlayer", [Player(attacker)]) => Condition::PlayerAttacks {
            attacker: attacker.clone(),
            defender: player.clone(),
        },
        ("ControlsPermanent", [Object(object)]) => Condition::ObjectMatches {
            object: object.clone(),
            filter: ObjectFilter::ControlledBy(PlayerSet::One(player.clone())),
        },
        ("DoesntControlPermanent", [Object(object)]) => negated(Condition::ObjectMatches {
            object: object.clone(),
            filter: ObjectFilter::ControlledBy(PlayerSet::One(player.clone())),
        }),
        ("IsProtectingBattle", [Object(object)]) => Condition::ObjectMatches {
            object: object.clone(),
            filter: ObjectFilter::ProtectedBy(PlayerSet::One(player.clone())),
        },
        ("SpentManaToCastSpell", [Object(object)]) => {
            let (comparison, value) = at_least_one();
            Condition::ObjectMatches {
                object: object.clone(),
                filter: ObjectFilter::ManaSpent {
                    color: None,
                    sources: None,
                    payers: Some(PlayerSet::One(player.clone())),
                    comparison,
                    value,
                },
            }
        }
        ("OwnsAPermanent", [Objects(objects)]) => at_least(
            count_of(filtered(
                objects,
                ObjectFilter::OwnedBy(PlayerSet::One(player.clone())),
            )),
            1,
        ),
        ("ControlsAll", [Objects(objects)]) => exactly(
            count_of(filtered(
                objects,
                not(ObjectFilter::ControlledBy(PlayerSet::One(player.clone()))),
            )),
            0,
        ),
        ("NumCardsOwnedInExileIs", [Than(comparison, value), Objects(objects)]) => compared(
            count_of(filtered(
                objects,
                ObjectFilter::OwnedBy(PlayerSet::One(player.clone())),
            )),
            comparison,
            value,
        ),
        ("HasNoCardsOfTypeInHand", [Filter(cards)]) => {
            exactly(count_of(cards_of(EventZone::Hand, player, cards)), 0)
        }
        ("HasNoCardsOfTypeInLibrary", [Filter(cards)]) => {
            exactly(count_of(cards_of(EventZone::Library, player, cards)), 0)
        }
        ("HasANumberOfCardsAmongCardsInGraveyardHandAndLibrary", [Than(comparison, value), Filter(cards)]) => {
            compared(
                count_of(ObjectSet::Union(
                    [EventZone::Graveyard, EventZone::Hand, EventZone::Library]
                        .into_iter()
                        .map(|zone| cards_of(zone, player, cards))
                        .collect(),
                )),
                comparison,
                value,
            )
        }
        ("ControlsNumThatShareAName", [Than(comparison, value), Objects(objects)]) => compared(
            Expr::LargestSharedGroup {
                objects: Box::new(controlled_by(player, filter_of(objects)?)),
                property: D::Names,
            },
            comparison,
            value,
        ),
        ("ControlsNumThatShareACreatureType", [Than(comparison, value), Objects(objects)]) => {
            compared(
                Expr::LargestSharedGroup {
                    objects: Box::new(controlled_by(player, filter_of(objects)?)),
                    property: D::CreatureTypes,
                },
                comparison,
                value,
            )
        }
        ("ControlsNumColorsOfPermanents", [Than(comparison, value)]) => compared(
            Expr::CountDistinct {
                objects: Box::new(controlled_by(player, ObjectFilter::Any)),
                property: D::Colors,
            },
            comparison,
            value,
        ),
        ("ControlsNumBasicLandTypes", [Than(comparison, value), Objects(objects)]) => compared(
            Expr::CountDistinct {
                objects: Box::new(controlled_by(player, filter_of(objects)?)),
                property: D::BasicLandTypes,
            },
            comparison,
            value,
        ),
        ("ControlsAPermanentOfEachColor", [Objects(objects)]) => exactly(
            Expr::CountDistinct {
                objects: Box::new(controlled_by(player, filter_of(objects)?)),
                property: D::Colors,
            },
            5,
        ),
        ("ControlsALandOfEachBasicLandType", []) => exactly(
            Expr::CountDistinct {
                objects: Box::new(controlled_by(
                    player,
                    ObjectFilter::CardType(crate::mtg_types::CardType::Land),
                )),
                property: D::BasicLandTypes,
            },
            5,
        ),
        ("ControlsLessPermanentsThanEachPlayer", [Players(others), Objects(objects)]) => {
            let filter = filter_of(objects)?;
            Condition::Compare {
                left: permanents_of(player, filter.clone()),
                comparison: Comparison::Less,
                right: among(
                    others,
                    PlayerNumber::Permanents(Box::new(filter)),
                    Aggregation::Minimum,
                ),
            }
        }
        ("AControllerOfTheMostPermanentsAmongPlayers", [Players(others), Objects(objects)]) => {
            let filter = filter_of(objects)?;
            Condition::Compare {
                left: permanents_of(player, filter.clone()),
                comparison: Comparison::GreaterOrEqual,
                right: among(
                    others,
                    PlayerNumber::Permanents(Box::new(filter)),
                    Aggregation::Maximum,
                ),
            }
        }
        ("AControllerOfTheLeastPermanentsAmongPlayers", [Players(others), Objects(objects)]) => {
            let filter = filter_of(objects)?;
            Condition::Compare {
                left: permanents_of(player, filter.clone()),
                comparison: Comparison::LessOrEqual,
                right: among(
                    others,
                    PlayerNumber::Permanents(Box::new(filter)),
                    Aggregation::Minimum,
                ),
            }
        }
        _ => return None,
    })
}

fn turn_window(form: &str) -> Option<TurnWindow> {
    use TurnWindow as T;
    Some(match form {
        "IsDuringAMainPhase" => T::MainPhase,
        "IsBeforeCombat" => T::BeforeCombat,
        "IsAfterCombat" => T::AfterCombat,
        "IsDuringEndOfCombatStep" => T::EndOfCombatStep,
        "IsBeforeEndOfCombatStep" => T::BeforeEndOfCombatStep,
        "IsBeforeEndStep" => T::BeforeEndStep,
        "IsAfterUpkeep" => T::AfterUpkeep,
        "IsBeforeFirstCombatPhaseOfTurn" => T::BeforeFirstCombat,
        "IsDuringUntapStep" => T::UntapStep,
        "IsDuringDrawStep" => T::DrawStep,
        "IsFirstEndStepOfTheTurn" => T::FirstEndStep,
        "IsAnExtraTurn" => T::ExtraTurn,
        _ => return None,
    })
}

fn reduce_game_condition(form: &str, operands: &[Operand]) -> Option<Condition> {
    use DistinctProperty as D;
    use Operand::{Comparison as Than, Filter, Object, Objects, Player, Players, Result as Run};
    if operands.is_empty() {
        if let Some(window) = turn_window(form) {
            return Some(Condition::Window(window));
        }
    }
    Some(match (form, operands) {
        ("AColorWasChosen", []) => Condition::ValueChosen(ValueKind::Color),
        ("ACreatureTypeWasChosen", []) => Condition::ValueChosen(ValueKind::CreatureType),
        ("IsTheNthTurnOfTheGame", [Than(comparison, value)]) => {
            compared(Expr::TurnNumber, comparison, value)
        }
        ("IsNotTheFirstTurnOfTheGame", []) => at_least(Expr::TurnNumber, 2),
        ("IsPlayersNthTurn", [Player(player), Than(comparison, value)]) => compared(
            Expr::PlayerNumber {
                player: player.clone(),
                number: PlayerNumber::TurnsBegun,
            },
            comparison,
            value,
        ),
        ("ACombatPermanentPassesFilter", [Object(object), Objects(objects)]) => {
            Condition::ObjectMatches {
                object: object.clone(),
                filter: event_filter(objects)?,
            }
        }
        ("AttackingCreaturesPassFilter", [Than(comparison, value), Objects(objects)]) => compared(
            count_of(filtered(objects, ObjectFilter::Attacking)),
            comparison,
            value,
        ),
        ("NumPlayersPassFilter", [Than(comparison, value), Players(among), Players(passing)]) => {
            compared(
                Expr::CountPlayers(PlayerSet::Intersection(vec![
                    among.clone(),
                    passing.clone(),
                ])),
                comparison,
                value,
            )
        }
        ("AnyCardInAnyPlayersGraveyardPassesFilter", [Filter(cards), Players(players)]) => {
            at_least(
                count_of(ObjectSet::Zone {
                    scope: ZoneScope::Players {
                        zone: EventZone::Graveyard,
                        players: players.clone(),
                    },
                    kind: ObjectKind::Card,
                    filter: cards.clone(),
                }),
                1,
            )
        }
        ("TopCardOfPlayersLibraryPassesFilter_Digital", [Player(owner), Filter(cards)]) => {
            at_least(
                count_of(ObjectSet::Matching {
                    objects: Box::new(ObjectSet::Take {
                        objects: Box::new(cards_of(EventZone::Library, owner, &ObjectFilter::Any)),
                        count: Expr::Constant(1),
                        from: SequenceEnd::Top,
                    }),
                    filter: cards.clone(),
                }),
                1,
            )
        }
        ("ThereAreANumberOfBasicLandTypesAmongPermanents", [Than(comparison, value), Objects(objects)]) => {
            compared(
                Expr::CountDistinct {
                    objects: Box::new(objects.clone()),
                    property: D::BasicLandTypes,
                },
                comparison,
                value,
            )
        }
        ("ThereAreANumberOfPermanentTypesAmongCardsInPlayersGraveyards", [Than(comparison, value), Players(players)]) => {
            compared(
                Expr::CountDistinct {
                    objects: Box::new(ObjectSet::Zone {
                        scope: ZoneScope::Players {
                            zone: EventZone::Graveyard,
                            players: players.clone(),
                        },
                        kind: ObjectKind::Card,
                        filter: ObjectFilter::Any,
                    }),
                    property: D::PermanentTypes,
                },
                comparison,
                value,
            )
        }
        // What a preceding instruction named.
        ("ACardWasChosenThisWay" | "APermanentWasChosenThisWay" | "PermanentsChangedControlThisWay", [Run(result)]) => {
            at_least(count_of(named_results(result)), 1)
        }
        ("ANumberWasChosenThisWay" | "EnergyWasPaidThisWay", [Run(result)]) => {
            at_least(Expr::Performed(result.clone()), 1)
        }
        ("AnAmountOfEnergyWasPaidThisWay", [Than(comparison, value), Run(result)]) => {
            compared(Expr::Performed(result.clone()), comparison, value)
        }
        ("ANumberOfTokensWereCreatedThisWay", [Than(comparison, value), Run(result)]) => {
            compared(count_of(named_results(result)), comparison, value)
        }
        ("ASpellWasCastThisWay", [Filter(filter), Run(result)]) => {
            at_least(count_of(matching_results(result, filter.clone())), 1)
        }
        ("APermanentWasCopiedThisWay" | "APermanentsAbilityIsCounteredThisWay", [Objects(objects), Run(result)]) => {
            at_least(count_of(matching_results(result, event_filter(objects)?)), 1)
        }
        _ => return None,
    })
}

fn reduce_player_condition(name: &str, operands: &[Operand]) -> Option<Condition> {
    let (family, form) = name.split_once("::")?;
    match (family, operands) {
        ("Players", [Operand::Player(player), rest @ ..]) => {
            player_check(form, player, rest)
                .or_else(|| player_fact(form, player, rest))
                .or_else(|| more_player_fact(form, player, rest))
        }
        ("Condition", _) => {
            reduce_game_condition(form, operands).or_else(|| as_cast(form, operands))
        }
        _ => None,
    }
}

/// A player a row names.
pub fn reduce_player(form: &str, operands: &[Operand]) -> Option<PlayerRef> {
    use Operand::{Object, Objects, Player};
    let most = |number| PlayerRef::Extreme {
        players: Box::new(PlayerSet::All),
        number,
        highest: true,
    };
    let seat = |of: &PlayerRef, side| PlayerRef::Seat {
        of: Box::new(of.clone()),
        side,
    };
    Some(match (form, operands) {
        ("PlayerToTheLeftOf", [Player(player)]) => seat(player, SeatSide::Left),
        ("PlayerToTheRightOf", [Player(player)]) => seat(player, SeatSide::Right),
        ("OpponentToTheLeftOfYou", []) => seat(&PlayerRef::AbilityController, SeatSide::Left),
        ("NextOpponentInTurnOrder", []) => seat(&PlayerRef::AbilityController, SeatSide::Next),
        ("AttackingPlayer" | "TheAttackingPlayer", []) => PlayerRef::AttackingPlayer,
        ("DefendingPlayer" | "Trigger_DefendingPlayer", []) => PlayerRef::DefendingPlayer,
        ("TheMonarch", []) => PlayerRef::Holder(Designation::Monarch),
        ("ThePlayerWithTheInitiative", []) => PlayerRef::Holder(Designation::Initiative),
        ("ThePlayerWithTheMostLife", []) => most(PlayerNumber::Life),
        ("ThePlayerWithTheMostCardsInHand", []) => most(PlayerNumber::CardsInHand),
        ("ThePlayerWhoControlsTheMostPermanents", [Objects(objects)]) => {
            most(PlayerNumber::Permanents(Box::new(event_filter(objects)?)))
        }
        ("ControllerOfAbility" | "ControllerOfTriggeredAbility", [Object(object)]) => {
            PlayerRef::ControllerOf(Box::new(object.clone()))
        }
        ("SingleTargetPlayerOfSpell", [Object(object)]) => {
            PlayerRef::SoleTargetOf(Box::new(object.clone()))
        }
        _ => return None,
    })
}
