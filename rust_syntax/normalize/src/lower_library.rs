impl Context {
    fn library_filter(&self, value: &source::CardsInLibrary) -> Result<ObjectFilter> {
        use source::CardsInLibrary as S;
        let card = match value {
            S::AnyCardInLibrary => return Ok(ObjectFilter::Any),
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
            S::IsArtifactType(v) => source::Cards::IsArtifactType(v.clone()),
            S::IsCardtype(v) => source::Cards::IsCardtype(v.clone()),
            S::IsColor(v) => source::Cards::IsColor(v.clone()),
            S::IsColored => source::Cards::IsColored,
            S::IsColorless => source::Cards::IsColorless,
            S::IsCreatureType(v) => source::Cards::IsCreatureType(v.clone()),
            S::IsEnchantmentType(v) => source::Cards::IsEnchantmentType(v.clone()),
            S::IsLandType(v) => source::Cards::IsLandType(v.clone()),
            S::IsNamed(v) => source::Cards::IsNamed(v.clone()),
            S::IsNotNamed(v) => source::Cards::IsNotNamed(v.clone()),
            S::IsNonCardtype(v) => source::Cards::IsNonCardtype(v.clone()),
            S::IsNonCreatureType(v) => source::Cards::IsNonCreatureType(v.clone()),
            S::IsNonSupertype(v) => source::Cards::IsNonSupertype(v.clone()),
            S::IsNumberColors(v) => source::Cards::IsNumberColors(v.clone()),
            S::IsPermanent => source::Cards::IsPermanent,
            S::IsMulticolored => source::Cards::IsMulticolored,
            S::IsSpellType(v) => source::Cards::IsSpellType(v.clone()),
            S::IsSupertype(v) => source::Cards::IsSupertype(v.clone()),
            S::ManaValueIs(v) => source::Cards::ManaValueIs(v.clone()),
            S::PowerIs(v) => source::Cards::PowerIs(v.clone()),
            S::ToughnessIs(v) => source::Cards::ToughnessIs(v.clone()),
            S::HasAbility(v) => source::Cards::HasAbility(v.clone()),
            _ => return Err(unsupported(value)),
        };
        self.card_filter(&card)
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
        let search = self.effect(Operation::Search {
            player: player.clone(),
            library: library.clone(),
            top,
            choices,
            also,
            either,
        });
        let Instruction::Effect { result, .. } = &search else {
            unreachable!()
        };
        let result = result.clone();
        let found = ObjectSet::Selected(result.clone());
        let mut instructions = vec![search];
        let mut aside = None;
        for (i, action) in actions.iter().enumerate().skip(start) {
            use source::SearchLibraryAction as S;
            let operation = match action {
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
                },
                S::PutFoundCardsOntoBattlefield(flags) => {
                    let (destination, entry) = self.move_entry(flags, player.clone())?;
                    Operation::Move {
                        objects: found.clone(),
                        destination,
                        entry,
                        face_down: false,
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
                    }
                }
                S::SetAsideFoundCards => {
                    aside = Some(found.clone());
                    continue;
                }
                S::Shuffle | S::ShuffleLibraryIfSearched => {
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
                S::DontShuffle => continue,
                S::PutSetAsideCardsOnTopOfLibrary => Operation::ReorderLibrary {
                    player: library.clone(),
                    library: library.clone(),
                    objects: aside
                        .clone()
                        .ok_or_else(|| invalid("set-aside reference has no declaration"))?,
                    end: SequenceEnd::Top,
                    order: CardOrder::Chosen,
                },
                S::PutFoundCardsOnBottomOfLibrary => Operation::ReorderLibrary {
                    player: library.clone(),
                    library: library.clone(),
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
                    }
                }
                S::DiscardACardAtRandom => Operation::Discard {
                    players: PlayerSet::One(player.clone()),
                    amount: Expr::Constant(1),
                    random: true,
                },
                S::GainLife(n) => Operation::GainLife {
                    players: PlayerSet::One(player.clone()),
                    amount: self.number(n)?,
                },
                _ => return Err(unsupported(action).at(format!("search[{i}]"))),
            };
            instructions.push(self.effect(operation));
        }
        Ok(Instruction::Group(Program { instructions }))
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
            player: player.clone(),
            library: player.clone(),
            top: None,
            choices: vec![choice],
            also: vec![],
            either: false,
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
