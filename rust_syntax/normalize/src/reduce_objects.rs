// Object predicates in typed form. Forge writes these as `Valid$` properties
// such as `Card.attackedThisTurn`; here each is a closed vocabulary or a shape
// a consumer can match on, so a row only decodes the source.

fn state(state: ObjectState) -> ObjectFilter {
    ObjectFilter::State(state)
}

fn not(filter: ObjectFilter) -> ObjectFilter {
    ObjectFilter::Not(Box::new(filter))
}

fn at_least_one() -> (Comparison, Expr) {
    (Comparison::GreaterOrEqual, Expr::Constant(1))
}

fn property(property: Property, (comparison, value): (Comparison, Expr)) -> ObjectFilter {
    ObjectFilter::Property {
        property,
        comparison,
        value,
    }
}

fn history(event: ObjectHistory, window: ObjectWindow) -> ObjectFilter {
    ObjectFilter::History { event, window }
}

fn cast_from(zone: EventZone) -> ObjectFilter {
    ObjectFilter::Cast {
        by: None,
        from: Some(ZoneScope::Players {
            zone,
            players: PlayerSet::All,
        }),
    }
}

fn cards_of(zone: EventZone, player: &PlayerRef, filter: &ObjectFilter) -> ObjectSet {
    ObjectSet::Zone {
        scope: ZoneScope::Players {
            zone,
            players: PlayerSet::One(player.clone()),
        },
        kind: ObjectKind::Card,
        filter: filter.clone(),
    }
}

/// A filter that names its own zone as the set of cards it selects.
fn zone_cards(filter: &ObjectFilter) -> Option<ObjectSet> {
    let (scope, rest) = match filter {
        ObjectFilter::InZone(scope) => (scope.clone(), ObjectFilter::Any),
        ObjectFilter::All(items) => {
            let scope = items.iter().find_map(|item| match item {
                ObjectFilter::InZone(scope) => Some(scope.clone()),
                _ => None,
            })?;
            let rest = items
                .iter()
                .filter(|item| !matches!(item, ObjectFilter::InZone(_)))
                .cloned()
                .collect();
            (scope, ObjectFilter::All(rest))
        }
        _ => return None,
    };
    Some(ObjectSet::Zone {
        scope,
        kind: ObjectKind::Card,
        filter: rest,
    })
}

fn shares(property: DistinctProperty, with: &ObjectRef) -> ObjectFilter {
    ObjectFilter::Shares {
        property,
        with: with.clone(),
    }
}

fn shares_any(property: DistinctProperty, with: ObjectSet) -> ObjectFilter {
    ObjectFilter::SharesWithAny {
        property,
        with: Box::new(with),
    }
}

fn exiled_by(player: &PlayerRef, with: Option<&ObjectRef>, this_turn: bool) -> ObjectFilter {
    ObjectFilter::ExiledBy {
        player: player.clone(),
        with: with.cloned(),
        this_turn,
    }
}

fn single_target(targets: Recipients) -> ObjectFilter {
    ObjectFilter::All(vec![
        ObjectFilter::OnStack(StackTrait::SingleTarget),
        ObjectFilter::TargetsAny(Box::new(targets)),
    ])
}

fn creature_types(types: &[crate::mtg_types::CreatureType]) -> ObjectFilter {
    ObjectFilter::Either(
        types
            .iter()
            .map(|ty| ObjectFilter::Subtype(crate::subtype::creature_type(ty)))
            .collect(),
    )
}

