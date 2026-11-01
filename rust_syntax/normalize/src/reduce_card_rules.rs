// Rules one or two cards print, on the shapes a family of cards could share: a
// would-be event narrowed by what causes it, an event followed by more, a play
// permission on terms, a cost that scales. Anything unmatched stays with its row.

/// The enclosing scope plus what a would-be event binds, for its own amount.
pub fn with_replacement(outer: &EventBindings, event: &ReplacementEvent) -> EventBindings {
    with_bindings(outer, &replacement_bindings(event))
}

/// How long a row's delayed trigger waits, when its own words end it.
pub fn future_expiry(name: &str) -> Option<Duration> {
    match name.split_once("::")?.1 {
        "WhenPlayerNextActivatesAnAbilityBySpendingAnAmountOfMana" => Some(Duration::EndOfTurn),
        "WhenPermanentWinsAFightAgainstPermanent" => {
            Some(Duration::UntilStateBasedActionsChecked)
        }
        _ => None,
    }
}

/// Whether a delayed trigger's body reads the event it waits for, not its creator's.
pub fn delayed_reads_event(event: &EventSpec) -> bool {
    matches!(
        event,
        EventSpec::Primitive { .. }
            | EventSpec::Activated { .. }
            | EventSpec::SpellOrActivation { .. }
    )
}

fn pending_damage(
    source_kind: ObjectKind,
    sources: ObjectFilter,
    recipients: DamageRecipients,
    kind: DamageKind,
) -> ReplacementEvent {
    ReplacementEvent::Damage(DamageEvent {
        source_kind,
        sources,
        recipients,
        kind,
        grouping: DamageGrouping::SourceRecipient,
        subject: DamageSubject::Recipient,
    })
}

fn damage_recipients(recipients: &Recipients) -> Option<DamageRecipients> {
    Some(match recipients {
        Recipients::Objects(objects) => DamageRecipients::Objects(event_filter(objects)?),
        Recipients::Players(players) => DamageRecipients::Players(players.clone()),
        _ => return None,
    })
}

