// Numbers, conditions and triggers that read what happened, as Forge's
// `Remembered$` counts, `ValidCause` and `Mode$ Always` do.

fn earlier(what: EarlierObjects, measure: ObjectMeasure) -> Expr {
    Expr::Earlier {
        what,
        measure,
        result: None,
        filter: None,
        by: None,
    }
}

/// The objects a form's words name, and what it measures of them.
fn earlier_form(form: &str) -> Option<(EarlierObjects, ObjectMeasure)> {
    use EarlierObjects as E;
    use ObjectMeasure::{Count, Greatest, Of, Total};
    use Property::{ManaValue, Power, Toughness};
    Some(match form {
        "ItsManaValue" => (E::It, Of(ManaValue)),
        "PowerOfTheRevealedCard" => (E::Revealed, Of(Power)),
        "ToughnessOfTheRevealedCard" => (E::Revealed, Of(Toughness)),
        "ManaValueOfTheCardRevealedThisWay" => (E::Revealed, Of(ManaValue)),
        "TotalManaValueOfTheCardsRevealedThisWay" => (E::Revealed, Total(ManaValue)),
        "ToughnessOfTheExiledCreature" => (E::Exiled, Of(Toughness)),
        "TotalManaValueOfMilledCards" => (E::Milled, Total(ManaValue)),
        "ManaValueOfTheFoundCard" | "ManaValueOfTheCardFoundThisWay" => (E::Found, Of(ManaValue)),
        "PowerOfTheDevouredCreature" => (E::Devoured, Of(Power)),
        "ManaValueOfCardPutInGraveyard" => (E::PutInGraveyard, Of(ManaValue)),
        "Emerge_ToughnessOfTheSacrificedCreature" => (E::Sacrificed, Of(Toughness)),
        "TheGreatestManaValueAmongTheCardsThatLeftTheGraveyardThisWay" => {
            (E::LeftGraveyard, Greatest(ManaValue))
        }
        "ManaValueOfThePermanentUnattachedThisWay" => (E::Unattached, Of(ManaValue)),
        "PowerOfTheSelectedPermanent" => (E::Selected, Of(Power)),
        "CounterSpell_ManaValueOfCounteredSpell" => (E::Countered, Of(ManaValue)),
        "ManaValueOfTheDiscardedCard" => (E::Discarded, Of(ManaValue)),
        "ManaValueOfTheWebslungCreature" => (E::Webslung, Of(ManaValue)),
        "TheNumberOfCardsManifestedThisWay" => (E::Manifested, Count),
        "TheTotalManaValueOfThePermanentsThisSpellTargets" => (E::Targeted, Total(ManaValue)),
        "TheNumberOfCardsPutIntoAGraveyardThisWay" => (E::PutInGraveyard, Count),
        "TheNumberOfPermanentCardsReturnedToPlayersHandThisWay" => (E::Returned, Count),
        "NumberOfPermanentsTappedThisWayByPlayer" => (E::Tapped, Count),
        "TheNumberOfCardsPlayerShuffledIntoLibraryThisWay" => (E::Shuffled, Count),
        "TheNumberOfCardsInHandRevealedByPlayerThisWay" => (E::Revealed, Count),
        _ => return None,
    })
}

fn sticker_measure(form: &str, rest: &[Operand]) -> Option<StickerMeasure> {
    use StickerMeasure as M;
    Some(match (form, rest) {
        ("TheNumberOfNameStickersOnPermanent", []) => M::Names,
        ("TheNumberOfNameStickersOnPermanentWithLength", [Operand::Comparison(c, n)]) => {
            M::NamesOfLength(c.clone(), Box::new(n.clone()))
        }
        ("TheNumberOfNameStickersOnPermanentThatBeginWithLetter", [letter]) => {
            M::NamesBeginningWith(Box::new(letter.clone()))
        }
        ("TheTotalPowerOfAllStickersOnPermanents", []) => M::TotalPower,
        ("TheTotalToughnessOfAllStickersOnPermanents", []) => M::TotalToughness,
        _ => return None,
    })
}

