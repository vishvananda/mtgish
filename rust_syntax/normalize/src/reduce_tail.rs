/// One-card-in-a-set actions that still have a general shape: a choice with terms,
/// a move, a reveal, a counter placement or a wrapper around another action.
fn reduce_tail_actions(actor: &PlayerSet, form: &str, operands: &[Operand]) -> Option<Operation> {
    use Destination as D;
    use EventZone as Z;
    use Operand::{
        Event, Filter, List, Number, Object, Objects, Player, Players, Term,
        Text, Vocabulary,
    };
    let you = || actor_of(actor);
    let keyword = |who: PlayerSet, action, object: Option<ObjectRef>, amount| Operation::Keyword {
        actor: who,
        action,
        object,
        other: None,
        amount,
        subtype: None,
    };
    let choose_with = |kind, terms| Operation::ChooseValueWith {
        actor: actor.clone(),
        kind,
        terms,
    };
    let choose_value = |kind| Operation::ChooseValue {
        actor: actor.clone(),
        kind,
        count: 1,
        options: vec![],
    };
        let reveal_and_discard = |cards: &ObjectFilter, count: Expr| {
        Operation::Together(vec![
            Operation::RevealFromHand {
                actor: actor.clone(),
                cards: ObjectFilter::Any,
                count: Some(count),
                at_random: true,
            },
            Operation::DiscardObjects {
                objects: ObjectSet::Matching {
                    objects: Box::new(ObjectSet::Earlier {
                        what: EarlierObjects::Revealed,
                        by: None,
                    }),
                    filter: cards.clone(),
                },
            },
        ])
    };
    let revealing_until = |library, filter: &ObjectFilter, count| Operation::RevealUntil {
        library,
        filter: filter.clone(),
        count,
        at_most: None,
    };
    let each = |players: &PlayerSet, action| Operation::EachPlayer {
        players: players.clone(),
        action: Box::new(action),
    };
    Some(match (form, operands) {
        ("RemoveAllDamageFromPermanent", [Object(object)]) => keyword(
            actor.clone(),
            KeywordAction::RemoveAllDamage,
            Some(object.clone()),
            None,
        ),
        ("PutACardFromOutsideGameInHand", [Filter(cards)]) => Operation::Wish {
            actor: actor.clone(),
            filter: cards.clone(),
            reveal: false,
        },
        (
            "ForEachPermanentConjureCardOntoBattlefield",
            [Objects(objects), Text(card), List(entry)],
        ) => Operation::EachObject {
            objects: objects.clone(),
            action: Box::new(Operation::Conjure {
                actor: actor.clone(),
                card: ConjuredCard::Named(card.clone()),
                count: Expr::Constant(1),
                zone: Z::Battlefield,
                owner: None,
                spot: None,
                entry: entry.clone(),
            }),
        },
        (
            "GainControlOfAPermanentControlledByEachPlayerUntil",
            [Objects(objects), Players(players), Operand::Duration(until)],
        ) => each(
            players,
            Operation::GiveControl {
                players: actor.clone(),
                objects: ObjectSet::Chosen {
                    chooser: actor.clone(),
                    objects: Box::new(ObjectSet::Matching {
                        objects: Box::new(objects.clone()),
                        filter: ObjectFilter::ControlledBy(PlayerSet::One(PlayerRef::Acting)),
                    }),
                    count: Count::Exactly(Expr::Constant(1)),
                },
                until: Some(until.clone()),
            },
        ),
        ("PutACommanderFromCommandZoneOntoBattlefield", [commanders, List(entry)]) => {
            Operation::ChangeZone {
                actor: actor.clone(),
                cards: commanders_in(Z::Command, Some(commanders), actor)?,
                count: Some(Count::UpTo(Expr::Constant(1))),
                random: false,
                group: None,
                destination: D::OwnersBattlefield,
                entry: entry.clone(),
                face_down: None,
            }
        }
        (
            "PermanentDealsDamageDividedAsPlayerChooses",
            [Object(source), Number(amount), Operand::Recipients(recipients), Player(chooser)],
        ) => Operation::DealDamageWith {
            source: source.clone(),
            recipients: recipients.clone(),
            amount: amount.clone(),
            rider: DamageRider::DividedBy(chooser.clone()),
        },
        (
            "MayCastASpellFromAmongGraveyardCardsIntoExileOrBattlefield",
            [Filter(spells), Filter(cards)],
        ) => cast_with(
            actor,
            CardPlayAction::CastSpell,
            ObjectSet::AnyZone(ObjectFilter::All(vec![spells.clone(), cards.clone()])),
            Count::UpTo(Expr::Constant(1)),
            PlayTerms {
                into_exile: true,
                ..play_terms()
            },
            None,
        ),
        ("ChooseACardtypeExceptFromList", except) if !except.is_empty() => choose_with(
            ValueKind::CardType,
            ChoiceTerms {
                except: except.to_vec(),
                ..Default::default()
            },
        ),
        ("ChooseACardtypeSharedAmongExiledCards", [cards @ Objects(_)]) => choose_with(
            ValueKind::CardType,
            ChoiceTerms {
                options: vec![cards.clone()],
                ..Default::default()
            },
        ),
        ("ChooseNumberAbilities", [Number(count), List(options)]) => choose_with(
            ValueKind::Ability,
            ChoiceTerms {
                options: options.clone(),
                count: Some(count.clone()),
                ..Default::default()
            },
        ),
        ("ChooseUptoNumberColors" | "CircleNumberColors", [Number(count), colors @ Term { .. }]) => {
            choose_with(
                ValueKind::Color,
                ChoiceTerms {
                    options: vec![colors.clone()],
                    count: Some(count.clone()),
                    up_to: form == "ChooseUptoNumberColors",
                    ..Default::default()
                },
            )
        }
        ("ChooseAColorAndCreatureType", pairs) if !pairs.is_empty() => Operation::ChooseValue {
            actor: actor.clone(),
            kind: ValueKind::ColorAndCreatureType,
            count: 1,
            options: pairs.to_vec(),
        },
        ("ChooseALandTypeAndABasicLandType", []) => Operation::Together(vec![
            choose_value(ValueKind::LandType),
            choose_value(ValueKind::BasicLandType),
        ]),
        ("SwapWordChoice", [Text(_), Text(_)]) => Operation::ChooseValue {
            actor: actor.clone(),
            kind: ValueKind::Word,
            count: 1,
            options: operands.to_vec(),
        },
        ("ChooseAPlayerOrPlaneswalkerCurrentlyAttackedByPlayer", [Player(attacker)]) => {
            Operation::ChooseDefender {
                actor: actor.clone(),
                attacker: attacker.clone(),
            }
        }
        ("ChooseASpellThatResolvedThisTurn", [Filter(spells)]) => Operation::ChooseObjects {
            player: you(),
            objects: spells_cast_this_turn_set(spells),
            count: Count::Exactly(Expr::Constant(1)),
            constraints: vec![],
        },
        ("ChooseACommanderOnTheBattlefieldOrInTheCommandZone", [commanders]) => {
            Operation::ChooseObjects {
                player: you(),
                objects: ObjectSet::Union(vec![
                    commanders_in(Z::Battlefield, Some(commanders), actor)?,
                    commanders_in(Z::Command, Some(commanders), actor)?,
                ]),
                count: Count::Exactly(Expr::Constant(1)),
                constraints: vec![],
            }
        }
        ("VoteForColor", colors) if !colors.is_empty() => Operation::Vote {
            actor: actor.clone(),
            ballot: Ballot::Words(
                colors
                    .iter()
                    .map(|color| match color {
                        Vocabulary { form, .. } => Some(form.clone()),
                        _ => None,
                    })
                    .collect::<Option<_>>()?,
            ),
            secret: false,
        },
        ("LookAtAPlayersHand", [Players(players)]) => Operation::LookAt {
            player: you(),
            objects: zone_set(
                Z::Hand,
                PlayerSet::One(PlayerRef::OneOf(Box::new(players.clone()))),
                ObjectFilter::Any,
            ),
        },
        ("MillNumberCardsForEach", [Number(amount), Number(each)]) => Operation::Mill {
            players: actor.clone(),
            amount: Expr::multiply(amount.clone(), each.clone()),
        },
        ("RollTwoD6", []) => Operation::Repeat {
            times: Expr::Constant(2),
            body: Box::new(keyword(actor.clone(), KeywordAction::RollDie(6), None, None)),
        },
        ("ExileUptoNumberGraveyardCards", [Number(count), Filter(cards)]) => Operation::ChangeZone {
            actor: actor.clone(),
            cards: ObjectSet::AnyZone(cards.clone()),
            count: Some(Count::UpTo(count.clone())),
            random: false,
            group: None,
            destination: D::Exile,
            entry: vec![],
            face_down: None,
        },
        ("PayAnyAmountOfLifeUpto", [Number(at_most)]) => Operation::PayRepeatedly {
            player: you(),
            cost: Cost {
                parts: vec![CostPart::Life(Expr::Constant(1))],
            },
            at_most: at_most.clone(),
        },
        ("TurnEachPermanentFaceDown", [Objects(objects)]) => Operation::KeywordEach {
            actor: actor.clone(),
            action: KeywordAction::TurnFaceDown,
            objects: objects.clone(),
            until: None,
        },
        ("ExchangeTextBoxesOfTwoPermanents", [first @ Object(_), second @ Object(_)]) => {
            Operation::Exchange {
                kind: ExchangeKind::TextBoxes,
                first: Box::new(first.clone()),
                second: Box::new(second.clone()),
                until: None,
            }
        }
        ("ExchangeOwnershipOfTwoCards", [first @ Object(_), Vocabulary { .. }]) => {
            Operation::Exchange {
                kind: ExchangeKind::Ownership,
                first: Box::new(first.clone()),
                second: Box::new(Object(ObjectRef::Source)),
                until: None,
            }
        }
        ("NoteTheMostPrevalentCreaturTypeInAPlayersLibrary", [Players(players)]) => {
            Operation::Note(NoteKind::MostCommonCreatureType(players.clone()))
        }
        ("ReorderPlayersGraveyard", [Player(player)]) => keyword(
            PlayerSet::One(player.clone()),
            KeywordAction::ReorderGraveyard,
            None,
            None,
        ),
        ("IntensifyCard", [Vocabulary { .. }]) => Operation::Intensify {
            cards: ObjectFilter::SameObject(ObjectRef::Source),
            owner: you(),
        },
        ("PerpetuallyIncreaseIntensityOfPermanent", [Object(object), Number(amount)]) => keyword(
            actor.clone(),
            KeywordAction::Intensify,
            Some(object.clone()),
            Some(amount.clone()),
        ),
        ("SetSchemeInMotion", [Vocabulary { .. }]) => keyword(
            actor.clone(),
            KeywordAction::SetSchemeInMotion,
            Some(ObjectRef::Source),
            None,
        ),
        ("SetRepeatablePlayers", [Players(players)]) => keyword(
            players.clone(),
            KeywordAction::SetRepeatablePlayers,
            None,
            None,
        ),
        ("TurnAnExiledPileFaceUp", []) => {
            keyword(actor.clone(), KeywordAction::TurnPileFaceUp, None, None)
        }
        ("ActivateAManaAbilityOfEachPermanentAndLoseUnspentMana", [Objects(objects)]) => {
            Operation::Together(vec![
                Operation::KeywordEach {
                    actor: actor.clone(),
                    action: KeywordAction::ActivateManaAbility,
                    objects: objects.clone(),
                    until: None,
                },
                keyword(actor.clone(), KeywordAction::LoseUnspentMana, None, None),
            ])
        }
        ("ShuffleHandAndPermanentsIntoLibrary", []) => Operation::ChangeZone {
            actor: actor.clone(),
            cards: ObjectSet::Union(vec![
                zone_set(Z::Hand, actor.clone(), ObjectFilter::Any),
                ObjectSet::Battlefield(ObjectFilter::OwnedBy(actor.clone())),
            ]),
            count: None,
            random: false,
            group: None,
            destination: D::OwnersLibraryShuffled,
            entry: vec![],
            face_down: None,
        },
        ("CloakEachExiledCard", [Objects(cards), List(entry)]) => Operation::Manifest {
            actor: actor.clone(),
            source: ManifestSource::Cards(cards.clone()),
            cloak: true,
            entry: entry.clone(),
        },
        ("PopulateWithFlags", entry) => Operation::Populate {
            actor: actor.clone(),
            entry: entry.to_vec(),
        },
        ("ReducePlayersSpeedToNoLessThan", [Player(player), Number(amount), Number(floor)]) => {
            Operation::ReduceSpeed {
                player: player.clone(),
                amount: amount.clone(),
                floor: floor.clone(),
            }
        }
        ("CounterSpellAndSpellsOfTypeAreCounteredOntoTheBattlefield", [Object(object), Filter(when), List(entry)]) => {
            Operation::CounterWith {
                object: object.clone(),
                when: Some(when.clone()),
                destination: D::Battlefield(you()),
                entry: entry.clone(),
            }
        }
        (
            "CounterSpellIntoExileWithANumberOfCountersAndWithEffects",
            [Object(object), amount @ Number(_), kind, effects @ List(_)],
        ) => Operation::CounterWith {
            object: object.clone(),
            when: None,
            destination: D::Exile,
            entry: vec![amount.clone(), kind.clone(), effects.clone()],
        },
        ("PutCountersWithRestriction", [List(actions), Term { form, operands: limit, .. }])
            if form == "CantCauseTheNumberOfCountersOfTypeOnPermanentToBeGreaterThan" =>
        {
            let [Term { form: action, operands: put, .. }] = actions.as_slice() else {
                return None;
            };
            let ([Number(amount), kind, Object(object)], [_, _, Number(at_most)], true) = (
                put.as_slice(),
                limit.as_slice(),
                action == "UptoNumberCountersOfTypeOnPermanent",
            ) else {
                return None;
            };
            Operation::PutCountersCapped {
                amount: amount.clone(),
                kind: counter_kind(kind)?,
                object: object.clone(),
                at_most: at_most.clone(),
            }
        }
        ("GetANumberTimeBoon", [Number(times), Event(event), body]) => Operation::CreateBoon {
            actor: actor.clone(),
            event: event.clone(),
            condition: None,
            times: times.clone(),
            body: Box::new(body.clone()),
        },
        ("EachPlayerCreatesTokensOfTheirChoice", [Players(players), options @ ..]) => {
            Operation::ChooseAction {
                actor: players.clone(),
                options: options.to_vec(),
                villainous: false,
            }
        }
        ("RevealACardFromHandAtRandomAndDiscardIfItIsACardOfType", [Filter(cards)]) => {
            reveal_and_discard(cards, Expr::Constant(1))
        }
        ("RevealNumberCardsFromHandAtRandomAndDiscardEachCardOfType", [Number(count), Filter(cards)]) => {
            reveal_and_discard(cards, count.clone())
        }
        ("RevealHandAndPlayerChoosesMultipleCardsToDiscard", [Player(chooser), List(filters)]) => {
            Operation::Together(
                filters
                    .iter()
                    .map(|filter| match filter {
                        Filter(filter) => Some(Operation::PickFromHand {
                            actor: PlayerSet::One(chooser.clone()),
                            player: you(),
                            filter: filter.clone(),
                            count: Count::Exactly(Expr::Constant(1)),
                            then: HandPick::Discard,
                        }),
                        _ => None,
                    })
                    .collect::<Option<_>>()?,
            )
        }
        ("RevealCardFromOutsideGameAndPutInHandOrPutAnExiledCardInOwnersHand", [Filter(cards), Objects(exiled)]) => {
            Operation::ChangeZone {
                actor: actor.clone(),
                cards: ObjectSet::Union(vec![
                    ObjectSet::OutsideTheGame(cards.clone()),
                    exiled.clone(),
                ]),
                count: Some(Count::UpTo(Expr::Constant(1))),
                random: false,
                group: None,
                destination: D::OwnersHand,
                entry: vec![],
                face_down: None,
            }
        }
        ("RevealNumberGroupCardsFromOutsideGame", [Number(count), Filter(cards), group]) => {
            Operation::ChooseObjects {
                player: you(),
                objects: ObjectSet::OutsideTheGame(cards.clone()),
                count: Count::Exactly(count.clone()),
                constraints: vec![group_constraint(group)?],
            }
        }
        (
            "EachPlayerRevealsCardsFromTheTopOfLibraryUntilANumberOfCardsOfTypeAreRevealed",
            [Players(players), Number(count), Filter(cards)],
        ) => each(players, revealing_until(PlayerRef::Acting, cards, count.clone())),
        ("MillCardsUntilACardOfTypeIsMilledOrUntilNumberCardsHaveBeenPutIntoGraveyardThisWay", [Filter(cards), Number(at_most)]) => {
            Operation::MillUntil {
                actor: actor.clone(),
                cards: cards.clone(),
                at_most: at_most.clone(),
            }
        }
        ("PermanentDoesntUntapDuringControllersNextNumberUntaps", [Object(object), Number(steps)]) => {
            Operation::EffectOn {
                subject: EffectSubject::Objects(ObjectSet::One(object.clone())),
                effects: vec![Vocabulary {
                    family: "PermanentRule".into(),
                    form: "DoesntUntapDuringControllersUntap".into(),
                }],
                until: Some(Duration::DuringControllersNextUntapSteps(steps.clone())),
            }
        }
        ("CreatureMustAttackDuringControllersNextCombatPhase", [Object(object)]) => {
            Operation::EffectOn {
                subject: EffectSubject::Objects(ObjectSet::One(object.clone())),
                effects: vec![Vocabulary {
                    family: "PermanentRule".into(),
                    form: "AttacksEachCombatIfAble".into(),
                }],
                until: Some(Duration::UntilNext {
                    point: TurnPoint::CombatEnds,
                    players: PlayerSet::One(PlayerRef::ControllerOf(Box::new(object.clone()))),
                }),
            }
        }
        _ => return None,
    })
}