/// Would-be events the player and object tables have no row for.
fn reduce_card_replacement(name: &str, operands: &[Operand]) -> Option<ReplacementEvent> {
    use Operand::{Filter, Number, Object, Objects, Player, Players, Recipients as To};
    let (_, form) = name.split_once("::")?;
    Some(match (form, operands) {
        (
            "ASpellOrAbilityWouldCauseASourceToDealDamageToRecipient",
            [by, Filter(sources), To(recipients)],
        ) => ReplacementEvent::Caused {
            cause: stack_filter(by)?,
            event: Box::new(pending_damage(
                ObjectKind::Any,
                sources.clone(),
                damage_recipients(recipients)?,
                DamageKind::Any,
            )),
        },
        (
            "NextTimeCombatDamageWouldBeDealtThisTurnByCreatureToARecipient",
            [Object(creature), To(recipients)],
        ) => pending_damage(
            ObjectKind::Permanent,
            ObjectFilter::SameObject(creature.clone()),
            damage_recipients(recipients)?,
            DamageKind::Combat,
        ),
        ("Each1DamagePlayerWouldBeDealt", [Player(player)]) => {
            ReplacementEvent::EachPoint(Box::new(pending_damage(
                ObjectKind::Any,
                ObjectFilter::Any,
                DamageRecipients::Players(PlayerSet::One(player.clone())),
                DamageKind::Any,
            )))
        }
        // At least as much damage as the planeswalker has loyalty, which is what
        // would remove every counter.
        ("LethalLoyaltyDamageWouldBeDealtToAPlaneswalker", [Objects(objects)]) => {
            ReplacementEvent::Measured {
                event: Box::new(pending_damage(
                    ObjectKind::Any,
                    ObjectFilter::Any,
                    DamageRecipients::Objects(event_filter(objects)?),
                    DamageKind::Any,
                )),
                comparison: Comparison::GreaterOrEqual,
                amount: Expr::CountCounters {
                    recipients: Box::new(Recipients::Objects(ObjectSet::One(
                        ObjectRef::EventSubject,
                    ))),
                    kind: Some(crate::counter::CounterKind::Named(
                        crate::counter::CounterName::LoyaltyCounter,
                    )),
                    information: Information::CurrentOrLastKnown,
                },
            }
        }
        (
            "PermanentWouldAssignDamageDealDamageBeDealDamageOrBecomeTapped",
            [Object(object)],
        ) => {
            let same = || ObjectFilter::SameObject(object.clone());
            ReplacementEvent::Either(vec![
                ReplacementEvent::Object {
                    event: ReplacedObjectEvent::AssignsCombatDamage,
                    objects: same(),
                },
                ReplacementEvent::Damage(DamageEvent {
                    source_kind: ObjectKind::Permanent,
                    sources: same(),
                    recipients: DamageRecipients::Any,
                    kind: DamageKind::Any,
                    grouping: DamageGrouping::SourceRecipient,
                    subject: DamageSubject::Source,
                }),
                pending_damage(
                    ObjectKind::Any,
                    ObjectFilter::Any,
                    DamageRecipients::Objects(same()),
                    DamageKind::Any,
                ),
                ReplacementEvent::Object {
                    event: ReplacedObjectEvent::BecomesTapped,
                    objects: same(),
                },
            ])
        }
        ("APlayerWouldPutAPermanentOnTheBattlefield", [Players(players), Objects(objects)]) => {
            ReplacementEvent::PutOntoBattlefield {
                players: players.clone(),
                objects: event_filter(objects)?,
            }
        }
        ("AnAbilityWouldPutCountersOfTypeOnAPermanent", [by, kind, Objects(objects)]) => {
            ReplacementEvent::Caused {
                cause: stack_filter(by)?,
                event: Box::new(ReplacementEvent::Counters {
                    kind: Some(counter_kind(kind)?),
                    objects: event_filter(objects)?,
                    players: None,
                    by: None,
                    effect_only: false,
                }),
            }
        }
        ("PlayersLifeTotalWouldBeReducedToNumberOrLess", [Player(player), Number(floor)]) => {
            ReplacementEvent::Measured {
                event: Box::new(ReplacementEvent::Player {
                    event: ReplacedPlayerEvent::ReduceLifeTotal,
                    players: PlayerSet::One(player.clone()),
                }),
                comparison: Comparison::LessOrEqual,
                amount: floor.clone(),
            }
        }
        _ => return None,
    })
}

/// What a replacement does in its own words, on the event's object or the source's pile.
fn reduce_card_replacement_action(
    actor: &PlayerSet,
    form: &str,
    operands: &[Operand],
) -> Option<Operation> {
    let on_subject = |action| Operation::Keyword {
        actor: actor.clone(),
        action,
        object: Some(ObjectRef::EventSubject),
        other: None,
        amount: None,
        subtype: None,
    };
    Some(match (form, operands) {
        ("TurnItFaceUpInstead", []) => on_subject(KeywordAction::TurnFaceUp),
        // The body runs first and the damage then lands, so only earlier damage heals.
        ("DealDamageButHealPreviousDamage", []) => on_subject(KeywordAction::RemoveAllDamage),
        // The top card of a shuffled face-down pile is a random one of them.
        ("PutTheTopCardOfTheExiledPileIntoHand", []) => Operation::ChangeZone {
            actor: actor.clone(),
            cards: exiled_with(&ObjectRef::Source),
            count: Some(Count::Exactly(Expr::Constant(1))),
            random: true,
            group: None,
            destination: Destination::OwnersHand,
            entry: vec![],
            face_down: None,
        },
        _ => return None,
    })
}

