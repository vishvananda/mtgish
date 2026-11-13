// "This way": what the last action of a kind did, read back as a set.
impl Context {
    fn this_way_set(
        &self,
        done: Done,
        phase: ObjectPhase,
        filter: Option<ObjectFilter>,
    ) -> Result<ObjectSet> {
        let objects = match self.done(done) {
            Ok(result) => ObjectSet::Transition { result, phase },
            Err(error) => {
                match Self::paid_actions(done)
                    .iter()
                    .find_map(|action| {
                        self.paid_set(*action, CostObjectView::Performed, phase.clone())
                    })
                {
                    Some(objects) => objects,
                    None if self.event.group => ObjectSet::EventGroup,
                    None => return Err(error),
                }
            }
        };
        Ok(match filter {
            Some(filter) => ObjectSet::Matching {
                objects: Box::new(objects),
                filter,
            },
            None => objects,
        })
    }

    /// What this scope put into a graveyard, by whichever action did it.
    fn graveyard_set(&self) -> Result<ObjectSet> {
        [Done::Graveyard, Done::Destroyed, Done::Sacrificed]
            .into_iter()
            .map(|done| self.this_way_set(done, ObjectPhase::After, None))
            .find(Result::is_ok)
            .unwrap_or_else(|| Err(invalid("nothing went to a graveyard in this scope")))
    }

