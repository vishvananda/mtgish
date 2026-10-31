/// Reveal steps that name an inspection step differently but mean the same thing.
const REVEAL_ALIASES: [(&str, &str); 13] = [
    ("PutAPileIntoHand", "PutFoundCardIntoHand"),
    ("PutTheChosenPileIntoHand", "PutFoundCardIntoHand"),
    ("PutTheChosenCardIntoHand", "PutFoundCardIntoHand"),
    ("PutTheChosenCardsIntoHand", "PutFoundCardIntoHand"),
    ("PutTheChosenCardOntoTheBattlefield", "PutFoundCardOntoBattlefield"),
    ("PutTheCardFoundThisWayIntoHand", "PutFoundCardIntoHand"),
    ("PutTheCardFoundThisWayOntoTheBattlefield", "PutFoundCardOntoBattlefield"),
    ("PutTheCardsFoundThisWayIntoHand", "PutFoundCardIntoHand"),
    ("PutTheCardsFoundThisWayOntoTheBattlefield", "PutFoundCardOntoBattlefield"),
    ("MayPutTheCardFoundThisWayOntoTheBattlefield", "MayPutFoundCardOntoBattlefield"),
    ("PutTheRemainingCardsIntoHand", "PutRemainingCardsInHand"),
    ("LeaveTheRemainingCardsOnTopOfLibraryInTheSameOrder", "LeaveRemainingCardsOnTopOfLibraryInSameOrder"),
    ("MayPutMultipleCardsOfTypeIntoHand", "MayRevealMultipleCardsOfTypeAndPutIntoHand"),
];

fn look_step(
    action: &source::RevealTheTopNumberCardsOfLibraryAction,
) -> Option<source::LookAtTopOfLibraryAction> {
    let mut inspected = crate::inspect::value(action).ok()?;
    let variant = inspected["variant"].as_str()?;
    let variant = REVEAL_ALIASES
        .iter()
        .find(|(reveal, _)| *reveal == variant)
        .map_or(variant, |(_, look)| look)
        .to_string();
    inspected["variant"] = variant.into();
    inspected["enum"] = "LookAtTopOfLibraryAction".into();
    crate::pipeline::source_tree::decode(&inspected).ok()
}

impl Context {
    /// "An opponent": a player of this set, picked by the controller.
    fn one_of(&self, players: &source::Players) -> Result<PlayerRef> {
        Ok(PlayerRef::OneOf(Box::new(self.players(players)?)))
    }
    /// The chooser's pile is the found set; the other is what remains.
    fn reveal_piles(
        &mut self,
        separator: PlayerRef,
        chooser: PlayerRef,
        state: &mut InspectionState,
        instructions: &mut Vec<Instruction>,
    ) {
        let piles = self.effect(Operation::SeparatePiles {
            separator,
            chooser,
            objects: state.remaining(),
        });
        let Instruction::Effect { result, .. } = &piles else {
            unreachable!()
        };
        state.found = Some(ObjectSet::Selected(result.clone()));
        instructions.push(piles);
    }
    fn reveal_move(
        &mut self,
        objects: ObjectSet,
        destination: Destination,
        state: &mut InspectionState,
        instructions: &mut Vec<Instruction>,
    ) {
        let operation = Operation::Move {
            objects,
            destination,
            entry: EntrySpec::default(),
            face_down: false,
        };
        self.inspection_assignment(operation, state, instructions);
    }
    /// A revealed cohort is an inspected cohort every player saw, so reveal steps
    /// share inspection's scopes for the found and remaining cards.
    fn reveal_library(
        &mut self,
        player: PlayerRef,
        reveal: Operation,
        found: Option<ObjectFilter>,
        actions: &[source::RevealTheTopNumberCardsOfLibraryAction],
    ) -> Result<Instruction> {
        self.inspection_hand = Some(vec![]);
        let reveal = self.effect(reveal);
        let Instruction::Effect { result, .. } = &reveal else {
            unreachable!()
        };
        let observed = ObjectSet::Observed(result.clone());
        self.revealed = Some(observed.clone());
        let mut state = InspectionState {
            found: found.map(|filter| ObjectSet::Matching {
                objects: Box::new(observed.clone()),
                filter,
            }),
            observed,
            assigned: vec![],
            did: None,
        };
        let mut instructions = vec![reveal];
        self.reveal_steps(&player, &player, state, actions, &mut instructions)?;
        Ok(Instruction::Group(Program { instructions }))
    }