/// Would-be events with no row in the player or object tables.
fn reduce_tail_replacement(name: &str, operands: &[Operand]) -> Option<ReplacementEvent> {
    use Operand::{Comparison as Compared, Filter, Object, Objects, Player, Players, Vocabulary};
    let form = name.strip_prefix("ReplacableEvent")?;
    let graveyard_card = |filter: &ObjectFilter, player: &PlayerRef| {
        ObjectFilter::All(vec![
            filter.clone(),
            ObjectFilter::OwnedBy(PlayerSet::One(player.clone())),
            ObjectFilter::Not(Box::new(history(ObjectHistory::Cycled, ObjectWindow::ThisGame))),
        ])
    };
    Some(match (form, operands) {
        (
            "WouldEnter::PermanentWouldEnterTheBattlefieldAndWasntCastOrNoManaWasSpentToCast",
            [Object(object)],
        ) => ReplacementEvent::Enter {
            objects: ObjectFilter::All(vec![
                ObjectFilter::SameObject(object.clone()),
                ObjectFilter::Either(vec![
                    ObjectFilter::Not(Box::new(ObjectFilter::Cast {
                        by: None,
                        from: None,
                    })),
                    ObjectFilter::ManaSpent {
                        color: None,
                        sources: None,
                        payers: None,
                        comparison: Comparison::Equal,
                        value: Expr::Constant(0),
                    },
                ]),
            ]),
        },
        (
            "WouldPutIntoGraveyard::WouldPutACardInPlayersGraveyardFromAnywhereNotCycled",
            [Filter(cards), Player(owner)],
        ) => ReplacementEvent::ZoneChange {
            objects: graveyard_card(cards, owner),
            from: None,
            to: EventZone::Graveyard,
        },
        (
            "TokensWouldBeCreatedUnderAPlayersControl::TokensWouldBeCreatedUnderAPlayersControl",
            [Players(players)],
        ) => ReplacementEvent::Tokens {
            tokens: ObjectFilter::Any,
            controllers: Some(players.clone()),
            creators: None,
            effect_only: false,
            each: false,
        },
        ("WouldPayLife::APlayerWouldPayAnAmountOfLife", [Players(players), Compared(comparison, amount)]) => {
            ReplacementEvent::Measured {
                event: Box::new(ReplacementEvent::Player {
                    event: ReplacedPlayerEvent::PayLife,
                    players: players.clone(),
                }),
                comparison: comparison.clone(),
                amount: amount.clone(),
            }
        }
        ("WouldCopyASpell::APlayerWouldCopyASpellAnyNumberOfTimes", [Players(players), Filter(spells)]) => {
            ReplacementEvent::CopySpell {
                players: players.clone(),
                spells: spells.clone(),
            }
        }
        ("WouldCounterASpell::ASpellOrAbilityWouldCounterASpell", [by, Filter(spells)]) => {
            ReplacementEvent::Countered {
                spells: spells.clone(),
                by: stack_filter(by)?,
            }
        }
        (
            "WouldDraw::ACyclingAbilityOfAPermanentWouldCausePlayerToDrawACard",
            [Objects(sources), Player(player)],
        ) => ReplacementEvent::DrawCausedBy {
            players: PlayerSet::One(player.clone()),
            abilities: ObjectFilter::AbilityOf(Box::new(event_filter(sources)?)),
        },
        ("WouldGainLife::ASpellOrAbilityWouldCauseItsControllerToGainLife", [_]) => {
            ReplacementEvent::Player {
                event: ReplacedPlayerEvent::GainLifeByEffect,
                players: PlayerSet::All,
            }
        }
        ("WouldProduceMana::ASpellOrAbilityWouldProduceColoredMana", [by]) => {
            ReplacementEvent::ProduceMana {
                sources: stack_filter(by)?,
                by: None,
            }
        }
        (
            "WouldProduceMana::TappingPermanentWouldProduceManaOfColor",
            [Object(object), Vocabulary { form: color, .. }],
        ) => ReplacementEvent::ProduceColor {
            sources: ObjectFilter::SameObject(object.clone()),
            color: match color.as_str() {
                "White" => ManaColor::White,
                "Blue" => ManaColor::Blue,
                "Black" => ManaColor::Black,
                "Red" => ManaColor::Red,
                "Green" => ManaColor::Green,
                _ => return None,
            },
        },
        _ => return None,
    })
}

