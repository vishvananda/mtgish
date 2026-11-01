// Actions the first passes left: conjuring what isn't one named card, notes,
// bids, guesses, piles, seeking and melding.

fn card_names(operands: &[Operand]) -> Option<Vec<String>> {
    operands
        .iter()
        .map(|operand| match operand {
            Operand::Text(name) => Some(name.clone()),
            _ => None,
        })
        .collect()
}

const POWER_NINE: [&str; 9] = [
    "Ancestral Recall",
    "Black Lotus",
    "Mox Emerald",
    "Mox Jet",
    "Mox Pearl",
    "Mox Ruby",
    "Mox Sapphire",
    "Time Walk",
    "Timetwister",
];

/// Conjuring by Forge's `MakeCard`, past the forms `reduce_conjure` lists.
fn reduce_more_conjure(actor: &PlayerSet, form: &str, operands: &[Operand]) -> Option<Operation> {
    use ConjureSpot as Spot;
    use ConjuredCard as C;
    use EventZone as Z;
    use Operand::{Filter, List, Number, Object, Objects, Player, Players, Result, Text};
    let one = Expr::Constant(1);
    let conjure = |actor: &PlayerSet, card, count, zone, owner: Option<&PlayerRef>, spot, entry: &[Operand]| {
        Operation::Conjure {
            actor: actor.clone(),
            card,
            count,
            zone,
            owner: owner.cloned(),
            spot,
            entry: entry.to_vec(),
        }
    };
    let simple = |card, zone, entry: &[Operand]| conjure(actor, card, one.clone(), zone, None, None, entry);
    let random = |cards: &Operand| C::Random(Box::new(cards.clone()));
    let library_of = |players: PlayerSet, cards: &ObjectFilter, count: Expr| {
        C::Duplicate(ObjectSet::Random {
            objects: Box::new(zone_set(Z::Library, players, cards.clone())),
            count,
        })
    };
    let dup = |object: &ObjectRef| C::Duplicate(ObjectSet::One(object.clone()));
    Some(match (form, operands) {
        ("ACardOfTypeFromSpellBookOntoBattlefield", [Text(book), cards, entry @ ..]) => simple(
            C::SpellbookMatching {
                book: book.clone(),
                cards: Box::new(cards.clone()),
            },
            Z::Battlefield,
            entry,
        ),
        ("ARandomCardFromSpellBookIntoExileFaceDown", [Text(book)]) => simple(
            C::Spellbook {
                book: book.clone(),
                pick: SpellbookPick::Random,
            },
            Z::Exile,
            &[Operand::Vocabulary {
                family: "EntryState".into(),
                form: "FaceDown".into(),
            }],
        ),
        ("ARandomCardIntoExile", [cards]) => simple(random(cards), Z::Exile, &[]),
        ("ARandomCardOntoBattlefield", [cards, entry @ ..]) => {
            simple(random(cards), Z::Battlefield, entry)
        }
        ("CardOrCardIntoHand", [Text(a), Text(b)]) => {
            simple(C::OneOf(vec![a.clone(), b.clone()]), Z::Hand, &[])
        }
        ("CardsOntoTheBattlefield", [List(names), entry @ ..]) => {
            simple(C::Each(card_names(names)?), Z::Battlefield, entry)
        }
        ("DuplicateOfARandomCardOfTypeFromAPlayersLibraryIntoHand", [Filter(cards), Players(players)]) => {
            simple(library_of(players.clone(), cards, one.clone()), Z::Hand, &[])
        }
        ("DuplicateOfARandomCardOfTypeFromPlayersLibraryIntoHand", [Filter(cards), Player(player)]) => {
            simple(
                library_of(PlayerSet::One(player.clone()), cards, one.clone()),
                Z::Hand,
                &[],
            )
        }
        ("DuplicateOfARandomCardOfTypeIntoHand", [cards]) => simple(random(cards), Z::Hand, &[]),
        ("DuplicateOfARandomCardOfTypeOntoBattlefield", [cards, entry @ ..]) => {
            simple(random(cards), Z::Battlefield, entry)
        }
        ("DuplicateOfCardInHandIntoHand", [Operand::Vocabulary { .. }]) => {
            simple(dup(&earlier_object(EarlierObjects::Selected)), Z::Hand, &[])
        }
        ("DuplicateOfEachCardSeekedThisWayIntoHand", []) => simple(
            C::Duplicate(ObjectSet::Earlier {
                what: EarlierObjects::Sought,
                by: None,
            }),
            Z::Hand,
            &[],
        ),
        (
            "DuplicateOfPermanentIntoTopNumberCardsOfPlayersLibraryAtRandom",
            [Object(object), Number(top), Player(player)],
        ) => conjure(
            actor,
            dup(object),
            one.clone(),
            Z::Library,
            Some(player),
            Some(Spot::AmongTop(top.clone())),
            &[],
        ),
        ("DuplicateOfThePermanentSacrificedThisWayIntoHand", [Result(result)]) => {
            simple(C::Duplicate(ObjectSet::Named(result.clone())), Z::Hand, &[])
        }
        ("DuplicateOfTheTopCardOfPlayersIntoHand", [Player(player)]) => simple(
            C::Duplicate(ObjectSet::LibraryTops(PlayerSet::One(player.clone()))),
            Z::Hand,
            &[],
        ),
        (
            "DuplicatesOfNumberRandomCardsOfTypeFromPlayersLibraryIntoHand",
            [Number(count), Filter(cards), Player(player)],
        ) => simple(
            library_of(PlayerSet::One(player.clone()), cards, count.clone()),
            Z::Hand,
            &[],
        ),
        ("MultipleCardsIntoLibraryAndShuffle", names) => conjure(
            actor,
            C::Each(card_names(names)?),
            one.clone(),
            Z::Library,
            None,
            Some(Spot::Shuffled),
            &[],
        ),
        ("ThePowerNineIntoLibraryAndShuffle", []) => conjure(
            actor,
            C::Each(POWER_NINE.iter().map(|name| name.to_string()).collect()),
            one.clone(),
            Z::Library,
            None,
            Some(Spot::Shuffled),
            &[],
        ),
        (
            "NumberCardsIntoTheTopNumberCardsOfEachPlayersLibraryAtRandom",
            [Number(count), Text(card), Number(top), Players(players)],
        ) => conjure(
            players,
            C::Named(card.clone()),
            count.clone(),
            Z::Library,
            None,
            Some(Spot::AmongTop(top.clone())),
            &[],
        ),
        ("NumberDuplicatesOfAnOutsideCardIntoHand", [Number(count), Filter(cards)]) => conjure(
            actor,
            C::Duplicate(ObjectSet::OutsideTheGame(cards.clone())),
            count.clone(),
            Z::Hand,
            None,
            None,
            &[],
        ),
        ("NumberDuplicatesOfGraveyardCardIntoExile", [Number(count), Object(card)]) => conjure(
            actor,
            dup(card),
            count.clone(),
            Z::Exile,
            None,
            None,
            &[],
        ),
        _ => return None,
    })
}