    /// A branch carries the steps after it, so each arm sees what the other moved.
    fn reveal_steps(
        &mut self,
        player: &PlayerRef,
        library: &PlayerRef,
        mut state: InspectionState,
        actions: &[source::RevealTheTopNumberCardsOfLibraryAction],
        instructions: &mut Vec<Instruction>,
    ) -> Result<()> {
        use source::RevealTheTopNumberCardsOfLibraryAction as S;
        for (i, action) in actions.iter().enumerate() {
            if let S::If(condition, then) | S::IfElse(condition, then, _) = action {
                if state.found.is_some() {
                    self.found = state.found.clone();
                }
                let condition = self.condition(condition)?;
                let mut yes = then.clone();
                yes.extend_from_slice(&actions[i + 1..]);
                let mut no = if let S::IfElse(_, _, otherwise) = action {
                    otherwise.clone()
                } else {
                    vec![]
                };
                no.extend_from_slice(&actions[i + 1..]);
                let mut then = vec![];
                let mut yes_context = self.clone();
                yes_context.reveal_steps(player, library, state.clone(), &yes, &mut then)?;
                self.result = yes_context.result;
                let mut otherwise = vec![];
                let mut no_context = self.clone();
                no_context.reveal_steps(player, library, state, &no, &mut otherwise)?;
                self.result = no_context.result;
                self.invalidate_child_bindings(&yes_context);
                self.invalidate_child_bindings(&no_context);
                instructions.push(Instruction::If {
                    condition,
                    then: Program { instructions: then },
                    otherwise: Program {
                        instructions: otherwise,
                    },
                });
                return Ok(());
            }
            self.reveal_step(player, library, &mut state, action, instructions)
                .map_err(|e| e.at(format!("reveal[{i}]")))?;
        }
        Ok(())
    }

