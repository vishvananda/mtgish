/// What a hand shows before anyone picks from it.
enum HandShown {
    /// The whole hand is revealed.
    Whole,
    /// Only the cards passing the pick's filter are revealed.
    Passing,
    /// Its owner reveals this many cards of their choice.
    Number(Expr),
    /// This player looks at it, and no one else sees it.
    Looked(PlayerRef),
    /// Nothing is shown; its owner picks.
    Hidden,
}

/// What happens to the cards picked from a hand.
enum HandFate {
    Discard,
    Exile(Option<Duration>),
    /// The cards stay; a later reference reads the pick.
    Keep,
}

impl Context {
    /// "Reveal your hand. A player chooses a card from it. Discard that card."
    /// `pick` is who chooses and how many; without it every passing card goes.
    fn hand_pick(
        &mut self,
        owner: PlayerSet,
        shown: HandShown,
        filter: ObjectFilter,
        pick: Option<(PlayerRef, Count)>,
        fate: HandFate,
    ) -> Result<Instruction> {
        let PlayerSet::One(holder) = &owner else {
            return Err(invalid("a hand belongs to one player"));
        };
        let holder = holder.clone();
        let hand = self.private_cards(EventZone::Hand, owner.clone(), ObjectFilter::Any);
        let passing = self.private_cards(EventZone::Hand, owner.clone(), filter.clone());
        let mut instructions = Vec::new();
        let mut objects = passing.clone();
        match shown {
            HandShown::Whole => {
                instructions.push(self.effect(Operation::Reveal { objects: hand }));
            }
            HandShown::Passing => {
                instructions.push(self.effect(Operation::Reveal { objects: passing }));
            }
            HandShown::Number(number) => {
                // Observe the actual reveal, including replacement/prevention,
                // instead of treating an offered selection as revealed history.
                let reveal = self.effect(Operation::RevealFromHand {
                    actor: owner.clone(), cards: ObjectFilter::Any,
                    count: Some(number), at_random: false,
                });
                let Instruction::Effect { result, .. } = &reveal else { unreachable!() };
                let mut filters = vec![ObjectFilter::OwnedBy(owner.clone())];
                if !matches!(filter, ObjectFilter::Any) { filters.push(filter.clone()); }
                objects = ObjectSet::Matching {
                    objects: Box::new(ObjectSet::InZone {
                        objects: Box::new(ObjectSet::Observed(result.clone())),
                        zone: EventZone::Hand,
                    }),
                    filter: if filters.len() == 1 { filters.pop().unwrap() } else { ObjectFilter::AllOf(filters) },
                };
                instructions.push(reveal);
            }
            HandShown::Looked(viewer) => {
                instructions.push(self.effect(Operation::LookAt {
                    player: viewer,
                    objects: hand,
                }));
            }
            HandShown::Hidden => {}
        }
        if let Some((player, count)) = pick {
            let choice = self.effect(Operation::ChooseObjects {
                constraints: vec![],
                player,
                objects,
                count,
            });
            let Instruction::Effect { result, .. } = &choice else {
                unreachable!()
            };
            objects = ObjectSet::Selected(result.clone());
            if matches!(fate, HandFate::Keep) {
                self.hand_choice = Some(result.clone());
            }
            instructions.push(choice);
        }
        match fate {
            HandFate::Keep => {}
            HandFate::Discard => {
                instructions.push(self.effect(Operation::DiscardObjects { objects }));
            }
            HandFate::Exile(period) => {
                let movement =
                    self.effect(crate::movement::move_plain(objects, Destination::Exile));
                self.exiled_permanents = false;
                let Instruction::Effect { result, .. } = &movement else {
                    unreachable!()
                };
                let moved = result.clone();
                instructions.push(movement);
                if let Some(duration) = period {
                    instructions.push(self.effect(Operation::ReturnAfter {
                        objects: ObjectSet::Transition {
                            result: moved,
                            phase: ObjectPhase::After,
                        },
                        duration,
                    }));
                }
            }
        }
        Ok(Instruction::Group(Program { instructions }))
    }

