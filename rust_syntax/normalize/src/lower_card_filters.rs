// Generic predicates retain their source domains. Queries and result aliases
// remain explicit in the zone adapter rather than becoming predicate literals.
impl Context {
    fn card_filter(&self, value: &source::Cards) -> Result<ObjectFilter> {
        self.pipeline_selector(crate::pipeline::selectors::SelectorRef::Cards(value))
            .unwrap_or_else(|| Err(unsupported(value)))
            .or_else(|error| self.primitive_predicate("Cards", value).ok_or(error))
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
            _ => Err(unsupported(value)),
        }
    }
    fn hand_filter(&self, value: &source::CardsInHand) -> Result<ObjectFilter> {
        self.pipeline_selector(crate::pipeline::selectors::SelectorRef::CardsInHand(value))
            .unwrap_or_else(|| Err(unsupported(value)))
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
            _ => Err(unsupported(value)),
        }
    }
}
