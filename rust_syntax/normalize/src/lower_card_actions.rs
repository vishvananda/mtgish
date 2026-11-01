// Forms that take several instructions, or a scope a pure reducer can't reach.
impl Context {
    /// Actions that lower to more than one instruction, each read in this scope.
    fn card_action(
        &mut self,
        value: &source::Action,
        actor: &PlayerSet,
    ) -> Result<Option<Instruction>> {
        use source::Action as S;
        Ok(Some(match value {
            S::ActionForEachCheckableAbility(abilities, actions) => {
                let mut instructions = vec![];
                for ability in abilities {
                    for action in actions {
                        let action = with_each_ability(action, ability)?;
                        instructions.push(self.action(&action, actor)?);
                    }
                }
                Instruction::Group(Program { instructions })
            }
            S::CreateCopiesOfRandomCardsWithManaCosts(number) => {
                let source::GameNumber::ManaCostsOfCombatCreatures(creatures) = number.as_ref()
                else {
                    return Ok(None);
                };
                let creatures = self.objects(creatures)?;
                let actor = actor.clone();
                self.for_each_object(creatures, |context| {
                    let binding = context
                        .iterated_object
                        .clone()
                        .ok_or_else(|| invalid("no creature is being iterated"))?;
                    let instruction = context.effect(Operation::CopyCards {
                        actor,
                        objects: ObjectSet::Random {
                            objects: Box::new(ObjectSet::OutsideTheGame(ObjectFilter::Shares {
                                property: DistinctProperty::ManaCosts,
                                with: ObjectRef::Bound(binding),
                            })),
                            count: Expr::Constant(1),
                        },
                        times: Expr::Constant(1),
                    });
                    Ok(Program {
                        instructions: vec![instruction],
                    })
                })?
            }
            S::DuringNextUntap(player, actions) => {
                let player = self.player(player)?;
                let mut child = self.nested_program();
                let body = child.action_list(actions, actor)?;
                self.result = child.result;
                self.invalidate_child_bindings(&child);
                self.effect(Operation::CreateReplacement {
                    event: ReplacementEvent::Player {
                        event: ReplacedPlayerEvent::UntapPermanents,
                        players: PlayerSet::One(player.clone()),
                    },
                    limit: ReplacementLimit::NextOccurrence,
                    duration: Duration::DuringNextUntapStep(player),
                    body,
                })
            }
            S::SacrificeAllPermanentsExceptForAPermanentOfEachLandType(objects) => {
                use source::LandType::{Forest, Island, Mountain, Plains, Swamp};
                let slots = [Plains, Island, Swamp, Mountain, Forest]
                    .iter()
                    .map(|ty| ObjectFilter::Subtype(crate::subtype::land_type(ty)))
                    .collect();
                self.sacrifice_rest(actor, objects, slots)?
            }
            S::SacrificeAllPermanentsExceptForASpecificPermanentOfEachTypeOfTheirChoice(
                objects,
                each,
            ) => {
                let slots = each
                    .iter()
                    .map(|kept| self.filter(kept))
                    .collect::<Result<_>>()?;
                self.sacrifice_rest(actor, objects, slots)?
            }
            S::EachPlayerMayChooseAnAction(players, costs) => {
                let players = self.players(players)?;
                self.offered = Some(players.clone());
                self.for_each_player(players, |context, each| {
                    let PlayerSet::One(payer) = each else {
                        return Err(invalid("a payment has one payer"));
                    };
                    let options = costs
                        .iter()
                        .map(|cost| context.cost_for(cost, payer))
                        .collect::<Result<_>>()?;
                    let payment = context.effect(Operation::PayCost {
                        cost: Cost {
                            parts: vec![CostPart::Choice(options)],
                        },
                        player: payer.clone(),
                        optional: true,
                    });
                    if let Instruction::Effect { result, .. } = &payment {
                        context.took = Some(result.clone());
                    }
                    Ok(payment)
                })?
            }
            S::EachPlayerStartingWithActionInDirection(players, first, _, actions) => {
                let ordered = S::EachPlayerStartingWithAction(
                    players.clone(),
                    first.clone(),
                    actions.clone(),
                );
                let mut each = self.action(&ordered, actor)?;
                if let Instruction::ForEachPlayer {
                    in_chosen_direction,
                    ..
                } = &mut each
                {
                    *in_chosen_direction = true;
                }
                each
            }
            S::IfHavePlayerAction(player, action, then) => {
                let PlayerSet::One(decider) = actor else {
                    return Err(invalid("an optional action needs one decider"));
                };
                let player = PlayerSet::One(self.player(player)?);
                let mut child = self.clone();
                let mut instructions = vec![child.action(action, &player)?];
                instructions.extend(child.action_list(then, actor)?.instructions);
                self.result = child.result;
                self.invalidate_child_bindings(&child);
                Instruction::Optional {
                    decider: decider.clone(),
                    body: Program { instructions },
                }
            }
            S::MayPutTheTopCardOfPlayersLibraryOfTypeInGraveyardForCost(player, cards, cost) => {
                let PlayerSet::One(payer) = actor else {
                    return Err(invalid("a payment has one payer"));
                };
                let top = ObjectSet::LibraryTops(PlayerSet::One(self.player(player)?));
                let matches = ObjectSet::Matching {
                    objects: Box::new(top.clone()),
                    filter: self.card_filter(cards)?,
                };
                let cost = self.cost_for(cost, payer)?;
                self.conditional(
                    any_of(matches),
                    |context| {
                        let payment = context.effect(Operation::PayCost {
                            player: payer.clone(),
                            cost,
                            optional: true,
                        });
                        let Instruction::Effect { result, .. } = &payment else {
                            unreachable!()
                        };
                        let paid = Condition::PaymentChosen(result.clone());
                        let moved = context.conditional(
                            paid,
                            |context| {
                                Ok(Program {
                                    instructions: vec![context.effect(Operation::Move {
                                        objects: top,
                                        destination: Destination::OwnersGraveyard,
                                        entry: EntrySpec::default(),
                                        face_down: false,
                                    })],
                                })
                            },
                            |_| Ok(Program::empty()),
                        )?;
                        Ok(Program {
                            instructions: vec![payment, moved],
                        })
                    },
                    |_| Ok(Program::empty()),
                )?
            }
            S::MultiCreateTokens(batches) => {
                use source::MultiCreateToken as M;
                let operations = batches
                    .iter()
                    .map(|batch| {
                        let (players, tokens, flags) = match batch {
                            M::CreateTokens(tokens, flags) => (actor.clone(), tokens, flags),
                            M::EachPlayerCreatesTokens(players, tokens, flags) => {
                                (self.players(players)?, tokens, flags)
                            }
                        };
                        Ok(Operation::CreateTokens {
                            players,
                            tokens: tokens
                                .iter()
                                .map(|token| self.token(token, 0))
                                .collect::<Result<_>>()?,
                            entry: self.token_entry(flags)?,
                        })
                    })
                    .collect::<Result<_>>()?;
                self.effect(Operation::Together(operations))
            }
            S::PlayerChoosesCostActionForEachPermanent(player, permanents, options) => {
                let objects = self.objects(permanents)?;
                self.for_each_object(objects, |context| {
                    let decider = context.player(player)?;
                    let branch = context.cost_options(&decider, options, actor)?;
                    Ok(Program {
                        instructions: vec![branch],
                    })
                })?
            }
            S::PutGraveyardCardIntoHandOrOntoBattlefield(card, flags) => {
                let options = S::ChooseAnAction(vec![
                    vec![S::PutGraveyardCardIntoHand(card.clone())],
                    vec![S::PutGraveyardCardOntoBattlefield(card.clone(), flags.clone())],
                ]);
                self.action(&options, actor)?
            }
            S::SacrificedPermanentDealsDistributedDamage => {
                let source = self.object(&source::Permanent::TheSacrificedPermanent)?;
                let operation = self.distributed_damage(source)?;
                self.effect(operation)
            }
            _ => return Ok(None),
        }))
    }

