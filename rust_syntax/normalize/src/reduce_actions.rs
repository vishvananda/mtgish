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
        Operation::ChangeZone {
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
        PlayerSet::One(player) => player.clone(),
        _ => PlayerRef::AbilityController,
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
            commanders_in(Z::Command, Some(commanders), actor)?,
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
        ("PutACardFromOutsideGameInHand", [_]) => {
            ZoneMove::new(ObjectSet::OutsideTheGame(any.clone()), D::OwnersHand).one()
        }
        ("ShuffleUptoNumberCardsFromOutsideTheGameIntoLibrary", [Number(n)]) => ZoneMove::new(
            ObjectSet::OutsideTheGame(any.clone()),
            D::OwnersLibraryShuffled,
        )
        .choose(up_to(n)),
        ("PutTheTopNumberCardsOfLibraryInHand", [Number(n)]) => {
            ZoneMove::new(library_top(&you(), n.clone()), D::OwnersHand)
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
            ZoneMove::new(ObjectSet::One(ObjectRef::EventSubject), D::OwnersLibraryTop)
        }
        ("PutDeadPermanentOnBottomOfLibrary", []) => {
            ZoneMove::new(ObjectSet::One(ObjectRef::EventSubject), D::OwnersLibraryBottom)
        }
        ("PutDeadPermanentOnTopOfLibraryOrOnBottomOfLibrary", []) => ZoneMove::new(
            ObjectSet::One(ObjectRef::EventSubject),
            D::OwnersLibraryTopOrBottom,
        ),
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
            "PutTheTopNumberCardsOfPlayersLibraryOntoBattlefieldAsFaceDownArtifactCreatures",
            [Number(n), pt, kind],
        ) => ZoneMove::new(library_top(&you(), n.clone()), D::OwnersBattlefield)
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
        ("MayPutACardFromHandOrGraveyardOnBattlefieldForEachPermanent", [Filter(cards), flags, Objects(each)]) => {
            ZoneMove::new(
                ObjectSet::Union(vec![hand(cards), graveyard(cards)]),
                D::OwnersBattlefield,
            )
            .choose(Count::UpTo(Expr::CountObjects(Box::new(each.clone()))))
            .entry(flags)
        }
        ("PutUptoNumberCardsFromHandAndOrGraveyardOnBattlefield", [Number(n), Filter(cards), Player(owner), flags]) => {
            ZoneMove::new(
                ObjectSet::Union(vec![hand(cards), in_graveyard(&of(owner), cards)]),
                D::OwnersBattlefield,
            )
            .choose(up_to(n))
            .entry(flags)
        }
        ("PutACardDiscardedThisWayOntoTheBattlefield", [Filter(cards), flags, Operand::Result(result)]) => {
            ZoneMove::new(
                ObjectSet::Matching {
                    objects: Box::new(ObjectSet::Named(result.clone())),
                    filter: cards.clone(),
                },
                D::OwnersBattlefield,
            )
            .one()
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
    let matching = |cards: ObjectSet, filter: &ObjectFilter| ObjectSet::Matching {
        objects: Box::new(cards),
        filter: filter.clone(),
    };
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