/// Predicates that read a result, a history or a single vocabulary term.
fn reduce_tail_filter(form: &str, operands: &[Operand]) -> Option<ObjectFilter> {
    use Operand::{Filter, Number, Object, Objects, Player, Players, Result as Run, Term};
    let earlier = |what| ObjectSet::Earlier { what, by: None };
    let earlier_by = |what, player: &PlayerRef| ObjectSet::Earlier {
        what,
        by: Some(PlayerSet::One(player.clone())),
    };
    let among = |set| ObjectFilter::InSet(Box::new(set));
    let not = |filter| ObjectFilter::Not(Box::new(filter));
    let exiled_by = |player: PlayerRef| ObjectFilter::ExiledBy {
        player,
        with: None,
        this_turn: false,
    };
    Some(match (form, operands) {
        ("AttachedToAPermanent", [Objects(hosts)]) => {
            ObjectFilter::AttachedTo(Box::new(among(hosts.clone())))
        }
        ("HadCountersOfTypePutOnItByAPlayerThisTurn", [kind, Players(by)]) => history(
            ObjectHistory::CountersPutBy {
                kind: counter_kind(kind)?,
                by: by.clone(),
            },
            ObjectWindow::ThisTurn,
        ),
        ("PermanentPutACounterOfTypeOnIt", [Object(with), kind]) => history(
            ObjectHistory::CountersPutWith {
                kind: counter_kind(kind)?,
                with: with.clone(),
            },
            ObjectWindow::ThisGame,
        ),
        ("PermanentHasNotRemovedACounterOfTypeFromIt", [Object(with), kind]) => not(history(
            ObjectHistory::CountersRemovedWith {
                kind: counter_kind(kind)?,
                with: with.clone(),
            },
            ObjectWindow::ThisGame,
        )),
        ("WasSurveilledThisTurn", []) => history(ObjectHistory::Surveilled, ObjectWindow::ThisTurn),
        ("TargetAbility" | "TargetSpellOrAbility", [term]) => stack_filter(term)?,
        ("HasAnActivatedAbilityOtherThanThisActivatedAbility", []) => {
            ObjectFilter::HasActivatedAbility(ActivationFilter::Not(Box::new(ActivationFilter::Of(
                Box::new(ObjectFilter::SameObject(ObjectRef::Source)),
            ))))
        }
        ("HasntBeenPhasedOutWithThisAbility", []) => not(history(
            ObjectHistory::PhasedOutWith(ObjectRef::Source),
            ObjectWindow::ThisGame,
        )),
        ("InThePermanentPileChosenThisWay", []) => ObjectFilter::InPile(None),
        ("ColorsWithMostVotesOrTiedForMostVotes", []) => ObjectFilter::State(ObjectState::MostVotes),
        ("IsLandTypeVariable", [Term { form, .. }]) if form == "TheFirstChosenLandType" => {
            ObjectFilter::NthChosen(0)
        }
        ("IsLandTypeVariable", [Term { form, .. }]) if form == "TheSecondChosenLandType" => {
            ObjectFilter::NthChosen(1)
        }
        ("HasTapSelfInCost", []) => {
            ObjectFilter::HasActivatedAbility(ActivationFilter::Is(StackTrait::TapSymbolInCost))
        }
        ("Enchant", [Objects(hosts)]) => ObjectFilter::CouldEnchant(Box::new(hosts.clone())),
        ("CanEnchantThatEnteringPermanent", []) => {
            ObjectFilter::CouldEnchant(Box::new(ObjectSet::One(ObjectRef::EventSubject)))
        }
        ("HasAbility", [Term { form, operands, .. }]) => {
            match reduce_filter(format!("CheckHasable::{form}"), operands.clone()) {
                ObjectFilter::Primitive { .. } => return None,
                filter => filter,
            }
        }
        ("IsNamed", [Term { form, .. }]) if form == "TheNameOfTheSacrificedCreature" => {
            ObjectFilter::SharesNameWith(ObjectRef::Earlier {
                what: EarlierObjects::Sacrificed,
                by: None,
            })
        }
        ("WasExiledThisWay", []) => among(earlier(EarlierObjects::Exiled)),
        ("WasTurnedFaceUpThisWay", []) => among(earlier(EarlierObjects::It)),
        ("WasExiledByPlayerThisWay", [Player(player), Run(result)]) => ObjectFilter::All(vec![
            ObjectFilter::InResult(result.clone()),
            exiled_by(player.clone()),
        ]),
        ("WasExiledWithAnAbility", [Term { operands, .. }]) => {
            let [Players(players)] = operands.as_slice() else {
                return None;
            };
            exiled_by(PlayerRef::OneOf(Box::new(players.clone())))
        }
        ("WasPutIntoGraveyardByPlayerThisWay", [Player(player)]) => {
            among(earlier_by(EarlierObjects::PutInGraveyard, player))
        }
        ("WasntPutIntoGraveyardThisWay", []) => not(among(earlier(EarlierObjects::PutInGraveyard))),
        ("DiedThisTurn", [Objects(creatures)]) => ObjectFilter::All(vec![
            event_filter(creatures)?,
            history(ObjectHistory::DiedOrPutFromBattlefield, ObjectWindow::ThisTurn),
        ]),
        ("TheTopNumberCardsOfTypeInPlayersGraveyard", [Number(count), Filter(cards), Player(player)]) => {
            among(ObjectSet::Take {
                objects: Box::new(zone_set(
                    EventZone::Graveyard,
                    PlayerSet::One(player.clone()),
                    cards.clone(),
                )),
                count: count.clone(),
                from: SequenceEnd::Top,
            })
        }
        ("SharesAColorWithExiledCard", [Object(card)]) => ObjectFilter::SharesColorWith(card.clone()),
        ("SharesACardtypeWithTopOfAnyPlayersLibrary", [Players(players)]) => {
            ObjectFilter::SharesWithAny {
                property: DistinctProperty::CardTypes,
                with: Box::new(ObjectSet::LibraryTops(players.clone())),
            }
        }
        ("SharesACreatureTypeWithYourCommander", []) => ObjectFilter::SharesWithAny {
            property: DistinctProperty::CreatureTypes,
            with: Box::new(ObjectSet::AnyZone(ObjectFilter::All(vec![
                ObjectFilter::State(ObjectState::Commander),
                ObjectFilter::OwnedBy(PlayerSet::One(PlayerRef::AbilityController)),
            ]))),
        },
        ("SharesACreatureTypeWithMostPrevalentCreatureTypeInPlayersLibrary", [Player(player)]) => {
            ObjectFilter::MostCommonCreatureTypeIn(player.clone())
        }
        ("SharesANameWithACardInHandRevealedThisWay", []) => {
            ObjectFilter::SharesNameWithAny(Box::new(earlier(EarlierObjects::Revealed)))
        }
        ("SharesANameWithAnotherCardInHandRevealedThisWay", [Run(result)]) => {
            ObjectFilter::SharesNameWithAnother(Box::new(ObjectSet::Observed(result.clone())))
        }
        ("ManaAmountOfTypeWasSpentToCastIt", [Operand::Comparison(comparison, value), Operand::Vocabulary { form, .. }])
            if form == "Colorless" =>
        {
            ObjectFilter::ManaSpent {
                color: Some(ManaColor::Colorless),
                sources: None,
                payers: None,
                comparison: comparison.clone(),
                value: value.clone(),
            }
        }
        _ => return None,
    })
}

