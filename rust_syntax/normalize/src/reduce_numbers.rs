// Numbers in typed form, from the same kinds of parts Forge's `Count$` strings
// use: what is counted, among which objects, and at what moment.

fn named_results(result: &ResultId) -> ObjectSet {
    ObjectSet::Recorded(result.clone())
}

fn matching_results(result: &ResultId, filter: ObjectFilter) -> ObjectSet {
    ObjectSet::Matching {
        objects: Box::new(named_results(result)),
        filter,
    }
}

fn counters_on(counter: &Operand, recipients: Recipients) -> Option<Expr> {
    Some(Expr::CountCounters {
        recipients: Box::new(recipients),
        kind: Some(counter_kind(counter)?),
        information: Information::CurrentOrLastKnown,
    })
}

fn spent_on(
    object: &ObjectRef,
    colors: Option<Vec<ManaColor>>,
    snow: bool,
    aggregation: SpentManaAggregation,
) -> Expr {
    Expr::ManaSpent {
        object: object.clone(),
        filter: SpentManaFilter { colors, snow },
        aggregation,
    }
}

fn spent_from(object: &ObjectRef, sources: ObjectFilter) -> Expr {
    Expr::ManaSpentFrom {
        object: object.clone(),
        sources: Box::new(sources),
    }
}

fn treasure() -> ObjectFilter {
    ObjectFilter::Subtype(crate::subtype::artifact_type(
        &crate::mtg_types::ArtifactType::Treasure,
    ))
}

fn symbol_color(symbol: &Operand) -> Option<ManaColor> {
    match vocabulary_form(symbol)? {
        "ManaProduceW" => Some(ManaColor::White),
        "ManaProduceU" => Some(ManaColor::Blue),
        "ManaProduceB" => Some(ManaColor::Black),
        "ManaProduceR" => Some(ManaColor::Red),
        "ManaProduceG" => Some(ManaColor::Green),
        _ => None,
    }
}

/// Commanders named by a source filter, wherever a commander can be.
fn commander_filter(term: &Operand) -> Option<ObjectFilter> {
    fn filter(term: &Operand) -> Option<ObjectFilter> {
        let Operand::Term { form, operands, .. } = term else {
            return None;
        };
        match (form.as_str(), operands.as_slice()) {
            ("IsACommander", []) => Some(ObjectFilter::State(ObjectState::Commander)),
            ("OwnedByAPlayer", [Operand::Players(players)]) => {
                Some(ObjectFilter::OwnedBy(players.clone()))
            }
            ("IsYourCommander", []) => Some(ObjectFilter::OwnedBy(PlayerSet::One(
                PlayerRef::AbilityController,
            ))),
            ("IsCardtype", [kind]) => Some(ObjectFilter::CardType(vocabulary_value(kind)?)),
            ("ManaValueIs", [Operand::Comparison(comparison, value)]) => {
                Some(ObjectFilter::Property { property: Property::ManaValue, test: NumberTest::Compare { comparison: comparison.clone(), value: value.clone() } })
            }
            ("And", items) => Some(ObjectFilter::AllOf(
                items.iter().map(filter).collect::<Option<_>>()?,
            )),
            _ => None,
        }
    }
    Some(ObjectFilter::AllOf(vec![
        ObjectFilter::State(ObjectState::Commander),
        filter(term)?,
    ]))
}

fn commanders(term: &Operand) -> Option<ObjectSet> {
    let filter = commander_filter(term)?;
    Some(ObjectSet::Union(vec![
        ObjectSet::Zone {
            scope: ZoneScope::Shared(EventZone::Command),
            kind: ObjectKind::Card,
            filter: filter.clone(),
        },
        ObjectSet::battlefield(filter),
    ]))
}

fn spells_cast_this_turn(spells: &ObjectFilter, measure: HistoryMeasure) -> Expr {
    Expr::EventHistory {
        event: Box::new(EventSpec::Spell {
            event: SpellEvent::Cast,
            players: PlayerSet::All,
            spells: spells.clone(),
        }),
        window: HistoryWindow::ThisTurn,
        measure,
    }
}