fn reduce_event_number(form: &str, operands: &[Operand]) -> Option<Expr> {
    use GameValue as G;
    use Operand::{Comparison as Compare, Filter, Object, Objects, Player, Result as Run};
    let dice = |measure| Some(Expr::Dice(measure));
    let value = |value| Some(Expr::Game(value));
    let noted = |object: &ObjectRef, measure| {
        Some(Expr::Noted {
            object: Some(object.clone()),
            measure,
        })
    };
    if let Some((what, measure)) = earlier_form(form) {
        let mut result = None;
        let mut filter = None;
        let mut by = None;
        for operand in operands {
            match operand {
                Run(run) => result = Some(run.clone()),
                Filter(cards) => filter = Some(Box::new(cards.clone())),
                Player(player) => by = Some(player.clone()),
                _ => return None,
            }
        }
        return Some(Expr::Earlier {
            what,
            measure,
            result,
            filter,
            by,
        });
    }
    match (form, operands) {
        ("TheNumberOfDiceResults", [Compare(comparison, value)]) => dice(DiceMeasure::Matching(
            comparison.clone(),
            Box::new(value.clone()),
        )),
        ("TheTotalOfTheDiceResults", []) => dice(DiceMeasure::Total),
        ("TheDiceResult" | "TriggerTheDiceResult", []) => dice(DiceMeasure::Result),
        ("TheFirstDiceResult", []) => dice(DiceMeasure::Nth(1)),
        ("TheSecondDiceResult", []) => dice(DiceMeasure::Nth(2)),
        ("TheNumberOfDifferentDiceResults", []) => dice(DiceMeasure::Different),
        ("TheGreatestNumberOfStoredDiceResultsThatAreEqual", []) => dice(DiceMeasure::MostEqual),
        ("NumCoinFlipsWon" | "FlipACoinUntilLose_NumFlipsWon", []) => {
            Some(Expr::CountCoinOutcomes {
                source: CoinCountSource::Latest,
                outcome: CoinOutcome::Won,
            })
        }
        ("NumCoinFlipsLost", []) => Some(Expr::CountCoinOutcomes {
            source: CoinCountSource::Latest,
            outcome: CoinOutcome::Lost,
        }),
        ("CurrentStake", []) => value(G::Stake),
        ("TheWinningBid", []) => value(G::WinningBid),
        ("TheLifePaid" | "WhenAPlayerPaysLife_AmountOfLifePaid", []) => value(G::LifePaid),
        ("LifeLostThisWay", _) => value(G::LifeLostThisWay),
        ("AmountOfManaPaidThisWay", _) => value(G::ManaPaidThisWay),
        ("TheNumberOfPlayersThatHaveLostTheGame", []) => value(G::PlayersWhoLost),
        ("NumberOfTimesThisAbilityHasResolvedThisTurn", []) => value(G::ResolutionsThisTurn),
        ("TheNotedNumber", []) => Some(Expr::Noted {
            object: None,
            measure: NotedMeasure::Number,
        }),
        ("HighestNotedValueForPermanent", [Object(o)]) => noted(o, NotedMeasure::Highest),
        ("LastNotedLifeTotalForPermanent", [Object(o)]) => noted(o, NotedMeasure::LifeTotal),
        ("TheNumberChosenForPermanent", [Object(o)]) => noted(o, NotedMeasure::Number),
        ("NumberOfCreatureTypesNotedByPermanent", [Object(o)]) => {
            noted(o, NotedMeasure::CreatureTypes)
        }
        ("TheNumberOfUnlockedDoorsAmongPermanents", [Objects(set)]) => {
            Some(Expr::CountObjects(Box::new(matching(
                set,
                &ObjectFilter::State(ObjectState::UnlockedDoor),
            ))))
        }
        (
            "TheTotalPowerOfAllStickersOnPermanents" | "TheTotalToughnessOfAllStickersOnPermanents",
            [Objects(set)],
        ) => Some(Expr::Stickers {
            objects: Box::new(set.clone()),
            measure: sticker_measure(form, &[])?,
        }),
        (_, [Object(object), rest @ ..]) if form.contains("Sticker") => Some(Expr::Stickers {
            objects: Box::new(ObjectSet::One(object.clone())),
            measure: sticker_measure(form, rest)?,
        }),
        _ => None,
    }
}

fn some_of(left: Expr) -> Condition {
    Condition::Compare {
        left,
        comparison: Comparison::GreaterOrEqual,
        right: Expr::Constant(1),
    }
}

fn none_of(left: Expr) -> Condition {
    Condition::Compare {
        left,
        comparison: Comparison::Equal,
        right: Expr::Constant(0),
    }
}

/// `Earlier` with its optional parts read from what follows a filter.
fn earlier_among(what: EarlierObjects, rest: &[Operand]) -> Option<Expr> {
    let mut result = None;
    let mut filter = None;
    let mut by = None;
    for operand in rest {
        match operand {
            Operand::Result(run) => result = Some(run.clone()),
            Operand::Filter(cards) => filter = Some(Box::new(cards.clone())),
            Operand::Objects(set) => filter = Some(Box::new(objects_filter(set))),
            Operand::Player(player) => by = Some(player.clone()),
            _ => return None,
        }
    }
    Some(Expr::Earlier {
        what,
        measure: ObjectMeasure::Count,
        result,
        filter,
        by,
    })
}