    /// The cost components that do what `done` names, when no action did.
    fn paid_actions(done: Done) -> &'static [crate::number_spec::PaymentAction] {
        use crate::number_spec::PaymentAction as P;
        match done {
            Done::Exiled => &[P::Exile],
            Done::Returned => &[P::Return],
            Done::Entered => &[P::Enter],
            Done::Sacrificed => &[P::Sacrifice],
            Done::Discarded => &[P::Discard],
            Done::Drew => &[P::Draw],
            Done::Milled => &[P::Mill],
            Done::Graveyard => &[P::Sacrifice, P::Discard, P::Mill],
            _ => &[],
        }
    }

    fn this_way_count(
        &self,
        done: Done,
        phase: ObjectPhase,
        filter: Option<ObjectFilter>,
    ) -> Result<Expr> {
        let after_exile = done == Done::Exiled && matches!(phase, ObjectPhase::After);
        let mut objects = self.this_way_set(done, phase, filter)?;
        if after_exile {
            // Movement receipts retain the arrival object even if a replacement
            // redirected it, and that ID keeps its arrival zone after later moves.
            objects = ObjectSet::Matching {
                objects: Box::new(objects),
                filter: ObjectFilter::InZone(ZoneScope::Shared(EventZone::Exile)),
            };
        }
        Ok(Expr::CountObjects(Box::new(objects)))
    }

    fn this_way_total(
        &self,
        done: Done,
        phase: ObjectPhase,
        filter: Option<ObjectFilter>,
        property: Property,
    ) -> Result<Expr> {
        Ok(Expr::Aggregate {
            objects: Box::new(self.this_way_set(done, phase, filter)?),
            property,
            aggregation: Aggregation::Sum,
        })
    }

    fn this_way_greatest(
        &self,
        done: Done,
        phase: ObjectPhase,
        filter: Option<ObjectFilter>,
        property: Property,
    ) -> Result<Expr> {
        Ok(Expr::Aggregate {
            objects: Box::new(self.this_way_set(done, phase, filter)?),
            property,
            aggregation: Aggregation::Maximum,
        })
    }

    fn this_way_types(&self, done: Done, phase: ObjectPhase) -> Result<Expr> {
        Ok(Expr::CountDistinct {
            objects: Box::new(self.this_way_set(done, phase, None)?),
            property: DistinctProperty::CardTypes,
        })
    }

    /// What a reveal or seek left in a hand, or else what went back to one.
    fn this_way_to_hand(&self) -> Result<ObjectSet> {
        match &self.to_hand {
            Some(result) => Ok(ObjectSet::Transition {
                result: result.clone(),
                phase: ObjectPhase::After,
            }),
            None => self.this_way_set(Done::Returned, ObjectPhase::After, None),
        }
    }

    /// What entered, by an action or by a cost.
    fn this_way_entered(&self, filter: Option<ObjectFilter>) -> Result<ObjectSet> {
        let entered = self.objects(&source::Permanents::ThePermanentsPutOnTheBattlefieldThisWay)?;
        Ok(match filter {
            Some(filter) => ObjectSet::Matching {
                objects: Box::new(entered),
                filter,
            },
            None => entered,
        })
    }

    fn excess_dealt(&self, to: ObjectFilter) -> Result<Expr> {
        Ok(Expr::ExcessDamage {
            result: self.done(Done::Damage)?,
            to: Box::new(to),
        })
    }

    /// Damage this replacement prevented, when its source passes `filter`.
    fn prevented_from(&self, filter: ObjectFilter) -> Result<Condition> {
        let some = |left| self.this_way_compare(left, Comparison::Greater, 0);
        Ok(Condition::AllOf(vec![
            some(Expr::Performed(self.done(Done::Prevented)?)),
            some(Expr::CountObjects(Box::new(ObjectSet::Matching {
                objects: Box::new(ObjectSet::Damage(DamageSubject::Source)),
                filter,
            }))),
        ]))
    }

    fn damaged_this_way(&self, filter: ObjectFilter) -> Result<Expr> {
        Ok(Expr::CountObjects(Box::new(ObjectSet::battlefield(
            ObjectFilter::AllOf(vec![
                ObjectFilter::DamagedBy(self.done(Done::Damage)?),
                filter,
            ]),
        ))))
    }

    fn players_damaged_this_way(&self, players: PlayerSet) -> Result<Expr> {
        Ok(Expr::CountPlayers(PlayerSet::Intersection(vec![
            PlayerSet::DamagedBy(self.done(Done::Damage)?),
            players,
        ])))
    }

    /// `None` for forms this table does not read.
    fn this_way_number(&self, value: &source::GameNumber) -> Option<Result<Expr>> {
        use ObjectPhase::{After, Before};
        use source::GameNumber as S;
        let cards = |c: &source::Cards| self.card_filter(c).map(Some);
        let permanents = |p: &source::Permanents| self.filter(p).map(Some);
        Some((|| match value {
            S::AmountOfExcessDamageDealtThisWay => self.excess_dealt(ObjectFilter::Any),
            S::AmountOfExcessDamageDealtToPermanentsThisWay(p) => {
                self.excess_dealt(self.filter(p)?)
            }
            S::NumPermanentsShuffledIntoLibraryThisWayByPlayer(player) => {
                let owner = PlayerSet::One(self.player(player)?);
                self.this_way_count(
                    Done::Library,
                    ObjectPhase::Before,
                    Some(ObjectFilter::OwnedBy(owner)),
                )
            }
            S::NumCardsShuffledIntoLibraryThisWay => {
                let result = self.done(Done::Library)?;
                match &self.iterated {
                    Some(player @ PlayerRef::Bound(_)) if self.looped.contains(&result.0) => {
                        Ok(Expr::CountObjects(Box::new(ObjectSet::MovedBy {
                            result,
                            phase: Before,
                            player: player.clone(),
                        })))
                    }
                    _ => self.this_way_count(Done::Library, Before, None),
                }
            }
            S::NumCardsPutIntoLibraryThisWay => {
                self.this_way_count(Done::Library, Before, None)
            }
            S::TheNumberOfPermanentsReturnedToHandThisWay | S::NumCardsReturnedToHandThisWay => {
                self.this_way_count(Done::Returned, Before, None)
            }
            S::NumberOfCardsPutIntoGraveyardThisWay => {
                self.this_way_count(Done::Graveyard, Before, None)
            }
            S::TheNumberOfCardsPutIntoAGraveyardThisWay(c) => self.this_way_count(
                Done::Graveyard,
                After,
                Some(self.grave_filter(c)?),
            ),
            S::NumberOfCardsOfTypeExiledThisWay(c)
            | S::TheNumberOfLibraryCardsOfTypeExiledThisWay(c)
            | S::NumGraveyardCardsOfTypeExiledThisWay(c) => {
                self.this_way_count(Done::Exiled, After, cards(c)?)
            }
            S::NumPermanentsOfTypeExiledThisWay(p) => {
                self.this_way_count(Done::Exiled, Before, permanents(p)?)
            }
            S::TheNumberOfLibraryCardsExiledThisWay | S::TheNumberOfCardsInHandExiledThisWay => {
                self.this_way_count(Done::Exiled, Before, None)
            }
            S::TheNumberOfPermanentsOfTypeSacrificedThisWay(p) => {
                self.this_way_count(Done::Sacrificed, Before, permanents(p)?)
            }
            S::NumCardsOfTypeDiscardedThisWay(c) => {
                self.this_way_count(Done::Discarded, After, cards(c)?)
            }
            S::NumCardsMilledIntoGraveyardThisWay(c) => {
                self.this_way_count(Done::Milled, After, cards(c)?)
            }
            S::ManaValueOfTheCardMilledThisWay => {
                self.this_way_total(Done::Milled, After, None, Property::ManaValue)
            }
            S::ManaValueOfTheCardExiledThisWay => {
                self.this_way_total(Done::Exiled, After, None, Property::ManaValue)
            }
            S::TotalManaValueOfTheCardsDiscardedThisWay => {
                self.this_way_total(Done::Discarded, After, None, Property::ManaValue)
            }
            S::ToughnessOfCreatureDestroyedThisWay => {
                self.this_way_total(Done::Destroyed, Before, None, Property::Toughness)
            }
            S::ToughnessOfCreaturePutOnBattlefieldThisWay => {
                self.this_way_total(Done::Entered, After, None, Property::Toughness)
            }
            S::TotalManaValueOfEachPermanentDestroyedThisWay(p) => self.this_way_total(
                Done::Destroyed,
                Before,
                permanents(p)?,
                Property::ManaValue,
            ),
            S::TotalPowerOfPermanentsExiledThisWay(p) => {
                self.this_way_total(Done::Exiled, Before, permanents(p)?, Property::Power)
            }
            S::TheNumberOfCardsPutIntoHandThisWay => {
                Ok(Expr::CountObjects(Box::new(self.this_way_to_hand()?)))
            }
            S::ManaValueOfCardPutInHandThisWay => Ok(Expr::Aggregate {
                objects: Box::new(self.this_way_to_hand()?),
                property: Property::ManaValue,
                aggregation: Aggregation::Sum,
            }),
            S::TheNumberOfPermanentsPutIntoAPlayersGraveyardThisWay(p, players) => self
                .this_way_count(
                    Done::Graveyard,
                    Before,
                    Some(ObjectFilter::AllOf(vec![
                        self.filter(p)?,
                        ObjectFilter::OwnedBy(self.players(players)?),
                    ])),
                ),
            S::TheNumberOfPermanentsThatDiedThisWay(p) => {
                self.this_way_count(Done::Graveyard, Before, permanents(p)?)
            }
            S::TheNumberOfCardtypesAmongCardsDiscardedThisWay
            | S::NumCardTypesOfCardDiscardedThisWay => self.this_way_types(Done::Discarded, After),
            S::TheNumberOfCardTypesAmongCardsPutInGraveyardThisWay => {
                self.this_way_types(Done::Graveyard, After)
            }
            S::TheGreatestManaValueAmongTheCardsDiscardedThisWay => {
                self.this_way_greatest(Done::Discarded, After, None, Property::ManaValue)
            }
            S::HighestManaValueAmongCardsMilledThisWay => {
                self.this_way_greatest(Done::Milled, After, None, Property::ManaValue)
            }
            S::TheGreatestPowerAmongCardsPutIntoGraveyardThisWay(c) => {
                self.this_way_greatest(Done::Graveyard, After, cards(c)?, Property::Power)
            }
            S::TheNumberOfPermanentsPutOntoTheBattlefieldThisWay(p) => Ok(Expr::CountObjects(
                Box::new(self.this_way_entered(permanents(p)?)?),
            )),
            S::TheNumberOfCardsReturnedToTheBattlefieldThisWay => {
                Ok(Expr::CountObjects(Box::new(self.this_way_entered(None)?)))
            }
            S::TheNumberOfPermanentsGainedControlOfThisWay => Ok(Expr::CountObjects(Box::new(
                ObjectSet::Affected(self.done(Done::Effect)?),
            ))),
            S::TheNumberOfPermanentsThatHadCountersPutOnItThisWay => Ok(Expr::CountObjects(
                Box::new(ObjectSet::Affected(self.done(Done::Counters)?)),
            )),
            S::TheNumberOfCountersPutThisWay => Ok(Expr::Performed(self.done(Done::Counters)?)),
            S::TheTotalAmountOfManaPaidThisWay => {
                Ok(match self.number(&S::AmountOfManaPaidThisWay)? {
                    Expr::Performed(result) => Expr::PerformedTotal(result),
                    other => other,
                })
            }
            S::TheNumberOfLibraryCardsRevealedThisWay => self
                .revealed
                .clone()
                .map(|objects| Expr::CountObjects(Box::new(objects)))
                .ok_or_else(|| invalid("no card was revealed in this scope")),
            _ => Err(unsupported(value)),
        })())
        .filter(|result| !matches!(result, Err(e) if e.source_type != "invariant"))
    }

    fn this_way_compare(&self, left: Expr, comparison: Comparison, right: i64) -> Condition {
        Condition::Compare { left, test: NumberTest::Compare { comparison: comparison, value: Expr::Constant(right) } }
    }

    /// `None` for forms this table does not read.
    fn this_way_condition(&self, value: &source::Condition) -> Option<Result<Condition>> {
        use ObjectPhase::{After, Before};
        use source::Condition as S;
        let any = |set: Result<Expr>| {
            set.map(|left| self.this_way_compare(left, Comparison::Greater, 0))
        };
        let measured = |left: Result<Expr>, comparison: &source::Comparison| {
            let test = self.number_test(comparison)?;
            Ok(Condition::Compare { left: left?, test })
        };
        let cards = |c: &source::Cards| self.card_filter(c).map(Some);
        let permanents = |p: &source::Permanents| self.filter(p).map(Some);
        Some((|| match value {
            S::DamageWasPreventedThisWay => any(Ok(Expr::Performed(self.done(Done::Prevented)?))),
            S::DamageFromASourceWasPreventedThisWay(sources) => {
                self.prevented_from(self.damage_sources(sources)?)
            }
            S::DamageFromAPermanentSourceWasPreventedThisWay(sources) => {
                self.prevented_from(self.filter(sources)?)
            }
            S::ExcessDamageWasDealtThisWay => any(self.excess_dealt(ObjectFilter::Any)),
            S::ExcessDamageWasDealtToAPermanentThisWay(p) => any(self.excess_dealt(self.filter(p)?)),
            S::AnAmountOfExcessDamageWasDealtThisWay(comparison) => {
                measured(self.excess_dealt(ObjectFilter::Any), comparison)
            }
            S::ACreatureOrPlaneswalkerWasDealtDamageThisWay(p)
            | S::PermanentDealtDamageToACreatureOrPlaneswalkerThisWay(_, p) => {
                any(self.damaged_this_way(self.filter(p)?))
            }
            S::APlayerWasDealtDamageThisWay(players) => {
                any(self.players_damaged_this_way(self.players(players)?))
            }
            S::PermanentDealtDamageToPlayerThisWay(_, player) => {
                any(self.players_damaged_this_way(PlayerSet::One(self.player(player)?)))
            }
            S::ACardWasPutIntoHandThisWay(c) | S::TheCardReturnedToHandThisWayPassesFilter(c) => {
                any(self.this_way_count(Done::Returned, After, cards(c)?))
            }
            S::ACardWasPutIntoGraveyardThisWay(c) => {
                any(self.this_way_count(Done::Graveyard, After, cards(c)?))
            }
            S::NumCardsDiscardedThisWayPassGroupFilter(comparison, c, group)
            | S::NumCardsFromHandRevealedThisWayPassGroupFilter(comparison, c, group) => {
                let subset = || -> Result<Condition> {
                    let (comparison, amount) = self.comparison(comparison)?;
                    let objects = match value {
                        S::NumCardsDiscardedThisWayPassGroupFilter(..) => {
                            self.this_way_set(Done::Discarded, After, None)?
                        }
                        _ => self.revealed_objects()?,
                    };
                    Ok(Condition::SubsetMeets {
                        objects: ObjectSet::Matching {
                            objects: Box::new(objects),
                            filter: self.card_filter(c)?,
                        },
                        comparison,
                        amount,
                        constraints: self.group_constraints(group)?,
                    })
                };
                subset()
            }
            S::ACardWasExiledThisWay(c) => any(self.this_way_count(Done::Exiled, After, cards(c)?)),
            S::APermanentWasExiledThisWay(p) => {
                any(self.this_way_count(Done::Exiled, Before, permanents(p)?))
            }
            S::APermanentWasPutOntoTheBattlefieldThisWay(p)
            | S::APermanentOfTypeWasPutOntoTheBattlefieldThisWay(p) => {
                any(self.this_way_count(Done::Entered, After, permanents(p)?))
            }
            S::PermanentWasDestroyedThisWay => {
                any(self.this_way_count(Done::Destroyed, Before, None))
            }
            S::ACardWasntDrawnThisWay | S::PlayerDidntDrawACardThisWay => {
                self.done(Done::Drew).map(|result| {
                    self.this_way_compare(Expr::Performed(result), Comparison::Equal, 0)
                })
            }
            S::ALibraryCardWasPutIntoHandThisWay => {
                any(self.this_way_count(Done::Returned, After, None))
            }
            S::AGraveyardCardWasReturnedToHandThisWay(c) => {
                any(self.this_way_count(Done::Returned, After, cards(c)?))
            }
            S::ANumberOfGraveyardCardsWereReturnedToHandThisWay(comparison) => {
                measured(self.this_way_count(Done::Returned, Before, None), comparison)
            }
            S::ANumberOfCardsWereExildThisWay(comparison, c) => {
                measured(self.this_way_count(Done::Exiled, After, cards(c)?), comparison)
            }
            S::PermanentDiesThisWay => any(self.this_way_count(Done::Graveyard, Before, None)),
            S::APermanentDiesThisWay(p) => {
                any(self.this_way_count(Done::Graveyard, Before, permanents(p)?))
            }
            S::APermanentWasntSacrificedThisWay(p) => self
                .this_way_count(Done::Sacrificed, Before, permanents(p)?)
                .map(|left| self.this_way_compare(left, Comparison::Equal, 0)),
            S::NumberOfCardTypesAmongThePermanentsSacrificedThisWayIs(comparison) => {
                measured(self.this_way_types(Done::Sacrificed, Before), comparison)
            }
            S::NumberPermanentsEnteredTheBattlefieldThisWay(comparison, p) => {
                measured(self.this_way_count(Done::Entered, After, permanents(p)?), comparison)
            }
            S::NoCardsWereRevealedThisWay => self.revealed_objects().map(|objects| {
                self.this_way_compare(Expr::CountObjects(Box::new(objects)), Comparison::Equal, 0)
            }),
            S::NoCardsOfTypeWereRevealedThisWay(c) => {
                let objects = ObjectSet::Matching {
                    objects: Box::new(self.revealed_objects()?),
                    filter: self.card_filter(c)?,
                };
                Ok(self.this_way_compare(
                    Expr::CountObjects(Box::new(objects)),
                    Comparison::Equal,
                    0,
                ))
            }
            S::AllCardsRevealedThisWayAreCardsOfType(c) => {
                let objects = ObjectSet::Matching {
                    objects: Box::new(self.revealed_objects()?),
                    filter: ObjectFilter::Not(Box::new(self.card_filter(c)?)),
                };
                Ok(self.this_way_compare(
                    Expr::CountObjects(Box::new(objects)),
                    Comparison::Equal,
                    0,
                ))
            }
            S::ANumberOfCardsWereRevealedThisWay(comparison, c) => {
                let objects = ObjectSet::Matching {
                    objects: Box::new(self.revealed_objects()?),
                    filter: self.card_filter(c)?,
                };
                measured(Ok(Expr::CountObjects(Box::new(objects))), comparison)
            }
            S::ACardWasFoundThisWay => {
                let found = self
                    .found
                    .clone()
                    .ok_or_else(|| invalid("no card was found in this scope"))?;
                any(Ok(Expr::CountObjects(Box::new(found))))
            }
            // A token made only by a payment's branch was made if it was paid.
            S::ATokenWasCreatedThisWay
                if self.created_tokens.is_none() && self.payment.is_some() =>
            {
                Ok(Condition::PaymentChosen(self.payment.clone().unwrap()))
            }
            S::ATokenWasCreatedThisWay => {
                let result = self
                    .created_tokens
                    .clone()
                    .ok_or_else(|| invalid("tokens-created reference has no creation in this scope"))?;
                any(Ok(Expr::CountObjects(Box::new(ObjectSet::Created(result)))))
            }
            S::NoOneTookAnActionThisWay => {
                let took = self.players(&source::Players::PlayerWhoTookActionThisWay)?;
                Ok(self.this_way_compare(
                    Expr::CountPlayers(took),
                    Comparison::Equal,
                    0,
                ))
            }
            S::AllPermanentsPassFilter(objects, filter) => {
                let rest = ObjectSet::Matching {
                    objects: Box::new(self.objects(objects)?),
                    filter: ObjectFilter::Not(Box::new(self.filter(filter)?)),
                };
                Ok(self.this_way_compare(
                    Expr::CountObjects(Box::new(rest)),
                    Comparison::Equal,
                    0,
                ))
            }
            S::PermanentTransformedThisWay(object) => Ok(Condition::ObjectMatches {
                object: self.object(object)?,
                filter: ObjectFilter::State(ObjectState::Transformed),
            }),
            S::TheLastCardExiledThisWayWasACard(filter) => Ok(Condition::ObjectMatches {
                object: self.result_object(self.exiled.as_ref(), 0)?,
                filter: self.card_filter(filter)?,
            }),
            S::TheChosenGraveyardCardPassesFilter(filter) => Ok(Condition::ObjectMatches {
                object: self.grave_card(&source::CardInGraveyards::TheChosenGraveyardCard)?,
                filter: self.card_filter(filter)?,
            }),
            S::DeadCardPassesFilter(filter) => Ok(Condition::ObjectMatches {
                object: self.grave_card(&source::CardInGraveyards::Trigger_ThatGraveyardCard)?,
                filter: self.card_filter(filter)?,
            }),
            S::NoCardsWerePutIntoHandThisWay => self
                .this_way_count(Done::Returned, Before, None)
                .map(|left| self.this_way_compare(left, Comparison::Equal, 0)),
            S::ANumberOfCardsWereDiscardedThisWay(comparison) => measured(
                self.this_way_count(Done::Discarded, Before, None),
                comparison,
            ),
            S::NumCardsHaveBeenMilledIntoGraveyardThisWay(comparison, c) => {
                measured(self.this_way_count(Done::Milled, After, cards(c)?), comparison)
            }
            S::TheNumberOfPermanentsReturnedToHandThisWayIs(comparison, p) => measured(
                self.this_way_count(Done::Returned, Before, permanents(p)?),
                comparison,
            ),
            _ => Err(unsupported(value)),
        })())
        .filter(|result| !matches!(result, Err(e) if e.source_type != "invariant"))
    }
    /// A choice in a graveyard: "those cards" reads it with the ones before it.
    fn note_grave_choice(&mut self, result: ResultId) {
        self.grave_picks.push(result.clone());
        self.grave_choice = Some(result);
    }
    /// Later "chosen" references of this zone read this choice.
    fn note_choice(&mut self, objects: &ObjectSet, result: ResultId) {
        let zone = match objects {
            ObjectSet::Zone { scope, .. } => match scope {
                ZoneScope::Shared(zone) | ZoneScope::Players { zone, .. } => zone,
            },
            ObjectSet::InZone { zone, .. } => zone,
            ObjectSet::Matching { objects, .. } | ObjectSet::Difference { objects, .. } => {
                return self.note_choice(objects, result)
            }
            ObjectSet::Targets(slot) => self
                .targets
                .iter()
                .find(|t| t.slot.0 == slot.0)
                .and_then(|t| match &t.kind {
                    TargetKind::Object(domain) => domain.zones.first(),
                    _ => None,
                })
                .unwrap_or(&EventZone::Battlefield),
            _ => &EventZone::Battlefield,
        };
        match zone {
            EventZone::Hand => self.hand_choice = Some(result),
            EventZone::Graveyard => self.note_grave_choice(result),
            EventZone::Exile => self.exile_choice = Some(result),
            _ => self.permanent_choice = Some(result),
        }
    }
    /// A resolution choice; the caller says which reference reads its result.
    fn pick(
        &mut self,
        actor: &PlayerSet,
        objects: ObjectSet,
        count: Count,
        constraints: Vec<SelectionConstraint>,
    ) -> Result<(Instruction, ResultId)> {
        let PlayerSet::One(player) = actor else {
            return Err(invalid("a choice needs one chooser"));
        };
        let choice = self.effect(Operation::ChooseObjects {
            constraints,
            player: player.clone(),
            objects,
            count,
        });
        let Instruction::Effect { result, .. } = &choice else {
            unreachable!()
        };
        let result = result.clone();
        Ok((choice, result))
    }
    /// "An artifact, a creature, and a land": one pick per filter from `objects`.
    fn pick_slots(
        &mut self,
        actor: &PlayerSet,
        slots: &[source::Permanents],
        objects: &source::Permanents,
    ) -> Result<Instruction> {
        let objects = self.objects(objects)?;
        let slots = slots
            .iter()
            .map(|slot| self.filter(slot))
            .collect::<Result<Vec<_>>>()?;
        self.pick_each(actor, objects, slots, Count::AsManyAsCan)
    }
    /// One object for each filter; `count` says whether a slot may stay empty.
    fn pick_each(
        &mut self,
        actor: &PlayerSet,
        objects: ObjectSet,
        slots: Vec<ObjectFilter>,
        count: impl FnOnce(Expr) -> Count,
    ) -> Result<Instruction> {
        let count = count(Expr::Constant(slots.len() as i64));
        let constraints = vec![SelectionConstraint::OneForEach(slots)];
        let (choice, result) = self.pick(actor, objects.clone(), count, constraints)?;
        self.note_choice(&objects, result);
        Ok(choice)
    }
    fn pick_graveyard(
        &mut self,
        actor: &PlayerSet,
        owner: &PlayerSet,
        cards: &source::Cards,
        amount: Expr,
    ) -> Result<Instruction> {
        let filter = self.card_filter(cards)?;
        let objects = self.private_cards(EventZone::Graveyard, owner.clone(), filter);
        let count = Count::AsManyAsCan(amount);
        let (choice, result) = self.pick(actor, objects, count, vec![])?;
        self.note_grave_choice(result);
        Ok(choice)
    }
}