fn pick_from_hand(
    actor: &PlayerSet,
    player: &PlayerRef,
    filter: &ObjectFilter,
    count: Count,
    then: HandPick,
) -> Operation {
    Operation::PickFromHand {
        actor: actor.clone(),
        player: player.clone(),
        filter: filter.clone(),
        count,
        then,
    }
}

fn actor_of(actor: &PlayerSet) -> PlayerRef {
    match actor {
        PlayerSet::One(player) => player.clone(),
        _ => PlayerRef::AbilityController,
    }
}

/// Everything else a row's action names by shape.
fn reduce_more_actions(actor: &PlayerSet, form: &str, operands: &[Operand]) -> Option<Operation> {
    use EventZone as Z;
    use Operand::{Filter, Integer, List, Number, Object, Objects, Player, Players, Result, Text};
    let one = || Count::Exactly(Expr::Constant(1));
    let free = || PlayTerms {
        cost: PlayCost::Free,
        ..play_terms()
    };
    let cast = |cards, count| cast_with(actor, CardPlayAction::CastSpell, cards, count, free(), None);
    let earlier = |what| ObjectSet::Earlier { what, by: None };
    let note = |kind| Some(Operation::Note(kind));
    let choose = |objects, count| Operation::ChooseObjects {
        player: actor_of(actor),
        objects,
        count,
        constraints: vec![],
    };
    let stickers = |kind, objects: ObjectSet, amount: Option<&Expr>, any_number| Operation::PutStickers {
        actor: actor.clone(),
        kind,
        objects,
        amount: amount.cloned(),
        any_number,
    };
    let piles = |objects: &ObjectSet, count: Option<&Expr>, names, chooser: Option<&PlayerSet>| Operation::Piles {
        actor: actor.clone(),
        objects: objects.clone(),
        count: count.cloned(),
        names,
        chooser: chooser.cloned(),
    };
    let pile = |from: Option<&PlayerSet>, random, names, object: Option<&ObjectRef>| Operation::ChoosePile {
        actor: actor.clone(),
        from: from.cloned(),
        random,
        names,
        object: object.cloned(),
    };
    let seek = |cards: Vec<ObjectFilter>, count: Expr, top: Option<&Expr>| Operation::Seek {
        actor: actor.clone(),
        cards,
        count,
        top: top.cloned(),
    };
    let guess = |object: Option<&ObjectRef>, filter: Option<&ObjectFilter>, player: Option<&PlayerRef>| Operation::Guess {
        actor: actor.clone(),
        object: object.cloned(),
        filter: filter.cloned(),
        player: player.cloned(),
    };
    Some(match (form, operands) {
        ("MeldExiledIntoNewPermanent", [Objects(objects), Text(into), entry @ ..]) => {
            Operation::Meld {
                objects: objects.clone(),
                into: into.clone(),
                entry: entry.to_vec(),
            }
        }
        ("CastCopiedCardWithoutPaying", []) => cast(earlier(EarlierObjects::CopiedSpell), one()),
        ("CastASpellFromMilledCardsWithoutPaying", [Filter(spells)]) => {
            cast(matching(&earlier(EarlierObjects::Milled), spells), one())
        }
        ("CastAnyNumberOfSpellsFromOutsideTheGameWithoutPaying", [Filter(spells)]) => cast(
            ObjectSet::OutsideTheGame(spells.clone()),
            Count::AnyNumber,
        ),
        ("PutANameStickerOnAPermanent", [Objects(objects)]) => {
            stickers(Some(StickerKind::Name), objects.clone(), None, false)
        }
        ("PutAnArtStickerOnAPermanent", [Objects(objects)]) => {
            stickers(Some(StickerKind::Art), objects.clone(), None, false)
        }
        ("PutAPowerAndToughnessStickerOnAPermanent", [Objects(objects)]) => {
            stickers(Some(StickerKind::PowerToughness), objects.clone(), None, false)
        }
        ("PutAStickerOnACardInPlayersGraveyard", [Filter(cards), Player(player)]) => stickers(
            None,
            zone_set(Z::Graveyard, PlayerSet::One(player.clone()), cards.clone()),
            None,
            false,
        ),
        ("PutUptoNumberNameStickersOnPermanent", [Number(amount), Object(object)]) => stickers(
            Some(StickerKind::Name),
            ObjectSet::One(object.clone()),
            Some(amount),
            false,
        ),
        ("DistrbuteUptoNumberArtStickersAmongAnyNumberOfPermanents", [Number(amount), Objects(objects)]) => {
            stickers(Some(StickerKind::Art), objects.clone(), Some(amount), true)
        }
        ("SeparatePermanentsIntoNumberPiles", [Objects(objects), Number(count)]) => {
            piles(objects, Some(count), vec![], None)
        }
        ("SeparatePermanentsIntoNamedPiles", [Objects(objects), List(names)]) => {
            piles(objects, None, card_names(names)?, None)
        }
        ("SeparatePermanentsIntoTwoPilesAndAPlayerChoosesAPile", [Objects(objects), Players(chooser)]) => {
            piles(objects, Some(&Expr::Constant(2)), vec![], Some(chooser))
        }
        ("ChooseAPileCreatedByEachPlayer", [Players(players)]) => {
            pile(Some(players), false, vec![], None)
        }
        ("ChooseAPileCreatedByEachPlayerAtRandom", [Players(players)]) => {
            pile(Some(players), true, vec![], None)
        }
        ("ChooseNamedPileForPermanent", [List(names), Object(object)]) => {
            pile(None, false, card_names(names)?, Some(object))
        }
        ("SeekACardFromTheTopNumberCardsOfLibrary", [Filter(cards), Number(top)]) => {
            seek(vec![cards.clone()], Expr::Constant(1), Some(top))
        }
        ("SeekCards", filters) => seek(
            filters
                .iter()
                .map(|operand| match operand {
                    Filter(filter) => Some(filter.clone()),
                    _ => None,
                })
                .collect::<Option<_>>()?,
            Expr::Constant(1),
            None,
        ),
        ("SeekNumberCardsFromTheTopNumberCardsOfLibrary", [Number(count), Filter(cards), Number(top)]) => {
            seek(vec![cards.clone()], count.clone(), Some(top))
        }
        ("NoteNumber", [Number(value)]) => return note(NoteKind::Number(value.clone())),
        ("RememberPlayer", [Player(player)]) => return note(NoteKind::Player(player.clone())),
        ("RememberLifeTotal", []) => return note(NoteKind::LifeTotal),
        ("NoteCountersOnPermanent", [Object(object)]) => {
            return note(NoteKind::CountersOn(object.clone()))
        }
        ("NoteManaTypeAndAmountSpentToActivateThisAbility", []) => {
            return note(NoteKind::ManaSpent { amount: true })
        }
        ("NoteTypeOfManaSpentToActivateThisAbility", []) => {
            return note(NoteKind::ManaSpent { amount: false })
        }
        ("StartBiddingWarAmongPlayersAtAnyNumber", [Players(players)]) => Operation::Bid {
            players: players.clone(),
            start: None,
        },
        ("StartBiddingWarAmongPlayersAtNumber", [Players(players), Number(start)]) => Operation::Bid {
            players: players.clone(),
            start: Some(start.clone()),
        },
        ("StartBiddingWarWithPlayer", [Player(player), Number(start)]) => Operation::Bid {
            players: PlayerSet::Union(vec![actor.clone(), PlayerSet::One(player.clone())]),
            start: Some(start.clone()),
        },
        ("GuessIfACardIsInPlayersHand", [Filter(cards), Player(player)]) => {
            guess(None, Some(cards), Some(player))
        }
        ("GuessIfCardInHandPassesFilter", [Object(card), Filter(filter)]) => {
            guess(Some(card), Some(filter), None)
        }
        ("GuessWhichNumberWasSecretlyChosen", []) => guess(None, None, None),
        ("Loyalty", [Integer(amount)]) => Operation::PayCost {
            player: actor_of(actor),
            cost: Cost {
                parts: vec![CostPart::Loyalty(Expr::Constant(*amount))],
            },
            optional: false,
        },
        ("DrawNumberCardsThenDiscardNumberOfThem", [Number(draw), Number(discard)]) => {
            Operation::DrawThenDiscard {
                players: actor.clone(),
                draw: draw.clone(),
                discard: discard.clone(),
            }
        }
        ("RedistributeLifeTotalsOfPlayers", [Players(players)]) => {
            Operation::RedistributeLife(players.clone())
        }
        ("LookAtPlayersHandAndChooseACardForPlayerToPlayControllingThemToDoSo", [Player(player), Filter(filter)]) => {
            pick_from_hand(actor, player, filter, one(), HandPick::TheyPlayIt)
        }
        ("LookAtPlayersHandAndChooseNumberCardsToPutOnTopOfTheirLibraryInAnyOrder", [Player(player), Number(count), Filter(filter)]) => {
            pick_from_hand(
                actor,
                player,
                filter,
                Count::Exactly(count.clone()),
                HandPick::TopOfLibrary,
            )
        }
        ("LookAtPlayersHandAndMayChooseACardToCastWithoutPaying", [Player(player), Filter(filter)]) => {
            pick_from_hand(
                actor,
                player,
                filter,
                Count::UpTo(Expr::Constant(1)),
                HandPick::CastFree,
            )
        }
        ("ChooseACardFromAmongCardsDiscardedThisWay", [Filter(cards), Result(result)]) => {
            choose(matching(&ObjectSet::Named(result.clone()), cards), one())
        }
        ("ChooseNumberCardsFromAmongCardsInHandRevealedThisWay", [Number(count), Filter(cards), Result(result)]) => {
            choose(
                matching(&ObjectSet::Named(result.clone()), cards),
                Count::Exactly(count.clone()),
            )
        }
        (
            "ChooseUptoNumberCardsFromAmongCardsInPlayersHandRevealedThisWay",
            [Number(count), Filter(cards), Player(_), Result(result)],
        ) => choose(
            matching(&ObjectSet::Named(result.clone()), cards),
            Count::UpTo(count.clone()),
        ),
        ("ChooseANamedAction", options) => Operation::ChooseAction {
            actor: actor.clone(),
            options: options.to_vec(),
            villainous: false,
        },
        _ => return None,
    })
}