/// A replacement's own action, read as the action of the same name where it has one.
pub fn reduce_replacement_action(
    actor: PlayerSet,
    name: String,
    operands: Vec<Operand>,
) -> Operation {
    let plain = name.split_once("::").and_then(|(_, form)| {
        reduce_tail_actions(&actor, form, &operands)
            .or_else(|| reduce_card_replacement_action(&actor, form, &operands))
            .or_else(|| reduce_card_actions(&actor, form, &operands))
            .or_else(|| reduce_last_replacement_action(&actor, form, &operands))
    });
    plain.unwrap_or(Operation::Primitive {
        actor,
        name,
        operands,
    })
}

fn reduce_tail_condition(form: &str, operands: &[Operand]) -> Option<Condition> {
    use Operand::Term;
    let matches = |object, filter| Condition::ObjectMatches { object, filter };
    Some(match (form, operands) {
        ("PermanentPutInGraveyardPassesFilter", [Term { form, .. }]) if form == "WasntSacrificed" => {
            matches(
                ObjectRef::EventSubject,
                ObjectFilter::Not(Box::new(history(
                    ObjectHistory::Sacrificed,
                    ObjectWindow::ThisTurn,
                ))),
            )
        }
        ("PermanentPutInGraveyardPassesFilter", [cards]) => {
            matches(ObjectRef::EventSubject, card_term(cards)?)
        }
        ("CastSpellOrActivatedAbilityPassesFilter", [Term { form, .. }])
            if form == "ManaFromATeasureWasSpentToCastItOrActivateIt" =>
        {
            matches(
                ObjectRef::EventSubject,
                ObjectFilter::ManaSpentFrom(Box::new(ObjectFilter::Subtype(
                    crate::subtype::artifact_type(&crate::mtg_types::ArtifactType::Treasure),
                ))),
            )
        }
        _ => return None,
    })
}

