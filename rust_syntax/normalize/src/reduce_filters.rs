// What a filter asks about an object, and who a condition asks about a player.

fn draft_note(kind: DraftNoteKind, player: &PlayerRef, card: &str) -> DraftNote {
    DraftNote {
        kind,
        player: player.clone(),
        card: card.to_string(),
    }
}

/// The filter a type variable reads, when it is not the one chosen here.
fn type_variable_filter(variable: &Operand) -> Option<ObjectFilter> {
    let shares = |property, with: &ObjectRef| ObjectFilter::Shares {
        property,
        with: with.clone(),
    };
    match variable {
        Operand::Filter(ObjectFilter::Primitive { name, operands }) => {
            match (name.as_str(), operands.as_slice()) {
                (
                    "CreatureTypeVariable::TheCreatureTypeChosenByPlayerThisWay",
                    [Operand::Player(player)],
                ) => Some(ObjectFilter::ChosenBy {
                    kind: ValueKind::CreatureType,
                    player: player.clone(),
                }),
                _ => None,
            }
        }
        Operand::Term { form, operands, .. } => match (form.as_str(), operands.as_slice()) {
            ("TheNotedCreatureType", []) => Some(ObjectFilter::Noted(ValueKind::CreatureType)),
            ("AnyLandTypeOfPermanent", [Operand::Object(object)]) => {
                Some(shares(DistinctProperty::LandTypes, object))
            }
            ("CardtypeOfExiled", [Operand::Object(object)]) => {
                Some(shares(DistinctProperty::CardTypes, object))
            }
            _ => None,
        },
        _ => None,
    }
}

fn card_types(operands: &[Operand]) -> Option<Vec<crate::mtg_types::CardType>> {
    operands.iter().map(vocabulary_value).collect()
}