fn reduce_event_condition(form: &str, operands: &[Operand]) -> Option<Condition> {
    use EarlierObjects as E;
    use Operand::{Comparison as Compare, Object, Objects, Player, Text};
    use PriorOutcome as O;
    let compare = |left, comparison: &Comparison, right: &Expr| Condition::Compare {
        left,
        comparison: comparison.clone(),
        right: right.clone(),
    };
    let happened = |what| some_of(earlier(what, ObjectMeasure::Count));
    let outcome = |outcome| Some(Condition::Outcome(outcome));
    match (form, operands) {
        ("EvidenceWasCollectedThisWay", _) => Some(happened(E::Evidence)),
        ("ACardWasSurveiledIntoGraveyardThisWay", rest) => {
            Some(some_of(earlier_among(E::PutInGraveyard, rest)?))
        }
        ("ACardWasExiledThisWay", rest) => Some(some_of(earlier_among(E::Exiled, rest)?)),
        ("APermanentWasPutIntoHandThisWay", rest) => {
            Some(some_of(earlier_among(E::Returned, rest)?))
        }
        ("APermanentWasPutOntoTheBattlefieldByPlayerThisWay", rest) => {
            Some(some_of(earlier_among(E::Entered, rest)?))
        }
        ("ACreatureWasExploitedThisWay", rest) => {
            Some(some_of(earlier_among(E::Sacrificed, rest)?))
        }
        ("GuestWasUnattachedFromAPermanentThisWay", rest) => {
            Some(some_of(earlier_among(E::Unattached, rest)?))
        }
        ("NumCoinFlipsWonIs", [Compare(comparison, value)]) => Some(compare(
            Expr::CountCoinOutcomes {
                source: CoinCountSource::Latest,
                outcome: CoinOutcome::Won,
            },
            comparison,
            value,
        )),
        ("NumCoinFlipsLostIs", [Compare(comparison, value)]) => Some(compare(
            Expr::CountCoinOutcomes {
                source: CoinCountSource::Latest,
                outcome: CoinOutcome::Lost,
            },
            comparison,
            value,
        )),
        ("YouWonTheCoinFlip", []) => Some(some_of(Expr::CountCoinOutcomes {
            source: CoinCountSource::Latest,
            outcome: CoinOutcome::Won,
        })),
        ("YouLostTheCoinFlip", []) => Some(some_of(Expr::CountCoinOutcomes {
            source: CoinCountSource::Latest,
            outcome: CoinOutcome::Lost,
        })),
        (
            "ADiceResultIs" | "WhenAPlayerRollsAnyNumberOfDice_AnyDiceResultIs",
            [Compare(c, value)],
        ) => Some(some_of(Expr::Dice(DiceMeasure::Matching(
            c.clone(),
            Box::new(value.clone()),
        )))),
        ("NumberDiceAreEqual", [Compare(comparison, value)]) => Some(compare(
            Expr::Dice(DiceMeasure::MostEqual),
            comparison,
            value,
        )),
        ("ANumberDiceValuesAre", [Compare(count, n), Compare(c, value)]) => Some(compare(
            Expr::Dice(DiceMeasure::Matching(c.clone(), Box::new(value.clone()))),
            count,
            n,
        )),
        ("CostWasPaid", []) => outcome(O::CostPaid),
        ("FreerunningCostWasPaid", []) => outcome(O::FreerunningPaid),
        ("MayhemCostWasPaid", []) => outcome(O::MayhemPaid),
        ("NoLifeWasPaidThisWay", _) => outcome(O::NoLifePaid),
        ("NoVotesWereCastThisWay", _) => outcome(O::NoVotes),
        ("ASpellWasntCastThisWay", _) => outcome(O::NoSpellCast),
        ("Trigger_WonTheClash", []) => outcome(O::WonClash),
        ("PlayerGuessedWrong", []) => outcome(O::GuessedWrong),
        ("RevealedCardsWerePutInHand", []) => outcome(O::RevealedPutInHand),
        ("ExiledPermanentWasUnearthed", []) => outcome(O::Unearthed),
        ("ItWasAnArtSticker", []) => outcome(O::ArtSticker),
        ("AllVotesWereForWord", [Text(word)]) => {
            Some(Condition::AllVotedFor(VoteOption::Word(word.clone())))
        }
        ("AColorIsTheMostCommonOrTiedForMostCommonColorAmongPermanents", [color, Objects(set)]) => {
            Some(Condition::MostCommonColor {
                color: Box::new(color.clone()),
                objects: set.clone(),
                tied: true,
            })
        }
        (
            "AColorIsTheMostCommonColorAmongPermanentsButNotTiedForTheMostComon",
            [color, Objects(set)],
        ) => Some(Condition::MostCommonColor {
            color: Box::new(color.clone()),
            objects: set.clone(),
            tied: false,
        }),
        (
            "CardIsInPlayersGraveyardWithANumberOfCardsAboveIt",
            [Object(card), Player(player), Compare(comparison, n), Operand::Filter(cards)],
        ) => Some(Condition::GraveyardPosition {
            card: card.clone(),
            player: player.clone(),
            above: cards.clone(),
            comparison: comparison.clone(),
            amount: n.clone(),
        }),
        (
            "CardIsInPlayersGraveyardWithACardAboveIt",
            [Object(card), Player(player), Operand::Filter(cards)],
        ) => Some(Condition::GraveyardPosition {
            card: card.clone(),
            player: player.clone(),
            above: cards.clone(),
            comparison: Comparison::GreaterOrEqual,
            amount: Expr::Constant(1),
        }),
        ("TriggerAPermanentOrPlayerWasDealtAnAmountOfDamage", [Compare(comparison, n)]) => {
            Some(compare(Expr::EventAmount, comparison, n))
        }
        ("ThereAreNumberUnlockedDoorsAmongPermanents", [Compare(comparison, n), Objects(set)]) => {
            Some(compare(
                Expr::CountObjects(Box::new(matching(
                    set,
                    &ObjectFilter::State(ObjectState::UnlockedDoor),
                ))),
                comparison,
                n,
            ))
        }
        ("DifferenceIs", [Operand::Number(a), Operand::Number(b), Compare(comparison, n)]) => {
            Some(compare(
                Expr::Difference(Box::new(a.clone()), Box::new(b.clone())),
                comparison,
                n,
            ))
        }
        ("ANumberOfPlayersHaveLostTheGame", [Compare(comparison, n)]) => Some(compare(
            Expr::Game(GameValue::PlayersWhoLost),
            comparison,
            n,
        )),
        _ => None,
    }
}