    /// The hand forms: who shows what, who picks, and where the picks go.
    fn hand_action(
        &mut self,
        value: &source::Action,
        actor: &PlayerSet,
    ) -> Result<Option<Instruction>> {
        use source::Action as S;
        let one = |n: Expr| Count::AsManyAsCan(n);
        let single = || Count::AsManyAsCan(Expr::Constant(1));
        let may = || Count::UpTo(Expr::Constant(1));
        let any = ObjectFilter::Any;
        let holder = || match actor {
            PlayerSet::One(player) => Ok(player.clone()),
            _ => Err(invalid("a hand belongs to one player")),
        };
        let (owner, shown, filter, pick, fate) = match value {
            S::DiscardACardOfType(cards) => (
                actor.clone(),
                HandShown::Hidden,
                self.card_filter(cards)?,
                Some((holder()?, single())),
                HandFate::Discard,
            ),
            S::DiscardNumberCardsOfType(number, cards) => (
                actor.clone(),
                HandShown::Hidden,
                self.card_filter(cards)?,
                Some((holder()?, one(self.number(number)?))),
                HandFate::Discard,
            ),
            S::DiscardEachCard(cards) => {
                let Ok(filter) = self.hand_filter(cards) else {
                    return Ok(None);
                };
                (
                    actor.clone(),
                    HandShown::Hidden,
                    filter,
                    None,
                    HandFate::Discard,
                )
            }
            S::SeekACard(cards) => return self.seek(cards, Expr::Constant(1), actor),
            S::SeekNumberCards(number, cards) => {
                let number = self.number(number)?;
                return self.seek(cards, number, actor);
            }
            S::RevealACardOfTypeFromHand(cards) => {
                let filter = self.card_filter(cards)?;
                let passing = self.private_cards(EventZone::Hand, actor.clone(), filter);
                let (choice, result) = self.pick(actor, passing, single(), vec![])?;
                let objects = ObjectSet::Selected(result.clone());
                let reveal = self.effect(Operation::Reveal { objects });
                self.hand_choice = Some(result);
                return Ok(Some(Instruction::Group(Program {
                    instructions: vec![choice, reveal],
                })));
            }
            S::RevealHandAndPlayerChoosesACard(chooser, cards) => (
                actor.clone(),
                HandShown::Whole,
                self.card_filter(cards)?,
                Some((self.player(chooser)?, single())),
                HandFate::Keep,
            ),
            S::RevealAllCardsOfTypeFromHandAndPlayerChoosesCard(cards, chooser) => (
                actor.clone(),
                HandShown::Passing,
                self.card_filter(cards)?,
                Some((self.player(chooser)?, single())),
                HandFate::Keep,
            ),
            S::RevealHandAndPlayerChoosesACardToDiscard(chooser, cards) => (
                actor.clone(),
                HandShown::Whole,
                self.card_filter(cards)?,
                Some((self.player(chooser)?, single())),
                HandFate::Discard,
            ),
            S::RevealHandAndPlayerMayChooseACardToDiscard(chooser, cards) => (
                actor.clone(),
                HandShown::Whole,
                self.card_filter(cards)?,
                Some((self.player(chooser)?, may())),
                HandFate::Discard,
            ),
            S::RevealHandAndPlayerChoosesNumberCardsToDiscard(chooser, number) => (
                actor.clone(),
                HandShown::Whole,
                any,
                Some((self.player(chooser)?, one(self.number(number)?))),
                HandFate::Discard,
            ),
            S::RevealHandAndPlayerChoosesNumberCardsOfTypeToDiscard(chooser, number, cards) => (
                actor.clone(),
                HandShown::Whole,
                self.card_filter(cards)?,
                Some((self.player(chooser)?, one(self.number(number)?))),
                HandFate::Discard,
            ),
            S::RevealANumberOfCardsFromHandAndPlayerChoosesACardToDiscard(number, chooser) => (
                actor.clone(),
                HandShown::Number(self.number(number)?),
                any,
                Some((self.player(chooser)?, single())),
                HandFate::Discard,
            ),
            S::RevealHandAndDiscardEachCard(cards) => (
                actor.clone(),
                HandShown::Whole,
                self.card_filter(cards)?,
                None,
                HandFate::Discard,
            ),
            S::RevealHandAndPlayerChoosesACardToExile(chooser, cards) => (
                actor.clone(),
                HandShown::Whole,
                self.card_filter(cards)?,
                Some((self.player(chooser)?, single())),
                HandFate::Exile(None),
            ),
            S::RevealHandAndPlayerChoosesACardToExileUntil(chooser, cards, period) => (
                actor.clone(),
                HandShown::Whole,
                self.card_filter(cards)?,
                Some((self.player(chooser)?, single())),
                HandFate::Exile(Some(self.duration(period)?)),
            ),
            S::RevealHandAndPlayerMayChooseACardToExile(chooser, cards) => (
                actor.clone(),
                HandShown::Whole,
                self.card_filter(cards)?,
                Some((self.player(chooser)?, may())),
                HandFate::Exile(None),
            ),
            S::RevealHandAndPlayerChoosesNumberCardsToExile(chooser, number, cards) => (
                actor.clone(),
                HandShown::Whole,
                self.card_filter(cards)?,
                Some((self.player(chooser)?, one(self.number(number)?))),
                HandFate::Exile(None),
            ),
            S::RevealANumberOfCardsFromHandAndPlayerChoosesACardToExile(number, chooser, cards)
            | S::RevealNumberCardsFromHandAndPlayerChoosesACardToExile(number, chooser, cards) => (
                actor.clone(),
                HandShown::Number(self.number(number)?),
                self.card_filter(cards)?,
                Some((self.player(chooser)?, single())),
                HandFate::Exile(None),
            ),
            S::RevealAllCardsOfTypeFromHandAndPlayerChoosesACardToExile(cards, chooser) => (
                actor.clone(),
                HandShown::Passing,
                self.card_filter(cards)?,
                Some((self.player(chooser)?, single())),
                HandFate::Exile(None),
            ),
            S::RevealHandAndExileEachCardOfType(cards) => (
                actor.clone(),
                HandShown::Whole,
                self.card_filter(cards)?,
                None,
                HandFate::Exile(None),
            ),
            S::LookAtPlayersHandAndChooseACardToDiscard(owner, cards) => {
                let PlayerSet::One(viewer) = actor else {
                    return Err(invalid("inspection requires one viewer"));
                };
                (
                    PlayerSet::One(self.player(owner)?),
                    HandShown::Looked(viewer.clone()),
                    self.card_filter(cards)?,
                    Some((viewer.clone(), single())),
                    HandFate::Discard,
                )
            }
            S::LookAtPlayersHandAndChooseNumCardsToDiscard(owner, number, cards) => {
                let PlayerSet::One(viewer) = actor else {
                    return Err(invalid("inspection requires one viewer"));
                };
                (
                    PlayerSet::One(self.player(owner)?),
                    HandShown::Looked(viewer.clone()),
                    self.card_filter(cards)?,
                    Some((viewer.clone(), one(self.number(number)?))),
                    HandFate::Discard,
                )
            }
            _ => return Ok(None),
        };
        self.hand_pick(owner, shown, filter, pick, fate).map(Some)
    }

