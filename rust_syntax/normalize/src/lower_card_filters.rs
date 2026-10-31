// Generic predicates retain their source domains. Queries and result aliases
// remain explicit in the zone adapter rather than becoming predicate literals.
impl Context {
    fn card_filter(&self, value: &source::Cards) -> Result<ObjectFilter> {
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
            source::CardsInExile::WasExiledWithPermanent(p) => {
                Ok(ObjectFilter::ExiledWith(self.object(p)?))
            }
            _ => rescue(unsupported(value), || {
                self.primitive_predicate("CardsInExile", value)
            })
            .or_else(|error| self.as_card_filter(value, error)),
        }
    }
}
