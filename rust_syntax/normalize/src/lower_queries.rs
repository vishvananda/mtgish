// Shared typed queries used by event matching, production restrictions, and effects.
impl Context {
    fn player_matches(&self, player: PlayerRef, filter: &source::Players) -> Result<Condition> {
        let outer = self.self_player.replace(Some(player.clone()));
        let check = self
            .reduced_player_check(player.clone(), filter)
            .or_else(|error| {
                match self.history_player_check(&player, filter) {
                    Some(check) => Ok(check),
                    None => rescue(error, || self.primitive_player_check(&player, filter)),
                }
            });
        self.self_player.replace(outer);
        check
    }

    fn reduced_player_check(
        &self,
        player: PlayerRef,
        filter: &source::Players,
    ) -> Result<Condition> {
        use source::Players as S;
        if let Some(check) = self.took_check(&player, filter)? {
            return Ok(check);
        }
        Ok(match filter {
            S::NumCardTypesInGraveyardIs(c) => {
                let (comparison, right) = self.comparison(c)?;
                Condition::Compare {
                    left: Expr::CountDistinct {
                        objects: Box::new(self.private_cards(
                            EventZone::Graveyard,
                            PlayerSet::One(player),
                            ObjectFilter::Any,
                        )),
                        property: DistinctProperty::CardTypes,
                    },
                    comparison,
                    right,
                }
            }
            S::NumCardsInHandIs(c) | S::NumCardsInGraveyardIs(c) => {
                let (comparison, right) = self.comparison(c)?;
                Condition::Compare {
                    left: Expr::CountObjects(Box::new(self.private_cards(
                        if matches!(filter, S::NumCardsInHandIs(_)) {
                            EventZone::Hand
                        } else {
                            EventZone::Graveyard
                        },
                        PlayerSet::One(player),
                        ObjectFilter::Any,
                    ))),
                    comparison,
                    right,
                }
            }
            S::NumCardsOfTypeInGraveyardIs(c, cards) => {
                let (comparison, right) = self.comparison(c)?;
                Condition::Compare {
                    left: Expr::CountObjects(Box::new(self.private_cards(
                        EventZone::Graveyard,
                        PlayerSet::One(player),
                        self.grave_filter(cards)?,
                    ))),
                    comparison,
                    right,
                }
            }
            S::IsTheirTurn => Condition::PlayerIsActive(player),
            S::IsNotTheirTurn => Condition::Not(Box::new(Condition::PlayerIsActive(player))),
            S::And(items) => Condition::All(
                items
                    .iter()
                    .map(|f| self.player_matches(player.clone(), f))
                    .collect::<Result<_>>()?,
            ),
            S::Or(items) => Condition::Either(
                items
                    .iter()
                    .map(|f| self.player_matches(player.clone(), f))
                    .collect::<Result<_>>()?,
            ),
            S::ControlsA(objects) => Condition::Controls {
                player,
                filter: self.filter(objects)?,
                minimum: 1,
            },
            S::ControlsNo(objects) => Condition::Not(Box::new(Condition::Controls {
                player,
                filter: self.filter(objects)?,
                minimum: 1,
            })),
            S::ControlsNum(comparison, objects) => {
                let (comparison, right) = self.comparison(comparison)?;
                Condition::Compare {
                    left: Expr::CountObjects(Box::new(ObjectSet::Battlefield(ObjectFilter::All(
                        vec![
                            self.filter(objects)?,
                            ObjectFilter::ControlledBy(PlayerSet::One(player)),
                        ],
                    )))),
                    comparison,
                    right,
                }
            }
            S::LifeTotalIs(comparison) => {
                let (comparison, right) = self.comparison(comparison)?;
                Condition::Compare {
                    left: Expr::PlayerLife(player),
                    comparison,
                    right,
                }
            }
            S::HasACounterOfType(k)
            | S::HasNoCountersOfType(k)
            | S::HasANumberOfCountersOfType(_, k) => {
                let (comparison, right) = match filter {
                    S::HasACounterOfType(_) => (Comparison::Greater, Expr::Constant(0)),
                    S::HasNoCountersOfType(_) => (Comparison::Equal, Expr::Constant(0)),
                    S::HasANumberOfCountersOfType(c, _) => self.comparison(c)?,
                    _ => unreachable!(),
                };
                Condition::Compare {
                    left: Expr::CountCounters {
                        recipients: Box::new(Recipients::Players(PlayerSet::One(player))),
                        kind: Some(crate::counter::lower(k)),
                        information: Information::CurrentOrLastKnown,
                    },
                    comparison,
                    right,
                }
            }
            _ => return Err(unsupported(filter)),
        })
    }
    fn color_filter(&self, value: &source::Color) -> Result<ObjectFilter> {
        Ok(match value {
            source::Color::White => ObjectFilter::Color(ManaColor::White),
            source::Color::Blue => ObjectFilter::Color(ManaColor::Blue),
            source::Color::Black => ObjectFilter::Color(ManaColor::Black),
            source::Color::Red => ObjectFilter::Color(ManaColor::Red),
            source::Color::Green => ObjectFilter::Color(ManaColor::Green),
            source::Color::Colorless => ObjectFilter::Colorless,
            _ => {
                return self
                    .primitive_predicate("Color", value)
                    .ok_or_else(|| unsupported(value))
            }
        })
    }

