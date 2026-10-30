// Reusable definitions expand to existing typed costs, changes, and programs.
impl Context {
    fn bind_mechanic(
        &self,
        recipe: &crate::pipeline::mechanics::MechanicRecipe,
    ) -> Result<Ability> {
        use crate::pipeline::mechanics::Mechanic;
        match recipe.mechanic {
            Mechanic::Crew => {
                if recipe.threshold < 0 {
                    return Err(invalid("crew threshold must be nonnegative"));
                }
                let mut context = self.fresh();
                let cost = Cost {
                    parts: vec![CostPart::Constrained {
                        part: Box::new(CostPart::ChangeTapState {
                            selections: vec![CostObjectSelection {
                                objects: ObjectSet::Battlefield(ObjectFilter::All(vec![
                                    ObjectFilter::CardType(source::CardType::Creature),
                                    ObjectFilter::Not(Box::new(ObjectFilter::Tapped)),
                                    ObjectFilter::ControlledBy(PlayerSet::One(
                                        PlayerRef::AbilityController,
                                    )),
                                    ObjectFilter::Not(Box::new(ObjectFilter::SameObject(
                                        ObjectRef::Source,
                                    ))),
                                ])),
                                choose: Some(Count::AnyNumber),
                            }],
                            tapped: true,
                            symbol: false,
                        }),
                        constraints: vec![SelectionConstraint::TotalProperty {
                            property: Property::Power,
                            comparison: Comparison::GreaterOrEqual,
                            amount: Expr::Constant(recipe.threshold),
                        }],
                    }],
                };
                let body = Program {
                    instructions: vec![context.effect(Operation::CreateContinuousEffect {
                        source: ObjectRef::Source,
                        affected: EffectSubjects::Objects(ObjectSet::One(ObjectRef::Source)),
                        changes: vec![
                            Change::AddCardType(source::CardType::Artifact),
                            Change::AddCardType(source::CardType::Creature),
                        ],
                        duration: Duration::EndOfTurn,
                        evaluation: EffectEvaluation::Snapshot,
                    })],
                };
                Ok(Ability::Activated {
                    origin: ActivationOrigin::Crew,
                    cost,
                    limits: ActivationLimits {
                        once: false,
                        once_each_turn: recipe.once_each_turn,
                        sorcery: false,
                        condition: None,
                    },
                    announcement: Announcement {
                        targets: vec![],
                        modes: None,
                    },
                    body,
                })
            }
        }
    }
}
