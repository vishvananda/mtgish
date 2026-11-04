// Binding pass for generated operation templates. Context and scope/result
// checks are shared with the existing compiler; no template parses Oracle text.
impl Context {
    fn pipeline_owners(
        &self,
        owners: &crate::pipeline::Owners,
        actor: &PlayerSet,
    ) -> Result<PlayerSet> {
        use crate::pipeline::Owners as O;
        Ok(match owners {
            O::Actor => actor.clone(),
            O::One(player) => PlayerSet::One(self.player(player)?),
            O::Many(players) => self.players(players)?,
        })
    }
    fn pipeline_subjects(
        &self,
        subjects: &crate::pipeline::Subjects,
        actor: &PlayerSet,
    ) -> Result<ObjectSet> {
        use crate::pipeline::Subjects as S;
        let zone = |zone: &crate::pipeline::CardZone| match zone {
            crate::pipeline::CardZone::Hand => EventZone::Hand,
            crate::pipeline::CardZone::Graveyard => EventZone::Graveyard,
            crate::pipeline::CardZone::Library => EventZone::Library,
        };
        Ok(match subjects {
            S::Permanent(p) => ObjectSet::One(self.object(p)?),
            S::Permanents(p) => self.objects(p)?,
            S::GraveCard(c) => ObjectSet::One(self.grave_card(c)?),
            S::GraveCards(c) => self.grave_cards(c)?,
            S::ExiledCard(c) => self.still_exiled(ObjectSet::One(self.exiled_card(c)?)),
            S::ExiledCards(c) => self.still_exiled(self.exile_cards(c)?),
            S::Spell(s) => ObjectSet::One(self.spell(s)?),
            S::HandCard(c) => ObjectSet::One(self.hand_card(c)?),
            S::HandCards(c) => {
                if self.hand_has_scope(c)? {
                    return Err(unsupported(c));
                }
                self.private_cards(EventZone::Hand, actor.clone(), self.hand_filter(c)?)
            }
            S::LibraryCards(c) => {
                self.private_cards(EventZone::Library, actor.clone(), self.library_filter(c)?)
            }
            S::Cards {
                zone: from,
                filter,
                owners,
            } => self.private_cards(
                zone(from),
                self.pipeline_owners(owners, actor)?,
                self.card_filter(filter)?,
            ),
            S::GraveCardsOf { filter, owners } => self.private_cards(
                EventZone::Graveyard,
                self.pipeline_owners(owners, actor)?,
                self.grave_filter(filter)?,
            ),
            S::MilledCards(c) => self.grave_cards(
                &source::CardsInGraveyards::CardsOfTypeMilledThisWay(Box::new(c.clone())),
            )?,
            S::WholeZone(from) => self.private_cards(zone(from), actor.clone(), ObjectFilter::Any),
        })
    }
    fn pipeline_selection(&self, selection: &crate::pipeline::SelectionInput) -> Result<Count> {
        use crate::pipeline::SelectionInput as S;
        Ok(match selection {
            S::AsManyAsCan(n) => Count::AsManyAsCan(self.pipeline_quantity(n)?),
            S::UpTo(n) => Count::UpTo(self.pipeline_quantity(n)?),
            S::AnyNumber => Count::AnyNumber,
        })
    }
    fn bind_pipeline_recipe(
        &mut self,
        recipe: crate::pipeline::ExpandedRecipe,
        actor: &PlayerSet,
    ) -> Result<Instruction> {
        use crate::pipeline::{
            ChangeTemplate, DestinationInput, DurationInput, ExpandedRecipe as R,
            ObjectOperation as O, PlayerOperation as P,
        };
        if let R::PlayerAmount { operation, amount } = &recipe {
            let amount = self.pipeline_quantity(amount).map_err(|e| e.at("amount"))?;
            let players = actor.clone();
            return Ok(self.effect(match operation {
                P::Draw => Operation::Draw { players, amount },
                P::Discard { random } => Operation::Discard {
                    players,
                    amount,
                    random: *random,
                },
                P::GainLife => Operation::GainLife { players, amount },
                P::LoseLife => Operation::LoseLife { players, amount },
                P::Mill => Operation::Mill { players, amount },
                P::Scry => Operation::Scry { players, amount },
                P::Surveil => Operation::Surveil { players, amount },
            }));
        }
        let (subjects, selection) = match &recipe {
            R::ContinuousEffect {
                subjects,
                selection,
                ..
            }
            | R::Move {
                subjects,
                selection,
                ..
            }
            | R::ObjectEffect {
                subjects,
                selection,
                ..
            }
            | R::Choose {
                subjects,
                selection,
                ..
            } => (subjects, selection),
            R::PlayerAmount { .. } => unreachable!(),
        };
        let mut objects = self
            .pipeline_subjects(subjects, actor)
            .map_err(|e| e.at("subjects"))?;
        let selection = selection
            .as_ref()
            .map(|s| self.pipeline_selection(s))
            .transpose()
            .map_err(|e| e.at("selection"))?;
        if let R::Choose { random, .. } = &recipe {
            let count = selection.ok_or_else(|| invalid("a choice needs a count"))?;
            let constraints = if *random {
                vec![SelectionConstraint::AtRandom]
            } else {
                vec![]
            };
            let (choice, result) = self.pick(actor, objects.clone(), count, constraints)?;
            self.note_choice(&objects, result);
            return Ok(choice);
        }
        // Sacrifice performs player choices as part of its simultaneous operation.
        // Moving that choice into a separate instruction would alter its policy.
        if matches!(
            &recipe,
            R::ObjectEffect {
                operation: O::Sacrifice,
                ..
            }
        ) {
            return Ok(self.effect(Operation::Sacrifice {
                players: actor.clone(),
                objects,
                choose: selection,
            }));
        }
        let mut instructions = vec![];
        if let Some(count) = selection {
            let PlayerSet::One(player) = actor else {
                return Err(invalid("resolution selection requires one actor"));
            };
            let random = matches!(&recipe, R::Move { random: true, .. });
            let choose = self.effect(Operation::ChooseObjects {
                player: player.clone(),
                objects,
                count,
                constraints: if random {
                    vec![SelectionConstraint::AtRandom]
                } else {
                    vec![]
                },
            });
            let Instruction::Effect { result, .. } = &choose else {
                unreachable!()
            };
            objects = ObjectSet::Selected(result.clone());
            instructions.push(choose);
        }
        let operation = match recipe {
            R::ContinuousEffect {
                change,
                duration,
                evaluation,
                ..
            } => {
                let changes = match change {
                    ChangeTemplate::ControllerOfActor => {
                        let PlayerSet::One(player) = actor else {
                            return Err(invalid("control effect requires one actor"));
                        };
                        vec![Change::SetController(player.clone())]
                    }
                    ChangeTemplate::LayerEffects(changes) => {
                        self.changes(&changes).map_err(|e| e.at("changes"))?
                    }
                    ChangeTemplate::Rules(rules) => {
                        self.rule_changes(&rules).map_err(|e| e.at("changes"))?
                    }
                };
                let duration = match duration {
                    DurationInput::Indefinite => Duration::Indefinite,
                    DurationInput::Source(duration) => {
                        self.duration(&duration).map_err(|e| e.at("duration"))?
                    }
                };
                Operation::CreateContinuousEffect {
                    source: ObjectRef::Source,
                    affected: EffectSubjects::Objects(objects),
                    changes,
                    duration,
                    evaluation,
                }
            }
            R::Move {
                destination,
                entry,
                depth,
                ..
            } => {
                let (destination, entry) = match destination {
                    DestinationInput::OwnersHand => (Destination::OwnersHand, EntrySpec::default()),
                    DestinationInput::OwnersGraveyard => {
                        (Destination::OwnersGraveyard, EntrySpec::default())
                    }
                    DestinationInput::OwnersLibraryTop => {
                        (Destination::OwnersLibraryTop, EntrySpec::default())
                    }
                    DestinationInput::OwnersLibraryBottom => {
                        (Destination::OwnersLibraryBottom, EntrySpec::default())
                    }
                    DestinationInput::OwnersLibraryShuffled => {
                        (Destination::OwnersLibraryShuffled, EntrySpec::default())
                    }
                    // 110.2a: a player told to put an object onto the battlefield controls it.
                    DestinationInput::BattlefieldYou => self
                        .move_entry(&entry, self.actor_or_you(actor)?)
                        .map_err(|e| e.at("entry"))?,
                    DestinationInput::BattlefieldActor => {
                        let PlayerSet::One(player) = actor else {
                            return Err(invalid("entering the battlefield needs one player"));
                        };
                        self.move_entry(&entry, player.clone())
                            .map_err(|e| e.at("entry"))?
                    }
                    DestinationInput::OwnersLibraryTopOrBottom => {
                        (Destination::OwnersLibraryTopOrBottom, EntrySpec::default())
                    }
                    DestinationInput::OwnersLibraryBottomRandom => {
                        (Destination::OwnersLibraryBottomRandom, EntrySpec::default())
                    }
                    DestinationInput::OwnersLibraryNth => {
                        let depth = depth.as_ref().ok_or_else(|| invalid("no library position"))?;
                        (
                            Destination::OwnersLibraryFromTop(self.pipeline_quantity(depth)?),
                            EntrySpec::default(),
                        )
                    }
                };
                Operation::Move {
                    objects,
                    destination,
                    entry,
                    face_down: false,
                    terms: crate::ir::MovementTerms::default(),
                }
            }
            R::ObjectEffect { operation, .. } => match operation {
                O::Tap { tapped } => Operation::Tap { objects, tapped },
                O::Destroy { regeneration } => Operation::Destroy {
                    objects,
                    regeneration,
                },
                O::Regenerate => Operation::CreateRegenerationShields { objects },
                O::Sacrifice => unreachable!(),
            },
            R::PlayerAmount { .. } | R::Choose { .. } => unreachable!(),
        };
        let effect = self.effect(operation);
        if instructions.is_empty() {
            Ok(effect)
        } else {
            instructions.push(effect);
            Ok(Instruction::Group(Program { instructions }))
        }
    }
}
