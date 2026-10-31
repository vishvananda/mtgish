// Predicate binding is shared across source domains; enclosing query builders
// retain zone scope, object kind, targets, and captured result identities.
impl Context {
    fn pipeline_selector(
        &self,
        input: crate::pipeline::selectors::SelectorRef<'_>,
    ) -> Option<Result<ObjectFilter>> {
        use crate::pipeline::selectors;
        if !self.prepared.selectors.is_empty() {
            if let Some(predicate) = self.prepared.selectors.get(&selectors::source_key(input)) {
                return Some(self.bind_predicate(predicate));
            }
        }
        selectors::rewrite(input).map(|p| self.bind_predicate(&p))
    }
    fn predicate_comparison(
        &self,
        value: &crate::pipeline::selectors::PredicateComparison,
    ) -> Result<(Comparison, Expr)> {
        match value {
            crate::pipeline::selectors::PredicateComparison::Source(c) => self.comparison(c),
            crate::pipeline::selectors::PredicateComparison::Fixed { operator, amount } => {
                Ok((operator.clone(), Expr::Constant(*amount)))
            }
        }
    }
    fn bind_predicate(
        &self,
        predicate: &crate::pipeline::selectors::Predicate,
    ) -> Result<ObjectFilter> {
        use crate::pipeline::selectors::{
            Predicate as P, PredicateLiteral as L, PredicateObject as O, PredicateVocabulary as V,
            SelectorSource as S,
        };
        // These operands previously flowed through the permanent-filter adapter,
        // which binds Self_It as the candidate. Direct zone/name predicates keep
        // their incoming context; establishing a candidate there would widen it.
        let mut candidate = self.clone();
        candidate.candidate = true;
        Ok(match predicate {
            P::Source(source) => {
                return match source {
                    S::Permanents(p) => self.filter(p),
                    S::Cards(p) => self.card_filter(p),
                    S::Spells(p) => self.spell_filter(p),
                    S::CardsInGraveyards(p) => self.grave_filter(p),
                    S::CardsInHand(p) => self.hand_filter(p),
                    S::CardsInExile(p) => self.exile_filter(p),
                }
            }
            P::Literal(value) => match value {
                L::Any => ObjectFilter::Any,
                L::Colorless => ObjectFilter::Colorless,
                L::Token => ObjectFilter::Token,
                L::Tapped => ObjectFilter::Tapped,
                L::Attacking => ObjectFilter::Attacking,
                L::Blocking => ObjectFilter::Blocking,
                L::Modified => ObjectFilter::Modified,
                L::IsCard => ObjectFilter::IsCard,
            },
            P::All(items) => ObjectFilter::All(
                items
                    .iter()
                    .map(|p| self.bind_predicate(p))
                    .collect::<Result<_>>()?,
            ),
            P::Either(items) => ObjectFilter::Either(
                items
                    .iter()
                    .map(|p| self.bind_predicate(p))
                    .collect::<Result<_>>()?,
            ),
            P::Not(p) => ObjectFilter::Not(Box::new(self.bind_predicate(p)?)),
            P::Vocabulary(value) => match value {
                V::CardType(v) => ObjectFilter::CardType(v.clone()),
                V::SuperType(v) => ObjectFilter::Supertype(v.clone()),
                V::SubType(v) => ObjectFilter::Subtype(v.clone()),
                V::CreatureType(v) => ObjectFilter::Subtype(crate::subtype::creature_type(v)),
                V::LandType(v) => ObjectFilter::Subtype(crate::subtype::land_type(v)),
                V::ArtifactType(v) => ObjectFilter::Subtype(crate::subtype::artifact_type(v)),
                V::EnchantmentType(v) => ObjectFilter::Subtype(crate::subtype::enchantment_type(v)),
                V::PlaneswalkerType(v) => {
                    ObjectFilter::Subtype(crate::subtype::planeswalker_type(v))
                }
                V::SpellType(v) => ObjectFilter::Subtype(crate::subtype::spell_type(v)),
            },
            P::Color(v) => self.color_filter(v)?,
            P::Keyword(v) => candidate.has_keyword(v)?,
            P::Name(v) => {
                let source::NameFilter::NamedCard(name) = v else {
                    return self
                        .primitive_predicate("NameFilter", v)
                        .ok_or_else(|| unsupported(v));
                };
                ObjectFilter::Name(name.clone())
            }
            P::ControlledBy(players) => {
                ObjectFilter::ControlledBy(candidate.pipeline_number_players(players)?)
            }
            P::OwnedBy(players) => {
                ObjectFilter::OwnedBy(candidate.pipeline_number_players(players)?)
            }
            P::SameObject(object) | P::SharesName(object) | P::Shares(_, object) => {
                let reference = match object {
                    O::Permanent(p) => candidate.object(p)?,
                    O::Spell(p) => self.spell(p)?,
                    O::SingleCard(p) => self.single_card(p)?,
                    O::CardInGraveyards(p) => self.grave_card(p)?,
                    O::CardInHand(p) => self.hand_card(p)?,
                    O::CardInExile(p) => self.exiled_card(p)?,
                };
                match predicate {
                    P::SharesName(_) => ObjectFilter::SharesNameWith(reference),
                    P::Shares(property, _) => ObjectFilter::Shares {
                        property: property.clone(),
                        with: reference,
                    },
                    _ => ObjectFilter::SameObject(reference),
                }
            }
            P::InZone { zone, players } => ObjectFilter::InZone(ZoneScope::Players {
                zone: zone.clone(),
                players: self.pipeline_number_players(players)?,
            }),
            P::Property {
                property,
                comparison,
            } => {
                let (comparison, value) = candidate.predicate_comparison(comparison)?;
                ObjectFilter::Property {
                    property: property.clone(),
                    comparison,
                    value,
                }
            }
            P::Counters {
                counter,
                comparison,
            } => {
                let (comparison, value) = candidate.predicate_comparison(comparison)?;
                ObjectFilter::Counters {
                    kind: counter.as_ref().map(crate::counter::lower),
                    comparison,
                    value,
                }
            }
        })
    }
}