/// What a filter asks that names an earlier object or a noted value.
fn reduce_object_trait(form: &str, operands: &[Operand]) -> Option<ObjectFilter> {
    use DistinctProperty as D;
    let scoped = || ObjectRef::Scoped;
    let shares = |property, with| ObjectFilter::Shares { property, with };
    let condition = |condition| Some(ObjectFilter::Condition(Box::new(condition)));
    Some(match (form, operands) {
        (_, [variable]) if form.ends_with("TypeVariable") || form == "IsCardtypeVariable" => {
            return type_variable_filter(variable)
        }
        ("PlayerRevealedACardAsCast", _)
        | ("PlayerControlledAPermanentAsCast", _)
        | ("PlayerChoseAPermanentAsCast", _) => return condition(as_cast(form, operands)?),
        ("PlayerHasCastAnotherSpellThisTurn", [Operand::Player(player), Operand::Filter(spells)]) => {
            let other = ObjectFilter::Not(Box::new(ObjectFilter::SameObject(ObjectRef::Source)));
            return condition(did(
                player,
                PlayerHistory::CastSpell(Box::new(ObjectFilter::All(vec![
                    spells.clone(),
                    other,
                ]))),
                ObjectWindow::ThisTurn,
            ));
        }
        ("WasExiledByPlayerForDraftCard", [Operand::Player(player), Operand::Text(card)]) => {
            ObjectFilter::DraftNoted(draft_note(DraftNoteKind::Exiled, player, card))
        }
        (
            "IsCreatureTypePlayerNotedForCardDuringDraft",
            [Operand::Player(player), Operand::Text(card)],
        ) => ObjectFilter::DraftNoted(draft_note(DraftNoteKind::CreatureType, player, card)),
        (
            "SharesAColorWithColorChosenByPlayerDuringDraft",
            [Operand::Player(player), Operand::Text(card)],
        ) => ObjectFilter::DraftNoted(draft_note(DraftNoteKind::Color, player, card)),
        (
            "TheNamePlayerNotedForCardDuringDraft",
            [Operand::Player(player), Operand::Text(card)],
        ) => ObjectFilter::DraftNoted(draft_note(DraftNoteKind::Name, player, card)),
        ("ANameChosenByPermanent", [Operand::Object(object)]) => ObjectFilter::ChosenOn {
            kind: ValueKind::CardName,
            object: object.clone(),
        },
        ("SharesACardtypeWithTheSacrificedPermanent", []) => shares(
            D::CardTypes,
            earlier_object(EarlierObjects::Sacrificed),
        ),
        ("SharesACardtypeWithTheCycledCard" | "SharesACardtypeWithEachableExiledPermanent", []) => {
            shares(D::CardTypes, scoped())
        }
        ("SharesACardtypeWithEachableExiledPermanent", [Operand::Object(with)]) => {
            shares(D::CardTypes, with.clone())
        }
        ("SharesAColorWithIt", []) => ObjectFilter::SharesColorWith(scoped()),
        ("SharesANameWithThePlayedCard", []) => {
            ObjectFilter::SharesNameWith(ObjectRef::EventSubject)
        }
        ("SharesCardtypeWithPermanent", [kind, Operand::Object(with)]) => {
            ObjectFilter::SharesTypeFrom {
                types: vec![vocabulary_value(kind)?],
                with: with.clone(),
            }
        }
        (
            "SharesACardtypeWithPermanentFromAmongCardtypes",
            [Operand::Object(with), Operand::List(kinds)],
        ) => ObjectFilter::SharesTypeFrom {
            types: card_types(kinds)?,
            with: with.clone(),
        },
        (
            "SharesAColorWithhTheMostCommonColorOrAColorTiedForMostCommonColorAmongPermanents",
            [Operand::Objects(objects)],
        ) => ObjectFilter::SharesMostCommonColor(Box::new(objects.clone())),
        ("InTheChosenPiles" | "InThePermanentPileChosenThisWay", []) => ObjectFilter::InPile(None),
        ("InThePileChosenForPermanent", [Operand::Object(object)]) => {
            ObjectFilter::InPile(Some(object.clone()))
        }
        ("InTheChosenSector", []) => ObjectFilter::InSector(None),
        ("SharesASectorWithPermanent", [Operand::Object(object)]) => {
            ObjectFilter::InSector(Some(object.clone()))
        }
        ("IsTheFirstChosenPermanentFilter", []) => ObjectFilter::NthChosen(0),
        ("IsTheSecondChosenPermanentFilter", []) => ObjectFilter::NthChosen(1),
        ("IsTheThirdChosenPermanentFilter", []) => ObjectFilter::NthChosen(2),
        ("CoinCameUpTails", []) => {
            return condition(Condition::CoinCameUp {
                player: None,
                outcome: CoinOutcome::Tails,
            })
        }
        ("AttachedToAPermanent", [Operand::Objects(objects)]) => {
            ObjectFilter::AttachedTo(Box::new(ObjectFilter::InSet(Box::new(objects.clone()))))
        }
        ("BlockedDeadAttacker", []) => ObjectFilter::BlockingObject(ObjectRef::EventSubject),
        ("WasBlockingDeadDefender", []) => ObjectFilter::BlockedByObject(ObjectRef::EventSubject),
        // "One of those creatures": other creatures you control the spell could target.
        ("CouldBeTargetedBySpell_ThoseTargets", []) => ObjectFilter::All(vec![
            ObjectFilter::TargetableBy(ObjectRef::EventSubject),
            ObjectFilter::Not(Box::new(ObjectFilter::SameObject(ObjectRef::Source))),
            ObjectFilter::ControlledBy(PlayerSet::One(PlayerRef::AbilityController)),
        ]),
        ("CanBeEnchantedByDeadGuest", []) => {
            ObjectFilter::CouldEnchant(Box::new(ObjectSet::One(ObjectRef::Source)))
        }
        ("IsAttackingTheSamePlayerOrPlaneswalkerAsCreature", [Operand::Object(object)]) => {
            ObjectFilter::AttackingSame(object.clone())
        }
        ("NotPutOntoBattlefieldByAbility", [_]) => {
            ObjectFilter::Not(Box::new(ObjectFilter::MadeBy(ObjectRef::Source)))
        }
        ("PutOntoBattlefieldByScheme", [_]) => ObjectFilter::MadeBy(ObjectRef::Source),
        ("WasExiledWithPlane", [_]) => ObjectFilter::ExiledWith(ObjectRef::Source),
        ("WasExiledThisWay", []) => ObjectFilter::InSet(Box::new(ObjectSet::Earlier {
            what: EarlierObjects::Exiled,
            by: None,
        })),
        ("WasExiledByPlayerThisWay", [Operand::Player(_), Operand::Result(result)]) => {
            ObjectFilter::InResult(result.clone())
        }
        ("RevealedFromLibraryThisWay", [Operand::Result(result)])
        | ("RevealedFromPlayersLibraryThisWay", [Operand::Player(_), Operand::Result(result)]) => {
            ObjectFilter::InResult(result.clone())
        }
        // Those the instruction before gave its abilities to, the event being over.
        ("Trigger_ThoseCreatures", [Operand::Result(result)]) => {
            ObjectFilter::InSet(Box::new(ObjectSet::Affected(result.clone())))
        }
        // Orvar's intervening "if": the other permanents you control it targets.
        ("TargetsAPermanent_ThosePermanents", []) => ObjectFilter::All(vec![
            ObjectFilter::InSet(Box::new(ObjectSet::TargetsOf(ObjectRef::EventSubject))),
            ObjectFilter::Not(Box::new(ObjectFilter::SameObject(ObjectRef::Source))),
            ObjectFilter::ControlledBy(PlayerSet::One(PlayerRef::AbilityController)),
        ]),
        ("Trigger_ThoseCreatures", []) | ("Trigger_ThoseGraveyardCards", []) => {
            ObjectFilter::InSet(Box::new(ObjectSet::Scoped))
        }
        ("SneakCostWasPaidThisTurn", []) => {
            ObjectFilter::State(ObjectState::CostPaid(CastCost::Sneak))
        }
        ("WasCastFromAmongCardsInExile", [Operand::Objects(objects)]) => {
            ObjectFilter::CastFromAmong(Box::new(objects.clone()))
        }
        ("TargetsOnlyASinglePermanentOrPlayer", []) => {
            ObjectFilter::OnStack(StackTrait::SingleTarget)
        }
        ("ManaAbility", []) => ObjectFilter::HasActivatedAbility(ActivationFilter::Mana),
        ("NonManaAbility", []) => ObjectFilter::HasActivatedAbility(ActivationFilter::Not(
            Box::new(ActivationFilter::Mana),
        )),
        ("TheChosenAbility", []) => ObjectFilter::Chosen(ValueKind::Ability),
        _ => return None,
    })
}