fn set_filter(operand: &Operand) -> Option<ObjectFilter> {
    match operand {
        Operand::Objects(set) => Some(objects_filter(set)),
        Operand::Filter(filter) => Some(filter.clone()),
        Operand::Object(object) => Some(ObjectFilter::SameObject(object.clone())),
        _ => None,
    }
}

fn zone_change(objects: ObjectFilter, from: Option<EventZone>, to: EventZone) -> EventSpec {
    EventSpec::ZoneChange { objects, from, to }
}

fn caused(cause: ObjectFilter, event: EventSpec) -> EventSpec {
    EventSpec::Caused {
        cause,
        event: Box::new(event),
    }
}

fn whole(event: EventSpec) -> EventSpec {
    EventSpec::Batch {
        event: Box::new(event),
        per: BatchPer::Whole,
        count: None,
        including: None,
    }
}

fn nth(event: EventSpec, comparison: &Comparison, value: &Expr) -> EventSpec {
    EventSpec::NthEachTurn {
        event: Box::new(event),
        comparison: comparison.clone(),
        value: value.clone(),
    }
}

fn owned_among(objects: ObjectFilter, players: &PlayerSet) -> ObjectFilter {
    ObjectFilter::All(vec![objects, ObjectFilter::OwnedBy(players.clone())])
}

fn controlled_among(objects: ObjectFilter, players: &PlayerSet) -> ObjectFilter {
    ObjectFilter::All(vec![objects, ObjectFilter::ControlledBy(players.clone())])
}

/// A state trigger (603.8) in terms of a condition, as Forge's `Mode$ Always`.
fn state_event(form: &str, operands: &[Operand]) -> Option<EventSpec> {
    use Operand::{Comparison as Compare, Objects, Players};
    let state = |condition| Some(EventSpec::State { condition });
    let player = || PlayerRef::Bound(LoopBinding(0));
    let any_player = |players: &PlayerSet, condition| {
        state(Condition::ExistsPlayer {
            players: players.clone(),
            binding: LoopBinding(0),
            condition: Box::new(condition),
        })
    };
    let count = |objects: ObjectSet| Expr::CountObjects(Box::new(objects));
    match (form, operands) {
        ("WhenAPlayerHasAnAmountOfLife", [Players(players), Compare(comparison, n)]) => any_player(
            players,
            Condition::Compare {
                left: Expr::player_life(player()),
                comparison: comparison.clone(),
                right: n.clone(),
            },
        ),
        ("WhenAPlayerHasNumberCardsInHand", [Players(players), Compare(comparison, n)]) => {
            any_player(
                players,
                Condition::Compare {
                    left: count(zone_set(
                        EventZone::Hand,
                        PlayerSet::One(player()),
                        ObjectFilter::Any,
                    )),
                    comparison: comparison.clone(),
                    right: n.clone(),
                },
            )
        }
        ("WhenAPlayerHasNoCardsInTheirGraveyard", [Players(players)]) => any_player(
            players,
            none_of(count(zone_set(
                EventZone::Graveyard,
                PlayerSet::One(player()),
                ObjectFilter::Any,
            ))),
        ),
        (
            "WhenAPlayerControlsNumberPermanents",
            [Players(players), Objects(set), Compare(c, n)],
        ) => any_player(
            players,
            Condition::Compare {
                left: count(matching(
                    set,
                    &ObjectFilter::ControlledBy(PlayerSet::One(player())),
                )),
                comparison: c.clone(),
                right: n.clone(),
            },
        ),
        ("WhenPlayersControlsNoPermanents", [Players(players), Objects(set)]) => any_player(
            players,
            none_of(count(matching(
                set,
                &ObjectFilter::ControlledBy(PlayerSet::One(player())),
            ))),
        ),
        (
            "WhenAColorIsntTheMostCommonOrTiedForMostCommonColorAmongPermanents",
            [color, Objects(set)],
        ) => state(Condition::Not(Box::new(Condition::MostCommonColor {
            color: Box::new(color.clone()),
            objects: set.clone(),
            tied: true,
        }))),
        _ => None,
    }
}

