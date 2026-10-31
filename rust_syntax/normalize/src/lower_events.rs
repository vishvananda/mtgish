// Event matching is independent of the triggered ability's resolution program.
impl Context {
    fn future_turn(
        &self,
        value: &source::FutureTrigger,
    ) -> Result<(EventSpec, DelayedStart, Option<Duration>)> {
        use source::FutureTrigger as S;
        let (point, players, start, expires) = match value {
            S::AtTheBeginningOfTheNextEndStep => (
                TurnPoint::EndStepBegins,
                PlayerSet::All,
                DelayedStart::Immediately,
                None,
            ),
            S::AtTheNextEndOfCombat => (
                TurnPoint::CombatEnds,
                PlayerSet::All,
                DelayedStart::Immediately,
                None,
            ),
            S::AtTheEndOfThisCombat => (
                TurnPoint::CombatEnds,
                PlayerSet::All,
                DelayedStart::Immediately,
                Some(Duration::EndOfCombat),
            ),
            S::AtNextEndOfCombatThisTurn => (
                TurnPoint::CombatEnds,
                PlayerSet::All,
                DelayedStart::Immediately,
                Some(Duration::EndOfTurn),
            ),
            S::AtTheBeginningOfTheNextUpkeep => (
                TurnPoint::UpkeepBegins,
                PlayerSet::All,
                DelayedStart::Immediately,
                None,
            ),
            S::AtTheBeginningOfTheNextTurnsUpkeep => (
                TurnPoint::UpkeepBegins,
                PlayerSet::All,
                DelayedStart::NextTurn,
                None,
            ),
            S::AtTheBeginningOfTheNextCleanupStep => (
                TurnPoint::CleanupStepBegins,
                PlayerSet::All,
                DelayedStart::Immediately,
                None,
            ),
            S::AtTheBeginningOfTheNextCombat => (
                TurnPoint::CombatBegins,
                PlayerSet::All,
                DelayedStart::Immediately,
                None,
            ),
            S::AtTheBeginningOfTheNextCombatPhaseThisTurn => (
                TurnPoint::CombatBegins,
                PlayerSet::All,
                DelayedStart::Immediately,
                Some(Duration::EndOfTurn),
            ),
            S::AtTheBeginningOfTheNextMainPhaseThisTurn => (
                TurnPoint::MainPhaseBegins,
                PlayerSet::All,
                DelayedStart::Immediately,
                Some(Duration::EndOfTurn),
            ),
            S::AtTheBeginningOfPlayersNextEndStep(p)
            | S::AtTheBeginningOfPlayersNextUpkeep(p)
            | S::AtTheBeginningOfPlayersNextDrawStep(p)
            | S::AtTheBeginningOfPlayersNextMainPhase(p)
            | S::AtTheBeginningOfPlayersNextFirstMainPhase(p)
            | S::AtTheBeginningOfPlayersNextDeclareAttackersStep(p)
            | S::AtTheBeginningOfPlayersDeclareAttackersStepOnTheirNextTurn(p)
            | S::AtTheBeginningOfPlayersEndStepNextTurn(p) => {
                let player = self.player(p)?;
                let start = if matches!(
                    value,
                    S::AtTheBeginningOfPlayersDeclareAttackersStepOnTheirNextTurn(_)
                        | S::AtTheBeginningOfPlayersEndStepNextTurn(_)
                ) {
                    DelayedStart::PlayersNextTurn(player.clone())
                } else {
                    DelayedStart::Immediately
                };
                let point = match value {
                    S::AtTheBeginningOfPlayersNextEndStep(_)
                    | S::AtTheBeginningOfPlayersEndStepNextTurn(_) => TurnPoint::EndStepBegins,
                    S::AtTheBeginningOfPlayersNextUpkeep(_) => TurnPoint::UpkeepBegins,
                    S::AtTheBeginningOfPlayersNextDrawStep(_) => TurnPoint::DrawStepBegins,
                    S::AtTheBeginningOfPlayersNextMainPhase(_) => TurnPoint::MainPhaseBegins,
                    S::AtTheBeginningOfPlayersNextFirstMainPhase(_) => {
                        TurnPoint::FirstMainPhaseBegins
                    }
                    _ => TurnPoint::DeclareAttackersStepBegins,
                };
                (point, PlayerSet::One(player), start, None)
            }
            _ => return Err(unsupported(value)),
        };
        Ok((EventSpec::Turn { point, players }, start, expires))
    }

    fn trigger(&self, value: &source::Trigger) -> Result<EventSpec> {
        self.reduced_trigger(value)
            .or_else(|error| rescue(error, || self.primitive_trigger(value)))
    }

