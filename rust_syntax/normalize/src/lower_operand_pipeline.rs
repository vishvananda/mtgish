// Generic operand binding preserves exact payment requirements, payer/context
// scopes, and the distinction between checking a choice and observing events.
impl Context {
    fn operand_recipe(
        &self,
        input: crate::pipeline::operands::OperandRef<'_>,
    ) -> Option<crate::pipeline::operands::OperandRecipe> {
        use crate::pipeline::operands;
        if !self.prepared.operands.is_empty() {
            if let Some(recipe) = self.prepared.operands.get(&operands::source_key(input)) {
                return Some(recipe.clone());
            }
        }
        operands::rewrite(input)
    }
    fn cost_object_selection(
        &self,
        subjects: &crate::pipeline::Subjects,
        selection: Option<&crate::pipeline::operands::CostSelection>,
    ) -> Result<CostObjectSelection> {
        use crate::pipeline::operands::CostSelection as S;
        Ok(CostObjectSelection {
            objects: self.pipeline_subjects(
                subjects,
                &PlayerSet::One(self.payer.clone().unwrap_or(PlayerRef::AbilityController)),
            )?,
            choose: selection
                .map(|s| match s {
                    S::Exactly(n) => self.pipeline_quantity(n).map(Count::Exactly),
                    S::AnyNumber => Ok(Count::AnyNumber),
                    S::OneOrMore => Ok(Count::AtLeast(Expr::Constant(1))),
                })
                .transpose()?,
        })
    }
    fn bind_cost_recipe(&self, recipe: &crate::pipeline::operands::CostRecipe) -> Result<Cost> {
        use crate::pipeline::operands::{AmountOperation as O, CostFilter as F, CostRecipe as R};
        let parts = match recipe {
            R::Source(cost) => return self.cost(cost),
            R::Constrained { cost, group } => {
                let constraints = self.group_constraints(group)?;
                let mut cost = self.bind_cost_recipe(cost)?;
                if cost.parts.len() != 1 {
                    return Err(invalid("group qualification requires one cost component"));
                }
                vec![CostPart::Constrained {
                    part: Box::new(cost.parts.remove(0)),
                    constraints,
                }]
            }
            R::All(costs) => {
                let mut parts = vec![];
                for cost in costs {
                    parts.extend(self.bind_cost_recipe(cost)?.parts);
                }
                parts
            }
            R::Choice(costs) => vec![CostPart::Choice(
                costs
                    .iter()
                    .map(|c| self.bind_cost_recipe(c))
                    .collect::<Result<_>>()?,
            )],
            R::Mana(mana) => vec![CostPart::Mana(
                mana.iter().map(mana_symbol).collect::<Result<_>>()?,
            )],
            R::ManaX { mana, variable } => {
                if !matches!(variable, source::GameNumber::ValueX) {
                    let cost = source::Cost::PayManaX(mana.clone(), Box::new(variable.clone()));
                    return self.primitive_cost(&cost).ok_or_else(|| unsupported(&cost));
                }
                vec![CostPart::Mana(
                    mana.iter().map(mana_symbol_x).collect::<Result<_>>()?,
                )]
            }
            R::TapPermanent { object, tapped } => {
                if matches!(object, source::Permanent::ThisPermanent) && *tapped {
                    vec![CostPart::TapSymbol]
                } else {
                    let symbol = matches!(object, source::Permanent::ThisPermanent);
                    let object = if symbol && self.keyword_host {
                        ObjectRef::KeywordHost
                    } else {
                        self.object(object)?
                    };
                    vec![CostPart::ChangeTapState {
                        selections: vec![CostObjectSelection {
                            objects: ObjectSet::One(object),
                            choose: None,
                        }],
                        tapped: *tapped,
                        symbol,
                    }]
                }
            }
            R::TapObjects {
                subjects,
                selection,
                tapped,
            } => vec![CostPart::ChangeTapState {
                selections: vec![self.cost_object_selection(subjects, selection.as_ref())?],
                tapped: *tapped,
                symbol: false,
            }],
            R::OwnMana => vec![CostPart::ManaCostOf(if self.keyword_host {
                ObjectRef::KeywordHost
            } else {
                ObjectRef::Source
            })],
            R::Amount { operation, amount } => {
                let count = self.pipeline_quantity(amount)?;
                vec![match operation {
                    O::Life => CostPart::Life(count),
                    O::Draw => CostPart::Draw { count },
                    O::Mill => CostPart::Mill { count },
                }]
            }
            R::Loyalty(n) => vec![CostPart::Loyalty(Expr::Constant(*n))],
            R::Sacrifice {
                subjects,
                selection,
            } => {
                let CostObjectSelection { objects, choose } =
                    self.cost_object_selection(subjects, selection.as_ref())?;
                vec![CostPart::Sacrifice { objects, choose }]
            }
            R::Return { subjects, amount } => vec![CostPart::Move {
                selections: vec![CostObjectSelection {
                    objects: self.objects(subjects)?,
                    choose: Some(Count::Exactly(self.pipeline_quantity(amount)?)),
                }],
                destination: Destination::OwnersHand,
                entry: EntrySpec::default(),
                face_down: false,
            }],
            R::Discard {
                amount,
                filter,
                random,
            } => vec![CostPart::Discard {
                count: self.pipeline_quantity(amount)?,
                filter: match filter {
                    F::Any => ObjectFilter::Any,
                    F::Cards(p) => self.bind_predicate(p)?,
                    F::HandCard(c) => ObjectFilter::SameObject(self.hand_card(c)?),
                },
                random: *random,
            }],
            R::Exile { items, flags } => {
                let (entry, face_down) = self.exile_entry(flags)?;
                vec![CostPart::Move {
                    destination: Destination::Exile,
                    selections: items
                        .iter()
                        .enumerate()
                        .map(|(i, c)| {
                            self.exile_cost_selection(c)
                                .map_err(|e| e.at(format!("selections[{i}]")))
                        })
                        .collect::<Result<_>>()?,
                    entry,
                    face_down,
                }]
            }
            R::RemoveCounters(removals) => vec![CostPart::RemoveCounters {
                removals: removals
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        self.remove_counter_cost(r)
                            .map_err(|e| e.at(format!("removals[{i}]")))
                    })
                    .collect::<Result<_>>()?,
            }],
        };
        Ok(Cost { parts })
    }
    fn bind_check_recipe(
        &self,
        recipe: &crate::pipeline::operands::CheckRecipe,
    ) -> Result<Condition> {
        use crate::pipeline::{
            operands::{CheckRecipe as R, MatchDomain},
            NumberPlayers,
        };
        Ok(match recipe {
            R::Source(c) => return self.condition(c),
            R::All(items) => Condition::All(
                items
                    .iter()
                    .map(|c| self.bind_check_recipe(c))
                    .collect::<Result<_>>()?,
            ),
            R::Either(items) => Condition::Either(
                items
                    .iter()
                    .map(|c| self.bind_check_recipe(c))
                    .collect::<Result<_>>()?,
            ),
            R::Payment { negated } => {
                let paid = Condition::PaymentChosen(
                    self.payment
                        .clone()
                        .ok_or_else(|| invalid("payment reference has no cost in this scope"))?,
                );
                if *negated {
                    Condition::Not(Box::new(paid))
                } else {
                    paid
                }
            }
            R::Compare { left, comparison } => {
                let (comparison, right) = self.comparison(comparison)?;
                Condition::Compare {
                    left: self.pipeline_quantity(left)?,
                    comparison,
                    right,
                }
            }
            R::SpellX(c) => {
                let (comparison, right) = self.comparison(c)?;
                Condition::Compare {
                    left: Expr::ObjectProperty {
                        object: ObjectRef::Source,
                        property: Property::SpellX,
                        information: Information::CurrentOrLastKnown,
                    },
                    comparison,
                    right,
                }
            }
            R::Matches {
                object,
                domain,
                filter,
            } => {
                let object = self.pipeline_number_object(object)?;
                let mut context = self.clone();
                if matches!(domain, MatchDomain::Permanents | MatchDomain::Spells) {
                    context.candidate = true;
                }
                Condition::ObjectMatches {
                    object,
                    filter: context.bind_predicate(filter)?,
                }
            }
            R::Active { players, negated } => {
                let active = match players {
                    NumberPlayers::Player(player) => {
                        Condition::PlayerIsActive(self.player(player)?)
                    }
                    NumberPlayers::Players(players) => {
                        let binding = Binding(self.binding_depth);
                        Condition::ExistsPlayer {
                            players: self.players(players)?,
                            binding: binding.clone(),
                            condition: Box::new(Condition::PlayerIsActive(PlayerRef::Bound(
                                binding,
                            ))),
                        }
                    }
                };
                if *negated {
                    Condition::Not(Box::new(active))
                } else {
                    active
                }
            }
            R::Player { players, filter } => match players {
                NumberPlayers::Player(player) => {
                    self.player_matches(self.player(player)?, filter)?
                }
                NumberPlayers::Players(players) => {
                    let players = self.players(players)?;
                    let binding = Binding(self.binding_depth);
                    let mut child = self.clone();
                    child.binding_depth = self
                        .binding_depth
                        .checked_add(1)
                        .ok_or_else(|| invalid("player predicate nesting is too deep"))?;
                    Condition::ExistsPlayer {
                        players,
                        binding: binding.clone(),
                        condition: Box::new(
                            child.player_matches(PlayerRef::Bound(binding), filter)?,
                        ),
                    }
                }
            },
            R::SourceZones(zones) => Condition::ObjectMatches {
                object: ObjectRef::Source,
                filter: ObjectFilter::Either(
                    zones
                        .iter()
                        .map(|zone| ObjectFilter::InZone(ZoneScope::Shared(zone.clone())))
                        .collect(),
                ),
            },
            R::Graveyard { object, owner } => Condition::ObjectMatches {
                object: object
                    .as_ref()
                    .map(|o| self.grave_card(o))
                    .transpose()?
                    .unwrap_or(ObjectRef::Source),
                filter: ObjectFilter::InZone(ZoneScope::Players {
                    zone: EventZone::Graveyard,
                    players: PlayerSet::One(self.player(owner)?),
                }),
            },
        })
    }
}