    fn spell(&self, value: &source::Spell) -> Result<ObjectRef> {
        match value {
            source::Spell::ThisSpell => Ok(ObjectRef::Source),
            source::Spell::Ref_TargetSpell => self.object_target(ObjectDomain::spell(), 0),
            source::Spell::ThatEnteringPermanent if self.event.permanent => {
                Ok(ObjectRef::EventSubject)
            }
            source::Spell::Trigger_ThatSpell if self.event.spell => Ok(ObjectRef::EventSubject),
            source::Spell::Trigger_ThatSpell if self.event.stack_object => {
                Ok(ObjectRef::EventStackObject)
            }
            _ => Err(unsupported(value)),
        }
    }

    fn spell_filter(&self, value: &source::Spells) -> Result<ObjectFilter> {
        let mut context = self.clone();
        context.candidate = true;
        context
            .spell_filter_inner(value)
            .or_else(|error| rescue(error, || context.primitive_predicate("Spells", value)))
            .or_else(|error| context.as_permanent_filter(value, error))
    }
    fn spell_filter_inner(&self, value: &source::Spells) -> Result<ObjectFilter> {
        if let Some(result)=self.pipeline_selector(crate::pipeline::selectors::SelectorRef::Spells(value)) {return result;}
        use source::Spells as S;
        Ok(match value {
            S::Ref_TargetSpells => ObjectFilter::InSet(Box::new(ObjectSet::Targets(
                self.target(TargetKind::Object(ObjectDomain::spell()), 0, 0)?,
            ))),
            S::CastByPlayer(p) => self.cast_by(PlayerSet::One(self.player(p)?)),
            S::CastByAPlayer(p) => self.cast_by(self.players(p)?),
            S::CastByPlayerFromHand(p, owner) => self.cast_history_filter(
                &source::Permanents::CastByPlayerFromHand(p.clone(), owner.clone()),
            )?,
            S::WasCastFromPlayersHand(p) => {
                self.cast_from(EventZone::Hand, PlayerSet::One(self.player(p)?))
            }
            S::WasCastFromTheirHand | S::WasntCastFromTheirHand | S::WasCastFromTheirLibrary => {
                let filter = self.cast_from(
                    if matches!(value, S::WasCastFromTheirLibrary) {
                        EventZone::Library
                    } else {
                        EventZone::Hand
                    },
                    PlayerSet::One(PlayerRef::CasterOf(Box::new(ObjectRef::Candidate))),
                );
                if matches!(value, S::WasntCastFromTheirHand) {
                    ObjectFilter::Not(Box::new(filter))
                } else {
                    filter
                }
            }
            S::WasCastFromAPlayersGraveyard(p) => {
                self.cast_from(EventZone::Graveyard, self.players(p)?)
            }
            S::WasCastFromExile | S::WasntCastFromExile => {
                let filter = ObjectFilter::Cast {
                    by: None,
                    from: Some(ZoneScope::Shared(EventZone::Exile)),
                };
                if matches!(value, S::WasntCastFromExile) {
                    ObjectFilter::Not(Box::new(filter))
                } else {
                    filter
                }
            }
            S::WasntCast => self.cast_history_filter(&source::Permanents::WasntCast)?,
            S::ManaWasSpentToCastIt(mana) => self.spent_colors(mana)?,
            S::ManaWasntSpentToCastIt(mana) => {
                ObjectFilter::Not(Box::new(self.spent_colors(std::slice::from_ref(mana))?))
            }
            S::NoManaWasSpentToCastIt => {
                self.cast_history_filter(&source::Permanents::NoManaWasSpentToCastIt)?
            }
            S::NoColoredManaWasSpentToCastIt => self.spent_filter(
                Some(vec![
                    ManaColor::White,
                    ManaColor::Blue,
                    ManaColor::Black,
                    ManaColor::Red,
                    ManaColor::Green,
                ]),
                false,
                SpentManaAggregation::Count,
                Comparison::Equal,
                Expr::Constant(0),
            ),
            S::AnAmountOfManaWasSpentToCastIt(c) => {
                let (comparison, amount) = self.comparison(c)?;
                self.spent_filter(None, false, SpentManaAggregation::Count, comparison, amount)
            }
            S::TargetsPermanent(p) => ObjectFilter::TargetsAny(Box::new(Recipients::Objects(
                ObjectSet::One(self.object(p)?),
            ))),
            S::TargetsAPermanent(p) => {
                ObjectFilter::TargetsAny(Box::new(Recipients::Objects(self.objects(p)?)))
            }
            S::TargetsPlayer(p) => ObjectFilter::TargetsAny(Box::new(Recipients::Players(
                PlayerSet::One(self.player(p)?),
            ))),
            S::TargetsAPlayer(p) => {
                ObjectFilter::TargetsAny(Box::new(Recipients::Players(self.players(p)?)))
            }
            S::TargetsSpell(p) => ObjectFilter::TargetsAny(Box::new(Recipients::Objects(
                ObjectSet::One(self.spell(p)?),
            ))),
            _ => return Err(unsupported(value)),
        })
    }
}
