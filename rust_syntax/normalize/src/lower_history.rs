// Occurrence history. "This turn" and similar forms ask how often an ordinary
// event happened, so they reuse `EventSpec` instead of adding a form per event.
impl Context {
    fn history(event: EventSpec, window: HistoryWindow, measure: HistoryMeasure) -> Expr {
        Expr::EventHistory {
            event: Box::new(event),
            window,
            measure,
        }
    }

    fn happened(event: EventSpec, window: HistoryWindow) -> Condition {
        Condition::Compare {
            left: Self::history(event, window, HistoryMeasure::Occurrences),
            comparison: Comparison::GreaterOrEqual,
            right: Expr::Constant(1),
        }
    }

    fn history_compare(
        &self,
        left: Expr,
        comparison: &source::Comparison,
    ) -> Result<Condition> {
        let (comparison, right) = self.comparison(comparison)?;
        Ok(Condition::Compare {
            left,
            comparison,
            right,
        })
    }

    fn cast_event(players: PlayerSet, spells: ObjectFilter) -> EventSpec {
        EventSpec::Spell {
            event: SpellEvent::Cast,
            players,
            spells,
        }
    }

    fn died_event(objects: ObjectFilter) -> EventSpec {
        EventSpec::ZoneChange {
            objects,
            from: Some(EventZone::Battlefield),
            to: EventZone::Graveyard,
        }
    }

    fn entered_event(objects: ObjectFilter) -> EventSpec {
        EventSpec::ZoneChange {
            objects,
            from: None,
            to: EventZone::Battlefield,
        }
    }

    fn player_event(name: &str, player: PlayerSet) -> EventSpec {
        EventSpec::Primitive {
            name: name.into(),
            operands: vec![Operand::Players(player)],
        }
    }

    fn drew_event(players: PlayerSet) -> EventSpec {
        Self::player_event("Trigger::WhenAPlayerDrawsACard", players)
    }

    fn life_event(gained: bool, players: PlayerSet) -> EventSpec {
        Self::player_event(
            if gained {
                "Trigger::WhenAPlayerGainsLife"
            } else {
                "Trigger::WhenAPlayerLosesLife"
            },
            players,
        )
    }

    fn discard_event(players: PlayerSet) -> EventSpec {
        EventSpec::Discard {
            players,
            cards: ObjectFilter::Any,
            source_only: false,
            kind: DiscardKind::Any,
            grouping: DiscardGrouping::Card,
        }
    }

    fn history_condition(&self, value: &source::Condition) -> Option<Result<Condition>> {
        use source::Condition as S;
        use HistoryMeasure as M;
        use HistoryWindow as W;
        Some((|| match value {
            S::ACreatureOrPlaneswalkerDiedThisTurn(objects) => Ok(Self::happened(
                Self::died_event(self.filter(objects)?),
                W::ThisTurn,
            )),
            S::APermanentLeftTheBattlefieldThisTurn(objects) => Ok(Self::happened(
                EventSpec::Object {
                    event: ObjectEvent::LeavesBattlefield,
                    objects: self.filter(objects)?,
                },
                W::ThisTurn,
            )),
            S::APermanentEnteredTheBattlefieldUnderPlayersControlThisTurn(objects, player) => {
                Ok(Self::happened(
                    Self::entered_event(ObjectFilter::All(vec![
                        self.filter(objects)?,
                        ObjectFilter::ControlledBy(PlayerSet::One(self.player(player)?)),
                    ])),
                    W::ThisTurn,
                ))
            }
            S::NumberPermanentsEnteredTheBattlefieldUnderPlayersControlThisTurn(
                comparison,
                objects,
                player,
            ) => self.history_compare(
                Self::history(
                    Self::entered_event(ObjectFilter::All(vec![
                        self.filter(objects)?,
                        ObjectFilter::ControlledBy(PlayerSet::One(self.player(player)?)),
                    ])),
                    W::ThisTurn,
                    M::Occurrences,
                ),
                comparison,
            ),
            S::ANumberOfPermanentsDiedThisTurn(comparison, objects) => self.history_compare(
                Self::history(
                    Self::died_event(self.filter(objects)?),
                    W::ThisTurn,
                    M::Occurrences,
                ),
                comparison,
            ),
            S::NumSpellsCastLastTurnIs(comparison, spells) => self.history_compare(
                Self::history(
                    Self::cast_event(PlayerSet::All, self.spell_filter(spells)?),
                    W::LastTurn,
                    M::Occurrences,
                ),
                comparison,
            ),
            _ => Err(unsupported(value)),
        })())
        .filter(Result::is_ok)
    }

