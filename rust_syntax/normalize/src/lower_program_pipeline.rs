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
    fn for_each_player(
        &mut self,
        players: PlayerSet,
        lower: impl FnOnce(&mut Self, &PlayerSet) -> Result<Instruction>,
    ) -> Result<Instruction> {
        let binding = Binding(self.binding_depth);
        let mut child = self.clone();
        child.binding_depth = self
            .binding_depth
            .checked_add(1)
            .ok_or_else(|| invalid("player iteration nesting is too deep"))?;
        child.iterated = Some(PlayerRef::Bound(binding.clone()));
        let one = PlayerSet::One(PlayerRef::Bound(binding.clone()));
        let body = match lower(&mut child, &one)? {
            Instruction::Group(program) => program,
            instruction => Program {
                instructions: vec![instruction],
            },
        };
        self.result = child.result;
        self.invalidate_child_bindings(&child);
        // Each run's picks add up, so the loop's choice reads as a set afterward.
        self.permanent_choice = child.permanent_choice.or(self.permanent_choice.take());
        self.hand_choice = child.hand_choice.or(self.hand_choice.take());
        self.exile_choice = child.exile_choice.or(self.exile_choice.take());
        self.grave_choice = child.grave_choice.or(self.grave_choice.take());
        Ok(Instruction::ForEachPlayer {
            players,
            binding,
            starting_with: None,
            body,
        })
    }

    /// Run `lower`'s body once per object; earlier bindings stay readable.
    fn for_each_object(
        &mut self,
        objects: ObjectSet,
        lower: impl FnOnce(&mut Self) -> Result<Program>,
    ) -> Result<Instruction> {
        let binding = Binding(self.binding_depth);
        let mut child = self.clone();
        child.binding_depth = self
            .binding_depth
            .checked_add(1)
            .ok_or_else(|| invalid("object iteration nesting is too deep"))?;
        child.iterated_object = Some(binding.clone());
        let body = lower(&mut child)?;
        self.result = child.result;
        self.invalidate_child_bindings(&child);
        Ok(Instruction::ForEachObject {
            objects,
            binding,
            body,
        })
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
                if let (
                    A::Batch(_),
                    R::Source(ProgramSource::Action(source::Action::RevealTopCardOfLibrary)),
                ) = (input, body.as_ref())
                {
                    let reveal = self.effect(Operation::Reveal {
                        objects: ObjectSet::LibraryTops(players),
                    });
                    if let Instruction::Effect { result, .. } = &reveal {
                        self.revealed = Some(ObjectSet::Observed(result.clone()));
                    }
                    return Ok(Program {
                        instructions: vec![reveal],
                    });
                }
                let lower = |context: &mut Self, players: &PlayerSet| match body.as_ref() {
                    crate::pipeline::programs::InstructionRecipe::Sequence(_) => Ok(
                        Instruction::Group(context.bind_program_recipe(body, players)?),
                    ),
                    _ => context.program_instruction(body, players),
                };
                if !matches!(input, A::Batch(_)) {
                    let outer = self.iterated.clone();
                    if let PlayerSet::One(player) = &players {
                        self.iterated = Some(player.clone());
                    }
                    let instruction = lower(self, &players);
                    self.iterated = outer;
                    instruction?
                } else {
                    let before = self.clone();
                    let noted = self.blockers.borrow().len();
                    match lower(self, &players) {
                        Ok(instruction) if batch_actor_instruction(&instruction) => instruction,
                        _ => {
                            *self = before;
                            self.blockers.borrow_mut().truncate(noted);
                            self.for_each_player(players, lower)?
                        }
                    }
                }
            }
            R::Optional { actor: input, body } => {
                let optional = |context: &mut Self, players: &PlayerSet| {
                    let PlayerSet::One(decider) = players else {
                        return Err(invalid("optional choice requires one decider"));
                    };
                    let body = context.nested_program_recipe(body, players)?;
                    context.note_moves(&body);
                    Ok(Instruction::Optional {
                        decider: decider.clone(),
                        body,
                    })
                };
                match input {
                    A::Inherit => optional(self, actor)?,
                    A::Player(p) => {
                        let players = PlayerSet::One(self.player(p)?);
                        optional(self, &players)?
                    }
                    A::Batch(p) => {
                        let players = self.players(p)?;
                        self.for_each_player(players, optional)?
                    }
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
                let cost = {
                    let mut context = self.clone();
                    context.payer = Some(payer.clone());
                    context.bind_cost_recipe(cost)?
                };
                // The amount chosen while paying {X} is X from here on.
                let chooses_x = cost.parts.iter().any(
                    |part| matches!(part, CostPart::Primitive { name, .. } if name == "Cost::PayManaAnyX"),
                );
                let reveals_top = cost.parts.iter().any(
                    |part| matches!(part, CostPart::Primitive { name, .. } if name == "Cost::RevealTopCardOfLibrary"),
                );
                if reveals_top {
                    self.revealed =
                        Some(self.library_slice(payer.clone(), Expr::Constant(1), SequenceEnd::Top));
                }
                let payment = self.effect(Operation::PayCost {
                    cost,
                    player: payer,
                    optional: *optional,
                });
                if let (true, Instruction::Effect { result, .. }) = (chooses_x, &payment) {
                    self.value_x = Some(result.clone());
                }
                payment
            }
            R::Targeted { targets, body } => {
                self.targets(targets)?;
                return self.bind_program_recipe(body, actor);
            }
            R::Distributed {
                targets,
                distribution,
                body,
            } => {
                self.distributed_targets(targets, distribution)?;
                return self.bind_program_recipe(body, actor);
            }
            R::Modal { bounds, modes } => {
                if self.modes.is_some() {
                    return Err(invalid("multiple mode declarations in one announcement"));
                }
                let (minimum, maximum, repeat) = match bounds {
                    B::Implied => (Expr::Constant(1), Expr::Constant(1), false),
                    B::Exact(n) => (Expr::Constant(*n), Expr::Constant(*n), false),
                    B::All => (
                        Expr::Constant(modes.len() as i64),
                        Expr::Constant(modes.len() as i64),
                        false,
                    ),
                    B::OneOrMore | B::Escalate(_) | B::DifferentTargets => {
                        (Expr::Constant(1), Expr::Constant(modes.len() as i64), false)
                    }
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
                    B::Unchosen(_) => (Expr::Constant(1), Expr::Constant(1), false),
                    B::OneOrBothIf(condition) => (
                        Expr::Constant(1),
                        Expr::Conditional {
                            condition: Box::new(self.proposal().condition(condition)?),
                            then: Box::new(Expr::Constant(2)),
                            otherwise: Box::new(Expr::Constant(1)),
                        },
                        false,
                    ),
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
                let unchosen = match bounds {
                    B::Unchosen(true) => Some(ModeHistory::ThisTurn),
                    B::Unchosen(false) => Some(ModeHistory::ThisGame),
                    _ => None,
                };
                let surcharge = match bounds {
                    B::Escalate(cost) => Some(self.proposal().cost(cost)?),
                    _ => None,
                };
                self.modes = Some(ModeSelection {
                    unchosen,
                    surcharge,
                    distinct_targets: matches!(bounds, B::DifferentTargets),
                    minimum,
                    maximum,
                    repeat,
                    modes,
                    implied: matches!(bounds, B::Implied),
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
