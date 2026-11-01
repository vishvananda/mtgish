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
