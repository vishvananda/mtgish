impl Context {
    fn graveyard_changes(&self, values: &[source::GraveyardCardEffect]) -> Result<Vec<Change>> {
        let mut changes = vec![];
        for (i, value) in values.iter().enumerate() {
            match value {
                source::GraveyardCardEffect::AddAbility(rules) => {
                    for (j, rule) in rules.iter().enumerate() {
                        // An ability definition starts a fresh reference frame.
                        // Source is the receiving card, not the granting spell.
                        changes.push(Change::GrantAbility(Box::new(
                            self.fresh()
                                .ability(rule)
                                .map_err(|e| e.at(format!("effects[{i}].abilities[{j}]")))?,
                        )));
                    }
                }
                _ => return Err(unsupported(value).at(format!("effects[{i}]"))),
            }
        }
        Ok(changes)
    }

    fn flashback(&self, cost: Cost) -> Ability {
        Ability::Casting(CastingRule::Permission {
            origin: CastingOrigin::Flashback,
            player: PlayerRef::OwnerOf(Box::new(ObjectRef::Source)),
            from: ZoneScope::Players {
                zone: EventZone::Graveyard,
                players: PlayerSet::One(PlayerRef::OwnerOf(Box::new(ObjectRef::Source))),
            },
            require: ObjectFilter::Either(vec![
                ObjectFilter::CardType(source::CardType::Instant),
                ObjectFilter::CardType(source::CardType::Sorcery),
            ]),
            cost,
            leave_stack: Some(Destination::Exile),
        })
    }

    fn casting_rule(&self, value: &source::CastEffect) -> Result<CastingRule> {
        use source::CastEffect as S;
        let conditional = |condition, rule| CastingRule::If {
            condition,
            rule: Box::new(rule),
        };
        let targets = |kind, filter| Condition::TargetsMatch {
            object: ObjectRef::Source,
            kind,
            filter,
        };
        Ok(match value {
            S::AdditionalCastingCost(cost)
            | S::AdditionalCastingCostX(cost)
            | S::AdditionalCastingCostForAllTargets(cost)
            | S::OptionalAdditionalCastingCost(cost)
            | S::AdditionalCastingCostForEachTarget(cost)
            | S::AdditionalCastingCostForEachTargetBeyondTheFirst(cost) => {
                CastingRule::AdditionalCost {
                    cost: self.cost(cost)?,
                    optional: matches!(value, S::OptionalAdditionalCastingCost(_)),
                    repetition: match value {
                        S::AdditionalCastingCostForEachTarget(_) => {
                            CostRepetition::TargetsBeyond(0)
                        }
                        S::AdditionalCastingCostForEachTargetBeyondTheFirst(_) => {
                            CostRepetition::TargetsBeyond(1)
                        }
                        _ => CostRepetition::Once,
                    },
                }
            }
            S::AdditionalCastingCostIf(cost, condition) => conditional(
                self.condition(condition)?,
                CastingRule::AdditionalCost {
                    cost: self.cost(cost)?,
                    optional: false,
                    repetition: CostRepetition::Once,
                },
            ),
            S::AdditionalCastingCostIfItTargetsAPermanent(cost, filter) => conditional(
                targets(ObjectKind::Permanent, self.filter(filter)?),
                CastingRule::AdditionalCost {
                    cost: self.cost(cost)?,
                    optional: false,
                    repetition: CostRepetition::Once,
                },
            ),
            S::AlternateCastingCost(cost) => CastingRule::AlternativeCost(self.cost(cost)?),
            S::AlternateCastingCostIf(cost, condition) => conditional(
                self.condition(condition)?,
                CastingRule::AlternativeCost(self.cost(cost)?),
            ),
            S::MayCastWithoutPayingIf(condition) => conditional(
                self.condition(condition)?,
                CastingRule::AlternativeCost(Cost { parts: vec![] }),
            ),
            S::MayCastWithoutPayingAndAsThoughItHadFlashIf(condition) => conditional(
                self.condition(condition)?,
                CastingRule::AlternativeCostWithFlash(Cost { parts: vec![] }),
            ),
            S::ReduceCastingCost(reduction) => {
                CastingRule::ReduceCost(self.mana_reduction(reduction, Expr::Constant(1))?)
            }
            S::ReduceCastingCostForEach(reduction, n) => {
                CastingRule::ReduceCost(self.mana_reduction(reduction, self.number(n)?)?)
            }
            S::ReduceCastingCostX(reduction, n) => {
                let amount = self.number(n)?;
                CastingRule::ReduceCost(
                    reduction
                        .iter()
                        .map(|symbol| match symbol {
                            source::CostReductionSymbolX::CostReduceX => ManaReduction {
                                color: None,
                                amount: amount.clone(),
                            },
                        })
                        .collect(),
                )
            }
            S::ReduceCastingCostIf(reduction, condition) => conditional(
                self.condition(condition)?,
                CastingRule::ReduceCost(self.mana_reduction(reduction, Expr::Constant(1))?),
            ),
            S::ReduceCastingCostIfItTargetsAPermanent(reduction, filter) => conditional(
                targets(ObjectKind::Permanent, self.filter(filter)?),
                CastingRule::ReduceCost(self.mana_reduction(reduction, Expr::Constant(1))?),
            ),
            S::ReduceCastingCostIfItTargetsASpell(reduction, filter) => conditional(
                targets(ObjectKind::Spell, self.spell_filter(filter)?),
                CastingRule::ReduceCost(self.mana_reduction(reduction, Expr::Constant(1))?),
            ),
            S::ReduceCastingCostIfItTargetsACard(reduction, filter) => conditional(
                targets(ObjectKind::Card, self.card_filter(filter)?),
                CastingRule::ReduceCost(self.mana_reduction(reduction, Expr::Constant(1))?),
            ),
            S::CantBeCastIf(condition) | S::CantBeCastUnless(condition) => {
                let condition = self.condition(condition)?;
                CastingRule::Require(if matches!(value, S::CantBeCastIf(_)) {
                    Condition::Not(Box::new(condition))
                } else {
                    condition
                })
            }
            S::CantBeCastFromAnywhereOtherThanGraveyard => {
                CastingRule::Require(Condition::WasCastFrom {
                    object: ObjectRef::Source,
                    zone: EventZone::Graveyard,
                })
            }
            S::XCantBeZero => CastingRule::Require(Condition::Compare {
                left: Expr::AnnouncedX,
                comparison: Comparison::NotEqual,
                right: Expr::Constant(0),
            }),
            S::XIs(comparison) => {
                let (comparison, right) = self.comparison(comparison)?;
                CastingRule::Require(Condition::Compare {
                    left: Expr::AnnouncedX,
                    comparison,
                    right,
                })
            }
            S::MayCastAsThoughItHadFlashForAdditionalCost(cost) => CastingRule::Flash {
                additional_cost: Some(self.cost(cost)?),
            },
            S::MayCastAsThoughItHadFlashIf(condition) => conditional(
                self.condition(condition)?,
                CastingRule::Flash {
                    additional_cost: None,
                },
            ),
            S::MayCastAsThoughItHadFlashIfItTargetsAPermanent(filter) => conditional(
                targets(ObjectKind::Permanent, self.filter(filter)?),
                CastingRule::Flash {
                    additional_cost: None,
                },
            ),
            S::MayCastAsThoughItHadFlashIfXIs(comparison) => {
                let (comparison, right) = self.comparison(comparison)?;
                conditional(
                    Condition::Compare {
                        left: Expr::AnnouncedX,
                        comparison,
                        right,
                    },
                    CastingRule::Flash {
                        additional_cost: None,
                    },
                )
            }
            S::CantChooseATarget(filter) => CastingRule::ProhibitTargets {
                domain: ObjectDomain::permanent(),
                filter: self.filter(filter)?,
            },
            S::CantSpendManaToCast => CastingRule::RestrictMana {
                portion: ManaCostPortion::All,
                filter: ManaPaymentFilter::None,
            },
            S::SpendOnlyColorManaOnX(color) => CastingRule::RestrictMana {
                portion: ManaCostPortion::X,
                filter: ManaPaymentFilter::Colors(vec![casting_color(color)?]),
            },
            S::SpendOnlyColorsOfManaOnX(colors) => CastingRule::RestrictMana {
                portion: ManaCostPortion::X,
                filter: ManaPaymentFilter::Colors(
                    colors.iter().map(casting_color).collect::<Result<_>>()?,
                ),
            },
            S::SpendOnlyManaFromPermanentsToCast(filter) => CastingRule::RestrictMana {
                portion: ManaCostPortion::All,
                filter: ManaPaymentFilter::ProducedBy(self.filter(filter)?),
            },
            S::MaySpendManaAsThoughAnyTypeToCast => CastingRule::SpendAsThoughAnyType,
            _ => return Err(unsupported(value)),
        })
    }
    fn mana_reduction(
        &self,
        value: &[source::CostReductionSymbol],
        factor: Expr,
    ) -> Result<Vec<ManaReduction>> {
        use source::CostReductionSymbol as S;
        value
            .iter()
            .map(|symbol| {
                let (color, n) = match symbol {
                    S::CostReduceGeneric(n) => (None, *n),
                    S::CostReduceW => (Some(ManaColor::White), 1),
                    S::CostReduceU => (Some(ManaColor::Blue), 1),
                    S::CostReduceB => (Some(ManaColor::Black), 1),
                    S::CostReduceR => (Some(ManaColor::Red), 1),
                    S::CostReduceG => (Some(ManaColor::Green), 1),
                };
                if n < 0 {
                    return Err(invalid("negative mana reduction"));
                }
                Ok(ManaReduction {
                    color,
                    amount: Expr::multiply(Expr::Constant(n.into()), factor.clone()),
                })
            })
            .collect()
    }
    fn exile_cost_selection(&self, value: &source::ExilableCost) -> Result<CostObjectSelection> {
        use source::ExilableCost as S;
        let payer = self
            .payer
            .clone()
            .unwrap_or(self.player(&source::Player::You)?);
        let hand =
            |filter| self.private_cards(EventZone::Hand, PlayerSet::One(payer.clone()), filter);
        let (objects, choose) = match value {
            S::Permanent(p) => (ObjectSet::One(self.object(p)?), None),
            S::EachPermanent(p) => (self.objects(p)?, None),
            S::APermanent(p) => (self.objects(p)?, Some(Count::Exactly(Expr::Constant(1)))),
            S::NumberPermanents(n, p) => (self.objects(p)?, Some(Count::Exactly(self.number(n)?))),
            S::AnyNumberOfPermanents(p) => (self.objects(p)?, Some(Count::AnyNumber)),
            S::Spell(s) => (ObjectSet::One(self.spell(s)?), None),
            S::ASpell(s) => (
                ObjectSet::Zone {
                    scope: ZoneScope::Shared(EventZone::Stack),
                    kind: ObjectKind::Spell,
                    filter: self.spell_filter(s)?,
                },
                Some(Count::Exactly(Expr::Constant(1))),
            ),
            S::CardInGraveyards(c) => (ObjectSet::One(self.grave_card(c)?), None),
            S::EachCardInGraveyards(c) => (self.grave_cards(c)?, None),
            S::ACardInGraveyards(c) => (
                self.grave_cards(c)?,
                Some(Count::Exactly(Expr::Constant(1))),
            ),
            S::NumberCardsInGraveyards(n, c) => {
                (self.grave_cards(c)?, Some(Count::Exactly(self.number(n)?)))
            }
            S::NumberOrMoreCardsInGraveyards(n, c) => {
                (self.grave_cards(c)?, Some(Count::AtLeast(self.number(n)?)))
            }
            S::AnyNumberOfCardsInGraveyards(c) => (self.grave_cards(c)?, Some(Count::AnyNumber)),
            S::CardInHand(c) => (ObjectSet::One(self.hand_card(c)?), None),
            S::ACardFromHand => (
                hand(ObjectFilter::Any),
                Some(Count::Exactly(Expr::Constant(1))),
            ),
            S::ACardOfTypeFromHand(c) => (
                hand(self.hand_filter(c)?),
                Some(Count::Exactly(Expr::Constant(1))),
            ),
            S::NumberCardsOfTypeFromHand(n, c) => (
                hand(self.hand_filter(c)?),
                Some(Count::Exactly(self.number(n)?)),
            ),
            S::ACardOfTypeFromPlayersHand(c, p) => (
                self.private_cards(
                    EventZone::Hand,
                    PlayerSet::One(self.player(p)?),
                    self.hand_filter(c)?,
                ),
                Some(Count::Exactly(Expr::Constant(1))),
            ),
            S::PlayersHand(p) => (
                self.private_cards(
                    EventZone::Hand,
                    PlayerSet::One(self.player(p)?),
                    ObjectFilter::Any,
                ),
                None,
            ),
            // Library costs must actually have enough cards, unlike an effect
            // that simply takes as many top cards as the library contains.
            S::TheTopCardOfLibrary => (
                self.library_slice(payer, Expr::Constant(1), SequenceEnd::Top),
                Some(Count::Exactly(Expr::Constant(1))),
            ),
            S::TheTopNumberCardsOfLibrary(n) => (
                self.library_slice(payer, self.number(n)?, SequenceEnd::Top),
                Some(Count::Exactly(self.number(n)?)),
            ),
            _ => return Err(unsupported(value)),
        };
        Ok(CostObjectSelection { objects, choose })
    }
}
fn casting_color(color: &source::Color) -> Result<ManaColor> {
    Ok(match color {
        source::Color::White => ManaColor::White,
        source::Color::Blue => ManaColor::Blue,
        source::Color::Black => ManaColor::Black,
        source::Color::Red => ManaColor::Red,
        source::Color::Green => ManaColor::Green,
        source::Color::Colorless => ManaColor::Colorless,
        _ => return Err(unsupported(color)),
    })
}