/// Changes to objects and spells, past `reduce_layer`'s table.
fn reduce_card_layer(form: &str, operands: &[Operand]) -> Option<Change> {
    use Operand::{Ability as Held, Cost as Paid, Filter, Player, Term, Text};
    let grant = |ability| Some(Change::GrantAbility(Box::new(ability)));
    match (form, operands) {
        (
            "AddAbility_ActivatedWithModifiers",
            [Paid(cost), Held(ability), Term { form: modifier, operands: of, .. }],
        ) => {
            let (Ability::Spell { announcement, body }, "OnlyPlayerMayActivate", [Player(player)]) =
                (&**ability, modifier.as_str(), of.as_slice())
            else {
                return None;
            };
            grant(Ability::Activated {
                origin: ActivationOrigin::Ordinary,
                cost: cost.clone(),
                limits: ActivationLimits {
                    once: false,
                    once_each_turn: false,
                    sorcery: false,
                    condition: None,
                    activators: None,
                    keyword: None,
                    modifiers: vec![Change::ActivationRule(ActivationRule::OnlyBy(
                        PlayerSet::One(player.clone()),
                    ))],
                },
                announcement: announcement.clone(),
                body: body.clone(),
            })
        }
        ("Freerunning", symbols) if !symbols.is_empty() => {
            grant(keyword_for(Mechanic::Freerunning, Some(mana_cost(symbols)?), None))
        }
        ("StickerKicker", [Paid(cost)]) => {
            grant(keyword_for(Mechanic::StickerKicker, Some(cost.clone()), None))
        }
        ("AdditionalCostForEachColorManaSymbolInCosts", [Paid(cost), color]) => {
            Some(Change::SpellCost(SpellCostModifier::Increase(repeated_cost(
                cost,
                Expr::CountManaSymbols {
                    objects: Box::new(ObjectSet::One(ObjectRef::Affected)),
                    colors: vec![color_form(color)?],
                },
            ))))
        }
        (
            "AddAbilityFromCardsRemovedFromDraftWithCardsNamedHasable",
            [Filter(cards), Text(card), kinds],
        ) => Some(gain_abilities(
            ObjectSet::OutsideTheGame(ObjectFilter::All(vec![
                cards.clone(),
                ObjectFilter::DraftNoted(draft_note(
                    DraftNoteKind::Removed,
                    &PlayerRef::AbilityController,
                    card,
                )),
            ])),
            kinds,
        )),
        _ => None,
    }
}

/// `cost`, paid `times` times over.
fn repeated_cost(cost: &Cost, times: Expr) -> Cost {
    Cost {
        parts: cost
            .parts
            .iter()
            .map(|part| CostPart::Repeated {
                part: Box::new(part.clone()),
                times: times.clone(),
            })
            .collect(),
    }
}

/// Play permissions on terms, and a rule that names its objects by a result.
fn reduce_card_player_rule(form: &str, operands: &[Operand]) -> Option<PlayerRule> {
    use CardPlayAction as A;
    use EventZone as Z;
    use Operand::{Filter, Objects, Player};
    let terms = play_terms();
    Some(match (form, operands) {
        ("CantSacrificePermanents", [Objects(objects)]) => {
            PlayerRule::Prohibit(PlayerAction::Sacrifice {
                objects: objects_filter(objects),
                paying: None,
            })
        }
        // The card prints casting from the top as its own rule, so this only waives
        // the cost, as Forge's `MayPlayDontGrantZonePermissions$`.
        ("MayCastASpellFromHandOrTopOfLibraryWithoutPayingOnceEachPlayersTurn", [Filter(spells)]) => {
            PlayerRule::SpellCost {
                spells: ObjectFilter::All(vec![
                    spells.clone(),
                    ObjectFilter::InSet(Box::new(ObjectSet::Union(vec![
                        zone_set(Z::Hand, you(), ObjectFilter::Any),
                        top_of_libraries(you()),
                    ]))),
                ]),
                modifier: SpellCostModifier::AlternateDuring {
                    cost: PlayCost::Free,
                    limit: Some(PlayLimit::EachTurn(1)),
                    during: you(),
                },
            }
        }
        (
            "MayCastSpellsFromTopOfLibrary_SpellsWithTrigger",
            [Filter(spells), Filter(triggering), then],
        ) => play_rule(
            top_of_libraries(you()),
            Z::Library,
            A::CastSpell,
            spells.clone(),
            PlayTerms {
                effects: vec![Filter(triggering.clone()), then.clone()],
                ..terms
            },
        ),
        (
            "MayPlayALandOrCastASpellFromAmongCardsInTheirGraveyardThatWerePutThereFromTheirLibraryOnceEachPlayers",
            [Objects(lands), Filter(spells), Filter(cards), Player(player)],
        ) => play_rule(
            zone_set(
                Z::Graveyard,
                you(),
                ObjectFilter::All(vec![
                    cards.clone(),
                    ObjectFilter::ArrivedThisTurn { from: Some(Z::Library) },
                ]),
            ),
            Z::Graveyard,
            A::Either,
            ObjectFilter::Either(vec![event_filter(lands)?, spells.clone()]),
            PlayTerms {
                limit: Some(PlayLimit::EachTurn(1)),
                during: Some(PlayerSet::One(player.clone())),
                ..terms
            },
        ),
        ("MayPlayLandsFromOutsideTheGame", [Filter(cards)]) => play_rule(
            ObjectSet::OutsideTheGame(cards.clone()),
            Z::Outside,
            A::PlayLand,
            ObjectFilter::Any,
            terms,
        ),
        _ => return None,
    })
}

