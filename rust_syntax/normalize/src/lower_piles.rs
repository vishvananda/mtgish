impl Context {
    /// "Separate into two piles" names what is split and who splits it; the
    /// choice of a pile finishes that one action, as Forge's `TwoPiles` does.
    fn pile_action(
        &mut self,
        value: &source::Action,
        actor: &PlayerSet,
    ) -> Option<Result<Instruction>> {
        use source::Action as S;
        let separator = |context: &Self| match actor {
            PlayerSet::One(player) => player.clone(),
            set @ PlayerSet::OpponentsOf(_) => PlayerRef::OneOf(Box::new(set.clone())),
            _ => context.acting.clone().unwrap_or(PlayerRef::AbilityController),
        };
        let objects = match value {
            S::SeparatePermanentsIntoNumberPiles(objects, count) => {
                if !matches!(&**count, source::GameNumber::Integer(2)) {
                    return None;
                }
                self.objects(objects)
            }
            S::SeparateExiledCardsIntoTwoPiles(cards) => self.exile_cards(cards),
            S::SeparateCardsInPlayersGraveyardIntoTwoPiles(cards, player) => {
                (|| {
                    Ok(self.private_cards(
                        EventZone::Graveyard,
                        PlayerSet::One(self.player(player)?),
                        self.card_filter(cards)?,
                    ))
                })()
            }
            S::ChooseAPermanentPile | S::ChooseAnExiledPile | S::ChooseAGraveyardPile => {
                let (splitter, objects) = self.pile.take()?;
                let piles = self.effect(Operation::SeparatePiles {
                    separator: splitter,
                    chooser: separator(self),
                    objects,
                });
                let Instruction::Effect { result, .. } = &piles else {
                    unreachable!()
                };
                self.chosen_pile = Some(result.clone());
                return Some(Ok(piles));
            }
            _ => return None,
        };
        let by = separator(self);
        Some(objects.map(|objects| {
            self.pile = Some((by, objects));
            Instruction::Group(Program {
                instructions: vec![],
            })
        }))
    }

    fn pile_chosen(&self) -> Result<ResultId> {
        self.chosen_pile
            .clone()
            .ok_or_else(|| invalid("no pile was chosen in this scope"))
    }

    /// What the last conjure or draft row made.
    fn conjured_object(&self) -> Result<ObjectRef> {
        self.result_object(self.conjured.as_ref(), 0)
    }

    fn conjured_cards(&self) -> Result<ObjectSet> {
        self.conjured
            .clone()
            .map(|result| ObjectSet::Transition {
                result,
                phase: ObjectPhase::After,
            })
            .ok_or_else(|| invalid("nothing was conjured in this scope"))
    }
}