    /// Each player keeps one permanent for each slot, then sacrifices the others
    /// `objects` holds: Cataclysm's two steps.
    fn sacrifice_rest(
        &mut self,
        players: &PlayerSet,
        objects: &source::Permanents,
        slots: Vec<ObjectFilter>,
    ) -> Result<Instruction> {
        let objects = self.objects(objects)?;
        let mut kept = None;
        let choice = self.for_each_player(players.clone(), |context, player| {
            let own = ObjectSet::Matching {
                objects: Box::new(objects),
                filter: ObjectFilter::ControlledBy(player.clone()),
            };
            let choice = context.pick_each(player, own, slots, Count::AsManyAsCan)?;
            if let Instruction::Effect { result, .. } = &choice {
                kept = Some(result.clone());
            }
            Ok(choice)
        })?;
        let kept = kept.ok_or_else(|| invalid("a choice has a result"))?;
        let sacrifice = self.effect(Operation::Sacrifice {
            players: players.clone(),
            objects: ObjectSet::Unselected(kept),
            choose: None,
        });
        Ok(Instruction::Group(Program {
            instructions: vec![choice, sacrifice],
        }))
    }

    /// One option is chosen; a costed option is paid before what follows it.
    fn cost_options(
        &mut self,
        decider: &PlayerRef,
        options: &[source::ActionOption],
        actor: &PlayerSet,
    ) -> Result<Instruction> {
        use source::ActionOption as O;
        let mut programs = Vec::new();
        let mut branches = Vec::new();
        for option in options {
            let mut child = self.clone();
            let (cost, actions) = match option {
                O::ActionOption(cost, actions) => (Some(cost), actions),
                O::DoNothingOption(actions) => (None, actions),
            };
            let mut instructions = vec![];
            if let Some(cost) = cost {
                let cost = child.cost_for(cost, decider)?;
                instructions.push(child.effect(Operation::PayCost {
                    player: decider.clone(),
                    cost,
                    optional: false,
                }));
            }
            instructions.extend(child.action_list(actions, actor)?.instructions);
            self.result = child.result;
            self.invalidate_child_bindings(&child);
            programs.push(Program { instructions });
            branches.push(child);
        }
        let branch = Instruction::ChooseOne {
            decider: decider.clone(),
            options: programs,
        };
        Ok(self.joined(branch, &branches.iter().collect::<Vec<_>>()))
    }

