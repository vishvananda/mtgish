/// A replacement body spells ordinary actions in its own enums, one family per
/// event. Mapping those enums back lets one action path serve every family.
fn ordinary_enum(name: &str) -> Option<&'static str> {
    let family = name.strip_prefix("ReplacementAction")?;
    Some(if family.ends_with("RemoveCounters") {
        "RemoveCountersAction"
    } else if family.ends_with("PutCounters") {
        "PutCountersAction"
    } else if family.ends_with("LookAtTheTopNumberCardsOfLibraryAction") {
        "LookAtTopOfLibraryAction"
    } else if family.ends_with("Cost") {
        "Cost"
    } else {
        "Action"
    })
}

fn retag(value: &mut serde_json::Value, rename: &impl Fn(&str) -> Option<&'static str>) {
    match value {
        serde_json::Value::Array(items) => items.iter_mut().for_each(|v| retag(v, rename)),
        serde_json::Value::Object(fields) => {
            if let Some(name) = fields.get("enum").and_then(|n| n.as_str()).and_then(rename) {
                fields.insert("enum".into(), name.into());
            }
            fields.values_mut().for_each(|v| retag(v, rename));
        }
        _ => {}
    }
}

/// Forms that say what becomes of the event, so a body using one is explicit.
/// A regeneration shield belongs here because it applies to the event itself.
const EVENT_FORMS: [&str; 35] = [
    "EntersTapped",
    "EntersWithACounter",
    "EntersUnderPlayersControl",
    "ShuffleItIntoLibraryInstead",
    "RevealItAndShuffleItIntoLibraryInstead",
    "ContinueDealingDamage",
    "CancelThatDamage",
    "SkipThatDraw",
    "SkipTurn",
    "CancelDestroy",
    "GainNoLifeInstead",
    "DealDamageInstead",
    "PutNewAmount",
    "WouldProduceMana_ProduceMultiple",
    "WouldProduceMana_AddMana",
    "WouldProduceMana_ReplaceType",
    "WouldProduceMana_ReplaceColorWithTheChosenColor",
    "EntersWithLayerEffect",
    "EntersWithLayerEffectOfChoice",
    "RollThatManyPlusOneAndIgnoreLowestInstead",
    "RollThatManyPlusOneAndPlayerChoosesOneToIgnore",
    "CreateTokensInstead",
    "DealToPlayerInstead",
    "DealToCreatureOrPlaneswalkerInstead",
    "DealToAnyTargetInstead",
    "ExileItInstead",
    "PutItInOwnersHandInstead",
    "PutIntoGraveyardInstead",
    "PutItOntoTheBattlefieldInstead",
    "PutItOnTopOfOwnersLibraryInstead",
    "DiscardItToTopOfLibraryInstead",
    "PutItOnBottomOfOwnersLibraryInstead",
    "ExileItWithACounterInstead",
    "ExileItWithNumberCountersInstead",
    "RegeneratePermanent",
];

fn names_event_change(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Array(items) => items.iter().any(names_event_change),
        serde_json::Value::Object(fields) => {
            fields
                .get("variant")
                .and_then(|v| v.as_str())
                .is_some_and(|v| EVENT_FORMS.contains(&v))
                || fields.values().any(names_event_change)
        }
        _ => false,
    }
}

/// A step that only decides whether or by whom its own inner list happens.
fn only_wraps(step: &serde_json::Value) -> bool {
    match step["variant"].as_str().unwrap_or_default() {
        "If" | "Unless" | "IfElse" | "MayAction" | "MayActions" => true,
        "PlayerAction" => only_wraps(&step["args"][1]),
        _ => false,
    }
}

fn unsupported_step(step: &serde_json::Value) -> Diagnostic {
    Diagnostic {
        path: String::new(),
        source_type: step["enum"].as_str().unwrap_or("value").into(),
        form: step["variant"].as_str().unwrap_or("value").into(),
        message: "semantic lowering has not been implemented for this form".into(),
    }
}

fn decode_step<T: serde::de::DeserializeOwned>(value: &serde_json::Value) -> Result<T> {
    crate::pipeline::source_tree::decode(value).map_err(|_| unsupported_step(value))
}

