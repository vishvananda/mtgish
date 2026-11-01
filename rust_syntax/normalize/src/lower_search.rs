// Searches, reveals and hand steps that need results of their own, so they lower
// here rather than reduce from a row.
impl Context {
    /// `None` when this action is not one of these.
    fn search_action(
        &mut self,
        value: &source::Action,
        actor: &PlayerSet,
    ) -> Option<Result<Instruction>> {
        use source::Action as S;
        match value {
            S::ChooseACardFromAmongTheTopNumberCardsInPlayersGraveyard(..)
            | S::DigitallySearchLibrary(_)
            | S::SearchLibraryAndOrOutsideTheGame(_)
            | S::SearchLibraryAndOrGraveyardAndOrOutsideTheGame(_)
            | S::ForEachPlayerSearchLibrary(..)
            | S::RevealLibrary(_)
            | S::EachPlayerRevealsCardsFromTheTopOfTheirLibraryUntilTheyRevealACardOfType(..)
            | S::RevealANumberOfCardsFromHandAndPlayerMayCastASpellFromAmongThemWithoutPaying(..)
            | S::RevealHandAndAlternateExilingCardsWithPlayer(_)
            | S::ReflexiveTriggerNEW(_)
            | S::Reflexive_Draw_EachPlayerMayDraw_WhenACardIsDrawnThisWay(..) => {}
            _ => return None,
        }
        Some(self.lower_search_action(value, actor))
    }

    fn lower_search_action(
        &mut self,
        value: &source::Action,
        actor: &PlayerSet,
    ) -> Result<Instruction> {
        use source::Action as S;
        if let S::ReflexiveTriggerNEW(actions) = value {
            return self.reflexive_on(None::<ResultId>, None, actions);
        }
        let PlayerSet::One(player) = actor else {
            return Err(invalid("this action needs one player"));
        };
        match value {
            S::ChooseACardFromAmongTheTopNumberCardsInPlayersGraveyard(cards, amount, owner) => {
                let filter = self.card_filter(cards)?;
                let owner = self.player(owner)?;
                let top = ObjectSet::Take {
                    objects: Box::new(self.private_cards(
                        EventZone::Graveyard,
                        PlayerSet::One(owner),
                        ObjectFilter::Any,
                    )),
                    count: self.number(amount)?,
                    from: SequenceEnd::Top,
                };
                let choice = self.effect(Operation::ChooseObjects {
                    player: player.clone(),
                    objects: ObjectSet::Matching {
                        objects: Box::new(top),
                        filter,
                    },
                    count: Count::AsManyAsCan(Expr::Constant(1)),
                    constraints: vec![],
                });
                if let Instruction::Effect { result, .. } = &choice {
                    self.note_grave_choice(result.clone());
                }
                Ok(choice)
            }
            S::DigitallySearchLibrary(actions) => {
                self.search_library(player.clone(), player.clone(), None, actions)
            }
            S::SearchLibraryAndOrOutsideTheGame(actions)
            | S::SearchLibraryAndOrGraveyardAndOrOutsideTheGame(actions) => {
                self.search_either = true;
                self.search_outside = true;
                if matches!(value, S::SearchLibraryAndOrGraveyardAndOrOutsideTheGame(_)) {
                    self.search_also = vec![EventZone::Graveyard];
                }
                self.search_library(player.clone(), player.clone(), None, actions)
            }
            S::ForEachPlayerSearchLibrary(players, actions) => {
                let players = self.players(players)?;
                self.for_each_player(players, |context, library| {
                    let PlayerSet::One(library) = library else {
                        return Err(invalid("each search is of one library"));
                    };
                    context.search_library(player.clone(), library.clone(), None, actions)
                })
            }
            S::RevealLibrary(actions) => {
                let objects = self.private_cards(
                    EventZone::Library,
                    PlayerSet::One(player.clone()),
                    ObjectFilter::Any,
                );
                self.reveal_library(player.clone(), Operation::Reveal { objects }, None, actions)
            }
            S::EachPlayerRevealsCardsFromTheTopOfTheirLibraryUntilTheyRevealACardOfType(
                players,
                cards,
                actions,
            ) => self.each_reveals_until(player, players, cards, actions),
            S::RevealANumberOfCardsFromHandAndPlayerMayCastASpellFromAmongThemWithoutPaying(
                amount,
                caster,
                spells,
            ) => self.cast_from_offered(player, amount, caster, spells),
            S::RevealHandAndAlternateExilingCardsWithPlayer(other) => {
                self.exile_hand_in_turns(player, other)
            }
            S::Reflexive_Draw_EachPlayerMayDraw_WhenACardIsDrawnThisWay(
                first,
                others,
                second,
                actions,
            ) => self.draws_each_reflexive(actor, first, others, second, actions),
            _ => Err(unsupported(value)),
        }
    }