fn fieldless_state(form: &str) -> Option<ObjectFilter> {
    use crate::mtg_types::CreatureType as C;
    use CastCost as Cost;
    use ObjectState as St;
    let outlaw = || creature_types(&[C::Assassin, C::Mercenary, C::Pirate, C::Rogue, C::Warlock]);
    Some(match form {
        "IsARingBearer" => state(St::RingBearer),
        "IsHarnessed" => state(St::Harnessed),
        "IsWorthy" => state(St::Worthy),
        "PowerAndToughnessAreEqual" => state(St::PowerEqualsToughness),
        "PowerAndToughnessArentEqual" => not(state(St::PowerEqualsToughness)),
        "PowerIsDifferentFromItsBasePower" => state(St::PowerDiffersFromBase),
        "IsBlockingAlone" => state(St::BlockingAlone),
        "IsntSaddled" => not(state(St::Saddled)),
        "IsNotSuspected" => not(state(St::Suspected)),
        "WasntKicked" => not(state(St::Kicked)),
        "WasKickedTwice" => state(St::KickedTwice),
        "HasAbilities" => not(state(St::NoAbilities)),
        "IsNonOutlaw" => not(outlaw()),
        "IsParty" => creature_types(&[C::Cleric, C::Rogue, C::Warrior, C::Wizard]),
        "IsModal" => state(St::Modal),
        "HasHybridManaInCost" => state(St::HybridInCost),
        "HasPhyrexianInManaCost" => state(St::PhyrexianInCost),
        "HasMoreThanOneOfTheSameManaSymbolInCost" => state(St::RepeatedManaSymbol),
        "DoesntHaveAName" => state(St::Nameless),
        "HasNonBasicLandType" => state(St::NonBasicLandType),
        "IsForetold" | "WasForetold" => state(St::Foretold),
        "IsWarped" => state(St::Warped),
        "WasPrepared" => state(St::Prepared),
        "ItEscaped" => state(St::Escaped),
        "WasUnearthed" => state(St::Unearthed),
        "WasEmbalmed" => state(St::Embalmed),
        "DevouredACreature" => state(St::DevouredACreature),
        "StartedThisTurnUntapped" => state(St::StartedTurnUntapped),
        "ControlledSinceBeforeCombatThisTurn" => state(St::ControlledSinceBeforeCombat),
        "ControlledSinceBeginningOfMostRecentTurn" => state(St::ControlledSinceMostRecentTurn),
        "IsFirstLandPlayedByPlayerThisTurn" => state(St::FirstLandPlayedThisTurn),
        "HadToAttackThisCombat" => state(St::HadToAttack),
        "CouldntAttackThisTurn" => state(St::CouldntAttack),
        "HasTheMostVotesOrTiedForTheMostVotes" | "GraveyardCardWithMostVotesOrTiedForMostVotes" => {
            state(St::MostVotes)
        }
        "ReceivedAVote" => state(St::ReceivedAVote),
        "ManaSpentIsLessThanManaValue" => state(St::ManaSpentBelowManaValue),
        "SnowManaOfSpellsColorWasSpentToCastIt" => state(St::SnowManaOfOwnColorSpent),
        "AdditionalCostWasPaid" => state(St::CostPaid(Cost::Additional)),
        "ProwlCostWasPaid" => state(St::CostPaid(Cost::Prowl)),
        "SneakCostWasPaid" => state(St::CostPaid(Cost::Sneak)),
        "SpectacleCostWasPaid" => state(St::CostPaid(Cost::Spectacle)),
        "SurgeCostWasPaid" => state(St::CostPaid(Cost::Surge)),
        "EmergeCostWasPaid" => state(St::CostPaid(Cost::Emerge)),
        "MadnessCostWasPaid" => state(St::CostPaid(Cost::Madness)),
        "WasCastUsingWebSlinging" => state(St::CostPaid(Cost::WebSlinging)),
        "WasCastUsingTeamwork" => state(St::CostPaid(Cost::Teamwork)),
        "HasANameSticker" => state(St::Sticker(StickerKind::Name)),
        "HasAnArtSticker" => state(St::Sticker(StickerKind::Art)),
        "HasAPowerAndToughnessSticker" => state(St::Sticker(StickerKind::PowerToughness)),
        "SharesANameOriginallyPrintedInArabianNights" => {
            ObjectFilter::NameFirstPrintedIn(FirstPrinting::ArabianNights)
        }
        "SharesANameOriginallyPrintedInAntiquities" => {
            ObjectFilter::NameFirstPrintedIn(FirstPrinting::Antiquities)
        }
        "SharesANameOriginallyPrintedInHomelands" => {
            ObjectFilter::NameFirstPrintedIn(FirstPrinting::Homelands)
        }
        "IsNotAllColors" => property(Property::ColorCount, (Comparison::Less, Expr::Constant(5))),
        "HasExiledCards" => property(Property::ExiledCount, at_least_one()),
        "HasBasicLandType" => {
            use crate::mtg_types::LandType as L;
            ObjectFilter::Either(
                [L::Plains, L::Island, L::Swamp, L::Mountain, L::Forest]
                    .iter()
                    .map(|ty| ObjectFilter::Subtype(crate::subtype::land_type(ty)))
                    .collect(),
            )
        }
        "WasCast" => ObjectFilter::Cast {
            by: None,
            from: None,
        },
        "WasntCastFromHand" | "WasntCastFromTheirHand" => not(cast_from(EventZone::Hand)),
        "WasCastFromTheirGraveyard" => cast_from(EventZone::Graveyard),
        "RegeneratedThisTurn" => history(ObjectHistory::Regenerated, ObjectWindow::ThisTurn),
        "HadAnAbilityActivatedThisTurn" => {
            history(ObjectHistory::AbilityActivated, ObjectWindow::ThisTurn)
        }
        "WasDrawnThisTurn" => history(ObjectHistory::Drawn(None), ObjectWindow::ThisTurn),
        "WasDiscardedIntoGraveyardThisTurn" => {
            history(ObjectHistory::Discarded, ObjectWindow::ThisTurn)
        }
        "WasMilledIntoGraveyardThisTurn" => history(ObjectHistory::Milled, ObjectWindow::ThisTurn),
        "WasExiledThisTurn" => history(ObjectHistory::Exiled, ObjectWindow::ThisTurn),
        "WasntPutIntoGraveyardThisCombat" => not(history(
            ObjectHistory::PutIntoGraveyard,
            ObjectWindow::ThisCombat,
        )),
        "HasntDealtCombatDamageThisGame" => not(history(
            ObjectHistory::DealtCombatDamage,
            ObjectWindow::ThisGame,
        )),
        "WasDealtDamageByThisSpell" => history(
            ObjectHistory::DamagedBy(ObjectRef::Source),
            ObjectWindow::ThisTurn,
        ),
        _ => return None,
    })
}