    fn history_player_check(
        &self,
        player: &PlayerRef,
        value: &source::Players,
    ) -> Option<Condition> {
        use source::Players as S;
        use HistoryMeasure as M;
        use HistoryWindow as W;
        let one = || PlayerSet::One(player.clone());
        (|| -> Result<Condition> {
            Ok(match value {
                S::CastASpellThisTurn(spells) => Self::happened(
                    Self::cast_event(one(), self.spell_filter(spells)?),
                    W::ThisTurn,
                ),
                S::HasntCastASpellThisTurn(spells) => Condition::Not(Box::new(Self::happened(
                    Self::cast_event(one(), self.spell_filter(spells)?),
                    W::ThisTurn,
                ))),
                S::CastNumSpellsThisTurn(comparison, spells) => self.history_compare(
                    Self::history(
                        Self::cast_event(one(), self.spell_filter(spells)?),
                        W::ThisTurn,
                        M::Occurrences,
                    ),
                    comparison,
                )?,
                S::NumSpellsCastLastTurnIs(comparison, spells) => self.history_compare(
                    Self::history(
                        Self::cast_event(one(), self.spell_filter(spells)?),
                        W::LastTurn,
                        M::Occurrences,
                    ),
                    comparison,
                )?,
                S::AttackedThisTurn => Self::happened(
                    Self::player_event("Trigger::WhenAPlayerAttacks", one()),
                    W::ThisTurn,
                ),
                S::GainedLifeThisTurn => {
                    Self::happened(Self::life_event(true, one()), W::ThisTurn)
                }
                S::LostLifeThisTurn => Self::happened(Self::life_event(false, one()), W::ThisTurn),
                S::GainedLifeAmountThisTurn(comparison) => self.history_compare(
                    Self::history(Self::life_event(true, one()), W::ThisTurn, M::TotalAmount),
                    comparison,
                )?,
                S::LostLifeAmountThisTurn(comparison) => self.history_compare(
                    Self::history(Self::life_event(false, one()), W::ThisTurn, M::TotalAmount),
                    comparison,
                )?,
                S::NumCardsDrawnThisTurnIs(comparison) => self.history_compare(
                    Self::history(Self::drew_event(one()), W::ThisTurn, M::Occurrences),
                    comparison,
                )?,
                S::DiscardedACardThisTurn => {
                    Self::happened(Self::discard_event(one()), W::ThisTurn)
                }
                S::HadAPermanentEnterTheBattlefieldUnderTheirControlThisTurn(objects) => {
                    Self::happened(
                        Self::entered_event(ObjectFilter::All(vec![
                            self.filter(objects)?,
                            ObjectFilter::ControlledBy(one()),
                        ])),
                        W::ThisTurn,
                    )
                }
                _ => return Err(unsupported(value)),
            })
        })()
        .ok()
    }

    fn history_filter(&self, value: &source::Permanents) -> Option<ObjectFilter> {
        use source::Permanents as S;
        use HistoryWindow as W;
        let this = || ObjectFilter::SameObject(ObjectRef::Candidate);
        let attacked = || {
            Self::happened(
                EventSpec::Object {
                    event: ObjectEvent::Attacks,
                    objects: this(),
                },
                W::ThisTurn,
            )
        };
        let damage = |recipients, sources, subject, source_kind| {
            Self::happened(
                EventSpec::Damage(DamageEvent {
                    source_kind,
                    sources,
                    recipients,
                    kind: DamageKind::Any,
                    grouping: DamageGrouping::SourceRecipient,
                    subject,
                }),
                W::ThisTurn,
            )
        };
        let condition = (|| -> Result<Condition> {
            Ok(match value {
                S::EnteredTheBattlefieldThisTurn => {
                    Self::happened(Self::entered_event(this()), W::ThisTurn)
                }
                S::AttackedThisTurn => attacked(),
                S::DidntAttackThisTurn => Condition::Not(Box::new(attacked())),
                S::WasDealtDamageThisTurn => damage(
                    DamageRecipients::Objects(this()),
                    ObjectFilter::Any,
                    DamageSubject::Recipient,
                    ObjectKind::Any,
                ),
                S::WasDealtDamageByPermanentThisTurn(source) => damage(
                    DamageRecipients::Objects(this()),
                    ObjectFilter::SameObject(self.object(source)?),
                    DamageSubject::Recipient,
                    ObjectKind::Permanent,
                ),
                S::DealtDamageThisTurn => damage(
                    DamageRecipients::Any,
                    this(),
                    DamageSubject::Source,
                    ObjectKind::Permanent,
                ),
                S::DealtDamageToPlayerThisTurn(player) => damage(
                    DamageRecipients::Players(PlayerSet::One(self.player(player)?)),
                    this(),
                    DamageSubject::Source,
                    ObjectKind::Permanent,
                ),
                _ => return Err(unsupported(value)),
            })
        })()
        .ok()?;
        Some(ObjectFilter::Condition(Box::new(condition)))
    }

