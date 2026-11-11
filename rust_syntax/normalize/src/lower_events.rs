// Event matching is independent of the triggered ability's resolution program.

/// What a "next ... this turn" event waits for.
enum NextEvent<'a> {
    Spell(&'a source::Player, &'a source::Spells),
    Trigger(source::Trigger),
}

fn next_event(value: &source::FutureTrigger) -> Option<NextEvent<'_>> {
    use source::FutureTrigger as S;
    use source::Trigger as T;
    Some(match value {
        S::WhenPlayerCastsTheirNextSpellThisTurn(player, spells) => {
            NextEvent::Spell(player, spells)
        }
        S::WhenPlayerCastsTheirNextSpellFromTheirHandThisTurn(player, spells) => {
            NextEvent::Trigger(T::WhenAPlayerCastsASpell(
                Box::new(source::Players::SinglePlayer(player.clone())),
                Box::new(source::Spells::And(vec![
                    (**spells).clone(),
                    source::Spells::WasCastFromTheirHand,
                ])),
            ))
        }
        S::WhenAPlayerNextAttacksThisTurn(players) => {
            NextEvent::Trigger(T::WhenAPlayerAttacks(players.clone()))
        }
        S::WhenAPlayerNextActivatesAnAbilityThisTurn(players, abilities) => NextEvent::Trigger(
            T::WhenAPlayerActivatesAnAbility(players.clone(), abilities.clone()),
        ),
        S::WhenPlayerNextActivatesAnAbilityThisTurn(player, abilities) => {
            NextEvent::Trigger(T::WhenAPlayerActivatesAnAbility(
                Box::new(source::Players::SinglePlayer(player.clone())),
                abilities.clone(),
            ))
        }
        _ => return None,
    })
}

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
            // Only an opening hand makes these, so the first upkeep is the next one.
            S::AtTheBeginningOfTheNextUpkeep | S::AtTheBeginningOfTheFirstUpkeep => (
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
            | S::AtTheBeginningOfPlayersFirstUpkeep(p)
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
                    S::AtTheBeginningOfPlayersNextUpkeep(_)
                    | S::AtTheBeginningOfPlayersFirstUpkeep(_) => TurnPoint::UpkeepBegins,
                    S::AtTheBeginningOfPlayersNextDrawStep(_) => TurnPoint::DrawStepBegins,
                    S::AtTheBeginningOfPlayersNextMainPhase(_) => TurnPoint::MainPhaseBegins,
                    S::AtTheBeginningOfPlayersNextFirstMainPhase(_) => {
                        TurnPoint::FirstMainPhaseBegins
                    }
                    _ => TurnPoint::DeclareAttackersStepBegins,
                };
                (point, PlayerSet::One(player), start, None)
            }
            S::AtTheBeginningOfTheEndStepOfTheExtraTurnCreatedThisWay => (
                TurnPoint::EndStepBegins,
                PlayerSet::All,
                DelayedStart::ExtraTurn(
                    self.extra_turn
                        .clone()
                        .ok_or_else(|| invalid("no extra turn was created in this scope"))?,
                ),
                None,
            ),
            _ => return Err(unsupported(value)),
        };
        Ok((EventSpec::Turn { point, players }, start, expires))
    }

    /// A delayed event's matcher: its body still reads only the creator's scope.
    fn future_event(&self, value: &source::FutureTrigger) -> Result<EventSpec> {
        use source::FutureTrigger as S;
        use source::Trigger as T;
        let one = |p: &source::Permanent| Box::new(source::Permanents::SinglePermanent(p.clone().into()));
        let trigger = match value {
            S::Or(events) => {
                return Ok(EventSpec::AnyOf(
                    events
                        .iter()
                        .map(|event| self.future_event(event))
                        .collect::<Result<_>>()?,
                ))
            }
            S::WhenPlayerCastsTheirNextSpellThisGame(player, spells) => T::WhenAPlayerCastsASpell(
                Box::new(source::Players::SinglePlayer(player.clone())),
                spells.clone(),
            ),
            S::AtTheBeginningOfPlayersFirstMainPhaseOfTheGame(player) => {
                T::AtTheBeginningOfAPlayersFirstMainPhase(Box::new(
                    source::Players::SinglePlayer(player.clone()),
                ))
            }
            S::WhenPermanentLeavesTheBattlefield(p) => T::WhenAPermanentLeavesTheBattlefield(one(p)),
            S::WhenPermanentDies(p) => T::WhenAPermanentDies(one(p)),
            S::WhenPermanentBecomesUntapped(p) => T::WhenAPermanentBecomesUntapped(one(p)),
            S::WhenPermanentIsPutIntoAPlayersGraveyard(p, players) => {
                T::WhenAPermanentIsPutIntoAPlayersGraveyard(one(p), players.clone())
            }
            S::WhenPlayerLosesControlOfPermanent(player, p) => T::WhenAPlayerLosesControlOfAPermanent(
                Box::new(source::Players::SinglePlayer(player.clone().into())),
                one(p),
            ),
            _ => as_family(value, "Trigger").ok_or_else(|| unsupported(value))?,
        };
        self.trigger(&trigger)
    }

    fn from_anywhere_graveyard_objects(
        &self,
        cards: &source::Cards,
        owners: &source::Players,
    ) -> Result<ObjectFilter> {
        let mut filters = vec![
            self.card_filter(cards)?,
            ObjectFilter::OwnedBy(self.players(owners)?),
        ];
        // A named self-reference denotes the source object, including a token
        // copy (CR 201.5, 111.7). Ordinary descriptions explicitly name cards.
        if !matches!(
            cards,
            source::Cards::SingleCard(source::SingleCard::ThisCard)
        ) {
            filters.push(ObjectFilter::Not(Box::new(ObjectFilter::Token)));
        }
        Ok(ObjectFilter::AllOf(filters))
    }

    // Use only when the event contract independently identifies a permanent,
    // including its pre-departure view. Raw counter/targeting events do not.
    fn permanent_event_filter(&self, objects: &source::Permanents) -> Result<ObjectFilter> {
        let mut context = self.clone();
        context.retain_permanent_type_guard = false;
        context.filter(objects)
    }

    fn trigger(&self, value: &source::Trigger) -> Result<EventSpec> {
        let mut context = self.clone();
        context.retain_permanent_type_guard = true;
        context.reduced_trigger(value).or_else(|error| {
            context.rescue(error, || {
                context.first_each_turn(value)
                    .or_else(|| context.primitive_trigger(value))
            })
        })
    }

    /// The plain event behind a "for the first time each turn" form.
    fn first_each_turn(&self, value: &source::Trigger) -> Option<EventSpec> {
        let mut inspected = crate::inspect::value(value).ok()?;
        let base = inspected["variant"]
            .as_str()?
            .strip_suffix("ForTheFirstTimeEachTurn")?
            .to_string();
        let for_object = base.contains("ASpecificPermanent");
        inspected["variant"] = base.replace("ASpecificPermanent", "APermanent").into();
        let base: source::Trigger = crate::pipeline::source_tree::decode(&inspected).ok()?;
        let event = Box::new(self.trigger(&base).ok()?);
        Some(if for_object {
            EventSpec::FirstForObjectEachTurn(event)
        } else {
            EventSpec::FirstEachTurn(event)
        })
    }

    fn reduced_trigger(&self, value: &source::Trigger) -> Result<EventSpec> {
        use source::Trigger as S;
        Ok(match value {
            S::WithOtherPermanent(trigger, object) => {
                let event = self.trigger(trigger)?;
                let mut matched = self.clone();
                matched.event = crate::semantics::event_bindings(&event);
                if matched.event.other {
                    return Err(invalid("secondary event object is already bound"));
                }
                EventSpec::WithOtherObject {
                    event: Box::new(event),
                    object: matched.object(object)?,
                }
            }
            S::WhenAPermanentDealsDamageToAnyNumberOfPlayersForTheFirstTimeEachTurn(p, players) => {
                let event = S::WhenAPermanentDealsDamageToAPlayer(p.clone(), players.clone());
                EventSpec::FirstForObjectEachTurn(Box::new(self.trigger(&event)?))
            }
            S::WhenAPermanentBecomesSaddledForTheFirstTimeInATurn(objects) => {
                EventSpec::FirstForObjectEachTurn(Box::new(EventSpec::Object {
                    event: ObjectEvent::BecomesSaddled,
                    objects: self.filter(objects)?,
                }))
            }
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
            S::WhenAPermanentExplores(objects) => EventSpec::Object {
                event: ObjectEvent::Explores,
                objects: self.permanent_event_filter(objects)?,
            },
            S::WhenAPlayerSacrificesAPermanent(players, objects) => EventSpec::Player {
                event: PlayerEvent::Sacrifices,
                players: self.players(players)?,
                objects: Some(self.permanent_event_filter(objects)?),
            },
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
                    objects: ObjectFilter::AllOf(vec![
                        self.permanent_event_filter(objects)?,
                        ObjectFilter::OwnedBy(self.players(players)?),
                    ]),
                    from: Some(EventZone::Battlefield),
                    to: EventZone::Graveyard,
                }
            }
            S::WhenACardIsPutIntoAPlayersGraveyardFromAnywhere(cards, owners) => {
                EventSpec::ZoneChange {
                    objects: self.from_anywhere_graveyard_objects(cards, owners)?,
                    from: None,
                    to: EventZone::Graveyard,
                }
            }
            S::WhenACardIsPutIntoAPlayersGraveyardFromTheirLibrary(cards, owners) => {
                EventSpec::ZoneChange {
                    objects: ObjectFilter::AllOf(vec![
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
            S::WhenAPlayerControlsAPermanent(players, objects) => {
                let condition =
                Condition::Compare { left: Expr::CountObjects(Box::new(ObjectSet::battlefield(ObjectFilter::AllOf(
                        vec![
                            self.filter(objects)?,
                            ObjectFilter::ControlledBy(self.players(players)?),
                        ],
                    )))), test: NumberTest::Compare { comparison: Comparison::Greater, value: Expr::Constant(0) } };
                match &**objects {
                    source::Permanents::SinglePermanent(object) => EventSpec::StateWithPlayer {
                        condition,
                        player: PlayerRef::ControllerOf(Box::new(self.object(object)?)),
                    },
                    _ => EventSpec::State { condition },
                }
            },
            S::Or(events) if !events.is_empty() => {
                let events = events.iter().enumerate().map(|(i, event)| {
                    self.trigger(event).map_err(|e| e.at(format!("events[{i}]")))
                }).collect::<Result<Vec<_>>>()?;
                let whole = |event: EventSpec| -> Option<EventSpec> {
                    match event {
                        EventSpec::Batch { event, per: BatchPer::Whole, count: None, including: None } => Some(*event),
                        EventSpec::Caused { cause, event } => match *event {
                            EventSpec::Batch { event, per: BatchPer::Whole, count: None, including: None } => Some(EventSpec::Caused { cause, event }),
                            _ => None,
                        },
                        _ => None,
                    }
                };
                let grouped = events.iter().cloned().map(whole).collect::<Option<Vec<_>>>();
                // Alternative origins of the same simultaneous zone-change
                // action share one batch. Do not merge unrelated event kinds
                // (for example fighting/blocking, or phasing/exiling).
                let destination = |event: &EventSpec| match event {
                    EventSpec::ZoneChange { to, .. } => Some(to.clone()),
                    EventSpec::Caused { event, .. } => match event.as_ref() {
                        EventSpec::ZoneChange { to, .. } => Some(to.clone()), _ => None,
                    },
                    _ => None,
                };
                let same_action = grouped.as_ref().is_some_and(|grouped| {
                    let first = grouped.first().and_then(destination);
                    first.is_some() && grouped.iter().all(|event| destination(event) == first)
                });
                if let Some(grouped) = grouped.filter(|_| same_action) {
                    EventSpec::Batch {
                        event: Box::new(EventSpec::AnyOf(grouped)),
                        per: BatchPer::Whole, count: None, including: None,
                    }
                } else { EventSpec::AnyOf(events) }
            },
            S::If(condition, event) => {
                let event = self.trigger(event)?;
                let mut event_scope = self.clone();
                event_scope.event = crate::semantics::event_bindings(&event);
                EventSpec::Filtered {
                    event: Box::new(event),
                    condition: event_scope.condition(condition)?,
                }
            }
            S::AtTheEndOfTheFirstCombat => {
                EventSpec::FirstEachTurn(Box::new(self.trigger(&S::AtTheEndOfCombat)?))
            }
            S::AtTheBeginningOfAPlayersFirstUpkeepEachTurn(players) => EventSpec::FirstEachTurn(
                Box::new(self.trigger(&S::AtTheBeginningOfAPlayersUpkeep(players.clone()))?),
            ),
            S::AtTheBeginningOfAPlayersFirstUpkeepOfTheGame(players) => EventSpec::FirstEachGame(
                Box::new(self.trigger(&S::AtTheBeginningOfAPlayersUpkeep(players.clone()))?),
            ),
            S::AtTheBeginningOfTheFirstUpkeepOfTheGame => {
                EventSpec::FirstEachGame(Box::new(self.trigger(
                    &S::AtTheBeginningOfAPlayersUpkeep(Box::new(source::Players::AnyPlayer)),
                )?))
            }
            _ => self.damage_trigger(value)?,
        })
    }
}