fn reduce_tail_number(form: &str, operands: &[Operand]) -> Option<Expr> {
    use Operand::{Object, Players};
    Some(match (form, operands) {
        ("ANumberOfCardsInAPlayersHand", [Players(players)]) => {
            Expr::CountObjects(Box::new(zone_set(
                EventZone::Hand,
                PlayerSet::One(PlayerRef::OneOf(Box::new(players.clone()))),
                ObjectFilter::Any,
            )))
        }
        ("TheNumberOfManaSymbolsInManaCostOfSpell", [Object(spell)]) => Expr::CountManaSymbols {
            objects: Box::new(ObjectSet::One(spell.clone())),
            colors: vec![ManaColor::Blue],
        },
        _ => return None,
    })
}

fn reduce_tail_rule(form: &str, operands: &[Operand]) -> Option<PlayerRule> {
    use CardPlayAction as A;
    use EventZone as Z;
    use Operand::{Cost as Paid, Filter, Player, Term};
    let card = |term: &Operand| match term {
        Term { form, operands, .. } => {
            match reduce_filter(format!("CardsInGraveyards::{form}"), operands.clone()) {
                ObjectFilter::Primitive { .. } => None,
                filter => Some(filter),
            }
        }
        _ => None,
    };
    let graveyard = |owner: &PlayerRef, cards: ObjectFilter| {
        zone_set(Z::Graveyard, PlayerSet::One(owner.clone()), cards)
    };
    Some(match (form, operands) {
        ("MayCastSpellsFromAmongCardsInPlayersGraveyard", [Filter(spells), cards, Player(owner)]) => {
            play_rule(
                graveyard(owner, card(cards)?),
                Z::Graveyard,
                A::CastSpell,
                spells.clone(),
                play_terms(),
            )
        }
        (
            "MayCastSpellsFromAmongCardsInPlayersGraveyardForAlternateCost",
            [Filter(spells), cards, Player(owner), cost @ Paid(_)],
        ) => play_rule(
            graveyard(owner, card(cards)?),
            Z::Graveyard,
            A::CastSpell,
            spells.clone(),
            PlayTerms {
                cost: PlayCost::Alternate(cost.clone()),
                ..play_terms()
            },
        ),
        ("MayPlayLandsFromAmongCardsInPlayersGraveyard", [cards, Player(owner)]) => play_rule(
            graveyard(owner, card(cards)?),
            Z::Graveyard,
            A::PlayLand,
            ObjectFilter::Any,
            play_terms(),
        ),
        ("CanOnlyUntapCardsOfTypeOfTheirChoiceDuringTheirUntapStep", types) if !types.is_empty() => {
            PlayerRule::UntapOnlyOneOfTypes(types.to_vec())
        }
        _ => return None,
    })
}