/// What a player did as this spell was cast, as Forge's `Presented` and
/// `RevealedOrControlled` conditions.
fn as_cast(form: &str, operands: &[Operand]) -> Option<Condition> {
    let (player, action) = match (form, operands) {
        ("PlayerRevealedACardAsCast", [Operand::Player(player), Operand::Filter(cards)]) => {
            (player, AsCastAction::Revealed(cards.clone()))
        }
        ("PlayerControlledAPermanentAsCast", [Operand::Player(player), Operand::Objects(objects)]) => {
            (player, AsCastAction::Controlled(objects.clone()))
        }
        ("PlayerChoseAPermanentAsCast", [Operand::Player(player), Operand::Objects(objects)]) => {
            (player, AsCastAction::Chose(objects.clone()))
        }
        _ => return None,
    };
    Some(Condition::AsCast {
        player: player.clone(),
        action,
    })
}

/// Facts about a player a `Players` row asks, past what `player_check` knows.
fn player_fact(form: &str, player: &PlayerRef, rest: &[Operand]) -> Option<Condition> {
    let member = |players: &PlayerSet| Condition::PlayerIn {
        player: player.clone(),
        players: players.clone(),
    };
    let extreme = |what, filter: Option<&ObjectFilter>, highest| Condition::HadExtreme {
        player: player.clone(),
        what,
        filter: filter.cloned(),
        highest,
    };
    let hand = |cards: &Operand| {
        card_term(cards).map(|filter| {
            count_of(ObjectSet::Zone {
                scope: ZoneScope::Players {
                    zone: EventZone::Hand,
                    players: PlayerSet::One(player.clone()),
                },
                kind: ObjectKind::Card,
                filter,
            })
        })
    };
    Some(match (form, rest) {
        ("NumCardsOfTypeInHandIs", [Operand::Comparison(comparison, value), cards]) => {
            compared(hand(cards)?, comparison, value)
        }
        ("HasACardInHand", [cards]) => at_least(hand(cards)?, 1),
        ("ExceptFor", [Operand::Players(players)]) => negated(member(players)),
        ("OneOfTheChosenPlayers" | "TheChosenPlayers", []) => {
            Condition::ValueChosenIs(player.clone())
        }
        ("ChoseHighestNumber", []) => Condition::ChoseExtreme {
            player: player.clone(),
            highest: true,
        },
        ("ChoseLowestNumber", []) => Condition::ChoseExtreme {
            player: player.clone(),
            highest: false,
        },
        ("ExiledACardWithTheHighestManaValue", []) => {
            extreme(EarlierObjects::Exiled, None, true)
        }
        ("DiscardedACardWithTheHighestManaValueAmongCardsDiscardedThisWay", [_]) => {
            extreme(EarlierObjects::Discarded, None, true)
        }
        ("OwnerOfACardWithTheLowestManaValueRevealedThisWay", [Operand::Filter(cards)]) => {
            extreme(EarlierObjects::Revealed, Some(cards), false)
        }
        ("OwnersOfExiledCards", [Operand::Objects(objects)]) => at_least(
            count_of(filtered(
                objects,
                ObjectFilter::OwnedBy(PlayerSet::One(player.clone())),
            )),
            1,
        ),
        ("VotedForTheSameChoiceAsPlayer", [Operand::Player(other)]) => Condition::VotedAs {
            player: player.clone(),
            other: other.clone(),
            same: true,
        },
        ("VotedForADifferentChoiceThanPlayer", [Operand::Player(other)]) => Condition::VotedAs {
            player: player.clone(),
            other: other.clone(),
            same: false,
        },
        ("CoinCameUpTails", []) => Condition::CoinCameUp {
            player: Some(player.clone()),
            outcome: CoinOutcome::Tails,
        },
        (
            "Trigger_ThosePlayers"
            | "Trigger_ThoseDefendingPlayers"
            | "NumPlayersPassFilter_ThosePlayers"
            | "RepeatablePlayers",
            [],
        ) => member(&PlayerSet::Scoped),
        _ => return None,
    })
}