/// The same form under the enum another family uses for it.
fn as_family<T: serde::de::DeserializeOwned>(value: &impl Serialize, family: &'static str) -> Option<T> {
    let mut inspected = crate::inspect::value(value).ok()?;
    let name = inspected["enum"].as_str()?.to_string();
    retag(&mut inspected, &|n: &str| (n == name).then_some(family));
    crate::pipeline::source_tree::decode(&inspected).ok()
}

/// `as_family` for a form whose operands change family with it.
fn cast<T: serde::de::DeserializeOwned>(
    value: &impl Serialize,
    families: &[(&'static str, &'static str)],
) -> Option<T> {
    let mut inspected = crate::inspect::value(value).ok()?;
    retag(&mut inspected, &|n: &str| {
        families.iter().find(|(from, _)| *from == n).map(|(_, to)| *to)
    });
    crate::pipeline::source_tree::decode(&inspected).ok()
}

impl Context {
    fn damage_replacement(
        &self,
        value: &source::ReplacableEventWouldDealDamage,
    ) -> Result<ReplacementEvent> {
        use source::ReplacableEventWouldDealDamage as S;
        match value {
            S::NoncombatDamageWouldBeDealtByASourceToARecipient(sources, recipients) => self
                .damage_shape(
                    DamageFrom::Sources(sources),
                    DamageTo::List(recipients),
                    DamageKind::Noncombat,
                )
                .map(ReplacementEvent::Damage),
            S::NoncombatDamageWouldBeDealtBySpellToARecipient(spell, recipients) => self
                .damage_shape(
                    DamageFrom::Spell(spell),
                    DamageTo::List(recipients),
                    DamageKind::Noncombat,
                )
                .map(ReplacementEvent::Damage),
            S::AnAmountOfDamageWouldBeDealtByASourceToARecipient(comparison, sources, recipients) => {
                let shape = self.damage_shape(
                    DamageFrom::Sources(sources),
                    DamageTo::List(recipients),
                    DamageKind::Any,
                )?;
                self.measured(ReplacementEvent::Damage(shape), comparison)
            }
            // The prevention family spells the same events.
            _ => as_family(value, "EventPreventDamage")
                .ok_or_else(|| unsupported(value))
                .and_then(|event| self.prevented_event(&event)),
        }
    }

    fn graveyard_replacement(
        &self,
        value: &source::ReplacableEventWouldPutIntoGraveyard,
    ) -> Result<ReplacementEvent> {
        use source::ReplacableEventWouldPutIntoGraveyard as S;
        let owned = |objects: ObjectFilter, owners: PlayerSet| {
            ObjectFilter::All(vec![objects, ObjectFilter::OwnedBy(owners)])
        };
        // A token is not a card, so "a card" leaves it to reach the graveyard.
        let card = |objects: ObjectFilter, owners: PlayerSet| {
            ObjectFilter::All(vec![
                objects,
                ObjectFilter::OwnedBy(owners),
                ObjectFilter::Not(Box::new(ObjectFilter::Token)),
            ])
        };
        let (objects, from) = match value {
            S::Or(items) => {
                return Ok(ReplacementEvent::Either(
                    items
                        .iter()
                        .map(|item| self.graveyard_replacement(item))
                        .collect::<Result<_>>()?,
                ))
            }
            S::APermanentWouldDie(objects) | S::APermanentWouldBePutIntoAGraveyard(objects) => {
                (self.filter(objects)?, Some(EventZone::Battlefield))
            }
            S::WouldPutAPermanentIntoAPlayersGraveyard(objects, owners) => (
                owned(self.filter(objects)?, self.players(owners)?),
                Some(EventZone::Battlefield),
            ),
            S::WouldPutAPermanentIntoPlayersGraveyard(objects, owner) => (
                owned(self.filter(objects)?, PlayerSet::One(self.player(owner)?)),
                Some(EventZone::Battlefield),
            ),
            S::WouldPutACardOrTokenInAPlayersGraveyardFromAnywhere(cards, owners) => {
                (owned(self.card_filter(cards)?, self.players(owners)?), None)
            }
            S::WouldPutACardInAPlayersGraveyardFromAnywhere(cards, owners) => {
                (card(self.card_filter(cards)?, self.players(owners)?), None)
            }
            S::WouldPutACardInPlayersGraveyardFromAnywhere(cards, owner) => (
                card(self.card_filter(cards)?, PlayerSet::One(self.player(owner)?)),
                None,
            ),
            _ => return Err(unsupported(value)),
        };
        Ok(ReplacementEvent::ZoneChange {
            objects,
            from,
            to: EventZone::Graveyard,
        })
    }