/// Steps of a look, reveal or search that say what happens to the cards in hand.
fn tail_inspection_step(
    actor: &PlayerSet,
    form: &str,
    cards: &ObjectSet,
    rest: &[Operand],
) -> Option<Operation> {
    use Destination as D;
    use Operand::{Filter, List, Number, Object, Objects, Player, Players};
    let you = || actor_of(actor);
    if form == "ExileResolvingSpell" {
        return Some(Operation::Move {
            objects: cards.clone(),
            destination: D::Exile,
            entry: EntrySpec::default(),
            face_down: false,
        });
    }
    let put = |cards: ObjectSet, destination, entry: &[Operand]| Operation::ChangeZone {
        actor: actor.clone(),
        cards,
        count: None,
        random: false,
        group: None,
        destination,
        entry: entry.to_vec(),
        face_down: None,
    };
    let picked = |count| ObjectSet::Chosen {
        chooser: actor.clone(),
        objects: Box::new(cards.clone()),
        count,
    };
    let set_aside = || ObjectSet::Earlier {
        what: EarlierObjects::Selected,
        by: None,
    };
    Some(match (form, rest) {
        ("ManifestAGenericCard", []) => Operation::Manifest {
            actor: actor.clone(),
            source: ManifestSource::Cards(picked(Count::Exactly(Expr::Constant(1)))),
            cloak: false,
            entry: vec![],
        },
        ("CloakNumberGenericCards", [Number(count)]) => Operation::Manifest {
            actor: actor.clone(),
            source: ManifestSource::Cards(picked(Count::Exactly(count.clone()))),
            cloak: true,
            entry: vec![],
        },
        ("ConjureADuplicateOfCardOntoTheBattlefield", [Object(card), List(entry)]) => {
            Operation::Conjure {
                actor: actor.clone(),
                card: ConjuredCard::Duplicate(ObjectSet::One(card.clone())),
                count: Expr::Constant(1),
                zone: EventZone::Battlefield,
                owner: None,
                spot: None,
                entry: entry.clone(),
            }
        }
        ("PutRemainingCardsOnTheTopOrBottomOfLibraryInAnyOrder", []) => {
            put(cards.clone(), D::OwnersLibraryTopOrBottom, &[])
        }
        ("PutTheRemainingCardsOnTheBottomOfLibraryInAnyOrder", []) => {
            put(cards.clone(), D::OwnersLibraryBottom, &[])
        }
        ("ShuffleAndPutTheRemainingCardsOnTopOfLibraryInAnyOrder", []) => Operation::Together(vec![
            Operation::Shuffle {
                players: actor.clone(),
                excluding: Some(cards.clone()),
            },
            put(cards.clone(), D::OwnersLibraryTop, &[]),
        ]),
        ("MayRevealUptoNumberCardsOfTypeAndSetAside", [Number(count), Filter(filter)]) => {
            Operation::ChooseObjects {
                player: you(),
                objects: matching(cards, filter),
                count: Count::UpTo(count.clone()),
                constraints: vec![],
            }
        }
        ("PutRemainingSetAsideCardsIntoHand", []) => put(set_aside(), D::OwnersHand, &[]),
        ("PutSetAsideCardsOfTypeOntoBattlefield", [Filter(filter), List(entry)]) => {
            put(matching(&set_aside(), filter), D::OwnersBattlefield, entry)
        }
        ("ChooseAnAction", options) if !options.is_empty() => Operation::ChooseAction {
            actor: actor.clone(),
            options: options.to_vec(),
            villainous: false,
        },
        ("APlayerChoosesFinalDestination", [Players(players), options @ ..]) => {
            Operation::ChooseAction {
                actor: players.clone(),
                options: options.to_vec(),
                villainous: false,
            }
        }
        ("ExileFoundCardsInShuffledFaceDownPile", []) => put(
            cards.clone(),
            D::Exile,
            &[Operand::Vocabulary {
                family: "ExileFlag".into(),
                form: "ExilesIntoShuffledFaceDownPile".into(),
            }],
        ),
        ("ExileLibraryAndGraveyard", []) => put(
            ObjectSet::Difference {
                objects: Box::new(ObjectSet::Union(vec![
                    zone_set(EventZone::Library, actor.clone(), ObjectFilter::Any),
                    zone_set(EventZone::Graveyard, actor.clone(), ObjectFilter::Any),
                ])),
                excluding: Box::new(cards.clone()),
            },
            D::Exile,
            &[],
        ),
        ("FindExactlyNumberGroupCards", [Number(count), Filter(filter), group]) => {
            Operation::ChooseObjects {
                player: you(),
                objects: matching(cards, filter),
                count: Count::Exactly(count.clone()),
                constraints: vec![group_constraint(group)?],
            }
        }
        ("PutExiledCardsOnTopOfLibraryIgnoreOrder", [Objects(exiled)]) => {
            put(exiled.clone(), D::OwnersLibraryTop, &[])
        }
        ("PutSetAsideCardsNthFromTheTop", [Number(depth)]) => {
            put(cards.clone(), D::OwnersLibraryFromTop(depth.clone()), &[])
        }
        ("PlayerChoosesACardName", [Player(player), options @ ..]) => Operation::ChooseValue {
            actor: PlayerSet::One(player.clone()),
            kind: ValueKind::CardName,
            count: 1,
            options: options.to_vec(),
        },
        _ => return None,
    })
}

