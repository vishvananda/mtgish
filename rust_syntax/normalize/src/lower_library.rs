impl Context {
    fn library_filter(&self, value: &source::CardsInLibrary) -> Result<ObjectFilter> {
        use source::CardsInLibrary as S;
        match value {
            S::AnyCardInLibrary => return Ok(ObjectFilter::Any),
            S::SharesANameWithPermanent(p) => {
                return Ok(ObjectFilter::SharesNameWith(self.object(p)?))
            }
            S::SharesANameWithSpell(s) => return Ok(ObjectFilter::SharesNameWith(self.spell(s)?)),
            S::TheCardsConjuredInLibraryThisWay => {
                return Ok(ObjectFilter::InSet(Box::new(self.conjured_cards()?)))
            }
            S::SharesANameWithDeadPermanent => {
                return Ok(ObjectFilter::SharesNameWith(self.dead_object()?))
            }
            S::SharesANameWithExiled(c) => {
                return Ok(ObjectFilter::SharesNameWith(self.exiled_card(c)?))
            }
            S::SharesANameWithGraveyardCard(c) => {
                return Ok(ObjectFilter::SharesNameWith(self.grave_card(c)?))
            }
            S::SharesANameWithTheCardRevealedThisWay => {
                return Ok(ObjectFilter::SharesNameWithAny(Box::new(
                    self.revealed_objects()?,
                )))
            }
            S::SharesANameWithTheCardChosenThisWay => {
                let chosen = self.hand_choice.as_ref().or(self.permanent_choice.as_ref());
                return Ok(ObjectFilter::SharesNameWith(self.result_object(chosen, 0)?));
            }
            S::RevealedFromLibraryThisWay => {
                return Ok(ObjectFilter::InSet(Box::new(self.revealed_objects()?)))
            }
            S::RevealedFromPlayersLibraryThisWay(player) => {
                return Ok(ObjectFilter::All(vec![
                    ObjectFilter::InSet(Box::new(self.revealed_objects()?)),
                    ObjectFilter::OwnedBy(PlayerSet::One(self.player(player)?)),
                ]))
            }
            S::SharesANameWithAnExiled(c) => {
                return Ok(ObjectFilter::SharesNameWithAny(Box::new(self.exile_cards(c)?)))
            }
            S::And(v) => {
                return Ok(ObjectFilter::All(
                    v.iter()
                        .map(|f| self.library_filter(f))
                        .collect::<Result<_>>()?,
                ))
            }
            S::Or(v) => {
                return Ok(ObjectFilter::Either(
                    v.iter()
                        .map(|f| self.library_filter(f))
                        .collect::<Result<_>>()?,
                ))
            }
            S::Not(f) => return Ok(ObjectFilter::Not(Box::new(self.library_filter(f)?))),
            S::IsMonocolored => {
                return Ok(ObjectFilter::Property {
                    property: Property::ColorCount,
                    comparison: Comparison::Equal,
                    value: Expr::Constant(1),
                })
            }
            S::HasBasicLandType => {
                return self.filter(&source::Permanents::Or(vec![
                    source::Permanents::IsLandType(source::LandType::Plains),
                    source::Permanents::IsLandType(source::LandType::Island),
                    source::Permanents::IsLandType(source::LandType::Swamp),
                    source::Permanents::IsLandType(source::LandType::Mountain),
                    source::Permanents::IsLandType(source::LandType::Forest),
                ]))
            }
            _ => self
                .as_card_filter(value, unsupported(value))
                .or_else(|error| self.rescue(error, || self.primitive_predicate("CardsInLibrary", value))),
        }
    }
    fn search_find(
        &self,
        value: &source::SearchLibraryAction,
    ) -> Result<Option<Vec<SearchChoice>>> {
        use source::SearchLibraryAction as S;
        let typed = |filter: &source::CardsInLibrary, count: Count| -> Result<SearchChoice> {
            let count = match count {
                Count::UpTo(n) if matches!(filter, source::CardsInLibrary::AnyCardInLibrary) => {
                    Count::AsManyAsCan(n)
                }
                c => c,
            };
            Ok(SearchChoice {
                filter: self.library_filter(filter)?,
                count,
                constraints: vec![],
            })
        };
        Ok(Some(match value {
            S::FindAGenericCard => vec![SearchChoice {
                filter: ObjectFilter::Any,
                count: Count::AsManyAsCan(Expr::Constant(1)),
                constraints: vec![],
            }],
            S::FindNumberGenericCards(n) => vec![SearchChoice {
                filter: ObjectFilter::Any,
                count: Count::AsManyAsCan(self.number(n)?),
                constraints: vec![],
            }],
            S::FindUptoNumberGenericCards(n) => vec![SearchChoice {
                filter: ObjectFilter::Any,
                count: Count::UpTo(self.number(n)?),
                constraints: vec![],
            }],
            S::FindACardOfType(f) => vec![typed(f, Count::UpTo(Expr::Constant(1)))?],
            // The game picks, so no one can fail to find a card that is there.
            S::FindACardOfTypeAtRandom(f) => vec![SearchChoice {
                filter: self.library_filter(f)?,
                count: Count::AsManyAsCan(Expr::Constant(1)),
                constraints: vec![SelectionConstraint::AtRandom],
            }],
            S::FindExactlyNumberGroupCards(n, f, g) => vec![SearchChoice {
                filter: self.library_filter(f)?,
                count: Count::Exactly(self.number(n)?),
                constraints: self.group_constraints(g)?,
            }],
            S::FindNumberCardsOfType(n, f) => vec![typed(f, Count::UpTo(self.number(n)?))?],
            S::FindUptoNumberCardsOfType(n, f) => vec![SearchChoice {
                filter: self.library_filter(f)?,
                count: Count::UpTo(self.number(n)?),
                constraints: vec![],
            }],
            S::FindAnyNumberOfCardsOfType(f) | S::FindAllCardsOfType(f) => {
                vec![typed(f, Count::AnyNumber)?]
            }
            S::FindCardsOfType(filters) => filters
                .iter()
                .map(|f| {
                    Ok(SearchChoice {
                        filter: self.card_filter(f)?,
                        count: if matches!(f, source::Cards::AnyCard) {
                            Count::AsManyAsCan(Expr::Constant(1))
                        } else {
                            Count::UpTo(Expr::Constant(1))
                        },
                        constraints: vec![],
                    })
                })
                .collect::<Result<_>>()?,
            S::FindAnyNumberOfGroupCards(f, g) => vec![SearchChoice {
                filter: self.library_filter(f)?,
                count: Count::AnyNumber,
                constraints: self.group_constraints(g)?,
            }],
            S::FindNumberGroupCards(n, f, g) | S::FindUptoNumberGroupCards(n, f, g) => {
                let constraints = self.group_constraints(g)?;
                vec![SearchChoice {
                    filter: self.library_filter(f)?,
                    count: Count::UpTo(self.number(n)?),
                    constraints,
                }]
            }
            _ => return Ok(None),
        }))
    }
    fn search_library(
        &mut self,
        player: PlayerRef,
        library: PlayerRef,
        top: Option<Expr>,
        actions: &[source::SearchLibraryAction],
    ) -> Result<Instruction> {
        if self
            .open_searches
            .iter()
            .any(|p| crate::semantics::same_player(p, &library))
        {
            return Err(invalid(
                "separated searches before one shuffle require a shared search episode",
            ));
        }
        let mut choices = Vec::new();
        let mut start = 0;
        for (i, action) in actions.iter().enumerate() {
            match self
                .search_find(action)
                .map_err(|e| e.at(format!("search[{i}]")))?
            {
                Some(found) => {
                    choices.extend(found);
                    start += 1;
                }
                None => break,
            }
        }
        let also = std::mem::take(&mut self.search_also);
        let either = std::mem::take(&mut self.search_either);
        let outside = self.search_outside;
        // Both cards that search outside the game say "you own" (400.11a).
        if outside {
            for choice in &mut choices {
                choice.filter = ObjectFilter::All(vec![
                    choice.filter.clone(),
                    ObjectFilter::OwnedBy(PlayerSet::One(player.clone())),
                ]);
            }
        }
        let search = self.effect(Operation::Search {
            searcher: player.clone(),
            library_owner: library.clone(),
            top,
            choices,
            also,
            either,
            outside,
        });
        let Instruction::Effect { result, .. } = &search else {
            unreachable!()
        };
        let result = result.clone();
        let mut instructions = vec![search];
        let mut aside = None;
        self.search_steps(
            &actions[start..],
            &player,
            &library,
            &result,
            &mut aside,
            &mut instructions,
        )?;
        self.search_outside = false;
        Ok(Instruction::Group(Program { instructions }))
    }

    /// What happens to the cards a search found, in order.
    fn search_steps(
        &mut self,
        actions: &[source::SearchLibraryAction],
        player: &PlayerRef,
        library: &PlayerRef,
        result: &ResultId,
        aside: &mut Option<ObjectSet>,
        instructions: &mut Vec<Instruction>,
    ) -> Result<()> {
        let mut found = ObjectSet::Selected(result.clone());
        let mut picked = None;
        self.found = Some(found.clone());
        for (i, action) in actions.iter().enumerate() {
            use source::SearchLibraryAction as S;
            let operation = match action {
                S::APlayerChoosesNumberFoundCards(players, _)
                | S::APlayerChoosesAFoundCard(players) => {
                    let count = match action {
                        S::APlayerChoosesNumberFoundCards(_, n) => self.number(n)?,
                        _ => Expr::Constant(1),
                    };
                    let player = match self.players(players)? {
                        PlayerSet::One(player) => player,
                        players => PlayerRef::OneOf(Box::new(players)),
                    };
                    let choice = self.effect(Operation::ChooseObjects {
                        constraints: vec![],
                        player,
                        objects: found.clone(),
                        count: Count::AsManyAsCan(count),
                    });
                    let Instruction::Effect { result: chosen, .. } = &choice else {
                        unreachable!()
                    };
                    picked = Some(chosen.clone());
                    instructions.push(choice);
                    continue;
                }
                S::ChooseAFoundCardAtRandom => {
                    let choice = self.effect(Operation::ChooseObjects {
                        constraints: vec![SelectionConstraint::AtRandom],
                        player: player.clone(),
                        objects: found.clone(),
                        count: Count::AsManyAsCan(Expr::Constant(1)),
                    });
                    let Instruction::Effect { result: chosen, .. } = &choice else {
                        unreachable!()
                    };
                    picked = Some(chosen.clone());
                    instructions.push(choice);
                    continue;
                }
                S::PutChosenFoundCardsIntoGraveyard
                | S::PutChosenFoundCardIntoHand
                | S::PutChosenFoundCardsIntoHand => {
                    let chosen = picked
                        .clone()
                        .ok_or_else(|| invalid("no found card was chosen"))?;
                    // "The rest" are what the later steps name.
                    found = ObjectSet::Unselected(chosen.clone());
                    self.found = Some(found.clone());
                    Operation::Move {
                        objects: ObjectSet::Selected(chosen),
                        destination: match action {
                            S::PutChosenFoundCardsIntoGraveyard => Destination::OwnersGraveyard,
                            _ => Destination::OwnersHand,
                        },
                        entry: EntrySpec::default(),
                        face_down: false,
                        terms: crate::ir::MovementTerms::default(),
                    }
                }
                S::If(condition, then) | S::IfElse(condition, then, _) => {
                    let condition = self.condition(condition)?;
                    let otherwise = match action {
                        S::IfElse(_, _, otherwise) => otherwise.as_slice(),
                        _ => &[],
                    };
                    let mut set_aside = aside.clone();
                    let mut other_aside = aside.clone();
                    let branch = self.conditional(
                        condition,
                        |context| {
                            let mut body = vec![];
                            context.search_steps(
                                then,
                                player,
                                library,
                                result,
                                &mut set_aside,
                                &mut body,
                            )?;
                            Ok(Program { instructions: body })
                        },
                        |context| {
                            let mut body = vec![];
                            context.search_steps(
                                otherwise,
                                player,
                                library,
                                result,
                                &mut other_aside,
                                &mut body,
                            )?;
                            Ok(Program { instructions: body })
                        },
                    )?;
                    instructions.push(branch);
                    continue;
                }
                S::APlayerChooseACardExiledThisWay(players) => {
                    let chooser = self.one_of(players)?;
                    let choice = self.effect(Operation::ChooseObjects {
                        constraints: vec![],
                        player: chooser,
                        objects: self.exile_cards(&source::CardsInExile::TheCardsExiledThisWay)?,
                        count: Count::AsManyAsCan(Expr::Constant(1)),
                    });
                    if let Instruction::Effect { result, .. } = &choice {
                        self.exile_choice = Some(result.clone());
                    }
                    instructions.push(choice);
                    continue;
                }
                S::MayCost(cost) => {
                    let cost: source::Cost =
                        as_family(cost, "Cost").ok_or_else(|| unsupported(cost))?;
                    let payer = self.payer.replace(player.clone());
                    let cost = self.cost(&cost);
                    self.payer = payer;
                    Operation::PayCost {
                        cost: cost?,
                        player: player.clone(),
                        optional: true,
                    }
                }
                S::ValueActions(cases) => {
                    let value = self.number(&source::GameNumber::TheDiceResult)?;
                    for source::SearchLibraryActionValueAction::ValueAction(range, steps) in cases {
                        let mut set_aside = aside.clone();
                        let branch = self.conditional(
                            game_range(&value, range),
                            |context| {
                                let mut body = vec![];
                                context.search_steps(
                                    steps,
                                    player,
                                    library,
                                    result,
                                    &mut set_aside,
                                    &mut body,
                                )?;
                                Ok(Program { instructions: body })
                            },
                            |_| Ok(Program::empty()),
                        )?;
                        instructions.push(branch);
                    }
                    continue;
                }
                S::RevealFoundCards => Operation::Reveal {
                    objects: found.clone(),
                },
                S::PutFoundCardsIntoHand
                | S::PutFoundCardsIntoGraveyard
                | S::ExileFoundCards
                | S::ExileFoundCardsFaceDown => Operation::Move {
                    objects: found.clone(),
                    destination: match action {
                        S::PutFoundCardsIntoHand => Destination::OwnersHand,
                        S::PutFoundCardsIntoGraveyard => Destination::OwnersGraveyard,
                        _ => Destination::Exile,
                    },
                    entry: EntrySpec::default(),
                    face_down: matches!(action, S::ExileFoundCardsFaceDown),
                    terms: crate::ir::MovementTerms::default(),
                },
                S::PutFoundCardsOntoBattlefield(flags) => {
                    let (destination, entry) = self.move_entry(flags, player.clone())?;
                    Operation::Move {
                        objects: found.clone(),
                        destination,
                        entry,
                        face_down: false,
                        terms: crate::ir::MovementTerms::default(),
                    }
                }
                S::PutAFoundCardIntoHand | S::PutAFoundCardOntoBattlefield(_) => {
                    let choice = self.effect(Operation::ChooseObjects {
                        constraints: vec![],
                        player: player.clone(),
                        objects: found.clone(),
                        count: Count::AsManyAsCan(Expr::Constant(1)),
                    });
                    let Instruction::Effect { result: chosen, .. } = &choice else {
                        unreachable!()
                    };
                    let objects = ObjectSet::Selected(chosen.clone());
                    let (destination, entry) = match action {
                        S::PutAFoundCardOntoBattlefield(flags) => {
                            self.move_entry(flags, player.clone())?
                        }
                        _ => (Destination::OwnersHand, EntrySpec::default()),
                    };
                    instructions.push(choice);
                    Operation::Move {
                        objects,
                        destination,
                        entry,
                        face_down: false,
                        terms: crate::ir::MovementTerms::default(),
                    }
                }
                S::SetAsideFoundCards => {
                    *aside = Some(found.clone());
                    continue;
                }
                S::SetAsideNonchosenFoundCards => {
                    let chosen = picked
                        .clone()
                        .ok_or_else(|| invalid("no found card was chosen"))?;
                    *aside = Some(ObjectSet::Unselected(chosen));
                    continue;
                }
                S::Shuffle | S::ShuffleLibraryIfSearched => {
                    // "Shuffle the rest into your library": those found outside
                    // the game are not there yet.
                    if self.search_outside && picked.is_some() {
                        let arrive = self.effect(Operation::Move {
                            objects: ObjectSet::Difference {
                                objects: Box::new(found.clone()),
                                excluding: Box::new(ObjectSet::InZone {
                                    objects: Box::new(found.clone()),
                                    zone: EventZone::Library,
                                }),
                            },
                            destination: Destination::OwnersLibraryTop,
                            entry: EntrySpec::default(),
                            face_down: false,
                            terms: crate::ir::MovementTerms::default(),
                        });
                        instructions.push(arrive);
                    }
                    let shuffle = self.effect(Operation::Shuffle {
                        players: PlayerSet::One(library.clone()),
                        excluding: aside.clone(),
                    });
                    if matches!(action, S::ShuffleLibraryIfSearched) {
                        instructions.push(Instruction::If {
                            condition: Condition::Searched(result.clone()),
                            then: Program {
                                instructions: vec![shuffle],
                            },
                            otherwise: Program::empty(),
                        });
                    } else {
                        instructions.push(shuffle);
                    }
                    continue;
                }
                S::DontShuffle => {
                    self.open_searches
                        .retain(|p| !crate::semantics::same_player(p, library));
                    continue;
                }
                S::PutSetAsideCardsOnTopOfLibrary => Operation::ReorderLibrary {
                    chooser: library.clone(),
                    library_owner: library.clone(),
                    objects: aside
                        .clone()
                        .ok_or_else(|| invalid("set-aside reference has no declaration"))?,
                    end: SequenceEnd::Top,
                    order: CardOrder::Chosen,
                },
                S::PutFoundCardsOnBottomOfLibrary => Operation::ReorderLibrary {
                    chooser: library.clone(),
                    library_owner: library.clone(),
                    objects: found.clone(),
                    end: SequenceEnd::Bottom,
                    order: CardOrder::Chosen,
                },
                S::PutSetAsideCardsIntoHand => Operation::Move {
                    objects: aside
                        .clone()
                        .ok_or_else(|| invalid("set-aside reference has no declaration"))?,
                    destination: Destination::OwnersHand,
                    entry: EntrySpec::default(),
                    face_down: false,
                    terms: crate::ir::MovementTerms::default(),
                },
                S::PutSetAsideCardsOntoBattlefield(flags) => {
                    let (destination, entry) = self.move_entry(flags, player.clone())?;
                    Operation::Move {
                        objects: aside
                            .clone()
                            .ok_or_else(|| invalid("set-aside reference has no declaration"))?,
                        destination,
                        entry,
                        face_down: false,
                        terms: crate::ir::MovementTerms::default(),
                    }
                }
                S::DiscardACardAtRandom => Operation::Discard {
                    players: PlayerSet::One(player.clone()),
                    amount: Expr::Constant(1),
                    random: true,
                },
                S::MayPlayFoundCardsWithoutPaying => Operation::PlayCards {
                    actor: PlayerSet::One(player.clone()),
                    action: CardPlayAction::Either,
                    cards: found.clone(),
                    count: Count::UpTo(Expr::Constant(1)),
                    cost: PlayCost::Free,
                    into_exile: false,
                },
                S::GainLife(n) => Operation::GainLife {
                    players: PlayerSet::One(player.clone()),
                    amount: self.number(n)?,
                },
                _ => {
                    match self.sibling_step(action, player, instructions) {
                        Some(step) => step.map_err(|e| e.at(format!("search[{i}]")))?,
                        None => instructions.push(
                            self.primitive_step(
                                "SearchLibraryAction",
                                action,
                                player,
                                ObjectSet::Selected(result.clone()),
                            )
                            .map_err(|e| e.at(format!("search[{i}]")))?,
                        ),
                    }
                    continue;
                }
            };
            let step = self.effect(operation);
            if let (S::ExileFoundCards | S::ExileFoundCardsFaceDown, Instruction::Effect { result, .. }) =
                (action, &step)
            {
                self.exiled = Some(result.clone());
                self.exiled_permanents = false;
            }
            instructions.push(step);
        }
        Ok(())
    }
    fn typecycling(&mut self, filter: ObjectFilter, mut cost: Cost) -> Result<Ability> {
        cost.parts.push(CostPart::Discard {
            count: Expr::Constant(1),
            filter: ObjectFilter::SameObject(ObjectRef::Source),
            random: false,
        });
        let player = PlayerRef::AbilityController;
        let choice = SearchChoice {
            filter: filter.clone(),
            count: Count::UpTo(Expr::Constant(1)),
            constraints: vec![],
        };
        let search = self.effect(Operation::Search {
            searcher: player.clone(),
            library_owner: player.clone(),
            top: None,
            choices: vec![choice],
            also: vec![],
            either: false,
            outside: false,
        });
        let Instruction::Effect { result, .. } = &search else {
            unreachable!()
        };
        let found = ObjectSet::Selected(result.clone());
        let steps = Program {
            instructions: vec![
                search,
                self.effect(Operation::Reveal {
                    objects: found.clone(),
                }),
                self.effect(Operation::Move {
                    objects: found,
                    destination: Destination::OwnersHand,
                    entry: EntrySpec::default(),
                    face_down: false,
                    terms: crate::ir::MovementTerms::default(),
                }),
                self.effect(Operation::Shuffle {
                    players: PlayerSet::One(player),
                    excluding: None,
                }),
            ],
        };
        let body = Program {
            instructions: vec![Instruction::Group(steps)],
        };
        Ok(Ability::Scoped {
            zones: vec![EventZone::Hand],
            condition: None,
            ability: Box::new(Ability::Activated {
                origin: ActivationOrigin::Cycling {
                    search: Some(filter),
                },
                cost,
                limits: ActivationLimits::unrestricted(),
                announcement: Announcement::empty(),
                body,
            }),
        })
    }
}