    fn replacement_event(&self, event: &serde_json::Value) -> Result<ReplacementEvent> {
        let family = event["enum"].as_str().unwrap_or_default();
        match family {
            "ReplacableEventWouldDealDamage" => self.damage_replacement(&decode_step(event)?),
            "ReplacableEventWouldPutIntoGraveyard" => {
                self.graveyard_replacement(&decode_step(event)?)
            }
            _ => self
                .primitive_inspected(family, event)
                .map(|(name, operands)| crate::semantics::reduce_replacement(name, operands))
                .ok_or_else(|| unsupported_step(event)),
        }
    }

    fn replacement_steps(
        &mut self,
        steps: &[serde_json::Value],
        actor: &PlayerRef,
    ) -> Result<Program> {
        let mut instructions = Vec::new();
        // Outside damage and zone changes, a body's actions happen instead of
        // the event unless the body says otherwise.
        if self.body_replaces
            && !steps.iter().all(only_wraps)
            && !steps.iter().any(names_event_change)
        {
            instructions.push(self.effect(Operation::ModifyEvent {
                change: EventChange::Cancel,
            }));
        }
        for (i, step) in steps.iter().enumerate() {
            let step = self
                .replacement_step(step, actor)
                .map_err(|e| e.at(format!("replacement[{i}]")))?;
            instructions.extend(step);
        }
        Ok(Program { instructions })
    }

    fn replacement_child(
        &mut self,
        steps: &[serde_json::Value],
        actor: &PlayerRef,
    ) -> Result<Program> {
        let mut child = self.clone();
        let program = child.replacement_steps(steps, actor)?;
        self.result = child.result;
        self.invalidate_child_bindings(&child);
        Ok(program)
    }