fn when_state(condition: Condition) -> EventSpec {
    EventSpec::State { condition }
}

/// What a card must be to count as this hand or card term.
fn card_term(operand: &Operand) -> Option<ObjectFilter> {
    match operand {
        Operand::Filter(filter) => Some(filter.clone()),
        Operand::Vocabulary { form, .. } if form == "ThisCardInHand" || form == "ThisCard" => {
            Some(ObjectFilter::SameObject(ObjectRef::Source))
        }
        Operand::Term { form, operands, .. } if form == "SingleCardInHand" => {
            card_term(operands.first()?)
        }
        Operand::Term { form, operands, .. } if form == "IsCreatureType" => {
            Some(ObjectFilter::Subtype(vocabulary_value(operands.first()?)?))
        }
        other => hand_filter(other),
    }
}

fn counters_filter(kind: &Operand, comparison: &Comparison, value: &Expr) -> Option<ObjectFilter> {
    Some(ObjectFilter::Counters {
        kind: Some(counter_kind(kind)?),
        comparison: comparison.clone(),
        value: value.clone(),
    })
}

/// Triggers the first passes left, each by what the rules watch for.
fn reduce_more_events(form: &str, operands: &[Operand]) -> Option<EventSpec> {
    use AttackDefenders as D;
    use EventZone as Z;
    use Operand::{Comparison as Compare, Filter, Object, Objects, Players};
    let all = |filters: Vec<ObjectFilter>| ObjectFilter::All(filters);
    let bare = || -> Option<(Comparison, Expr)> { None };
    let batch = |event: EventSpec, count, including| EventSpec::Batch {
        event: Box::new(event),
        per: BatchPer::Whole,
        count,
        including,
    };
    let attack = |attackers, by: Option<&PlayerSet>, defenders| EventSpec::Attack {
        attackers,
        by: by.cloned(),
        defenders,
    };
    let spell = |players: &PlayerSet, spells| EventSpec::Spell {
        event: SpellEvent::Cast,
        players: players.clone(),
        spells,
    };
    let player = |event, players: &PlayerSet, objects| EventSpec::Player {
        event,
        players: players.clone(),
        objects,
    };
    let discard = |players: &PlayerSet, cards, kind, grouping| EventSpec::Discard {
        players: players.clone(),
        cards,
        source_only: false,
        kind,
        grouping,
    };
    let amount_is = |event, comparison: &Comparison, value: &Expr| EventSpec::Filtered {
        event: Box::new(event),
        condition: Condition::Compare {
            left: Expr::EventAmount,
            comparison: comparison.clone(),
            right: value.clone(),
        },
    };
    let one = |object: &ObjectRef| ObjectFilter::SameObject(object.clone());
    let others = |object: &ObjectRef, set: &ObjectSet| {
        all(vec![
            ObjectFilter::Not(Box::new(one(object))),
            objects_filter(set),
        ])
    };
    Some(match (form, operands) {
        ("WhenACardBecomesPlotted", [cards]) => EventSpec::Object {
            event: ObjectEvent::Plotted,
            objects: card_term(cards)?,
        },
        (
            "WhenACardIsPutIntoAGraveyardFromAnywhereOtherThanTheBattlefield",
            [Filter(cards), Players(owners)],
        ) => EventSpec::ZoneChangeFromElsewhere {
            objects: all(vec![
                cards.clone(),
                ObjectFilter::OwnedBy(owners.clone()),
            ]),
            except: vec![Z::Battlefield],
            to: Z::Graveyard,
        },
        ("WhenACreatureAttacksAPlayerAlone", [Objects(set), Players(players)]) => attack(
            all(vec![
                objects_filter(set),
                ObjectFilter::State(ObjectState::AttackingAlone),
            ]),
            None,
            D::Players(players.clone()),
        ),
        ("WhenACreatureAttacksAPlayerAndIsntBlocked", [Objects(set), Players(players)]) => {
            EventSpec::AttackUnblocked {
                attackers: objects_filter(set),
                defenders: D::Players(players.clone()),
                count: None,
            }
        }
        (
            "WhenANumberOfCreaturesAttacksAPlayerAndArentBlocked",
            [Compare(comparison, value), Objects(set), Players(players)],
        ) => EventSpec::AttackUnblocked {
            attackers: objects_filter(set),
            defenders: D::Players(players.clone()),
            count: Some((comparison.clone(), value.clone())),
        },
        ("WhenACreatureBecomesBlockedByACreature", [Objects(set), Objects(blockers)]) => {
            EventSpec::Pair {
                event: PairEvent::BecomesBlockedBy,
                objects: objects_filter(set),
                others: objects_filter(blockers),
            }
        }
        ("WhenAManaAbilityOfAPermanentResolves", [Objects(set)]) => {
            EventSpec::Resolves(all(vec![
                ObjectFilter::OnStack(StackTrait::Mana),
                ObjectFilter::AbilityOf(Box::new(objects_filter(set))),
            ]))
        }
        ("WhenASpellOrAbilityIsPutOntoTheStack", [cause]) => {
            EventSpec::OntoStack(stack_filter(cause)?)
        }
        ("WhenAPermanentAttackingCausesItsAbilityToTrigger", [Objects(set)]) => {
            EventSpec::CausesTrigger {
                cause: Box::new(attack(objects_filter(set), None, D::Any)),
                own_ability: true,
            }
        }
        ("WhenAPermanentEnteringTheBattlefieldCausesAnAbilityToTrigger", [Objects(set)]) => {
            EventSpec::CausesTrigger {
                cause: Box::new(zone_change(objects_filter(set), None, Z::Battlefield)),
                own_ability: false,
            }
        }
        (
            "WhenAPermanentEnteringTheBattlefieldUnderAPlayersControlCausesItsAbilityToTrigger",
            [Objects(set), Players(players)],
        ) => EventSpec::CausesTrigger {
            cause: Box::new(zone_change(
                all(vec![
                    objects_filter(set),
                    ObjectFilter::ControlledBy(players.clone()),
                ]),
                None,
                Z::Battlefield,
            )),
            own_ability: true,
        },
        ("WhenAPermanentEntersTheBattlefieldFromExileOrWasCastFromExile", [Objects(set)]) => {
            EventSpec::Either(vec![
                zone_change(objects_filter(set), Some(Z::Exile), Z::Battlefield),
                zone_change(
                    all(vec![
                        objects_filter(set),
                        ObjectFilter::Cast {
                            by: None,
                            from: Some(ZoneScope::Shared(Z::Exile)),
                        },
                    ]),
                    None,
                    Z::Battlefield,
                ),
            ])
        }
        ("WhenAPermanentEntersTheBattlefieldTransformed", [Objects(set)]) => zone_change(
            all(vec![
                objects_filter(set),
                ObjectFilter::State(ObjectState::Transformed),
            ]),
            None,
            Z::Battlefield,
        ),
        ("WhenAPermanentExploresACardOfType", [Objects(set), Filter(cards)]) => {
            EventSpec::ExploresCard {
                objects: objects_filter(set),
                cards: cards.clone(),
            }
        }
        ("WhenAPermanentHasPower", [Objects(set), Compare(comparison, value)]) => {
            EventSpec::ObjectBecomes(all(vec![
                objects_filter(set),
                ObjectFilter::Property {
                    property: Property::Power,
                    comparison: comparison.clone(),
                    value: value.clone(),
                },
            ]))
        }
        ("WhenAPermanentHasAbility", [Objects(set), Operand::Term { form, .. }]) if form == "Flying" => {
            EventSpec::ObjectBecomes(all(vec![
                objects_filter(set),
                ObjectFilter::HasKeyword(Keyword::Flying),
            ]))
        }
        ("WhenAPermanentHasNumberCountersOfType", [Objects(set), Compare(comparison, value), kind]) => {
            EventSpec::ObjectBecomes(all(vec![
                objects_filter(set),
                counters_filter(kind, comparison, value)?,
            ]))
        }
        ("WhenAPlaneHasNumberCountersOfType", [_, Compare(comparison, value), kind]) => {
            EventSpec::ObjectBecomes(all(vec![
                one(&ObjectRef::Source),
                counters_filter(kind, comparison, value)?,
            ]))
        }
        ("WhenAnyNumberOfPermanentsAreOnTheBattlefield", [Objects(set)]) => {
            EventSpec::ObjectBecomes(objects_filter(set))
        }
        ("WhenAPermanentEntersTheBattlefieldWithAnyCounters", [Objects(set)]) => zone_change(
            all(vec![
                objects_filter(set),
                ObjectFilter::Counters {
                    kind: None,
                    comparison: Comparison::GreaterOrEqual,
                    value: Expr::Constant(1),
                },
            ]),
            None,
            Z::Battlefield,
        ),
        ("WhenAPlayerSetsASchemeInMotion", [Players(players), Operand::Term { form, operands, .. }])
            if form == "IsNonSupertype" =>
        {
            player(
                PlayerEvent::SetsSchemeInMotion,
                players,
                Some(ObjectFilter::Not(Box::new(ObjectFilter::Supertype(
                    vocabulary_value(operands.first()?)?,
                )))),
            )
        }
        ("WhenAPermanentIsTappedForManaOfColor", [Objects(set), mana]) => EventSpec::TappedForManaOf {
            objects: objects_filter(set),
            by: None,
            mana: Box::new(mana.clone()),
        },
        ("WhenAPlayerTapsAPermanentForManaOfColor", [Players(players), Objects(set), mana]) => {
            EventSpec::TappedForManaOf {
                objects: objects_filter(set),
                by: Some(players.clone()),
                mana: Box::new(mana.clone()),
            }
        }
        ("WhenAnAbilityCausesAPlayerToAddMana", [cause, Players(players), mana]) => {
            EventSpec::ManaAdded {
                abilities: stack_filter(cause)?,
                players: players.clone(),
                mana: Box::new(mana.clone()),
            }
        }
        (
            "WhenAPlayerAttacksWithASingleCreatureAndANumberOfOtherCreatures",
            [Players(players), Objects(ObjectSet::One(object)), Compare(comparison, value), Objects(set)],
        ) => EventSpec::AttacksWith {
            object: object.clone(),
            others: objects_filter(set),
            count: (comparison.clone(), value.clone()),
            by: Some(players.clone()),
            different_defenders: false,
        },
        (
            "WhenASingleCreatureAndANumberOfOtherCreaturesAttackDifferentPlayers",
            [Object(object), Compare(comparison, value), Objects(set)],
        ) => EventSpec::AttacksWith {
            object: object.clone(),
            others: objects_filter(set),
            count: (comparison.clone(), value.clone()),
            by: None,
            different_defenders: true,
        },
        ("WhenASingleCreatureAttacksWithExactlyOneOtherCreature", [Object(object)]) => {
            EventSpec::AttacksWith {
                object: object.clone(),
                others: ObjectFilter::Any,
                count: (Comparison::Equal, Expr::Constant(1)),
                by: None,
                different_defenders: false,
            }
        }
        ("WhenAPlayerAttacksWithAnyNumberOfGroupCreatures", [Players(players), Objects(set), group]) => {
            EventSpec::AttacksAsGroup {
                by: players.clone(),
                attackers: objects_filter(set),
                group: Box::new(group.clone()),
            }
        }
        ("WhenAllCreaturesAttack", [Objects(set)]) => EventSpec::AllAttack(objects_filter(set)),
        ("WhenAPlayerCastsASpellThatTargetsAnyNumberOfPermanents", [Players(players), Filter(spells), Objects(set)]) => {
            EventSpec::SpellTargeting {
                players: players.clone(),
                spells: spells.clone(),
                targets: Some(set.clone()),
                count: None,
            }
        }
        ("WhenAPlayerCastsASpellWithANumberOfTargets", [Players(players), Filter(spells), Compare(comparison, value)]) => {
            EventSpec::SpellTargeting {
                players: players.clone(),
                spells: spells.clone(),
                targets: None,
                count: Some((comparison.clone(), value.clone())),
            }
        }
        ("WhenAPlayerKicksASpell", [Players(players), Filter(spells)]) => spell(
            players,
            all(vec![
                spells.clone(),
                ObjectFilter::State(ObjectState::Kicked),
            ]),
        ),
        ("WhenAPlayerCastsASpellOrActivatesAnAbility", [Players(players), Filter(spells), abilities]) => {
            EventSpec::SpellOrActivation {
                players: players.clone(),
                spells: spells.clone(),
                abilities: stack_filter(abilities)?,
            }
        }
        ("WhenAPlayerPaysLifeToActivateAnAbility", [Players(players), abilities]) => {
            EventSpec::PaysToActivate {
                players: players.clone(),
                abilities: stack_filter(abilities)?,
                payment: ActivationPayment::Life,
            }
        }
        ("WhenAPlayerSacrificesAnyNumberOfPermanentsToActivateAnAbility", [Players(players), abilities]) => {
            EventSpec::PaysToActivate {
                players: players.clone(),
                abilities: stack_filter(abilities)?,
                payment: ActivationPayment::Sacrifice,
            }
        }
        ("WhenAPlayerChoosesTargetsForASpellOrAbility", [Players(players), abilities]) => {
            EventSpec::TargetsChosen {
                players: players.clone(),
                objects: stack_filter(abilities)?,
            }
        }
        ("WhenAPlayerGainsControlOfAPermanentFromAPlayer", [Players(to), Objects(set), Players(from)]) => {
            EventSpec::ControlChanges {
                objects: objects_filter(set),
                to: to.clone(),
                from: from.clone(),
            }
        }
        ("WhenAPlayerPutsCountersOnAPlayer", [Players(by), Players(players)]) => {
            EventSpec::CountersOnPlayer {
                by: by.clone(),
                players: players.clone(),
            }
        }
        ("WhenAPlayerRemovesACounterOfTypeFromAnExiledCard", [Players(by), kind, Objects(set)]) => {
            EventSpec::Counters {
                change: CounterChange::Removed,
                kind: Some(counter_kind(kind)?),
                objects: objects_filter(set),
                by: Some(by.clone()),
            }
        }
        ("WhenAPlayerSacrificesAnyNumberOfPermanents", [Players(players)]) => {
            batch(player(PlayerEvent::Sacrifices, players, None), bare(), None)
        }
        ("WhenAPlayerUntapsAnyNumberOfPermanentDuringTheirUntapStep", [Players(players), Objects(set)]) => {
            player(
                PlayerEvent::UntapsDuringUntapStep,
                players,
                Some(objects_filter(set)),
            )
        }
        ("WhenASpecificCardIsPutIntoAPlayersHandFromTheirLibrary", [cards, Players(owners)]) => {
            zone_change(
                all(vec![
                    card_term(cards)?,
                    ObjectFilter::OwnedBy(owners.clone()),
                ]),
                Some(Z::Library),
                Z::Hand,
            )
        }
        ("WhenASpellBecomesTheTargetOfASpellOrAbility", [Filter(spells), cause]) => {
            EventSpec::BecomesTarget {
                objects: spells.clone(),
                kind: TargetingKind::SpellOrAbility,
                by: Box::new(cause.clone()),
            }
        }
        (
            "WhenAnyNumberOfPlayersAndOrPermanentsBecomeTheTargetOfASpellOrAbility",
            [Players(players), Objects(set), cause],
        ) => batch(
            EventSpec::Either(vec![
                player(PlayerEvent::BecomesTarget, players, None),
                EventSpec::BecomesTarget {
                    objects: objects_filter(set),
                    kind: TargetingKind::SpellOrAbility,
                    by: Box::new(cause.clone()),
                },
            ]),
            bare(),
            None,
        ),
        ("WhenASpellOrAbilityCausesAPlayerToDiscardACard", [cause, Players(players), cards]) => {
            discard(
                players,
                card_term(cards)?,
                DiscardKind::CausedBy(stack_filter(cause)?),
                DiscardGrouping::Card,
            )
        }
        ("WhenASpellOrAbilityCausesAPlayerToDiscardAnyNumberOfCards", [cause, Players(players)]) => {
            discard(
                players,
                ObjectFilter::Any,
                DiscardKind::CausedBy(stack_filter(cause)?),
                DiscardGrouping::Player,
            )
        }
        ("WhenAnyNumberOfPlayersDiscardAnyNumberOfCards", [Players(players), cards]) => batch(
            discard(players, card_term(cards)?, DiscardKind::Any, DiscardGrouping::Player),
            bare(),
            None,
        ),
        (
            "WhenASpellOrAbilityCausesAPlayerToLoseAnAmountOfLife",
            [cause, Players(players), Compare(comparison, value)],
        ) => caused(
            stack_filter(cause)?,
            amount_is(player(PlayerEvent::LosesLife, players, None), comparison, value),
        ),
        ("WhenAnyNumberOfPlayersEachLoseAnAmountOfLife", [Players(players), Compare(comparison, value)]) => {
            batch(
                amount_is(player(PlayerEvent::LosesLife, players, None), comparison, value),
                bare(),
                None,
            )
        }
        _ => return None,
    })
}
