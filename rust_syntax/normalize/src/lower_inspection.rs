#[derive(Clone)]
struct InspectionState {
    observed: ObjectSet,
    assigned: Vec<ObjectSet>,
    found: Option<ObjectSet>,
    /// Whether an optional step that is no payment was taken.
    did: Option<Condition>,
    /// How many top cards of the library a look took in.
    top: Option<Expr>,
}
impl InspectionState {
    fn remaining(&self) -> ObjectSet {
        let objects = if self.assigned.is_empty() {
            self.observed.clone()
        } else {
            ObjectSet::Difference {
                objects: Box::new(self.observed.clone()),
                excluding: Box::new(ObjectSet::Union(self.assigned.clone())),
            }
        };
        ObjectSet::InZone {
            objects: Box::new(objects),
            zone: EventZone::Library,
        }
    }
}

/// The ordinary cost a may-step pays, if it only pays.
fn inspection_payment(cost: &source::LookAtTopOfLibraryCost) -> Option<source::Cost> {
    use source::LookAtTopOfLibraryCost as S;
    Some(match cost {
        S::And(parts) => source::Cost::And(
            parts
                .iter()
                .map(inspection_payment)
                .collect::<Option<_>>()?,
        ),
        S::PayLife(life) => source::Cost::PayLife(life.clone()),
        S::PayMana(mana) => source::Cost::PayMana(mana.clone()),
        S::SacrificePermanent(object) => source::Cost::SacrificePermanent(object.clone()),
        S::SacrificeAPermanent(objects) => source::Cost::SacrificeAPermanent(objects.clone()),
        _ => return None,
    })
}

impl Context {
    /// A step that is an ordinary action sharing a name across these enums. Keeps
    /// nothing from a failed attempt, so a row can still stand in for the step.
    fn sibling_step(
        &mut self,
        step: &impl Serialize,
        player: &PlayerRef,
        instructions: &mut Vec<Instruction>,
    ) -> Option<Result<()>> {
        let action: source::Action = as_family(step, "Action")?;
        let mut attempt = self.clone();
        match attempt.action(&action, &PlayerSet::One(player.clone())) {
            Ok(instruction) => {
                *self = attempt;
                instructions.push(instruction);
                Some(Ok(()))
            }
            Err(_) => None,
        }
    }

    fn inspect_library(
        &mut self,
        player: PlayerRef,
        library: PlayerRef,
        amount: Expr,
        actions: &[source::LookAtTopOfLibraryAction],
    ) -> Result<Instruction> {
        self.inspection_hand = Some(vec![]);
        let top = Some(amount.clone());
        let look = self.effect(Operation::LookAt {
            player: player.clone(),
            objects: self.library_slice(library.clone(), amount, SequenceEnd::Top),
        });
        let Instruction::Effect { result, .. } = &look else {
            unreachable!()
        };
        let state = InspectionState {
            observed: ObjectSet::Observed(result.clone()),
            assigned: vec![],
            did: None,
            found: None,
            top,
        };
        let mut body = self.inspection_steps(&player, &library, state, actions)?;
        body.instructions.insert(0, look);
        Ok(Instruction::Group(body))
    }

    fn inspection_steps(
        &mut self,
        player: &PlayerRef,
        library: &PlayerRef,
        mut state: InspectionState,
        actions: &[source::LookAtTopOfLibraryAction],
    ) -> Result<Program> {
        use source::LookAtTopOfLibraryAction as S;
        let mut instructions = vec![];
        for (i, action) in actions.iter().enumerate() {
            if state.found.is_some() {
                self.found = state.found.clone();
            }
            self.inspected_put = state.assigned.last().cloned();
            // Continue inside each branch so its local choices and remaining
            // set never escape their declaration scope or need implicit phi IDs.
            if let S::If(condition, then)
            | S::Unless(condition, then)
            | S::IfElse(condition, then, _) = action
            {
                let mut condition = match (condition, &state.did) {
                    (source::Condition::CostWasPaid, Some(did)) => did.clone(),
                    _ => self.condition(condition)?,
                };
                if matches!(action, S::Unless(..)) {
                    condition = Condition::Not(Box::new(condition));
                }
                let mut yes = then.clone();
                yes.extend_from_slice(&actions[i + 1..]);
                let mut no = if let S::IfElse(_, _, otherwise) = action {
                    otherwise.clone()
                } else {
                    vec![]
                };
                no.extend_from_slice(&actions[i + 1..]);
                let mut yes_context = self.clone();
                let yes = yes_context.inspection_steps(player, library, state.clone(), &yes)?;
                self.result = yes_context.result;
                let mut no_context = self.clone();
                let no = no_context.inspection_steps(player, library, state, &no)?;
                self.result = no_context.result;
                self.invalidate_child_bindings(&yes_context);
                self.invalidate_child_bindings(&no_context);
                instructions.push(Instruction::If {
                    condition,
                    then: yes,
                    otherwise: no,
                });
                return Ok(Program { instructions });
            }
            let start = instructions.len();
            self.inspection_step(
                player,
                library,
                &mut state,
                action,
                false,
                &mut instructions,
            )
            .map_err(|e| e.at(format!("inspection[{i}]")))?;
            self.note_moves(&Program {
                instructions: instructions[start..].to_vec(),
            });
        }
        Ok(Program { instructions })
    }

