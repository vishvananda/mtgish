// One binding implementation for generic numeric recipes. This stage never
// evaluates a query: targets, zone membership, and LKI remain in the final IR.
impl Context {
    fn pipeline_number_object(&self, object: &crate::pipeline::NumberObject) -> Result<ObjectRef> {
        use crate::pipeline::NumberObject as O;
        match object {
            O::Payment(action) => self.paid_object(*action),
            O::Permanent(p) => self.object(p),
            O::Spell(s) => self.spell(s),
            O::GraveCard(c) => self.grave_card(c),
            O::ExiledCard(c) => self.exiled_card(c),
            O::HandCard(c) => self.hand_card(c),
        }
    }
    fn pipeline_number_players(
        &self,
        players: &crate::pipeline::NumberPlayers,
    ) -> Result<PlayerSet> {
        match players {
            crate::pipeline::NumberPlayers::Player(p) => Ok(PlayerSet::One(self.player(p)?)),
            crate::pipeline::NumberPlayers::Players(p) => self.players(p),
        }
    }
    fn pipeline_number_query(&self, query: &crate::pipeline::NumberQuery) -> Result<ObjectSet> {
        use crate::pipeline::NumberQuery as Q;
        Ok(match query {
            Q::Payment(action) => self.paid_objects(*action, CostObjectView::Performed)?,
            Q::Battlefield(p) => self.battlefield_objects(p)?,
            Q::Objects(p) => self.objects(p)?,
            Q::GraveCards(c) => self.grave_cards(c)?,
            Q::ExiledCards(c) => self.exile_cards(c)?,
            Q::PrivateCards {
                zone,
                players,
                filter,
                excluding,
            } => {
                let mut filter = filter
                    .as_ref()
                    .map(|f| self.card_filter(f))
                    .transpose()?
                    .unwrap_or(ObjectFilter::Any);
                if let Some(object) = excluding {
                    filter = ObjectFilter::All(vec![
                        filter,
                        ObjectFilter::Not(Box::new(ObjectFilter::SameObject(
                            self.pipeline_number_object(object)?,
                        ))),
                    ]);
                }
                self.private_cards(zone.clone(), self.pipeline_number_players(players)?, filter)
            }
            Q::Union(queries) => ObjectSet::Union(
                queries
                    .iter()
                    .map(|q| self.pipeline_number_query(q))
                    .collect::<Result<_>>()?,
            ),
        })
    }
    fn pipeline_quantity(&self, amount: &crate::pipeline::Quantity) -> Result<Expr> {
        use crate::pipeline::{NumberRecipients, Quantity as Q};
        Ok(match amount {
            Q::Constant(n) => Expr::Constant(*n),
            Q::Source(n) => self.number(n)?,
            Q::Add(a, b) => Expr::add(self.pipeline_quantity(a)?, self.pipeline_quantity(b)?),
            Q::Multiply(a, b) => {
                Expr::multiply(self.pipeline_quantity(a)?, self.pipeline_quantity(b)?)
            }
            Q::Subtract(a, b) => Expr::Subtract(
                Box::new(self.pipeline_quantity(a)?),
                Box::new(self.pipeline_quantity(b)?),
            ),
            Q::Minimum(a, b) => Expr::Minimum(
                Box::new(self.pipeline_quantity(a)?),
                Box::new(self.pipeline_quantity(b)?),
            ),
            Q::Divide {
                numerator,
                denominator,
                rounding,
            } => Expr::Divide {
                numerator: Box::new(self.pipeline_quantity(numerator)?),
                denominator: Box::new(self.pipeline_quantity(denominator)?),
                rounding: rounding.clone(),
            },
            Q::Property { object, property } => Expr::ObjectProperty {
                object: self.pipeline_number_object(object)?,
                property: property.clone(),
                information: Information::CurrentOrLastKnown,
            },
            Q::PlayerLife(player) => Expr::PlayerLife(self.player(player)?),
            Q::Count(query) => Expr::CountObjects(Box::new(self.pipeline_number_query(query)?)),
            Q::Aggregate {
                query,
                property,
                aggregation,
            } => Expr::Aggregate {
                objects: Box::new(self.pipeline_number_query(query)?),
                property: property.clone(),
                aggregation: aggregation.clone(),
            },
            Q::Distinct { query, property } => Expr::CountDistinct {
                objects: Box::new(self.pipeline_number_query(query)?),
                property: property.clone(),
            },
            Q::Counters {
                recipients,
                counter,
            } => Expr::CountCounters {
                recipients: Box::new(match recipients {
                    NumberRecipients::Objects(q) => {
                        Recipients::Objects(self.pipeline_number_query(q)?)
                    }
                    NumberRecipients::Players(p) => {
                        Recipients::Players(self.pipeline_number_players(p)?)
                    }
                }),
                kind: counter.as_ref().map(crate::counter::lower),
                information: Information::CurrentOrLastKnown,
            },
        })
    }
}