/// Entry flags that say what the entering object is or how long its exile lasts.
fn reduced_entry_flag(form: &str, operands: &[Operand]) -> Option<EntryState> {
    use Operand::{Duration as Lasting, List, Objects};
    let face_down = |pt: &Operand, kind: Option<&Operand>, artifact| FaceDownAs {
        power_toughness: Box::new(pt.clone()),
        creature_type: kind.map(|kind| Box::new(kind.clone())),
        artifact,
    };
    Some(match (form, operands) {
        ("EntersAsFaceDownArtifactCreature", [pt, kind]) => {
            EntryState::FaceDownAs(face_down(pt, Some(kind), true))
        }
        ("EntersAsFaceDownCreatureWithAbilitiesAndNotedName", [pt, List(abilities), named]) => {
            let Operand::Term { operands, .. } = named else {
                return None;
            };
            let [Operand::Object(card)] = operands.as_slice() else {
                return None;
            };
            EntryState::FaceDownWith {
                face_down: face_down(pt, None, false),
                abilities: abilities.clone(),
                noted: Some(card.clone()),
            }
        }
        ("EntersAsFaceDownLand", [land]) => EntryState::FaceDownLand(Box::new(land.clone())),
        ("EntersAsNonAuraEnchantment", []) => EntryState::NotAnAura,
        ("EntersWithNotedCounters", []) => EntryState::NotedCounters,
        ("EntersWithPerpetualEffect", effects) => EntryState::Perpetual(perpetual_changes(effects)?),
        ("MayActions", flags) if !flags.is_empty() => EntryState::May(flags.to_vec()),
        ("Limited", [Lasting(until)]) => EntryState::ExiledFor(until.clone()),
        ("LimitedWithTriggerEnters", [Lasting(until), Objects(objects), body]) => {
            EntryState::ExiledForWith {
                until: until.clone(),
                objects: objects.clone(),
                body: Box::new(body.clone()),
            }
        }
        ("EntersWithRuleEffectUntil", [List(rules), Lasting(until)]) => EntryState::RulesUntil {
            rules: rules.clone(),
            until: until.clone(),
        },
        _ => return None,
    })
}