    /// A random passing card, or as many as the number, goes from library to hand.
    fn seek(
        &mut self,
        cards: &source::Cards,
        number: Expr,
        actor: &PlayerSet,
    ) -> Result<Option<Instruction>> {
        let (Ok(filter), PlayerSet::One(seeker)) = (self.card_filter(cards), actor) else {
            return Ok(None);
        };
        let choice = self.effect(Operation::ChooseObjects {
            constraints: vec![SelectionConstraint::AtRandom],
            player: seeker.clone(),
            objects: self.private_cards(EventZone::Library, actor.clone(), filter),
            count: Count::AsManyAsCan(number),
        });
        let Instruction::Effect { result, .. } = &choice else {
            unreachable!()
        };
        let objects = ObjectSet::Selected(result.clone());
        let movement = self.effect(crate::movement::move_plain(
            objects,
            Destination::OwnersHand,
        ));
        let Instruction::Effect { result, .. } = &movement else {
            unreachable!()
        };
        self.to_hand = Some(result.clone());
        self.seeked.push(result.clone());
        Ok(Some(Instruction::Group(Program {
            instructions: vec![choice, movement],
        })))
    }

    /// Every card this scope's seeks put into a hand.
    fn seeked_cards(&self) -> Result<ObjectSet> {
        let mut sets: Vec<_> = self
            .seeked
            .iter()
            .map(|result| ObjectSet::Transition {
                result: result.clone(),
                phase: ObjectPhase::After,
            })
            .collect();
        match sets.len() {
            0 => Err(invalid("no card was sought in this scope")),
            1 => Ok(sets.pop().unwrap()),
            _ => Ok(ObjectSet::Union(sets)),
        }
    }

    /// "Look at that player's hand and exile a card from it" paid as a cost.
    fn hand_cost(&mut self, value: &source::Cost) -> Result<Option<Instruction>> {
        use source::Cost as C;
        let (owner, cards, period) = match value {
            C::LookAtPlayersHandAndChooseACardToExile(owner, cards) => (owner, &**cards, None),
            C::LookAtPlayersHandAndChooseACardToExileUntil(owner, cards, period) => {
                (owner, cards, Some(period))
            }
            _ => return Ok(None),
        };
        let viewer = self.payer.clone().unwrap_or(PlayerRef::AbilityController);
        let period = period.map(|p| self.duration(p)).transpose()?;
        let owner = PlayerSet::One(self.player(owner)?);
        let filter = self.hand_filter(cards)?;
        self.hand_pick(
            owner,
            HandShown::Looked(viewer.clone()),
            filter,
            Some((viewer, Count::AsManyAsCan(Expr::Constant(1)))),
            HandFate::Exile(period),
        )
        .map(Some)
    }
}