    /// Every player reveals until a match, then one set of steps reads all of it.
    fn each_reveals_until(
        &mut self,
        player: &PlayerRef,
        players: &source::Players,
        cards: &source::Cards,
        actions: &[source::RevealTheTopNumberCardsOfLibraryAction],
    ) -> Result<Instruction> {
        let filter = self.card_filter(cards)?;
        let players = self.players(players)?;
        self.inspection_hand = Some(vec![]);
        let mut revealed = None;
        let each = self.for_each_player(players, |context, library| {
            let PlayerSet::One(library) = library else {
                return Err(invalid("each reveal is of one library"));
            };
            let reveal = context.effect(Operation::RevealUntil {
                library: library.clone(),
                filter: filter.clone(),
                count: Expr::Constant(1),
                at_most: None,
            });
            if let Instruction::Effect { result, .. } = &reveal {
                revealed = Some(result.clone());
            }
            Ok(reveal)
        })?;
        let revealed = revealed.ok_or_else(|| invalid("nothing was revealed"))?;
        let observed = ObjectSet::Observed(revealed);
        self.revealed = Some(observed.clone());
        let state = InspectionState {
            found: Some(ObjectSet::Matching {
                objects: Box::new(observed.clone()),
                filter,
            }),
            observed,
            assigned: vec![],
            did: None,
            top: None,
        };
        let mut instructions = vec![each];
        self.reveal_steps(player, player, state, actions, &mut instructions)?;
        Ok(Instruction::Group(Program { instructions }))
    }

    /// "Target opponent chooses X cards from their hand. Look at those cards. You
    /// may cast a spell from among them without paying its mana cost."
    fn cast_from_offered(
        &mut self,
        holder: &PlayerRef,
        amount: &source::GameNumber,
        caster: &source::Player,
        spells: &source::Spells,
    ) -> Result<Instruction> {
        let amount = self.number(amount)?;
        let caster = self.player(caster)?;
        let filter = self.spell_filter(spells)?;
        let hand = self.private_cards(
            EventZone::Hand,
            PlayerSet::One(holder.clone()),
            ObjectFilter::Any,
        );
        let mut instructions = vec![];
        let offered = self.inspection_choice(
            holder,
            hand,
            ObjectFilter::Any,
            Count::AsManyAsCan(amount),
            vec![],
            &mut instructions,
        );
        instructions.push(self.effect(Operation::LookAt {
            player: caster.clone(),
            objects: offered.clone(),
        }));
        instructions.push(self.effect(Operation::PlayCards {
            actor: PlayerSet::One(caster),
            action: CardPlayAction::CastSpell,
            cards: ObjectSet::Matching {
                objects: Box::new(offered),
                filter,
            },
            count: Count::UpTo(Expr::Constant(1)),
            cost: PlayCost::Free,
            into_exile: false,
        }));
        Ok(Instruction::Group(Program { instructions }))
    }

    /// The holder and another player exile a card from a revealed hand in turn
    /// until it is empty. Each exile is one result, so what each player exiled
    /// stays readable afterward.
    fn exile_hand_in_turns(
        &mut self,
        holder: &PlayerRef,
        other: &source::Player,
    ) -> Result<Instruction> {
        let other = self.player(other)?;
        let hand = self.private_cards(
            EventZone::Hand,
            PlayerSet::One(holder.clone()),
            ObjectFilter::Any,
        );
        let reveal = self.effect(Operation::Reveal {
            objects: hand.clone(),
        });
        let mut child = self.clone();
        let mut body = vec![];
        let mut turns = vec![];
        for player in [holder.clone(), other] {
            let picked = child.inspection_choice(
                &player,
                hand.clone(),
                ObjectFilter::Any,
                Count::AsManyAsCan(Expr::Constant(1)),
                vec![],
                &mut body,
            );
            let exile = child.effect(Operation::Move {
                objects: picked,
                destination: Destination::Exile,
                entry: EntrySpec::default(),
                face_down: false,
            });
            if let Instruction::Effect { result, .. } = &exile {
                turns.push((player, result.clone()));
            }
            body.push(exile);
        }
        let again = child.effect(Operation::RepeatProcess);
        body.push(Instruction::If {
            condition: Condition::Compare {
                left: Expr::CountObjects(Box::new(hand)),
                comparison: Comparison::GreaterOrEqual,
                right: Expr::Constant(1),
            },
            then: Program {
                instructions: vec![again],
            },
            otherwise: Program::empty(),
        });
        self.result = child.result;
        self.invalidate_child_bindings(&child);
        self.adopt_loop(&child);
        self.exile_turns = turns;
        Ok(Instruction::Group(Program {
            instructions: vec![reveal, Instruction::Repeatable(Program { instructions: body })],
        }))
    }

