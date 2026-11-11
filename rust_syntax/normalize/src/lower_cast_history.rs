// Shared cast/entry provenance and actual-spent-mana queries. These read
// recorded facts, not current controller, mana cost symbols, or live hand/GY.
impl Context {
    fn cast_by(&self, players: PlayerSet) -> ObjectFilter {
        ObjectFilter::Cast {
            by: Some(players),
            from: None,
        }
    }
    fn cast_from(&self, zone: EventZone, players: PlayerSet) -> ObjectFilter {
        ObjectFilter::Cast {
            by: None,
            from: Some(ZoneScope::Players { zone, players }),
        }
    }
    fn spent_mana(
        &self,
        object: ObjectRef,
        colors: Option<Vec<ManaColor>>,
        snow: bool,
        aggregation: SpentManaAggregation,
    ) -> Expr {
        Expr::ManaSpent {
            object,
            filter: SpentManaFilter { colors, snow },
            aggregation,
        }
    }
    fn spent_filter(
        &self,
        colors: Option<Vec<ManaColor>>,
        snow: bool,
        aggregation: SpentManaAggregation,
        comparison: Comparison,
        amount: Expr,
    ) -> ObjectFilter {
        ObjectFilter::Condition(Box::new(Condition::Compare { left: self.spent_mana(ObjectRef::Candidate, colors, snow, aggregation), test: NumberTest::Compare { comparison: comparison, value: amount } }))
    }
    fn spent_colors(&self, mana: &[source::ManaProduce]) -> Result<ObjectFilter> {
        if mana.is_empty() {
            return Err(invalid("spent mana requirement cannot be empty"));
        }
        // Count repeated requirements of a type, rather than satisfying two
        // identical symbols with one spent unit. Different types are disjoint.
        let mut colors: Vec<(ManaColor, i64)> = vec![];
        for item in mana {
            use source::ManaProduce as S;
            let color = match item {
                S::ManaProduceW => ManaColor::White,
                S::ManaProduceU => ManaColor::Blue,
                S::ManaProduceB => ManaColor::Black,
                S::ManaProduceR => ManaColor::Red,
                S::ManaProduceG => ManaColor::Green,
                S::ManaProduceC => ManaColor::Colorless,
                _ => return Err(unsupported(item)),
            };
            if let Some((_, n)) = colors.iter_mut().find(|(c, _)| *c == color) {
                *n += 1;
            } else {
                colors.push((color, 1));
            }
        }
        Ok(ObjectFilter::AllOf(
            colors
                .into_iter()
                .map(|(color, n)| {
                    self.spent_filter(
                        Some(vec![color]),
                        false,
                        SpentManaAggregation::Count,
                        Comparison::GreaterOrEqual,
                        Expr::Constant(n),
                    )
                })
                .collect(),
        ))
    }
    fn cast_history_filter(&self, value: &source::Permanents) -> Result<ObjectFilter> {
        use source::Permanents as S;
        Ok(match value {
            S::ItWasCast => ObjectFilter::Cast {
                by: None,
                from: None,
            },
            S::WasntCast => ObjectFilter::Not(Box::new(ObjectFilter::Cast {
                by: None,
                from: None,
            })),
            S::CastByPlayer(p) => self.cast_by(PlayerSet::One(self.player(p)?)),
            S::CastByPlayerFromHand(p, owner) | S::CastByPlayerFromPlayersGraveyard(p, owner) => {
                ObjectFilter::Cast {
                    by: Some(PlayerSet::One(self.player(p)?)),
                    from: Some(ZoneScope::Players {
                        zone: if matches!(value, S::CastByPlayerFromHand(..)) {
                            EventZone::Hand
                        } else {
                            EventZone::Graveyard
                        },
                        players: PlayerSet::One(self.player(owner)?),
                    }),
                }
            }
            S::CastByPlayerFromAnyPlayersGraveyard(p, owners) => ObjectFilter::Cast {
                by: Some(PlayerSet::One(self.player(p)?)),
                from: Some(ZoneScope::Players {
                    zone: EventZone::Graveyard,
                    players: self.players(owners)?,
                }),
            },
            S::CastFromPlayersLibrary(p) => {
                self.cast_from(EventZone::Library, PlayerSet::One(self.player(p)?))
            }
            S::WasCastFromAPlayersGraveyard(players) => ObjectFilter::Cast {
                by: None,
                from: Some(ZoneScope::Players {
                    zone: EventZone::Graveyard,
                    players: self.players(players)?,
                }),
            },
            S::WasCastByAPlayer(players) => ObjectFilter::Cast {
                by: Some(self.players(players)?),
                from: None,
            },
            S::WasCastFromTheirHand => self.cast_from(
                EventZone::Hand,
                PlayerSet::One(PlayerRef::CasterOf(Box::new(ObjectRef::Candidate))),
            ),
            S::EnteredFromPlayersGraveyard(p) | S::EnteredFromPlayersLibrary(p) => {
                ObjectFilter::EnteredFrom(ZoneScope::Players {
                    zone: if matches!(value, S::EnteredFromPlayersGraveyard(_)) {
                        EventZone::Graveyard
                    } else {
                        EventZone::Library
                    },
                    players: PlayerSet::One(self.player(p)?),
                })
            }
            S::EnteredFromAPlayersGraveyard(p) => ObjectFilter::EnteredFrom(ZoneScope::Players {
                zone: EventZone::Graveyard,
                players: self.players(p)?,
            }),
            S::ManaWasSpentToCastIt(mana) => self.spent_colors(mana)?,
            S::NoManaWasSpentToCastIt => self.spent_filter(
                None,
                false,
                SpentManaAggregation::Count,
                Comparison::Equal,
                Expr::Constant(0),
            ),
            S::SnowManaWasSpentToCastIt => self.spent_filter(
                None,
                true,
                SpentManaAggregation::Count,
                Comparison::Greater,
                Expr::Constant(0),
            ),
            S::NumberOfColorsOfManaSpentToCastItIs(c)
            | S::ManaAmountOfSameColorWasSpentToCastIt(c) => {
                let (comparison, amount) = self.comparison(c)?;
                self.spent_filter(
                    None,
                    false,
                    if matches!(value, S::NumberOfColorsOfManaSpentToCastItIs(_)) {
                        SpentManaAggregation::DistinctColors
                    } else {
                        SpentManaAggregation::MaximumPerColor
                    },
                    comparison,
                    amount,
                )
            }
            S::OnTheBattlefield => ObjectFilter::InZone(ZoneScope::Shared(EventZone::Battlefield)),
            S::NumOtherPermanentsAreOnTheBattlefield(c, objects) => {
                let test = self.number_test(c)?;
                ObjectFilter::Condition(Box::new(Condition::Compare { left: Expr::CountObjects(Box::new(ObjectSet::Difference {
                        objects: Box::new(ObjectSet::battlefield(self.filter(objects)?)),
                        excluding: Box::new(ObjectSet::One(ObjectRef::Candidate)),
                    })), test }))
            }
            _ => return Err(unsupported(value)),
        })
    }
}
