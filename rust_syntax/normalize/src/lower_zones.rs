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
        use source::CardInGraveyards as S;
        let n = match value {
            S::Ref_TargetGraveyardCard | S::Ref_TargetGraveyardCard1 => 0,
            S::Ref_TargetGraveyardCard2 => 1,
            S::Ref_TargetGraveyardCard3 => 2,
            S::Ref_TargetGraveyardCard4 => 3,
            S::Ref_TargetGraveyardCard5 => 4,
            S::ThisGraveyardCard => return Ok(ObjectRef::Source),
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
    fn exiled_card(&self, value: &source::CardInExile) -> Result<ObjectRef> {
        use source::CardInExile as S;
        match value {
            S::Ref_TargetExiledCard | S::Ref_TargetExiledCard1 => {
                self.object_target(ObjectDomain::card(EventZone::Exile), 0)
            }
            S::Ref_TargetExiledCard2 => self.object_target(ObjectDomain::card(EventZone::Exile), 1),
            S::TheCardExiledThisWay
            | S::TheSingleCardExiledThisWay
            | S::TheSinglePermanentExiledThisWay
            | S::TheSpecificCardExiledThisWay => self.result_object(self.exiled.as_ref(), 0),
            S::ThisExiledCard | S::ThisExiledPermanentCard => Ok(ObjectRef::Source),
            _ => Err(unsupported(value)),
        }
    }
    fn hand_card(&self, value: &source::CardInHand) -> Result<ObjectRef> {
        match value {
            source::CardInHand::ThisCardInHand => Ok(ObjectRef::Source),
            source::CardInHand::Trigger_ThatCardInHand
            | source::CardInHand::Trigger_ThatDiscardedCard
                if self.event.discarded =>
            {
                Ok(ObjectRef::Discarded)
            }
            _ => Err(unsupported(value)),
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
            S::Ref_TargetGraveyardCards | S::Ref_TargetGraveyardCards1 => {
                ObjectSet::Targets(self.target(
                    TargetKind::Object(ObjectDomain::card(EventZone::Graveyard)),
                    0,
                    0,
                )?)
            }
            S::Ref_TargetGraveyardCards2 => ObjectSet::Targets(self.target(
                TargetKind::Object(ObjectDomain::card(EventZone::Graveyard)),
                1,
                0,
            )?),
            S::SingleGraveyardCard(c) => ObjectSet::One(self.grave_card(c)?),
            _ => self.private_cards(
                EventZone::Graveyard,
                PlayerSet::All,
                self.grave_filter(value)?,
            ),
        })
    }
    fn exile_cards(&self, value: &source::CardsInExile) -> Result<ObjectSet> {
        Ok(match value {
            source::CardsInExile::TheCardsExiledThisWay | source::CardsInExile::TheExiledCards => {
                ObjectSet::Transition {
                    result: self
                        .exiled
                        .clone()
                        .ok_or_else(|| invalid("exiled cards have no movement in this scope"))?,
                    phase: ObjectPhase::After,
                }
            }
            source::CardsInExile::SingleExiledCard(c) => ObjectSet::One(self.exiled_card(c)?),
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
    fn exilable(
        &self,
        value: &source::Exilable,
        actor: &PlayerSet,
    ) -> Result<(ObjectSet, Option<Count>)> {
        use source::Exilable as S;
        let (objects, choose) = match value {
            S::Permanent(p) => (ObjectSet::One(self.object(p)?), None),
            S::EachPermanent(p) => (self.objects(p)?, None),
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
                    self.player(&source::Player::You)?,
                    Expr::Constant(1),
                    SequenceEnd::Top,
                ),
                None,
            ),
            S::TheTopNumberCardsOfLibrary(n) => (
                self.library_slice(
                    self.player(&source::Player::You)?,
                    self.number(n)?,
                    SequenceEnd::Top,
                ),
                None,
            ),
            S::TheBottomNumberCardsOfLibrary(n) => (
                self.library_slice(
                    self.player(&source::Player::You)?,
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
                    PlayerSet::One(self.player(&source::Player::You)?),
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
            _ => return Err(unsupported(value)),
        };
        Ok((objects, choose))
    }
    fn exile_entry(&self, flags: &[source::ExileFlag]) -> Result<(EntrySpec, bool)> {
        let mut entry = EntrySpec::default();
        let mut face_down = false;
        for (i, flag) in flags.iter().enumerate() {
            match flag {
                source::ExileFlag::ExilesFaceUp => face_down = false,
                source::ExileFlag::ExilesFaceDown => face_down = true,
                source::ExileFlag::ExilesWithACounter(k) => entry
                    .counters
                    .push((crate::counter::lower(k), Expr::Constant(1))),
                source::ExileFlag::ExilesWithNumberCounters(n, k) => entry
                    .counters
                    .push((crate::counter::lower(k), self.number(n)?)),
                _ => return Err(unsupported(flag).at(format!("flags[{i}]"))),
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
        for (i, item) in items.iter().enumerate() {
            let (mut objects, choose) = self
                .exilable(item, actor)
                .map_err(|e| e.at(format!("objects[{i}]")))?;
            if let Some(count) = choose {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("exile choice needs one explicit chooser"));
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
        let movement = self.effect(Operation::Move {
            objects,
            destination: Destination::Exile,
            entry,
            face_down,
        });
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
            return Ok(self.effect(Operation::Move {
                objects,
                destination,
                entry,
                face_down: false,
            }));
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
        let movement = self.effect(Operation::Move {
            objects,
            destination,
            entry,
            face_down: false,
        });
        Ok(Instruction::Group(Program {
            instructions: vec![choice, movement],
        }))
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
        let mut destination = Destination::Battlefield(default);
        for (i, flag) in flags.iter().enumerate() {
            match flag {
                source::EnterFlag::EntersNormally => {}
                source::EnterFlag::EntersTapped => entry.tapped = true,
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
                source::EnterFlag::EntersTransformed => entry.states.push(EntryState::Transformed),
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
                source::EnterFlag::EntersWithLayerEffect(changes) => {
                    entry.states.push(EntryState::Changes(self.changes(changes)?))
                }
                source::EnterFlag::EntersWithLayerEffectUntil(changes, duration) => {
                    entry.states.push(EntryState::ChangesUntil {
                        changes: self.changes(changes)?,
                        duration: self.duration(duration)?,
                    })
                }
                _ => return Err(unsupported(flag).at(format!("entry[{i}]"))),
            }
        }
        Ok((destination, entry))
    }
}
