impl Context {
    fn player_changes(&self, values: &[source::PlayerEffect]) -> Result<Vec<Change>> {
        values
            .iter()
            .enumerate()
            .map(|(i, value)| {
                self.player_rule(value)
                    .map(Change::PlayerRule)
                    .map_err(|e| e.at(format!("player_rules[{i}]")))
            })
            .collect()
    }

    fn player_rule(&self, value: &source::PlayerEffect) -> Result<PlayerRule> {
        self.reduced_player_rule(value).or_else(|error| {
            self.primitive("PlayerEffect", value)
                .map(|(name, operands)| PlayerRule::Primitive { name, operands })
                .ok_or(error)
        })
    }
    fn reduced_player_rule(&self, value: &source::PlayerEffect) -> Result<PlayerRule> {
        use source::PlayerEffect as S;
        let prohibit = PlayerRule::Prohibit;
        let casting = |spells, from| PlayerAction::CastSpell {
            filter: spells,
            from,
        };
        let land = |from, filter| PlayerAction::PlayLand { from, filter };
        let limit = |action, amount| PlayerRule::LimitPerTurn { action, amount };
        let hand = PlayerRule::HandSize;
        let origin = |zone, players| Some(ZoneScope::Players { zone, players });
        let permission = |cards, from, action, filter| PlayerRule::PlayPermission {
            cards,
            from,
            action,
            filter,
        };
        let top = || self.library_slice(PlayerRef::Affected, Expr::Constant(1), SequenceEnd::Top);
        let grave = || {
            self.private_cards(
                EventZone::Graveyard,
                PlayerSet::One(PlayerRef::Affected),
                ObjectFilter::Any,
            )
        };
        let reduction = |spells: &source::Spells,
                         symbols: &[source::CostReductionSymbol],
                         factor,
                         spill|
         -> Result<_> {
            Ok(PlayerRule::SpellCost {
                spells: self.spell_filter(spells)?,
                modifier: SpellCostModifier::Reduce {
                    reductions: self.mana_reduction(symbols, factor)?,
                    spill_into_generic: spill,
                },
            })
        };
        Ok(match value {
            S::MayPlayCardsFromTopOfLibrary => permission(
                top(),
                EventZone::Library,
                CardPlayAction::Either,
                ObjectFilter::Any,
            ),
            S::MayCastTopCardOfLibrary => permission(
                top(),
                EventZone::Library,
                CardPlayAction::CastSpell,
                ObjectFilter::Any,
            ),
            S::MayCastSpellsFromTopOfLibrary(spells) => permission(
                top(),
                EventZone::Library,
                CardPlayAction::CastSpell,
                self.spell_filter(spells)?,
            ),
            S::MayPlayLandsFromTopOfLibrary(cards) => permission(
                top(),
                EventZone::Library,
                CardPlayAction::PlayLand,
                self.card_filter(cards)?,
            ),
            S::MayCastSpellsFromGraveyard(spells) => permission(
                grave(),
                EventZone::Graveyard,
                CardPlayAction::CastSpell,
                self.spell_filter(spells)?,
            ),
            S::MayPlayLandsFromGraveyard(cards) => permission(
                self.private_cards(
                    EventZone::Graveyard,
                    PlayerSet::One(PlayerRef::Affected),
                    self.grave_filter(cards)?,
                ),
                EventZone::Graveyard,
                CardPlayAction::PlayLand,
                ObjectFilter::Any,
            ),
            S::MayPlayGraveyardCards(cards) => permission(
                self.grave_cards(cards)?,
                EventZone::Graveyard,
                CardPlayAction::Either,
                ObjectFilter::Any,
            ),
            S::MayCastGraveyardCard(card) => permission(
                ObjectSet::One(self.grave_card(card)?),
                EventZone::Graveyard,
                CardPlayAction::CastSpell,
                ObjectFilter::Any,
            ),
            S::MayPlayExiledCard(card) => permission(
                ObjectSet::One(self.exiled_card(card)?),
                EventZone::Exile,
                CardPlayAction::Either,
                ObjectFilter::Any,
            ),
            S::MayCastExiledCard(card) => permission(
                ObjectSet::One(self.exiled_card(card)?),
                EventZone::Exile,
                CardPlayAction::CastSpell,
                ObjectFilter::Any,
            ),
            S::MayPlayExiledCards(cards) => permission(
                self.exile_cards(cards)?,
                EventZone::Exile,
                CardPlayAction::Either,
                ObjectFilter::Any,
            ),
            S::MayPlayLandsFromAmongExiled(cards) => permission(
                self.exile_cards(cards)?,
                EventZone::Exile,
                CardPlayAction::PlayLand,
                ObjectFilter::Any,
            ),
            S::MayCastSpellsFromAmongExiled(spells, cards) => permission(
                self.exile_cards(cards)?,
                EventZone::Exile,
                CardPlayAction::CastSpell,
                self.spell_filter(spells)?,
            ),
            S::Protection(quality) => PlayerRule::GrantKeyword(PlayerKeyword::Protection(
                Box::new(self.protection(quality)?),
            )),
            S::HexproofFrom(quality) => PlayerRule::GrantKeyword(PlayerKeyword::HexproofFrom(
                Box::new(self.protection(quality)?),
            )),
            S::CantGainLife => prohibit(PlayerAction::GainLife),
            S::CantLoseLife => prohibit(PlayerAction::LoseLife),
            S::CantDrawCards => prohibit(PlayerAction::Draw),
            S::CantSearchLibraries => prohibit(PlayerAction::SearchLibraries),
            S::CantWinTheGame => prohibit(PlayerAction::WinGame),
            S::CantLoseTheGame => prohibit(PlayerAction::LoseGame { reason: None }),
            S::DoesntLoseTheGameForHaving0OrLessLife => prohibit(PlayerAction::LoseGame {
                reason: Some(GameLossReason::NonpositiveLife),
            }),
            S::CantBecomeTheMonarch => prohibit(PlayerAction::BecomeMonarch),
            S::CantGetCounters => prohibit(PlayerAction::ReceiveCounters(None)),
            S::CantGetCountersOfType(kind) => prohibit(PlayerAction::ReceiveCounters(Some(
                crate::counter::lower(kind),
            ))),
            S::CantCycleCards => prohibit(PlayerAction::Activate(ActivationFilter::Cycling)),
            S::CantActivateAbilities(filter) => {
                prohibit(PlayerAction::Activate(self.activation_filter(filter)?))
            }
            S::CantActivateNonManaAbilities => prohibit(PlayerAction::Activate(
                ActivationFilter::Not(Box::new(ActivationFilter::Mana)),
            )),
            S::CantPlayLands => prohibit(land(None, ObjectFilter::Any)),
            S::CantPlayLandsFromGraveyards => prohibit(land(
                origin(EventZone::Graveyard, PlayerSet::All),
                ObjectFilter::Any,
            )),
            S::CantPlayLandsFromTheirHand => prohibit(land(
                origin(EventZone::Hand, PlayerSet::One(PlayerRef::Affected)),
                ObjectFilter::Any,
            )),
            S::CantPlayLandsOfType(filter) => prohibit(land(None, self.filter(filter)?)),
            S::CantCastSpells(spells)
            | S::CantCastSpellsFromExile(spells)
            | S::CantCastSpellsFromGraveyards(spells)
            | S::CantCastSpellsFromLibraries(spells)
            | S::CantCastSpellsFromTheirHand(spells) => prohibit(casting(
                self.spell_filter(spells)?,
                match value {
                    S::CantCastSpellsFromExile(_) => Some(ZoneScope::Shared(EventZone::Exile)),
                    S::CantCastSpellsFromGraveyards(_) => {
                        origin(EventZone::Graveyard, PlayerSet::All)
                    }
                    S::CantCastSpellsFromLibraries(_) => origin(EventZone::Library, PlayerSet::All),
                    S::CantCastSpellsFromTheirHand(_) => {
                        origin(EventZone::Hand, PlayerSet::One(PlayerRef::Affected))
                    }
                    _ => None,
                },
            )),
            S::CantDrawMoreThanOneCardEachTurn => limit(PlayerAction::Draw, Expr::Constant(1)),
            S::CantCastMoreThanNumberSpellsEachTurn(n, spells) => {
                limit(casting(self.spell_filter(spells)?, None), self.number(n)?)
            }
            S::HasNoMaximumHandSize => hand(HandSizeRule::Unlimited),
            S::SetMaximumHandSize(n) => hand(HandSizeRule::Set(self.number(n)?)),
            S::IncreaseMaximumHandSize(n) => hand(HandSizeRule::Adjust(self.number(n)?)),
            S::ReduceMaximumHandSize(n) => hand(HandSizeRule::Adjust(Expr::multiply(
                Expr::Constant(-1),
                self.number(n)?,
            ))),
            S::MayPlayAnAdditionalLand => {
                PlayerRule::AdditionalLandPlays(Count::Exactly(Expr::Constant(1)))
            }
            S::MayPlayAdditionalLands(n) => {
                PlayerRule::AdditionalLandPlays(Count::Exactly(self.number(n)?))
            }
            S::MayPlayAnyNumberOfLandsDuringThierTurn => {
                PlayerRule::AdditionalLandPlays(Count::AnyNumber)
            }
            S::DecreaseSpellCost(spells, symbols) => {
                reduction(spells, symbols, Expr::Constant(1), true)?
            }
            S::DecreaseSpellCostForEach(spells, symbols, n) => {
                reduction(spells, symbols, self.number(n)?, true)?
            }
            S::DecreaseSpellCostOnlyColored(spells, symbols) => {
                reduction(spells, symbols, Expr::Constant(1), false)?
            }
            S::DecreaseSpellCostX(spells, symbols, n) => PlayerRule::SpellCost {
                spells: self.spell_filter(spells)?,
                modifier: SpellCostModifier::Reduce {
                    reductions: symbols
                        .iter()
                        .map(|symbol| match symbol {
                            source::CostReductionSymbolX::CostReduceX => Ok(ManaReduction {
                                color: None,
                                amount: self.number(n)?,
                            }),
                        })
                        .collect::<Result<_>>()?,
                    spill_into_generic: true,
                },
            },
            S::IncreaseSpellCost(spells, cost) => PlayerRule::SpellCost {
                spells: self.spell_filter(spells)?,
                modifier: SpellCostModifier::Increase(self.cost_for(cost, &PlayerRef::Affected)?),
            },
            S::SetMinimumSpellCost(spells, n) => PlayerRule::SpellCost {
                spells: self.spell_filter(spells)?,
                modifier: SpellCostModifier::Minimum(self.number(n)?),
            },
            S::MayCastSpellsAsThoughTheyHadFlash(spells) => PlayerRule::SpellTiming {
                spells: self.spell_filter(spells)?,
                timing: SpellTiming::Flash,
            },
            S::CanCastSpellsOnlyAnyTimeTheyCouldCastASorcery => PlayerRule::SpellTiming {
                spells: ObjectFilter::Any,
                timing: SpellTiming::SorceryOnly,
            },
            S::CanCastSpellsOnlyDuringTheirTurn => PlayerRule::SpellTiming {
                spells: ObjectFilter::Any,
                timing: SpellTiming::OwnTurnOnly,
            },
            S::SkipsDrawStep => PlayerRule::Skip(TurnSection::Draw),
            S::SkipsUntapStep => PlayerRule::Skip(TurnSection::Untap),
            S::SkipsUpkeepStep => PlayerRule::Skip(TurnSection::Upkeep),
            S::SkipsMainPhase => PlayerRule::Skip(TurnSection::Main),
            S::SkipsCombatPhase => PlayerRule::Skip(TurnSection::Combat),
            S::LifeTotalCantChange => PlayerRule::LifeTotalLocked,
            S::DamageDoesntCauseLifeLoss => PlayerRule::DamageDoesNotCauseLifeLoss,
            S::CantUntapMoreThanNumberPermanentsDuringTheirUntapStep(n, p) => {
                PlayerRule::UntapLimit {
                    permanents: self.filter(p)?,
                    amount: self.number(n)?,
                }
            }
            S::Hexproof => PlayerRule::GrantKeyword(PlayerKeyword::Hexproof),
            S::Shroud => PlayerRule::GrantKeyword(PlayerKeyword::Shroud),
            S::DoesntLoseManaAsStepsAndPhasesEnd => PlayerRule::RetainMana { colors: None },
            S::DoesntLoseColoredManaAsStepsAndPhasesEnd(mana) => PlayerRule::RetainMana {
                colors: match mana {
                    source::UnspentMana::AnyUnspentMana => None,
                    source::UnspentMana::UnspentGreenMana => Some(vec![ManaColor::Green]),
                    source::UnspentMana::UnspentRedMana => Some(vec![ManaColor::Red]),
                },
            },
            S::MayLookAtTopCardOfLibraryAnyTime
            | S::PlaysWithTopOfLibraryRevealed
            | S::PlaysWithHandRevealed => {
                let hand = matches!(value, S::PlaysWithHandRevealed);
                let objects = self.private_cards(
                    if hand {
                        EventZone::Hand
                    } else {
                        EventZone::Library
                    },
                    PlayerSet::One(PlayerRef::Affected),
                    ObjectFilter::Any,
                );
                PlayerRule::Visibility {
                    objects: if hand {
                        objects
                    } else {
                        ObjectSet::Take {
                            objects: Box::new(objects),
                            count: Expr::Constant(1),
                            from: SequenceEnd::Top,
                        }
                    },
                    audience: if matches!(value, S::MayLookAtTopCardOfLibraryAnyTime) {
                        VisibilityAudience::AffectedPlayer
                    } else {
                        VisibilityAudience::Everyone
                    },
                }
            }
            _ => return Err(unsupported(value)),
        })
    }
}