    fn reveal_step(
        &mut self,
        player: &PlayerRef,
        library: &PlayerRef,
        state: &mut InspectionState,
        action: &source::RevealTheTopNumberCardsOfLibraryAction,
        instructions: &mut Vec<Instruction>,
    ) -> Result<()> {
        use source::RevealTheTopNumberCardsOfLibraryAction as S;
        let one = || Expr::Constant(1);
        let found = |state: &InspectionState| {
            state
                .found
                .clone()
                .ok_or_else(|| invalid("inspection has no preceding found-card choice"))
        };
        match action {
            S::PlayerAction(who, step) => {
                let who = self.player(who)?;
                return self.reveal_step(&who, library, state, step, instructions);
            }
            S::ChooseAnyNumberOfCards(cards) => {
                let filter = self.card_filter(cards)?;
                state.found = Some(self.inspection_choice(
                    player,
                    state.remaining(),
                    filter,
                    Count::AnyNumber,
                    vec![],
                    instructions,
                ));
                return Ok(());
            }
            S::ChooseMultipleCardsOfType(cards) => {
                let filters = cards
                    .iter()
                    .map(|cards| self.card_filter(cards))
                    .collect::<Result<Vec<_>>>()?;
                state.found = Some(self.inspection_choice(
                    player,
                    state.remaining(),
                    ObjectFilter::Any,
                    Count::UpTo(Expr::Constant(filters.len() as i64)),
                    vec![SelectionConstraint::OneForEach(filters)],
                    instructions,
                ));
                return Ok(());
            }
            S::PutTheCardsFoundThisWayIntoGraveyard => {
                self.reveal_move(found(state)?, Destination::OwnersGraveyard, state, instructions);
                return Ok(());
            }
            S::PutTheCardsNotFoundThisWayIntoGraveyard => {
                let objects = ObjectSet::Difference {
                    objects: Box::new(state.remaining()),
                    excluding: Box::new(found(state)?),
                };
                self.reveal_move(objects, Destination::OwnersGraveyard, state, instructions);
                return Ok(());
            }
            S::PutTheCardFoundThisWayOnTopOfLibrary => {
                let operation = Operation::ReorderLibrary {
                    player: player.clone(),
                    library: library.clone(),
                    objects: found(state)?,
                    end: SequenceEnd::Top,
                    order: CardOrder::Chosen,
                };
                self.inspection_assignment(operation, state, instructions);
                return Ok(());
            }
            S::ExileTheCardFoundThisWayWithNumberCountersOfType(amount, kind) => {
                let operation = Operation::Move {
                    objects: found(state)?,
                    destination: Destination::Exile,
                    entry: EntrySpec {
                        counters: vec![(crate::counter::lower(kind), self.number(amount)?)],
                        ..EntrySpec::default()
                    },
                    face_down: false,
                };
                self.inspection_assignment(operation, state, instructions);
                return Ok(());
            }
            S::PutEachCardOfTypeChosenThisWayOntoTheBattlefield(cards, flags) => {
                let objects = ObjectSet::Matching {
                    objects: Box::new(found(state)?),
                    filter: self.card_filter(cards)?,
                };
                let (destination, entry) = self.move_entry(flags, player.clone())?;
                let operation = Operation::Move {
                    objects,
                    destination,
                    entry,
                    face_down: false,
                };
                self.inspection_assignment(operation, state, instructions);
                return Ok(());
            }
            // One of the chosen cards goes first; the rest stay chosen for the next step.
            S::PutAChosenCardOntoTheBattlefield(flags)
            | S::PutACardFoundThisWayOntoTheBattlefield(flags) => {
                let chosen = found(state)?;
                let (destination, entry) = self.move_entry(flags, player.clone())?;
                let objects = self.inspection_choice(
                    player,
                    chosen.clone(),
                    ObjectFilter::Any,
                    Count::AsManyAsCan(one()),
                    vec![SelectionConstraint::CanMoveTo {
                        destination: destination.clone(),
                        entry: entry.clone(),
                        face_down: false,
                    }],
                    instructions,
                );
                state.found = Some(ObjectSet::Difference {
                    objects: Box::new(chosen),
                    excluding: Box::new(objects.clone()),
                });
                let operation = Operation::Move {
                    objects,
                    destination,
                    entry,
                    face_down: false,
                };
                self.inspection_assignment(operation, state, instructions);
                return Ok(());
            }
            S::PutAGenericCardFromTheChosenPileIntoHand => {
                let pile = found(state)?;
                let objects = self.inspection_choice(
                    player,
                    pile,
                    ObjectFilter::Any,
                    Count::AsManyAsCan(one()),
                    vec![],
                    instructions,
                );
                self.reveal_move(objects, Destination::OwnersHand, state, instructions);
                return Ok(());
            }
            S::PlayerMillsNumberCards(who, amount) => {
                let players = PlayerSet::One(self.player(who)?);
                let amount = self.number(amount)?;
                instructions.push(self.effect(Operation::Mill { players, amount }));
                return Ok(());
            }
            S::SeperateThoseCardsIntoTwoPiles => return Ok(()),
            S::APlayerSeparatesThoseCardsIntoTwoPiles(players) => {
                let separator = self.one_of(players)?;
                self.reveal_piles(separator, player.clone(), state, instructions);
                return Ok(());
            }
            S::APlayerChoosesAPile(players) => {
                let chooser = self.one_of(players)?;
                self.reveal_piles(player.clone(), chooser, state, instructions);
                return Ok(());
            }
            S::APlayerChoosesAGenericCard(players) => {
                let chooser = self.one_of(players)?;
                state.found = Some(self.inspection_choice(
                    &chooser,
                    state.remaining(),
                    ObjectFilter::Any,
                    Count::AsManyAsCan(one()),
                    vec![],
                    instructions,
                ));
                return Ok(());
            }
            S::APlayerChoosesACardOfType(players, cards) => {
                let chooser = self.one_of(players)?;
                let filter = self.card_filter(cards)?;
                state.found = Some(self.inspection_choice(
                    &chooser,
                    state.remaining(),
                    filter,
                    Count::AsManyAsCan(one()),
                    vec![],
                    instructions,
                ));
                return Ok(());
            }
            S::APlayerPutsAGenericCardIntoGraveyard(players)
            | S::APlayerPutsNumberGenericCardsIntoGraveyard(players, _)
            | S::APlayerExilesACardOfType(players, _) => {
                let chooser = self.one_of(players)?;
                let amount = match action {
                    S::APlayerPutsNumberGenericCardsIntoGraveyard(_, n) => self.number(n)?,
                    _ => one(),
                };
                let (filter, destination) = match action {
                    S::APlayerExilesACardOfType(_, cards) => {
                        (self.card_filter(cards)?, Destination::Exile)
                    }
                    _ => (ObjectFilter::Any, Destination::OwnersGraveyard),
                };
                let objects = self.inspection_choice(
                    &chooser,
                    state.remaining(),
                    filter,
                    Count::AsManyAsCan(amount),
                    vec![],
                    instructions,
                );
                self.reveal_move(objects, destination, state, instructions);
                return Ok(());
            }
            S::PutTheChosenCardIntoGraveyard => {
                let objects = state
                    .found
                    .clone()
                    .ok_or_else(|| invalid("inspection has no preceding found-card choice"))?;
                self.reveal_move(objects, Destination::OwnersGraveyard, state, instructions);
                return Ok(());
            }
            S::MayPutMultipleCardsOfTypeIntoHand(cards) => {
                let filters = cards
                    .iter()
                    .map(|cards| self.card_filter(cards))
                    .collect::<Result<Vec<_>>>()?;
                let objects = self.inspection_choice(
                    player,
                    state.remaining(),
                    ObjectFilter::Any,
                    Count::UpTo(Expr::Constant(filters.len() as i64)),
                    vec![SelectionConstraint::OneForEach(filters)],
                    instructions,
                );
                self.reveal_move(objects, Destination::OwnersHand, state, instructions);
                return Ok(());
            }
            _ => {}
        }
        let (filter, count, to_battlefield) = match action {
            S::PutEachCardOfTypeIntoHand(f) => (f, None, None),
            S::PutEachCardOfTypeOntoTheBattlefield(f, flags) => (f, None, Some(flags)),
            S::PutACardOfTypeIntoHand(f) => (f, Some(Count::AsManyAsCan(one())), None),
            S::MayPutACardOfTypeIntoHand(f) => (f, Some(Count::UpTo(one())), None),
            S::MayPutAnyNumberOfCardsOfTypeIntoHand(f) => (f, Some(Count::AnyNumber), None),
            S::PutUptoNumberCardsOfTypeIntoHand(n, f) => {
                (f, Some(Count::UpTo(self.number(n)?)), None)
            }
            S::PutACardOfTypeOntoTheBattlefield(f, flags) => {
                (f, Some(Count::AsManyAsCan(one())), Some(flags))
            }
            S::MayPutAnyNumberOfCardsOfTypeOntoTheBattlefield(f, flags) => {
                (f, Some(Count::AnyNumber), Some(flags))
            }
            S::MayCastTheCardFoundThisWayWithoutPaying => {
                let cards = state
                    .found
                    .clone()
                    .ok_or_else(|| invalid("inspection has no preceding found-card choice"))?;
                instructions.push(self.effect(Operation::PlayCards {
                    actor: PlayerSet::One(player.clone()),
                    action: CardPlayAction::CastSpell,
                    cards,
                    count: Count::UpTo(one()),
                    cost: PlayCost::Free,
                    into_exile: false,
                }));
                return Ok(());
            }
            S::ExileTheCardFoundThisWay => {
                let objects = state
                    .found
                    .clone()
                    .ok_or_else(|| invalid("inspection has no preceding found-card choice"))?;
                let operation = Operation::Move {
                    objects,
                    destination: Destination::Exile,
                    entry: EntrySpec::default(),
                    face_down: false,
                };
                self.inspection_assignment(operation, state, instructions);
                return Ok(());
            }
            _ => {
                let Some(step) = look_step(action) else {
                    if let Some(step) = self.sibling_step(action, player, instructions) {
                        return step;
                    }
                    let objects = state.remaining();
                    instructions.push(self.primitive_step(
                        "RevealTheTopNumberCardsOfLibraryAction",
                        action,
                        player,
                        objects,
                    )?);
                    return Ok(());
                };
                return self.inspection_step(player, library, state, &step, false, instructions);
            }
        };
        let filter = self.card_filter(filter)?;
        let (destination, entry) = match to_battlefield {
            Some(flags) => self.move_entry(flags, player.clone())?,
            None => (Destination::OwnersHand, EntrySpec::default()),
        };
        let objects = match count {
            Some(count) => self.inspection_choice(
                player,
                state.remaining(),
                filter,
                count,
                vec![SelectionConstraint::CanMoveTo {
                    destination: destination.clone(),
                    entry: entry.clone(),
                    face_down: false,
                }],
                instructions,
            ),
            None => ObjectSet::Matching {
                objects: Box::new(state.remaining()),
                filter,
            },
        };
        let operation = Operation::Move {
            objects,
            destination,
            entry,
            face_down: false,
        };
        self.inspection_assignment(operation, state, instructions);
        Ok(())
    }
}
