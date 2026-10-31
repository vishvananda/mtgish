// Generic predicates retain their source domains. Queries and result aliases
// remain explicit in the zone adapter rather than becoming predicate literals.
impl Context {
    /// Shares a card type with anything an action of this scope touched.
    fn shares_type_this_way(&self, value: &source::Cards) -> Option<Result<ObjectFilter>> {
        use source::Cards as S;
        let shares = |set: Result<ObjectSet>| {
            set.map(|with| ObjectFilter::SharesWithAny {
                property: DistinctProperty::CardTypes,
                with: Box::new(with),
            })
        };
        let discarded = || self.this_way_set(Done::Discarded, ObjectPhase::After, None);
        Some(match value {
            S::SharesACardtypeWithCardsDiscardedThisWay => shares(discarded()),
            S::DoesntShareACardtypeWithTheCardsDiscardedThisWay => {
                shares(discarded()).map(|filter| ObjectFilter::Not(Box::new(filter)))
            }
            S::SharesACardtypeWithCardDiscardedByPlayerThisWay(player) => shares(
                self.player(player)
                    .and_then(|p| self.moved_by(Done::Discarded, ObjectPhase::After, &p, None)),
            ),
            S::SharesACardtypeWithThePermanentDestroyedThisWay => {
                shares(self.this_way_set(Done::Destroyed, ObjectPhase::Before, None))
            }
            _ => return None,
        })
    }
    fn card_filter(&self, value: &source::Cards) -> Result<ObjectFilter> {
        if matches!(value, source::Cards::TheChosenLibraryFilter) {
            return Ok(ObjectFilter::ChosenFilter);
        }
        if let Some(result) = self.shares_type_this_way(value) {
            return result;
        }
        self.pipeline_selector(crate::pipeline::selectors::SelectorRef::Cards(value))
            .unwrap_or_else(|| Err(unsupported(value)))
            .or_else(|error| rescue(error, || self.primitive_predicate("Cards", value)))
            .or_else(|error| self.as_permanent_filter(value, error))
    }
    /// The same predicate under `Permanents` reads the same characteristics.
    fn as_permanent_filter(
        &self,
        value: &impl Serialize,
        error: Diagnostic,
    ) -> Result<ObjectFilter> {
        rescue(error, || {
            let filter: source::Permanents = as_family(value, "Permanents")?;
            self.filter(&filter).ok()
        })
    }
    /// The same predicate under `Cards` reads the same characteristics.
    fn as_card_filter(&self, value: &impl Serialize, error: Diagnostic) -> Result<ObjectFilter> {
        rescue(error, || {
            let filter: source::Cards = as_family(value, "Cards")?;
            self.card_filter(&filter).ok()
        })
    }
    fn grave_filter(&self, value: &source::CardsInGraveyards) -> Result<ObjectFilter> {
        if let Some(result) = self.pipeline_selector(
            crate::pipeline::selectors::SelectorRef::CardsInGraveyards(value),
        ) {
            return result;
        }
        match value {
            source::CardsInGraveyards::Ref_TargetGraveyardCards
            | source::CardsInGraveyards::Ref_TargetGraveyardCards1
            | source::CardsInGraveyards::Ref_TargetGraveyardCards2 => {
                Ok(ObjectFilter::InSet(Box::new(self.grave_cards(value)?)))
            }
            source::CardsInGraveyards::InTheGraveyardPileChosenThisWay
            | source::CardsInGraveyards::InTheGraveyardPileNotChosenThisWay => {
                Ok(ObjectFilter::InSet(Box::new(self.grave_cards(value)?)))
            }
            source::CardsInGraveyards::WasAttachedToDeadPermanent => {
                Ok(ObjectFilter::WasAttachedTo(self.dead_object()?))
            }
            source::CardsInGraveyards::WasPutIntoGraveyardFromAnywhereThisTurn => {
                Ok(ObjectFilter::ArrivedThisTurn { from: None })
            }
            source::CardsInGraveyards::WasPutIntoGraveyardFromTheBattlefieldThisTurn => {
                Ok(ObjectFilter::ArrivedThisTurn {
                    from: Some(EventZone::Battlefield),
                })
            }
            source::CardsInGraveyards::WasPutIntoGraveyardFromLibraryThisTurn => {
                Ok(ObjectFilter::ArrivedThisTurn {
                    from: Some(EventZone::Library),
                })
            }
            source::CardsInGraveyards::WasPutIntoGraveyardFromAnywhereOtherThanTheBattlefieldThisTurn => {
                Ok(ObjectFilter::All(vec![
                    ObjectFilter::ArrivedThisTurn { from: None },
                    ObjectFilter::Not(Box::new(ObjectFilter::ArrivedThisTurn {
                        from: Some(EventZone::Battlefield),
                    })),
                ]))
            }
            source::CardsInGraveyards::WasPutIntoGraveyardThisWay => Ok(ObjectFilter::InSet(
                Box::new(self.this_way_set(Done::Graveyard, ObjectPhase::After, None)?),
            )),
            _ => rescue(unsupported(value), || {
                self.primitive_predicate("CardsInGraveyards", value)
            })
            .or_else(|error| self.as_card_filter(value, error)),
        }
    }
    fn hand_filter(&self, value: &source::CardsInHand) -> Result<ObjectFilter> {
        if let source::CardsInHand::TheCardsSeekedThisWay = value {
            return Ok(ObjectFilter::InSet(Box::new(self.seeked_cards()?)));
        }
        if let source::CardsInHand::TheCardsConjuredIntoHandThisWay
        | source::CardsInHand::TheCardsConjuredThisWay
        | source::CardsInHand::TheCardsDraftedThisWay = value
        {
            return Ok(ObjectFilter::InSet(Box::new(self.conjured_cards()?)));
        }
        if let (source::CardsInHand::TheChosenCardsInHand, Some(result)) =
            (value, &self.hand_choice)
        {
            return Ok(ObjectFilter::InSet(Box::new(ObjectSet::Selected(
                result.clone(),
            ))));
        }
        if let (source::CardsInHand::TheCardsInHandNotChosenThisWay, Some(result)) =
            (value, &self.hand_choice)
        {
            return Ok(ObjectFilter::InSet(Box::new(ObjectSet::Unselected(
                result.clone(),
            ))));
        }
        self.pipeline_selector(crate::pipeline::selectors::SelectorRef::CardsInHand(value))
            .unwrap_or_else(|| Err(unsupported(value)))
            .or_else(|error| rescue(error, || self.primitive_predicate("CardsInHand", value)))
            .or_else(|error| self.as_card_filter(value, error))
    }
    fn exile_filter(&self, value: &source::CardsInExile) -> Result<ObjectFilter> {
        if let Some(result) =
            self.pipeline_selector(crate::pipeline::selectors::SelectorRef::CardsInExile(value))
        {
            return result;
        }
        match value {
            source::CardsInExile::TheCardsExiledThisWay | source::CardsInExile::TheExiledCards => {
                Ok(ObjectFilter::InSet(Box::new(self.exile_cards(value)?)))
            }
            source::CardsInExile::InTheExiledPileChosenThisWay
            | source::CardsInExile::InTheChosenPile
            | source::CardsInExile::InTheExiledPileNotChosenThisWay
                if self.chosen_pile.is_some() =>
            {
                Ok(ObjectFilter::InSet(Box::new(self.exile_cards(value)?)))
            }
            source::CardsInExile::TheCardsConjuredThisWay => {
                Ok(ObjectFilter::InSet(Box::new(self.conjured_cards()?)))
            }
            source::CardsInExile::WasExiledWithDeadPermanent => {
                Ok(ObjectFilter::ExiledWith(self.dead_object()?))
            }
            source::CardsInExile::WasExiledThisWay => Ok(ObjectFilter::InSet(Box::new(
                self.this_way_set(Done::Exiled, ObjectPhase::After, None)?,
            ))),
            source::CardsInExile::WasExiledByPlayerThisWay(player) => {
                let player = self.player(player)?;
                Ok(ObjectFilter::InSet(Box::new(self.moved_by(
                    Done::Exiled,
                    ObjectPhase::After,
                    &player,
                    None,
                )?)))
            }
            source::CardsInExile::WasExiledWithPermanent(p)
            | source::CardsInExile::UsedToCraftPermanent(p) => {
                Ok(ObjectFilter::ExiledWith(self.object(p)?))
            }
            _ => rescue(unsupported(value), || {
                self.primitive_predicate("CardsInExile", value)
            })
            .or_else(|error| self.as_card_filter(value, error)),
        }
    }
}
