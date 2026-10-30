// One binder for program recipes. It reuses the existing Context frame and
// branch invalidation; it does not turn each-player batches into iteration.
impl Context {
    fn prepared_program(
        &self,
        input: crate::pipeline::programs::ProgramRef<'_>,
    ) -> Option<crate::pipeline::programs::InstructionRecipe> {
        use crate::pipeline::programs;
        if !self.prepared.programs.is_empty() {
            if let Some(recipe) = self.prepared.programs.get(&programs::source_key(input)) {
                return Some(recipe.clone());
            }
        }
        programs::rewrite(input)
    }
    fn program_instruction(
        &mut self,
        recipe: &crate::pipeline::programs::InstructionRecipe,
        actor: &PlayerSet,
    ) -> Result<Instruction> {
        let mut program = self.bind_program_recipe(recipe, actor)?;
        if program.instructions.len() != 1 {
            return Err(invalid("an action recipe requires exactly one instruction"));
        }
        Ok(program.instructions.remove(0))
    }
    fn nested_program_recipe(
        &mut self,
        recipe: &crate::pipeline::programs::InstructionRecipe,
        actor: &PlayerSet,
    ) -> Result<Program> {
        let mut child = self.clone();
        let result = child.bind_program_recipe(recipe, actor)?;
        self.result = child.result;
        self.invalidate_child_bindings(&child);
        Ok(result)
    }
    fn bind_program_recipe(
        &mut self,
        recipe: &crate::pipeline::programs::InstructionRecipe,
        actor: &PlayerSet,
    ) -> Result<Program> {
        use crate::pipeline::programs::{
            ActorInput as A, InstructionRecipe as R, ModeBounds as B, ProgramSource,
        };
        let instruction = match recipe {
            R::Source(ProgramSource::Action(a)) => self.action(a, actor)?,
            R::Source(ProgramSource::Actions(a)) => return self.actions(a),
            R::Effect(effect) => self.bind_pipeline_recipe(effect.clone(), actor)?,
            R::Sequence(actions) => {
                return Ok(Program {
                    instructions: actions
                        .iter()
                        .enumerate()
                        .map(|(i, a)| {
                            self.program_instruction(a, actor)
                                .map_err(|e| e.at(format!("actions[{i}]")))
                        })
                        .collect::<Result<_>>()?,
                })
            }
            R::Actor { actor: input, body } => {
                let players = match input {
                    A::Inherit => actor.clone(),
                    A::Player(p) => PlayerSet::One(self.player(p)?),
                    A::Batch(p) => self.players(p)?,
                };
                let instruction = match body.as_ref() {
                    crate::pipeline::programs::InstructionRecipe::Sequence(_) => {
                        Instruction::Group(self.bind_program_recipe(body, &players)?)
                    }
                    _ => self.program_instruction(body, &players)?,
                };
                if matches!(input, A::Batch(_)) && !batch_actor_instruction(&instruction) {
                    return Err(invalid(
                        "each-player composition requires an explicit iteration or batch policy",
                    ));
                }
                instruction
            }
            R::Optional { actor: input, body } => {
                let players = match input {
                    A::Inherit => actor.clone(),
                    A::Player(p) => PlayerSet::One(self.player(p)?),
                    A::Batch(_) => {
                        return Err(invalid(
                            "batch optional decisions require an explicit iteration policy",
                        ))
                    }
                };
                let PlayerSet::One(decider) = &players else {
                    return Err(invalid("optional choice requires one decider"));
                };
                Instruction::Optional {
                    decider: decider.clone(),
                    body: self.nested_program_recipe(body, &players)?,
                }
            }
            R::Conditional {
                condition,
                negated,
                then,
                otherwise,
            } => {
                let mut condition = self.bind_check_recipe(condition)?;
                if *negated {
                    condition = Condition::Not(Box::new(condition));
                }
                self.conditional(
                    condition,
                    |c| c.bind_program_recipe(then, actor),
                    |c| c.bind_program_recipe(otherwise, actor),
                )?
            }
            R::Payment {
                player,
                cost,
                optional,
            } => {
                let payer = if let Some(p) = player {
                    self.player(p)?
                } else {
                    let PlayerSet::One(p) = actor else {
                        return Err(invalid("payment requires one explicit payer"));
                    };
                    p.clone()
                };
                self.effect(Operation::PayCost {
                    cost: {
                        let mut context = self.clone();
                        context.payer = Some(payer.clone());
                        context.bind_cost_recipe(cost)?
                    },
                    player: payer,
                    optional: *optional,
                })
            }
            R::Targeted { targets, body } => {
                self.targets(targets)?;
                return self.bind_program_recipe(body, actor);
            }
            R::Modal { bounds, modes } => {
                if self.modes.is_some() {
                    return Err(invalid("multiple mode declarations in one announcement"));
                }
                let (minimum, maximum, repeat) = match bounds {
                    B::Exact(n) => (Expr::Constant(*n), Expr::Constant(*n), false),
                    B::All => (
                        Expr::Constant(modes.len() as i64),
                        Expr::Constant(modes.len() as i64),
                        false,
                    ),
                    B::OneOrMore => (Expr::Constant(1), Expr::Constant(modes.len() as i64), false),
                    B::AnyNumber => (Expr::Constant(0), Expr::Constant(modes.len() as i64), false),
                    B::UpTo(n) => (Expr::Constant(0), Expr::Constant(*n), false),
                    B::UpToSource(n) => (
                        Expr::Constant(0),
                        self.proposal().pipeline_quantity(n)?,
                        false,
                    ),
                    B::RepeatSource(n) => {
                        let amount = self.proposal().pipeline_quantity(n)?;
                        (amount.clone(), amount, true)
                    }
                };
                let modes = modes
                    .iter()
                    .enumerate()
                    .map(|(i, mode)| {
                        let mut context = Context {
                            event: self.event,
                            result: self.result,
                            acted: self.acted.clone(),
                            singular: self.singular.clone(),
                            paid: self
                                .paid
                                .iter()
                                .filter(|(_, binding)| {
                                    !matches!(
                                        binding,
                                        Some(PaidBinding {
                                            payment: PaymentRef::Resolution(_),
                                            ..
                                        })
                                    )
                                })
                                .map(|(k, v)| (*k, v.clone()))
                                .collect(),
                            ..self.fresh()
                        };
                        let body = context
                            .bind_program_recipe(
                                mode,
                                &PlayerSet::One(PlayerRef::AbilityController),
                            )
                            .map_err(|e| e.at(format!("modes[{i}]")))?;
                        Ok(Mode {
                            announcement: Announcement {
                                targets: context.targets,
                                modes: context.modes,
                            },
                            body,
                        })
                    })
                    .collect::<Result<_>>()?;
                self.modes = Some(ModeSelection {
                    minimum,
                    maximum,
                    repeat,
                    modes,
                });
                return Ok(Program::empty());
            }
        };
        Ok(Program {
            instructions: vec![instruction],
        })
    }
}
fn batch_actor_instruction(instruction: &Instruction) -> bool {
    matches!(
        instruction,
        Instruction::Effect {
            operation: Operation::CreateTokens { .. }
                | Operation::Draw { .. }
                | Operation::Discard { .. }
                | Operation::GainLife { .. }
                | Operation::LoseLife { .. }
                | Operation::Mill { .. }
                | Operation::Scry { .. }
                | Operation::Surveil { .. }
                | Operation::Shuffle { .. }
                | Operation::Sacrifice { .. },
            ..
        }
    )
}
