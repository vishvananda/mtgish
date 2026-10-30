impl Context {
    fn spell_ability(&mut self, actions: &source::Actions) -> Result<Ability> {
        let body = self.actions(actions)?;
        Ok(Ability::Spell {
            announcement: Announcement {
                targets: self.targets.clone(),
                modes: self.modes.clone(),
            },
            body,
        })
    }

    fn cost_branch(
        &mut self,
        keyword: &str,
        cost: &source::Cost,
        paid: &source::Actions,
        unpaid: &source::Actions,
    ) -> Result<Ability> {
        Ok(Ability::CostBranch {
            keyword: keyword.into(),
            cost: self.cost(cost)?,
            paid: Box::new(self.fresh().spell_ability(paid)?),
            unpaid: Box::new(self.fresh().spell_ability(unpaid)?),
        })
    }

    fn conditional(
        &mut self,
        condition: Condition,
        then: impl FnOnce(&mut Self) -> Result<Program>,
        otherwise: impl FnOnce(&mut Self) -> Result<Program>,
    ) -> Result<Instruction> {
        let mut then_context = self.clone();
        let then = then(&mut then_context)?;
        let mut other_context = self.clone();
        other_context.result = then_context.result;
        let otherwise = otherwise(&mut other_context)?;
        self.result = other_context.result;
        self.invalidate_child_bindings(&then_context);
        self.invalidate_child_bindings(&other_context);
        Ok(Instruction::If {
            condition,
            then,
            otherwise,
        })
    }

    fn entry_actions(&mut self, values: &[source::ReplacementActionWouldEnter]) -> Result<Program> {
        let mut instructions = Vec::new();
        for (i, value) in values.iter().enumerate() {
            // An explicit unchanged entry is the identity branch of a known
            // replacement, not a fallback for unsupported source semantics.
            if matches!(value, source::ReplacementActionWouldEnter::EntersNormally) {
                continue;
            }
            instructions.push(
                self.entry_action(value)
                    .map_err(|e| e.at(format!("entry[{i}]")))?,
            );
        }
        Ok(Program { instructions })
    }

    fn entry_child(&mut self, values: &[source::ReplacementActionWouldEnter]) -> Result<Program> {
        let mut child = self.clone();
        let program = child.entry_actions(values)?;
        self.result = child.result;
        self.invalidate_child_bindings(&child);
        Ok(program)
    }

    fn entry_cost(&self, value: &source::ReplacementActionWouldEnterCost) -> Result<Cost> {
        use source::ReplacementActionWouldEnterCost as S;
        Ok(Cost {
            parts: vec![match value {
                S::PayLife(n) => CostPart::Life(self.number(n)?),
                S::SacrificeAPermanent(p) => CostPart::Sacrifice {
                    objects: self.objects(p)?,
                    choose: Some(Count::Exactly(Expr::Constant(1))),
                },
                S::SacrificeNumberPermanents(n, p) => CostPart::Sacrifice {
                    objects: self.objects(p)?,
                    choose: Some(Count::Exactly(self.number(n)?)),
                },
                _ => return Err(unsupported(value)),
            }],
        })
    }

    fn entry_action(&mut self, value: &source::ReplacementActionWouldEnter) -> Result<Instruction> {
        use source::ReplacementActionWouldEnter as S;
        let change = match value {
            S::EntersTapped => EventChange::SetTapped(true),
            S::EntersUntapped => EventChange::SetTapped(false),
            S::EntersWithACounter(k) => EventChange::AddCounters {
                kind: crate::counter::lower(k),
                amount: Expr::Constant(1),
            },
            S::EntersWithNumberCounters(n, k) | S::EntersWithNumberCountersForEach(n, k, _) => {
                EventChange::AddCounters {
                    kind: crate::counter::lower(k),
                    amount: match value {
                        S::EntersWithNumberCountersForEach(_, _, each) => {
                            Expr::multiply(self.number(n)?, self.number(each)?)
                        }
                        _ => self.number(n)?,
                    },
                }
            }
            S::EntersUnderPlayersControl(p) => EventChange::SetController(self.player(p)?),
            S::EntersUnderAPlayersControl(p) => {
                EventChange::SetController(match self.players(p)? {
                    PlayerSet::One(player) => player,
                    _ => return Err(invalid("entry controller requires a player choice")),
                })
            }
            S::MayCost(cost) | S::MustCost(cost) => {
                return Ok(self.effect(Operation::PayCost {
                    player: self.player(&source::Player::You)?,
                    cost: self.entry_cost(cost)?,
                    optional: matches!(value, S::MayCost(_)),
                }));
            }
            S::If(condition, then) | S::Unless(condition, then) => {
                let mut condition = self.condition(condition)?;
                if matches!(value, S::Unless(..)) {
                    condition = Condition::Not(Box::new(condition));
                }
                return self.conditional(
                    condition,
                    |c| c.entry_actions(then),
                    |_| Ok(Program::empty()),
                );
            }
            S::IfElse(condition, then, otherwise) => {
                let condition = self.condition(condition)?;
                return self.conditional(
                    condition,
                    |c| c.entry_actions(then),
                    |c| c.entry_actions(otherwise),
                );
            }
            S::IfPassesFilter(filter, then) | S::IfElsePassesFilter(filter, then, _) => {
                let condition = Condition::ObjectMatches {
                    object: ObjectRef::EventSubject,
                    filter: self.filter(filter)?,
                };
                return self.conditional(
                    condition,
                    |c| c.entry_actions(then),
                    |c| match value {
                        S::IfElsePassesFilter(_, _, otherwise) => c.entry_actions(otherwise),
                        _ => Ok(Program::empty()),
                    },
                );
            }
            S::MayAction(action) => {
                return Ok(Instruction::Optional {
                    decider: self.player(&source::Player::You)?,
                    body: self.entry_child(std::slice::from_ref(action.as_ref()))?,
                })
            }
            S::MayActions(actions) => {
                return Ok(Instruction::Optional {
                    decider: self.player(&source::Player::You)?,
                    body: self.entry_child(actions)?,
                })
            }
            _ => {
                let (name, operands) = self
                    .primitive("ReplacementActionWouldEnter", value)
                    .ok_or_else(|| unsupported(value))?;
                return Ok(self.effect(Operation::Primitive {
                    actor: PlayerSet::One(self.player(&source::Player::You)?),
                    name,
                    operands,
                }));
            }
        };
        Ok(self.effect(Operation::ModifyEvent { change }))
    }

    fn entry_replacement(
        &mut self,
        active: ReplacementActive,
        objects: ObjectFilter,
        actions: &[source::ReplacementActionWouldEnter],
    ) -> Result<Ability> {
        self.event = crate::semantics::EventBindings {
            object: true,
            permanent: true,
            ..Default::default()
        };
        if matches!(active, ReplacementActive::SourceEntering) {
            // Interpret "you" using the incoming object's prospective controller;
            // it may be entering under someone else's control.
            self.you = Some(PlayerRef::ControllerOf(Box::new(ObjectRef::EventSubject)));
            self.cast_x = Some(ObjectRef::EventSubject);
        }
        Ok(Ability::Replacement {
            active,
            event: ReplacementEvent::Enter { objects },
            body: self.entry_actions(actions)?,
        })
    }
}
