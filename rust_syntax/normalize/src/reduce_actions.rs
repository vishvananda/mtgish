// One-shot actions by what they do, as Forge's `ChangeZone` and `Play`: what moves
// or is cast, how many, chosen or at random, and where it goes. Effects that wait
// for a later event read as `EffectOn`. What is unmatched stays with its row.

fn subject_set(operand: &Operand) -> Option<ObjectSet> {
    match operand {
        Operand::Object(object) => Some(ObjectSet::One(object.clone())),
        Operand::Objects(objects) => Some(objects.clone()),
        Operand::Filter(filter) => Some(ObjectSet::AnyZone(filter.clone())),
        _ => None,
    }
}

fn effects_list(operand: &Operand) -> Vec<Operand> {
    match operand {
        Operand::List(items) => items.clone(),
        other => vec![other.clone()],
    }
}

fn effect_on(subject: EffectSubject, effects: Vec<Operand>, until: Option<Duration>) -> Operation {
    Operation::EffectOn {
        subject,
        effects,
        until,
    }
}

fn next_spell(term: &Operand) -> Option<EffectSubject> {
    let Operand::Term { form, operands, .. } = term else {
        return None;
    };
    let (this_turn, from) = match form.as_str() {
        "TheNextSpellPlayerCasts" => (false, None),
        "TheNextSpellPlayerCastsThisTurn" => (true, None),
        "TheNextSpellPlayerCastsFromTheirHandThisTurn" => (true, Some(EventZone::Hand)),
        _ => return None,
    };
    let [Operand::Filter(spells), Operand::Player(caster)] = operands.as_slice() else {
        return None;
    };
    Some(EffectSubject::NextSpell {
        spells: spells.clone(),
        caster: caster.clone(),
        this_turn,
        from,
    })
}

/// Effects on what an action names, for the whole game or until something.
fn reduce_effect_on(form: &str, operands: &[Operand]) -> Option<Operation> {
    use Operand::{Duration as Until, List, Object, Objects, Player, Players};
    let objects = |set| EffectSubject::Objects(set);
    match (form, operands) {
        (
            "CreatePerpetualCardInHandEffect"
            | "CreatePerpetualCardInLibraryEffect"
            | "CreatePerpetualAllCardsEffect"
            | "CreatePerpetualEachGraveyardCardEffect"
            | "CreatePerpetualPermanentEffect"
            | "CreatePerpetualGraveyardCardEffect"
            | "CreatePerpetualCardsInHandEffect",
            [subject, effects],
        ) => Some(effect_on(
            objects(subject_set(subject)?),
            effects_list(effects),
            None,
        )),
        ("CreatePerpetualDeadCardEffect", effects) => Some(effect_on(
            objects(ObjectSet::One(ObjectRef::EventSubject)),
            effects.to_vec(),
            None,
        )),
        ("CreatePerpetualPermanentOrGraveyardCardEffect", [Object(a), Object(b), effects]) => {
            Some(effect_on(
                objects(ObjectSet::Union(vec![
                    ObjectSet::One(a.clone()),
                    ObjectSet::One(b.clone()),
                ])),
                effects_list(effects),
                None,
            ))
        }
        (
            "CreatePerpetualCardsInEachPlayersHandEffect",
            [Operand::Filter(cards), Players(players), effects],
        ) => Some(effect_on(
            objects(zone_set(EventZone::Hand, players.clone(), cards.clone())),
            effects_list(effects),
            None,
        )),
        (
            "CreatePerpetualCardsInPlayersHandAndCardsInPlayersLibraryEffect",
            [Operand::Filter(hand), Players(a), Operand::Filter(library), Players(b), effects],
        ) => Some(effect_on(
            objects(ObjectSet::Union(vec![
                zone_set(EventZone::Hand, a.clone(), hand.clone()),
                zone_set(EventZone::Library, b.clone(), library.clone()),
            ])),
            effects_list(effects),
            None,
        )),
        ("CreateFuturePlayerEffect", [Player(player), effect]) => Some(effect_on(
            EffectSubject::Players(PlayerSet::One(player.clone())),
            vec![effect.clone()],
            None,
        )),
        ("CreateFutureSpellEffect", [spell, List(effects)]) => {
            Some(effect_on(next_spell(spell)?, effects.clone(), None))
        }
        ("CreateLimitedSpellEffect", [Until(until), Object(spell), List(effects)]) => {
            Some(effect_on(
                objects(ObjectSet::One(spell.clone())),
                effects.clone(),
                Some(until.clone()),
            ))
        }
        ("CreateCardInHandEffectUntil", [Object(card), List(effects), Until(until)]) => {
            Some(effect_on(
                objects(ObjectSet::One(card.clone())),
                effects.clone(),
                Some(until.clone()),
            ))
        }
        ("CreateGroupExileEffect", [Until(until), Objects(cards), List(effects)]) => {
            Some(effect_on(
                objects(cards.clone()),
                effects.clone(),
                Some(until.clone()),
            ))
        }
        _ => None,
    }
}

struct ZoneMove {
    cards: ObjectSet,
    count: Option<Count>,
    random: bool,
    group: Option<Operand>,
    destination: Destination,
    entry: Vec<Operand>,
    face_down: Option<FaceDownAs>,
}

impl ZoneMove {
    fn new(cards: ObjectSet, destination: Destination) -> Self {
        Self {
            cards,
            count: None,
            random: false,
            group: None,
            destination,
            entry: vec![],
            face_down: None,
        }
    }
    fn choose(mut self, count: Count) -> Self {
        self.count = Some(count);
        self
    }
    fn one(self) -> Self {
        self.choose(Count::Exactly(Expr::Constant(1)))
    }
    fn at_random(mut self) -> Self {
        self.random = true;
        self.one()
    }
    fn group(mut self, group: &Operand) -> Self {
        self.group = Some(group.clone());
        self
    }
    fn entry(mut self, flags: &Operand) -> Self {
        self.entry = effects_list(flags);
        self
    }
    fn face_down(mut self, as_: FaceDownAs) -> Self {
        self.face_down = Some(as_);
        self
    }
    fn operation(self, actor: &PlayerSet) -> Operation {
        Operation::DraftChangeZone {
            actor: actor.clone(),
            cards: self.cards,
            count: self.count,
            random: self.random,
            group: self.group.map(Box::new),
            destination: self.destination,
            entry: self.entry,
            face_down: self.face_down,
        }
    }
}

fn artifact_creature(pt: &Operand, kind: &Operand) -> FaceDownAs {
    FaceDownAs {
        power_toughness: Box::new(pt.clone()),
        creature_type: Some(Box::new(kind.clone())),
        artifact: true,
    }
}

/// Commanders in a zone, which a row names by owner or not at all.
fn commanders_in(zone: EventZone, term: Option<&Operand>, actor: &PlayerSet) -> Option<ObjectSet> {
    let filter = match term {
        Some(term) => commander_filter(term)?,
        None => ObjectFilter::State(ObjectState::Commander),
    };
    Some(match zone {
        EventZone::Graveyard => zone_set(zone, actor.clone(), filter),
        zone => ObjectSet::Zone {
            scope: ZoneScope::Shared(zone),
            kind: ObjectKind::Card,
            filter,
        },
    })
}

/// A filter a row spells in `CardsInHand` terms.
fn hand_filter(operand: &Operand) -> Option<ObjectFilter> {
    match operand {
        Operand::Filter(filter) => Some(filter.clone()),
        Operand::Term { form, operands, .. } => match (form.as_str(), operands.as_slice()) {
            ("AnyCard", []) => Some(ObjectFilter::Any),
            ("IsCardtype", [kind]) => Some(ObjectFilter::CardType(vocabulary_value(kind)?)),
            ("IsNonCardtype", [kind]) => Some(ObjectFilter::Not(Box::new(ObjectFilter::CardType(
                vocabulary_value(kind)?,
            )))),
            _ => None,
        },
        _ => None,
    }
}

// A death trigger can use the first graveyard incarnation, never the departed
// battlefield object or an incarnation produced by a subsequent zone change.
fn dead_graveyard_card() -> ObjectSet {
    ObjectSet::One(ObjectRef::Query {
        objects: Box::new(ObjectSet::InZone {
            objects: Box::new(ObjectSet::One(ObjectRef::EventDestination)),
            zone: EventZone::Graveyard,
        }),
        index: 0,
    })
}