    /// "Whenever a card is drawn this way": each draw of one card has its own
    /// reflexive trigger, which waits for the resolution to end (603.12).
    fn draws_each_reflexive(
        &mut self,
        actor: &PlayerSet,
        first: &source::Drawable,
        others: &source::Players,
        second: &source::Drawable,
        actions: &source::Actions,
    ) -> Result<Instruction> {
        use source::Drawable::ACard;
        if !matches!((first, second), (ACard, ACard)) {
            return Err(invalid("a trigger for each card needs one card per draw"));
        }
        let one = || Expr::Constant(1);
        let others = self.players(others)?;
        let draw = self.effect(Operation::Draw {
            players: actor.clone(),
            amount: one(),
        });
        let Instruction::Effect { result, .. } = &draw else {
            unreachable!()
        };
        let drawn = self.reflexive_on(result.clone(), None, actions)?;
        let each = self.for_each_player(others, |context, drawer| {
            let PlayerSet::One(decider) = drawer else {
                return Err(invalid("each draw is one player's"));
            };
            let draw = context.effect(Operation::Draw {
                players: drawer.clone(),
                amount: one(),
            });
            let Instruction::Effect { result, .. } = &draw else {
                unreachable!()
            };
            let drawn = context.reflexive_on(result.clone(), None, actions)?;
            Ok(Instruction::Optional {
                decider: decider.clone(),
                body: Program {
                    instructions: vec![draw, drawn],
                },
            })
        })?;
        Ok(Instruction::Group(Program {
            instructions: vec![draw, drawn, each],
        }))
    }

    /// `PayCost` for each of the remaining cards; the unpaid ones then move
    /// together. "Any opponent pays" is the payer `FirstWilling`.
    fn each_unless_paid(
        &mut self,
        payer: PlayerRef,
        cost: Cost,
        destination: Destination,
        state: &mut InspectionState,
        instructions: &mut Vec<Instruction>,
    ) -> Result<()> {
        let mut paid = None;
        let each = self.for_each_object(state.remaining(), |context| {
            let payment = context.effect(Operation::PayCost {
                player: payer,
                cost,
                optional: true,
            });
            if let Instruction::Effect { result, .. } = &payment {
                paid = Some(result.clone());
            }
            Ok(Program {
                instructions: vec![payment],
            })
        })?;
        instructions.push(each);
        let paid = paid.ok_or_else(|| invalid("no payment was offered"))?;
        self.reveal_move(ObjectSet::Unpaid(paid), destination, state, instructions);
        Ok(())
    }

    /// `None` when `reveal_step` has this step itself.
    fn reveal_step_more(
        &mut self,
        player: &PlayerRef,
        state: &mut InspectionState,
        action: &source::RevealTheTopNumberCardsOfLibraryAction,
        instructions: &mut Vec<Instruction>,
    ) -> Option<Result<()>> {
        use source::RevealTheTopNumberCardsOfLibraryAction as S;
        match action {
            S::APlayerChoosesMultipleCardsOfType(..)
            | S::ForEachCardPutIntoHandUnlessAnyPlayerAction(..)
            | S::EachPlayerStartingWithChoosesADifferentCardToPutIntoHand(..)
            | S::ForEachColorAmongPermanentsYouMayExileACardOfThatColorFoundThisWay(_)
            | S::MayPutMultipleCardsOfTypeIntoHandOrOntoTheBattlefield(..) => {}
            _ => return None,
        }
        Some(self.lower_reveal_step(player, state, action, instructions))
    }

    fn one_of_each(&self, cards: &[source::Cards]) -> Result<Vec<ObjectFilter>> {
        cards.iter().map(|cards| self.card_filter(cards)).collect()
    }