/// Triggers by what caused them, as Forge's `ValidCause`, and batches of events.
fn reduce_event_form(form: &str, operands: &[Operand]) -> Option<EventSpec> {
    use EventZone as Z;
    use Operand::{Comparison as Compare, Filter, Number, Objects, Players};
    let player = |event, players: &PlayerSet, objects| EventSpec::Player {
        event,
        players: players.clone(),
        objects,
    };
    if let Some(event) = state_event(form, operands) {
        return Some(event);
    }
    Some(match (form, operands) {
        ("WhenASpellOrAbilityCountersASpell", [cause, Filter(spells)]) => caused(
            stack_filter(cause)?,
            EventSpec::Object {
                event: ObjectEvent::Countered,
                objects: spells.clone(),
            },
        ),
        ("WhenASpellIsCountered", [Filter(spells)]) => EventSpec::Object {
            event: ObjectEvent::Countered,
            objects: spells.clone(),
        },
        ("WhenASpellOrAbilityExilesAnyNumberOfPermanents", [cause, Objects(set)]) => caused(
            stack_filter(cause)?,
            whole(zone_change(
                objects_filter(set),
                Some(Z::Battlefield),
                Z::Exile,
            )),
        ),
        ("WhenASpellOrAbilityDestroysAPermanent", [cause, Objects(set)]) => caused(
            stack_filter(cause)?,
            EventSpec::Object {
                event: ObjectEvent::Destroyed,
                objects: objects_filter(set),
            },
        ),
        ("WhenASpellOrAbilityCausesAPlayerToShuffleTheirLibrary", [cause, Players(players)]) => {
            caused(
                stack_filter(cause)?,
                player(PlayerEvent::Shuffles, players, None),
            )
        }
        ("WhenASpellOrAbilityCausesItsControllerToShuffleTheirLibrary", [cause]) => caused(
            stack_filter(cause)?,
            player(PlayerEvent::Shuffles, &PlayerSet::All, None),
        ),
        (
            "WhenASpellOrAbilityCausesAPermanentToBePutIntoAPlayersGraveyard",
            [cause, Objects(set), Players(players)],
        ) => caused(
            stack_filter(cause)?,
            zone_change(
                owned_among(objects_filter(set), players),
                Some(Z::Battlefield),
                Z::Graveyard,
            ),
        ),
        (
            "WhenASpellOrAbilityCausesAnyNumberOfCardToBeExiledFromAPlayersLibrary",
            [cause, Players(players)],
        ) => caused(
            stack_filter(cause)?,
            whole(zone_change(
                ObjectFilter::OwnedBy(players.clone()),
                Some(Z::Library),
                Z::Exile,
            )),
        ),
        ("WhenASpellCausesAPlayerToGainLife", [Filter(spells), Players(players)]) => caused(
            spells.clone(),
            player(PlayerEvent::GainsLife, players, None),
        ),
        ("WhenACardIsPutIntoAGraveyardFromAnywhereOtherThanTheBattlefield", [Filter(cards)]) => {
            EventSpec::ZoneChangeFromElsewhere {
                objects: cards.clone(),
                except: vec![Z::Battlefield],
                to: Z::Graveyard,
            }
        }
        (
            "WhenAPermanentEntersTheBattlefieldFromAnywhereOtherThanAGraveyardOrExile",
            [Objects(set)],
        ) => EventSpec::ZoneChangeFromElsewhere {
            objects: objects_filter(set),
            except: vec![Z::Graveyard, Z::Exile],
            to: Z::Battlefield,
        },
        ("WhenAPermanentEntersTheBattlefieldFromAnywhereOtherThanTheirHand", [Objects(set)]) => {
            EventSpec::ZoneChangeFromElsewhere {
                objects: objects_filter(set),
                except: vec![Z::Hand],
                to: Z::Battlefield,
            }
        }
        ("WhenACardIsPutIntoExile", [Filter(cards)]) => zone_change(cards.clone(), None, Z::Exile),
        ("WhenACardIsPutIntoTheCommandZoneFromAnywhere", [Filter(cards)]) => {
            zone_change(cards.clone(), None, Z::Command)
        }
        ("WhenAPermanentEntersTheBattlefieldDuringTheDeclareAttacksStep", [Objects(set)]) => {
            EventSpec::Filtered {
                event: Box::new(zone_change(objects_filter(set), None, Z::Battlefield)),
                condition: Condition::Window(TurnWindow::DeclareAttackers),
            }
        }
        (
            "WhenAnyNumberOfPermanentsEnterTheBattlefieldUnderAPlayersControl",
            [Objects(set), Players(players)],
        ) => whole(zone_change(
            controlled_among(objects_filter(set), players),
            None,
            Z::Battlefield,
        )),
        (
            "WhenAnyNumberOfPermanentsArePutIntoAPlayersGraveyards",
            [Objects(set), Players(players)],
        ) => whole(zone_change(
            owned_among(objects_filter(set), players),
            Some(Z::Battlefield),
            Z::Graveyard,
        )),
        (
            "WhenAnyNumberOfPermanentsAndOrGraveyardCardsArePutIntoExile",
            [Objects(set), Filter(cards)],
        ) => whole(EventSpec::Either(vec![
            zone_change(objects_filter(set), Some(Z::Battlefield), Z::Exile),
            zone_change(cards.clone(), Some(Z::Graveyard), Z::Exile),
        ])),
        (
            "WhenAnyNumberOfCardsArePutIntoExileFromAPlayersGraveyardAndOrLibrary",
            [Filter(cards), Players(players)],
        ) => whole(EventSpec::Either(vec![
            zone_change(
                owned_among(cards.clone(), players),
                Some(Z::Graveyard),
                Z::Exile,
            ),
            zone_change(
                owned_among(cards.clone(), players),
                Some(Z::Library),
                Z::Exile,
            ),
        ])),
        ("WhenAPlayerDrawsTheirNthCardDuringTheirDrawStep", [Players(players), Compare(c, n)]) => {
            EventSpec::Filtered {
                event: Box::new(nth(player(PlayerEvent::DrawsCard, players, None), c, n)),
                condition: Condition::Window(TurnWindow::DrawStep),
            }
        }
        ("WhenAPlayerDrawsACardExceptTheFirstCardDuringTheirDrawStep", [Players(players)]) => {
            EventSpec::Filtered {
                event: Box::new(player(PlayerEvent::DrawsCard, players, None)),
                condition: Condition::Not(Box::new(Condition::FirstDrawInOwnDrawStep(
                    PlayerRef::EventSubject,
                ))),
            }
        }
        ("WhenAPlayerDrawsTheirNthCardDuringTheirTurn", [Players(players), Compare(c, n)]) => {
            nth(player(PlayerEvent::DrawsCard, players, None), c, n)
        }
        ("WhenTheNthSpellIsCastInATurn", [Filter(spells), Compare(c, n)]) => nth(
            EventSpec::Spell {
                event: SpellEvent::Cast,
                players: PlayerSet::All,
                spells: spells.clone(),
            },
            c,
            n,
        ),
        ("WhenAPlayerRollsTheirNthDieEachTurn", [Players(players), Number(n)]) => nth(
            player(PlayerEvent::RollsADie, players, None),
            &Comparison::Equal,
            n,
        ),
        ("WhenTheNthCounterOfTypeIsPutOnAPermanent", [kind, Objects(set), Number(n)]) => {
            EventSpec::NthCounter {
                kind: Box::new(kind.clone()),
                objects: objects_filter(set),
                nth: n.clone(),
            }
        }
        ("WhenAPlayerPlaysACardFromExile", [Players(players), Objects(set)]) => player(
            PlayerEvent::PlaysCardFromExile,
            players,
            Some(objects_filter(set)),
        ),
        ("WhenAPlayerPlaysALandFromAmongCardsInExile", [Players(players), lands, Objects(set)]) => {
            player(
                PlayerEvent::PlaysLandFromExile,
                players,
                Some(ObjectFilter::All(vec![
                    set_filter(lands)?,
                    objects_filter(set),
                ])),
            )
        }
        (
            "WhenAPlayerCastsASpellFromAnywhereOtherThanTheirHand",
            [Players(players), Filter(spells)],
        ) => EventSpec::Spell {
            event: SpellEvent::Cast,
            players: players.clone(),
            spells: ObjectFilter::All(vec![
                spells.clone(),
                ObjectFilter::Not(Box::new(cast_from(EventZone::Hand))),
            ]),
        },
        ("WhenAPlayerCastsASpellOrActivatesAnAbility", [Players(players), Filter(spells)]) => {
            EventSpec::Either(vec![
                EventSpec::Spell {
                    event: SpellEvent::Cast,
                    players: players.clone(),
                    spells: spells.clone(),
                },
                EventSpec::Activated {
                    players: players.clone(),
                    abilities: spells.clone(),
                },
            ])
        }
        ("WhenAPlayerAttacksAnyNumberOfPlayers", [Players(by), Players(defenders)]) => {
            whole(EventSpec::Attack {
                attackers: ObjectFilter::Any,
                by: Some(by.clone()),
                defenders: AttackDefenders::Players(defenders.clone()),
            })
        }
        ("WhenAPlayerAttacksAnyNumberOfPlaneswalkers", [Players(by), Objects(set)]) => {
            whole(EventSpec::Attack {
                attackers: ObjectFilter::Any,
                by: Some(by.clone()),
                defenders: AttackDefenders::Objects(objects_filter(set)),
            })
        }
        (
            "WhenAnyNumberOfCreaturesAttackAnyNumberOfPlayers",
            [Objects(set), Players(defenders)],
        ) => whole(EventSpec::Attack {
            attackers: objects_filter(set),
            by: None,
            defenders: AttackDefenders::Players(defenders.clone()),
        }),
        (
            "WhenAPlayerAttacksAPlaneswalkerWithAnyNumberOfCreatures",
            [Players(by), Objects(defenders), Objects(set)],
        ) => EventSpec::Batch {
            event: Box::new(EventSpec::Attack {
                attackers: objects_filter(set),
                by: Some(by.clone()),
                defenders: AttackDefenders::Objects(objects_filter(defenders)),
            }),
            per: BatchPer::Defender,
            count: None,
            including: None,
        },
        _ => return None,
    })
}