    fn history_number(&self, value: &source::GameNumber) -> Option<Expr> {
        use source::GameNumber as S;
        use HistoryMeasure as M;
        use HistoryWindow as W;
        let one = |p: &source::Player| self.player(p).ok().map(PlayerSet::One);
        Some(match value {
            S::NumCreaturesOrPlaneswalkersThatDiedThisTurn(objects) => Self::history(
                Self::died_event(self.filter(objects).ok()?),
                W::ThisTurn,
                M::Occurrences,
            ),
            S::LifeGainedByPlayerThisTurn(p) => {
                Self::history(Self::life_event(true, one(p)?), W::ThisTurn, M::TotalAmount)
            }
            S::LifeLostByPlayerThisTurn(p) => {
                Self::history(Self::life_event(false, one(p)?), W::ThisTurn, M::TotalAmount)
            }
            S::TotalLifeLostByPlayersThisTurn(players) => Self::history(
                Self::life_event(false, self.players(players).ok()?),
                W::ThisTurn,
                M::TotalAmount,
            ),
            S::NumSpellsCastByPlayerThisTurn(spells, p) => Self::history(
                Self::cast_event(one(p)?, self.spell_filter(spells).ok()?),
                W::ThisTurn,
                M::Occurrences,
            ),
            S::NumCardsDrawnByPlayerThisTurn(p) => {
                Self::history(Self::drew_event(one(p)?), W::ThisTurn, M::Occurrences)
            }
            S::NumCardsPlayerDiscardedThisTurn(p) => {
                Self::history(Self::discard_event(one(p)?), W::ThisTurn, M::Occurrences)
            }
            S::NumPlayers(players) => Expr::CountPlayers(self.players(players).ok()?),
            _ => return None,
        })
    }

    /// The permanent a dies trigger matched, read with last-known information.
    fn dead_object(&self) -> Result<ObjectRef> {
        if self.event.permanent {
            Ok(ObjectRef::EventSubject)
        } else {
            Err(invalid("this event names no dead permanent"))
        }
    }

    fn dead_property(&self, property: Property) -> Result<Expr> {
        Ok(Expr::ObjectProperty {
            object: self.dead_object()?,
            property,
            information: Information::CurrentOrLastKnown,
        })
    }

    fn dead_action(
        &mut self,
        value: &source::Action,
        actor: &PlayerSet,
    ) -> Option<Result<Instruction>> {
        use source::Action as S;
        use source::CardInGraveyards as G;
        let card = || Box::new(G::Trigger_ThatGraveyardCard);
        let rewritten = match value {
            S::ReturnDeadGraveyardCardToBattlefield(flags) => {
                S::PutGraveyardCardOntoBattlefield(card(), flags.clone())
            }
            S::ReturnDeadGraveyardCardToHand => S::PutGraveyardCardIntoHand(card()),
            S::ReturnDeadGraveyardCardToBottomOfLibrary => {
                S::PutGraveyardCardOnBottomOfLibrary(card())
            }
            S::ReturnDeadGraveyardCardToTopOfLibrary => S::PutGraveyardCardOnTopOfLibrary(card()),
            S::DeadPermanentDealsDistributedDamage => {
                let operation = self
                    .dead_object()
                    .and_then(|source| self.distributed_damage(source));
                return Some(operation.map(|operation| self.effect(operation)));
            }
            S::DeadPermanentDealsDamage(amount, recipients)
            | S::HaveDeadPermanentDealDamage(amount, recipients) => {
                let operation = (|| {
                    Ok(Operation::DealDamage {
                        source: self.dead_object()?,
                        recipients: self.recipients(recipients)?,
                        amount: self.number(amount)?,
                        preventable: true,
                    })
                })();
                return Some(operation.map(|operation| self.effect(operation)));
            }
            _ => return None,
        };
        Some(self.action(&rewritten, actor))
    }
}