    /// A step that changes the pending event, or `None` if this form is not one.
    fn event_change(
        &mut self,
        variant: &str,
        args: &serde_json::Value,
    ) -> Result<Option<Vec<EventChange>>> {
        let exile = EventChange::SetDestination(Destination::Exile);
        Ok(Some(match variant {
            "ContinueDealingDamage" => vec![],
            "ShuffleItIntoLibraryInstead" => {
                vec![EventChange::SetDestination(Destination::OwnersLibraryShuffled)]
            }
            "CancelThatDamage" | "SkipThatDraw" | "SkipTurn" | "CancelDestroy"
            | "GainNoLifeInstead" => vec![EventChange::Cancel],
            "DealDamageInstead" | "PutNewAmount" => vec![EventChange::SetAmount(
                self.number(&decode_step::<source::GameNumber>(args)?)?,
            )],
            "WouldProduceMana_ProduceMultiple" => vec![EventChange::SetAmount(Expr::multiply(
                self.number(&decode_step::<source::GameNumber>(args)?)?,
                Expr::EventAmount,
            ))],
            "WouldProduceMana_AddMana" => vec![EventChange::AddMana(
                self.mana_production(&decode_step(args)?)?,
            )],
            "WouldProduceMana_ReplaceType" => vec![EventChange::ReplaceMana(
                self.mana_production(&decode_step(args)?)?,
            )],
            "WouldProduceMana_ReplaceColorWithTheChosenColor" => {
                vec![EventChange::ReplaceColoredMana(
                    self.mana_production(&source::ManaProduce::ManaOfTheChosenColor)?,
                )]
            }
            "EntersWithLayerEffect" => {
                let effects = args
                    .as_array()
                    .map_or(&[][..], |a| a)
                    .iter()
                    .map(decode_step)
                    .collect::<Result<Vec<source::LayerEffect>>>()?;
                vec![EventChange::Entry(EntryState::Changes(self.changes(&effects)?))]
            }
            "RollThatManyPlusOneAndIgnoreLowestInstead" => {
                vec![EventChange::RollExtraDie(DiceIgnore::Lowest)]
            }
            "RollThatManyPlusOneAndPlayerChoosesOneToIgnore" => {
                vec![EventChange::RollExtraDie(DiceIgnore::ChosenBy(
                    self.player(&decode_step(args)?)?,
                ))]
            }
            "EntersWithLayerEffectOfChoice" => {
                let options = args
                    .as_array()
                    .map_or(&[][..], |a| a)
                    .iter()
                    .map(|option| {
                        let effects = option
                            .as_array()
                            .map_or(&[][..], |a| a)
                            .iter()
                            .map(decode_step)
                            .collect::<Result<Vec<source::LayerEffect>>>()?;
                        self.changes(&effects)
                    })
                    .collect::<Result<_>>()?;
                vec![EventChange::Entry(EntryState::ChooseChanges(options))]
            }
            "CreateTokensInstead" => {
                let mut copies = 0;
                let mut tokens = Vec::new();
                for token in args.as_array().map_or(&[][..], |a| a) {
                    if token["variant"] == "ThoseTokens" {
                        copies += 1;
                    } else {
                        tokens.push(self.token(&decode_step(token)?, 0)?);
                    }
                }
                vec![EventChange::ReplaceTokens { copies, tokens }]
            }
            "DealToPlayerInstead" => vec![EventChange::RedirectDamage(Recipients::Players(
                PlayerSet::One(self.player(&decode_step::<source::Player>(args)?)?),
            ))],
            "DealToCreatureOrPlaneswalkerInstead" => {
                vec![EventChange::RedirectDamage(Recipients::Objects(
                    ObjectSet::One(self.object(&decode_step::<source::Permanent>(args)?)?),
                ))]
            }
            "DealToAnyTargetInstead" => {
                use source::SingleDamageRecipient as S;
                let recipient = decode_step::<S>(args)?;
                vec![EventChange::RedirectDamage(match &recipient {
                    S::Player(p) => Recipients::Players(PlayerSet::One(self.player(p)?)),
                    S::Permanent(p) => Recipients::Objects(ObjectSet::One(self.object(p)?)),
                    S::Ref_AnyTarget | S::Ref_TargetPlayerOrPermanent => {
                        Recipients::Targets(self.target(TargetKind::DamageRecipient, 0, 0)?)
                    }
                    _ => return Err(unsupported(&recipient)),
                })]
            }
            "ExileItInstead" => vec![exile],
            "PutItInOwnersHandInstead" => {
                vec![EventChange::SetDestination(Destination::OwnersHand)]
            }
            "PutIntoGraveyardInstead" => {
                vec![EventChange::SetDestination(Destination::OwnersGraveyard)]
            }
            "PutItOntoTheBattlefieldInstead" => {
                let flags = args
                    .as_array()
                    .map_or(&[][..], |a| a)
                    .iter()
                    .map(decode_step)
                    .collect::<Result<Vec<source::EnterFlag>>>()?;
                let you = self.player(&source::Player::You)?;
                let (_, entry) = self.move_entry(&flags, you)?;
                let mut changes = vec![EventChange::SetDestination(Destination::OwnersBattlefield)];
                if entry.tapped {
                    changes.push(EventChange::SetTapped(true));
                }
                changes.extend(
                    entry
                        .counters
                        .into_iter()
                        .map(|(kind, amount)| EventChange::AddCounters { kind, amount }),
                );
                changes.extend(entry.states.into_iter().map(EventChange::Entry));
                changes
            }
            "PutItOnTopOfOwnersLibraryInstead" | "DiscardItToTopOfLibraryInstead" => {
                vec![EventChange::SetDestination(Destination::OwnersLibraryTop)]
            }
            "PutItOnBottomOfOwnersLibraryInstead" => {
                vec![EventChange::SetDestination(Destination::OwnersLibraryBottom)]
            }
            "ExileItWithACounterInstead" => vec![
                exile,
                EventChange::AddCounters {
                    kind: crate::counter::lower(&decode_step(args)?),
                    amount: Expr::Constant(1),
                },
            ],
            "ExileItWithNumberCountersInstead" => vec![
                exile,
                EventChange::AddCounters {
                    kind: crate::counter::lower(&decode_step(&args[1])?),
                    amount: self.number(&decode_step::<source::GameNumber>(&args[0])?)?,
                },
            ],
            _ => return Ok(None),
        }))
    }