fn reduce_card_expiration(form: &str, operands: &[Operand]) -> Option<Duration> {
    use Operand::{Comparison as Compared, Player, Vocabulary};
    Some(match (form, operands) {
        // A plane stops being face up as it goes back to the planar deck.
        ("AsLongAsPlaneIsFaceUp", [Vocabulary { form, .. }]) if form == "ThisPlane" => {
            Duration::WhileMatches {
                object: ObjectRef::Source,
                filter: ObjectFilter::Not(Box::new(ObjectFilter::State(ObjectState::FaceDown))),
            }
        }
        (
            "UntilPlayerRollsValueWhileRollingToVisitAttractions",
            [Player(player), Compared(comparison, value)],
        ) => Duration::UntilEvent(Box::new(EventSpec::Filtered {
            event: Box::new(EventSpec::Player {
                event: PlayerEvent::RollsToVisit,
                players: PlayerSet::One(player.clone()),
                objects: None,
            }),
            condition: Condition::Compare {
                left: Expr::Dice(DiceMeasure::Result),
                comparison: comparison.clone(),
                right: value.clone(),
            },
        })),
        _ => return None,
    })
}

/// Events a delayed trigger waits for that no trigger row spells.
fn reduce_card_future_event(form: &str, operands: &[Operand]) -> Option<EventSpec> {
    use Operand::{Comparison as Compared, Object, Player, Players};
    Some(match (form, operands) {
        ("WhenAPlayerPlaneswalks", [Players(players)]) => EventSpec::Player {
            event: PlayerEvent::Planeswalks,
            players: players.clone(),
            objects: None,
        },
        // Winning is the other creature leaving while this one stays, as Forge's
        // trigger on leaving the battlefield with `IsPresent$`.
        ("WhenPermanentWinsAFightAgainstPermanent", [Object(winner), Object(loser)]) => {
            EventSpec::Filtered {
                event: Box::new(EventSpec::Object {
                    event: ObjectEvent::LeavesBattlefield,
                    objects: ObjectFilter::SameObject(loser.clone()),
                }),
                condition: Condition::ObjectMatches {
                    object: winner.clone(),
                    filter: ObjectFilter::InZone(ZoneScope::Shared(EventZone::Battlefield)),
                },
            }
        }
        (
            "WhenPlayerNextActivatesAnAbilityBySpendingAnAmountOfMana",
            [Player(player), abilities, Compared(comparison, value)],
        ) => EventSpec::Activated {
            players: PlayerSet::One(player.clone()),
            abilities: ObjectFilter::All(vec![
                stack_filter(abilities)?,
                ObjectFilter::ManaSpent {
                    color: None,
                    sources: None,
                    payers: None,
                    comparison: comparison.clone(),
                    value: value.clone(),
                },
            ]),
        },
        _ => return None,
    })
}

fn reduce_card_mana(form: &str, operands: &[Operand]) -> Option<ManaProduction> {
    use Operand::{Player, Text};
    match (form, operands) {
        ("AnyManaColorChosenByPlayerDuringDraft", [Player(player), Text(card)]) => Some(
            ManaProduction::DraftNotedColor(draft_note(DraftNoteKind::Color, player, card)),
        ),
        _ => None,
    }
}

/// One `StaticLayerEffect` written as a term of a rule.
fn static_layer_term(term: &Operand) -> Option<Change> {
    let Operand::Term {
        family,
        form,
        operands,
    } = term
    else {
        return None;
    };
    Some(match (form.as_str(), operands.as_slice()) {
        ("LosesAllAbilities", []) => Change::RemoveAllAbilities,
        ("SetPowerAndToughnessBoth", [Operand::Number(amount)]) => Change::SetBasePowerToughness {
            power: amount.clone(),
            toughness: amount.clone(),
        },
        _ => match reduce_change(format!("{family}::{form}"), operands.clone()) {
            Change::Primitive { .. } => return None,
            change => change,
        },
    })
}