/// A row that names what an earlier step acted on, by the step's words.
fn earlier_reference(form: &str) -> Option<EarlierObjects> {
    use EarlierObjects as E;
    Some(match form {
        "ThePermanentThisSpellBecame" => E::SpellBecame,
        "ThePermanentGainedControlOfThisWay" => E::GainedControl,
        "TheCardsConjuredIntoHandThisWay" => E::Conjured,
        "TheLastCardDrawnThisTurn" => E::LastDrawn,
        "TheCardRevealedFromHandThisWay" => E::Revealed,
        "TheLastGraveyardCardChosenThisWay"
        | "TheChosenCardsInHand"
        | "TheChosenPermanentForPlayer" => E::Selected,
        "ThePermanentChosenThisWay"
        | "TheExiledCardChosenThisWay"
        | "TheChosenExiledCard"
        | "TheChosenPermanents"
        | "ThePermanentsChosenThisWay"
        | "TheGraveyardCardsChosenThisWay"
        | "TheSecretlyChosenPermanents" => E::Selected,
        "TheLastExiledCard" => E::LastExiled,
        "TheCardExiledThisWay"
        | "TheCardsExiledThisWay"
        | "TheCardsExiledByPlayerThisWay"
        | "ThePermanentExiledByPlayerThisWay" => E::Exiled,
        "TheCreatureUnequippedThisWay" => E::Unattached,
        "TheCopiedSpell" => E::CopiedSpell,
        "TheCardsPutIntoAGraveyardThisWay" => E::PutInGraveyard,
        "TheCreatedTokens" | "TheTokensCreatedByAPlayerThisWay" => E::Created,
        "RevealedFromLibraryThisWay" | "RevealedFromPlayersLibraryThisWay" => E::Revealed,
        "TheSpellMostRecentlyCastThisTurn" => E::LastCast,
        _ => return None,
    })
}

