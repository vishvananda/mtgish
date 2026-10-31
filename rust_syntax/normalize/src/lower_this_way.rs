// "This way": what the last action of a kind did, read back as a set.
impl Context {
    fn this_way_set(
        &self,
        done: Done,
        phase: ObjectPhase,
        filter: Option<ObjectFilter>,
    ) -> Result<ObjectSet> {
        let objects = ObjectSet::Transition {
            result: self.done(done)?,
            phase,
        };
        Ok(match filter {
            Some(filter) => ObjectSet::Matching {
                objects: Box::new(objects),
                filter,
            },
            None => objects,
        })
    }

    fn this_way_count(
        &self,
        done: Done,
        phase: ObjectPhase,
        filter: Option<ObjectFilter>,
    ) -> Result<Expr> {
        Ok(Expr::CountObjects(Box::new(
            self.this_way_set(done, phase, filter)?,
        )))
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

    /// `None` for forms this table does not read.
    fn this_way_number(&self, value: &source::GameNumber) -> Option<Result<Expr>> {
        use ObjectPhase::{After, Before};
        use source::GameNumber as S;
        let cards = |c: &source::Cards| self.card_filter(c).map(Some);
        let permanents = |p: &source::Permanents| self.filter(p).map(Some);
        Some((|| match value {
            S::NumCardsShuffledIntoLibraryThisWay | S::NumCardsPutIntoLibraryThisWay => {
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
            _ => Err(unsupported(value)),
        })())
        .filter(|result| !matches!(result, Err(e) if e.source_type != "invariant"))
    }

    fn this_way_compare(&self, left: Expr, comparison: Comparison, right: i64) -> Condition {
        Condition::Compare {
            left,
            comparison,
            right: Expr::Constant(right),
        }
    }

    /// `None` for forms this table does not read.
    fn this_way_condition(&self, value: &source::Condition) -> Option<Result<Condition>> {
        use ObjectPhase::{After, Before};
        use source::Condition as S;
        let any = |set: Result<Expr>| {
            set.map(|left| self.this_way_compare(left, Comparison::Greater, 0))
        };
        let measured = |left: Result<Expr>, comparison: &source::Comparison| {
            let (comparison, right) = self.comparison(comparison)?;
            Ok(Condition::Compare {
                left: left?,
                comparison,
                right,
            })
        };
        let cards = |c: &source::Cards| self.card_filter(c).map(Some);
        let permanents = |p: &source::Permanents| self.filter(p).map(Some);
        Some((|| match value {
            S::ACardWasPutIntoHandThisWay(c) | S::TheCardReturnedToHandThisWayPassesFilter(c) => {
                any(self.this_way_count(Done::Returned, After, cards(c)?))
            }
            S::ACardWasPutIntoGraveyardThisWay(c) => {
                any(self.this_way_count(Done::Graveyard, After, cards(c)?))
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
}
