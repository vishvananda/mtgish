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
        };
        let mut instructions = vec![reveal];
        for (i, action) in actions.iter().enumerate() {
            self.reveal_step(&player, &mut state, action, &mut instructions)
                .map_err(|e| e.at(format!("reveal[{i}]")))?;
        }
        Ok(Instruction::Group(Program { instructions }))
    }

    fn reveal_step(
        &mut self,
        player: &PlayerRef,
        state: &mut InspectionState,
        action: &source::RevealTheTopNumberCardsOfLibraryAction,
        instructions: &mut Vec<Instruction>,
    ) -> Result<()> {
        use source::RevealTheTopNumberCardsOfLibraryAction as S;
        let one = || Expr::Constant(1);
        match action {
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
                let library = player.clone();
                return self.inspection_step(player, &library, state, &step, false, instructions);
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