/// The spells cast this turn that `spells` asks for, as they last existed. Who
/// cast one is the event's, and the rest is said of the spell.
fn spells_cast_this_turn_set(spells: &ObjectFilter) -> ObjectSet {
    let caster = |filter: &ObjectFilter| match filter {
        ObjectFilter::Cast {
            by: Some(by),
            from: None,
        } => Some(by.clone()),
        _ => None,
    };
    let (players, spells) = match spells {
        ObjectFilter::AllOf(filters) => match filters.iter().find_map(caster) {
            Some(by) => {
                let rest = filters.iter().filter(|f| caster(f).is_none());
                (by, ObjectFilter::AllOf(rest.cloned().collect()))
            }
            None => (PlayerSet::All, spells.clone()),
        },
        other => match caster(other) {
            Some(by) => (by, ObjectFilter::Any),
            None => (PlayerSet::All, spells.clone()),
        },
    };
    ObjectSet::FromHistory {
        event: Box::new(EventSpec::Spell {
            event: SpellEvent::Cast,
            players,
            spells,
        }),
        window: HistoryWindow::ThisTurn,
    }
}

fn as_of(moment: Moment, value: Expr) -> Expr {
    Expr::AsOf {
        moment,
        value: Box::new(value),
    }
}

fn reduce_game_number(form: &str, operands: &[Operand]) -> Option<Expr> {
    use DistinctProperty as D;
    use Operand::{Filter, Number, Object, Objects, Player, Players, Result as Run};
    use SpentManaAggregation::{Count, DistinctColors};
    let player_number = |player: &PlayerRef, number| Expr::PlayerNumber {
        player: player.clone(),
        number,
    };
    let aggregate = |objects: ObjectSet, property, aggregation| Expr::Aggregate {
        objects: Box::new(objects),
        property,
        aggregation,
    };
    let count = |objects: ObjectSet| Expr::CountObjects(Box::new(objects));
    let distinct = |objects: ObjectSet, property| Expr::CountDistinct {
        objects: Box::new(objects),
        property,
    };
    Some(match (form, operands) {
        // The amount the matched event carries.
        (
            "WouldDrawACard_ThatMany"
            | "Trigger_ThatMuch"
            | "APlayerWouldMillAnyNumberOfCards_ThatMuch"
            | "WhenAPlayerPaysLife_AmountOfLifePaid"
            | "WhenAnyNumberOfPermanentEnterTheBattlefieldUnderControl_AmountOfPermanents"
            | "WhenASpellOrAbilityExilesAnyNumberOfPermanents_AmountOfPermanents"
            | "AsLoseUnspentMana_AmountOfUnspentMana"
            | "WhenPermanentsDealDamageToPlayers_NumPlayersDealtDamage"
            | "WhenAnyNumberOfCreaturesDealCombatDamageToAnyNumberOfPlayers_AmountOfOpponentsDealtDamage"
            | "Trigger_NumberOfPlayersBeingAttacked",
            [],
        ) => Expr::EventAmount,
        ("WhenAPlayerCastsASpell_ManaValueOfThatSpell", []) => {
            property_of(&ObjectRef::EventSubject, Property::ManaValue)
        }
        ("WhenAPlayerCastsASpell_ThatSpellX", []) => Expr::CastX(ObjectRef::EventSubject),
        // Counters.
        (
            "TheNumberOfCountersOfTypeOnPlane"
            | "TheNumberOfCountersOfTypeOnScheme"
            | "TheNumberOfCountersOfTypeOnVanguard",
            [counter, _],
        ) => counters_on(counter, Recipients::Objects(ObjectSet::One(ObjectRef::Source)))?,
        ("TheNumberOfCountersOfTypeOnSpell", [counter, Object(object)]) => {
            counters_on(counter, Recipients::Objects(ObjectSet::One(object.clone())))?
        }
        ("TheTotalNumberOfCountersOfTypeAmongCardsInExile", [counter, Objects(objects)]) => {
            counters_on(counter, Recipients::Objects(objects.clone()))?
        }
        ("TheTotalNumberOfCountersOfTypeAmongCardsInGraveyards", [counter, Filter(cards)]) => {
            counters_on(counter, Recipients::Objects(zone_cards(cards)?))?
        }
        // Mana spent.
        ("TheAmountOfSnowManaSpentToCastSpell", [Object(object)]) => {
            spent_on(object, None, true, Count)
        }
        ("NumColorsManaSpentToCastSpell", [Object(object)]) => {
            spent_on(object, None, false, DistinctColors)
        }
        ("NumColorsManaSpentToCastSelf", []) => {
            spent_on(&ObjectRef::Source, None, false, DistinctColors)
        }
        ("WhenAPermanentEntersTheBattlefield_NumberOfColorsOfManaSpentToCast", []) => {
            spent_on(&ObjectRef::EventSubject, None, false, DistinctColors)
        }
        // "for each {U}{U} spent": whole pairs of one color.
        ("WhenAPermanentEntersTheBattlefield_AmountOfManaOfTypeSpentToCast", symbols) => {
            let colors = symbols.iter().map(symbol_color).collect::<Option<Vec<_>>>()?;
            let color = colors.first()?.clone();
            if colors.iter().any(|other| *other != color) {
                return None;
            }
            Expr::Divide {
                numerator: Box::new(spent_on(
                    &ObjectRef::EventSubject,
                    Some(vec![color]),
                    false,
                    Count,
                )),
                denominator: Box::new(Expr::Constant(colors.len() as i64)),
                rounding: Rounding::Down,
            }
        }
        ("TheManaValueMinusTheManaSpentOnSpell", [Object(object)]) => Expr::Subtract(
            Box::new(property_of(object, Property::ManaValue)),
            Box::new(spent_on(object, None, false, Count)),
        ),
        ("TheAmountOfManaFromATreasureSpentToCastSpell", [Object(object)]) => {
            spent_from(object, treasure())
        }
        ("WhenAPermanentEntersTheBattlefield_AmountOfManaFromTreasureSpentToCast", []) => {
            spent_from(&ObjectRef::EventSubject, treasure())
        }
        (
            "WhenAPermanentEntersTheBattlefield_AmountOfManaFromAPermanentSpentToCast",
            [Objects(sources)],
        ) => spent_from(&ObjectRef::EventSubject, event_filter(sources)?),
        ("TheAmountOfManaFromPermanentsSpentToCastSpell", [Objects(sources), Object(object)]) => {
            spent_from(object, event_filter(sources)?)
        }
        // Symbols.
        ("NumColorManaSymbolsInCostsOfPermanents", [color, Objects(objects)]) => {
            Expr::CountManaSymbols {
                objects: Box::new(objects.clone()),
                colors: vec![simple_color(color)?],
            }
        }
        ("NumColorManaSymbolsInCostsOfPermanent", [color, Object(object)]) => {
            Expr::CountManaSymbols {
                objects: Box::new(ObjectSet::One(object.clone())),
                colors: vec![simple_color(color)?],
            }
        }
        ("NumColorManaSymbolsInCostsOfCardsInPlayersGraveyard", [color, Filter(cards), Player(owner)]) => {
            Expr::CountManaSymbols {
                objects: Box::new(cards_of(EventZone::Graveyard, owner, cards)),
                colors: vec![simple_color(color)?],
            }
        }
        // Distinct values.
        ("NumDifferentManaValueAmongCardsInPlayersGraveyard", [Filter(cards), Player(owner)]) => {
            distinct(cards_of(EventZone::Graveyard, owner, cards), D::ManaValues)
        }
        (
            "TheNumberOfDifferentManaCostsAmongCardsInPlayersGraveyard",
            [Filter(cards), Player(owner)],
        ) => distinct(cards_of(EventZone::Graveyard, owner, cards), D::ManaCosts),
        ("TheNumberOfPermanentTypesAmongGraveyardCards", [Filter(cards)]) => {
            distinct(zone_cards(cards)?, D::PermanentTypes)
        }
        ("TheNumberOfDifferentColorPairsAmongPermanents", [Objects(objects)]) => {
            distinct(objects.clone(), D::ColorPairs)
        }
        ("GreatestNumberOfPermanentsThatHaveCreatureTypeInCommon", [Objects(objects)]) => {
            Expr::LargestSharedGroup {
                objects: Box::new(objects.clone()),
                property: D::CreatureTypes,
            }
        }
        // Properties.
        (
            "TheHighestManaValueAmongPermanentsOrCardsInTheCommandZone"
            | "TheGreatestManaValueOfACommanderInTheCommandZoneOrOnTheBattlefield",
            [term],
        ) => aggregate(commanders(term)?, Property::ManaValue, Aggregation::Maximum),
        ("TheManaValueOfCommander" | "ManaValueOfSpell", [Object(object)]) => {
            property_of(object, Property::ManaValue)
        }
        ("TheNumberOfSubtypesPermanentHas", [Object(object)]) => {
            property_of(object, Property::SubtypeCount)
        }
        ("TheNumberOfSupertypesPermanentHas", [Object(object)]) => {
            property_of(object, Property::SupertypeCount)
        }
        ("ToxicValueOfPermanent", [Object(object)]) => property_of(object, Property::ToxicValue),
        ("NumPointsOfBushidoPermanentHas", [Object(object)]) => {
            property_of(object, Property::BushidoValue)
        }
        ("NumTimesPermanentRegeneratedThisTurn", [Object(object)]) => {
            property_of(object, Property::RegenerationsThisTurn)
        }
        ("AmountOfGenericManaInSpellsManaCost", [Object(object)]) => {
            property_of(object, Property::GenericManaInCost)
        }
        ("TheGreatestManaValueAmongCardsInPlayersHand", [Operand::Term { form, .. }, Player(owner)])
            if form == "AnyCard" =>
        {
            aggregate(
                cards_of(EventZone::Hand, owner, &ObjectFilter::Any),
                Property::ManaValue,
                Aggregation::Maximum,
            )
        }
        // Players.
        ("NumberOfTurnsPlayerHasBeguan", [Player(player)]) => {
            player_number(player, PlayerNumber::TurnsBegun)
        }
        ("TheNumberOfAttractionsPlayerHasVisitedThisTurn", [Player(player)]) => {
            player_number(player, PlayerNumber::AttractionsVisitedThisTurn)
        }
        ("TheNumberOfTimesPlayerHasDecendedThisTurn", [Player(player)]) => {
            player_number(player, PlayerNumber::DescentsThisTurn)
        }
        ("NumCardsPlayerCycledOrDiscardedThisTurn", [Player(player)]) => {
            player_number(player, PlayerNumber::CardsCycledOrDiscardedThisTurn)
        }
        ("NumDifferentlyNamedDungeonsPlayerHasComplete", [Player(player)]) => {
            player_number(player, PlayerNumber::DungeonsCompleted)
        }
        ("NumberOfColorsInPlayersCommandersColorIdentity", [Player(player)]) => {
            player_number(player, PlayerNumber::CommanderColorIdentity)
        }
        ("AmountOfUnspentManaOfColorPlayerHas", [color, Player(player)]) => {
            player_number(player, PlayerNumber::UnspentManaOf(simple_color(color)?))
        }
        ("PlayersChosenNumber", [Player(player)]) => {
            player_number(player, PlayerNumber::ChosenNumber)
        }
        ("TheHighestSpeedAmongPlayers", [Players(players)]) => {
            among(players, PlayerNumber::Speed, Aggregation::Maximum)
        }
        ("MaxPermanentsControlledByAPlayer", [Objects(objects), Players(players)]) => among(
            players,
            PlayerNumber::Permanents(Box::new(event_filter(objects)?)),
            Aggregation::Maximum,
        ),
        // What a preceding instruction named.
        (
            "TheNumberOfSpellsAndAbilitiesCounteredThisWay"
            | "NumHandCardsExiledFaceDownThisWay"
            | "TheNumberOfCreaturesGoadedThisWay"
            | "NumPermanentsPhasedOutThisWay"
            | "TheNumberOfSpellsCounteredThisWay",
            [Run(result)],
        ) => count(named_results(result)),
        // What the surveil looked at and left where it was.
        ("TheNumberOfCardsSurveilledOnTopOfLibraryThisWay", [Run(result)]) => {
            Expr::SurveilKeptOnTop(result.clone())
        }
        ("NumberOfCardsOfTypeExiledThisWay", [Filter(filter), Run(result)]) => {
            count(matching_results(result, ObjectFilter::AllOf(vec![
                filter.clone(),
                ObjectFilter::InZone(ZoneScope::Shared(EventZone::Exile)),
            ])))
        }
        ("TheNumberOfPermanentsDealtDamageThisWay", [Objects(objects), Run(result)]) => {
            count(matching_results(result, event_filter(objects)?))
        }
        ("TheNumberOfCountersRemovedThisWay", [Run(result)]) => {
            Expr::CountersRemoved(result.clone())
        }
        ("TheAmountOfManaLostThisWay", [Run(_)]) => Expr::Game(GameValue::ManaLostThisWay),
        ("TheClampedAmountOfDamageDealtThisWay", [Run(result)]) => {
            Expr::ClampedDamage(result.clone())
        }
        (
            "ManaValueOfTheCardFoundThisWay"
            | "TheManaValueOfTheCardDiscoveredThisWay"
            | "ManaValueOfTheCardsRevealedThisWay",
            [Run(result)],
        ) => aggregate(named_results(result), Property::ManaValue, Aggregation::Sum),
        (
            "ManaValueOfTheCardRevealedByPlayerThisWay" | "ManaValueOfCardDiscardedByPlayerThisWay",
            [Player(owner), Run(result)],
        ) => aggregate(
            matching_results(
                result,
                ObjectFilter::OwnedBy(PlayerSet::One(owner.clone())),
            ),
            Property::ManaValue,
            Aggregation::Sum,
        ),
        ("ManaValueOfTheCardsRevealedByPlayersThisWay", [Players(owners), Run(result)]) => {
            aggregate(
                matching_results(result, ObjectFilter::OwnedBy(owners.clone())),
                Property::ManaValue,
                Aggregation::Sum,
            )
        }
        // Spells cast.
        ("TheTotalManaValueOfSpellsCastThisTurn", [Filter(spells)]) => spells_cast_this_turn(
            spells,
            HistoryMeasure::Property(Property::ManaValue, Aggregation::Sum),
        ),
        ("TheGreatestManaValueAmongSpellsCastThisTurn", [Filter(spells)]) => spells_cast_this_turn(
            spells,
            HistoryMeasure::Property(Property::ManaValue, Aggregation::Maximum),
        ),
        ("TheNumberOfCardTypesAmongSpellsCastThisTurn", [Filter(spells)]) => {
            spells_cast_this_turn(spells, HistoryMeasure::Distinct(D::CardTypes))
        }
        // Earlier moments.
        ("TheNumberOfPermanentsOnTheBattlefieldAsSpellWasCast", [Objects(objects)]) => {
            as_of(Moment::ThisSpellCast, count(objects.clone()))
        }
        ("TheGreatestPowerAmongPermanentsAsSpellWasCast", [Objects(objects)]) => as_of(
            Moment::ThisSpellCast,
            aggregate(objects.clone(), Property::Power, Aggregation::Maximum),
        ),
        ("TheNumberOfPermanentsOnTheBattlefieldAtBeginningOfTurn", [Objects(objects)]) => {
            as_of(Moment::TurnBegan, count(objects.clone()))
        }
        ("LifeTotalOfPlayerAsTurnBegan", [Player(player)]) => {
            as_of(Moment::TurnBegan, Expr::player_life(player.clone()))
        }
        ("Power", [Number(base), Number(exponent)]) => {
            Expr::Exponentiate(Box::new(base.clone()), Box::new(exponent.clone()))
        }
        _ => return None,
    })
}