    fn reduced_trigger(&self, value: &source::Trigger) -> Result<EventSpec> {
        use source::Trigger as S;
        Ok(match value {
            S::WhenAPlayerCyclesACard(players, cards)
            | S::WhenAPlayerCyclesOrDiscardsACard(players, cards)
            | S::WhenAPlayerDiscardsACard(players, cards)
            | S::WhenAPlayerDiscardsAnyNumberOfCards(players, cards) => {
                let source_only = matches!(
                    cards.as_ref(),
                    source::CardsInHand::SingleCardInHand(source::CardInHand::ThisCardInHand)
                );
                EventSpec::Discard {
                    players: self.players(players)?,
                    cards: if source_only {
                        ObjectFilter::Any
                    } else {
                        self.hand_filter(cards)?
                    },
                    source_only,
                    kind: if matches!(value, S::WhenAPlayerCyclesACard(..)) {
                        DiscardKind::Cycling
                    } else {
                        DiscardKind::Any
                    },
                    grouping: if matches!(value, S::WhenAPlayerDiscardsAnyNumberOfCards(..)) {
                        DiscardGrouping::Player
                    } else {
                        DiscardGrouping::Card
                    },
                }
            }
            S::WhenAPermanentHasNumberCountersOfType(objects, comparison, kind) => {
                let source::Permanents::SinglePermanent(object) = objects.as_ref() else {
                    return Err(unsupported(value));
                };
                let (comparison, value) = self.comparison(comparison)?;
                EventSpec::State {
                    condition: Condition::ObjectMatches {
                        object: self.object(object)?,
                        filter: ObjectFilter::Counters {
                            kind: Some(crate::counter::lower(kind)),
                            comparison,
                            value,
                        },
                    },
                }
            }
            S::WhenAPermanentEntersTheBattlefield(objects) => EventSpec::ZoneChange {
                objects: self.filter(objects)?,
                from: None,
                to: EventZone::Battlefield,
            },
            S::WhenAPermanentDies(objects) => EventSpec::ZoneChange {
                objects: self.filter(objects)?,
                from: Some(EventZone::Battlefield),
                to: EventZone::Graveyard,
            },
            S::WhenAPermanentIsPutIntoAPlayersGraveyard(objects, players) => {
                EventSpec::ZoneChange {
                    // A permanent is put into its owner's graveyard (CR 400.3).
                    // This is a battlefield departure, not a card's "from anywhere" trigger.
                    objects: ObjectFilter::All(vec![
                        self.filter(objects)?,
                        ObjectFilter::OwnedBy(self.players(players)?),
                    ]),
                    from: Some(EventZone::Battlefield),
                    to: EventZone::Graveyard,
                }
            }
            S::WhenACardIsPutIntoAPlayersGraveyardFromAnywhere(cards, owners) => {
                EventSpec::ZoneChange {
                    objects: ObjectFilter::All(vec![
                        self.card_filter(cards)?,
                        ObjectFilter::OwnedBy(self.players(owners)?),
                        ObjectFilter::Not(Box::new(ObjectFilter::Token)),
                    ]),
                    from: None,
                    to: EventZone::Graveyard,
                }
            }
            S::WhenACardIsPutIntoAPlayersGraveyardFromTheirLibrary(cards, owners) => {
                EventSpec::ZoneChange {
                    objects: ObjectFilter::All(vec![
                        self.card_filter(cards)?,
                        ObjectFilter::OwnedBy(self.players(owners)?),
                    ]),
                    from: Some(EventZone::Library),
                    to: EventZone::Graveyard,
                }
            }
            S::WhenAPermanentLeavesTheBattlefield(objects) => EventSpec::Object {
                event: ObjectEvent::LeavesBattlefield,
                objects: self.filter(objects)?,
            },
            S::WhenAPermanentEntersTheBattlefieldFromExile(objects) => EventSpec::ZoneChange {
                objects: self.filter(objects)?,
                from: Some(EventZone::Exile),
                to: EventZone::Battlefield,
            },
            S::WhenACreatureAttacks(objects)
            | S::WhenACreatureAttacksAlone(objects)
            | S::WhenACreatureBlocks(objects)
            | S::WhenACreatureBecomesBlocked(objects)
            | S::WhenACreatureAttacksAndIsntBlocked(objects)
            | S::WhenAPermanentBecomesTapped(objects)
            | S::WhenAPermanentBecomesUntapped(objects)
            | S::WhenAPermanentIsTurnedFaceUp(objects) => EventSpec::Object {
                event: match value {
                    S::WhenACreatureAttacks(_) => ObjectEvent::Attacks,
                    S::WhenACreatureAttacksAlone(_) => ObjectEvent::AttacksAlone,
                    S::WhenACreatureBlocks(_) => ObjectEvent::Blocks,
                    S::WhenACreatureBecomesBlocked(_) => ObjectEvent::BecomesBlocked,
                    S::WhenACreatureAttacksAndIsntBlocked(_) => ObjectEvent::AttacksAndIsNotBlocked,
                    S::WhenAPermanentBecomesTapped(_) => ObjectEvent::BecomesTapped,
                    S::WhenAPermanentBecomesUntapped(_) => ObjectEvent::BecomesUntapped,
                    S::WhenAPermanentIsTurnedFaceUp(_) => ObjectEvent::TurnsFaceUp,
                    _ => unreachable!(),
                },
                objects: self.filter(objects)?,
            },
            S::WhenAPlayerCastsASpell(players, spells)
            | S::WhenAPlayerCopiesASpell(players, spells) => EventSpec::Spell {
                event: if matches!(value, S::WhenAPlayerCopiesASpell(..)) {
                    SpellEvent::Copy
                } else {
                    SpellEvent::Cast
                },
                players: self.players(players)?,
                spells: self.spell_filter(spells)?,
            },
            S::WhenASpellIsCast(spells) => EventSpec::Spell {
                event: SpellEvent::Cast,
                players: PlayerSet::All,
                spells: self.spell_filter(spells)?,
            },
            S::AtTheBeginningOfAPlayersUpkeep(players)
            | S::AtTheBeginningOfAPlayersDrawStep(players)
            | S::AtTheBeginningOfAPlayersEndStep(players)
            | S::AtTheBeginningOfAPlayersMainPhases(players)
            | S::AtTheBeginningOfAPlayersFirstMainPhase(players)
            | S::AtTheBeginningOfAPlayersSecondMainPhase(players)
            | S::AtTheBeginningOfAPlayersPostcombatMainPhase(players)
            | S::AtTheBeginningOfCombatDuringAPlayersTurn(players) => EventSpec::Turn {
                point: match value {
                    S::AtTheBeginningOfAPlayersUpkeep(_) => TurnPoint::UpkeepBegins,
                    S::AtTheBeginningOfAPlayersDrawStep(_) => TurnPoint::DrawStepBegins,
                    S::AtTheBeginningOfAPlayersEndStep(_) => TurnPoint::EndStepBegins,
                    S::AtTheBeginningOfAPlayersMainPhases(_) => TurnPoint::MainPhaseBegins,
                    S::AtTheBeginningOfAPlayersFirstMainPhase(_) => TurnPoint::FirstMainPhaseBegins,
                    S::AtTheBeginningOfAPlayersSecondMainPhase(_) => {
                        TurnPoint::SecondMainPhaseBegins
                    }
                    S::AtTheBeginningOfAPlayersPostcombatMainPhase(_) => {
                        TurnPoint::PostcombatMainPhaseBegins
                    }
                    S::AtTheBeginningOfCombatDuringAPlayersTurn(_) => TurnPoint::CombatBegins,
                    _ => unreachable!(),
                },
                players: self.players(players)?,
            },
            S::AtTheBeginningOfCombat => EventSpec::Turn {
                point: TurnPoint::CombatBegins,
                players: PlayerSet::All,
            },
            S::AtTheEndOfCombat => EventSpec::Turn {
                point: TurnPoint::CombatEnds,
                players: PlayerSet::All,
            },
            S::WhenAPlayerControlsAPermanent(players, objects) => EventSpec::State {
                condition: Condition::Compare {
                    left: Expr::CountObjects(Box::new(ObjectSet::Battlefield(ObjectFilter::All(
                        vec![
                            self.filter(objects)?,
                            ObjectFilter::ControlledBy(self.players(players)?),
                        ],
                    )))),
                    comparison: Comparison::Greater,
                    right: Expr::Constant(0),
                },
            },
            S::Or(events) if !events.is_empty() => EventSpec::Either(
                events
                    .iter()
                    .enumerate()
                    .map(|(i, event)| {
                        self.trigger(event)
                            .map_err(|e| e.at(format!("events[{i}]")))
                    })
                    .collect::<Result<_>>()?,
            ),
            S::If(condition, event) => {
                let event = self.trigger(event)?;
                let mut event_scope = self.clone();
                event_scope.event = crate::semantics::event_bindings(&event);
                EventSpec::Filtered {
                    event: Box::new(event),
                    condition: event_scope.condition(condition)?,
                }
            }
            _ => self.damage_trigger(value)?,
        })
    }
}
