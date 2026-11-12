impl Context {
    fn still_exiled(&self, objects: ObjectSet) -> ObjectSet {
        // Exact post-exile incarnations, not a live query for later objects
        // with the same card identity. Also excludes diverted movements.
        ObjectSet::InZone {
            objects: Box::new(objects),
            zone: EventZone::Exile,
        }
    }

    fn hand_has_scope(&self, value: &source::CardsInHand) -> Result<bool> {
        match value {
            source::CardsInHand::InAPlayersHand(_) => Ok(true),
            source::CardsInHand::And(v) => {
                let mut found = false;
                for child in v {
                    found |= self.hand_has_scope(child)?;
                }
                Ok(found)
            }
            source::CardsInHand::Or(v) => {
                let scopes = v
                    .iter()
                    .map(|c| self.hand_has_scope(c))
                    .collect::<Result<Vec<_>>>()?;
                if scopes.iter().any(|b| *b) && scopes.iter().any(|b| !*b) {
                    return Err(invalid(
                        "mixed explicit and implicit hand scopes require separate queries",
                    ));
                }
                Ok(scopes.iter().any(|b| *b))
            }
            _ => Ok(false),
        }
    }
    fn object_target(&self, domain: ObjectDomain, ordinal: usize) -> Result<ObjectRef> {
        Ok(ObjectRef::Target {
            slot: self.target(TargetKind::Object(domain), ordinal, 0)?,
            index: 0,
        })
    }
    fn result_object(&self, result: Option<&ResultId>, index: u32) -> Result<ObjectRef> {
        Ok(ObjectRef::Result {
            result: result
                .cloned()
                .ok_or_else(|| invalid("movement reference has no transition in this scope"))?,
            phase: ObjectPhase::After,
            index,
        })
    }
    fn single_card(&self, value: &source::SingleCard) -> Result<ObjectRef> {
        match value {
            source::SingleCard::ThisCard => Ok(ObjectRef::Source),
            source::SingleCard::TheCardPutOntoTheBattlefieldThisWay => {
                self.result_object(self.entered.as_ref(), 0)
            }
            _ => Err(unsupported(value)),
        }
    }
    fn grave_card(&self, value: &source::CardInGraveyards) -> Result<ObjectRef> {
        if matches!(value, source::CardInGraveyards::TheChosenGraveyardCard)
            && self.grave_choice.is_none()
            && self.row_choice.is_some()
        {
            return self.result_object(self.row_choice.as_ref(), 0);
        }
        if let source::CardInGraveyards::TheCardMilledThisWay = value {
            return Ok(ObjectRef::Result {
                result: self.done(Done::Milled)?,
                phase: ObjectPhase::After,
                index: 0,
            });
        }
        use source::CardInGraveyards as S;
        if let S::TheCardConjuredIntoGraveyardThisWay = value {
            return self.conjured_object();
        }
        if let S::TheSacrificedPermanent = value {
            return self.paid_object(crate::number_spec::PaymentAction::Sacrifice);
        }
        if let (S::Trigger_ThatSacrificedPermanent, true) = (value, self.event.permanent) {
            return Ok(ObjectRef::EventSubject);
        }
        if let S::EnchantedGraveyardCard | S::HostGraveyardCard = value {
            return Ok(ObjectRef::AttachedTo(Box::new(ObjectRef::Source)));
        }
        let n = match value {
            S::Ref_TargetGraveyardCard => self.slot_of_arity(
                &TargetKind::Object(ObjectDomain::card(EventZone::Graveyard)),
                false,
                0,
            ),
            S::Ref_TargetGraveyardCard1 => 0,
            S::Ref_TargetGraveyardCard2 => 1,
            S::Ref_TargetGraveyardCard3 => 2,
            S::Ref_TargetGraveyardCard4 => 3,
            S::Ref_TargetGraveyardCard5 => 4,
            S::ThisGraveyardCard if self.event.enchanted_left => return self.departed_aura(),
            S::ThisGraveyardCard | S::ThisSacrificedPermanent => return Ok(ObjectRef::Source),
            S::TheCardPutIntoGraveyardThisWay => {
                return Ok(ObjectRef::Query {
                    objects: Box::new(self.graveyard_set()?),
                    index: 0,
                })
            }
            S::TheCardDiscardedThisWay => {
                use crate::number_spec::PaymentAction;
                if !self.did.contains_key(&Done::Discarded)
                    && self.paid.contains_key(&PaymentAction::Discard)
                {
                    let binding = self.paid_binding(PaymentAction::Discard)?;
                    if self.announcing || !binding.singular {
                        return Err(invalid(
                            "discarded-card reference requires one completed discard cost",
                        ));
                    }
                    return Ok(ObjectRef::Query {
                        objects: Box::new(ObjectSet::CostObjects {
                            payment: binding.payment.clone(),
                            component: binding.component.clone(),
                            view: CostObjectView::Performed,
                            phase: ObjectPhase::After,
                        }),
                        index: 0,
                    });
                }
                return Ok(ObjectRef::Result {
                    result: self.done(Done::Discarded)?,
                    phase: ObjectPhase::After,
                    index: 0,
                })
            }
            S::TheGraveyardCardChosenThisWay | S::TheChosenGraveyardCard
                if self.grave_choice.is_some() =>
            {
                return self.result_object(self.grave_choice.as_ref(), 0);
            }
            S::Ref_TargetGraveyardCardInPlayersGraveyard(player) => {
                return Ok(ObjectRef::Query {
                    objects: Box::new(ObjectSet::Matching {
                        objects: Box::new(ObjectSet::Targets(self.target(
                            TargetKind::Object(ObjectDomain::card(EventZone::Graveyard)),
                            0,
                            0,
                        )?)),
                        filter: ObjectFilter::OwnedBy(PlayerSet::One(self.player(player)?)),
                    }),
                    index: 0,
                })
            }
            S::TheGraveyardCardChosenThisWay if self.row_choice.is_some() => {
                return self.result_object(self.row_choice.as_ref(), 0);
            }
            S::Trigger_ThatGraveyardCard if self.event.moved || self.event.discarded => {
                let object = if self.event.moved {
                    ObjectRef::EventDestination
                } else {
                    ObjectRef::Discarded
                };
                return Ok(ObjectRef::Query {
                    objects: Box::new(ObjectSet::InZone {
                        objects: Box::new(ObjectSet::One(object)),
                        zone: EventZone::Graveyard,
                    }),
                    index: 0,
                });
            }
            S::TopCardOfPlayersGraveyard(p) => {
                return Ok(ObjectRef::Query {
                    objects: Box::new(self.private_cards(
                        EventZone::Graveyard,
                        PlayerSet::One(self.player(p)?),
                        ObjectFilter::Any,
                    )),
                    index: 0,
                })
            }
            S::TopCardOfTypeOfPlayersGraveyard(cards, p) => {
                return Ok(ObjectRef::Query {
                    objects: Box::new(self.private_cards(
                        EventZone::Graveyard,
                        PlayerSet::One(self.player(p)?),
                        self.card_filter(cards)?,
                    )),
                    index: 0,
                })
            }
            S::TheBottomCardOfPlayersGraveyard(p) => {
                return Ok(ObjectRef::Query {
                    objects: Box::new(ObjectSet::Take {
                        objects: Box::new(self.private_cards(
                            EventZone::Graveyard,
                            PlayerSet::One(self.player(p)?),
                            ObjectFilter::Any,
                        )),
                        count: Expr::Constant(1),
                        from: SequenceEnd::Bottom,
                    }),
                    index: 0,
                })
            }
            _ => return Err(unsupported(value)),
        };
        self.object_target(ObjectDomain::card(EventZone::Graveyard), n)
    }
    /// A follow-up action on "that card" finds every incarnation produced by
    /// the one original object (CR 712.21c). Explicit first/second/choice reads
    /// stay scalar. Callers retain the current-zone guard on this group.
    fn exiled_followup_group(&self, value: &source::CardInExile) -> Option<ObjectSet> {
        if !matches!(
            value,
            source::CardInExile::TheCardExiledThisWay | source::CardInExile::TheExiledCard
        ) {
            return None;
        }
        match &self.exiled {
            Some(result)
                if self
                    .exiled_one_input
                    .as_ref()
                    .is_some_and(|one| one.0 == result.0) =>
            {
                Some(ObjectSet::Transition {
                    result: result.clone(),
                    phase: ObjectPhase::After,
                })
            }
            None => self
                .paid_exile()
                .filter(|(_, one)| *one)
                .map(|(objects, _)| objects),
            _ => None,
        }
    }
    fn exiled_card(&self, value: &source::CardInExile) -> Result<ObjectRef> {
        use source::CardInExile as S;
        match value {
            S::TheCardConjuredIntoExileThisWay => self.conjured_object(),
            S::TheExiledDeadPermanent if self.paid_exile().is_some() => Ok(ObjectRef::Query {
                objects: Box::new(self.paid_exile().unwrap().0),
                index: 0,
            }),
            S::TheExiledTopOfLibrary => self.result_object(self.exiled.as_ref(), 0),
            S::TheFirstCardExiledThisWay | S::TheSecondCardExiledThisWay
                if self.exiled.is_none() && self.paid_exile().is_some() =>
            {
                Ok(ObjectRef::Query {
                    objects: Box::new(self.paid_exile().unwrap().0),
                    index: u32::from(matches!(value, S::TheSecondCardExiledThisWay)),
                })
            }
            // What a replacement sends to exile instead, once it has gone there.
            S::TheCardExiledThisWay if self.replaced_exile && self.exiled.is_none() => {
                Ok(ObjectRef::Query {
                    objects: Box::new(ObjectSet::InZone {
                        objects: Box::new(ObjectSet::One(ObjectRef::EventDestination)),
                        zone: EventZone::Exile,
                    }),
                    index: 0,
                })
            }
            S::EachableExiled if self.iterated_object.is_some() => {
                Ok(ObjectRef::Bound(self.iterated_object.clone().unwrap()))
            }
            S::TheExiledCardChosenThisWay | S::TheChosenExiledCard
                if self.exile_choice.is_some() =>
            {
                Ok(ObjectRef::Result {
                    result: self.exile_choice.clone().unwrap(),
                    phase: ObjectPhase::After,
                    index: 0,
                })
            }
            S::Trigger_ThatExiledCard | S::WhenAPermanentIsExiled_ThatExiledPermanent
                if self.event.moved =>
            {
                Ok(ObjectRef::Query {
                    objects: Box::new(ObjectSet::InZone {
                        objects: Box::new(ObjectSet::One(ObjectRef::EventDestination)),
                        zone: EventZone::Exile,
                    }),
                    index: 0,
                })
            }
            // Played from exile, it has left by then; the event still names the card.
            S::Trigger_ThatExiledCard if self.event.object => Ok(ObjectRef::EventSubject),
            S::Ref_TargetExiledCard | S::Ref_TargetExiledCard1 => {
                self.object_target(ObjectDomain::card(EventZone::Exile), 0)
            }
            S::Ref_TargetExiledCard2 => self.object_target(ObjectDomain::card(EventZone::Exile), 1),
            S::TheSpecificCardExiledThisWay | S::TheExiledCardFoundThisWay
                if self.exile_found.is_some() =>
            {
                let found = self.exile_found.clone().unwrap();
                // A later run of a player loop reads its own player's card.
                let filter = match &self.iterated {
                    Some(player @ PlayerRef::Bound(_)) => ObjectFilter::AllOf(vec![
                        found,
                        ObjectFilter::OwnedBy(PlayerSet::One(player.clone())),
                    ]),
                    _ => found,
                };
                Ok(ObjectRef::Query {
                    objects: Box::new(ObjectSet::Matching {
                        objects: Box::new(ObjectSet::Transition {
                            result: self
                                .exiled
                                .clone()
                                .ok_or_else(|| invalid("exiled cards have no movement"))?,
                            phase: ObjectPhase::After,
                        }),
                        filter,
                    }),
                    index: 0,
                })
            }
            S::TheCardExiledThisWay
            | S::TheSingleCardExiledThisWay
            | S::TheSinglePermanentExiledThisWay
            | S::TheSpecificCardExiledThisWay => match self.paid_exile() {
                Some((objects, true)) if self.exiled.is_none() => Ok(ObjectRef::Query {
                    objects: Box::new(objects),
                    index: 0,
                }),
                _ => self.result_object(self.exiled.as_ref(), 0),
            },
            S::ThisExiledCard | S::ThisExiledPermanentCard => Ok(ObjectRef::Source),
            S::TheExiledCard if self.exiled.is_none() => match self.paid_exile() {
                Some((objects, true)) => Ok(ObjectRef::Query {
                    objects: Box::new(objects),
                    index: 0,
                }),
                Some(_) => Err(invalid("the exiled card names a plural exile")),
                None => Ok(ObjectRef::Query {
                    objects: Box::new(self.exiled_with_source()),
                    index: 0,
                }),
            },
            S::TheExiledCard => self.result_object(self.exiled.as_ref(), 0),
            _ => self
                .primitive("CardInExile", value)
                .map(|(name, operands)| crate::semantics::reduce_object(name, operands))
                .ok_or_else(|| unsupported(value)),
        }
    }
    fn hand_card(&self, value: &source::CardInHand) -> Result<ObjectRef> {
        match value {
            source::CardInHand::ThisCardInHand => Ok(ObjectRef::Source),
            source::CardInHand::EachableCardInHand if self.iterated_object.is_some() => {
                Ok(ObjectRef::Bound(self.iterated_object.clone().unwrap()))
            }
            source::CardInHand::TheCardConjuredIntoHandThisWay
            | source::CardInHand::TheCardConjuredThisWay
            | source::CardInHand::TheCardDraftedThisWay
                if self.conjured.is_some() =>
            {
                self.conjured_object()
            }
            source::CardInHand::TheCardInHandChosenThisWay
            | source::CardInHand::TheCardChosenThisWay
            | source::CardInHand::TheChosenCardInHand
                if self.hand_choice.is_some() =>
            {
                Ok(ObjectRef::Result {
                    result: self.hand_choice.clone().unwrap(),
                    phase: ObjectPhase::After,
                    index: 0,
                })
            }
            source::CardInHand::TheCardSeekedThisWay
            | source::CardInHand::TheCardDraftedThisWay
            | source::CardInHand::TheCardConjuredThisWay
                if self.to_hand.is_some() =>
            {
                Ok(ObjectRef::Result {
                    result: self.to_hand.clone().unwrap(),
                    phase: ObjectPhase::After,
                    index: 0,
                })
            }
            source::CardInHand::TheCardSeekedThisWay if !self.seeked.is_empty() => {
                Ok(ObjectRef::Query {
                    objects: Box::new(self.seeked_cards()?),
                    index: 0,
                })
            }
            source::CardInHand::TheCardReturnedToHandThisWay if self.replaced_hand => {
                Ok(ObjectRef::Query {
                    objects: Box::new(ObjectSet::InZone {
                        objects: Box::new(ObjectSet::One(ObjectRef::EventDestination)),
                        zone: EventZone::Hand,
                    }),
                    index: 0,
                })
            }
            source::CardInHand::TheCardReturnedToHandThisWay
            | source::CardInHand::TheCardPutInHandThisWay => Ok(ObjectRef::Result {
                result: self
                    .to_hand
                    .clone()
                    .or_else(|| self.did.get(&Done::Returned).cloned())
                    .ok_or_else(|| invalid("nothing went to a hand in this scope"))?,
                phase: ObjectPhase::After,
                index: 0,
            }),
            // "You may reveal the card as you draw it": the event is that reveal.
            source::CardInHand::TheCardInHandRevealedThisWay
                if self.revealed.is_none() && self.event.object =>
            {
                Ok(ObjectRef::EventSubject)
            }
            source::CardInHand::TheCardRevealedThisWay
            | source::CardInHand::TheCardInHandRevealedThisWay
            | source::CardInHand::TheCardRevealedFromHandThisWay => Ok(ObjectRef::Query {
                objects: Box::new(self.revealed_objects()?),
                index: 0,
            }),
            source::CardInHand::Trigger_ThatCardInHand
            | source::CardInHand::Trigger_ThatDiscardedCard
                if self.event.discarded =>
            {
                Ok(ObjectRef::Discarded)
            }
            source::CardInHand::Trigger_ThatCardInHand if self.event.object => {
                Ok(ObjectRef::EventSubject)
            }
            _ => self
                .primitive("CardInHand", value)
                .map(|(name, operands)| crate::semantics::reduce_object(name, operands))
                .ok_or_else(|| unsupported(value)),
        }
    }
    fn private_cards(
        &self,
        zone: EventZone,
        players: PlayerSet,
        filter: ObjectFilter,
    ) -> ObjectSet {
        ObjectSet::Zone {
            scope: ZoneScope::Players { zone, players },
            kind: ObjectKind::Card,
            filter,
        }
    }
    fn grave_cards(&self, value: &source::CardsInGraveyards) -> Result<ObjectSet> {
        use source::CardsInGraveyards as S;
        Ok(match value {
            S::InTheGraveyardPileChosenThisWay => ObjectSet::Selected(self.pile_chosen()?),
            S::InTheGraveyardPileNotChosenThisWay => ObjectSet::Unselected(self.pile_chosen()?),
            S::Trigger_ThoseGraveyardCards if self.event.group => ObjectSet::EventGroup,
            S::ThePermanentsSacrificedThisWay => {
                self.this_way_set(Done::Sacrificed, ObjectPhase::After, None)?
            }
            S::Ref_TargetGraveyardCards | S::Ref_TargetGraveyardCards1 => {
                let kind = TargetKind::Object(ObjectDomain::card(EventZone::Graveyard));
                let ordinal = match value {
                    S::Ref_TargetGraveyardCards => self.slot_of_arity(&kind, true, 0),
                    _ => 0,
                };
                ObjectSet::Targets(self.target(kind, ordinal, 0)?)
            }
            S::Ref_TargetGraveyardCards2 => ObjectSet::Targets(self.target(
                TargetKind::Object(ObjectDomain::card(EventZone::Graveyard)),
                1,
                0,
            )?),
            S::SingleGraveyardCard(c) => ObjectSet::One(self.grave_card(c)?),
            S::TheChosenGraveyardCards | S::TheGraveyardCardsChosenThisWay
                if self.grave_choice.is_some() =>
            {
                let last = self.grave_choice.clone().unwrap();
                match self.grave_picks.last() {
                    Some(result) if result.0 == last.0 => Self::all_picked(&self.grave_picks),
                    _ => ObjectSet::Selected(last),
                }
            }
            S::TheDiscardedCardsChosenThisWay if self.did.contains_key(&Done::Discarded) => {
                let discard = self.done(Done::Discarded)?.0;
                let picks = self.picks.iter().map(|(_, result)| result.clone());
                let picks: Vec<ResultId> = picks.filter(|result| result.0 > discard).collect();
                if picks.is_empty() {
                    return Err(invalid("no card was chosen from among those discarded"));
                }
                Self::all_picked(&picks)
            }
            S::TheGraveyardCardsNotChosenThisWay if self.grave_choice.is_some() => {
                ObjectSet::Unselected(self.grave_choice.clone().unwrap())
            }
            S::CardsOfTypeMilledThisWay(cards) => ObjectSet::Matching {
                objects: Box::new(self.this_way_set(Done::Milled, ObjectPhase::After, None)?),
                filter: self.card_filter(cards)?,
            },
            _ => self.private_cards(
                EventZone::Graveyard,
                PlayerSet::All,
                self.grave_filter(value)?,
            ),
        })
    }
    /// What these choices chose, together.
    fn all_picked(picks: &[ResultId]) -> ObjectSet {
        match picks {
            [one] => ObjectSet::Selected(one.clone()),
            many => ObjectSet::Union(many.iter().cloned().map(ObjectSet::Selected).collect()),
        }
    }
    /// Another ability of this card did the exiling (607.2a).
    fn exiled_with_source(&self) -> ObjectSet {
        ObjectSet::Zone {
            scope: ZoneScope::Shared(EventZone::Exile),
            kind: ObjectKind::Card,
            filter: ObjectFilter::ExiledWith(ObjectRef::Source),
        }
    }
    fn exile_cards(&self, value: &source::CardsInExile) -> Result<ObjectSet> {
        Ok(match value {
            source::CardsInExile::InTheExiledPileChosenThisWay
            | source::CardsInExile::InTheChosenPile
                if self.chosen_pile.is_some() =>
            {
                ObjectSet::Selected(self.pile_chosen()?)
            }
            source::CardsInExile::InTheExiledPileNotChosenThisWay if self.chosen_pile.is_some() => {
                ObjectSet::Unselected(self.pile_chosen()?)
            }
            source::CardsInExile::TheExiledCardChosenThisWay if self.exile_choice.is_some() => {
                ObjectSet::Selected(self.exile_choice.clone().unwrap())
            }
            source::CardsInExile::TheCardsConjuredThisWay => self.conjured_cards()?,
            source::CardsInExile::Trigger_ThoseExiledCards if self.event.group => {
                ObjectSet::EventGroup
            }
            source::CardsInExile::TheExiledCardsChosenThisWay if self.exile_choice.is_some() => {
                ObjectSet::Selected(self.exile_choice.clone().unwrap())
            }
            source::CardsInExile::TheSpecificCardsExiledThisWay
            | source::CardsInExile::TheNonSpecificCardsExiledThisWay
                if self.exile_found.is_some() =>
            {
                let found = self.exile_found.clone().unwrap();
                ObjectSet::Matching {
                    objects: Box::new(self.this_way_set(Done::Exiled, ObjectPhase::After, None)?),
                    filter: match value {
                        source::CardsInExile::TheSpecificCardsExiledThisWay => found,
                        _ => ObjectFilter::Not(Box::new(found)),
                    },
                }
            }
            source::CardsInExile::TheCardsExiledThisWay | source::CardsInExile::TheExiledCards => {
                match (&self.exiled, self.paid_exile()) {
                    (Some(result), _) => ObjectSet::Transition {
                        result: result.clone(),
                        phase: ObjectPhase::After,
                    },
                    (None, Some((objects, _))) => objects,
                    (None, None) if matches!(value, source::CardsInExile::TheExiledCards) => {
                        self.exiled_with_source()
                    }
                    (None, None) => {
                        return Err(invalid("exiled cards have no movement in this scope"))
                    }
                }
            }
            source::CardsInExile::SingleExiledCard(c) => ObjectSet::One(self.exiled_card(c)?),
            source::CardsInExile::Or(values) => ObjectSet::Union(
                values
                    .iter()
                    .map(|cards| self.exile_cards(cards))
                    .collect::<Result<_>>()?,
            ),
            _ => ObjectSet::Zone {
                scope: ZoneScope::Shared(EventZone::Exile),
                kind: ObjectKind::Card,
                filter: self.exile_filter(value)?,
            },
        })
    }
    fn library_slice(&self, player: PlayerRef, count: Expr, from: SequenceEnd) -> ObjectSet {
        ObjectSet::Take {
            objects: Box::new(self.private_cards(
                EventZone::Library,
                PlayerSet::One(player),
                ObjectFilter::Any,
            )),
            count,
            from,
        }
    }
    /// Whose library or battlefield an action names when it names none.
    fn actor_or_you(&self, actor: &PlayerSet) -> Result<PlayerRef> {
        match actor {
            PlayerSet::One(player) => Ok(player.clone()),
            _ => self.player(&source::Player::You),
        }
    }

    fn exilable(
        &self,
        value: &source::Exilable,
        actor: &PlayerSet,
    ) -> Result<(ObjectSet, Option<Count>)> {
        use source::Exilable as S;
        let (objects, choose) = match value {
            S::Permanent(p) => (ObjectSet::One(self.object(p)?), None),
            S::EachPermanent(p) => (self.objects(p)?, None),
            S::EachCardInLibrary(cards) => (
                self.private_cards(
                    EventZone::Library,
                    actor.clone(),
                    self.library_filter(cards)?,
                ),
                None,
            ),
            S::APermanent(p) => (
                self.objects(p)?,
                Some(Count::AsManyAsCan(Expr::Constant(1))),
            ),
            S::NumberPermanents(n, p) => {
                (self.objects(p)?, Some(Count::AsManyAsCan(self.number(n)?)))
            }
            S::UptoOnePermanent(p) => (self.objects(p)?, Some(Count::UpTo(Expr::Constant(1)))),
            S::AnyNumberOfPermanents(p) => (self.objects(p)?, Some(Count::AnyNumber)),
            S::CardInGraveyards(c) => (ObjectSet::One(self.grave_card(c)?), None),
            S::EachCardInGraveyards(c) => (self.grave_cards(c)?, None),
            S::ACardFromHand | S::ACardFromHandAtRandom => (
                self.private_cards(EventZone::Hand, actor.clone(), ObjectFilter::Any),
                Some(Count::AsManyAsCan(Expr::Constant(1))),
            ),
            S::AnyNumberOfCardsFromHand => (
                self.private_cards(EventZone::Hand, actor.clone(), ObjectFilter::Any),
                Some(Count::AnyNumber),
            ),
            S::ARandomCardFromPlayersHand(player) => (
                self.private_cards(
                    EventZone::Hand,
                    PlayerSet::One(self.player(player)?),
                    ObjectFilter::Any,
                ),
                Some(Count::AsManyAsCan(Expr::Constant(1))),
            ),
            S::TheTopCardOfEachPlayersLibraries(players) => {
                (ObjectSet::LibraryTops(self.players(players)?), None)
            }
            S::ACardOfTypeFromHand(cards) => (
                self.private_cards(EventZone::Hand, actor.clone(), self.hand_filter(cards)?),
                Some(Count::AsManyAsCan(Expr::Constant(1))),
            ),
            S::AllCardsOfTypeFromLibrary(cards) => (
                self.private_cards(
                    EventZone::Library,
                    actor.clone(),
                    self.library_filter(cards)?,
                ),
                None,
            ),
            S::ACardFromPlayersLibraryAtRandom(player) => (
                self.private_cards(
                    EventZone::Library,
                    PlayerSet::One(self.player(player)?),
                    ObjectFilter::Any,
                ),
                Some(Count::AsManyAsCan(Expr::Constant(1))),
            ),
            S::ACardOfTypeFromPlayersLibraryAtRandom(cards, player) => (
                self.private_cards(
                    EventZone::Library,
                    PlayerSet::One(self.player(player)?),
                    self.library_filter(cards)?,
                ),
                Some(Count::AsManyAsCan(Expr::Constant(1))),
            ),
            S::NumberCardsFromPlayersLibraryAtRandom(n, player) => (
                self.private_cards(
                    EventZone::Library,
                    PlayerSet::One(self.player(player)?),
                    ObjectFilter::Any,
                ),
                Some(Count::AsManyAsCan(self.number(n)?)),
            ),
            S::ACardInGraveyardsAtRandom(c) => (
                self.grave_cards(c)?,
                Some(Count::AsManyAsCan(Expr::Constant(1))),
            ),
            S::NumberCardsInGraveyardsAtRandom(n, c) => (
                self.grave_cards(c)?,
                Some(Count::AsManyAsCan(self.number(n)?)),
            ),
            S::ACardOfTypeFromPlayersHand(cards, player) => (
                self.private_cards(
                    EventZone::Hand,
                    PlayerSet::One(self.player(player)?),
                    self.hand_filter(cards)?,
                ),
                Some(Count::AsManyAsCan(Expr::Constant(1))),
            ),
            S::NumberCardsFromHand(n) => (
                self.private_cards(EventZone::Hand, actor.clone(), ObjectFilter::Any),
                Some(Count::AsManyAsCan(self.number(n)?)),
            ),
            S::ACardInGraveyards(c) => (
                self.grave_cards(c)?,
                Some(Count::AsManyAsCan(Expr::Constant(1))),
            ),
            S::NumberCardsInGraveyards(n, c) => (
                self.grave_cards(c)?,
                Some(Count::AsManyAsCan(self.number(n)?)),
            ),
            S::UptoNumberCardsInGraveyards(n, c) => {
                (self.grave_cards(c)?, Some(Count::UpTo(self.number(n)?)))
            }
            S::UptoOneCardInGraveyards(c) => {
                (self.grave_cards(c)?, Some(Count::UpTo(Expr::Constant(1))))
            }
            S::AnyNumberOfCardsInGraveyards(c) => (self.grave_cards(c)?, Some(Count::AnyNumber)),
            S::UptoOneCardOfEachCardTypeAmongCardsInGraveyards(c) => (
                self.grave_cards(c)?,
                Some(Count::UpTo(Expr::Constant(each_card_type().len() as i64))),
            ),
            S::Spell(s) => (ObjectSet::One(self.spell(s)?), None),
            S::EachSpell(s) => (
                ObjectSet::Zone {
                    scope: ZoneScope::Shared(EventZone::Stack),
                    kind: ObjectKind::Spell,
                    filter: self.spell_filter(s)?,
                },
                None,
            ),
            S::TheTopCardOfLibrary => (
                self.library_slice(
                    self.actor_or_you(actor)?,
                    Expr::Constant(1),
                    SequenceEnd::Top,
                ),
                None,
            ),
            S::TheTopNumberCardsOfLibrary(n) => (
                self.library_slice(self.actor_or_you(actor)?, self.number(n)?, SequenceEnd::Top),
                None,
            ),
            S::TheBottomNumberCardsOfLibrary(n) => (
                self.library_slice(
                    self.actor_or_you(actor)?,
                    self.number(n)?,
                    SequenceEnd::Bottom,
                ),
                None,
            ),
            S::TheTopCardOfPlayersLibrary(p) => (
                self.library_slice(self.player(p)?, Expr::Constant(1), SequenceEnd::Top),
                None,
            ),
            S::TheTopNumberCardsOfPlayersLibrary(n, p) => (
                self.library_slice(self.player(p)?, self.number(n)?, SequenceEnd::Top),
                None,
            ),
            S::AllCardsFromLibrary => (
                self.private_cards(
                    EventZone::Library,
                    PlayerSet::One(self.actor_or_you(actor)?),
                    ObjectFilter::Any,
                ),
                None,
            ),
            S::AllCardsFromPlayersLibrary(p) => (
                self.private_cards(
                    EventZone::Library,
                    PlayerSet::One(self.player(p)?),
                    ObjectFilter::Any,
                ),
                None,
            ),
            S::PlayersHand(p) => (
                self.private_cards(
                    EventZone::Hand,
                    PlayerSet::One(self.player(p)?),
                    ObjectFilter::Any,
                ),
                None,
            ),
            S::EachPlayersHand(p) => (
                self.private_cards(EventZone::Hand, self.players(p)?, ObjectFilter::Any),
                None,
            ),
            S::CardInHand(c) => (ObjectSet::One(self.hand_card(c)?), None),
            S::EachCardInHand(c) => (
                self.private_cards(
                    EventZone::Hand,
                    if self.hand_has_scope(c)? {
                        PlayerSet::All
                    } else {
                        actor.clone()
                    },
                    self.hand_filter(c)?,
                ),
                None,
            ),
            _ => {
                let (name, operands) = self
                    .primitive("Exilable", value)
                    .ok_or_else(|| unsupported(value))?;
                (crate::semantics::reduce_objects(name, operands), None)
            }
        };
        Ok((objects, choose))
    }
    /// An exile reaching into each of several players' zones is that exile once
    /// per player. A pick stays with the actor; a library names its own owner.
    fn exile_from_each(
        item: &source::Exilable,
        flags: &[source::ExileFlag],
    ) -> Option<source::Action> {
        use source::{
            Action as A, CardsInGraveyards as C, Exilable as S, GameNumber as N, Player as P,
            Players as Ps,
        };
        let exile = |item| A::Exile(vec![item], flags.to_vec());
        let in_graveyard = |cards: &C| {
            Box::new(C::And(vec![
                cards.clone(),
                C::InAPlayersGraveyard(Box::new(Ps::SinglePlayer(Box::new(P::EachablePlayer)))),
            ]))
        };
        Some(match item {
            S::TheBottomCardOfEachPlayersLibraries(players) => A::EachPlayerAction(
                players.clone(),
                Box::new(exile(S::TheBottomNumberCardsOfLibrary(Box::new(
                    N::Integer(1),
                )))),
            ),
            S::TheTopNumberCardsOfEachPlayersLibrary(n, players) => A::EachPlayerAction(
                players.clone(),
                Box::new(exile(S::TheTopNumberCardsOfLibrary(n.clone()))),
            ),
            S::ACardInGraveyardsAtRandomInEachPlayersGraveyard(cards, players) => {
                A::ActionForEachPlayer(
                    players.clone(),
                    vec![exile(S::ACardInGraveyardsAtRandom(in_graveyard(cards)))],
                )
            }
            S::ACardOfTypeInEachPlayersGraveyard(cards, players) => A::ActionForEachPlayer(
                players.clone(),
                vec![exile(S::ACardInGraveyards(in_graveyard(cards)))],
            ),
            S::UptoOneCardOfTypeInEachPlayersGraveyard(cards, players) => A::ActionForEachPlayer(
                players.clone(),
                vec![exile(S::UptoOneCardInGraveyards(in_graveyard(cards)))],
            ),
            S::ACardOfTypeFromEachPlayersHand(cards, players) => A::ActionForEachPlayer(
                players.clone(),
                vec![exile(S::ACardOfTypeFromPlayersHand(
                    cards.clone(),
                    Box::new(P::EachablePlayer),
                ))],
            ),
            _ => return None,
        })
    }
    fn exile_entry(&self, flags: &[source::ExileFlag]) -> Result<(EntrySpec, bool)> {
        let mut entry = EntrySpec::default();
        let mut face_down = false;
        for (i, flag) in flags.iter().enumerate() {
            match flag {
                source::ExileFlag::ExilesFaceUp => face_down = false,
                source::ExileFlag::ExilesFaceDown => face_down = true,
                source::ExileFlag::ExilesIntoFaceUpPile => {
                    entry.states.push(EntryState::ExilePile { shuffled: false })
                }
                source::ExileFlag::ExilesIntoFaceDownPile => {
                    face_down = true;
                    entry.states.push(EntryState::ExilePile { shuffled: false })
                }
                source::ExileFlag::ExilesIntoShuffledFaceDownPile => {
                    face_down = true;
                    entry.states.push(EntryState::ExilePile { shuffled: true })
                }
                source::ExileFlag::ExilesWithACounter(k) => entry
                    .counters
                    .push((crate::counter::lower(k), Expr::Constant(1))),
                source::ExileFlag::ExilesWithNumberCounters(n, k) => entry
                    .counters
                    .push((crate::counter::lower(k), self.number(n)?)),
                _ => {
                    let (name, operands) = self
                        .primitive("ExileFlag", flag)
                        .ok_or_else(|| unsupported(flag).at(format!("flags[{i}]")))?;
                    entry
                        .states
                        .push(crate::semantics::reduce_entry_flag(name, operands));
                }
            }
        }
        Ok((entry, face_down))
    }
    fn exile(
        &mut self,
        items: &[source::Exilable],
        flags: &[source::ExileFlag],
        actor: &PlayerSet,
    ) -> Result<Instruction> {
        if let [item] = items {
            if let Some(action) = Self::exile_from_each(item, flags) {
                return self.action(&action, actor);
            }
        }
        // "Exile until": one effect, ending in a return to where the object was.
        let period = flags.iter().find_map(|flag| match flag {
            source::ExileFlag::Limited(period) => Some(period),
            _ => None,
        });
        let period = period.map(|p| self.duration(p)).transpose()?;
        let flags: Vec<_> = flags
            .iter()
            .filter(|flag| !matches!(flag, source::ExileFlag::Limited(_)))
            .cloned()
            .collect();
        let (entry, face_down) = self.exile_entry(&flags)?;
        let mut instructions = Vec::new();
        let mut sets = Vec::new();
        self.exile_found = None;
        for (i, item) in items.iter().enumerate() {
            if let source::Exilable::CardsFromTheTopOfLibraryUntilGroupCardsAreExiled(group) = item
            {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("exiling from a library needs one player"));
                };
                let constraints = self.group_constraints(group)?;
                let seen = self.effect(Operation::RevealUntilGroup {
                    library: player.clone(),
                    constraints,
                });
                let Instruction::Effect { result, .. } = &seen else {
                    unreachable!()
                };
                sets.push(ObjectSet::Observed(result.clone()));
                instructions.push(seen);
                continue;
            }
            if let source::Exilable::CardsFromTheTopOfLibraryUntilACardOfTypeIsExiled(_)
            | source::Exilable::CardsFromTheTopOfLibraryUntilANumberOfCardsOfTypeAreExiled(
                ..,
            ) = item
            {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("exiling from a library needs one player"));
                };
                let (cards, count) = match item {
                    source::Exilable::CardsFromTheTopOfLibraryUntilANumberOfCardsOfTypeAreExiled(
                        n,
                        cards,
                    ) => (cards, self.number(n)?),
                    source::Exilable::CardsFromTheTopOfLibraryUntilACardOfTypeIsExiled(cards) => {
                        (cards, Expr::Constant(1))
                    }
                    _ => unreachable!(),
                };
                let filter = self.library_filter(cards)?;
                let seen = self.effect(Operation::RevealUntil {
                    library: player.clone(),
                    filter: filter.clone(),
                    count,
                    at_most: None,
                });
                let Instruction::Effect { result, .. } = &seen else {
                    unreachable!()
                };
                sets.push(ObjectSet::Observed(result.clone()));
                instructions.push(seen);
                self.exile_found = Some(filter);
                continue;
            }
            let (mut objects, choose) = self
                .exilable(item, actor)
                .map_err(|e| e.at(format!("objects[{i}]")))?;
            if let Some(count) = choose {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("exile choice needs one explicit chooser"));
                };
                let random = matches!(
                    item,
                    source::Exilable::ACardFromHandAtRandom
                        | source::Exilable::ARandomCardFromPlayersHand(_)
                        | source::Exilable::ACardFromPlayersLibraryAtRandom(_)
                        | source::Exilable::ACardOfTypeFromPlayersLibraryAtRandom(..)
                        | source::Exilable::NumberCardsFromPlayersLibraryAtRandom(..)
                        | source::Exilable::ACardInGraveyardsAtRandom(_)
                        | source::Exilable::NumberCardsInGraveyardsAtRandom(..)
                );
                let choice = self.effect(Operation::ChooseObjects {
                    constraints: if random {
                        vec![SelectionConstraint::AtRandom]
                    } else if matches!(
                        item,
                        source::Exilable::UptoOneCardOfEachCardTypeAmongCardsInGraveyards(_)
                    ) {
                        vec![SelectionConstraint::OneForEach(each_card_type())]
                    } else {
                        vec![]
                    },
                    player: player.clone(),
                    objects,
                    count,
                });
                let Instruction::Effect { result, .. } = &choice else {
                    unreachable!()
                };
                objects = ObjectSet::Selected(result.clone());
                instructions.push(choice);
            }
            sets.push(objects);
        }
        let objects = if sets.len() == 1 {
            sets.pop().unwrap()
        } else {
            ObjectSet::Union(sets)
        };
        let movement = self.effect(
            crate::movement::move_to(
                objects,
                Destination::Exile,
                entry,
                face_down,
                crate::ir::MovementTerms::default(),
            )
            .map_err(|message| invalid(&message))?,
        );
        self.exiled_permanents = items.iter().all(|item| {
            matches!(
                item,
                source::Exilable::Permanent(_)
                    | source::Exilable::EachPermanent(_)
                    | source::Exilable::APermanent(_)
                    | source::Exilable::NumberPermanents(..)
                    | source::Exilable::UptoOnePermanent(_)
                    | source::Exilable::AnyNumberOfPermanents(_)
            )
        });
        let moved = match &movement {
            Instruction::Effect { result, .. } => result.clone(),
            _ => unreachable!(),
        };
        instructions.push(movement);
        if let Some(duration) = period {
            instructions.push(self.effect(Operation::ReturnAfter {
                objects: ObjectSet::Transition {
                    result: moved,
                    phase: ObjectPhase::After,
                },
                duration,
            }));
        }
        Ok(if instructions.len() == 1 {
            instructions.pop().unwrap()
        } else {
            Instruction::Group(Program { instructions })
        })
    }
    /// "Return a card from your graveyard to your hand": one chosen, then moved.
    /// A card put from a graveyard into a hand comes from that player's own graveyard:
    /// every card with the row says so, and its filter names no graveyard.
    pub(crate) fn own_graveyard(objects: ObjectSet, player: &PlayerRef) -> ObjectSet {
        let mut objects = match objects {
            ObjectSet::Zone {
                scope:
                    ZoneScope::Players {
                        zone: EventZone::Graveyard,
                        players: PlayerSet::All,
                    },
                kind,
                filter,
            } => ObjectSet::Zone {
                scope: ZoneScope::Players {
                    zone: EventZone::Graveyard,
                    players: PlayerSet::One(player.clone()),
                },
                kind,
                filter,
            },
            objects => objects,
        };
        // The population already supplies this zone and owner. Remove only
        // an identical qualification; an opponent's graveyard remains a real
        // restriction and cannot be widened to the payer's.
        if let ObjectSet::Zone { scope, filter, .. } = &mut objects {
            match filter {
                ObjectFilter::InZone(inner) if serde_json::to_value(&*scope).unwrap() == serde_json::to_value(&*inner).unwrap() => *filter = ObjectFilter::Any,
                ObjectFilter::AllOf(items) => {
                    let scope = serde_json::to_value(&*scope).unwrap();
                    items.retain(|item| !matches!(item, ObjectFilter::InZone(inner) if serde_json::to_value(inner).unwrap() == scope));
                    if items.is_empty() { *filter = ObjectFilter::Any; }
                    else if items.len() == 1 { *filter = items.pop().unwrap(); }
                }
                _ => {}
            }
        }
        objects
    }
    fn put_from_graveyard_into_hand(
        &mut self,
        cards: &source::CardsInGraveyards,
        actor: &PlayerSet,
    ) -> Result<Instruction> {
        let PlayerSet::One(player) = actor else {
            return Err(invalid(
                "returning a card from a graveyard needs one player",
            ));
        };
        let objects = Self::own_graveyard(self.grave_cards(cards)?, player);
        let choice = self.effect(Operation::ChooseObjects {
            constraints: vec![],
            player: player.clone(),
            objects,
            count: Count::Exactly(Expr::Constant(1)),
        });
        let Instruction::Effect { result, .. } = &choice else {
            unreachable!()
        };
        let objects = ObjectSet::Selected(result.clone());
        let movement = self.effect(crate::movement::move_plain(
            objects,
            Destination::OwnersHand,
        ));
        Ok(Instruction::Group(Program {
            instructions: vec![choice, movement],
        }))
    }
    /// The library is shuffled even if there was nothing to move.
    fn shuffle_into_library(
        &mut self,
        zones: &[EventZone],
        actor: &PlayerSet,
    ) -> Result<Instruction> {
        let objects = zones
            .iter()
            .map(|zone| self.private_cards(zone.clone(), actor.clone(), ObjectFilter::Any))
            .collect();
        let movement = self.effect(crate::movement::move_plain(
            ObjectSet::Union(objects),
            Destination::OwnersLibraryTop,
        ));
        let shuffle = self.effect(Operation::Shuffle {
            players: actor.clone(),
            excluding: None,
        });
        Ok(Instruction::Group(Program {
            instructions: vec![movement, shuffle],
        }))
    }
    /// "Put a card from your hand onto the battlefield": `count` chosen, or all.
    fn put_from_hand(
        &mut self,
        cards: &source::CardsInHand,
        flags: &[source::EnterFlag],
        count: Option<Count>,
        actor: &PlayerSet,
    ) -> Result<Instruction> {
        let PlayerSet::One(player) = actor else {
            return Err(invalid("putting a card from hand needs one player"));
        };
        if self.hand_has_scope(cards)? {
            return Err(unsupported(cards));
        }
        let (destination, entry) = self.move_entry(flags, player.clone())?;
        let filter = self.hand_filter(cards)?;
        let objects = self.private_cards(EventZone::Hand, actor.clone(), filter);
        let Some(count) = count else {
            let movement = self.effect(
                crate::movement::move_to(
                    objects,
                    destination,
                    entry,
                    false,
                    crate::ir::MovementTerms::default(),
                )
                .map_err(|message| invalid(&message))?,
            );
            self.note_moves(&Program {
                instructions: vec![movement.clone()],
            });
            return Ok(movement);
        };
        let choice = self.effect(Operation::ChooseObjects {
            constraints: vec![],
            player: player.clone(),
            objects,
            count,
        });
        let Instruction::Effect { result, .. } = &choice else {
            unreachable!()
        };
        let objects = ObjectSet::Selected(result.clone());
        let movement = self.effect(
            crate::movement::move_to(
                objects,
                destination,
                entry,
                false,
                crate::ir::MovementTerms::default(),
            )
            .map_err(|message| invalid(&message))?,
        );
        let body = Program {
            instructions: vec![choice, movement],
        };
        self.note_moves(&body);
        Ok(Instruction::Group(body))
    }
    fn skip_next_untap(&self, objects: ObjectSet) -> Result<Operation> {
        Ok(Operation::CreateContinuousEffect {
            source: ObjectRef::Source,
            affected: EffectSubjects::Objects(objects),
            changes: self
                .rule_changes(&[source::PermanentRule::DoesntUntapDuringControllersUntap])?,
            duration: Duration::DuringControllersNextUntapStep,
            evaluation: EffectEvaluation::LiveObjects,
        })
    }
    fn move_entry(
        &self,
        flags: &[source::EnterFlag],
        default: PlayerRef,
    ) -> Result<(Destination, EntrySpec)> {
        let mut entry = EntrySpec::default();
        let mut destination = Destination::Battlefield(default.clone());
        for (i, flag) in flags.iter().enumerate() {
            match flag {
                source::EnterFlag::EntersNormally => {}
                source::EnterFlag::EntersTapped => entry.tapped = true,
                source::EnterFlag::IfPassesFilter(filter, flags) => {
                    let (_, then) = self.move_entry(flags, default.clone())?;
                    entry.states.push(EntryState::When {
                        filter: self.filter(filter)?,
                        entry: Box::new(then),
                    });
                }
                source::EnterFlag::If(condition, flags) => {
                    let source::Condition::EnteringPermanentPassesFilter(filter) = condition else {
                        return Err(unsupported(flag).at(format!("flags[{i}]")));
                    };
                    let (_, then) = self.move_entry(flags, default.clone())?;
                    entry.states.push(EntryState::When {
                        filter: self.filter(filter)?,
                        entry: Box::new(then),
                    });
                }
                source::EnterFlag::EntersWithACounter(k) => entry
                    .counters
                    .push((crate::counter::lower(k), Expr::Constant(1))),
                source::EnterFlag::EntersWithNumberCounters(n, k) => entry
                    .counters
                    .push((crate::counter::lower(k), self.number(n)?)),
                source::EnterFlag::EntersUnderPlayersControl(p) => {
                    destination = Destination::Battlefield(self.player(p)?)
                }
                source::EnterFlag::EntersUnderOwnersControl => {
                    destination = Destination::OwnersBattlefield
                }
                source::EnterFlag::EntersUnderAPlayersControl(players) => {
                    destination =
                        Destination::Battlefield(PlayerRef::OneOf(Box::new(self.players(players)?)))
                }
                source::EnterFlag::EntersTransformed | source::EnterFlag::EntersConverted => {
                    entry.states.push(EntryState::Transformed)
                }
                source::EnterFlag::EntersFaceDown => entry.states.push(EntryState::FaceDown),
                source::EnterFlag::EntersFlipped => entry.states.push(EntryState::Flipped),
                source::EnterFlag::IfCardPassesFilter(cards, flags) => {
                    let (_, then) = self.move_entry(flags, default.clone())?;
                    entry.states.push(EntryState::When {
                        filter: self.card_filter(cards)?,
                        entry: Box::new(then),
                    });
                }
                source::EnterFlag::EntersAttacking => entry.states.push(EntryState::Attacking {
                    player: None,
                    or_planeswalker: false,
                }),
                source::EnterFlag::EntersAttackingPlayer(p) => {
                    entry.states.push(EntryState::Attacking {
                        player: Some(self.player(p)?),
                        or_planeswalker: false,
                    })
                }
                source::EnterFlag::EntersAttackingPlayerOrPlaneswalkerControlledBy(p) => {
                    entry.states.push(EntryState::Attacking {
                        player: Some(self.player(p)?),
                        or_planeswalker: true,
                    })
                }
                source::EnterFlag::EntersBlockingAttacker(p) => {
                    entry.states.push(EntryState::Blocking(self.object(p)?))
                }
                source::EnterFlag::EntersAttachedToPermanent(p) => {
                    entry.states.push(EntryState::AttachedTo(self.object(p)?))
                }
                source::EnterFlag::EntersAttachedToAPermanent(p) => entry
                    .states
                    .push(EntryState::AttachedToChosen(self.filter(p)?)),
                source::EnterFlag::EntersAttachedToPlayer(p) => entry
                    .states
                    .push(EntryState::AttachedToPlayer(self.player(p)?)),
                source::EnterFlag::EntersWithLayerEffect(changes) => entry
                    .states
                    .push(EntryState::Changes(self.changes(changes)?)),
                source::EnterFlag::EntersWithLayerEffectUntil(changes, duration) => {
                    entry.states.push(EntryState::ChangesUntil {
                        changes: self.changes(changes)?,
                        duration: self.duration(duration)?,
                    })
                }
                _ => {
                    let (name, operands) = self
                        .primitive("EnterFlag", flag)
                        .ok_or_else(|| unsupported(flag).at(format!("entry[{i}]")))?;
                    entry
                        .states
                        .push(crate::semantics::reduce_entry_flag(name, operands));
                }
            }
        }
        Ok((destination, entry))
    }
}