    /// One step of a replacement body. Forms shared by every family are
    /// structural; anything else that isn't an event change is an ordinary action.
    fn replacement_step(
        &mut self,
        step: &serde_json::Value,
        actor: &PlayerRef,
    ) -> Result<Option<Instruction>> {
        let variant = step["variant"].as_str().unwrap_or_default();
        let args = &step["args"];
        if variant == "RevealItAndShuffleItIntoLibraryInstead" {
            let instructions = vec![
                self.effect(Operation::Reveal {
                    objects: ObjectSet::One(ObjectRef::EventSubject),
                }),
                self.effect(Operation::ModifyEvent {
                    change: EventChange::SetDestination(Destination::OwnersLibraryShuffled),
                }),
            ];
            return Ok(Some(Instruction::Group(Program { instructions })));
        }
        // A timed entry replacement says what an "as this enters" one says.
        if step["enum"] == "ReplacementActionWouldEnter"
            && matches!(
                variant,
                "EntersTapped" | "EntersWithACounter" | "EntersUnderPlayersControl"
            )
        {
            let value: source::ReplacementActionWouldEnter = decode_step(step)?;
            return self.entry_action(&value).map(Some);
        }
        if let Some(mut changes) = self.event_change(variant, args)? {
            let mut instructions: Vec<_> = changes
                .drain(..)
                .map(|change| self.effect(Operation::ModifyEvent { change }))
                .collect();
            return Ok(match instructions.len() {
                0 => None,
                1 => instructions.pop(),
                _ => Some(Instruction::Group(Program { instructions })),
            });
        }
        let list = |value: &'_ serde_json::Value| -> Vec<serde_json::Value> {
            value.as_array().cloned().unwrap_or_default()
        };
        Ok(Some(match variant {
            "If" | "Unless" => {
                let mut condition = self.condition(&decode_step::<source::Condition>(&args[0])?)?;
                if variant == "Unless" {
                    condition = Condition::Not(Box::new(condition));
                }
                let then = list(&args[1]);
                self.conditional(
                    condition,
                    |c| c.replacement_steps(&then, actor),
                    |_| Ok(Program::empty()),
                )?
            }
            "IfElse" => {
                let condition = self.condition(&decode_step::<source::Condition>(&args[0])?)?;
                let (then, otherwise) = (list(&args[1]), list(&args[2]));
                self.conditional(
                    condition,
                    |c| c.replacement_steps(&then, actor),
                    |c| c.replacement_steps(&otherwise, actor),
                )?
            }
            "ItExplores" => self.effect(crate::semantics::reduce_action(
                PlayerSet::One(actor.clone()),
                "Action::ExploreWithPermanent".into(),
                vec![Operand::Object(ObjectRef::EventSubject)],
            )),
            "ChooseAnAction" => {
                let options = list(args)
                    .iter()
                    .map(|option| {
                        self.replacement_child(&list(option), actor)
                            .map(Operand::Program)
                    })
                    .collect::<Result<_>>()?;
                self.effect(Operation::ChooseAction {
                    actor: PlayerSet::One(actor.clone()),
                    options,
                    villainous: false,
                })
            }
            "MayAction" => Instruction::Optional {
                decider: actor.clone(),
                body: self.replacement_child(std::slice::from_ref(args), actor)?,
            },
            "MayActions" => Instruction::Optional {
                decider: actor.clone(),
                body: self.replacement_child(&list(args), actor)?,
            },
            "PlayerAction" => {
                let player = self.player(&decode_step::<source::Player>(&args[0])?)?;
                return self.replacement_step(&args[1], &player);
            }
            "PlayerActions" => {
                let player = self.player(&decode_step::<source::Player>(&args[0])?)?;
                Instruction::Group(self.replacement_child(&list(&args[1]), &player)?)
            }
            _ => {
                let mut ordinary = step.clone();
                retag(&mut ordinary, &ordinary_enum);
                match crate::pipeline::source_tree::decode::<source::Action>(&ordinary) {
                    Ok(action) => self.action(&action, &PlayerSet::One(actor.clone()))?,
                    Err(_) => {
                        let family = step["enum"].as_str().unwrap_or_default();
                        let (name, operands) = self
                            .primitive_inspected(family, step)
                            .ok_or_else(|| unsupported_step(step))?;
                        self.effect(Operation::Primitive {
                            actor: PlayerSet::One(actor.clone()),
                            name,
                            operands,
                        })
                    }
                }
            }
        }))
    }

    fn replacement_body(
        &mut self,
        event: &ReplacementEvent,
        steps: &[serde_json::Value],
    ) -> Result<Program> {
        let mut child = self.clone();
        child.event = crate::semantics::replacement_bindings(event);
        child.body_replaces = crate::semantics::body_replaces(event);
        // "They lose twice that much": the player who would lose it, not the controller.
        let actor = match event {
            ReplacementEvent::Player {
                event: ReplacedPlayerEvent::LoseLife,
                ..
            } => PlayerRef::EventSubject,
            _ => self.player(&source::Player::You)?,
        };
        let body = child.replacement_steps(steps, &actor)?;
        self.result = child.result;
        Ok(body)
    }

    /// Every `Rule::Replace...` over a would-be event has one shape: event and body.
    fn replacement_rule(&mut self, value: &source::Rule) -> Option<Result<Ability>> {
        if let source::Rule::AsPutIntoAGraveyardFromAnywhere(card, steps) = value {
            return Some((|| {
                let event = ReplacementEvent::ZoneChange {
                    objects: ObjectFilter::SameObject(self.single_card(card)?),
                    from: None,
                    to: EventZone::Graveyard,
                };
                let steps: Vec<_> = steps.iter().filter_map(|s| crate::inspect::value(s).ok()).collect();
                let body = self.replacement_body(&event, &steps)?;
                Ok(Ability::Replacement {
                    active: ReplacementActive::Everywhere,
                    event,
                    body,
                })
            })());
        }
        let inspected = crate::inspect::value(value).ok()?;
        let variant = inspected["variant"].as_str()?;
        if !variant.starts_with("Replace") || variant == "ReplaceWouldEnter" {
            return None;
        }
        let [event, steps] = inspected["args"].as_array()?.as_slice() else {
            return None;
        };
        event["enum"].as_str()?.strip_prefix("ReplacableEvent")?;
        // A card replaces its own discard from the hand it is still in.
        let active = if event.to_string().contains("ThisCardInHand") {
            ReplacementActive::Everywhere
        } else {
            ReplacementActive::Battlefield
        };
        Some((|| {
            let event = self.replacement_event(event)?;
            let body = self.replacement_body(&event, steps.as_array().map_or(&[], |s| s))?;
            Ok(Ability::Replacement {
                active,
                event,
                body,
            })
        })())
    }

    /// `Action::CreateReplaceWould...Until` and `Action::CreateFutureReplaceWould...`.
    fn replacement_creation(&mut self, value: &source::Action) -> Option<Result<Operation>> {
        let inspected = crate::inspect::value(value).ok()?;
        let variant = inspected["variant"].as_str()?;
        let future = variant.starts_with("CreateFutureReplace");
        if !future && !variant.starts_with("CreateReplace") {
            return None;
        }
        let args = inspected["args"].as_array()?;
        args.first()?["enum"].as_str()?.contains("ReplacableEvent").then_some(())?;
        Some((|| {
            let (event, limit, duration) = match args.as_slice() {
                [event, _, expiration] if !future => (
                    self.replacement_event(event)?,
                    ReplacementLimit::UntilExpired,
                    self.duration(&decode_step::<source::Expiration>(expiration)?)?,
                ),
                [event, _] if future => {
                    let (event, limit) = self.future_replacement(event)?;
                    (event, limit, Duration::EndOfTurn)
                }
                _ => return Err(unsupported(value)),
            };
            let body = self.replacement_body(&event, args[1].as_array().map_or(&[], |s| s))?;
            Ok(Operation::CreateReplacement {
                event,
                limit,
                duration,
                body,
            })
        })())
    }

    fn future_replacement(
        &self,
        event: &serde_json::Value,
    ) -> Result<(ReplacementEvent, ReplacementLimit)> {
        let family = event["enum"].as_str().unwrap_or_default();
        if family == "FutureReplacableEventWouldDealDamage" {
            let mut prevention = event.clone();
            retag(&mut prevention, &|n: &str| {
                (n == family).then_some("FutureEventPreventDamage")
            });
            let prevention = decode_step(&prevention).map_err(|_| unsupported_step(event))?;
            return self.future_prevention(&prevention);
        }
        self.primitive_inspected(family, event)
            .map(|(name, operands)| {
                (
                    ReplacementEvent::Primitive { name, operands },
                    ReplacementLimit::NextOccurrence,
                )
            })
            .ok_or_else(|| unsupported_step(event))
    }
}
