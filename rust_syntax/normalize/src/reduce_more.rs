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
    use Operand::{Filter, List, Number, Object, Player, Players, Result, Text};
    let one = Expr::Constant(1);
    let conjure = |actor: &PlayerSet,
                   card,
                   count,
                   zone,
                   owner: Option<&PlayerRef>,
                   spot,
                   entry: &[Operand]| {
        crate::movement::conjure_to(
            actor.clone(),
            card,
            count,
            zone,
            owner.cloned(),
            spot,
            crate::movement::entry_from_source(entry),
        )
    };
    let simple =
        |card, zone, entry: &[Operand]| conjure(actor, card, one.clone(), zone, None, None, entry);
    let random = |cards: &Operand| C::Random(Box::new(cards.clone()));
    let library_of = |players: PlayerSet, cards: &ObjectFilter, count: Expr| {
        C::Duplicate(ObjectSet::DraftRandom {
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
        )
        .ok()?,
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
        )
        .ok()?,
        ("ARandomCardIntoExile", [cards]) => simple(random(cards), Z::Exile, &[]).ok()?,
        ("ARandomCardOntoBattlefield", [cards, entry @ ..]) => {
            simple(random(cards), Z::Battlefield, entry).ok()?
        }
        ("CardOrCardIntoHand", [Text(a), Text(b)]) => {
            simple(C::OneOf(vec![a.clone(), b.clone()]), Z::Hand, &[]).ok()?
        }
        ("CardsOntoTheBattlefield", [List(names), entry @ ..]) => {
            simple(C::Each(card_names(names)?), Z::Battlefield, entry).ok()?
        }
        (
            "DuplicateOfARandomCardOfTypeFromAPlayersLibraryIntoHand",
            [Filter(cards), Players(players)],
        ) => simple(
            library_of(players.clone(), cards, one.clone()),
            Z::Hand,
            &[],
        )
        .ok()?,
        (
            "DuplicateOfARandomCardOfTypeFromPlayersLibraryIntoHand",
            [Filter(cards), Player(player)],
        ) => simple(
            library_of(PlayerSet::One(player.clone()), cards, one.clone()),
            Z::Hand,
            &[],
        )
        .ok()?,
        ("DuplicateOfARandomCardOfTypeIntoHand", [cards]) => {
            simple(random(cards), Z::Hand, &[]).ok()?
        }
        ("DuplicateOfARandomCardOfTypeOntoBattlefield", [cards, entry @ ..]) => {
            simple(random(cards), Z::Battlefield, entry).ok()?
        }
        ("DuplicateOfCardInHandIntoHand", [Operand::Vocabulary { .. }]) => {
            simple(dup(&earlier_object(EarlierObjects::Selected)), Z::Hand, &[]).ok()?
        }
        ("DuplicateOfEachCardSeekedThisWayIntoHand", []) => simple(
            C::Duplicate(ObjectSet::Earlier {
                what: EarlierObjects::Sought,
                by: None,
            }),
            Z::Hand,
            &[],
        )
        .ok()?,
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
        )
        .ok()?,
        ("DuplicateOfThePermanentSacrificedThisWayIntoHand", [Result(result)]) => simple(
            C::Duplicate(ObjectSet::Recorded(result.clone())),
            Z::Hand,
            &[],
        )
        .ok()?,
        ("DuplicateOfTheTopCardOfPlayersIntoHand", [Player(player)]) => simple(
            C::Duplicate(ObjectSet::LibraryTops(PlayerSet::One(player.clone()))),
            Z::Hand,
            &[],
        )
        .ok()?,
        (
            "DuplicatesOfNumberRandomCardsOfTypeFromPlayersLibraryIntoHand",
            [Number(count), Filter(cards), Player(player)],
        ) => simple(
            library_of(PlayerSet::One(player.clone()), cards, count.clone()),
            Z::Hand,
            &[],
        )
        .ok()?,
        ("MultipleCardsIntoLibraryAndShuffle", names) => conjure(
            actor,
            C::Each(card_names(names)?),
            one.clone(),
            Z::Library,
            None,
            Some(Spot::Shuffled),
            &[],
        )
        .ok()?,
        ("ThePowerNineIntoLibraryAndShuffle", []) => conjure(
            actor,
            C::Each(POWER_NINE.iter().map(|name| name.to_string()).collect()),
            one.clone(),
            Z::Library,
            None,
            Some(Spot::Shuffled),
            &[],
        )
        .ok()?,
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
        )
        .ok()?,
        ("NumberDuplicatesOfAnOutsideCardIntoHand", [Number(count), Filter(cards)]) => conjure(
            actor,
            C::Duplicate(ObjectSet::OutsideTheGame(cards.clone())),
            count.clone(),
            Z::Hand,
            None,
            None,
            &[],
        )
        .ok()?,
        ("NumberDuplicatesOfGraveyardCardIntoExile", [Number(count), Object(card)]) => {
            conjure(actor, dup(card), count.clone(), Z::Exile, None, None, &[]).ok()?
        }
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
    let cast =
        |cards, count| cast_with(actor, CardPlayAction::CastSpell, cards, count, free(), None);
    let earlier = |what| ObjectSet::Earlier { what, by: None };
    let note = |kind| Some(Operation::Note(kind));
    let choose = |objects, count| Operation::ChooseObjects {
        player: actor_of(actor),
        objects,
        count,
        constraints: vec![],
    };
    let stickers =
        |kind, objects: ObjectSet, amount: Option<&Expr>, any_number| Operation::PutStickers {
            actor: actor.clone(),
            kind,
            objects,
            amount: amount.cloned(),
            any_number,
            ticket_cost: None,
        };
    let piles = |objects: &ObjectSet, count: Option<&Expr>, names, chooser: Option<&PlayerSet>| {
        Operation::Piles {
            actor: actor.clone(),
            objects: objects.clone(),
            count: count.cloned(),
            names,
            chooser: chooser.cloned(),
        }
    };
    let pile = |from: Option<&PlayerSet>, random, names, object: Option<&ObjectRef>| {
        Operation::ChoosePile {
            actor: actor.clone(),
            from: from.cloned(),
            random,
            names,
            object: object.cloned(),
            then: None,
        }
    };
    let seek = |cards: Vec<ObjectFilter>, count: Expr, top: Option<&Expr>| Operation::Seek {
        actor: actor.clone(),
        cards,
        count,
        top: top.cloned(),
    };
    let guess = |object: Option<&ObjectRef>,
                 filter: Option<&ObjectFilter>,
                 player: Option<&PlayerRef>| Operation::Guess {
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
                entry: crate::movement::battlefield_from_parts(
                    &(crate::movement::entry_from_source(entry)),
                )
                .ok()?,
            }
        }
        ("CastCopiedCardWithoutPaying", []) => cast(earlier(EarlierObjects::CopiedSpell), one()),
        ("CastASpellFromMilledCardsWithoutPaying", [Filter(spells)]) => {
            cast(matching(&earlier(EarlierObjects::Milled), spells), one())
        }
        ("CastAnyNumberOfSpellsFromOutsideTheGameWithoutPaying", [Filter(spells)]) => {
            cast(ObjectSet::OutsideTheGame(spells.clone()), Count::AnyNumber)
        }
        ("PutANameStickerOnAPermanent", [Objects(objects)]) => {
            stickers(Some(StickerKind::Name), objects.clone(), None, false)
        }
        ("PutAnArtStickerOnAPermanent", [Objects(objects)]) => {
            stickers(Some(StickerKind::Art), objects.clone(), None, false)
        }
        ("PutAPowerAndToughnessStickerOnAPermanent", [Objects(objects)]) => stickers(
            Some(StickerKind::PowerToughness),
            objects.clone(),
            None,
            false,
        ),
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
        (
            "DistrbuteUptoNumberArtStickersAmongAnyNumberOfPermanents",
            [Number(amount), Objects(objects)],
        ) => stickers(Some(StickerKind::Art), objects.clone(), Some(amount), true),
        ("SeparatePermanentsIntoNumberPiles", [Objects(objects), Number(count)]) => {
            piles(objects, Some(count), vec![], None)
        }
        ("SeparatePermanentsIntoNamedPiles", [Objects(objects), List(names)]) => {
            piles(objects, None, card_names(names)?, None)
        }
        (
            "SeparatePermanentsIntoTwoPilesAndAPlayerChoosesAPile",
            [Objects(objects), Players(chooser)],
        ) => piles(objects, Some(&Expr::Constant(2)), vec![], Some(chooser)),
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
        (
            "SeekNumberCardsFromTheTopNumberCardsOfLibrary",
            [Number(count), Filter(cards), Number(top)],
        ) => seek(vec![cards.clone()], count.clone(), Some(top)),
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
        ("StartBiddingWarAmongPlayersAtNumber", [Players(players), Number(start)]) => {
            Operation::Bid {
                players: players.clone(),
                start: Some(start.clone()),
            }
        }
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
        (
            "LookAtPlayersHandAndChooseACardForPlayerToPlayControllingThemToDoSo",
            [Player(player), Filter(filter)],
        ) => pick_from_hand(actor, player, filter, one(), HandPick::TheyPlayIt),
        (
            "LookAtPlayersHandAndChooseNumberCardsToPutOnTopOfTheirLibraryInAnyOrder",
            [Player(player), Number(count), Filter(filter)],
        ) => pick_from_hand(
            actor,
            player,
            filter,
            Count::Exactly(count.clone()),
            HandPick::TopOfLibrary,
        ),
        (
            "LookAtPlayersHandAndMayChooseACardToCastWithoutPaying",
            [Player(player), Filter(filter)],
        ) => pick_from_hand(
            actor,
            player,
            filter,
            Count::UpTo(Expr::Constant(1)),
            HandPick::CastFree,
        ),
        ("ChooseACardFromAmongCardsDiscardedThisWay", [Filter(cards), Result(result)]) => {
            choose(matching(&ObjectSet::Recorded(result.clone()), cards), one())
        }
        (
            "ChooseNumberCardsFromAmongCardsInHandRevealedThisWay",
            [Number(count), Filter(cards), Result(result)],
        ) => choose(
            matching(&ObjectSet::Recorded(result.clone()), cards),
            Count::Exactly(count.clone()),
        ),
        (
            "ChooseUptoNumberCardsFromAmongCardsInPlayersHandRevealedThisWay",
            [Number(count), Filter(cards), Player(_), Result(result)],
        ) => choose(
            matching(&ObjectSet::Recorded(result.clone()), cards),
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
        Operand::Term { form, operands, .. } if form == "And" || form == "Or" => {
            let filters = operands.iter().map(card_term).collect::<Option<Vec<_>>>()?;
            Some(if form == "And" {
                ObjectFilter::All(filters)
            } else {
                ObjectFilter::Either(filters)
            })
        }
        Operand::Term { form, operands, .. } if form == "Other" => {
            Some(ObjectFilter::Not(Box::new(card_term(operands.first()?)?)))
        }
        Operand::Term { form, operands, .. } if form == "IsCardtype" => {
            Some(ObjectFilter::CardType(vocabulary_value(operands.first()?)?))
        }
        Operand::Term { form, operands, .. } if form == "IsLandType" => {
            Some(ObjectFilter::Subtype(crate::subtype::land_type(
                &vocabulary_value(operands.first()?)?,
            )))
        }
        Operand::Term { form, operands, .. } if form == "IsNamed" => match operands.first()? {
            Operand::Term { operands, .. } => match operands.as_slice() {
                [Operand::Text(name)] => Some(ObjectFilter::Name(name.clone())),
                _ => None,
            },
            _ => None,
        },
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
    Some(match (form, operands) {
        ("WhenACardBecomesPlotted", [cards]) => EventSpec::Object {
            event: ObjectEvent::Plotted,
            objects: card_term(cards)?,
        },
        (
            "WhenACardIsPutIntoAGraveyardFromAnywhereOtherThanTheBattlefield",
            [Filter(cards), Players(owners)],
        ) => EventSpec::ZoneChangeFromElsewhere {
            objects: all(vec![cards.clone(), ObjectFilter::OwnedBy(owners.clone())]),
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
        ("WhenAManaAbilityOfAPermanentResolves", [Objects(set)]) => EventSpec::Resolves(all(vec![
            ObjectFilter::OnStack(StackTrait::Mana),
            ObjectFilter::AbilityOf(Box::new(objects_filter(set))),
        ])),
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
        ("WhenAPermanentHasAbility", [Objects(set), Operand::Term { form, .. }])
            if form == "Flying" =>
        {
            EventSpec::ObjectBecomes(all(vec![
                objects_filter(set),
                ObjectFilter::HasKeyword(Keyword::Flying),
            ]))
        }
        (
            "WhenAPermanentHasNumberCountersOfType",
            [Objects(set), Compare(comparison, value), kind],
        ) => EventSpec::ObjectBecomes(all(vec![
            objects_filter(set),
            counters_filter(kind, comparison, value)?,
        ])),
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
        (
            "WhenAPlayerSetsASchemeInMotion",
            [Players(players), Operand::Term { form, operands, .. }],
        ) if form == "IsNonSupertype" => player(
            PlayerEvent::SetsSchemeInMotion,
            players,
            Some(ObjectFilter::Not(Box::new(ObjectFilter::Supertype(
                vocabulary_value(operands.first()?)?,
            )))),
        ),
        ("WhenAPermanentIsTappedForManaOfColor", [Objects(set), mana]) => {
            EventSpec::TappedForManaOf {
                objects: objects_filter(set),
                by: None,
                mana: Box::new(mana.clone()),
            }
        }
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
        (
            "WhenAPlayerAttacksWithAnyNumberOfGroupCreatures",
            [Players(players), Objects(set), group],
        ) => EventSpec::AttacksAsGroup {
            by: players.clone(),
            attackers: objects_filter(set),
            group: Box::new(group.clone()),
        },
        ("WhenAllCreaturesAttack", [Objects(set)]) => EventSpec::AllAttack(objects_filter(set)),
        (
            "WhenAPlayerCastsASpellThatTargetsAnyNumberOfPermanents",
            [Players(players), Filter(spells), Objects(set)],
        ) => EventSpec::SpellTargeting {
            players: players.clone(),
            spells: spells.clone(),
            targets: Some(set.clone()),
            count: None,
        },
        (
            "WhenAPlayerCastsASpellWithANumberOfTargets",
            [Players(players), Filter(spells), Compare(comparison, value)],
        ) => EventSpec::SpellTargeting {
            players: players.clone(),
            spells: spells.clone(),
            targets: None,
            count: Some((comparison.clone(), value.clone())),
        },
        ("WhenAPlayerKicksASpell", [Players(players), Filter(spells)]) => spell(
            players,
            all(vec![
                spells.clone(),
                ObjectFilter::State(ObjectState::Kicked),
            ]),
        ),
        (
            "WhenAPlayerCastsASpellOrActivatesAnAbility",
            [Players(players), Filter(spells), abilities],
        ) => EventSpec::SpellOrActivation {
            players: players.clone(),
            spells: spells.clone(),
            abilities: stack_filter(abilities)?,
        },
        ("WhenAPlayerPaysLifeToActivateAnAbility", [Players(players), abilities]) => {
            EventSpec::PaysToActivate {
                players: players.clone(),
                abilities: stack_filter(abilities)?,
                payment: ActivationPayment::Life,
            }
        }
        (
            "WhenAPlayerSacrificesAnyNumberOfPermanentsToActivateAnAbility",
            [Players(players), abilities],
        ) => EventSpec::PaysToActivate {
            players: players.clone(),
            abilities: stack_filter(abilities)?,
            payment: ActivationPayment::Sacrifice,
        },
        ("WhenAPlayerChoosesTargetsForASpellOrAbility", [Players(players), abilities]) => {
            EventSpec::TargetsChosen {
                players: players.clone(),
                objects: stack_filter(abilities)?,
            }
        }
        (
            "WhenAPlayerGainsControlOfAPermanentFromAPlayer",
            [Players(to), Objects(set), Players(from)],
        ) => EventSpec::ControlChanges {
            objects: objects_filter(set),
            to: to.clone(),
            from: from.clone(),
        },
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
        (
            "WhenAPlayerUntapsAnyNumberOfPermanentDuringTheirUntapStep",
            [Players(players), Objects(set)],
        ) => player(
            PlayerEvent::UntapsDuringUntapStep,
            players,
            Some(objects_filter(set)),
        ),
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
                objects: ObjectFilter::All(vec![
                    ObjectFilter::OnStack(StackTrait::Spell),
                    spells.clone(),
                ]),
                kind: TargetingKind::SpellOrAbility,
                by: Box::new(cause.clone()),
            }
        }
        (
            "WhenAnyNumberOfPlayersAndOrPermanentsBecomeTheTargetOfASpellOrAbility",
            [Players(players), Objects(set), cause],
        ) => batch(
            EventSpec::Either(vec![
                player(PlayerEvent::BecomesTarget, players, stack_filter(cause)),
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
        (
            "WhenASpellOrAbilityCausesAPlayerToDiscardAnyNumberOfCards",
            [cause, Players(players)],
        ) => discard(
            players,
            ObjectFilter::Any,
            DiscardKind::CausedBy(stack_filter(cause)?),
            DiscardGrouping::Player,
        ),
        ("WhenAnyNumberOfPlayersDiscardAnyNumberOfCards", [Players(players), cards]) => batch(
            discard(
                players,
                card_term(cards)?,
                DiscardKind::Any,
                DiscardGrouping::Player,
            ),
            bare(),
            None,
        ),
        (
            "WhenASpellOrAbilityCausesAPlayerToLoseAnAmountOfLife",
            [cause, Players(players), Compare(comparison, value)],
        ) => caused(
            stack_filter(cause)?,
            amount_is(
                player(PlayerEvent::LosesLife, players, None),
                comparison,
                value,
            ),
        ),
        (
            "WhenAnyNumberOfPlayersEachLoseAnAmountOfLife",
            [Players(players), Compare(comparison, value)],
        ) => batch(
            amount_is(
                player(PlayerEvent::LosesLife, players, None),
                comparison,
                value,
            ),
            bare(),
            None,
        ),
        _ => return None,
    })
}

/// A casting rule by how it changes the cast, as Forge's `ReduceCost` static.
fn reduce_casting(form: &str, operands: &[Operand]) -> Option<CastingRule> {
    use Operand::{Cost as Paid, Number, Program};
    let one = Expr::Constant(1);
    let reduce = |terms: &Operand, factor: &Expr| {
        Some(CastingRule::ReduceCost(reduction_terms(terms, factor)?))
    };
    let only_if = |condition, rule| {
        Some(CastingRule::If {
            condition,
            rule: Box::new(rule),
        })
    };
    match (form, operands) {
        ("MayCastAsThoughItHadFlashWithSpecialAction", [rider @ Program(_)]) => {
            Some(CastingRule::FlashWithRider(Box::new(rider.clone())))
        }
        ("PayLifeForEachPreviousCastRatherThanManaForEachPreviousCast", [Number(life)]) => {
            Some(CastingRule::TaxPaidAsLife(life.clone()))
        }
        ("ReduceCastingCostForAlternateCost", [terms, Paid(cost)]) => {
            Some(CastingRule::ReduceForEachPaid {
                reductions: reduction_terms(terms, &one)?,
                cost: cost.clone(),
            })
        }
        ("ReduceCastingCostForEachWithMaxReduction", [terms, Number(each), cap]) => {
            let caps = reduction_terms(cap, &one)?;
            let [cap] = caps.as_slice() else {
                return None;
            };
            let mut reductions = reduction_terms(terms, each)?;
            for reduction in &mut reductions {
                reduction.amount = Expr::Minimum(
                    Box::new(reduction.amount.clone()),
                    Box::new(cap.amount.clone()),
                );
            }
            Some(CastingRule::ReduceCost(reductions))
        }
        ("ReduceCastingCostIfItTargetsASpellOrAbility", [terms, targets]) => only_if(
            Condition::TargetsMatch {
                object: ObjectRef::Source,
                kind: ObjectKind::Any,
                filter: stack_filter(targets)?,
            },
            reduce(terms, &one)?,
        ),
        ("ReduceCastingCostIfItsBargained", [terms]) => only_if(
            Condition::ObjectMatches {
                object: ObjectRef::Source,
                filter: ObjectFilter::State(ObjectState::Bargained),
            },
            reduce(terms, &one)?,
        ),
        _ => None,
    }
}

/// What an activated ability's own line changes about activating it.
pub fn reduce_activation(name: String, operands: Vec<Operand>) -> Change {
    let form = name.strip_prefix("ActivateModifier::").unwrap_or(&name);
    reduced_activation(form, &operands).map_or(
        Change::Primitive {
            name: name.clone(),
            operands: operands.clone(),
        },
        Change::ActivationRule,
    )
}

fn reduced_activation(form: &str, operands: &[Operand]) -> Option<ActivationRule> {
    use Operand::{Comparison as Compare, Cost as Paid, Number, Objects};
    let one = Expr::Constant(1);
    let restrict = |portion, color: &Operand| {
        Some(ActivationRule::RestrictMana {
            portion,
            color: match vocabulary_form(color)? {
                "TheChosenColor" => None,
                _ => Some(vocabulary_value(color)?),
            },
        })
    };
    let targeted = |terms: &Operand, condition| {
        Some(ActivationRule::Cost {
            modifier: reduced_cost(terms, &one)?,
            condition: Some(condition),
        })
    };
    match (form, operands) {
        ("ReduceCostIfItTargetsAPermanent", [Objects(set), terms]) => targeted(
            terms,
            Condition::TargetsMatch {
                object: ObjectRef::Source,
                kind: ObjectKind::Permanent,
                filter: objects_filter(set),
            },
        ),
        (
            "ReduceCostIfItTargetsANumberOfPermanent",
            [Compare(comparison, value), Objects(set), terms],
        ) => targeted(
            terms,
            Condition::TargetCount {
                object: ObjectRef::Source,
                filter: objects_filter(set),
                comparison: comparison.clone(),
                value: value.clone(),
            },
        ),
        ("ReduceCostX", [terms, Number(amount)]) => Some(ActivationRule::Cost {
            modifier: reduced_cost(terms, amount)?,
            condition: None,
        }),
        ("ReduceManaCostForEachAlternateCost", [Paid(cost)]) => {
            Some(ActivationRule::ManaInstead(cost.clone()))
        }
        ("SpendOnlyColoredMana", [color]) => restrict(ManaCostPortion::All, color),
        ("SpendOnlyColoredManaOnX", [color]) => restrict(ManaCostPortion::X, color),
        _ => None,
    }
}

/// Rules past the first passes, by the shape Forge's statics give them.
fn reduce_more_abilities(name: &str, operands: &[Operand]) -> Option<Ability> {
    use Operand::{Filter, List, Objects, Players, Term};
    let form = name.strip_prefix("Rule::").unwrap_or(name);
    let again = |cause, abilities: &Operand| {
        Some(Ability::TriggerRule {
            cause: Some(cause),
            abilities: Some(stack_filter(abilities)?),
            rule: TriggerRule::TriggersAgain,
        })
    };
    let spell = |event, players: &PlayerSet, spells: &ObjectFilter| EventSpec::Spell {
        event,
        players: players.clone(),
        spells: spells.clone(),
    };
    match (form, operands) {
        (
            "APermanentBecomingTheTargetOfASpellOrAbilityCausesAbilitiesToTriggerAnAdditionalTime",
            [Objects(set), by, abilities],
        ) => again(
            EventSpec::BecomesTarget {
                objects: objects_filter(set),
                kind: TargetingKind::SpellOrAbility,
                by: Box::new(by.clone()),
            },
            abilities,
        ),
        (
            "APermanentBeingDealtDamageCausesAbilitiesToTriggerAnAdditionalTime",
            [Objects(set), abilities],
        ) => again(
            EventSpec::Damage(DamageEvent {
                source_kind: ObjectKind::Any,
                sources: ObjectFilter::Any,
                recipients: DamageRecipients::Objects(objects_filter(set)),
                kind: DamageKind::Any,
                grouping: DamageGrouping::Recipient,
                subject: DamageSubject::Recipient,
            }),
            abilities,
        ),
        (
            "APermanentDealingCombatDamageToAPlayerCausesAbilitiesToTriggerAnAdditionalTime",
            [Objects(set), Players(players), abilities],
        ) => again(
            EventSpec::Damage(DamageEvent {
                source_kind: ObjectKind::Permanent,
                sources: objects_filter(set),
                recipients: DamageRecipients::Players(players.clone()),
                kind: DamageKind::Combat,
                grouping: DamageGrouping::SourceRecipient,
                subject: DamageSubject::Source,
            }),
            abilities,
        ),
        (
            "APlayerCastingOrCopyingASpellCausesAnAbilityToTriggerAnAdditionalTime",
            [Players(players), Filter(spells), abilities],
        ) => again(
            EventSpec::Either(vec![
                spell(SpellEvent::Cast, players, spells),
                spell(SpellEvent::Copy, players, spells),
            ]),
            abilities,
        ),
        ("ActivatedAbilityEffect", [abilities, Term { form, operands, .. }]) => {
            let [Operand::Cost(cost)] = operands.as_slice() else {
                return None;
            };
            if form != "AdditionalCost" {
                return None;
            }
            Some(Ability::Static {
                kind: StaticKind::Ordinary,
                affected: EffectSubjects::Players(PlayerSet::All),
                changes: vec![Change::PlayerRule(PlayerRule::ActivationCost {
                    abilities: stack_filter(abilities)?,
                    change: ActivationCostChange::Additional(cost.clone()),
                    floor: None,
                })],
            })
        }
        ("StackEffect", [any, List(effects)]) => {
            let [Operand::Vocabulary { form, .. }] = effects.as_slice() else {
                return None;
            };
            if form != "CantBeCountered" {
                return None;
            }
            Some(Ability::Static {
                kind: StaticKind::Ordinary,
                affected: EffectSubjects::Objects(ObjectSet::AnyZone(stack_filter(any)?)),
                changes: vec![Change::Rule(RuleChange::Prohibit(ObjectAction::Counter))],
            })
        }
        _ => None,
    }
}

fn group_constraint(term: &Operand) -> Option<SelectionConstraint> {
    use DistinctProperty as D;
    use SelectionConstraint as C;
    let (form, operands): (&str, &[Operand]) = match term {
        Operand::Term { form, operands, .. } => (form, operands),
        Operand::Vocabulary { form, .. } => (form, &[]),
        _ => return None,
    };
    Some(match (form, operands) {
        ("ShareAColor", []) => C::CommonProperty(D::Colors),
        ("ShareACardType", []) => C::CommonProperty(D::CardTypes),
        ("ShareACreatureType", []) => C::CommonProperty(D::CreatureTypes),
        ("SameNames", []) => C::CommonProperty(D::Names),
        ("ShareAllCardTypes", []) => C::AllSameProperty(D::CardTypes),
        ("HasAColorNotInCommon", []) => C::NoCommonProperty(D::Colors),
        ("DifferentNames", []) => C::DifferentNames,
        ("TotalPowerIs", [Operand::Comparison(comparison, amount)]) => C::TotalProperty {
            property: Property::Power,
            comparison: comparison.clone(),
            amount: amount.clone(),
        },
        _ => return None,
    })
}

fn turn_history(event: EventSpec, measure: HistoryMeasure) -> Expr {
    Expr::EventHistory {
        event: Box::new(event),
        window: HistoryWindow::ThisTurn,
        measure,
    }
}

fn occurred(event: EventSpec) -> Expr {
    turn_history(event, HistoryMeasure::Occurrences)
}

fn dealt(sources: ObjectFilter, recipients: DamageRecipients, kind: DamageKind) -> EventSpec {
    EventSpec::Damage(DamageEvent {
        source_kind: ObjectKind::Any,
        sources,
        recipients,
        kind,
        grouping: DamageGrouping::SourceRecipient,
        subject: DamageSubject::Source,
    })
}

fn left_graveyard_filter(owners: &PlayerSet, cards: &ObjectFilter) -> ObjectFilter {
    ObjectFilter::All(vec![cards.clone(), ObjectFilter::OwnedBy(owners.clone())])
}

/// What happened this turn or this way, read as a condition.
fn reduce_more_conditions(form: &str, operands: &[Operand]) -> Option<Condition> {
    use EventZone as Z;
    use Operand::{Comparison as Compare, Filter, Object, Objects, Player, Players, Result as Run};
    let some_happened = |event| some_of(occurred(event));
    let subset = |objects: ObjectSet, comparison: &Comparison, amount: &Expr, group: &Operand| {
        Some(Condition::SubsetMeets {
            objects,
            comparison: comparison.clone(),
            amount: amount.clone(),
            constraints: vec![group_constraint(group)?],
        })
    };
    Some(match (form, operands) {
        (
            "ANumberOfCardsWerePutIntoPlayersGraveyardFromAnywhereOtherThanTheBattlefieldThisTurn",
            [Compare(comparison, value), Filter(cards), Players(owners)],
        ) => compared(
            occurred(EventSpec::ZoneChangeFromElsewhere {
                objects: left_graveyard_filter(owners, cards),
                except: vec![Z::Battlefield],
                to: Z::Graveyard,
            }),
            comparison,
            value,
        ),
        ("ANumberOfCreaturesAttackedThisTurn", [Compare(comparison, value), Objects(set)]) => {
            compared(
                occurred(EventSpec::Object {
                    event: ObjectEvent::Attacks,
                    objects: objects_filter(set),
                }),
                comparison,
                value,
            )
        }
        ("ANumberOfGraveyardCardsLeftThisTurn", [Compare(comparison, value), Filter(cards)]) => {
            compared(
                occurred(EventSpec::Object {
                    event: ObjectEvent::LeavesGraveyard,
                    objects: cards.clone(),
                }),
                comparison,
                value,
            )
        }
        ("ANumberOfSourcesDealtDamageThisTurn", [Compare(comparison, value), Filter(sources)]) => {
            compared(
                turn_history(
                    dealt(sources.clone(), DamageRecipients::Any, DamageKind::Any),
                    HistoryMeasure::DistinctSources,
                ),
                comparison,
                value,
            )
        }
        ("ASourceDealtDamageThisTurn", [Filter(sources), Compare(comparison, value)]) => compared(
            turn_history(
                dealt(sources.clone(), DamageRecipients::Any, DamageKind::Any),
                HistoryMeasure::MostFromOneSource,
            ),
            comparison,
            value,
        ),
        ("SourcesDealtNonCombatDamageThisTurn", [Filter(sources), Compare(comparison, value)]) => {
            compared(
                turn_history(
                    dealt(
                        sources.clone(),
                        DamageRecipients::Any,
                        DamageKind::Noncombat,
                    ),
                    HistoryMeasure::TotalAmount,
                ),
                comparison,
                value,
            )
        }
        ("ExcessDamageWasDealtToACreatureOrPlaneswalkerThisTurn", [Objects(set)]) => {
            some_happened(dealt(
                ObjectFilter::Any,
                DamageRecipients::Objects(objects_filter(set)),
                DamageKind::Excess,
            ))
        }
        ("APermanentWasDestroyedByASpellOrAbilityThisTurn", [Objects(set), cause]) => {
            some_happened(caused(
                stack_filter(cause)?,
                EventSpec::Object {
                    event: ObjectEvent::Destroyed,
                    objects: objects_filter(set),
                },
            ))
        }
        ("ASpellWasCountedByASpellOrAbilityThisTurn", [Filter(spells), cause]) => {
            some_happened(caused(
                stack_filter(cause)?,
                EventSpec::Object {
                    event: ObjectEvent::Countered,
                    objects: spells.clone(),
                },
            ))
        }
        ("APlayerTurnedAPermanentFaceUpThisTurn", [Players(players), Objects(set)]) => {
            some_happened(EventSpec::Player {
                event: PlayerEvent::TurnsFaceUp,
                players: players.clone(),
                objects: Some(objects_filter(set)),
            })
        }
        ("NoPermanentsLeftTheBattlefieldThisTurn", [Objects(set)]) => {
            none_of(occurred(EventSpec::Object {
                event: ObjectEvent::LeavesBattlefield,
                objects: objects_filter(set),
            }))
        }
        ("CardIsOnlyCardInPlayersGraveyard", [Object(object), Filter(cards), Player(player)]) => {
            Condition::All(vec![
                Condition::ObjectMatches {
                    object: object.clone(),
                    filter: cards.clone(),
                },
                exactly(
                    count_of(zone_set(
                        Z::Graveyard,
                        PlayerSet::One(player.clone()),
                        cards.clone(),
                    )),
                    1,
                ),
            ])
        }
        ("CastSpellOrActivatedAbilityPassesFilter", [term]) => Condition::ObjectMatches {
            object: ObjectRef::EventStackObject,
            filter: stack_filter(term)?,
        },
        ("SpellOrAbilityPassesFilter", [Object(object), term]) => Condition::ObjectMatches {
            object: object.clone(),
            filter: stack_filter(term)?,
        },
        ("ManaFromAPermanentWasSpentToActivateThisAbility", [Objects(set)]) => {
            Condition::ObjectMatches {
                object: ObjectRef::Source,
                filter: ObjectFilter::ManaSpentFrom(Box::new(objects_filter(set))),
            }
        }
        ("NumberOfColorsOfManaSpentToActivateThisAbilityIs", [Compare(comparison, value)]) => {
            compared(
                Expr::ManaSpent {
                    object: ObjectRef::Source,
                    filter: SpentManaFilter {
                        colors: None,
                        snow: false,
                    },
                    aggregation: SpentManaAggregation::DistinctColors,
                },
                comparison,
                value,
            )
        }
        (
            "TheNumberOfCardsOfTypeInPlayersLibraryIs",
            [Filter(cards), Player(player), Compare(comparison, value)],
        ) => compared(
            count_of(zone_set(
                Z::Library,
                PlayerSet::One(player.clone()),
                cards.clone(),
            )),
            comparison,
            value,
        ),
        ("TheTotalManaValueOfExiledCardsIs", [Objects(set), Compare(comparison, value)]) => {
            compared(
                Expr::Aggregate {
                    objects: Box::new(set.clone()),
                    property: Property::ManaValue,
                    aggregation: Aggregation::Sum,
                },
                comparison,
                value,
            )
        }
        ("TriggerChoseCreatureAsRingBearer", [Objects(set)]) => Condition::ObjectMatches {
            object: ObjectRef::RingBearer(PlayerRef::AbilityController),
            filter: objects_filter(set),
        },
        ("TriggerXIs", [Compare(comparison, value)]) => {
            compared(Expr::CastX(ObjectRef::EventSubject), comparison, value)
        }
        ("WhenAPermanentBecomesTapped_NotTappedForAttacking", []) => {
            negated(Condition::Outcome(PriorOutcome::TappedForAttacking))
        }
        ("PermanentsPassGroupFilter", [Objects(set), group]) => subset(
            set.clone(),
            &Comparison::Equal,
            &count_of(set.clone()),
            group,
        )?,
        (
            "NumGroupCardsWereMilledThisWay" | "ANumberOfGroupCardsWereExiledThisWay",
            [Compare(comparison, value), Filter(cards), group, Run(result)],
        ) => subset(
            matching(&ObjectSet::Recorded(result.clone()), cards),
            comparison,
            value,
            group,
        )?,
        _ => return None,
    })
}

/// Numbers past the first passes, each a measure of what the game recorded.
fn reduce_more_numbers(form: &str, operands: &[Operand]) -> Option<Expr> {
    use EventZone as Z;
    use GameValue as G;
    use Operand::{Filter, Object, Objects, Player, Players, Result as Run, Text};
    let exiled_with = |object: &ObjectRef| ObjectSet::Zone {
        scope: ZoneScope::Shared(Z::Exile),
        kind: ObjectKind::Card,
        filter: ObjectFilter::ExiledWith(object.clone()),
    };
    let earlier_count = |what, filter: Option<ObjectFilter>| Expr::Earlier {
        what,
        measure: ObjectMeasure::Count,
        result: None,
        filter: filter.map(Box::new),
        by: None,
    };
    let draft = |kind, player: &PlayerRef, card: &str| {
        Some(DraftNote {
            kind,
            player: player.clone(),
            card: card.into(),
        })
    };
    let cast_before = |spells: &ObjectFilter, players: PlayerSet, object: &ObjectRef| Expr::AsOf {
        moment: Moment::Cast(object.clone()),
        value: Box::new(occurred(EventSpec::Spell {
            event: SpellEvent::Cast,
            players,
            spells: spells.clone(),
        })),
    };
    let from_sources = |sources| Expr::ManaSpentFrom {
        object: ObjectRef::Source,
        sources: Box::new(sources),
    };
    let attacked_by = |attacker: &PlayerRef, binding: LoopBinding| PlayerSet::Matching {
        binding: binding.clone(),
        condition: Box::new(Condition::PlayerAttacks {
            attacker: attacker.clone(),
            defender: PlayerRef::Bound(binding),
        }),
    };
    Some(match (form, operands) {
        ("AmountOfExcessDamageDealtThisWay", []) => Expr::Game(G::ExcessDamageThisWay),
        ("AsLoseUnspentMana_AmountOfUnspentMana", []) => Expr::Game(G::ManaLostThisWay),
        ("EachableNumber", []) => Expr::Game(G::EachValue),
        ("TheGreatestNumberOfCardsDiscardedThisWay", []) => Expr::Game(G::MostDiscardedThisWay),
        ("LifePaidWithVanguard", [_]) => Expr::Game(G::LifePaid),
        ("DamageDealtByResolvedSpell", [Object(object)]) => Expr::DamageBy(object.clone()),
        ("EachAdditionalManaCostPaid", symbols) => Expr::AdditionalCostPaid(symbols.to_vec()),
        ("HighestNumberPlayerNotedForCardDuringDraft", [Player(player), Text(card)]) => {
            Expr::DraftNoted {
                note: draft(DraftNoteKind::Number, player, card)?,
                highest: true,
            }
        }
        ("NumCardsPlayerRemovedWithCardDuringDraft", [Player(player), Text(card)]) => {
            Expr::DraftNoted {
                note: draft(DraftNoteKind::Removed, player, card)?,
                highest: false,
            }
        }
        ("ManaValueOfTheExiledCardUsedToCraftPermanent", [Object(object)]) => Expr::Aggregate {
            objects: Box::new(exiled_with(object)),
            property: Property::ManaValue,
            aggregation: Aggregation::Sum,
        },
        ("TheNumberOfColorsAmongTheCardsUsedToCraftPermanent", [Object(object)]) => {
            Expr::CountDistinct {
                objects: Box::new(exiled_with(object)),
                property: DistinctProperty::Colors,
            }
        }
        ("NumPermanentsAttachedToDeadPermanent", [Objects(set)]) => count_of(filtered(
            set,
            ObjectFilter::WasAttachedTo(ObjectRef::EventSubject),
        )),
        ("NumPermanentsDevouredByEnteringPermanent", [Objects(set)]) => {
            earlier_count(EarlierObjects::Devoured, Some(objects_filter(set)))
        }
        (
            "TheNumberOfPermanentsSacrificedAsPermenantEnteredBattlefield",
            [Objects(set), Object(_)],
        ) => earlier_count(EarlierObjects::Sacrificed, Some(objects_filter(set))),
        (
            "NumSpellsCastByAnyPlayerBeforeSpellThisTurn",
            [Filter(spells), Players(players), Object(object)],
        ) => cast_before(spells, players.clone(), object),
        (
            "NumSpellsCastByPlayerBeforeSpellThisTurn",
            [Filter(spells), Player(player), Object(object)],
        ) => cast_before(spells, PlayerSet::One(player.clone()), object),
        ("NumberOfColorManaSymbolsInManaCostOfTheSacrificedPermanent", [color]) => {
            Expr::CountManaSymbols {
                objects: Box::new(ObjectSet::Earlier {
                    what: EarlierObjects::Sacrificed,
                    by: None,
                }),
                colors: vec![vocabulary_value(color)?],
            }
        }
        ("TheAmountOfManaFromATreasureSpentToActivateThisAbility", []) => {
            from_sources(ObjectFilter::Subtype(
                serde_json::from_value(serde_json::Value::String("Treasure".into())).ok()?,
            ))
        }
        ("TheNumberGuessedThisWay", [Run(result)]) => Expr::BoundValue(result.clone()),
        ("TheAmountOfManaFromSourcesSpentToCastIt", [Operand::Term { form, operands, .. }])
            if form == "IsCardtype" =>
        {
            from_sources(ObjectFilter::CardType(vocabulary_value(operands.first()?)?))
        }
        ("TheAmountOfEnergyPaidOrLostByPlayersThisTurn", [Players(players)]) => {
            Expr::AmongPlayers {
                players: players.clone(),
                number: PlayerNumber::EnergyPaidOrLostThisTurn,
                aggregation: Aggregation::Sum,
            }
        }
        ("TheGreatestNumberOfCardsDrawnByAPlayerThisTurn", [Players(players)]) => {
            Expr::AmongPlayers {
                players: players.clone(),
                number: PlayerNumber::CardsDrawnThisTurn,
                aggregation: Aggregation::Maximum,
            }
        }
        (
            "TheGreatestAmongOfDamageDealtByASourceToAPlayerOrAPermanentThisTurn",
            [Filter(sources), Players(players), Objects(set)],
        ) => turn_history(
            dealt(
                sources.clone(),
                DamageRecipients::ObjectsAndPlayers {
                    objects: objects_filter(set),
                    players: players.clone(),
                },
                DamageKind::Any,
            ),
            HistoryMeasure::MostFromOneSource,
        ),
        ("TheNumberOfLettersInNameStickersOnPermanent", [letter, Object(object)]) => {
            Expr::Stickers {
                objects: Box::new(ObjectSet::One(object.clone())),
                measure: StickerMeasure::Letter(Box::new(letter.clone())),
            }
        }
        ("TheNumberOfUniqueVowelsOnNameSticker", [_]) => Expr::Stickers {
            objects: Box::new(ObjectSet::Earlier {
                what: EarlierObjects::It,
                by: None,
            }),
            measure: StickerMeasure::UniqueVowels,
        },
        ("ManaValueOfYourCommander", []) => Expr::Aggregate {
            objects: Box::new(ObjectSet::AnyZone(ObjectFilter::All(vec![
                ObjectFilter::State(ObjectState::Commander),
                ObjectFilter::OwnedBy(PlayerSet::One(PlayerRef::AbilityController)),
            ]))),
            property: Property::ManaValue,
            aggregation: Aggregation::Sum,
        },
        ("NumManaSymbolsInCostOfSpell", [symbol, Object(object)]) => Expr::CountManaSymbols {
            objects: Box::new(ObjectSet::One(object.clone())),
            colors: vec![match symbol {
                Operand::Term { form, .. } | Operand::Vocabulary { form, .. } => {
                    match form.as_str() {
                        "ManaCostW" => ManaColor::White,
                        "ManaCostU" => ManaColor::Blue,
                        "ManaCostB" => ManaColor::Black,
                        "ManaCostR" => ManaColor::Red,
                        "ManaCostG" => ManaColor::Green,
                        _ => return None,
                    }
                }
                _ => return None,
            }],
        },
        ("TheNumberOfChosenColorsItIs", []) => Expr::ChosenColorsOf(ObjectRef::Source),
        ("TheNumberOfChosenColorsSpellIs", [Object(object)]) => {
            Expr::ChosenColorsOf(object.clone())
        }
        (
            "TheNumberOfColorsAmongPermanentsAndSpellsCastThisTurn",
            [Objects(set), Filter(spells)],
        ) => Expr::CountDistinct {
            objects: Box::new(ObjectSet::Union(vec![
                set.clone(),
                spells_cast_this_turn_set(spells),
            ])),
            property: DistinctProperty::Colors,
        },
        ("TheNumberOfCardsInHandRevealedThisWayThatShareAManaValue", []) => {
            Expr::SharingWithAnother {
                objects: Box::new(ObjectSet::Earlier {
                    what: EarlierObjects::Revealed,
                    by: None,
                }),
                property: DistinctProperty::ManaValues,
            }
        }
        ("TheNumberOfAbilitiesAmongPermanents", [abilities, Objects(set)]) => {
            Expr::AbilitiesAmong {
                abilities: Box::new(abilities.clone()),
                objects: Box::new(set.clone()),
            }
        }
        // "Permanents you've sacrificed this turn": the sacrifices that player made.
        ("TheNumberOfCardTypesAmongThePermanentsSacrificedByPlayerThisTurn", [Player(player)]) => {
            turn_history(
                EventSpec::Player {
                    event: PlayerEvent::Sacrifices,
                    players: PlayerSet::One(player.clone()),
                    objects: Some(ObjectFilter::Any),
                },
                HistoryMeasure::Distinct(DistinctProperty::CardTypes),
            )
        }
        (
            "TheNumberOfCardsThatWerePutIntoAPlayersGraveyardFromTheirHandOrLibraryThisTurn",
            [Players(owners)],
        ) => {
            let from = |zone| {
                zone_change(
                    ObjectFilter::OwnedBy(owners.clone()),
                    Some(zone),
                    Z::Graveyard,
                )
            };
            occurred(EventSpec::Either(vec![from(Z::Hand), from(Z::Library)]))
        }
        ("TheNumberOfCountersOfTypeMovedThisWay", [_, Run(result)]) => {
            Expr::Performed(result.clone())
        }
        ("TheNumberOfCreaturesThatConvokedPermanent", [Object(object)]) => {
            count_of(ObjectSet::battlefield(history(
                ObjectHistory::Convoked(object.clone()),
                ObjectWindow::ThisGame,
            )))
        }
        ("TheNumberOfPlayersAttackedByPlayerThisCombat", [Players(players), Player(player)]) => {
            Expr::CountPlayers(PlayerSet::Intersection(vec![
                players.clone(),
                attacked_by(player, LoopBinding(1)),
            ]))
        }
        (
            "TheNumberOfSpellsCastByAnyPlayerSinceTheBeginningOfPlayersLastTurn",
            [Filter(spells), Players(players), Player(player)],
        ) => Expr::EventHistory {
            event: Box::new(EventSpec::Spell {
                event: SpellEvent::Cast,
                players: players.clone(),
                spells: spells.clone(),
            }),
            window: HistoryWindow::SinceLastTurnOf(player.clone()),
            measure: HistoryMeasure::Occurrences,
        },
        ("TheNumberOfTimesModeChosenForSpell", [Object(object)]) => {
            Expr::ModesChosen(object.clone())
        }
        ("TheNumberOfTurnsPlayerHasBegunSinceItWasForetold", []) => Expr::TurnsSinceForetold,
        ("TheTotalNumberOfCountersOfTypeAmongCardsInGraveyards", [kind, Filter(cards)]) => {
            Expr::CountCounters {
                recipients: Box::new(Recipients::Objects(zone_set(
                    Z::Graveyard,
                    PlayerSet::All,
                    cards.clone(),
                ))),
                kind: Some(counter_kind(kind)?),
                information: Information::CurrentOrLastKnown,
            }
        }
        ("TheTotalPowerOfPermanentsThatDiedThisTurn", [Objects(set)]) => turn_history(
            zone_change(objects_filter(set), Some(Z::Battlefield), Z::Graveyard),
            HistoryMeasure::Property(Property::Power, Aggregation::Sum),
        ),
        ("TotalManaValueOfSpells", [Filter(spells)]) => Expr::Aggregate {
            objects: Box::new(ObjectSet::Zone {
                scope: ZoneScope::Shared(Z::Stack),
                kind: ObjectKind::Spell,
                filter: spells.clone(),
            }),
            property: Property::ManaValue,
            aggregation: Aggregation::Sum,
        },
        ("Trigger_NumberOfPlayersBeingAttacked", []) => Expr::CountPlayers(PlayerSet::Matching {
            binding: LoopBinding(1),
            condition: Box::new(Condition::PlayerState {
                player: PlayerRef::Bound(LoopBinding(1)),
                state: PlayerState::BeingAttacked,
            }),
        }),
        _ => return None,
    })
}

/// Facts about one player, tested as that player is bound.
fn more_player_fact(form: &str, player: &PlayerRef, rest: &[Operand]) -> Option<Condition> {
    use EventZone as Z;
    use Operand::{Comparison as Compare, Filter, Player, Players, Result as Run, Text};
    let one = || PlayerSet::One(player.clone());
    let state = |state| {
        Some(Condition::PlayerState {
            player: player.clone(),
            state,
        })
    };
    let by_them = |what, filter: Option<ObjectFilter>, result: Option<&ResultId>| Expr::Earlier {
        what,
        measure: ObjectMeasure::Count,
        result: result.cloned(),
        filter: filter.map(Box::new),
        by: Some(player.clone()),
    };
    let draft = |kind, by: &PlayerRef, card: &str| Condition::NotedForDraft {
        player: player.clone(),
        note: DraftNote {
            kind,
            player: by.clone(),
            card: card.into(),
        },
    };
    let attacks = |defender: PlayerRef| Condition::PlayerAttacks {
        attacker: player.clone(),
        defender,
    };
    let hand = |filter| count_of(zone_set(Z::Hand, one(), filter));
    Some(match (form, rest) {
        ("AllCardsInHandAre", [cards, named]) => exactly(
            hand(ObjectFilter::All(vec![
                card_term(cards)?,
                ObjectFilter::Not(Box::new(card_term(named)?)),
            ])),
            0,
        ),
        ("ChoseAFirstPermanentThisWay", []) => Condition::ChoseNth {
            player: player.clone(),
            nth: 1,
        },
        ("ChoseASecondPermanentThisWay", []) => Condition::ChoseNth {
            player: player.clone(),
            nth: 2,
        },
        ("ControlsNum", [Operand::Term { form, .. }, Operand::Objects(set)]) if form == "Prime" => {
            Condition::IsPrime(count_of(filtered(set, ObjectFilter::ControlledBy(one()))))
        }
        ("CouldMulligan", []) => return state(PlayerState::CouldMulligan),
        ("DidntDiscardedACardOfTypeThisWay", [Filter(cards)]) => none_of(by_them(
            EarlierObjects::Discarded,
            Some(cards.clone()),
            None,
        )),
        (
            "DidntDiscardedNumberCardsOfTypeThisWay",
            [Compare(comparison, value), cards, Run(result)],
        ) => negated(compared(
            by_them(
                EarlierObjects::Discarded,
                Some(card_term(cards)?),
                Some(result),
            ),
            comparison,
            value,
        )),
        ("RevealedACardOfTheirChosenName", []) => some_of(by_them(
            EarlierObjects::Revealed,
            Some(ObjectFilter::Chosen(ValueKind::CardName)),
            None,
        )),
        ("DidntRevealACardOfTheirChosenName", []) => none_of(by_them(
            EarlierObjects::Revealed,
            Some(ObjectFilter::Chosen(ValueKind::CardName)),
            None,
        )),
        ("DidntWinTheSubgameThisWay", [Run(result)]) => negated(Condition::WonSubgame {
            player: player.clone(),
            result: result.clone(),
        }),
        ("EveryCardInTheirCardPoolStartedTheGameInTheirLibraryOrTheCommandZone", []) => {
            return state(PlayerState::WholePoolInDeck)
        }
        ("GuessedCorrectlyForDraftCard", [Text(card)]) => {
            draft(DraftNoteKind::Guessed, player, card)
        }
        ("APlayerNotedByPlayerForCardDuringDraft", [Player(by), Text(card)]) => {
            draft(DraftNoteKind::Player, by, card)
        }
        ("HadANumberOfCardsInTheirStartingDeck", [Compare(comparison, value)]) => compared(
            Expr::PlayerNumber {
                player: player.clone(),
                number: PlayerNumber::StartingDeckSize,
            },
            comparison,
            value,
        ),
        ("HasNotCompletedDungeon", [Text(name)]) => negated(Condition::CompletedDungeon {
            player: player.clone(),
            name: name.clone(),
        }),
        ("HaventAddedManaWithThisAbility", []) => {
            exactly(Expr::Game(GameValue::ResolutionsThisTurn), 0)
        }
        ("IsAttackingNumberPlayers", [Compare(comparison, value), Players(players)]) => compared(
            Expr::CountPlayers(PlayerSet::Intersection(vec![
                players.clone(),
                PlayerSet::Matching {
                    binding: LoopBinding(1),
                    condition: Box::new(attacks(PlayerRef::Bound(LoopBinding(1)))),
                },
            ])),
            comparison,
            value,
        ),
        ("IsNotAttackingAPlayer", [Players(players)]) => negated(Condition::ExistsPlayer {
            players: players.clone(),
            binding: LoopBinding(1),
            condition: Box::new(attacks(PlayerRef::Bound(LoopBinding(1)))),
        }),
        ("NumCardsInHandAtBeginningOfTurnWas", [Compare(comparison, value)]) => compared(
            Expr::AsOf {
                moment: Moment::TurnBegan,
                value: Box::new(Expr::PlayerNumber {
                    player: player.clone(),
                    number: PlayerNumber::CardsInHand,
                }),
            },
            comparison,
            value,
        ),
        ("NumCardtypesOnBattlefiendAndInGraveyardIs", [Compare(comparison, value)]) => compared(
            Expr::CountDistinct {
                objects: Box::new(ObjectSet::Union(vec![
                    ObjectSet::battlefield(ObjectFilter::ControlledBy(one())),
                    zone_set(Z::Graveyard, one(), ObjectFilter::Any),
                ])),
                property: DistinctProperty::CardTypes,
            },
            comparison,
            value,
        ),
        ("Opponent", []) => Condition::PlayerIn {
            player: player.clone(),
            players: PlayerSet::OpponentsOf(PlayerRef::AbilityController),
        },
        ("OwnedAPermanentChosenThisWay", [Run(result)]) => at_least(
            count_of(matching(
                &ObjectSet::Recorded(result.clone()),
                &ObjectFilter::OwnedBy(one()),
            )),
            1,
        ),
        ("OwnsASpell", [Filter(spells)]) => at_least(
            count_of(ObjectSet::Zone {
                scope: ZoneScope::Shared(Z::Stack),
                kind: ObjectKind::Spell,
                filter: ObjectFilter::All(vec![spells.clone(), ObjectFilter::OwnedBy(one())]),
            }),
            1,
        ),
        ("PossibleDefendingPlayerThisCombat", []) => return state(PlayerState::PossibleDefender),
        ("RolledHighestD20Value", []) => return state(PlayerState::RolledHighest),
        ("ShuffledLibraryThisWay", []) => return state(PlayerState::ShuffledThisWay),
        ("SpellDefendingPlayer", []) => return state(PlayerState::BeingAttacked),
        _ => return None,
    })
}

/// A step taken with cards being looked at, revealed or found, handed first
/// as the cards that step works with.
pub fn reduce_inspection(actor: PlayerSet, name: String, operands: Vec<Operand>) -> Operation {
    let reduced = name
        .split_once("::")
        .and_then(|(_, form)| match operands.as_slice() {
            [Operand::Objects(cards), rest @ ..] => inspection_step(&actor, form, cards, rest)
                .or_else(|| tail_inspection_step(&actor, form, cards, rest)),
            _ => None,
        });
    reduced.unwrap_or(Operation::Primitive {
        actor,
        name,
        operands,
    })
}

fn inspection_step(
    actor: &PlayerSet,
    form: &str,
    cards: &ObjectSet,
    rest: &[Operand],
) -> Option<Operation> {
    use CardPlayAction as A;
    use Destination as D;
    use Operand::{Filter, List, Number, Player, Players};
    let free = || PlayTerms {
        cost: PlayCost::Free,
        ..play_terms()
    };
    let may_one = || Count::UpTo(Expr::Constant(1));
    let exactly_one = || Count::Exactly(Expr::Constant(1));
    let cast =
        |who: &PlayerSet, cards, count| cast_with(who, A::CastSpell, cards, count, free(), None);
    let put = |who: &PlayerSet, cards, count, destination, entry: &[Operand]| {
        Operation::DraftChangeZone {
            actor: who.clone(),
            cards,
            count,
            random: false,
            group: None,
            destination,
            entry: entry.to_vec(),
            face_down: None,
        }
    };
    let grouped = |cards, group: Operand, destination| Operation::DraftChangeZone {
        actor: actor.clone(),
        cards,
        count: Some(Count::AnyNumber),
        random: false,
        group: Some(Box::new(group)),
        destination,
        entry: vec![],
        face_down: None,
    };
    let flag = |form: &str| Operand::Vocabulary {
        family: "ExileFlag".into(),
        form: form.into(),
    };
    let per_type = |form: &str| Operand::Vocabulary {
        family: "GroupFilter".into(),
        form: form.into(),
    };
    let piles = |names: &[&str]| Operation::Piles {
        actor: actor.clone(),
        objects: cards.clone(),
        count: Some(Expr::Constant(2)),
        names: names.iter().map(|name| name.to_string()).collect(),
        chooser: None,
    };
    let pile = |who: PlayerSet, then| Operation::ChoosePile {
        actor: who,
        from: None,
        random: false,
        names: vec![],
        object: None,
        then: Some(then),
    };
    let flags = |operand: &Operand| match operand {
        List(items) => items.clone(),
        single => vec![single.clone()],
    };
    Some(match (form, rest) {
        ("MayCastASpellFromAmongThemWithoutPaying", [Filter(spells)]) => {
            cast(actor, matching(cards, spells), may_one())
        }
        ("APlayerMayCastASpellFromAmongThemWithoutPaying", [Players(who), Filter(spells)]) => {
            cast(who, matching(cards, spells), may_one())
        }
        (
            "APlayerMayCastUptoNumberSpellsFromAmongThemWithoutPaying",
            [Players(who), Number(amount), Filter(spells)],
        ) => cast(who, matching(cards, spells), Count::UpTo(amount.clone())),
        ("MayCastFoundCardsWithoutPaying", []) => cast(actor, cards.clone(), Count::AnyNumber),
        ("CastFoundCardsWithoutPaying", []) => cast(actor, cards.clone(), exactly_one()),
        (
            "MayPutFoundCardsOntoBattlefield" | "PutAnyNumberOfFoundCardsOntoBattlefield",
            [entry],
        ) => put(
            actor,
            cards.clone(),
            Some(Count::AnyNumber),
            D::OwnersBattlefield,
            &flags(entry),
        ),
        ("PutNumberFoundCardsOntoBattlefield", [Number(amount), entry]) => put(
            actor,
            cards.clone(),
            Some(Count::Exactly(amount.clone())),
            D::OwnersBattlefield,
            &flags(entry),
        ),
        (
            "APlayerMayPutACardOfTypeOntoTheBattlefield",
            [Players(who), Filter(wanted), entry @ ..],
        ) => put(
            who,
            matching(cards, wanted),
            Some(may_one()),
            D::OwnersBattlefield,
            entry,
        ),
        (
            "MayPutUptoNumberCardsOfTypeOntoTheBattlefield",
            [Number(amount), Filter(wanted), entry],
        ) => put(
            actor,
            matching(cards, wanted),
            Some(Count::UpTo(amount.clone())),
            D::OwnersBattlefield,
            &flags(entry),
        ),
        ("PutEachCardOfTypeIntoGraveyard", [Filter(wanted)]) => put(
            actor,
            matching(cards, wanted),
            None,
            D::OwnersGraveyard,
            &[],
        ),
        ("MayPutAnyNumberOfGroupCardsIntoHand", [Filter(wanted), group]) => {
            grouped(matching(cards, wanted), group.clone(), D::OwnersHand)
        }
        ("MayPutACardOfEachCardtypeIntoHand", []) => {
            grouped(cards.clone(), per_type("OnePerCardType"), D::OwnersHand)
        }
        ("MayExileACardOfEachCardType", []) => {
            grouped(cards.clone(), per_type("OnePerCardType"), D::Exile)
        }
        ("MayPutACardOfEachCardtypeAmongSpellsCastThisTurnIntoHand", [spells @ Filter(_)]) => {
            grouped(
                cards.clone(),
                Operand::Term {
                    family: "GroupFilter".into(),
                    form: "OnePerCardTypeAmong".into(),
                    operands: vec![spells.clone()],
                },
                D::OwnersHand,
            )
        }
        ("PutAGenericCardAndAllCardsWithTheSameNameIntoHand", []) => {
            grouped(cards.clone(), per_type("OneAndAllSameName"), D::OwnersHand)
        }
        ("ExileTheRemainingCardsWithACounterOfType", [kind]) => {
            put(actor, cards.clone(), None, D::Exile, &[kind.clone()])
        }
        ("ExileAnyNumberOfGenericCardsInAFaceDownPile", []) => put(
            actor,
            cards.clone(),
            Some(Count::AnyNumber),
            D::Exile,
            &[flag("FaceDownPile")],
        ),
        ("ExileTheRemainingCardsInAFaceUpPile", []) => {
            put(actor, cards.clone(), None, D::Exile, &[flag("FaceUpPile")])
        }
        ("ExileNumberGenericCardsAtRandom", [Number(amount)]) => Operation::DraftChangeZone {
            actor: actor.clone(),
            cards: cards.clone(),
            count: Some(Count::Exactly(amount.clone())),
            random: true,
            group: None,
            destination: D::Exile,
            entry: vec![],
            face_down: None,
        },
        ("PlayerLooksAtRemainingCardsAndPutsAGenericCardIntoHand", [Player(who)]) => put(
            &PlayerSet::One(who.clone()),
            cards.clone(),
            Some(exactly_one()),
            D::OwnersHand,
            &[],
        ),
        ("SeparateIntoFaceUpFileAndFaceDownPile", []) => piles(&["face up", "face down"]),
        ("SeparateIntoTwoFaceDownPiles", []) => piles(&["face down", "face down"]),
        ("APlayerChoosesAPileTopPutIntoHand", [Players(who)]) => pile(who.clone(), D::OwnersHand),
        ("PlayerChoosesPileTopPutIntoHand", [Player(who)]) => {
            pile(PlayerSet::One(who.clone()), D::OwnersHand)
        }
        ("PlayerExilesAPile", [Player(who)]) => pile(PlayerSet::One(who.clone()), D::Exile),
        (
            "APlayerPutsTheRemainingCardsOnTheTopOfLibraryInAnyOrder",
            [Players(PlayerSet::One(who))],
        ) => Operation::ReorderLibrary {
            chooser: who.clone(),
            library_owner: who.clone(),
            objects: cards.clone(),
            end: SequenceEnd::Top,
            order: CardOrder::Chosen,
        },
        ("ChooseACardThatsExactlyEachColorPair", []) => Operation::ChooseObjects {
            player: actor_of(actor),
            objects: cards.clone(),
            count: Count::AnyNumber,
            constraints: vec![SelectionConstraint::NoCommonProperty(
                DistinctProperty::ColorPairs,
            )],
        },
        ("PutFoundCardsAndExiledCardsOntoBattlefield", [Operand::Objects(exiled), entry]) => put(
            actor,
            ObjectSet::Union(vec![cards.clone(), exiled.clone()]),
            None,
            D::OwnersBattlefield,
            &flags(entry),
        ),
        ("EachPlayerLosesLife", [Players(players), Number(amount)]) => Operation::LoseLife {
            players: players.clone(),
            amount: amount.clone(),
        },
        _ => return None,
    })
}

/// What a reflexive trigger waits on, with the action it follows.
fn reduce_reflexive(actor: &PlayerSet, form: &str, operands: &[Operand]) -> Option<Operation> {
    use Operand::{Filter, Number, Object, Player, Recipients as To, Term};
    let then = |action, when, body: &Operand| {
        Some(Operation::WithReflexive {
            action: Box::new(action),
            when,
            body: Box::new(body.clone()),
        })
    };
    let keyword = |action, object: Option<&ObjectRef>, other: Option<&ObjectRef>, amount| {
        crate::keyword_actions::operation(
            actor,
            action,
            object.cloned(),
            other.cloned(),
            amount,
            None,
        )
    };
    let you = actor_of(actor);
    match (form, operands) {
        ("Reflexive_CollectEvidenceAnyX_WhenYouDo", [body]) => then(
            keyword(
                KeywordKind::CollectEvidence,
                None,
                None,
                Some(Expr::AnnouncedX),
            ),
            ReflexiveWhen::YouDo,
            body,
        ),
        (
            "Reflexive_DealDamage_WhenExcessDamageIsDealtThisWay",
            [Term {
                operands: dealer, ..
            }, Number(amount), To(recipients), body],
        ) => {
            let [Object(source)] = dealer.as_slice() else {
                return None;
            };
            then(
                Operation::DealDamage {
                    source: source.clone(),
                    recipients: recipients.clone(),
                    amount: amount.clone(),
                    preventable: true,
                },
                ReflexiveWhen::ExcessDamage(None),
                body,
            )
        }
        (
            "Reflexive_Fight_WhenExcessDamageIsDealtToPermanent",
            [Object(first), Object(second), Object(recipient), body],
        ) => then(
            keyword(KeywordKind::Fight, Some(first), Some(second), None),
            ReflexiveWhen::ExcessDamage(Some(recipient.clone())),
            body,
        ),
        (
            "Reflexive_HavePlayerGainControlOfPermanent_WhenYouDo",
            [Player(player), Object(object), body],
        ) => then(
            Operation::GiveControl {
                players: PlayerSet::One(player.clone()),
                objects: ObjectSet::One(object.clone()),
                until: None,
            },
            ReflexiveWhen::YouDo,
            body,
        ),
        (
            "Reflexive_RevealTheTopNumberCardsOfLibrary_WhenAnyNumberOfCardsOfTypeRevealedThisWay",
            [Number(top), Filter(cards), body],
        ) => then(
            Operation::Reveal {
                objects: ObjectSet::Take {
                    objects: Box::new(zone_set(
                        EventZone::Library,
                        PlayerSet::One(you),
                        ObjectFilter::Any,
                    )),
                    count: top.clone(),
                    from: SequenceEnd::Top,
                },
            },
            ReflexiveWhen::Revealed(cards.clone()),
            body,
        ),
        ("RegeneratePermanent_WhenItRengeratesThisWay", [Object(object), body]) => then(
            Operation::CreateRegenerationShields {
                objects: ObjectSet::One(object.clone()),
            },
            ReflexiveWhen::Regenerates,
            body,
        ),
        _ => None,
    }
}

/// Actions left after the keyword table, each said by what it does.
fn reduce_rest_actions(actor: &PlayerSet, form: &str, operands: &[Operand]) -> Option<Operation> {
    use CardPlayAction as A;
    use Destination as D;
    use EventZone as Z;
    use Operand::{
        Cost as Paid, Duration as Lasting, Filter, List, Number, Object, Objects, Player, Players,
        Recipients as To, Result as Run, Term,
    };
    let free = || PlayTerms {
        cost: PlayCost::Free,
        ..play_terms()
    };
    let may_one = || Count::UpTo(Expr::Constant(1));
    let one = || Count::Exactly(Expr::Constant(1));
    let cast = |who: &PlayerSet, cards, count, group: Option<&Operand>, terms: PlayTerms| {
        cast_with(who, A::CastSpell, cards, count, terms, group)
    };
    let choose = |objects, count| Operation::ChooseObjects {
        player: actor_of(actor),
        objects,
        count,
        constraints: vec![],
    };
    let hand_of = |players: &PlayerSet, cards: &ObjectFilter| {
        zone_set(Z::Hand, players.clone(), cards.clone())
    };
    let grave_of = |players: &PlayerSet, cards: &ObjectFilter| {
        zone_set(Z::Graveyard, players.clone(), cards.clone())
    };
    let both = |first: &ObjectFilter, second: &ObjectFilter| {
        ObjectFilter::All(vec![first.clone(), second.clone()])
    };
    let copies = |objects, times| Operation::CopyCards {
        actor: actor.clone(),
        objects,
        times,
    };
    let pile_form = |from, names, then| Operation::ChoosePile {
        actor: actor.clone(),
        from,
        random: false,
        names,
        object: None,
        then,
    };
    let revealed = || ObjectSet::Earlier {
        what: EarlierObjects::Revealed,
        by: None,
    };
    let pumped =
        |object: &ObjectRef, power: Expr, toughness: Expr| Operation::CreateContinuousEffect {
            source: ObjectRef::Source,
            affected: EffectSubjects::Objects(ObjectSet::One(object.clone())),
            changes: vec![Change::Pump { power, toughness }],
            duration: Duration::EndOfTurn,
            evaluation: EffectEvaluation::Snapshot,
        };
    let stat = |object: &ObjectRef, property| Expr::ObjectProperty {
        object: object.clone(),
        property,
        information: Information::CurrentOrLastKnown,
    };
    Some(match (form, operands) {
        (
            "CastASpellFromHandGraveyardOrExileWithoutPaying",
            [Filter(spells), Filter(hand), Filter(grave), Objects(exile)],
        ) => cast(
            actor,
            ObjectSet::Union(vec![
                hand_of(actor, &both(spells, hand)),
                grave_of(actor, &both(spells, grave)),
                matching(exile, spells),
            ]),
            may_one(),
            None,
            free(),
        ),
        ("CastASpellFromRevealedCardInHandsWithoutPaying", [Filter(cards), Filter(spells)]) => {
            cast(
                actor,
                matching(&revealed(), &both(cards, spells)),
                may_one(),
                None,
                free(),
            )
        }
        (
            "CastUptoNumberGroupSpellsFromGraveyardOrHandWithoutPayingIntoExile",
            [Number(amount), Filter(spells), group],
        ) => cast(
            actor,
            ObjectSet::Union(vec![grave_of(actor, spells), hand_of(actor, spells)]),
            Count::UpTo(amount.clone()),
            Some(group),
            PlayTerms {
                into_exile: true,
                ..free()
            },
        ),
        (
            "MayCastASpellFromAmongCardsInHandsWithoutPaying",
            [Filter(spells), Term { operands, .. }],
        ) => {
            let [Players(players)] = operands.as_slice() else {
                return None;
            };
            cast(actor, hand_of(players, spells), may_one(), None, free())
        }
        (
            "ForEachPlayerMayCastASpellFromAmongCardsThatPlayerMilledThisWayWithoutPaying",
            [Players(players), Filter(spells), Run(result)],
        ) => cast(
            // "For each player, you may cast a card that player milled this way"
            // (The Ur-Sphinx): you cast, once for each player's milled cards.
            actor,
            matching(&ObjectSet::Recorded(result.clone()), spells),
            Count::PerPlayer {
                players: players.clone(),
                each: Box::new(may_one()),
            },
            None,
            free(),
        ),
        ("PlayACardFromOutsideGame", []) => cast_with(
            actor,
            A::Either,
            ObjectSet::OutsideTheGame(ObjectFilter::Any),
            one(),
            play_terms(),
            None,
        ),
        ("ChooseACardFromOutsideGame", [Objects(cards)]) => choose(cards.clone(), one()),
        ("ChooseACardInHandOrAPermanent", [Filter(cards), Objects(set)]) => choose(
            ObjectSet::Union(vec![hand_of(actor, cards), set.clone()]),
            one(),
        ),
        ("ChooseASecondPermanentAtRandom", [Objects(set)]) => Operation::ChooseObjects {
            player: actor_of(actor),
            objects: set.clone(),
            count: one(),
            constraints: vec![SelectionConstraint::AtRandom],
        },
        ("ChooseAPermanentPile" | "ChooseAnExiledPile", []) => pile_form(None, vec![], None),
        ("DestroyEachPermanent", [Term { form, .. }]) if form == "InTheChosenPile" => {
            Operation::Destroy {
                objects: ObjectSet::battlefield(ObjectFilter::InPile(None)),
                regeneration: true,
            }
        }
        ("TapEachPermanent", [Term { form, .. }]) if form == "NotInTheChosenPile" => {
            Operation::SetTapped {
                objects: ObjectSet::battlefield(ObjectFilter::Not(Box::new(ObjectFilter::InPile(
                    None,
                )))),
                tapped: true,
            }
        }
        ("ChooseActionAtRandom", options) if options.iter().all(|o| matches!(o, List(_))) => {
            Operation::ChooseAtRandom {
                kind: ValueKind::Action,
                options: options.to_vec(),
            }
        }
        ("AnyPlayerMayPayMana", [Players(players), List(mana), List(unpaid), List(paid)]) => {
            Operation::AnyPlayerMayPay {
                players: players.clone(),
                mana: mana.clone(),
                unpaid: unpaid.clone(),
                paid: paid.clone(),
            }
        }
        ("EachPlayerRepeatedMayCost", [Players(players), Number(at_most), Paid(cost)]) => {
            Operation::PlayersPayRepeatedly {
                players: players.clone(),
                cost: cost.clone(),
                at_most: at_most.clone(),
            }
        }
        ("APlayerGainsControlOfPermanent", [Players(players), Object(object)]) => {
            Operation::GiveControl {
                players: players.clone(),
                objects: ObjectSet::One(object.clone()),
                until: None,
            }
        }
        ("CopyCard", [Operand::Vocabulary { form, .. }]) if form == "TheCardWithTheChosenName" => {
            copies(
                ObjectSet::OutsideTheGame(ObjectFilter::Chosen(ValueKind::CardName)),
                Expr::Constant(1),
            )
        }
        ("CopyCardWithTheNotedName", []) => copies(
            ObjectSet::OutsideTheGame(ObjectFilter::Noted(ValueKind::CardName)),
            Expr::Constant(1),
        ),
        ("CopyEnchantedGraveyardCard", []) => copies(
            ObjectSet::One(ObjectRef::AttachedTo(Box::new(ObjectRef::Source))),
            Expr::Constant(1),
        ),
        ("CopyNumberCardsAtRandom", [Number(amount), Filter(cards)]) => copies(
            ObjectSet::DraftRandom {
                objects: Box::new(ObjectSet::OutsideTheGame(cards.clone())),
                count: amount.clone(),
            },
            Expr::Constant(1),
        ),
        ("CopyEachAbilityAndMayChooseNewTargets", [abilities]) => Operation::CopyEach {
            actor: actor.clone(),
            objects: stack_filter(abilities)?,
            targets: NewTargets::May,
        },
        ("CounterEachSpellAndAbility", [Objects(set)]) => Operation::Counter {
            objects: set.clone(),
            destination: D::OwnersGraveyard,
        },
        ("DiscardTheCardRevealedThisWay", []) => Operation::DiscardObjects {
            objects: revealed(),
        },
        ("CreateFutureTrigger_UnlessPlayerPaysManaBefore", [event, body, Player(payer), mana]) => {
            Operation::DelayedUnlessPaid {
                event: Box::new(event.clone()),
                body: Box::new(body.clone()),
                payer: payer.clone(),
                mana: Box::new(mana.clone()),
            }
        }
        (
            "MultiDraw",
            [Term {
                operands: first, ..
            }, Term {
                operands: second, ..
            }],
        ) => {
            let ([Number(amount)], [Players(players)]) = (first.as_slice(), second.as_slice())
            else {
                return None;
            };
            Operation::Together(vec![
                Operation::Draw {
                    players: actor.clone(),
                    amount: amount.clone(),
                },
                Operation::Draw {
                    players: players.clone(),
                    amount: Expr::Constant(1),
                },
            ])
        }
        ("MultiFight", [Objects(fighters), Objects(others)]) => Operation::FightEach {
            fighters: fighters.clone(),
            others: others.clone(),
        },
        ("NumberEachPermanentStartingFromZero", [Objects(set)]) => {
            Operation::Note(NoteKind::Numbered(set.clone()))
        }
        (
            "PutUptoOneCardOfEachCardtypeAmongPermanentsFromHandOnTheBattlefield",
            [Objects(set), List(entry)],
        ) => Operation::DraftChangeZone {
            actor: actor.clone(),
            cards: hand_of(actor, &ObjectFilter::Any),
            count: Some(Count::AnyNumber),
            random: false,
            group: Some(Box::new(Operand::Term {
                family: "GroupFilter".into(),
                form: "OnePerCardTypeAmong".into(),
                operands: vec![Objects(set.clone())],
            })),
            destination: D::OwnersBattlefield,
            entry: entry.clone(),
            face_down: None,
        },
        (
            "PermanentDealsDamageAndPermanentDealsDamage",
            [Object(first), Number(first_amount), To(first_to), Object(second), Number(second_amount), To(second_to)],
        ) => Operation::Together(vec![
            Operation::DealDamage {
                source: first.clone(),
                recipients: first_to.clone(),
                amount: first_amount.clone(),
                preventable: true,
            },
            Operation::DealDamage {
                source: second.clone(),
                recipients: second_to.clone(),
                amount: second_amount.clone(),
                preventable: true,
            },
        ]),
        ("GraveyardCardDealsMultipleDamage", [Object(source), List(parts)]) => {
            Operation::DealDamageParts {
                source: source.clone(),
                parts: parts
                    .iter()
                    .map(|part| match part {
                        Term { operands, .. } => match operands.as_slice() {
                            [Number(amount), To(recipients)] => Some(DamagePart {
                                recipients: recipients.clone(),
                                amount: amount.clone(),
                            }),
                            _ => None,
                        },
                        _ => None,
                    })
                    .collect::<Option<_>>()?,
                preventable: true,
            }
        }
        ("TripleCreaturesPowerAndToughnessUntilEndOfTurn", [Object(object)]) => pumped(
            object,
            Expr::Multiply(
                Box::new(Expr::Constant(2)),
                Box::new(stat(object, Property::Power)),
            ),
            Expr::Multiply(
                Box::new(Expr::Constant(2)),
                Box::new(stat(object, Property::Toughness)),
            ),
        ),
        ("DoubleCreaturesPowerNumberTimesUntilEndOfTurn", [Object(object), Number(times)]) => {
            pumped(
                object,
                Expr::Multiply(
                    Box::new(stat(object, Property::Power)),
                    Box::new(Expr::Subtract(
                        Box::new(Expr::Exponentiate(
                            Box::new(Expr::Constant(2)),
                            Box::new(times.clone()),
                        )),
                        Box::new(Expr::Constant(1)),
                    )),
                ),
                Expr::Constant(0),
            )
        }
        ("EachPlayerCantCastSpellsDuringTheirNextTurn", [Players(players), Filter(spells)]) => {
            Operation::EffectOn {
                subject: EffectSubject::Players(players.clone()),
                effects: vec![
                    Filter(spells.clone()),
                    Operand::Vocabulary {
                        family: "PlayerEffect".into(),
                        form: "CantCastSpells".into(),
                    },
                ],
                until: Some(Duration::ThroughNextTurn),
            }
        }
        ("PhaseOutPermanentUntilWithEffects", [Object(object), Lasting(until), effect]) => {
            Operation::EffectOn {
                subject: EffectSubject::Objects(ObjectSet::One(object.clone())),
                effects: vec![effect.clone()],
                until: Some(until.clone()),
            }
        }
        _ => return None,
    })
}

fn perpetual_changes(effects: &[Operand]) -> Option<Vec<Change>> {
    let mut changes = Vec::new();
    for effect in effects {
        let Operand::Term { form, operands, .. } = effect else {
            return None;
        };
        let reduced = match (form.as_str(), operands.as_slice()) {
            ("AdjustPT", [Operand::Integer(power), Operand::Integer(toughness)]) => {
                vec![Change::Pump {
                    power: Expr::Constant(*power),
                    toughness: Expr::Constant(*toughness),
                }]
            }
            ("AddAbility", [Operand::Ability(ability)]) => {
                vec![Change::GrantAbility(ability.clone())]
            }
            ("AdjustPTX", [power, toughness, Operand::Number(x)]) => vec![Change::Pump {
                power: mod_x(power, x)?,
                toughness: mod_x(toughness, x)?,
            }],
            ("SetPT", [pt]) => vec![set_pt(pt)?],
            _ => reduce_changes(format!("LayerEffect::{form}"), operands.clone()),
        };
        if reduced
            .iter()
            .any(|change| matches!(change, Change::Primitive { .. }))
        {
            return None;
        }
        changes.extend(reduced);
    }
    Some(changes)
}

/// Perpetual changes to cards the row picked out by a term.
fn reduce_perpetual(form: &str, operands: &[Operand]) -> Option<Operation> {
    use EventZone as Z;
    use Operand::{Filter, List, Object, Objects, Player, Players, Term, Vocabulary};
    let picked = || ObjectSet::One(earlier_object(EarlierObjects::Selected));
    let found = || ObjectSet::One(earlier_object(EarlierObjects::Found));
    let (objects, effects) = match (form, operands) {
        ("CreatePerpetualCardInHandEffect", [Object(card), List(effects)]) => {
            (ObjectSet::One(card.clone()), effects)
        }
        ("CreatePerpetualCardInHandEffect", [Vocabulary { .. }, List(effects)]) => {
            (picked(), effects)
        }
        ("CreatePerpetualCardsInHandEffect", [Term { .. }, List(effects)]) => (picked(), effects),
        ("CreatePerpetualCardInLibraryEffect", [Object(card), List(effects)]) => {
            (ObjectSet::One(card.clone()), effects)
        }
        ("CreatePerpetualCardInLibraryEffect", [Vocabulary { .. }, List(effects)]) => {
            (found(), effects)
        }
        ("CreatePerpetualCardInLibraryEffect", [Term { form, operands, .. }, List(effects)]) => {
            let [Filter(cards), Player(player)] = operands.as_slice() else {
                return None;
            };
            let library = zone_set(Z::Library, PlayerSet::One(player.clone()), cards.clone());
            let objects = match form.as_str() {
                "TheTopCardOfTypeInPlayersLibrary" => ObjectSet::Take {
                    objects: Box::new(library),
                    count: Expr::Constant(1),
                    from: SequenceEnd::Top,
                },
                "ARandomCardOfTypeFromPlayersLibrary" => ObjectSet::DraftRandom {
                    objects: Box::new(library),
                    count: Expr::Constant(1),
                },
                _ => return None,
            };
            (objects, effects)
        }
        (
            "CreatePerpetualCardsInEachPlayersHandEffect",
            [Objects(set), Players(players), List(effects)],
        ) => (
            zone_set(Z::Hand, players.clone(), objects_filter(set)),
            effects,
        ),
        (
            "CreatePerpetualCardsInPlayersHandAndCardsInPlayersLibraryEffect",
            [hand, Players(holders), Filter(library), Players(owners), List(effects)],
        ) => (
            ObjectSet::Union(vec![
                zone_set(Z::Hand, holders.clone(), card_term(hand)?),
                zone_set(Z::Library, owners.clone(), library.clone()),
            ]),
            effects,
        ),
        _ => return None,
    };
    Some(Operation::CreatePerpetualEffect {
        objects,
        changes: perpetual_changes(effects)?,
    })
}