/// The event's own subject, under the name a row gives it.
fn event_alias(form: &str) -> bool {
    matches!(
        form,
        "WouldUntapDuringsItsControllersUntapStep_ThatPermanent"
    )
}

pub fn reduce_object(name: String, operands: Vec<Operand>) -> ObjectRef {
    let form = name.split_once("::").map_or("", |(_, form)| form);
    if let [.., Operand::Result(result)] = operands.as_slice() {
        let by = operands.iter().find_map(|operand| match operand {
            Operand::Player(player) => Some(player.clone()),
            _ => None,
        });
        return ObjectRef::Recorded {
            result: result.clone(),
            by,
        };
    }
    if event_alias(form) {
        return ObjectRef::EventSubject;
    }
    if let Some(object) = reduce_value_object(form, &operands) {
        return object;
    }
    match (form, operands.as_slice(), earlier_reference(form)) {
        (_, [], Some(what)) => ObjectRef::Earlier { what, by: None },
        (_, [Operand::Player(player)], Some(what)) => ObjectRef::Earlier {
            what,
            by: Some(player.clone()),
        },
        ("PermanentSourceOfAbility", [Operand::Object(ability)], _) => {
            ObjectRef::SourceOf(Box::new(ability.clone()))
        }
        ("ThisExiledPermanentCard", [], _) => ObjectRef::Source,
        ("Trigger_ThatPermanent" | "ThatSpell", [], _) => ObjectRef::Scoped,
        // In the filter of the damage event that names it: what would deal the damage.
        ("WouldDealDamage_ThatPermanent", [], _) => ObjectRef::Damage(DamageSubject::Source),
        // The card an instruction on each exiled card is looking at.
        ("EachableExiled", [], _) => ObjectRef::Affected,
        ("Trigger_ThatSpellOrAbility", [], _) => ObjectRef::EventStackObject,
        ("PlayersRingBearer", [Operand::Player(player)], _) => {
            ObjectRef::RingBearer(player.clone())
        }
        ("TheCreatureHauntedByExiledCard", [Operand::Object(object)], _) => {
            ObjectRef::HauntedBy(Box::new(object.clone()))
        }
        ("TheFirstPermanentChosenByPlayerThisWay", [Operand::Player(player)], _) => {
            ObjectRef::NthChosen {
                index: 0,
                by: Some(player.clone()),
            }
        }
        ("TheSecondPermanentChosenByPlayerThisWay", [Operand::Player(player)], _) => {
            ObjectRef::NthChosen {
                index: 1,
                by: Some(player.clone()),
            }
        }
        ("TheSpellThatGrantedThisAbility" | "ThePermanentThatCreatedThisEmblem", [], _) => {
            ObjectRef::GrantingObject
        }
        _ => ObjectRef::Primitive { name, operands },
    }
}