fn reduce_object_filter(form: &str, operands: &[Operand]) -> Option<ObjectFilter> {
    use DistinctProperty as D;
    use ObjectWindow as W;
    use ObjectHistory as H;
    use Operand::{Comparison as Than, Filter, Object, Objects, Player, Players, Result as Run};
    if operands.is_empty() {
        return fieldless_state(form);
    }
    Some(match (form, operands) {
        // What a preceding instruction named, whatever kind of result it holds.
        (
            "InTheExiledPileChosenThisWay"
            | "TheGraveyardCardsSurveiledThisWay"
            | "TheCardsExiledThisWay"
            | "TheOtherPermanentsExiledThisWay"
            | "TheExiledPileChosenThisWay"
            | "ThePilesExiledThisWay"
            | "ThePermanentsPutOnTheBattlefieldThisWay"
            | "RemovedFromCombatThisWay"
            | "TheCardsReturnedToHandThisWay"
            | "ThePermanentsTappedThisWay"
            | "ThePermanentsAffectedThisWay"
            | "TheCardsInHandChosenThisWay"
            | "TheDiscardedCardsChosenThisWay"
            | "TheCardsSeekedThisWay"
            | "WasDestroyedThisWay"
            | "WasTurnedFaceUpThisWay"
            | "ThePermanentsChosenThisWay"
            | "TheGraveyardCardsChosenThisWay",
            [Run(result)],
        ) => ObjectFilter::InResult(result.clone()),
        ("InTheExiledPileNotChosenThisWay" | "TheGraveyardCardsNotChosenThisWay", [Run(result)]) => {
            not(ObjectFilter::InResult(result.clone()))
        }
        ("TheCardsOfTypeRevealedThisWay", [Filter(filter), Run(result)]) => {
            ObjectFilter::All(vec![
                filter.clone(),
                ObjectFilter::InResult(result.clone()),
            ])
        }
        // Sharing a characteristic.
        ("SharesAColorWithAPermanent", [Objects(with)]) => {
            ObjectFilter::SharesColorWithAny(Box::new(with.clone()))
        }
        ("SharesACreatureTypeWithPermanents", [Objects(with)]) => {
            shares_any(D::CreatureTypes, with.clone())
        }
        ("SharesACardtypeWithAnExiledCard", [Objects(with)]) => {
            shares_any(D::CardTypes, with.clone())
        }
        ("DoesntShareALandTypeWithAPermanent", [Objects(with)]) => {
            not(shares_any(D::LandTypes, with.clone()))
        }
        ("DoesntShareANameWithAPermanent", [Objects(with)]) => {
            not(ObjectFilter::SharesNameWithAny(Box::new(with.clone())))
        }
        ("SharesANameWithCardInPlayersGraveyard", [Filter(cards), Player(owner)]) => {
            ObjectFilter::SharesNameWithAny(Box::new(cards_of(EventZone::Graveyard, owner, cards)))
        }
        ("DoesntShareANameWithACardInPlayersGraveyard", [Filter(cards), Player(owner)]) => {
            not(ObjectFilter::SharesNameWithAny(Box::new(cards_of(
                EventZone::Graveyard,
                owner,
                cards,
            ))))
        }
        ("DoesntShareANameWithACardInPlayersLibrary", [Player(owner)]) => {
            not(ObjectFilter::SharesNameWithAny(Box::new(cards_of(
                EventZone::Library,
                owner,
                &ObjectFilter::Any,
            ))))
        }
        ("SharesAColorWithACardInPlayersGraveyard", [Filter(cards), Player(owner)]) => {
            ObjectFilter::SharesColorWithAny(Box::new(cards_of(EventZone::Graveyard, owner, cards)))
        }
        ("SharesANameWithAGraveyardCard", [Filter(cards)]) => {
            ObjectFilter::SharesNameWithAny(Box::new(zone_cards(cards)?))
        }
        ("SharesAColorWithTheTopCardOfPlayersLibrary", [Player(owner)]) => {
            ObjectFilter::SharesColorWithAny(Box::new(ObjectSet::Take {
                objects: Box::new(cards_of(EventZone::Library, owner, &ObjectFilter::Any)),
                count: Expr::Constant(1),
                from: SequenceEnd::Top,
            }))
        }
        ("SharesAManaValueWithSpell" | "SharesAManaValueWithExiledCard", [Object(with)]) => {
            shares(D::ManaValues, with)
        }
        ("DoesntSharesACardtypeWithSpell", [Object(with)]) => not(shares(D::CardTypes, with)),
        ("DoesntShareACreatureTypeWithPermanent", [Object(with)]) => {
            not(shares(D::CreatureTypes, with))
        }
        ("SharesAPermanentCardtypeWithPermanent", [Object(with)]) => {
            shares(D::PermanentTypes, with)
        }
        ("DoesntShareANameWithSpell", [Object(with)]) => {
            not(ObjectFilter::SharesNameWith(with.clone()))
        }
        ("SharesTotalPowerAndToughnessWithPermanent", [Object(with)]) => property(
            Property::TotalPowerAndToughness,
            (
                Comparison::Equal,
                Expr::ObjectProperty {
                    object: with.clone(),
                    property: Property::TotalPowerAndToughness,
                    information: Information::CurrentOrLastKnown,
                },
            ),
        ),
        ("SharesAColorWith", [color]) if vocabulary_form(color) == Some("TheChosenColors") => {
            ObjectFilter::Chosen(ValueKind::Color)
        }
        // Numbers an object has.
        ("XIs", [Than(comparison, value)]) => {
            property(Property::SpellX, (comparison.clone(), value.clone()))
        }
        ("IntensityIs", [Than(comparison, value)]) => {
            property(Property::Intensity, (comparison.clone(), value.clone()))
        }
        ("TotalPowerAndToughnessIs", [Than(comparison, value)]) => property(
            Property::TotalPowerAndToughness,
            (comparison.clone(), value.clone()),
        ),
        ("NumCreatureTypesIs", [Than(comparison, value)]) => property(
            Property::CreatureTypeCount,
            (comparison.clone(), value.clone()),
        ),
        ("HasNumberCardTypes", [Than(comparison, value)]) => {
            property(Property::CardTypeCount, (comparison.clone(), value.clone()))
        }
        ("NumberOfDifferentManaColorSymbolsInCostIs", [Than(comparison, value)]) => property(
            Property::ColorSymbolCount,
            (comparison.clone(), value.clone()),
        ),
        ("HasExiledNumCards", [Than(comparison, value)]) => {
            property(Property::ExiledCount, (comparison.clone(), value.clone()))
        }
        ("WasCrewedByNumberCreatures", [Than(comparison, value)]) => {
            property(Property::CrewCount, (comparison.clone(), value.clone()))
        }
        (
            "BasePowerAndToughnessIsEqualTo",
            [Operand::Term {
                family, operands, ..
            }],
        ) if family == "PT" => {
            let [Operand::Integer(power), Operand::Integer(toughness)] = operands.as_slice() else {
                return None;
            };
            ObjectFilter::All(vec![
                property(
                    Property::BasePower,
                    (Comparison::Equal, Expr::Constant(*power)),
                ),
                property(
                    Property::BaseToughness,
                    (Comparison::Equal, Expr::Constant(*toughness)),
                ),
            ])
        }
        ("APermanentWithTheLowestManaValue", [Objects(among)]) => ObjectFilter::Extreme {
            among: Box::new(among.clone()),
            property: Property::ManaValue,
            highest: false,
        },
        // How it was cast.
        ("WasntCastFromAPlayersHand", [Players(players)]) => not(ObjectFilter::Cast {
            by: None,
            from: Some(ZoneScope::Players {
                zone: EventZone::Hand,
                players: players.clone(),
            }),
        }),
        ("PlayedByAPlayer", [Players(players)]) => ObjectFilter::Cast {
            by: Some(players.clone()),
            from: None,
        },
        ("CastByPlayerDuringPlayersMainPhase", [Player(by), Player(_)]) => {
            ObjectFilter::All(vec![
                ObjectFilter::Cast {
                    by: Some(PlayerSet::One(by.clone())),
                    from: None,
                },
                state(ObjectState::CastDuringOwnMainPhase),
            ])
        }
        ("AlternateCostWasPaid", cost) => ObjectFilter::AlternateCostPaid(cost.to_vec()),
        ("ManaCostIs", cost) => ObjectFilter::ManaCostIs(cost.to_vec()),
        ("NotInAPlayersStartingDeck", [Players(players)]) => {
            not(ObjectFilter::InStartingDeck(players.clone()))
        }
        ("HasColorManaSymbolInManaCost", [color]) => {
            ObjectFilter::ManaSymbolInCost(simple_color(color)?)
        }
        ("ManaAmountOfTypeWasSpentToCastIt", [Than(comparison, value), color]) => {
            ObjectFilter::ManaSpent {
                color: Some(simple_color(color)?),
                sources: None,
                payers: None,
                comparison: comparison.clone(),
                value: value.clone(),
            }
        }
        ("AnAmountOfManaFromPermanentSpentWasToCastIt", [Than(comparison, value), Objects(from)]) => {
            ObjectFilter::ManaSpent {
                color: None,
                sources: Some(Box::new(event_filter(from)?)),
                payers: None,
                comparison: comparison.clone(),
                value: value.clone(),
            }
        }
        ("APlayerSpentManaToCastIt", [Players(players)]) => {
            let (comparison, value) = at_least_one();
            ObjectFilter::ManaSpent {
                color: None,
                sources: None,
                payers: Some(players.clone()),
                comparison,
                value,
            }
        }
        // Targets a stack object has or could have.
        ("TargetsOnlyASinglePlayer", [Players(players)]) => {
            single_target(Recipients::Players(players.clone()))
        }
        ("DoesntTargetAPermanent", [Objects(objects)]) => not(ObjectFilter::TargetsAny(Box::new(
            Recipients::Objects(objects.clone()),
        ))),
        ("CanTargetOnly", [Objects(objects)]) => {
            ObjectFilter::CanTargetOnly(Box::new(event_filter(objects)?))
        }
        ("WouldDestroyAPermanent", [Objects(objects)]) => {
            ObjectFilter::WouldDestroy(Box::new(event_filter(objects)?))
        }
        ("CouldBeTargetedBySpell", [Object(spell)]) => ObjectFilter::TargetableBy(spell.clone()),
        ("CanEnchantPermanent", [Object(object)]) => {
            ObjectFilter::CouldEnchant(Box::new(ObjectSet::One(object.clone())))
        }
        ("CanEnchantAPermanent", [Objects(objects)]) => {
            ObjectFilter::CouldEnchant(Box::new(objects.clone()))
        }
        ("CouldProduce", [symbol @ Operand::Vocabulary { .. }]) => {
            ObjectFilter::CouldProduce(Box::new(symbol.clone()))
        }
        ("CouldProduceAnyManaColorPermanentCouldProduce", [Object(object)]) => {
            ObjectFilter::CouldProduceManaOf(object.clone())
        }
        // Combat.
        ("IsAttackingPlayerOrPlaneswalkerTheyControl", [Player(player)]) => {
            ObjectFilter::AttackingDefender(Box::new(AttackDefenders::PlayersAndPlaneswalkers(
                PlayerSet::One(player.clone()),
            )))
        }
        ("IsAttackingAPlayerOrPlaneswalkerTheyControl", [Players(players)]) => {
            ObjectFilter::AttackingDefender(Box::new(AttackDefenders::PlayersAndPlaneswalkers(
                players.clone(),
            )))
        }
        ("IsAttackingAPermanent", [Objects(objects)]) => ObjectFilter::AttackingDefender(Box::new(
            AttackDefenders::Objects(event_filter(objects)?),
        )),
        ("IsAttackingABattle", [Objects(objects)]) => {
            ObjectFilter::AttackingDefender(Box::new(AttackDefenders::Objects(ObjectFilter::All(
                vec![
                    ObjectFilter::CardType(crate::mtg_types::CardType::Battle),
                    event_filter(objects)?,
                ],
            ))))
        }
        ("IsBlockingAnAttacker", [Objects(objects)]) => {
            ObjectFilter::BlockingAny(Box::new(event_filter(objects)?))
        }
        ("IsBlockedByADefender", [Objects(objects)]) => {
            ObjectFilter::BlockedByAny(Box::new(event_filter(objects)?))
        }
        ("IsPairedWithA", [Objects(objects)]) => {
            ObjectFilter::PairedWith(Box::new(event_filter(objects)?))
        }
        ("BandedWith", [Object(object)]) => ObjectFilter::BandedWith(object.clone()),
        ("HasAnAttachment", [Objects(objects)]) => {
            ObjectFilter::HasAttached(Box::new(event_filter(objects)?))
        }
        ("IsEnchantedByANumberOfEnchantingPermanents", [Than(comparison, value), Objects(objects)]) => {
            ObjectFilter::AttachedCount {
                attachments: Box::new(event_filter(objects)?),
                comparison: comparison.clone(),
                value: value.clone(),
            }
        }
        ("ProtectedByAPlayer", [Players(players)]) => ObjectFilter::ProtectedBy(players.clone()),
        // What happened to it.
        ("EnteredUnderPlayersControl", [Player(player)]) => {
            ObjectFilter::ControlledBy(PlayerSet::One(player.clone()))
        }
        ("AttackedDuringLastPlayersTurn", [Player(player)]) => {
            history(H::Attacked, W::LastTurnOf(player.clone()))
        }
        ("AttackedSincePlayersLastUpkeep", [Player(player)]) => {
            history(H::Attacked, W::SinceLastUpkeepOf(player.clone()))
        }
        ("BlockedSincePlayersLastUpkeep", [Player(player)]) => {
            history(H::Blocked, W::SinceLastUpkeepOf(player.clone()))
        }
        ("WasBlockedSincePlayersLastUpkeep", [Player(player)]) => {
            history(H::WasBlocked, W::SinceLastUpkeepOf(player.clone()))
        }
        ("EnteredTheBattlefieldSinceLastTurnOf", [Player(player)]) => {
            history(H::Entered, W::SinceLastTurnOf(player.clone()))
        }
        ("AttackedABattleThisTurn", [Objects(objects)]) => history(
            H::AttackedDefender(Box::new(event_filter(objects)?)),
            W::ThisTurn,
        ),
        ("EnlistedAPermanentThisCombat", [Objects(objects)]) => history(
            H::Enlisted(Box::new(event_filter(objects)?)),
            W::ThisCombat,
        ),
        ("ConvokedSpell" | "ConvokedPermanent", [Object(object)]) => {
            history(H::Convoked(object.clone()), W::ThisGame)
        }
        ("ExploitedCreature", [Object(object)]) => {
            history(H::Exploited(object.clone()), W::ThisGame)
        }
        ("WasTappedToPayForAbilitiesOfPermanent", [Object(object)]) => {
            history(H::TappedToPayFor(object.clone()), W::ThisTurn)
        }
        ("AmongCardsDrawByPlayerThisTurn", [Player(player)]) => history(
            H::Drawn(Some(PlayerSet::One(player.clone()))),
            W::ThisTurn,
        ),
        ("AmongCardsDrawByAPlayerThisTurn", [Players(players)]) => {
            history(H::Drawn(Some(players.clone())), W::ThisTurn)
        }
        ("WasPutIntoExileFromAPlayersGraveyardThisTurn", [Players(players)]) => {
            history(H::ExiledFromGraveyard(players.clone()), W::ThisTurn)
        }
        // Who exiled it.
        ("WasExiledByPlayer", [Player(player)]) => exiled_by(player, None, false),
        ("WasExiledByPlayerWithPermanent", [Player(player), Object(with)]) => {
            exiled_by(player, Some(with), false)
        }
        ("WasExiledByPlayerWithPermanentThisTurn", [Player(player), Object(with)]) => {
            exiled_by(player, Some(with), true)
        }
        ("WasExiledWithPermanentsDelveAbility", [Object(with)]) => {
            ObjectFilter::ExiledWith(with.clone())
        }
        _ => return None,
    })
}

fn vocabulary_form(operand: &Operand) -> Option<&str> {
    match operand {
        Operand::Vocabulary { form, .. } => Some(form),
        _ => None,
    }
}
