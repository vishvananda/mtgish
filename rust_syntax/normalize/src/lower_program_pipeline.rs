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
        let binding = LoopBinding(self.binding_depth);
        let mut child = self.clone();
        child.entered_from_one = None;
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
        self.adopt_loop(&child);
        if Self::one_coin_per_player(&body, &binding) && Self::stable_coin_players(&players) {
            self.coin_players = Some(players.clone());
        }
        // Each run's picks add up, so the loop's choice reads as a set afterward.
        let picks = child.picks[self.picks.len()..].iter().map(|(who, result)| {
            let own = matches!(who, Some(PlayerRef::Bound(b)) if b.0 == binding.0);
            (who.clone().filter(|_| !own), result.clone())
        });
        self.picks.extend(picks.collect::<Vec<_>>());
        self.permanent_choice = child.permanent_choice.or(self.permanent_choice.take());
        self.hand_choice = child.hand_choice.or(self.hand_choice.take());
        self.exile_choice = child.exile_choice.or(self.exile_choice.take());
        self.grave_choice = child.grave_choice.or(self.grave_choice.take());
        self.took = child.took.or(self.took.take());
        Ok(Instruction::ForEachPlayer {
            players,
            binding,
            starting_with: None,
            in_chosen_direction: false,
            body,
        })
    }

    /// A loop's runs add up, so what its body did reads as a set afterward.
    fn adopt_loop(&mut self, child: &Self) {
        self.did.extend(child.did.clone());
        self.looped.extend(child.did.values().map(|result| result.0));
        for (action, result) in &child.acted {
            self.acted.insert(*action, result.clone());
        }
        self.created_tokens = child.created_tokens.clone().or(self.created_tokens.take());
        self.entered = child.entered.clone().or(self.entered.take());
        if child.exiled.is_some() {
            self.exiled = child.exiled.clone();
            self.exile_found = child.exile_found.clone();
            self.exiled_permanents = child.exiled_permanents;
        }
    }

    /// Run `lower`'s body once per object; earlier bindings stay readable.
    fn for_each_object(
        &mut self,
        objects: ObjectSet,
        lower: impl FnOnce(&mut Self) -> Result<Program>,
    ) -> Result<Instruction> {
        let binding = LoopBinding(self.binding_depth);
        let mut child = self.clone();
        child.entered_from_one = None;
        child.binding_depth = self
            .binding_depth
            .checked_add(1)
            .ok_or_else(|| invalid("object iteration nesting is too deep"))?;
        child.iterated_object = Some(binding.clone());
        let body = lower(&mut child)?;
        self.result = child.result;
        self.invalidate_child_bindings(&child);
        self.adopt_loop(&child);
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
        child.entered_from_one = None;
        let result = child.bind_program_recipe(recipe, actor)?;
        self.result = child.result;
        self.invalidate_child_bindings(&child);
        // What a "may" chose is still what "those cards" reads after it.
        let picks = child.picks[self.picks.len()..].iter().cloned();
        self.picks.extend(picks.collect::<Vec<_>>());
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
            R::Effect(effect) => self.acted_by(actor, |context| {
                context.bind_pipeline_recipe(effect.clone(), actor)
            })?,
            R::Sequence(actions) => {
                let outer = self.reads_later;
                let instructions = actions
                    .iter()
                    .enumerate()
                    .map(|(i, a)| {
                        // Only a branch something later reads joins its results.
                        self.reads_later = outer
                            || matches!(
                                a,
                                R::Conditional { .. }
                                    | R::Source(ProgramSource::Action(
                                        source::Action::ChooseAnAction(..)
                                    ))
                            ) && format!("{:?}", &actions[i + 1..]).contains("ThisWay");
                        self.program_instruction(a, actor)
                            .map_err(|e| e.at(format!("actions[{i}]")))
                    })
                    .collect::<Result<_>>();
                self.reads_later = outer;
                return Ok(Program {
                    instructions: instructions?,
                });
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
                    self.offered = Some(players.clone());
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
                    // "For each opponent, you may put ... from that player's
                    // graveyard" (Sepulchral Primordial): under your "may", the
                    // player walked is still that player.
                    let walked = matches!(context.iterated, Some(PlayerRef::Bound(_)))
                        && matches!(decider, PlayerRef::AbilityController);
                    let outer = match walked {
                        true => context.iterated.clone(),
                        false => context.iterated.replace(decider.clone()),
                    };
                    // After an earlier creation "those tokens" could name either.
                    let shadowing = context.created_tokens.is_some();
                    let body = context.nested_program_recipe(body, players);
                    context.iterated = outer;
                    let mut body = body?;
                    context.note_moves(&body);
                    if shadowing {
                        context.created_tokens = None;
                        context.forgotten = true;
                    }
                    context.took = last_effect(&body).or(context.took.take());
                    // A chosen resolving payment already records the may.
                    // Its reflexive trigger observes that choice, including
                    // replacement, and does nothing when payment is declined.
                    if Self::optional_reflexive_payment(&mut body, decider) {
                        return Ok(Instruction::Group(body));
                    }
                    let guard = Self::optional_fixed_exile_guard(&body);
                    let optional = Instruction::Optional { decider: decider.clone(), body };
                    Ok(match guard {
                        Some(condition) => Instruction::If {
                            condition,
                            then: Program { instructions: vec![optional] },
                            otherwise: Program { instructions: vec![] },
                        },
                        None => optional,
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
                        self.offered = Some(players.clone());
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
                    context.resolving_payment = true;
                    context.bind_cost_recipe(cost)?
                };
                // Source MustCost also spells compulsory effects whose success is tested later. Keep these
                // as ordinary effects with their actual receipts; PaymentChosen retains its CR 118.12 meaning.
                fn required_effect(body: &Program) -> Option<(&Operation, RequiredEffectOutcome)> {
                    match body.instructions.as_slice() {
                        [Instruction::Group(body)] => required_effect(body),
                        [Instruction::Effect { operation, .. }] => match operation.action() {
                            Operation::Destroy { .. } => Some((operation, RequiredEffectOutcome::Destroyed)),
                            Operation::Move { destination: Destination::OwnersHand, .. }
                                => Some((operation, RequiredEffectOutcome::ReturnedToHand)),
                            _ => None,
                        },
                        _ => None,
                    }
                }
                if let (false, [CostPart::PlayerAction { body, .. }]) = (*optional, cost.parts.as_slice()) {
                    if let Some((operation, outcome)) = required_effect(body) {
                        let step = self.effect(operation.clone());
                        let Instruction::Effect { result, .. } = &step else { unreachable!() };
                        self.payment = Some(result.clone());
                        self.required_effect_outcomes.insert(result.0, outcome);
                        return Ok(Program { instructions: vec![step] });
                    }
                }
                // The amount chosen while paying {X} is X from here on.
                let chooses_x = cost
                    .parts
                    .iter()
                    .any(|part| matches!(part, CostPart::ManaWithX { x: None, .. }));
                let reveals_top = cost.parts.iter().any(
                    |part| matches!(part, CostPart::Primitive { name, .. } if name == "Cost::RevealTopCardOfLibrary"),
                );
                if reveals_top {
                    self.revealed =
                        Some(self.library_slice(payer.clone(), Expr::Constant(1), SequenceEnd::Top));
                }
                // A forced choice among exiled cards has nothing to decline, so
                // it counts as paid and later steps name what it chose.
                if let (false, [CostPart::PlayerAction { body, .. }]) =
                    (*optional, cost.parts.as_slice())
                {
                    if let [Instruction::Effect {
                        operation: choice @ Operation::ChooseObjects { objects, .. },
                        ..
                    }] = body.instructions.as_slice()
                    {
                        if matches!(objects, ObjectSet::Transition { .. }) {
                            let choice = self.effect(choice.clone());
                            if let Instruction::Effect { result, .. } = &choice {
                                self.exile_choice = Some(result.clone());
                            }
                            let payment = self.effect(Operation::PayCost {
                                cost: Cost { parts: vec![] },
                                player: payer,
                                optional: false,
                            });
                            return Ok(Program {
                                instructions: vec![Instruction::Group(Program {
                                    instructions: vec![choice, payment],
                                })],
                            });
                        }
                    }
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
            R::Modal {
                bounds,
                modes,
                weights,
            } => {
                if self.modes.is_some() {
                    return Err(invalid("multiple mode declarations in one announcement"));
                }
                let (minimum, maximum, repeat) = match bounds {
                    B::Implied
                    | B::Random
                    | B::Chooser(_)
                    | B::Either(_)
                    | B::UnchosenLastCombat(_) => (Expr::Constant(1), Expr::Constant(1), false),
                    B::ExactlyDifferentTargets(n) | B::MayDifferentTargets(n) => {
                        (Expr::Constant(*n), Expr::Constant(*n), false)
                    }
                    B::Budget(n) => (
                        Expr::Constant(0),
                        self.proposal().pipeline_quantity(n)?,
                        true,
                    ),
                    B::OneOrMoreIf(condition) => (
                        Expr::Constant(1),
                        Expr::Conditional {
                            condition: Box::new(self.proposal().condition(condition)?),
                            then: Box::new(Expr::Constant(modes.len() as i64)),
                            otherwise: Box::new(Expr::Constant(1)),
                        },
                        false,
                    ),
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
                            cast_x: self.cast_x.clone(),
                            result: self.result,
                            acted: self.acted.clone(),
                            singular: self.singular.clone(),
                            did: self.did.clone(),
                            exiled: self.exiled.clone(),
                            exiled_permanents: self.exiled_permanents,
                            entered: self.entered.clone(),
                            created_tokens: self.created_tokens.clone(),
                            rolled: self.rolled.clone(),
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
                            weight: weights.get(i).copied().unwrap_or(1),
                            requires: match (bounds, i) {
                                (B::Either(condition), 0) => {
                                    Some(self.proposal().condition(condition)?)
                                }
                                (B::Either(condition), _) => Some(Condition::Not(Box::new(
                                    self.proposal().condition(condition)?,
                                ))),
                                _ => None,
                            },
                        })
                    })
                    .collect::<Result<_>>()?;
                let unchosen = match bounds {
                    B::Unchosen(true) => Some(ModeHistory::ThisTurn),
                    B::Unchosen(false) => Some(ModeHistory::ThisGame),
                    B::UnchosenLastCombat(player) => {
                        Some(ModeHistory::LastCombat(self.proposal().player(player)?))
                    }
                    _ => None,
                };
                let surcharge = match bounds {
                    B::Escalate(cost) => Some(self.proposal().cost(cost)?),
                    _ => None,
                };
                self.modes = Some(crate::mode_policies::LegacyModeSelection {
                    unchosen,
                    surcharge,
                    distinct_targets: matches!(
                        bounds,
                        B::DifferentTargets
                            | B::ExactlyDifferentTargets(_)
                            | B::MayDifferentTargets(_)
                    ),
                    chooser: match bounds {
                        B::Chooser(players) => Some(self.proposal().players(players)?),
                        _ => None,
                    },
                    random: matches!(bounds, B::Random),
                    budget: match bounds {
                        B::Budget(n) => Some(self.proposal().pipeline_quantity(n)?),
                        _ => None,
                    },
                    may_decline: matches!(bounds, B::MayDifferentTargets(_)),
                    minimum,
                    maximum,
                    repeat,
                    modes,
                    implied: matches!(bounds, B::Implied | B::Either(_)),
                }.current().map_err(|e| invalid(&e))?);
                return Ok(Program::empty());
            }
        };
        Ok(Program {
            instructions: vec![instruction],
        })
    }
}
fn batch_actor_instruction(instruction: &Instruction) -> bool {
    // "Each player chooses, then sacrifices the rest": one sacrifice for them all.
    if let Instruction::Group(program) = instruction {
        return matches!(
            program.instructions.as_slice(),
            [
                Instruction::ForEachPlayer { .. },
                Instruction::Effect {
                    operation: Operation::Sacrifice { .. },
                    ..
                }
            ]
        );
    }
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