    /// Excess damage goes to a player instead, and what follows it happens if any did.
    fn excess_damage(
        &mut self,
        permanent: &source::Permanent,
        amount: &source::GameNumber,
        recipients: &source::DamageRecipient,
        instead: &[source::Action],
    ) -> Result<Instruction> {
        let (redirect, then) = instead
            .split_at_checked(1)
            .ok_or_else(|| invalid("excess damage goes to one player"))?;
        let rider = self.excess_rider(redirect);
        let operation = self.deal_with(self.object(permanent), amount, recipients, rider)?;
        let deal = self.effect(operation);
        if then.is_empty() {
            return Ok(deal);
        }
        let you = PlayerSet::One(self.player(&source::Player::You)?);
        let excess = Condition::Compare {
            left: Expr::Game(GameValue::ExcessDamageThisWay),
            comparison: Comparison::Greater,
            right: Expr::Constant(0),
        };
        let follow = self.conditional(
            excess,
            |context| context.action_list(then, &you),
            |_| Ok(Program::empty()),
        )?;
        Ok(Instruction::Group(Program {
            instructions: vec![deal, follow],
        }))
    }

    /// Entry steps that need this scope: a flip, or a result read for each keyword.
    fn card_entry(
        &mut self,
        value: &source::ReplacementActionWouldEnter,
    ) -> Result<Option<Instruction>> {
        use source::ReplacementActionWouldEnter as S;
        Ok(Some(match value {
            S::EntersWithLayerEffectUntil(effects, until) => {
                let change = EventChange::Entry(EntryState::ChangesUntil {
                    changes: self.changes(effects)?,
                    duration: self.duration(until)?,
                });
                self.effect(Operation::ModifyEvent { change })
            }
            S::FlipACoin_OnHeadAndOnTails(heads, tails) => {
                let you = PlayerSet::One(self.player(&source::Player::You)?);
                let (flip, result) = self.coin_flip(&you)?;
                let branch = self.conditional(
                    Condition::CoinFlip {
                        result,
                        outcome: CoinOutcome::Heads,
                    },
                    |context| context.entry_actions(heads),
                    |context| context.entry_actions(tails),
                )?;
                Instruction::Group(Program {
                    instructions: vec![flip, branch],
                })
            }
            S::EntersWithAnAbilityCounterForEachAbilityOnACardDiscardedThisWay(abilities) => {
                let mut instructions = vec![];
                for ability in abilities {
                    let kind = ability_counter(ability).ok_or_else(|| unsupported(ability))?;
                    let filter = self.has_keyword(ability)?;
                    let discarded =
                        self.this_way_set(Done::Discarded, ObjectPhase::After, Some(filter))?;
                    instructions.push(self.conditional(
                        any_of(discarded),
                        |context| {
                            Ok(Program {
                                instructions: vec![context.effect(Operation::ModifyEvent {
                                    change: EventChange::AddCounters {
                                        kind: crate::counter::lower(&kind),
                                        amount: Expr::Constant(1),
                                    },
                                })],
                            })
                        },
                        |_| Ok(Program::empty()),
                    )?);
                }
                Instruction::Group(Program { instructions })
            }
            _ => return Ok(None),
        }))
    }
}