pub fn reduce_objects(name: String, operands: Vec<Operand>) -> ObjectSet {
    let form = name.split_once("::").map_or("", |(_, form)| form);
    if let [.., Operand::Result(result)] = operands.as_slice() {
        return ObjectSet::Recorded(result.clone());
    }
    if let Some(objects) = tail_objects(form, &operands) {
        return objects;
    }
    if let Some(objects) = reduce_value_objects(form, &operands) {
        return objects;
    }
    match (operands.as_slice(), earlier_reference(form)) {
        ([], Some(what)) => ObjectSet::Earlier { what, by: None },
        ([Operand::Player(player)], Some(what)) => ObjectSet::Earlier {
            what,
            by: Some(PlayerSet::One(player.clone())),
        },
        ([Operand::Players(players)], Some(what)) => ObjectSet::Earlier {
            what,
            by: Some(players.clone()),
        },
        _ => ObjectSet::Primitive { name, operands },
    }
}

/// A set an object filter names, by the step that made it.
fn earlier_filter(form: &str, operands: &[Operand]) -> Option<ObjectFilter> {
    if let [.., Operand::Result(result)] = operands {
        if earlier_reference(form).is_some() {
            return Some(ObjectFilter::InSet(Box::new(ObjectSet::Recorded(
                result.clone(),
            ))));
        }
    }
    let what = earlier_reference(form)?;
    let by = match operands {
        [] => None,
        [Operand::Player(player)] => Some(PlayerSet::One(player.clone())),
        [Operand::Players(players)] => Some(players.clone()),
        _ => return None,
    };
    Some(ObjectFilter::InSet(Box::new(ObjectSet::Earlier {
        what,
        by,
    })))
}

/// A player read off the step that named them.
pub fn reduce_player_ref(name: String, operands: Vec<Operand>) -> PlayerRef {
    let form = name.split_once("::").map_or("", |(_, form)| form);
    if let Some(player) = reduce_value_player(form, &operands) {
        return player;
    }
    match (form, operands.as_slice()) {
        (_, [Operand::Result(result)]) => PlayerRef::Named {
            result: result.clone(),
            index: match form {
                "TheSecondPlayerChosenThisWay" => 1,
                "TheThirdPlayerChosenThisWay" => 2,
                _ => 0,
            },
        },
        ("WinningBidder", []) => PlayerRef::Prior(PriorPlayer::WinningBidder),
        ("RememberedPlayer", []) => PlayerRef::Prior(PriorPlayer::Remembered),
        ("ThePlayerWhoCreatedThisAbility" | "ThePlayerThatAddedThisAbility", []) => {
            PlayerRef::Prior(PriorPlayer::GrantedBy)
        }
        ("ControllerOfLeavingPermanent", []) => {
            PlayerRef::ControllerOf(Box::new(ObjectRef::EventSubject))
        }
        (
            "Trigger_ThatPlayer"
            | "NumPlayers_ThatPlayer"
            | "APlayerWouldMillAnyNumberOfCards_ThatPlayer"
            | "MillCards_ThatPlayer"
            | "LoseLifeForEach_ThatPlayer"
            | "AsLoseUnspentMana_ThePlayerLosingMana"
            | "EachablePlayer",
            [],
        ) => PlayerRef::Prior(PriorPlayer::That),
        ("WouldDealDamage_ControllerOfDamageSource", []) => {
            PlayerRef::ControllerOf(Box::new(ObjectRef::Damage(DamageSubject::Source)))
        }
        ("Trigger_ControllerOfThoseCreatures" | "ControllerOfDeadPermanent", []) => {
            PlayerRef::ControllerOf(Box::new(ObjectRef::Scoped))
        }
        // Each row has the one target: the permanent just returned to that hand.
        ("OwnerOfTargetPermanent", []) => PlayerRef::OwnerOf(Box::new(ObjectRef::Target {
            slot: TargetSlot(0),
            index: 0,
        })),
        ("AssociatedPlayerForPermanent", [Operand::Object(object)]) => {
            PlayerRef::NotedFor(Box::new(object.clone()))
        }
        ("TheOtherChosenPlayer", [Operand::Player(player)]) => {
            PlayerRef::OtherChosen(Box::new(player.clone()))
        }
        ("NearestPlayerInChosenDirection", [Operand::Players(players)]) => {
            PlayerRef::InChosenDirection {
                from: None,
                among: Some(Box::new(players.clone())),
            }
        }
        ("PlayerInTheChosenDirectionOf", [Operand::Player(player)]) => {
            PlayerRef::InChosenDirection {
                from: Some(Box::new(player.clone())),
                among: None,
            }
        }
        _ => PlayerRef::Primitive { name, operands },
    }
}