pub fn reduce_entry_flag(name: String, operands: Vec<Operand>) -> EntryState {
    let reduced = name
        .split_once("::")
        .and_then(|(_, form)| reduced_entry_flag(form, &operands));
    reduced.unwrap_or(EntryState::Primitive { name, operands })
}

fn tail_objects(form: &str, operands: &[Operand]) -> Option<ObjectSet> {
    use Operand::{Filter, Number, Objects, Players, Term};
    let hand = |term: &Operand| match term {
        Term { operands, .. } => match operands.as_slice() {
            [Players(players)] => Some(players.clone()),
            _ => None,
        },
        _ => None,
    };
    Some(match (form, operands) {
        (
            "NumberPermanentsCardsInHandOrCardsInGraveyards",
            [Number(count), Objects(permanents), held, Filter(graveyard)],
        ) => ObjectSet::Chosen {
            chooser: hand(held)?,
            objects: Box::new(ObjectSet::Union(vec![
                permanents.clone(),
                zone_set(EventZone::Hand, hand(held)?, ObjectFilter::Any),
                ObjectSet::AnyZone(graveyard.clone()),
            ])),
            count: Count::Exactly(count.clone()),
        },
        ("NumberPermanentsOrCardsInHand", [Number(count), Objects(permanents), held]) => {
            ObjectSet::Chosen {
                chooser: hand(held)?,
                objects: Box::new(ObjectSet::Union(vec![
                    permanents.clone(),
                    zone_set(EventZone::Hand, hand(held)?, ObjectFilter::Any),
                ])),
                count: Count::Exactly(count.clone()),
            }
        }
        ("TheBottomCardOfTypeInLibrary", [Filter(cards)]) => ObjectSet::Take {
            objects: Box::new(zone_set(
                EventZone::Library,
                PlayerSet::One(PlayerRef::AbilityController),
                cards.clone(),
            )),
            count: Expr::Constant(1),
            from: SequenceEnd::Bottom,
        },
        _ => return None,
    })
}

/// "An instant spell, a sorcery spell, or a loyalty ability": what speaks of a spell
/// says nothing of an activation. A filter of neither sort speaks of both.
fn spell_and_ability_parts(filter: ObjectFilter) -> (ObjectFilter, ObjectFilter) {
    let of_a_spell = |part: &ObjectFilter| match part {
        ObjectFilter::All(parts) => parts
            .iter()
            .any(|p| matches!(p, ObjectFilter::OnStack(StackTrait::Spell))),
        _ => false,
    };
    let one = |mut parts: Vec<ObjectFilter>| match parts.len() {
        1 => parts.remove(0),
        _ => ObjectFilter::Either(parts),
    };
    match &filter {
        ObjectFilter::Either(parts) => {
            let (spells, abilities): (Vec<_>, Vec<_>) =
                parts.iter().cloned().partition(|part| of_a_spell(part));
            if spells.is_empty() || abilities.is_empty() {
                (filter.clone(), filter)
            } else {
                (one(spells), one(abilities))
            }
        }
        _ => (filter.clone(), filter),
    }
}

/// Delayed triggers that read like their ordinary trigger, or as one spell or activation.
pub fn reduce_future_event(name: String, operands: Vec<Operand>) -> EventSpec {
    use Operand::Player;
    let form = name.split_once("::").map_or("", |(_, form)| form);
    let reduced = match (form, operands.as_slice()) {
        (
            "WhenPlayerCastsTheirNextSpellOrActivatesTheirNextAbilityThisTurn",
            [Player(player), which],
        ) => stack_filter(which).map(|filter| {
            let (spells, abilities) = spell_and_ability_parts(filter);
            EventSpec::SpellOrActivation {
                players: PlayerSet::One(player.clone()),
                spells,
                abilities,
            }
        }),
        (form, operands) => match reduce_event(format!("Trigger::{form}"), operands.to_vec()) {
            EventSpec::Primitive { .. } => reduce_card_future_event(form, operands),
            event => Some(event),
        },
    };
    reduced.unwrap_or(EventSpec::Primitive { name, operands })
}

fn reduce_tail_cost(name: &str, operands: &[Operand]) -> Option<CostPart> {
    use Operand::{Filter, Number};
    Some(match (name, operands) {
        ("Cost::RevealNumberGroupCardsFromHand", [Number(count), Filter(cards), group]) => {
            CostPart::Constrained {
                part: Box::new(CostPart::Reveal {
                    count: Count::Exactly(count.clone()),
                    filter: cards.clone(),
                    random: false,
                }),
                constraints: vec![group_constraint(group)?],
            }
        }
        _ => return None,
    })
}