/// Cards that change zones, as Forge's `ChangeZone` with `Origin$` and `Destination$`.
fn reduce_change_zone(actor: &PlayerSet, form: &str, operands: &[Operand]) -> Option<Operation> {
    use Destination as D;
    use EventZone as Z;
    use Operand::{Filter, List, Number, Object, Objects, Player, Players};
    let in_graveyard = |players: &PlayerSet, cards: &ObjectFilter| {
        zone_set(Z::Graveyard, players.clone(), cards.clone())
    };
    let of = |player: &PlayerRef| PlayerSet::One(player.clone());
    let graveyard = |cards: &ObjectFilter| in_graveyard(actor, cards);
    let hand = |cards: &ObjectFilter| zone_set(Z::Hand, actor.clone(), cards.clone());
    let any = ObjectFilter::Any;
    let up_to = |n: &Expr| Count::UpTo(n.clone());
    let library_top = |player: &PlayerRef, n: Expr| ObjectSet::Take {
        objects: Box::new(zone_set(Z::Library, of(player), ObjectFilter::Any)),
        count: n,
        from: SequenceEnd::Top,
    };
    let library_bottom = |player: &PlayerRef| ObjectSet::Take {
        objects: Box::new(zone_set(Z::Library, of(player), ObjectFilter::Any)),
        count: Expr::Constant(1),
        from: SequenceEnd::Bottom,
    };
    let you = || match actor {
        PlayerSet::One(player) => Some(player.clone()),
        _ => None,
    };
    let moved = match (form, operands) {
        ("ReturnACardFromGraveyardToHandAtRandom", [Filter(cards)]) => {
            ZoneMove::new(graveyard(cards), D::OwnersHand).at_random()
        }
        (
            "ReturnAnyNumberOfGroupCardsFromPlayersGraveyardToBattlefield",
            [Filter(cards), group, Player(owner), flags],
        ) => ZoneMove::new(in_graveyard(&of(owner), cards), D::OwnersBattlefield)
            .choose(Count::AnyNumber)
            .group(group)
            .entry(flags),
        ("ReturnAnyNumberOfGroupCardsFromGraveyardToHand", [Filter(cards), group]) => {
            ZoneMove::new(graveyard(cards), D::OwnersHand)
                .choose(Count::AnyNumber)
                .group(group)
        }
        (
            "PutAnyNumberOfCardsFromHandOrFromPlayersGraveyardOnBattlefield",
            [Filter(cards), Player(owner), flags],
        ) => ZoneMove::new(
            ObjectSet::Union(vec![hand(cards), in_graveyard(&of(owner), cards)]),
            D::OwnersBattlefield,
        )
        .choose(Count::AnyNumber)
        .entry(flags),
        ("PutACommanderFromCommandZoneIntoHand", [commanders]) => ZoneMove::new(
            commanders_in(Z::Command, Some(commanders), actor)?,
            D::OwnersHand,
        )
        .one(),
        ("PutEachCommanderFromCommandZoneIntoHand", [commanders]) => ZoneMove::new(
            commanders_in(Z::Command, Some(commanders), actor)?,
            D::OwnersHand,
        ),
        ("PutEachCommanderFromGraveyardIntoHand", []) => ZoneMove::new(
            commanders_in(Z::Graveyard, None, actor)?,
            D::OwnersHand,
        ),
        ("PutACommanderFromCommandZoneOntoBattlefield", [commanders, flags]) => ZoneMove::new(
            ObjectSet::Matching {
                objects: Box::new(commanders_in(Z::Command, Some(commanders), actor)?),
                filter: ObjectFilter::OwnedBy(actor.clone()),
            },
            D::OwnersBattlefield,
        )
        .one()
        .entry(flags),
        ("PutEachCommanderFromCommandZoneOntoBattlefield", [commanders, flags]) => ZoneMove::new(
            commanders_in(Z::Command, Some(commanders), actor)?,
            D::OwnersBattlefield,
        )
        .entry(flags),
        ("PutCommanderFromCommandZoneOntoBattlefield", [_, flags]) => {
            ZoneMove::new(ObjectSet::One(ObjectRef::Source), D::OwnersBattlefield).entry(flags)
        }
        (
            "ReturnMultipleCardsFromPlayersGraveyardToBattlefield",
            [List(filters), Player(owner), flags],
        ) => {
            let parts = filters
                .iter()
                .map(|filter| match filter {
                    Filter(cards) => Some(cards.clone()),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?;
            let count = parts.len() as i64;
            ZoneMove::new(
                in_graveyard(&of(owner), &ObjectFilter::Either(parts)),
                D::OwnersBattlefield,
            )
            .choose(Count::UpTo(Expr::Constant(count)))
            .entry(flags)
        }
        (
            "ReturnUptoNumberCardsFromPlayersGraveyardToBattlefield",
            [Number(n), Filter(cards), Player(owner), flags],
        ) => ZoneMove::new(in_graveyard(&of(owner), cards), D::OwnersBattlefield)
            .choose(up_to(n))
            .entry(flags),
        (
            "ReturnUptoNumberGroupCardsFromPlayersGraveyardToBattlefield",
            [Number(n), Filter(cards), group, Player(owner), flags],
        ) => ZoneMove::new(in_graveyard(&of(owner), cards), D::OwnersBattlefield)
            .choose(up_to(n))
            .group(group)
            .entry(flags),
        ("ReturnGraveyardCardToHand", [Object(card)]) => {
            ZoneMove::new(ObjectSet::One(card.clone()), D::OwnersHand)
        }
        ("PutACardFromGraveyardIntoHandExceptForGraveyardCard", [Filter(cards), Object(card)]) => {
            ZoneMove::new(
                ObjectSet::Difference {
                    objects: Box::new(graveyard(cards)),
                    excluding: Box::new(ObjectSet::One(card.clone())),
                },
                D::OwnersHand,
            )
            .one()
        }
        ("ReturnEachExiledCardToGraveyard", [Objects(cards)]) => {
            ZoneMove::new(cards.clone(), D::OwnersGraveyard)
        }
        ("ReturnEachExiledCardToOwnersHand" | "PutExiledPileIntoOwnersHand", [Objects(cards)]) => {
            ZoneMove::new(cards.clone(), D::OwnersHand)
        }
        ("PutExiledCardOntoBattlefield", [Object(card), flags])
        | ("PutCardFromHandOnBattlefield", [Object(card), flags]) => {
            ZoneMove::new(ObjectSet::One(card.clone()), D::OwnersBattlefield).entry(flags)
        }
        ("ReturnThisExiledCardToBattlefield", [flags]) => {
            ZoneMove::new(ObjectSet::One(ObjectRef::Source), D::OwnersBattlefield).entry(flags)
        }
        ("PutACardAndOrACardFromHandOnBattlefield", [a, b, flags]) => ZoneMove::new(
            hand(&ObjectFilter::Either(vec![
                hand_filter(a)?,
                hand_filter(b)?,
            ])),
            D::OwnersBattlefield,
        )
        .choose(Count::UpTo(Expr::Constant(2)))
        .entry(flags),
        ("PutAnyNumberOfCardsFromAmongExileOntoBattlefield", [Filter(cards), Objects(among), flags]) => {
            ZoneMove::new(
                ObjectSet::Matching {
                    objects: Box::new(among.clone()),
                    filter: cards.clone(),
                },
                D::OwnersBattlefield,
            )
            .choose(Count::AnyNumber)
            .entry(flags)
        }
        ("PutACardFromOutsideGameOnTopOfLibrary", [Filter(cards)]) => {
            ZoneMove::new(ObjectSet::OutsideTheGame(cards.clone()), D::OwnersLibraryTop).one()
        }
        ("PutACardFromOutsideGameInHand", [Filter(cards)]) => {
            ZoneMove::new(ObjectSet::OutsideTheGame(cards.clone()), D::OwnersHand).one()
        }
        // "You put that card into your hand": the card an earlier instruction
        // chose from outside the game, not a fresh choice (Extrapolate the Impossible).
        ("PutACardFromOutsideGameInHand", [Objects(ObjectSet::Selected(chosen) | ObjectSet::Named(chosen))]) => {
            ZoneMove::new(ObjectSet::Selected(chosen.clone()), D::OwnersHand)
        }
        ("PutACardFromOutsideGameInHand", [_]) => {
            ZoneMove::new(ObjectSet::OutsideTheGame(any.clone()), D::OwnersHand).one()
        }
        ("ShuffleUptoNumberCardsFromOutsideTheGameIntoLibrary", [Number(n)]) => ZoneMove::new(
            ObjectSet::OutsideTheGame(any.clone()),
            D::OwnersLibraryShuffled,
        )
        .choose(up_to(n)),
        ("PutTheTopNumberCardsOfLibraryInHand", [Number(n)]) => {
            ZoneMove::new(library_top(&you()?, n.clone()), D::OwnersHand)
        }
        ("PutTheTopCardOfPlayersLibraryOnTheBottomOfTheirLibrary", [Player(player)]) => {
            ZoneMove::new(library_top(player, Expr::Constant(1)), D::OwnersLibraryBottom)
        }
        ("PutTheBottomCardOfPlayersLibraryIntoGraveyard", [Player(player)]) => {
            ZoneMove::new(library_bottom(player), D::OwnersGraveyard)
        }
        ("PutTheTopCardOfPlayersLibraryInGraveyard" | "PutTopOfOtherLibraryInGraveyard", [Player(player)]) => {
            ZoneMove::new(library_top(player, Expr::Constant(1)), D::OwnersGraveyard)
        }
        ("PutCardFromHandIntoPlayersHand", [Object(card), Player(player)]) => {
            ZoneMove::new(ObjectSet::One(card.clone()), D::Hand(player.clone()))
        }
        ("ReturnPermanentToCommandZone", [Object(card)]) => {
            ZoneMove::new(ObjectSet::One(card.clone()), D::CommandZone)
        }
        ("ReturnEachPermanentToCommandZone", [Objects(cards)]) => {
            ZoneMove::new(cards.clone(), D::CommandZone)
        }
        ("PutDeadPermanentOnTopOfLibrary", []) => {
            ZoneMove::new(dead_graveyard_card(), D::OwnersLibraryTop)
        }
        ("PutDeadPermanentOnBottomOfLibrary", []) => {
            ZoneMove::new(dead_graveyard_card(), D::OwnersLibraryBottom)
        }
        ("PutDeadPermanentOnTopOfLibraryOrOnBottomOfLibrary", []) => {
            ZoneMove::new(dead_graveyard_card(), D::OwnersLibraryTopOrBottom)
        }
        (
            "PutEachExiledCardOntoTheBattlefieldFaceDownAsAnArtifactCreature",
            [Objects(cards), pt, kind],
        ) => ZoneMove::new(cards.clone(), D::OwnersBattlefield)
            .face_down(artifact_creature(pt, kind)),
        (
            "PutEachGraveyardCardOntoBattlefieldFaceDownAsAnArtifactCreature",
            [Filter(cards), flags, pt, kind],
        ) => ZoneMove::new(graveyard(cards), D::OwnersBattlefield)
            .entry(flags)
            .face_down(artifact_creature(pt, kind)),
        (
            "PutEachGraveyardCardOntoBattlefieldFaceDownAsAnArtifactCreature",
            [Objects(cards), flags, pt, kind],
        ) => ZoneMove::new(cards.clone(), D::OwnersBattlefield)
            .entry(flags)
            .face_down(artifact_creature(pt, kind)),
        (
            "PutTheTopNumberCardsOfPlayersLibraryOntoBattlefieldAsFaceDownArtifactCreatures",
            [Number(n), pt, kind],
        ) => ZoneMove::new(library_top(&you()?, n.clone()), D::OwnersBattlefield)
            .face_down(artifact_creature(pt, kind)),
        ("PutAnyNumberOfCardsFromHandOntoBattlefieldAsFaceDownArtifactCreatures", [pt]) => {
            ZoneMove::new(hand(&any), D::OwnersBattlefield)
                .choose(Count::AnyNumber)
                .face_down(FaceDownAs {
                    power_toughness: Box::new(pt.clone()),
                    creature_type: None,
                    artifact: true,
                })
        }
        ("ReturnEachCardFromPlayersGraveyardToHandThatWasPutThereFromBattlefieldThisTurn", [Filter(cards), Player(owner)]) => {
            ZoneMove::new(
                in_graveyard(
                    &of(owner),
                    &ObjectFilter::All(vec![
                        cards.clone(),
                        ObjectFilter::History {
                            event: ObjectHistory::DiedOrPutFromBattlefield,
                            window: ObjectWindow::ThisTurn,
                        },
                    ]),
                ),
                D::OwnersHand,
            )
        }
        (
            "ReturnEachCardFromEachPlayersGraveyardToBattlefieldThatWasPutThereFromTheBattlefieldThisTurn",
            [Filter(cards), Players(owners), flags],
        ) => ZoneMove::new(
            in_graveyard(
                owners,
                &ObjectFilter::All(vec![
                    cards.clone(),
                    ObjectFilter::History {
                        event: ObjectHistory::DiedOrPutFromBattlefield,
                        window: ObjectWindow::ThisTurn,
                    },
                ]),
            ),
            D::OwnersBattlefield,
        )
        .entry(flags),
        (
            "ReturnEachCardFromEachPlayersGraveyardToBattlefieldThatWasPutThereFromAnywhereThisTurn",
            [Filter(cards), Players(owners), flags],
        ) => ZoneMove::new(
            in_graveyard(
                owners,
                &ObjectFilter::All(vec![
                    cards.clone(),
                    ObjectFilter::History {
                        event: ObjectHistory::PutIntoGraveyard,
                        window: ObjectWindow::ThisTurn,
                    },
                ]),
            ),
            D::OwnersBattlefield,
        )
        .entry(flags),
        ("PutACardFromEachPlayersGraveyardOntoTheBattlefield", [Filter(cards), Players(owners), flags]) => {
            ZoneMove::new(in_graveyard(owners, cards), D::OwnersBattlefield)
                .choose(Count::PerPlayer {
                    players: owners.clone(),
                    each: Box::new(Count::Exactly(Expr::Constant(1))),
                })
                .entry(flags)
        }
        ("ReturnUptoOneCardOfEachPermanentTypeInPlayersGraveyardToBattlefield", [Player(owner), flags]) => {
            ZoneMove::new(in_graveyard(&of(owner), &ObjectFilter::Any), D::OwnersBattlefield)
                .choose(Count::AnyNumber)
                .group(&Operand::Vocabulary {
                    family: "GroupFilter".into(),
                    form: "OnePerPermanentType".into(),
                })
                .entry(flags)
        }
        ("ReturnEachCardFromGraveyardToHandThatWasCycledOrDiscardedThisTurn", [Filter(cards)]) => {
            ZoneMove::new(
                graveyard(&ObjectFilter::All(vec![
                    cards.clone(),
                    ObjectFilter::History {
                        event: ObjectHistory::CycledOrDiscarded,
                        window: ObjectWindow::ThisTurn,
                    },
                ])),
                D::OwnersHand,
            )
        }
        // "For each of those tokens, you may put an Aura card from your hand
        // and/or graveyard onto the battlefield attached to it" (Liberated
        // Livestock): up to one card for each, and "it" is that one.
        ("MayPutACardFromHandOrGraveyardOnBattlefieldForEachPermanent", [Filter(cards), flags, Objects(each)]) => {
            let put = ZoneMove::new(
                ObjectSet::Union(vec![hand(cards), graveyard(cards)]),
                D::OwnersBattlefield,
            )
            .choose(Count::UpTo(Expr::Constant(1)))
            .entry(flags);
            return Some(Operation::EachObject {
                objects: each.clone(),
                action: Box::new(put.operation(actor)),
            });
        }
        ("PutUptoNumberCardsFromHandAndOrGraveyardOnBattlefield", [Number(n), Filter(cards), Player(owner), flags]) => {
            ZoneMove::new(
                ObjectSet::Union(vec![hand(cards), in_graveyard(&of(owner), cards)]),
                D::OwnersBattlefield,
            )
            .choose(up_to(n))
            .entry(flags)
        }
        // The one card with this row says "up to one".
        ("PutACardDiscardedThisWayOntoTheBattlefield", [Filter(cards), flags, Operand::Result(result)]) => {
            ZoneMove::new(
                ObjectSet::Matching {
                    objects: Box::new(ObjectSet::Named(result.clone())),
                    filter: cards.clone(),
                },
                D::OwnersBattlefield,
            )
            .choose(Count::UpTo(Expr::Constant(1)))
            .entry(flags)
        }
        ("PutEachCardOfTypeMilledThisWayOntoTheBattlefield", [Filter(cards), flags, Operand::Result(result)]) => {
            ZoneMove::new(
                ObjectSet::Matching {
                    objects: Box::new(ObjectSet::Named(result.clone())),
                    filter: cards.clone(),
                },
                D::OwnersBattlefield,
            )
            .entry(flags)
        }
        ("ReturnAnyNumberOfGroupCardsMilledThisWayToBattlefield", [Filter(cards), group, flags, Operand::Result(result)]) => {
            ZoneMove::new(
                ObjectSet::Matching {
                    objects: Box::new(ObjectSet::Named(result.clone())),
                    filter: cards.clone(),
                },
                D::OwnersBattlefield,
            )
            .choose(Count::AnyNumber)
            .group(group)
            .entry(flags)
        }
        ("PutEachCardOfTypeMilledThisWayOntoTheBattlefieldAsFaceDownArtifactCreatures", [Filter(cards), pt, kind, Operand::Result(result)]) => {
            ZoneMove::new(
                ObjectSet::Matching {
                    objects: Box::new(ObjectSet::Named(result.clone())),
                    filter: cards.clone(),
                },
                D::OwnersBattlefield,
            )
            .face_down(artifact_creature(pt, kind))
        }
        ("PutACardOfTypeAndOrACardOfTypeMilledThisWayOntoTheBattlefield", [List(filters), flags, Operand::Result(result)]) => {
            let parts = filters
                .iter()
                .map(|filter| match filter {
                    Filter(cards) => Some(cards.clone()),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?;
            let count = parts.len() as i64;
            ZoneMove::new(
                ObjectSet::Matching {
                    objects: Box::new(ObjectSet::Named(result.clone())),
                    filter: ObjectFilter::Either(parts),
                },
                D::OwnersBattlefield,
            )
            .choose(Count::UpTo(Expr::Constant(count)))
            .entry(flags)
        }
        ("ReturnEachCardFromPlayersGraveyardToBattlefieldThatWasDestroyThisWay", [Filter(cards), Player(_), flags, Operand::Result(result)]) => {
            ZoneMove::new(
                ObjectSet::Matching {
                    objects: Box::new(ObjectSet::Named(result.clone())),
                    filter: cards.clone(),
                },
                D::OwnersBattlefield,
            )
            .entry(flags)
        }
        ("AnteTopCardOfLibrary", []) => ZoneMove::new(
            library_top(&you()?, Expr::Constant(1)),
            D::Ante,
        ),
        ("PutEachCardFromAnteInGraveyard", [Operand::Term { form, operands, .. }]) => {
            let filter = match (form.as_str(), operands.as_slice()) {
                ("OwnedByAPlayer", [Players(players)]) => ObjectFilter::OwnedBy(players.clone()),
                _ => return None,
            };
            ZoneMove::new(
                ObjectSet::Zone {
                    scope: ZoneScope::Shared(Z::Ante),
                    kind: ObjectKind::Card,
                    filter,
                },
                D::OwnersGraveyard,
            )
        }
        ("ShuffleAllButNumberCardsInHandIntoLibrary", [Number(n)]) => ZoneMove::new(
            ObjectSet::Difference {
                objects: Box::new(hand(&any)),
                excluding: Box::new(chosen(actor, &hand(&any), Count::Exactly(n.clone()))),
            },
            D::OwnersLibraryShuffled,
        ),
        ("ShuffleExiledCardsAndPutOnTopOfLibrary", [Objects(cards)]) => {
            let mut moved = ZoneMove::new(cards.clone(), D::OwnersLibraryTop);
            moved.random = true;
            moved
        }
        ("ReturnPermanentOrExiledCardToOwnersHand", [Object(card)]) => {
            ZoneMove::new(ObjectSet::One(card.clone()), D::OwnersHand)
        }
        ("ReturnTheExiledDeadPermanentToGraveyard", []) => {
            ZoneMove::new(ObjectSet::One(ObjectRef::EventSubject), D::OwnersGraveyard)
        }
        ("PutCardFromAnywhereIntoPlayersGraveyard", [_]) => {
            ZoneMove::new(ObjectSet::One(ObjectRef::Source), D::OwnersGraveyard)
        }
        ("PutCardsInLibraryIntoGraveyard", [Filter(cards)]) => {
            ZoneMove::new(ObjectSet::AnyZone(cards.clone()), D::OwnersGraveyard)
        }
        ("PutLibraryCardsIntoHand", [Filter(cards)]) => {
            ZoneMove::new(ObjectSet::AnyZone(cards.clone()), D::OwnersHand)
        }
        ("ShufflePermanentAndGraveyardIntoLibrary", [Object(card)]) => ZoneMove::new(
            ObjectSet::Union(vec![ObjectSet::One(card.clone()), graveyard(&any)]),
            D::OwnersLibraryShuffled,
        ),
        ("ShuffleSpellAndGraveyardCardsIntoLibraries", [Object(card), Objects(cards)]) => {
            ZoneMove::new(
                ObjectSet::Union(vec![ObjectSet::One(card.clone()), cards.clone()]),
                D::OwnersLibraryShuffled,
            )
        }
        ("PutARandomCardOfTypeFromAmongTheTopNumberCardsOfLibraryIntoHand", [Filter(cards), Number(n)]) => {
            ZoneMove::new(
                matching(&library_top(&you()?, n.clone()), cards),
                D::OwnersHand,
            )
            .at_random()
        }
        _ => return None,
    };
    Some(moved.operation(actor))
}

fn cast_with(
    actor: &PlayerSet,
    action: CardPlayAction,
    cards: ObjectSet,
    count: Count,
    terms: PlayTerms,
    group: Option<&Operand>,
) -> Operation {
    Operation::PlayWith {
        actor: actor.clone(),
        action,
        cards,
        count,
        terms,
        group: group.map(|group| Box::new(group.clone())),
    }
}

/// Cards cast or played as an effect resolves, on terms `PlayTerms` names.
fn reduce_cast(actor: &PlayerSet, form: &str, operands: &[Operand]) -> Option<Operation> {
    use CardPlayAction as A;
    use EventZone as Z;
    use Operand::{Filter, Object, Objects, Player, Players};
    let one = || Count::Exactly(Expr::Constant(1));
    let may = || Count::UpTo(Expr::Constant(1));
    let terms = play_terms;
    let free = || PlayTerms {
        cost: PlayCost::Free,
        ..terms()
    };
    let free_exile = || PlayTerms {
        cost: PlayCost::Free,
        into_exile: true,
        ..terms()
    };
    let spend = |spend_as| PlayTerms {
        spend_as: Some(spend_as),
        ..terms()
    };
    let alternate = |cost: Operand| PlayTerms {
        cost: PlayCost::Alternate(cost),
        ..terms()
    };
    let matching = |cards: ObjectSet, filter: &ObjectFilter| matching(&cards, filter);
    let top = |players: PlayerSet| top_of_libraries(players);
    let of = |player: &PlayerRef| PlayerSet::One(player.clone());
    let graveyard = |players: PlayerSet, cards: &ObjectFilter| {
        zone_set(Z::Graveyard, players, cards.clone())
    };
    let hand = |cards: &ObjectFilter| zone_set(Z::Hand, actor.clone(), cards.clone());
    let single = |card: &ObjectRef| ObjectSet::One(card.clone());
    let cast = |cards, count, play_terms| cast_with(actor, A::CastSpell, cards, count, play_terms, None);
    let play = |cards, count, play_terms| cast_with(actor, A::Either, cards, count, play_terms, None);
    let every = |set: &ObjectSet| Expr::CountObjects(Box::new(set.clone()));
    Some(match (form, operands) {
        ("CastTopCardOfLibraryWithoutPaying", []) => cast(top(actor.clone()), one(), free()),
        ("PlayTopCardOfLibraryWithoutPaying", []) => play(top(actor.clone()), one(), free()),
        ("CastTopSpellOfLibraryWithoutPaying" | "CastASpellFromTopOfLibraryWithoutPaying", [Filter(spells)]) => {
            cast(matching(top(actor.clone()), spells), one(), free())
        }
        ("CastTopCardOfPlayersLibraryWithoutPaying", [Player(player)]) => {
            cast(top(of(player)), one(), free())
        }
        ("CastTopCardOfLibraryForAlternateCost", symbols) => cast(
            top(actor.clone()),
            one(),
            alternate(Operand::Cost(mana_cost(symbols)?)),
        ),
        ("CastASpellFromPlayersGraveyardWithoutPaying", [Filter(spells), Player(owner)]) => {
            cast(graveyard(of(owner), spells), one(), free())
        }
        ("CastASpellFromPlayersGraveyardWithoutPayingIntoExile", [Filter(spells), Player(owner)]) => {
            cast(graveyard(of(owner), spells), one(), free_exile())
        }
        ("CastASpellFromAPlayersGraveyardWithoutPayingIntoExile", [Filter(spells), Players(owners)]) => {
            cast(graveyard(owners.clone(), spells), one(), free_exile())
        }
        ("MayCastASpellFromEachPlayersGraveyardWithoutPayingIntoExile", [Filter(spells), Players(owners)]) => {
            cast(
                graveyard(owners.clone(), spells),
                Count::PerPlayer {
                    players: owners.clone(),
                    each: Box::new(may()),
                },
                free_exile(),
            )
        }
        ("CastAnyNumberOfCardsInPlayersGraveyardWithoutPaying", [Filter(cards), Player(owner)]) => {
            cast(graveyard(of(owner), cards), Count::AnyNumber, free())
        }
        ("CastSpellFromGraveyardWithoutPayingIntoExile", [Filter(spells), Object(card)]) => {
            cast(matching(single(card), spells), one(), free_exile())
        }
        ("MayCastGraveyardCardIntoExileAndMaySpendManaAsThoughAnyType", [Object(card)]) => cast(
            single(card),
            may(),
            PlayTerms {
                into_exile: true,
                ..spend(SpendAs::AnyType)
            },
        ),
        ("MayCastGraveyardCardAndMaySpendManaAsThoughAnyType", [Object(card)]) => {
            cast(single(card), may(), spend(SpendAs::AnyType))
        }
        ("MayCastASpellFromGraveyardIntoExile", [Filter(spells), Player(owner)]) => cast(
            graveyard(of(owner), spells),
            may(),
            PlayTerms {
                into_exile: true,
                ..terms()
            },
        ),
        ("CastASpellFromExileWithEffect", [Filter(spells), Objects(cards), effects]) => cast(
            matching(cards.clone(), spells),
            one(),
            PlayTerms {
                effects: effects_list(effects),
                ..terms()
            },
        ),
        ("CastExiledCardForAlternateCost", [Object(card), cost]) => {
            cast(single(card), one(), alternate(cost.clone()))
        }
        ("CastCopiedCardForAlternateCost", [cost, Objects(copies)]) => {
            cast(copies.clone(), one(), alternate(cost.clone()))
        }
        ("CastCopiedCardForReducedCost", [mana, Objects(copies)]) => cast(
            copies.clone(),
            one(),
            PlayTerms {
                cost: PlayCost::Reduced(mana.clone()),
                ..terms()
            },
        ),
        ("CastExiledCardForReducedCost", [Object(card), reduction]) => cast(
            single(card),
            one(),
            PlayTerms {
                cost: PlayCost::Reduced(reduction.clone()),
                ..terms()
            },
        ),
        ("CastASpellFromHandForAlternateCost", [Filter(spells), cost]) => {
            cast(hand(spells), one(), alternate(cost.clone()))
        }
        ("CastSpellFromHandOrGraveyardAlternateCost", [Filter(spells), cost]) => cast(
            ObjectSet::Union(vec![hand(spells), graveyard(actor.clone(), spells)]),
            one(),
            alternate(cost.clone()),
        ),
        ("CastAnyNumberOfGroupSpellsFromExileWithoutPaying", [Filter(spells), group, Objects(cards)]) => {
            cast_with(
                actor,
                A::CastSpell,
                matching(cards.clone(), spells),
                Count::AnyNumber,
                free(),
                Some(group),
            )
        }
        ("CastAnyNumberOfGroupSpellsFromAmongCopiedCardsWithoutPaying", [Filter(spells), group, Objects(copies)]) => {
            cast_with(
                actor,
                A::CastSpell,
                matching(copies.clone(), spells),
                Count::AnyNumber,
                free(),
                Some(group),
            )
        }
        ("CastSpellsFromExileWithoutPaying", [Filter(spells), Objects(cards)]) => {
            let cards = matching(cards.clone(), spells);
            let count = Count::Exactly(every(&cards));
            cast(cards, count, free())
        }
        ("CastEachCopiedCardWithoutPaying", [Objects(copies)]) => {
            cast(copies.clone(), Count::Exactly(every(copies)), free())
        }
        ("MayCastCopiedCardWithoutPaying", [Objects(copies)]) => {
            cast(copies.clone(), may(), free())
        }
        ("PlayAnExiledCardAndMaySpendManaAsThoughAnyColorToCast", [Objects(cards)]) => {
            play(cards.clone(), one(), spend(SpendAs::AnyColor))
        }
        ("CastAnExiledCardAndMaySpendManaAsThoughAnyTypeToCast", [Objects(cards)]) => {
            cast(cards.clone(), one(), spend(SpendAs::AnyType))
        }
        ("CastASpellAndMaySpendManaAsThoughAnyTypeToCast", [Filter(spells)]) => {
            cast(hand(spells), one(), spend(SpendAs::AnyType))
        }
        ("PlayAnyNumberOfLandsOrCastAnyNumberOfSpellsFromExileWithoutPaying", [Filter(spells), Objects(cards)]) => {
            play(
                ObjectSet::Matching {
                    objects: Box::new(cards.clone()),
                    filter: ObjectFilter::Either(vec![
                        ObjectFilter::CardType(crate::mtg_types::CardType::Land),
                        spells.clone(),
                    ]),
                },
                Count::AnyNumber,
                free(),
            )
        }
        ("PlayALandOrCastASpellFromAmongExiledCardsWithoutPaying", [Objects(lands), Filter(spells), Objects(cards)]) => {
            play(
                ObjectSet::Matching {
                    objects: Box::new(cards.clone()),
                    filter: ObjectFilter::Either(vec![objects_filter(lands), spells.clone()]),
                },
                one(),
                free(),
            )
        }
        ("CastExiledCardWithoutPayingOntoBottomOfLibrary", [Object(card)]) => cast(
            single(card),
            one(),
            PlayTerms {
                then: Some(Destination::OwnersLibraryBottom),
                ..free()
            },
        ),
        ("CastASpellFromExileOntoBottomOfLibrary", [Filter(spells), Objects(cards)]) => cast(
            matching(cards.clone(), spells),
            one(),
            PlayTerms {
                then: Some(Destination::OwnersLibraryBottom),
                ..terms()
            },
        ),
        ("CastTheCardDiscardedThisWayWithoutPaying" | "CastTheCardRevealedThisWayWithoutPaying" | "CastASpellDrawnThisWayWithoutPaying", [Operand::Result(result)]) => {
            cast(ObjectSet::Named(result.clone()), one(), free())
        }
        ("CastCommanderFromCommandZoneWithoutPaying", []) => cast(
            commanders_in(Z::Command, None, actor)?,
            one(),
            free(),
        ),
        ("RevealHandAndPlayerMayCastASpellFromAmongThemWithoutPaying", [Player(player), Filter(spells)]) => {
            cast_with(
                &of(player),
                A::CastSpell,
                hand(spells),
                may(),
                free(),
                None,
            )
        }
        _ => return None,
    })
}

fn random(objects: &ObjectSet, count: i64) -> ObjectSet {
    ObjectSet::DraftRandom {
        objects: Box::new(objects.clone()),
        count: Expr::Constant(count),
    }
}

fn chosen(chooser: &PlayerSet, objects: &ObjectSet, count: Count) -> ObjectSet {
    ObjectSet::DraftChosen {
        chooser: chooser.clone(),
        objects: Box::new(objects.clone()),
        count,
    }
}

fn matching(objects: &ObjectSet, filter: &ObjectFilter) -> ObjectSet {
    ObjectSet::Matching {
        objects: Box::new(objects.clone()),
        filter: filter.clone(),
    }
}

/// What an action does to a set of objects, chosen or at random.
fn reduce_set_action(actor: &PlayerSet, form: &str, operands: &[Operand]) -> Option<Operation> {
    use KeywordAction as K;
    use Operand::{Filter, Number, Object, Objects, Player, Players, Recipients as Hosts};
    let one = || Count::Exactly(Expr::Constant(1));
    let any = || Count::AnyNumber;
    let each = |action, objects: &ObjectSet| Operation::KeywordEach {
        actor: actor.clone(),
        action,
        objects: objects.clone(),
        until: None,
    };
    let pick = |objects: &ObjectSet, count| chosen(actor, objects, count);
    let destroy = |objects, regeneration| Operation::Destroy {
        objects,
        regeneration,
    };
    let tap = |objects: &ObjectSet, tapped| Operation::Tap {
        objects: objects.clone(),
        tapped,
    };
    let sacrifice = |objects, choose| Operation::Sacrifice {
        players: actor.clone(),
        objects,
        choose,
    };
    let me = || match actor {
        PlayerSet::One(player) => Some(player.clone()),
        _ => None,
    };
    let hand = |players: PlayerSet| zone_set(EventZone::Hand, players, ObjectFilter::Any);
    let look = |objects| {
        me().map(|player| Operation::LookAt {
            player,
            objects,
        })
    };
    let attach = |attachments: &ObjectSet, count, hosts, each_host| Operation::AttachEach {
        actor: actor.clone(),
        attachments: attachments.clone(),
        count,
        hosts,
        each_host,
    };
    let objects_of = |set: ObjectSet| Recipients::Objects(set);
    Some(match (form, operands) {
        ("DestroyAPermanentAtRandom", [Objects(set)]) => destroy(random(set, 1), true),
        ("DestroyUptoOnePermanentEachPlayerControls", [Objects(set), Players(players)]) => destroy(
            pick(
                set,
                Count::PerPlayer {
                    players: players.clone(),
                    each: Box::new(Count::UpTo(Expr::Constant(1))),
                },
            ),
            true,
        ),
        ("DestroyEachPermanentNoRegenSubset", [Objects(all), Objects(subset)]) => {
            Operation::Together(vec![
                destroy(matching(all, &objects_filter(subset)), false),
                destroy(
                    ObjectSet::Difference {
                        objects: Box::new(all.clone()),
                        excluding: Box::new(subset.clone()),
                    },
                    true,
                ),
            ])
        }
        ("SimultaneouslyTapEachPermanentAndUntapEachPermanent", [Objects(a), Objects(b)]) => {
            Operation::Together(vec![tap(a, true), tap(b, false)])
        }
        ("PhaseInEachPermanentAndPhaseOutEachPermanent", [Objects(a), Objects(b)]) => {
            Operation::Together(vec![each(K::PhaseIn, a), each(K::PhaseOut, b)])
        }
        (
            "SimultaneouslySacrificePermanentAndPutGraveyardCardOntoBattlefield",
            [Object(a), Object(b), flags],
        ) => Operation::Together(vec![
            sacrifice(ObjectSet::One(a.clone()), None),
            ZoneMove::new(ObjectSet::One(b.clone()), Destination::OwnersBattlefield)
                .entry(flags)
                .operation(actor),
        ]),
        ("SacrificePermanents", sets) => Operation::Together(
            sets.iter()
                .map(|set| match set {
                    Objects(set) => Some(sacrifice(set.clone(), Some(one()))),
                    _ => None,
                })
                .collect::<Option<_>>()?,
        ),
        ("TapAllButNumberPermanents", [Number(n), Objects(set)]) => tap(
            &ObjectSet::Difference {
                objects: Box::new(set.clone()),
                excluding: Box::new(pick(set, Count::Exactly(n.clone()))),
            },
            true,
        ),
        ("SacrificeAPermanentOfAPlayersChoice", [Objects(set), Players(players)]) => {
            // Only a permanent the player controls can be sacrificed (CR 701.21a):
            // the choice is among those.
            let own = matching(set, &ObjectFilter::ControlledBy(actor.clone()));
            sacrifice(chosen(players, &own, one()), None)
        }
        ("SacrificeHalfOfThePermanentsRoundedUp", [Objects(set)]) => sacrifice(
            set.clone(),
            Some(Count::Exactly(Expr::Divide {
                numerator: Box::new(Expr::CountObjects(Box::new(set.clone()))),
                denominator: Box::new(Expr::Constant(2)),
                rounding: Rounding::Up,
            })),
        ),
        ("EachCreatureConnives", [Objects(set)]) => each(K::Connive, set),
        ("ExploreWithEachPermanent", [Objects(set)]) => each(K::Explore, set),
        ("PrepareEachPermanent", [Objects(set)]) => each(K::Prepare, set),
        ("PhaseInEachPermanent", [Objects(set)]) => each(K::PhaseIn, set),
        ("RemoveEachCreatureFromCombat", [Objects(set)]) => each(K::RemoveFromCombat, set),
        ("UngoadEachCreature", [Objects(set)]) => each(K::Ungoad, set),
        ("TurnEachExiledCardFaceUp", [Objects(set)]) => each(K::TurnFaceUp, set),
        ("TurnEachMorphPermanentFaceDown", [Objects(set)]) => each(K::TurnFaceDown, set),
        ("SetEachCreatureAsBlocked", [Objects(set)]) => each(K::BecomeBlocked, set),
        ("TurnAPermanentFaceUp", [Objects(set)]) => each(K::TurnFaceUp, &pick(set, one())),
        ("TransformAPermanent", [Objects(set)]) => each(K::Transform, &pick(set, one())),
        ("UnlockADoorOfAPermanent", [Objects(set)]) => each(K::UnlockDoor, &pick(set, one())),
        ("TransformAnyNumberOfPermanents", [Objects(set)]) => {
            each(K::Transform, &pick(set, any()))
        }
        ("PhaseOutAnyNumberOfPermanents", [Objects(set)]) => each(K::PhaseOut, &pick(set, any())),
        ("LookAtTheTopCardOfEachPlayersLibrary", [Players(players)]) => {
            look(top_of_libraries(players.clone()))?
        }
        ("LookAtEachFaceDownPermanent", [Objects(set)]) => look(set.clone())?,
        ("LookAtARandomCardInPlayersHand", [Player(player)]) => {
            look(random(&hand(PlayerSet::One(player.clone())), 1))?
        }
        ("RevealTheTopCardOfPlayersLibrary", [Player(player)]) => Operation::Reveal {
            objects: top_of_libraries(PlayerSet::One(player.clone())),
        },
        ("RevealFaceDownPermanent", [Object(object)]) => Operation::Reveal {
            objects: ObjectSet::One(object.clone()),
        },
        ("RevealTheCardPutInHandThisWay", [Operand::Result(result)]) => Operation::Reveal {
            objects: ObjectSet::Named(result.clone()),
        },
        ("RevealUptoNumberCardsOfTypeFromHand", [Number(n), cards]) => Operation::Reveal {
            objects: pick(
                &matching(&hand(actor.clone()), &hand_filter(cards)?),
                Count::UpTo(n.clone()),
            ),
        },
        ("DiscardTheCardRevealedThisWay", [Operand::Result(result)]) => Operation::DiscardObjects {
            objects: ObjectSet::Named(result.clone()),
        },
        ("RevealHandAndDiscardACardOfTypeAtRandom", [Filter(cards)]) => {
            Operation::Together(vec![
                Operation::Reveal {
                    objects: hand(actor.clone()),
                },
                Operation::DiscardObjects {
                    objects: random(&matching(&hand(actor.clone()), cards), 1),
                },
            ])
        }
        ("HaveEachPlayerLoseLife", [Players(players), Number(n)]) => Operation::LoseLife {
            players: players.clone(),
            amount: n.clone(),
        },
        ("AttachPermanentToPlayer", [Object(a), Player(player)]) => attach(
            &ObjectSet::One(a.clone()),
            None,
            Recipients::Players(PlayerSet::One(player.clone())),
            false,
        ),
        ("AttachPermanentsToPermanent", [Objects(a), Object(host)]) => Operation::Attach {
            attachments: a.clone(),
            host: host.clone(),
        },
        ("AttachAnyNumberOfPermanentsToPlayerOrPermanent", [Objects(a), Hosts(hosts)]) => {
            attach(a, Some(any()), hosts.clone(), false)
        }
        ("AttachEachPermanentToAPermanent", [Objects(a), Objects(hosts)]) => {
            attach(a, None, objects_of(pick(hosts, one())), false)
        }
        ("AttachAnyNumberOfPermanentsToAnyPermanents", [Objects(a), Objects(hosts)]) => {
            attach(a, Some(any()), objects_of(hosts.clone()), false)
        }
        ("AttachUptoOnePermanentToEachPermanent", [Objects(a), Objects(hosts)]) => attach(
            a,
            Some(Count::UpTo(Expr::Constant(1))),
            objects_of(hosts.clone()),
            true,
        ),
        ("AttachEachPermanentToAPlayerOfChoice", [Objects(a), Players(players)]) => {
            attach(a, None, Recipients::Players(players.clone()), false)
        }
        ("AttachPermanentToACardInAPlayersGraveyard", [Object(a), Filter(cards), Players(players)]) => {
            attach(
                &ObjectSet::One(a.clone()),
                None,
                objects_of(pick(
                    &zone_set(EventZone::Graveyard, players.clone(), cards.clone()),
                    one(),
                )),
                false,
            )
        }
        ("UnattachEachPermanentFromPermanent", [Objects(a), Object(host)]) => each(
            K::Unattach,
            &matching(
                a,
                &ObjectFilter::AttachedTo(Box::new(ObjectFilter::SameObject(host.clone()))),
            ),
        ),
        ("UnattachEachPermanentFromEachPermanent", [Objects(a), Objects(hosts)]) => each(
            K::Unattach,
            &matching(a, &ObjectFilter::AttachedTo(Box::new(objects_filter(hosts)))),
        ),
        ("ManifestNumberCardsFromHand", [Number(n)]) => Operation::Manifest {
            actor: actor.clone(),
            source: ManifestSource::Cards(pick(&hand(actor.clone()), Count::Exactly(n.clone()))),
            cloak: false,
            entry: crate::movement::entry_from_source(&[]),
        },
        ("ManifestACardFromHand" | "CloakACardFromHand", []) => Operation::Manifest {
            actor: actor.clone(),
            source: ManifestSource::Cards(pick(&hand(actor.clone()), one())),
            cloak: form.starts_with("Cloak"),
            entry: crate::movement::entry_from_source(&[]),
        },
        ("ManifestEachCardOfTypeFromHand", [Filter(cards)]) => Operation::Manifest {
            actor: actor.clone(),
            source: ManifestSource::Cards(matching(&hand(actor.clone()), cards)),
            cloak: false,
            entry: crate::movement::entry_from_source(&[]),
        },
        ("TurnPermanentFaceDownAsArtifactCreature", [Object(object), pt, kind]) => {
            Operation::TurnFaceDownAs {
                objects: ObjectSet::One(object.clone()),
                as_creature: artifact_creature(pt, kind),
            }
        }
        ("TurnEachPermanentFaceDownAsCreature", [Objects(set), pt, kind]) => {
            Operation::TurnFaceDownAs {
                objects: set.clone(),
                as_creature: FaceDownAs {
                    power_toughness: Box::new(pt.clone()),
                    creature_type: Some(Box::new(kind.clone())),
                    artifact: false,
                },
            }
        }
        _ => return None,
    })
}

fn exchange(
    kind: ExchangeKind,
    first: Operand,
    second: Operand,
    until: Option<Duration>,
) -> Operation {
    Operation::Exchange {
        kind,
        first: Box::new(first),
        second: Box::new(second),
        until,
    }
}

/// Two things trade places, as Forge's `ExchangeControl`, `ExchangeLife` and
/// `ExchangeZone`.
fn reduce_exchange(actor: &PlayerSet, form: &str, operands: &[Operand]) -> Option<Operation> {
    use ExchangeKind as E;
    use Operand::{Duration as Until, Object, Objects, Player};
    let you = || Operand::Players(actor.clone());
    let zones = |a, b| exchange(E::Zones(a, b), you(), you(), None);
    Some(match (form, operands) {
        ("ExchangeOwnershipOfTwoCards", [a @ Object(_), b @ Object(_)]) => {
            exchange(E::Ownership, a.clone(), b.clone(), None)
        }
        ("ExchangeLifeTotalWithPermanentsPower", [object @ Object(_)]) => {
            exchange(E::LifeWithPower, you(), object.clone(), None)
        }
        ("ExchangeLifeTotalWithPermanentsToughness", [object @ Object(_)]) => {
            exchange(E::LifeWithToughness, you(), object.clone(), None)
        }
        ("PlayersExchangeLifeTotals", [a @ Player(_), b @ Player(_)]) => {
            exchange(E::LifeTotals, a.clone(), b.clone(), None)
        }
        ("ExchangeControlOfSpellAndPermanent", [a @ Object(_), b @ Object(_)]) => {
            exchange(E::Control, a.clone(), b.clone(), None)
        }
        ("ExchangeControlOfEachPermanentWithPlayer", [a @ Objects(_), b @ Player(_)]) => {
            exchange(E::Control, a.clone(), b.clone(), None)
        }
        (
            "ExchangeControlOfEachPermanentWithPlayerUntil",
            [a @ Objects(_), b @ Player(_), Until(until)],
        ) => exchange(E::Control, a.clone(), b.clone(), Some(until.clone())),
        ("ExchangeTextBoxesOfTwoPermanentsUntil", [a @ Object(_), b @ Object(_), Until(until)]) => {
            exchange(E::TextBoxes, a.clone(), b.clone(), Some(until.clone()))
        }
        ("ExchangePowerOfTwoCreaturesUntil", [a @ Object(_), b @ Object(_), Until(until)]) => {
            exchange(E::Power, a.clone(), b.clone(), Some(until.clone()))
        }
        ("PerpetuallyExchangePowerOfPermanentAndPermanent", [a @ Object(_), b @ Object(_)]) => {
            exchange(E::Power, a.clone(), b.clone(), None)
        }
        ("ExchangeAnteCardWithTopCardOfPlayersLibrary", [a @ Object(_), Player(player)]) => {
            exchange(
                E::Ante,
                a.clone(),
                Objects(top_of_libraries(PlayerSet::One(player.clone()))),
                None,
            )
        }
        ("ExchangeHandAndGraveyard", []) => zones(EventZone::Hand, EventZone::Graveyard),
        ("ExchangeGraveyardAndLibrary", []) => zones(EventZone::Graveyard, EventZone::Library),
        ("ExchangeHandAndLibraryThenShuffle", []) => Operation::Together(vec![
            zones(EventZone::Hand, EventZone::Library),
            Operation::Shuffle {
                players: actor.clone(),
                excluding: None,
            },
        ]),
        _ => return None,
    })
}

/// Spells and abilities acted on where they are, past what `reduce_stack_copy` reads.
fn reduce_stack_action(actor: &PlayerSet, form: &str, operands: &[Operand]) -> Option<Operation> {
    use NewTargets as T;
    use Operand::{Filter, Number, Object, Players};
    let copy = |object: &ObjectRef, times, targets| Operation::CopyOnStack {
        actor: actor.clone(),
        object: object.clone(),
        times,
        targets,
        effects: vec![],
        card: None,
    };
    let once = || Expr::Constant(1);
    let retarget = |object: &ObjectRef, targets| Operation::Retarget {
        actor: actor.clone(),
        object: object.clone(),
        targets,
    };
    let on_stack = |filter: ObjectFilter| ObjectSet::Zone {
        scope: ZoneScope::Shared(EventZone::Stack),
        kind: ObjectKind::Spell,
        filter,
    };
    let take = |object: &ObjectRef, targets| Operation::TakeSpell {
        actor: actor.clone(),
        object: object.clone(),
        targets,
    };
    let except = |term: &Operand| T::Random(Some(Box::new(term.clone())));
    Some(match (form, operands) {
        ("CopyEachSpellAndMayChooseNewTargets", [Filter(spells)]) => Operation::CopyEach {
            actor: actor.clone(),
            objects: ObjectFilter::All(vec![ObjectFilter::OnStack(StackTrait::Spell), spells.clone()]),
            targets: T::May,
        },
        ("CopyEachAbilityAndMayChooseNewTargets", [term]) => Operation::CopyEach {
            actor: actor.clone(),
            objects: stack_filter(term)?,
            targets: T::May,
        },
        ("CopyActivatedAbilityAndMayChooseNewTargets", [Object(o)]) => copy(o, once(), T::May),
        ("CopySpellOrAbilityNumberTimesAndMayChooseNewTargets", [Object(o), Number(n)]) => {
            copy(o, n.clone(), T::May)
        }
        ("CopySpellAndRandomlyChooseNewTargetsExceptFor", [Object(o), term]) => {
            copy(o, once(), except(term))
        }
        (
            "CopySpellForEachSpellPermanentCardAndOrPlayerItCouldTarget"
            | "CopySpellOrAbilityForEachPermanentOrPlayerItCouldTarget"
            | "CopySpellForEachOtherPermanentOrPlayerAndMustChooseThemAsNewTarget",
            [Object(o)],
        ) => copy(o, once(), T::EachLegal),
        (
            "CopySpellForEachPlayerAndMustChooseThemOrAPermanentTheyControlAsNewTarget",
            [Object(o), Players(players)],
        ) => copy(o, once(), T::EachPlayer(players.clone())),
        ("GainControlOfSpellAndMayChooseNewTargets", [Object(o)]) => take(o, T::May),
        ("GainControlOfSpellAndRandomlyChooseNewTargetsExceptFor", [Object(o), term]) => {
            take(o, except(term))
        }
        ("ReselectTargetOfSpellOrAbilityAtRandom", [Object(o)]) => retarget(o, T::Random(None)),
        ("ChangeATargetOfSpellOrAbilityToPermanent", [Object(o), Object(to)]) => {
            retarget(o, T::To(ObjectSet::One(to.clone())))
        }
        ("ChooseNewTargetsForAnyNumberOfSpellsOrAbilities", [term]) => Operation::RetargetEach {
            actor: actor.clone(),
            objects: stack_filter(term)?,
            targets: T::May,
        },
        ("CounterEachSpellAndAbility", [term]) => Operation::Counter {
            objects: ObjectSet::Zone {
                scope: ZoneScope::Shared(EventZone::Stack),
                kind: ObjectKind::Any,
                filter: stack_filter(term)?,
            },
            destination: Destination::OwnersGraveyard,
        },
        ("CounterEachAbility", [term]) => Operation::Counter {
            objects: on_stack(stack_filter(term)?),
            destination: Destination::OwnersGraveyard,
        },
        ("CounterSpellIntoTopOrBottomOfLibrary", [Object(o)]) => Operation::Counter {
            objects: ObjectSet::One(o.clone()),
            destination: Destination::OwnersLibraryTopOrBottom,
        },
        ("SetValueXOfSpellOrAbility", [Object(o), Number(n)]) => Operation::SetX {
            object: o.clone(),
            value: n.clone(),
        },
        ("DoubleXValueOfSpell", [Object(o)]) => Operation::SetX {
            object: o.clone(),
            value: Expr::Multiply(Box::new(Expr::CastX(o.clone())), Box::new(Expr::Constant(2))),
        },
        _ => return None,
    })
}

fn ints(operands: &[Operand]) -> Option<Vec<Operand>> {
    operands
        .iter()
        .map(|operand| matches!(operand, Operand::Integer(_)).then(|| operand.clone()))
        .collect()
}

fn bound(comparison: Comparison, value: i64) -> Operand {
    Operand::Comparison(comparison, Expr::Constant(value))
}

/// A value chosen under terms: from what, not what, in secret or not twice.
fn reduce_choice(actor: &PlayerSet, form: &str, operands: &[Operand]) -> Option<Operation> {
    use Comparison::{Greater, GreaterOrEqual, LessOrEqual};
    use Operand::{Filter, Integer, List, Number, Objects, Player, Players, Program as Actions};
    use ValueKind as V;
    let terms = ChoiceTerms::default;
    let with = |kind, terms| Operation::ChooseValue {
        actor: actor.clone(),
        kind,
        terms,
    };
    let secret = |options: Vec<Operand>| ChoiceTerms {
        options,
        secret: true,
        ..terms()
    };
    let unchosen = |options: Vec<Operand>| ChoiceTerms {
        options,
        unchosen: true,
        ..terms()
    };
    let random = |kind, options: Vec<Operand>| Operation::ChooseAtRandom { kind, options };
    Some(match (form, operands) {
        ("ChooseACreatureTypeOtherThan", [other]) => with(
            V::CreatureType,
            ChoiceTerms {
                except: vec![other.clone()],
                ..terms()
            },
        ),
        ("ChooseACreatureType_And_ChooseACreatureTypeOtherThan", [other]) => with(
            V::CreatureType,
            ChoiceTerms {
                count: ChoiceCount::Exactly(Expr::Constant(2)),
                except: vec![other.clone()],
                ..terms()
            },
        ),
        ("ChooseANumberGreaterThanNumber", [Integer(n)]) => with(
            V::Number,
            ChoiceTerms {
                options: vec![bound(Greater, *n)],
                ..terms()
            },
        ),
        ("SecretlyChooseANumberGreaterThanNumber", [Integer(n)]) => {
            with(V::Number, secret(vec![bound(Greater, *n)]))
        }
        ("SecretlyChooseANumberGreaterThanOrEqualToNumber", [Integer(n)]) => {
            with(V::Number, secret(vec![bound(GreaterOrEqual, *n)]))
        }
        ("SecretlyChooseANumberBetween", [Integer(a), Integer(b)]) => with(
            V::Number,
            secret(vec![bound(GreaterOrEqual, *a), bound(LessOrEqual, *b)]),
        ),
        ("SecretlyChooseANumberBetweenThatHasntBeenChosen", [Integer(a), Integer(b)]) => with(
            V::Number,
            ChoiceTerms {
                unchosen: true,
                ..secret(vec![bound(GreaterOrEqual, *a), bound(LessOrEqual, *b)])
            },
        ),
        ("SecretlyChooseANumber", []) => with(V::Number, secret(vec![])),
        ("SecretlyChooseAPlayer", [players @ Players(_)]) => {
            with(V::Player, secret(vec![players.clone()]))
        }
        ("SecretlyChooseACreatureTypeFromList", kinds) => {
            with(V::CreatureType, secret(kinds.to_vec()))
        }
        ("SecretlyChooseAPermanent", [set @ Objects(_)]) => {
            with(V::Object, secret(vec![set.clone()]))
        }
        ("SecretlyChooseAGraveyardCard", [cards @ Filter(_)]) => {
            with(V::Object, secret(vec![cards.clone()]))
        }
        ("SecretlyChooseACardInHand", [cards]) => with(
            V::Object,
            secret(vec![Filter(hand_filter(cards)?), Players(actor.clone())]),
        ),
        ("ChooseAPermanentThatHasntBeenChosen", [set @ Objects(_)]) => {
            with(V::Object, unchosen(vec![set.clone()]))
        }
        ("ChooseAGraveyardCardThatHasntBeenChosen", [cards @ Filter(_)]) => {
            with(V::Object, unchosen(vec![cards.clone()]))
        }
        ("ChooseAnUnchosenCardInPlayersGraveyard", [Filter(cards), Player(player)]) => with(
            V::Object,
            unchosen(vec![Objects(zone_set(
                EventZone::Graveyard,
                PlayerSet::One(player.clone()),
                cards.clone(),
            ))]),
        ),
        ("ChooseACardNameThatHasntBeenChosen", [names]) => {
            with(V::CardName, unchosen(vec![names.clone()]))
        }
        ("NoteAnUnnotedCreatureType", [kinds]) => with(V::CreatureType, unchosen(vec![kinds.clone()])),
        ("ChooseAPermanentType", []) => with(V::PermanentType, terms()),
        ("ChooseANonBasicLandType", []) => with(V::NonbasicLandType, terms()),
        ("ChooseALetter", []) => with(V::Letter, terms()),
        ("ChooseAnOrderForCardTypes", kinds) => with(
            V::CardType,
            ChoiceTerms {
                options: kinds.to_vec(),
                count: ChoiceCount::Exactly(Expr::Constant(kinds.len() as i64)),
                ordered: true,
                ..terms()
            },
        ),
        ("ForEachPlayerChooseAWord", [Players(players), List(words)]) => Operation::ChooseValue {
            actor: players.clone(),
            kind: V::Word,
            terms: ChoiceTerms::ordinary(1, words.clone()),
        },
        ("ChooseANumberBetweenAtRandom", [Number(a), Number(b)]) => random(
            V::Number,
            vec![
                Operand::Comparison(GreaterOrEqual, a.clone()),
                Operand::Comparison(LessOrEqual, b.clone()),
            ],
        ),
        ("ChooseANumberFromAmongAtRandom", values) => random(V::Number, ints(values)?),
        ("ChooseAnAbilityAtRandom", abilities) => random(V::Ability, abilities.to_vec()),
        ("ChooseARandomColor", [colors]) => random(V::Color, vec![colors.clone()]),
        ("ChooseNumberAbilitiesAtRandom", [Number(n), List(abilities)]) => with(
            V::Ability,
            ChoiceTerms {
                options: abilities.clone(),
                count: ChoiceCount::Exactly(n.clone()),
                random: true,
                ..terms()
            },
        ),
        ("ChooseRandomColorPermanentDoesntHaveProtectionFrom", [object]) => with(
            V::Color,
            ChoiceTerms {
                except: vec![object.clone()],
                random: true,
                ..terms()
            },
        ),
        ("ChooseActionAtRandom", [List(actions)]) => {
            if !actions.iter().all(|action| matches!(action, Actions(_))) {
                return None;
            }
            random(V::Action, actions.clone())
        }
        ("RevealSecretlyChosenNumbers" | "RevealTheSecretlyChosenNumber", []) => {
            Operation::RevealChoices { kind: V::Number }
        }
        ("RevealSecretlyChosenPermanents" | "RevealTheSecretlyChosenCard", []) => {
            Operation::RevealChoices { kind: V::Object }
        }
        ("RevealVotesForWord", []) => Operation::RevealChoices { kind: V::Word },
        ("RevealVotesForPlayer", []) => Operation::RevealChoices { kind: V::Player },
        ("RevealVotesForPermanent", []) => Operation::RevealChoices { kind: V::Object },
        _ => return None,
    })
}

/// Coins flipped until something stops them.
fn reduce_coins(actor: &PlayerSet, form: &str, operands: &[Operand]) -> Option<Operation> {
    use Operand::{Number, Objects};
    let flips = |times, each, until_lost, may_stop, called| Operation::CoinFlips {
        actor: actor.clone(),
        times,
        each,
        until_lost,
        may_stop,
        called,
    };
    Some(match (form, operands) {
        ("FlipACoinUntilLose", []) => flips(None, None, true, false, false),
        ("FlipACoinUntilLoseOrStop", []) => flips(None, None, true, true, false),
        ("FlipACoinNumberTimesOrUntilLose", [Number(n)]) => {
            flips(Some(n.clone()), None, true, false, false)
        }
        ("FlipACoinAndCallIt", []) => flips(Some(Expr::Constant(1)), None, false, false, true),
        ("FlipACoinForEachPermanent", [Objects(set)]) => {
            flips(None, Some(set.clone()), false, false, false)
        }
        _ => return None,
    })
}

/// Declared attackers and blockers changed, as Forge's `ChangeCombatants`.
fn reduce_combat_change(form: &str, operands: &[Operand]) -> Option<Operation> {
    use CombatChange as C;
    use Operand::{Object, Objects};
    let one = |object: &ObjectRef| ObjectSet::One(object.clone());
    let change = match (form, operands) {
        ("ReselectWhichPlayerOrPermanentCreatureIsAttacking", [Object(o)]) => C::Reselect {
            objects: one(o),
            players_only: false,
            optional: false,
        },
        ("ReselectWhichPlayerCreatureIsAttacking", [Object(o)]) => C::Reselect {
            objects: one(o),
            players_only: true,
            optional: false,
        },
        ("MayReselectWhichPlayerOrPermanentEachCreatureIsAttacking", [Objects(set)]) => {
            C::Reselect {
                objects: set.clone(),
                players_only: false,
                optional: true,
            }
        }
        ("SetAttackAssignmentOfCreature", [Object(o), to]) => C::AssignAttack {
            objects: one(o),
            to: Box::new(to.clone()),
        },
        ("SetAttackAssignmentOfCreatures", [Objects(set), to]) => C::AssignAttack {
            objects: set.clone(),
            to: Box::new(to.clone()),
        },
        ("ChooseAnAttackingCreatureForBlockerToBlock", [Objects(attackers), Object(blocker)]) => {
            C::Block {
                blocker: blocker.clone(),
                attackers: attackers.clone(),
            }
        }
        ("TrySwappingBlockAssignmentsOfTwoAttackingCreatures", [Object(a), Object(b)]) => {
            C::SwapBlocks {
                first: a.clone(),
                second: b.clone(),
                attacking: true,
            }
        }
        ("TrySwappingBlockAssignmentsOfTwoBlockingCreatures", [Object(a), Object(b)]) => {
            C::SwapBlocks {
                first: a.clone(),
                second: b.clone(),
                attacking: false,
            }
        }
        ("OnlyAllowedAttackersUntilEndOfTurn", [Objects(set)]) => C::OnlyAttackers {
            objects: set.clone(),
            during: None,
        },
        (
            "OnlyAllowedAttackersDuringTheAdditionalCombatStepAddedThisWay",
            [Objects(set), Operand::Result(result)],
        ) => C::OnlyAttackers {
            objects: set.clone(),
            during: Some(result.clone()),
        },
        ("OnlyAllowedBlockersUntilEndOfTurn", [Objects(set)]) => C::OnlyBlockers(set.clone()),
        _ => return None,
    };
    Some(Operation::ChangeCombat { change })
}

/// Who does a list of actions, and who decides whether they happen.
fn reduce_player_does(actor: &PlayerSet, form: &str, operands: &[Operand]) -> Option<Operation> {
    use Operand::{List, Number, Player, Players, Program as Actions};
    let does = |players: PlayerSet, any_one, optional, body: Vec<Operand>, otherwise: Vec<Operand>| {
        Operation::PlayersDo {
            players,
            any_one,
            optional,
            once_each_turn: false,
            body,
            otherwise,
        }
    };
    let you = || actor.clone();
    let one = |player: &PlayerRef| PlayerSet::One(player.clone());
    Some(match (form, operands) {
        ("MayHavePlayerAction", [Player(player), body @ Actions(_)]) => {
            does(one(player), false, true, vec![body.clone()], vec![])
        }
        ("IfHavePlayerAction", [Player(player), body @ Actions(_), List(then)]) => {
            does(one(player), false, true, vec![body.clone()], then.clone())
        }
        ("APlayerMayAction", [Players(players), body @ Actions(_)]) => {
            does(players.clone(), true, true, vec![body.clone()], vec![])
        }
        ("APlayerActions", [Players(players), List(body)]) => {
            does(players.clone(), true, false, body.clone(), vec![])
        }
        ("ActionForEachPlayerInTurnOrder", [Players(players), body @ Actions(_)]) => {
            does(players.clone(), false, false, vec![body.clone()], vec![])
        }
        ("MayActionOnceEachTurn", [body @ Actions(_)]) => Operation::PlayersDo {
            players: you(),
            any_one: false,
            optional: true,
            once_each_turn: true,
            body: vec![body.clone()],
            otherwise: vec![],
        },
        ("GetOneTimeBoon", [Operand::Event(event), body @ Actions(_)]) => Operation::CreateBoon {
            actor: you(),
            event: event.clone(),
            condition: None,
            times: Expr::Constant(1),
            body: Box::new(body.clone()),
        },
        (
            "GetOneTimeConditionalBoon",
            [Operand::Event(event), Operand::Condition(condition), body @ Actions(_)],
        ) => Operation::CreateBoon {
            actor: you(),
            event: event.clone(),
            condition: Some(Box::new(condition.clone())),
            times: Expr::Constant(1),
            body: Box::new(body.clone()),
        },
        ("GetANumberTimeBoon", [Number(n), Operand::Event(event), body @ Actions(_)]) => {
            Operation::CreateBoon {
                actor: you(),
                event: event.clone(),
                condition: None,
                times: n.clone(),
                body: Box::new(body.clone()),
            }
        }
        ("ThereIsAnAdditionalCombatPhaseWithTrigger", [body @ Actions(_)]) => Operation::AddPhases {
            actor: you(),
            phases: AddedPhases::Combat,
            count: Expr::Constant(1),
            at_start: Some(Box::new(body.clone())),
        },
        ("AfterTheSecondMainPhaseThisTurnThereIsAnAdditionalCombatPhaseAndAnAdditionalMainPhaseWithAtTheBeginningOfCombatTrigger", [body @ Actions(_)]) => {
            Operation::AddPhases {
                actor: you(),
                phases: AddedPhases::CombatAndMain,
                count: Expr::Constant(1),
                at_start: Some(Box::new(body.clone())),
            }
        }
        ("AfterThisMainPhaseThereAreAnAdditionalNumberCombatPhases", [Number(n)]) => {
            Operation::AddPhases {
                actor: you(),
                phases: AddedPhases::Combat,
                count: n.clone(),
                at_start: None,
            }
        }
        ("ThereIsAnAdditionalUpkeepStep", []) => Operation::AddPhases {
            actor: you(),
            phases: AddedPhases::Upkeep,
            count: Expr::Constant(1),
            at_start: None,
        },
        ("ThereAreNumberAdditionalUpkeepSteps", [Number(n)]) => Operation::AddPhases {
            actor: you(),
            phases: AddedPhases::Upkeep,
            count: n.clone(),
            at_start: None,
        },
        ("ThereIsAnAdditionalEndStep", []) => Operation::AddPhases {
            actor: you(),
            phases: AddedPhases::End,
            count: Expr::Constant(1),
            at_start: None,
        },
        ("ChooseAnyNumberPermanentsAndPayManaForEach", [Operand::Objects(objects), List(mana)]) => {
            Operation::ChooseAndPay {
                actor: you(),
                objects: objects.clone(),
                cost: mana_cost(mana)?,
            }
        }
        ("IncubateNumberTimes", [Number(amount), Number(times)]) => Operation::Repeat {
            times: times.clone(),
            body: Box::new(Operation::Keyword {
                actor: you(),
                action: KeywordAction::Incubate,
                object: None,
                other: None,
                amount: Some(amount.clone()),
                subtype: None,
            }),
        },
        _ => return None,
    })
}