    /// "You may" a cost whose "if you do" is read by the steps after it. Paying
    /// is `PayCost`; a step on the cards themselves is done if it found a card.
    fn inspection_may(
        &mut self,
        player: &PlayerRef,
        library: &PlayerRef,
        state: &mut InspectionState,
        cost: &source::LookAtTopOfLibraryCost,
        instructions: &mut Vec<Instruction>,
    ) -> Result<()> {
        use source::LookAtTopOfLibraryAction as A;
        use source::LookAtTopOfLibraryCost as S;
        let took = |objects| Condition::Compare {
            left: Expr::CountObjects(Box::new(objects)),
            comparison: Comparison::GreaterOrEqual,
            right: Expr::Constant(1),
        };
        match cost {
            S::PutACardOfTypeOntoTheBattlefield(filter, flags) => {
                let step = A::MayPutACardOfTypeOntoTheBattlefield(filter.clone(), flags.clone());
                let before = state.assigned.len();
                self.inspection_step(player, library, state, &step, false, instructions)?;
                state.did = Some(took(ObjectSet::Union(state.assigned[before..].to_vec())));
            }
            S::RevealACardOfType(filter) => {
                let step = A::RevealACardOfType(filter.clone());
                self.inspection_step(player, library, state, &step, true, instructions)?;
                state.did = state.found.clone().map(took);
            }
            _ => {
                let Some(cost) = inspection_payment(cost) else {
                    let step = self.primitive_step(
                        "LookAtTopOfLibraryCost",
                        cost,
                        player,
                        state.remaining(),
                    )?;
                    instructions.push(step);
                    return Ok(());
                };
                let payer = self.payer.replace(player.clone());
                let cost = self.cost(&cost);
                self.payer = payer;
                state.did = None;
                let operation = Operation::PayCost {
                    player: player.clone(),
                    cost: cost?,
                    optional: true,
                };
                instructions.push(self.effect(operation));
            }
        }
        Ok(())
    }

    fn inspection_choice(
        &mut self,
        player: &PlayerRef,
        objects: ObjectSet,
        filter: ObjectFilter,
        count: Count,
        constraints: Vec<SelectionConstraint>,
        instructions: &mut Vec<Instruction>,
    ) -> ObjectSet {
        let objects = if matches!(filter, ObjectFilter::Any) {
            objects
        } else {
            ObjectSet::Matching {
                objects: Box::new(objects),
                filter,
            }
        };
        let choice = self.effect(Operation::ChooseObjects {
            player: player.clone(),
            objects,
            count,
            constraints,
        });
        let Instruction::Effect { result, .. } = &choice else {
            unreachable!()
        };
        let chosen = ObjectSet::Selected(result.clone());
        instructions.push(choice);
        chosen
    }

    fn inspection_assignment(
        &mut self,
        operation: Operation,
        state: &mut InspectionState,
        instructions: &mut Vec<Instruction>,
    ) {
        let into_hand = matches!(
            operation,
            Operation::Move {
                destination: Destination::OwnersHand,
                ..
            }
        );
        let instruction = self.effect(operation);
        let Instruction::Effect { result, .. } = &instruction else {
            unreachable!()
        };
        if into_hand {
            if let Some(results) = &mut self.inspection_hand {
                results.push(result.clone());
            }
        }
        // Only actual placements consume cards from the cohort. A failed move
        // leaves the original card available to the later remaining-card step.
        state.assigned.push(ObjectSet::Transition {
            result: result.clone(),
            phase: ObjectPhase::Before,
        });
        instructions.push(instruction);
    }