    fn lower_reveal_step(
        &mut self,
        player: &PlayerRef,
        state: &mut InspectionState,
        action: &source::RevealTheTopNumberCardsOfLibraryAction,
        instructions: &mut Vec<Instruction>,
    ) -> Result<()> {
        use source::RevealTheTopNumberCardsOfLibraryAction as S;
        match action {
            S::APlayerChoosesMultipleCardsOfType(players, cards) => {
                let chooser = self.one_of(players)?;
                let filters = self.one_of_each(cards)?;
                state.found = Some(self.inspection_choice(
                    &chooser,
                    state.remaining(),
                    ObjectFilter::Any,
                    Count::AsManyAsCan(Expr::Constant(filters.len() as i64)),
                    vec![SelectionConstraint::OneForEach(filters)],
                    instructions,
                ));
                Ok(())
            }
            S::ForEachCardPutIntoHandUnlessAnyPlayerAction(players, cost) => {
                use source::RevealTheTopNumberCardsOfLibraryCost as C;
                let C::PayLife(life) = cost;
                let payer = PlayerRef::FirstWilling(Box::new(self.players(players)?));
                let cost = self.cost_for(&source::Cost::PayLife(life.clone()), &payer)?;
                self.each_unless_paid(payer, cost, Destination::OwnersHand, state, instructions)
            }
            S::EachPlayerStartingWithChoosesADifferentCardToPutIntoHand(players, first, cards) => {
                let players = self.players(players)?;
                let first = self.player(first)?;
                let filter = self.card_filter(cards)?;
                let pool = state.remaining();
                let mut picked = None;
                let mut each = self.for_each_player(players, |context, chooser| {
                    let PlayerSet::One(chooser) = chooser else {
                        return Err(invalid("each pick is one player's"));
                    };
                    let mut body = vec![];
                    picked = Some(context.inspection_choice(
                        chooser,
                        pool,
                        filter,
                        Count::AsManyAsCan(Expr::Constant(1)),
                        vec![SelectionConstraint::Unpicked],
                        &mut body,
                    ));
                    Ok(Instruction::Group(Program { instructions: body }))
                })?;
                if let Instruction::ForEachPlayer { starting_with, .. } = &mut each {
                    *starting_with = Some(first);
                }
                instructions.push(each);
                let picked = picked.ok_or_else(|| invalid("no player picked a card"))?;
                state.found = Some(picked.clone());
                self.reveal_move(picked, Destination::OwnersHand, state, instructions);
                Ok(())
            }
            S::ForEachColorAmongPermanentsYouMayExileACardOfThatColorFoundThisWay(objects) => {
                let objects = self.objects(objects)?;
                // A color no permanent has offers no card, so the filter says so.
                let filters: Vec<_> = [
                    ManaColor::White,
                    ManaColor::Blue,
                    ManaColor::Black,
                    ManaColor::Red,
                    ManaColor::Green,
                ]
                .into_iter()
                .map(|color| {
                    let among = Condition::Compare {
                        left: Expr::CountObjects(Box::new(ObjectSet::Matching {
                            objects: Box::new(objects.clone()),
                            filter: ObjectFilter::Color(color.clone()),
                        })),
                        comparison: Comparison::GreaterOrEqual,
                        right: Expr::Constant(1),
                    };
                    ObjectFilter::All(vec![
                        ObjectFilter::Color(color),
                        ObjectFilter::Condition(Box::new(among)),
                    ])
                })
                .collect();
                let picked = self.inspection_choice(
                    player,
                    state.remaining(),
                    ObjectFilter::Any,
                    Count::UpTo(Expr::Constant(filters.len() as i64)),
                    vec![
                        SelectionConstraint::OneForEach(filters),
                        SelectionConstraint::CanMoveTo {
                            destination: Destination::Exile,
                            entry: EntrySpec::default(),
                            face_down: false,
                        },
                    ],
                    instructions,
                );
                self.reveal_move(picked, Destination::Exile, state, instructions);
                Ok(())
            }
            S::MayPutMultipleCardsOfTypeIntoHandOrOntoTheBattlefield(cards, flags) => {
                let filters = self.one_of_each(cards)?;
                let chosen = self.inspection_choice(
                    player,
                    state.remaining(),
                    ObjectFilter::Any,
                    Count::UpTo(Expr::Constant(filters.len() as i64)),
                    vec![SelectionConstraint::OneForEach(filters)],
                    instructions,
                );
                let (destination, entry) = self.move_entry(flags, player.clone())?;
                let entering = self.inspection_choice(
                    player,
                    chosen.clone(),
                    ObjectFilter::Any,
                    Count::AnyNumber,
                    vec![SelectionConstraint::CanMoveTo {
                        destination: destination.clone(),
                        entry: entry.clone(),
                        face_down: false,
                    }],
                    instructions,
                );
                let operation = Operation::Move {
                    objects: entering.clone(),
                    destination,
                    entry,
                    face_down: false,
                };
                self.inspection_assignment(operation, state, instructions);
                let rest = ObjectSet::Difference {
                    objects: Box::new(chosen),
                    excluding: Box::new(entering),
                };
                self.reveal_move(rest, Destination::OwnersHand, state, instructions);
                Ok(())
            }
            _ => Err(unsupported(action)),
        }
    }