/// One slot for each card type a card in a game's main zones can have.
fn each_card_type() -> Vec<ObjectFilter> {
    use crate::mtg_types::CardType::*;
    [
        Artifact,
        Battle,
        Creature,
        Enchantment,
        Instant,
        Kindred,
        Land,
        Planeswalker,
        Sorcery,
    ]
    .into_iter()
    .map(ObjectFilter::CardType)
    .collect()
}

fn any_of(objects: ObjectSet) -> Condition {
    Condition::Compare {
        left: Expr::CountObjects(Box::new(objects)),
        comparison: Comparison::Greater,
        right: Expr::Constant(0),
    }
}

/// The counter named for a keyword (CR 122.1b).
fn ability_counter(ability: &source::CheckHasable) -> Option<source::CounterType> {
    use source::CheckHasable as S;
    use source::CounterType as C;
    Some(match ability {
        S::Deathtouch => C::DeathtouchCounter,
        S::DoubleStrike => C::DoubleStrikeCounter,
        S::FirstStrike => C::FirstStrikeCounter,
        S::Flying => C::FlyingCounter,
        S::Haste => C::HasteCounter,
        S::AnyHexproof => C::HexproofCounter,
        S::Indestructible => C::IndestructibleCounter,
        S::Lifelink => C::LifelinkCounter,
        S::Menace => C::MenaceCounter,
        S::Reach => C::ReachCounter,
        S::Trample => C::TrampleCounter,
        S::Vigilance => C::VigilanceCounter,
        _ => return None,
    })
}

/// The action with this ability wherever it says "that ability".
fn with_each_ability(
    action: &source::Action,
    ability: &source::CheckHasable,
) -> Result<source::Action> {
    fn replace(value: &mut serde_json::Value, ability: &serde_json::Value) {
        use serde_json::Value as V;
        match value {
            V::Object(fields) => {
                if fields.get("enum").and_then(V::as_str) == Some("CheckHasable")
                    && fields.get("variant").and_then(V::as_str) == Some("EachableAbility")
                {
                    *value = ability.clone();
                    return;
                }
                fields.values_mut().for_each(|child| replace(child, ability));
            }
            V::Array(items) => items.iter_mut().for_each(|child| replace(child, ability)),
            _ => {}
        }
    }
    let inspected = crate::inspect::value(ability).map_err(|_| unsupported(ability))?;
    let mut body = crate::inspect::value(action).map_err(|_| unsupported(action))?;
    replace(&mut body, &inspected);
    crate::pipeline::source_tree::decode(&body).map_err(|_| unsupported(action))
}