    fn inspection_step(
        &mut self,
        player: &PlayerRef,
        library: &PlayerRef,
        state: &mut InspectionState,
        action: &source::LookAtTopOfLibraryAction,
        optional: bool,
        instructions: &mut Vec<Instruction>,
    ) -> Result<()> {
        use source::LookAtTopOfLibraryAction as S;
        if let S::MayAction(action) = action {
            return self.inspection_step(player, library, state, action, true, instructions);
        }
        if let S::MayCost(cost) = action {
            return self.inspection_may(player, library, state, cost, instructions);
        }
        if !optional {
            let more = self.inspection_step_more(player, library, state, action, instructions);
            if let Some(done) = more {
                return done;
            }
        }
        // Whatever the body picks is no longer among the rest, wherever it went.
        if let S::MayActions(steps) = action {
            let body = self.inspection_steps(player, library, state.clone(), steps)?;
            let picked = body.instructions.iter().find_map(|step| match step {
                Instruction::Effect {
                    result,
                    operation: Operation::ChooseObjects { .. },
                } => Some(result.clone()),
                _ => None,
            });
            if let Some(picked) = picked {
                state.assigned.push(ObjectSet::Selected(picked));
            }
            instructions.push(Instruction::Optional {
                decider: player.clone(),
                body,
            });
            return Ok(());
        }
        if let S::MayRevealMultipleCardsOfTypeAndPutIntoHand(filters) = action {
            let mut selected = vec![];
            for filter in filters {
                let pool = if selected.is_empty() {
                    state.remaining()
                } else {
                    ObjectSet::Difference {
                        objects: Box::new(state.remaining()),
                        excluding: Box::new(ObjectSet::Union(selected.clone())),
                    }
                };
                selected.push(self.inspection_choice(
                    player,
                    pool,
                    self.library_filter(filter)?,
                    Count::UpTo(Expr::Constant(1)),
                    vec![SelectionConstraint::CanMoveTo {
                        destination: Destination::OwnersHand,
                        entry: EntrySpec::default(),
                        face_down: false,
                    }],
                    instructions,
                ));
            }
            let objects = ObjectSet::Union(selected);
            instructions.push(self.effect(Operation::Reveal {
                objects: objects.clone(),
            }));
            state.found = Some(objects.clone());
            self.inspection_assignment(
                Operation::Move {
                    objects,
                    destination: Destination::OwnersHand,
                    entry: EntrySpec::default(),
                    face_down: false,
                },
                state,
                instructions,
            );
            return Ok(());
        }
        let remaining = state.remaining();
        let one = || Expr::Constant(1);
        let count = |n, may| {
            if may || optional {
                Count::UpTo(n)
            } else {
                Count::AsManyAsCan(n)
            }
        };
        let default_entry = EntrySpec::default();
        // A choice may reveal, move, or reorder. Reveal-only choices remain
        // among the rest until a later instruction assigns the found card.
        let mut choice: Option<(
            ObjectFilter,
            Count,
            bool,
            Option<(Destination, EntrySpec, bool)>,
            Option<(SequenceEnd, CardOrder)>,
        )> = None;
        match action {
            S::PutAGenericCardIntoHand
            | S::MayPutAGenericCardIntoHand
            | S::PutNumberGenericCardsIntoHand(_)
            | S::PutUptoNumberGenericCardsIntoHand(_)
            | S::PutAnyNumberOfGenericCardsIntoHand => {
                let n = match action {
                    S::PutNumberGenericCardsIntoHand(n)
                    | S::PutUptoNumberGenericCardsIntoHand(n) => self.number(n)?,
                    _ => one(),
                };
                let c = if matches!(action, S::PutAnyNumberOfGenericCardsIntoHand) {
                    Count::AnyNumber
                } else {
                    count(
                        n,
                        matches!(
                            action,
                            S::MayPutAGenericCardIntoHand | S::PutUptoNumberGenericCardsIntoHand(_)
                        ),
                    )
                };
                choice = Some((
                    ObjectFilter::Any,
                    c,
                    false,
                    Some((Destination::OwnersHand, default_entry, false)),
                    None,
                ));
            }
            S::MayRevealACardOfTypeAndPutIntoHand(f)
            | S::MayRevealAnyNumberOfCardsOfTypeAndPutThemIntoHand(f) => {
                choice = Some((
                    self.card_filter(f)?,
                    if matches!(
                        action,
                        S::MayRevealAnyNumberOfCardsOfTypeAndPutThemIntoHand(_)
                    ) {
                        Count::AnyNumber
                    } else {
                        Count::UpTo(one())
                    },
                    true,
                    Some((Destination::OwnersHand, default_entry, false)),
                    None,
                ));
            }
            S::MayRevealUptoNumberCardsOfTypeAndPutIntoHand(n, f) => {
                choice = Some((
                    self.library_filter(f)?,
                    Count::UpTo(self.number(n)?),
                    true,
                    Some((Destination::OwnersHand, default_entry, false)),
                    None,
                ))
            }
            S::RevealACardOfType(f) => {
                choice = Some((self.card_filter(f)?, count(one(), false), true, None, None))
            }
            S::MayPutACardOfTypeOntoTheBattlefield(f, flags)
            | S::MayRevealAndPutACardOfTypeOntoTheBattlefield(f, flags)
            | S::MayPutUptoNumberCardsOntoTheBattlefield(_, f, flags)
            | S::MayPutAnyNumberOfCardsOntoTheBattlefield(f, flags)
            | S::PutAnyNumberOfCardsOntoTheBattlefield(f, flags)
            | S::MayPutUptoNumberGroupCardsOntoTheBattlefield(_, f, _, flags)
            | S::MayPutAnyNumberOfGroupCardsOntoTheBattlefield(f, _, flags) => {
                let (destination, entry) = self.move_entry(flags, player.clone())?;
                let c = match action {
                    S::MayPutUptoNumberCardsOntoTheBattlefield(n, ..)
                    | S::MayPutUptoNumberGroupCardsOntoTheBattlefield(n, ..) => {
                        Count::UpTo(self.number(n)?)
                    }
                    S::MayPutAnyNumberOfCardsOntoTheBattlefield(..)
                    | S::PutAnyNumberOfCardsOntoTheBattlefield(..)
                    | S::MayPutAnyNumberOfGroupCardsOntoTheBattlefield(..) => Count::AnyNumber,
                    _ => Count::UpTo(one()),
                };
                choice = Some((
                    self.card_filter(f)?,
                    c,
                    matches!(action, S::MayRevealAndPutACardOfTypeOntoTheBattlefield(..)),
                    Some((destination, entry, false)),
                    None,
                ));
            }
            S::PutAGenericCardIntoGraveyard
            | S::ExileAGenericCard
            | S::ExileAGenericCardFaceDown
            | S::MayExileAGenericCard
            | S::ExileAnyNumberOfGenericCards
            | S::MayExileAnyNumberOfGenericCards
            | S::ExileNumberGenericCardsFaceDown(_) => {
                let destination = if matches!(action, S::PutAGenericCardIntoGraveyard) {
                    Destination::OwnersGraveyard
                } else {
                    Destination::Exile
                };
                let c = match action {
                    S::ExileAnyNumberOfGenericCards | S::MayExileAnyNumberOfGenericCards => {
                        Count::AnyNumber
                    }
                    S::ExileNumberGenericCardsFaceDown(n) => count(self.number(n)?, false),
                    _ => count(one(), matches!(action, S::MayExileAGenericCard)),
                };
                choice = Some((
                    ObjectFilter::Any,
                    c,
                    false,
                    Some((
                        destination,
                        default_entry,
                        matches!(
                            action,
                            S::ExileAGenericCardFaceDown | S::ExileNumberGenericCardsFaceDown(_)
                        ),
                    )),
                    None,
                ));
            }
            S::MayExileACardOfType(f) | S::MayExileUptoNumberCardsOfType(_, f) => {
                choice = Some((
                    self.card_filter(f)?,
                    Count::UpTo(if let S::MayExileUptoNumberCardsOfType(n, _) = action {
                        self.number(n)?
                    } else {
                        one()
                    }),
                    false,
                    Some((Destination::Exile, default_entry, false)),
                    None,
                ))
            }
            S::PutAGenericCardOnTopOfLibrary
            | S::PutAGenericCardOnBottomOfLibrary
            | S::PutAnyNumberOfGenericCardsOnBottomOfLibraryAnyOrder
            | S::PutUptoNumberGenericCardsOnTopOfLibraryInAnyOrder(_) => {
                let c = match action {
                    S::PutAnyNumberOfGenericCardsOnBottomOfLibraryAnyOrder => Count::AnyNumber,
                    S::PutUptoNumberGenericCardsOnTopOfLibraryInAnyOrder(n) => {
                        Count::UpTo(self.number(n)?)
                    }
                    _ => count(one(), false),
                };
                let end = if matches!(
                    action,
                    S::PutAGenericCardOnBottomOfLibrary
                        | S::PutAnyNumberOfGenericCardsOnBottomOfLibraryAnyOrder
                ) {
                    SequenceEnd::Bottom
                } else {
                    SequenceEnd::Top
                };
                choice = Some((
                    ObjectFilter::Any,
                    c,
                    false,
                    None,
                    Some((end, CardOrder::Chosen)),
                ));
            }
            S::MayRevealACardOfTypeAndPutOnTopOfLibrary(f)
            | S::MayRevealAnyNumberOfCardOfTypeAndPutOnTopOfLibrary(f)
            | S::MayRevealAnyNumberOfCardsOfTypeAndPutOnTopOfLibraryInAnyOrder(f) => {
                let c = if matches!(action, S::MayRevealACardOfTypeAndPutOnTopOfLibrary(_)) {
                    Count::UpTo(one())
                } else {
                    Count::AnyNumber
                };
                choice = Some((
                    self.card_filter(f)?,
                    c,
                    true,
                    None,
                    Some((SequenceEnd::Top, CardOrder::Chosen)),
                ));
            }
            _ => {}
        }
        if let Some((filter, c, reveal, movement, order)) = choice {
            let mut constraints = vec![];
            if let Some((destination, entry, face_down)) = &movement {
                constraints.push(SelectionConstraint::CanMoveTo {
                    destination: destination.clone(),
                    entry: entry.clone(),
                    face_down: *face_down,
                });
            }
            if let S::MayPutUptoNumberGroupCardsOntoTheBattlefield(_, _, group, _)
            | S::MayPutAnyNumberOfGroupCardsOntoTheBattlefield(_, group, _) = action
            {
                constraints.extend(self.group_constraints(group)?);
            }
            let selected =
                self.inspection_choice(player, remaining, filter, c, constraints, instructions);
            state.found = Some(selected.clone());
            if reveal {
                instructions.push(self.effect(Operation::Reveal {
                    objects: selected.clone(),
                }));
            }
            if let Some((destination, entry, face_down)) = movement {
                self.inspection_assignment(
                    Operation::Move {
                        objects: selected,
                        destination,
                        entry,
                        face_down,
                    },
                    state,
                    instructions,
                );
            } else if let Some((end, order)) = order {
                self.inspection_assignment(
                    Operation::ReorderLibrary {
                        player: player.clone(),
                        library: library.clone(),
                        objects: selected,
                        end,
                        order,
                    },
                    state,
                    instructions,
                );
            }
            return Ok(());
        }
        if optional {
            let mut body = vec![];
            return match self.sibling_step(action, player, &mut body) {
                Some(step) => {
                    step?;
                    instructions.push(Instruction::Optional {
                        decider: player.clone(),
                        body: Program { instructions: body },
                    });
                    Ok(())
                }
                None => Err(unsupported(action)),
            };
        }
        let operation = match action {
            S::PutTheRemainingCardsIntoGraveyard
            | S::PutTheRemainingCardsIntoHand
            | S::PutRemainingCardsInHand
            | S::ExileTheRemainingCards
            | S::ExileTheRemainingCardsFaceDown => {
                let destination = match action {
                    S::PutTheRemainingCardsIntoGraveyard => Destination::OwnersGraveyard,
                    S::ExileTheRemainingCards | S::ExileTheRemainingCardsFaceDown => {
                        Destination::Exile
                    }
                    _ => Destination::OwnersHand,
                };
                Operation::Move {
                    objects: remaining,
                    destination,
                    entry: EntrySpec::default(),
                    face_down: matches!(action, S::ExileTheRemainingCardsFaceDown),
                }
            }
            S::PutTheRemainingCardsOnTopOfLibraryInAnyOrder
            | S::PutTheRemainingCardsOnTheBottomOfLibraryInARandomOrder
            | S::PutTheRemainingCardsOnTheBottomOfLibraryInAnyOrder => Operation::ReorderLibrary {
                player: player.clone(),
                library: library.clone(),
                objects: remaining,
                end: if matches!(action, S::PutTheRemainingCardsOnTopOfLibraryInAnyOrder) {
                    SequenceEnd::Top
                } else {
                    SequenceEnd::Bottom
                },
                order: if matches!(
                    action,
                    S::PutTheRemainingCardsOnTheBottomOfLibraryInARandomOrder
                ) {
                    CardOrder::Random
                } else {
                    CardOrder::Chosen
                },
            },
            S::PutTheRemainingCardsBackIntoLibraryAndShuffle => Operation::Shuffle {
                players: PlayerSet::One(library.clone()),
                excluding: None,
            },
            // A repeated look names the cards by where they are, and the shuffle
            // moves them from there, so they are noted first.
            S::ShuffleAndPutTheRemainingCardsOnTopOfLibraryInAnyOrder
                if matches!(state.observed, ObjectSet::Take { .. }) =>
            {
                let note = self.effect(Operation::NoteObjects { objects: remaining });
                let Instruction::Effect { result, .. } = &note else {
                    unreachable!()
                };
                let noted = ObjectSet::Selected(result.clone());
                instructions.push(note);
                instructions.push(self.effect(Operation::Shuffle {
                    players: PlayerSet::One(library.clone()),
                    excluding: Some(noted.clone()),
                }));
                Operation::ReorderLibrary {
                    player: player.clone(),
                    library: library.clone(),
                    objects: noted,
                    end: SequenceEnd::Top,
                    order: CardOrder::Chosen,
                }
            }
            S::ShuffleAndPutTheRemainingCardsOnTopOfLibraryInAnyOrder => {
                instructions.push(self.effect(Operation::Shuffle {
                    players: PlayerSet::One(library.clone()),
                    excluding: Some(remaining.clone()),
                }));
                Operation::ReorderLibrary {
                    player: player.clone(),
                    library: library.clone(),
                    objects: remaining,
                    end: SequenceEnd::Top,
                    order: CardOrder::Chosen,
                }
            }
            S::LeaveRemainingCardsOnTopOfLibraryInSameOrder => return Ok(()),
            S::PutFoundCardIntoHand
            | S::PutFoundCardOntoBattlefield(_)
            | S::MayPutFoundCardOntoBattlefield(_) => {
                let mut objects = state
                    .found
                    .clone()
                    .ok_or_else(|| invalid("inspection has no preceding found-card choice"))?;
                let (destination, entry) = match action {
                    S::PutFoundCardOntoBattlefield(flags)
                    | S::MayPutFoundCardOntoBattlefield(flags) => {
                        self.move_entry(flags, player.clone())?
                    }
                    _ => (Destination::OwnersHand, EntrySpec::default()),
                };
                if matches!(action, S::MayPutFoundCardOntoBattlefield(_)) {
                    objects = self.inspection_choice(
                        player,
                        objects,
                        ObjectFilter::Any,
                        Count::UpTo(one()),
                        vec![SelectionConstraint::CanMoveTo {
                            destination: destination.clone(),
                            entry: entry.clone(),
                            face_down: false,
                        }],
                        instructions,
                    );
                }
                Operation::Move {
                    objects,
                    destination,
                    entry,
                    face_down: false,
                }
            }
            S::GainLife(n) => Operation::GainLife {
                players: PlayerSet::One(player.clone()),
                amount: self.number(n)?,
            },
            _ => {
                if let Some(step) = self.sibling_step(action, player, instructions) {
                    return step;
                }
                let (name, mut operands) = self
                    .primitive("LookAtTopOfLibraryAction", action)
                    .ok_or_else(|| unsupported(action))?;
                operands.insert(0, Operand::Objects(remaining));
                crate::semantics::reduce_inspection(PlayerSet::One(player.clone()), name, operands)
            }
        };
        if matches!(
            operation,
            Operation::Move { .. } | Operation::ReorderLibrary { .. }
        ) {
            self.inspection_assignment(operation, state, instructions);
        } else {
            let to_hand = crate::semantics::yields(&operation) == Some("hand");
            let instruction = self.effect(operation);
            if let (true, Instruction::Effect { result, .. }) = (to_hand, &instruction) {
                self.to_hand = Some(result.clone());
            }
            instructions.push(instruction);
        }
        Ok(())
    }
}