    /// `None` when `inspection_step` has this step itself.
    fn inspection_step_more(
        &mut self,
        player: &PlayerRef,
        library: &PlayerRef,
        state: &mut InspectionState,
        action: &source::LookAtTopOfLibraryAction,
        instructions: &mut Vec<Instruction>,
    ) -> Option<Result<()>> {
        use source::LookAtTopOfLibraryAction as S;
        match action {
            S::ExileAGenericCardFaceDownWithACounter(_)
            | S::ForEachCardPutIntoGraveyardUnlessCost(_)
            | S::RepeatableActions(_)
            | S::LookAtTheNextNumberCardsOnTopOfLibrary(_) => {}
            _ => return None,
        }
        Some(self.lower_inspection_step(player, library, state, action, instructions))
    }

    fn lower_inspection_step(
        &mut self,
        player: &PlayerRef,
        library: &PlayerRef,
        state: &mut InspectionState,
        action: &source::LookAtTopOfLibraryAction,
        instructions: &mut Vec<Instruction>,
    ) -> Result<()> {
        use source::LookAtTopOfLibraryAction as S;
        match action {
            S::ExileAGenericCardFaceDownWithACounter(kind) => {
                let entry = EntrySpec {
                    counters: vec![(crate::counter::lower(kind), Expr::Constant(1))],
                    ..EntrySpec::default()
                };
                let objects = self.inspection_choice(
                    player,
                    state.remaining(),
                    ObjectFilter::Any,
                    Count::AsManyAsCan(Expr::Constant(1)),
                    vec![SelectionConstraint::CanMoveTo {
                        destination: Destination::Exile,
                        entry: entry.clone(),
                        face_down: true,
                    }],
                    instructions,
                );
                let operation = Operation::Move {
                    objects,
                    destination: Destination::Exile,
                    entry,
                    face_down: true,
                };
                self.inspection_assignment(operation, state, instructions);
                Ok(())
            }
            S::ForEachCardPutIntoGraveyardUnlessCost(cost) => {
                let cost = inspection_payment(cost).ok_or_else(|| unsupported(cost))?;
                let cost = self.cost_for(&cost, player)?;
                self.each_unless_paid(
                    player.clone(),
                    cost,
                    Destination::OwnersGraveyard,
                    state,
                    instructions,
                )
            }
            // Each run's look is of the library's top cards at that moment, so
            // the steps and what follows the loop name them by where they are.
            S::RepeatableActions(steps) => {
                let amount = state
                    .top
                    .clone()
                    .ok_or_else(|| invalid("a repeated look needs a count of cards"))?;
                let top = self.library_slice(library.clone(), amount, SequenceEnd::Top);
                state.observed = top;
                state.assigned.clear();
                state.found = None;
                state.did = None;
                let mut child = self.clone();
                let body = child.inspection_steps(player, library, state.clone(), steps)?;
                self.result = child.result;
                self.invalidate_child_bindings(&child);
                self.adopt_loop(&child);
                instructions.push(Instruction::Repeatable(body));
                Ok(())
            }
            S::LookAtTheNextNumberCardsOnTopOfLibrary(amount) => {
                let top = self.library_slice(library.clone(), self.number(amount)?, SequenceEnd::Top);
                instructions.push(self.effect(Operation::LookAt {
                    player: player.clone(),
                    objects: top.clone(),
                }));
                state.observed = top;
                state.assigned.clear();
                state.found = None;
                Ok(())
            }
            _ => Err(unsupported(action)),
        }
    }
}