/// Rules that stand alone as abilities.
fn reduce_card_ability(name: &str, operands: &[Operand]) -> Option<Ability> {
    use Operand::{Condition as Holds, Duration as Lasts, Event, List, Objects, Program, Term, Vocabulary};
    let form = name.strip_prefix("Rule::")?;
    match (form, operands) {
        ("ActivatedAbilityEffect", [abilities, Term { form, operands, .. }]) => {
            let ("AdditionalCostForEachColorManaSymbolInCosts", [Operand::Cost(cost), color]) =
                (form.as_str(), operands.as_slice())
            else {
                return None;
            };
            Some(Ability::Static {
                kind: StaticKind::Ordinary,
                affected: EffectSubjects::Players(PlayerSet::All),
                changes: vec![Change::PlayerRule(PlayerRule::ActivationCost {
                    abilities: stack_filter(abilities)?,
                    change: ActivationCostChange::Additional(repeated_cost(
                        cost,
                        Expr::ActivationCostSymbols(vec![color_form(color)?]),
                    )),
                    floor: None,
                })],
            })
        }
        (
            "EachPermanentAndSpellEffect",
            [Vocabulary { form: which, .. }, Term { form, operands, .. }],
        ) => {
            let ("AnyPermanentOrSpell", "ReplaceAllColorWordsWithNewColorWord", [word]) =
                (which.as_str(), form.as_str(), operands.as_slice())
            else {
                return None;
            };
            Some(Ability::Static {
                kind: StaticKind::Ordinary,
                affected: EffectSubjects::Objects(ObjectSet::Union(vec![
                    ObjectSet::Battlefield(ObjectFilter::Any),
                    ObjectSet::Zone {
                        scope: ZoneScope::Shared(EventZone::Stack),
                        kind: ObjectKind::Spell,
                        filter: ObjectFilter::Any,
                    },
                ])),
                changes: vec![Change::ReplaceWord {
                    from: Box::new(Vocabulary {
                        family: "Color".into(),
                        form: "AnyColor".into(),
                    }),
                    to: Box::new(word.clone()),
                }],
            })
        }
        ("EachPermanentStickyLayerEffect", [Objects(objects), List(effects), Lasts(until)]) => {
            Some(Ability::Static {
                kind: StaticKind::ContinuesAfterLeaving(until.clone()),
                affected: EffectSubjects::Objects(objects.clone()),
                changes: effects
                    .iter()
                    .map(static_layer_term)
                    .collect::<Option<_>>()?,
            })
        }
        // Covercast works from everywhere but the battlefield, as Forge's
        // `TriggerZones$`. "Another" is in the Oracle text, not in the row.
        ("TriggerI_Covercast", [Event(event), Holds(condition), Program(body)]) => {
            let mut event = (**event).clone();
            let EventSpec::Spell { spells, .. } = &mut event else {
                return None;
            };
            *spells = ObjectFilter::All(vec![
                spells.clone(),
                ObjectFilter::Not(Box::new(ObjectFilter::SameObject(ObjectRef::Source))),
            ]);
            Some(Ability::Scoped {
                zones: vec![
                    EventZone::Hand,
                    EventZone::Library,
                    EventZone::Graveyard,
                    EventZone::Exile,
                    EventZone::Stack,
                ],
                condition: None,
                ability: Box::new(Ability::Triggered {
                    event,
                    intervening_if: Some(condition.clone()),
                    limits: TriggerLimits::default(),
                    announcement: Announcement {
                        targets: vec![],
                        modes: None,
                    },
                    body: body.clone(),
                }),
            })
        }
        ("TriggerModalA", [List(options)]) => Some(Ability::Triggers(
            options
                .iter()
                .map(|option| match option {
                    Operand::Ability(ability) => Some((**ability).clone()),
                    _ => None,
                })
                .collect::<Option<_>>()?,
        )),
        _ => None,
    }
}
