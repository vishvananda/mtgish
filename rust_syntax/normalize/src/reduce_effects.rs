// Continuous effects: what changes an object, how long it lasts and what mana does.

/// One or more cost symbols a row left as terms, scaled.
fn reduction_terms(operand: &Operand, factor: &Expr) -> Option<Vec<ManaReduction>> {
    let items = match operand {
        Operand::List(items) => items.as_slice(),
        single => std::slice::from_ref(single),
    };
    items
        .iter()
        .map(|item| {
            let (form, operands): (&str, &[Operand]) = match item {
                Operand::Term { form, operands, .. } => (form, operands),
                Operand::Vocabulary { form, .. } => (form, &[]),
                _ => return None,
            };
            let (color, amount) = match (form, operands) {
                ("CostReduceGeneric", [Operand::Integer(n)]) if *n >= 0 => (None, *n),
                ("CostReduceX", []) => (None, 1),
                ("CostReduceW", []) => (Some(ManaColor::White), 1),
                ("CostReduceU", []) => (Some(ManaColor::Blue), 1),
                ("CostReduceB", []) => (Some(ManaColor::Black), 1),
                ("CostReduceR", []) => (Some(ManaColor::Red), 1),
                ("CostReduceG", []) => (Some(ManaColor::Green), 1),
                _ => return None,
            };
            Some(ManaReduction {
                color,
                amount: Expr::multiply(Expr::Constant(amount.into()), factor.clone()),
            })
        })
        .collect()
}

fn reduced_cost(operand: &Operand, factor: &Expr) -> Option<SpellCostModifier> {
    Some(SpellCostModifier::Reduce {
        reductions: reduction_terms(operand, factor)?,
        spill_into_generic: true,
    })
}

fn gain_abilities(objects: ObjectSet, kinds: &Operand) -> Change {
    Change::GainAbilitiesOf {
        objects,
        kinds: Box::new(kinds.clone()),
        once_each_turn: false,
        spend_as: None,
    }
}

/// The objects a term of activated abilities reads.
fn activated_sources(abilities: &Operand) -> Option<ObjectSet> {
    let Operand::Term { form, operands, .. } = abilities else {
        return None;
    };
    match (form.as_str(), operands.as_slice()) {
        ("AbilityOfAPermanent", [Operand::Objects(objects)]) => Some(objects.clone()),
        ("AbilityOfPermanent", [Operand::Object(object)]) => Some(ObjectSet::One(object.clone())),
        ("And", items) => items.iter().find_map(activated_sources),
        _ => None,
    }
}

/// Activated abilities, with leave to spend mana on them differently.
fn activated_with_spending(abilities: &Operand, spend_as: SpendAs) -> Option<Change> {
    let narrowed = matches!(abilities, Operand::Term { form, .. } if form == "And");
    Some(Change::GainAbilitiesOf {
        objects: activated_sources(abilities)?,
        kinds: Box::new(if narrowed {
            abilities.clone()
        } else {
            Operand::Term {
                family: "CheckHasable".into(),
                form: "ActivatedAbility".into(),
                operands: vec![],
            }
        }),
        once_each_turn: false,
        spend_as: Some(spend_as),
    })
}

fn copy_of(object: ObjectRef, effects: &Operand) -> Option<Change> {
    Some(Change::CopyOf {
        object,
        exceptions: crate::copying::from_source(std::slice::from_ref(effects)).ok()?,
    })
}

fn earlier_object(what: EarlierObjects) -> ObjectRef {
    ObjectRef::Earlier { what, by: None }
}

fn type_mode(form: &str) -> Option<ChangeMode> {
    if form.starts_with("Add") {
        Some(ChangeMode::Add)
    } else if form.starts_with("Set") {
        Some(ChangeMode::Set)
    } else {
        None
    }
}

/// A chosen, listed or borrowed type a change adds or sets.
fn type_variable(form: &str, variable: &Operand) -> Option<Change> {
    let mode = type_mode(form)?;
    if let Some(kind) = chosen_kind(variable) {
        return Some(Change::ChosenType { kind, mode });
    }
    let Operand::Term { form, operands, .. } = variable else {
        return None;
    };
    match (form.as_str(), operands.as_slice()) {
        ("EachBasicLandType", []) => Some(Change::LandTypes {
            scope: LandTypeScope::Basic,
            mode,
        }),
        ("CreatureTypesOfExiled", [Operand::Object(object)]) => Some(Change::CreatureTypesOf {
            object: object.clone(),
            mode,
        }),
        _ => None,
    }
}

/// A continuous change a layer, static or spell row names, by what it changes.
fn reduce_layer(form: &str, operands: &[Operand]) -> Option<Change> {
    let one = Expr::Constant(1);
    let cards_of = |zone, player: &PlayerRef, filter: &ObjectFilter| ObjectSet::Zone {
        scope: ZoneScope::Players {
            zone,
            players: PlayerSet::One(player.clone()),
        },
        kind: ObjectKind::Card,
        filter: filter.clone(),
    };
    Some(match (form, operands) {
        (
            "AddCreatureTypeVariable"
            | "AddLandTypeVariable"
            | "SetCreatureTypeVariable"
            | "SetLandTypeVariable",
            [variable],
        ) => match (type_variable(form, variable), form) {
            (None, "SetLandTypeVariable") => Change::SetLandType(Box::new(variable.clone())),
            (change, _) => return change,
        },
        ("RemoveAllLandTypes", []) => Change::RemoveAllLandTypes,
        ("HasAllLandTypes", []) => Change::LandTypes {
            scope: LandTypeScope::All,
            mode: ChangeMode::Add,
        },
        ("HasAllCreatureTypes", []) => Change::CreatureTypes {
            mode: ChangeMode::Set,
            types: CreatureTypeSet::All,
        },
        ("AddCreatureType", [kind]) => Change::CreatureTypes {
            mode: ChangeMode::Add,
            types: CreatureTypeSet::Listed(vec![vocabulary_value(kind)?]),
        },
        ("RemoveCreatureType", [kind]) => {
            Change::RemoveSubtype(crate::subtype::creature_type(&vocabulary_value(kind)?))
        }
        ("RemoveArtifactType", [kind]) => {
            Change::RemoveSubtype(crate::subtype::artifact_type(&vocabulary_value(kind)?))
        }
        ("AddCardtype", [kind]) => Change::AddCardType(vocabulary_value(kind)?),
        ("SetColor", [color]) => return settable_color(color, ChangeMode::Set),
        ("AddColor", [color]) => return settable_color(color, ChangeMode::Add),
        ("SetLandTypes", kinds) if !kinds.is_empty() => {
            Change::SetLandType(Box::new(Operand::List(kinds.to_vec())))
        }
        ("ReplaceCreatureTypeVariableWithNewCreatureType", [from, to]) => Change::ReplaceWord {
            from: Box::new(from.clone()),
            to: Box::new(to.clone()),
        },
        ("SetNameToTheChosenName", []) => Change::ChosenType {
            kind: ValueKind::CardName,
            mode: ChangeMode::Set,
        },
        ("HasAllNamesOfNonlegendaryCreatures", []) => {
            Change::AddNames(ObjectFilter::AllOf(vec![
                ObjectFilter::Not(Box::new(ObjectFilter::Supertype(
                    crate::mtg_types::SuperType::Legendary,
                ))),
                ObjectFilter::CardType(crate::mtg_types::CardType::Creature),
            ]))
        }
        ("IsAColorlessSourceOfDamage", []) => Change::ColorlessDamageSource,
        ("AdjustPT", [Operand::Integer(power), Operand::Integer(toughness)]) => Change::Pump {
            power: Expr::Constant(*power),
            toughness: Expr::Constant(*toughness),
        },
        ("AddAbilityIfItDoesntHaveIt", [Operand::Ability(ability)]) => {
            Change::GrantAbilityIfAbsent(ability.clone())
        }
        ("AddAbilityFromTopOfLibraryHasable", [kinds]) => gain_abilities(
            ObjectSet::LibraryTops(PlayerSet::One(PlayerRef::AbilityController)),
            kinds,
        ),
        (
            "AddAbilityFromCardsInPlayersGraveyardHasable",
            [Operand::Filter(cards), Operand::Player(owner), kinds],
        ) => gain_abilities(cards_of(EventZone::Graveyard, owner, cards), kinds),
        ("AddAbilityFromCardsInHandHasable", [cards, kinds]) => gain_abilities(
            cards_of(
                EventZone::Hand,
                &PlayerRef::AbilityController,
                &hand_filter(cards)?,
            ),
            kinds,
        ),
        (
            "AddAbilityFromAnExiledHasable_MayOnlyActivateOnecEachTurn",
            [Operand::Objects(objects), kinds],
        ) => Change::GainAbilitiesOf {
            objects: objects.clone(),
            kinds: Box::new(kinds.clone()),
            once_each_turn: true,
            spend_as: None,
        },
        ("AddActivatedAbilitiesAndMaySpendManaAsThoughItWasAnyColorToActivate", [abilities]) => {
            return activated_with_spending(abilities, SpendAs::AnyColor)
        }
        (
            "AddActivatedAbilitiesAndMaySpendColorManaAsThoughAnyColorToActivate",
            [abilities, color],
        ) => {
            return activated_with_spending(abilities, SpendAs::ColorAsAnyColor(color_form(color)?))
        }
        ("IsACopyOfLibraryCard", [Operand::Objects(objects), effects]) => copy_of(
            ObjectRef::Query {
                objects: Box::new(objects.clone()),
                index: 0,
            },
            effects,
        )?,
        ("IsACopyOfPermanentSpell", [Operand::Object(object), effects]) => {
            copy_of(object.clone(), effects)?
        }
        ("IsACopyOfTheRevealedCard", [effects]) => {
            copy_of(earlier_object(EarlierObjects::Revealed), effects)?
        }
        ("IsACopyOf_TheObjectChosenToCopy", [effects]) => {
            copy_of(earlier_object(EarlierObjects::Selected), effects)?
        }
        ("HasTextOfGraveyardCardAndTheText", [Operand::Object(object), abilities]) => {
            Change::TextOf {
                object: object.clone(),
                plus: Box::new(abilities.clone()),
            }
        }
        ("IncreaseManaCost", [mana]) => Change::SpellCost(SpellCostModifier::Increase(mana_cost(
            std::slice::from_ref(mana),
        )?)),
        ("IncreaseManaCostForEach", [mana, Operand::Number(each)]) => {
            let symbols = mana_terms(std::slice::from_ref(mana))?;
            Change::SpellCost(SpellCostModifier::Increase(Cost {
                parts: vec![CostPart::Repeated {
                    part: Box::new(CostPart::Mana(symbols)),
                    times: each.clone(),
                }],
            }))
        }
        ("DecreaseManaCost", [symbols]) => Change::SpellCost(reduced_cost(symbols, &one)?),
        ("DecreaseManaCostForEach", [symbols, Operand::Number(each)]) => {
            Change::SpellCost(reduced_cost(symbols, each)?)
        }
        _ => return None,
    })
}

/// The cards exiled to craft an object, as Forge's `ExiledWith`.
fn exiled_with(object: &ObjectRef) -> ObjectSet {
    ObjectSet::Zone {
        scope: ZoneScope::Shared(EventZone::Exile),
        kind: ObjectKind::Card,
        filter: ObjectFilter::ExiledWith(object.clone()),
    }
}

/// Mana read off other objects, as Forge's `Produced$ Special` values.
pub fn reduce_mana_production(name: &str, operands: &[Operand]) -> Option<ManaProduction> {
    let among = |objects: &ObjectSet, each| ManaProduction::ColorsAmong {
        objects: objects.clone(),
        each,
    };
    let one = |object: &ObjectRef| ObjectSet::One(object.clone());
    Some(match (name.strip_prefix("ManaProduce::")?, operands) {
        ("EachManaColorAmongPermanents", [Operand::Objects(objects)]) => among(objects, true),
        (
            "AnyManaColorAmongPermanents" | "AnyManaColorOfAnExiledCard",
            [Operand::Objects(objects)],
        ) => among(objects, false),
        ("AnyManaColorOfPermanent" | "AnyManaColorOfExiledCard", [Operand::Object(object)]) => {
            among(&one(object), false)
        }
        (
            "AnyManaColorAmongCardsInAPlayersGraveyard",
            [Operand::Filter(cards), Operand::Players(players)],
        ) => among(
            &ObjectSet::Zone {
                scope: ZoneScope::Players {
                    zone: EventZone::Graveyard,
                    players: players.clone(),
                },
                kind: ObjectKind::Card,
                filter: cards.clone(),
            },
            false,
        ),
        ("EachColorAmongCardsUsedToCraftPermanent", [Operand::Object(object)]) => {
            among(&exiled_with(object), true)
        }
        ("AnyColorManaSymbolInTheCardRevealedThisWay", [Operand::Result(result)]) => among(
            &one(&ObjectRef::Recorded {
                result: result.clone(),
                by: None,
            }),
            false,
        ),
        ("AnyManaTypePermanentCouldProduce", [Operand::Object(object)]) => {
            ManaProduction::CouldProduce {
                objects: one(object),
                colors_only: false,
            }
        }
        ("AnyManaTypeTheSacrificedPermanentCouldProduce", []) => ManaProduction::CouldProduce {
            objects: ObjectSet::Earlier {
                what: EarlierObjects::Sacrificed,
                by: None,
            },
            colors_only: false,
        },
        ("ManaCostOfPermanent", [Operand::Object(object)]) => {
            ManaProduction::ManaCostOf(object.clone())
        }
        ("OneManaOfEachColorInManaCostOfTheMilledCard", []) => {
            ManaProduction::SymbolsOf(earlier_object(EarlierObjects::Milled))
        }
        ("AnyTwoDifferentManaColors", []) => ManaProduction::DifferentColors(2),
        ("AnyOtherManaColor", []) => ManaProduction::OtherColor,
        ("TheManaLostThisWay", [Operand::Result(result)]) => {
            ManaProduction::LostThisWay(result.clone())
        }
        ("LastNotedManaTypeAndAmount", []) => ManaProduction::LastNoted { amount: true },
        ("ManaOfTheLastNotedType", []) => ManaProduction::LastNoted { amount: false },
        (form, operands) => return reduce_card_mana(form, operands),
    })
}

/// Spells cast from a zone, with whatever else limits them.
fn cast_limited(spells: &ObjectFilter, from: ObjectFilter) -> ManaUse {
    ManaUse::CastSpell(match spells {
        ObjectFilter::Any => from,
        spells => ObjectFilter::AllOf(vec![spells.clone(), from]),
    })
}

fn reduce_mana_effect(form: &str, operands: &[Operand]) -> Option<ManaUse> {
    let graveyard = |players: &PlayerSet| ObjectFilter::Cast {
        by: None,
        from: Some(ZoneScope::Players {
            zone: EventZone::Graveyard,
            players: players.clone(),
        }),
    };
    let when = |spells: &ObjectFilter, abilities, this_turn, body: &Operand| ManaUse::WhenSpent {
        spells: spells.clone(),
        abilities,
        this_turn,
        body: Box::new(body.clone()),
    };
    Some(match (form, operands) {
        ("TriggerSpentOnSpell", [Operand::Filter(spells), body]) => {
            when(spells, false, false, body)
        }
        ("TriggerSpentOnSpellThisTurn", [Operand::Filter(spells), body]) => {
            when(spells, false, true, body)
        }
        ("TriggerSpentOnSpellOrAbility", [_, body]) => when(&ObjectFilter::Any, true, false, body),
        ("FlagSpellsCastWith", [Operand::Filter(spells), Operand::List(effects)]) => {
            ManaUse::SpentGives {
                spells: spells.clone(),
                effects: effects.clone(),
            }
        }
        ("FlagPermanentsCastWith", [Operand::Objects(objects), Operand::List(effects)]) => {
            ManaUse::SpentGives {
                spells: event_filter(objects)?,
                effects: effects.clone(),
            }
        }
        ("CanOnlySpendToCastGraveyardSpells", [Operand::Filter(spells)]) => {
            cast_limited(spells, graveyard(&PlayerSet::All))
        }
        (
            "CanOnlySpendToCastSpellsFromAPlayersGraveyard",
            [Operand::Filter(spells), Operand::Players(players)],
        ) => cast_limited(spells, graveyard(players)),
        ("CanOnlySpendToCastSpellsFromExile", [Operand::Filter(spells)]) => {
            cast_limited(
                spells,
                ObjectFilter::Cast {
                    by: None,
                    from: Some(ZoneScope::Shared(EventZone::Exile)),
                },
            )
        }
        (
            "CanOnlySpendToCastSpellsFromAnywhereOtherThanPlayersHand",
            [Operand::Filter(spells), Operand::Players(players)],
        ) => cast_limited(
            spells,
            ObjectFilter::Not(Box::new(ObjectFilter::Cast {
                by: None,
                from: Some(ZoneScope::Players {
                    zone: EventZone::Hand,
                    players: players.clone(),
                }),
            })),
        ),
        ("CanOnlySpendToCastExiledCard", [Operand::Object(card)]) => {
            ManaUse::CastSpell(ObjectFilter::SameObject(card.clone()))
        }
        ("CanOnlySpendToCastForetoldSpells", []) => {
            ManaUse::CastSpell(ObjectFilter::State(ObjectState::Foretold))
        }
        ("CanOnlySpendToPayACostThatContainsManaSymbol", [symbol]) => {
            ManaUse::Purpose(ManaPurpose::CostContaining(Box::new(symbol.clone())))
        }
        ("CanOnlySpendToTurnAManifestedCreatureFaceUp", []) => {
            ManaUse::Purpose(ManaPurpose::TurnFaceUp(ObjectFilter::State(
                ObjectState::Manifested,
            )))
        }
        _ => return None,
    })
}

/// A duration by what ends it, as Forge's `Duration$` and `ForgetOnMoved$`.
fn reduce_expiration(form: &str, operands: &[Operand]) -> Option<Duration> {
    let matches = |object: &ObjectRef, filter| Duration::WhileMatches {
        object: object.clone(),
        filter,
    };
    Some(match (form, operands) {
        (
            "ForAsLongAsPermanentRemainsAttachedToPermanent",
            [Operand::Object(attachment), Operand::Object(host)],
        ) => matches(
            attachment,
            ObjectFilter::AttachedTo(Box::new(ObjectFilter::SameObject(host.clone()))),
        ),
        ("UntilCardIsNoLongerInGraveyard", [Operand::Object(card)]) => matches(
            card,
            ObjectFilter::InZone(ZoneScope::Players {
                zone: EventZone::Graveyard,
                players: PlayerSet::All,
            }),
        ),
        ("UntilPermanentIsTurnedFaceUp", [Operand::Object(object)]) => {
            matches(object, ObjectFilter::State(ObjectState::FaceDown))
        }
        ("UntilPermanentIsTurnedFaceDown", [Operand::Object(object)]) => matches(
            object,
            ObjectFilter::Not(Box::new(ObjectFilter::State(ObjectState::FaceDown))),
        ),
        ("UntilItIsNoLongerEnchanted", []) => {
            Duration::WhileItMatches(ObjectFilter::State(ObjectState::Enchanted))
        }
        ("UntilItIsNoLongerExiled", []) => {
            Duration::WhileExiled(earlier_object(EarlierObjects::Exiled))
        }
        ("UntilEndOfTheNextTurn", []) => Duration::ThroughNextTurn,
        ("UntilTheEndOfCombatOnPlayersNextTurn", [Operand::Player(player)]) => {
            Duration::EndOfCombatOnNextTurn(player.clone())
        }
        ("UntilPlayersNextUntapStep", [Operand::Player(player)]) => {
            Duration::UntilNextUntapStep(player.clone())
        }
        ("UntilTopCardOfPlayersLibraryChanges", [Operand::Player(player)]) => {
            Duration::UntilLibraryTopChanges(player.clone())
        }
        ("DuringTheCombatPhaseCreatedThisWay", [Operand::Result(result)]) => {
            Duration::ExtraCombat(result.clone())
        }
        ("ForAsLongAsPlayerHasACounterOfType", [Operand::Player(player), counter]) => {
            Duration::WhilePlayerHasCounter {
                player: player.clone(),
                kind: counter_kind(counter)?,
            }
        }
        ("UntilAPlayerIsNoLongerTheMonarch", [Operand::Players(PlayerSet::One(player))]) => {
            Duration::While(Box::new(Condition::PlayerState {
                player: player.clone(),
                state: PlayerState::Monarch,
            }))
        }
        ("UntilAPlayerCastsASpell", [Operand::Players(players), Operand::Filter(spells)]) => {
            Duration::UntilEvent(Box::new(EventSpec::Spell {
                event: SpellEvent::Cast,
                players: players.clone(),
                spells: spells.clone(),
            }))
        }
        ("UntilAPlayerExilesACardWithThisAbility", [Operand::Players(players)]) => {
            Duration::UntilAnotherExiled {
                by: players.clone(),
                with: ObjectRef::Source,
            }
        }
        (
            "UntilPlayerExilesAnotherCardWithPermanent",
            [Operand::Player(player), Operand::Object(object)],
        ) => Duration::UntilAnotherExiled {
            by: PlayerSet::One(player.clone()),
            with: object.clone(),
        },
        _ => return reduce_card_expiration(form, operands),
    })
}

fn counter_kinds(operands: &[Operand]) -> Option<Vec<crate::counter::CounterKind>> {
    operands.iter().map(counter_kind).collect()
}

fn checks(operands: &[Operand]) -> Vec<Operand> {
    match operands {
        [Operand::List(items)] => items.clone(),
        items => items.to_vec(),
    }
}

/// Counters that go somewhere other than one fixed place, as Forge's
/// `DividedAsYouChoose$`, `CounterType$ ... Chosen` and `PutCounterAll`.
fn reduce_more_counters(form: &str, operands: &[Operand]) -> Option<Operation> {
    use Operand::{List, Number, Object, Objects, Player};
    let one = |object: &ObjectRef| Recipients::Objects(ObjectSet::One(object.clone()));
    let many = |objects: &ObjectSet| Recipients::Objects(objects.clone());
    let player = |who: &PlayerRef| Recipients::Players(PlayerSet::One(who.clone()));
    let duplicate = |from, to, copies| {
        Some(Operation::DuplicateCounters {
            from: CounterSource::On(from),
            to,
            copies,
            kind: None,
        })
    };
    let placed = |recipients, kind: &Operand, amount: &Expr| {
        Some(Operation::PutCounters {
            placements: vec![CounterPlacement {
                recipients,
                kind: counter_kind(kind)?,
                amount: amount.clone(),
                selection: None,
            }],
        })
    };
    let chosen = |objects: ObjectSet, choice| Operation::PutChosenCounters { objects, choice };
    let at_source = Recipients::Objects(ObjectSet::One(ObjectRef::Source));
    match (form, operands) {
        ("ACounterOfTypeOnPlane" | "ACounterOfTypeOnScheme" | "ACounterOfTypeOnVanguard", [kind, _]) => {
            placed(at_source, kind, &Expr::Constant(1))
        }
        ("NumberCountersOfTypeOnSpell", [Number(amount), kind, Object(object)]) => {
            placed(one(object), kind, amount)
        }
        (
            "DifferentNumberCountersOfTypeOnEachPermanent"
            | "DifferentNumberCountersOfTypeOnEachCardInExile",
            [Number(amount), kind, Objects(objects)],
        ) => Some(Operation::PutCountersEach {
            amount: amount.clone(),
            kind: counter_kind(kind)?,
            objects: objects.clone(),
            up_to: false,
        }),
        (
            "DistributeNumberCountersOfTypeAmongAnyNumberOfPermanents",
            [Number(amount), kind, Objects(objects)],
        ) => Some(Operation::DistributeCounters {
            amount: amount.clone(),
            kind: counter_kind(kind)?,
            objects: objects.clone(),
            any_number: true,
        }),
        (
            "DistributeNumberCountersOfTypeAmongPermanents",
            [Number(amount), kind, Objects(objects)],
        ) => Some(Operation::DistributeCounters {
            amount: amount.clone(),
            kind: counter_kind(kind)?,
            objects: objects.clone(),
            any_number: false,
        }),
        (
            "UptoSingleNumberCountersOfTypeOnEachPermanent",
            [Number(amount), kind, Objects(objects)],
        ) => Some(Operation::PutCountersEach {
            amount: amount.clone(),
            kind: counter_kind(kind)?,
            objects: objects.clone(),
            up_to: true,
        }),
        ("ACounterOfTypeOfMultipleChoiceOnEachPermanent", [List(kinds), Objects(objects)]) => {
            Some(chosen(objects.clone(), CounterChoice::OneOf(counter_kinds(kinds)?)))
        }
        ("ACounterOfTypeAtRandomThatPermanentDoesHaveOnPermanent", [List(kinds), Object(object)]) => {
            Some(chosen(
                ObjectSet::One(object.clone()),
                CounterChoice::RandomMissing(counter_kinds(kinds)?),
            ))
        }
        ("ACounterOfTypeOnAPermanentOfEachColor", [kind, Objects(objects)]) => Some(chosen(
            objects.clone(),
            CounterChoice::OnePerColor(counter_kind(kind)?),
        )),
        ("AnAbilityCounterOnAPermanentForEachAbilityOnCardsInGraveyards", [Objects(objects), kinds @ ..]) => {
            Some(chosen(
                objects.clone(),
                CounterChoice::PerAbility {
                    of: None,
                    kinds: checks(kinds),
                    missing_only: false,
                },
            ))
        }
        (
            "AnAbilityCounterOnPermanentForEachAbilityOnPermanentIfItDoesntHaveIt",
            [Object(object), kinds @ ..],
        ) => Some(chosen(
            ObjectSet::One(object.clone()),
            CounterChoice::PerAbility {
                of: Some(object.clone()),
                kinds: checks(kinds),
                missing_only: true,
            },
        )),
        ("DuplicateThoseCounterOnPermanent", [Object(to)]) => {
            Some(Operation::DuplicateCounters {from: CounterSource::EventPut, to: one(to), copies: CounterCopies::All, kind: None})
        }
        ("DuplicateACounterOfEachOfThoseKindsOnPermanent", [Object(to)]) => {
            Some(Operation::DuplicateCounters {from: CounterSource::EventPut, to: one(to), copies: CounterCopies::OneOfEachKind, kind: None})
        }
        ("DoubleAllCountersPlayerHas", [Player(who)]) => {
            duplicate(player(who), player(who), CounterCopies::All)
        }
        ("DuplicateCountersOfPermanentOnEachPermanent", [Object(from), Objects(to)]) => {
            duplicate(one(from), many(to), CounterCopies::All)
        }
        ("DuplicateACounterOfPlayerOnPlayer", [Player(from), Player(to)]) => {
            duplicate(player(from), player(to), CounterCopies::One)
        }
        ("DuplicateACounterOfCardInExileOnCardInExile", [Object(from), Object(to)]) => {
            duplicate(one(from), one(to), CounterCopies::One)
        }
        ("DuplicateACounterOfAPermanentOnEachOtherPermanent", [Objects(from), Objects(to)]) => {
            duplicate(many(from), many(to), CounterCopies::One)
        }
        (
            "DuplicateACounterOfAPermanentThatPermanentDoesntHaveOnPermanent",
            [Objects(from), Object(_), Object(to)],
        ) => duplicate(many(from), one(to), CounterCopies::OneMissing),
        (
            "ADuplicateOfEachKindOfCounterAmongPermanentDistibutedToPermanents",
            [Objects(from), Objects(to)],
        ) => duplicate(many(from), many(to), CounterCopies::OneOfEachKindDistributed),
        ("DuplicateCountersOfTypeOfPermanentOnPermanent", [kind, Object(from), Object(to)]) => {
            Some(Operation::DuplicateCounters {
                from: CounterSource::On(one(from)),
                to: one(to),
                copies: CounterCopies::All,
                kind: Some(counter_kind(kind)?),
            })
        }
        _ => None,
    }
}

/// A state a permanent enters with, past the ones `reduce_entry` names.
fn reduce_more_entry(form: &str, operands: &[Operand]) -> Option<EntryState> {
    use Operand::{List, Number, Object, Objects};
    let copy = |objects: ObjectSet, effects: &Operand, until: Option<&Duration>| Some(EntryState::CopyOf {
        objects,
        exceptions: crate::copying::from_source(std::slice::from_ref(effects)).ok()?,
        until: until.cloned(),
    });
    let choice = |kinds: &[Operand], amount: Expr, distinct| {
        Some(EntryState::CountersOfChoice {
            kinds: counter_kinds(kinds)?,
            amount,
            distinct,
        })
    };
    match (form, operands) {
        ("EntersWithACounterOfChoice", kinds) => choice(kinds, Expr::Constant(1), false),
        ("EntersWithNumberDifferentCountersOfChoice", [Number(amount), List(kinds)]) => {
            choice(kinds, amount.clone(), true)
        }
        ("EntersWithNumberCombinationCountersOfChoice", [Number(amount), List(kinds)]) => {
            choice(kinds, amount.clone(), false)
        }
        ("EntersWithNumberCounters", [Number(amount), kind]) => {
            Some(EntryState::Changes(vec![Change::EntersWithCounters {
                kind: counter_kind(kind)?,
                amount: amount.clone(),
            }]))
        }
        ("EntersAttachedToAPermanent", [Objects(objects)]) => {
            Some(EntryState::AttachedToChosen(event_filter(objects)?))
        }
        ("EntersWithDuplicateCountersOfPermanent", [Object(object)]) => {
            Some(EntryState::CountersOf {
                object: object.clone(),
                one_of_each: false,
            })
        }
        ("EntersWithACounterOfTypeForEachKindOfCounterOnPermanent", [Object(object)]) => {
            Some(EntryState::CountersOf {
                object: object.clone(),
                one_of_each: true,
            })
        }
        (
            "EnterAsACopyOfACardInAPlayersGraveyard",
            [Operand::Filter(cards), Operand::Players(players), effects],
        ) => copy(
            ObjectSet::Zone {
                scope: ZoneScope::Players {
                    zone: EventZone::Graveyard,
                    players: players.clone(),
                },
                kind: ObjectKind::Card,
                filter: cards.clone(),
            },
            effects,
            None,
        ),
        ("EnterAsACopyOfACardInExile", [Objects(objects), effects]) => {
            copy(objects.clone(), effects, None)
        }
        ("EnterAsCopyOfExiled", [Object(object), effects]) => {
            copy(ObjectSet::One(object.clone()), effects, None)
        }
        ("EnterAsACopyOfAPermanentUntil", [Objects(objects), effects, Operand::Duration(until)]) => {
            copy(objects.clone(), effects, Some(until))
        }
        _ => None,
    }
}

fn wider_zone_change(objects: ObjectFilter, to: EventZone) -> ReplacementEvent {
    ReplacementEvent::ZoneChange {
        objects,
        from: None,
        to,
    }
}

/// A would-be event by what it replaces, past `reduced_replacement`'s tables.
fn reduce_more_replacement(row: &str, operands: &[Operand]) -> Option<ReplacementEvent> {
    use Operand::{Filter, Object, Objects, Player, Players};
    let enter = |filter| Some(ReplacementEvent::Enter { objects: filter });
    let both = |a, b| ObjectFilter::AllOf(vec![a, b]);
    let not = |filter| ObjectFilter::Not(Box::new(filter));
    let cast = ObjectFilter::Cast {
        by: None,
        from: None,
    };
    let discard = |players: PlayerSet, causes, card, effect_only| {
        Some(ReplacementEvent::Discard {
            players,
            causes,
            card,
            effect_only,
        })
    };
    let player_event = |event, players: PlayerSet| {
        Some(ReplacementEvent::Player { event, players })
    };
    let no_object = || not(ObjectFilter::Any);
    match (row, operands) {
        ("APermanentWouldEnterTheBattlefield", [Objects(objects)]) => enter(event_filter(objects)?),
        ("APermanentWouldEnterTheBattlefieldAndWasntCast", [Objects(objects)]) => {
            enter(both(event_filter(objects)?, not(cast)))
        }
        ("APermanentWouldEnterTheBattlefieldUnderAPlayersControl", [Objects(objects), Players(players)]) => {
            enter(both(
                event_filter(objects)?,
                ObjectFilter::ControlledBy(players.clone()),
            ))
        }
        ("APermanentWouldEnterTheBattlefieldFromExileOrAfterBeingCastFromExile", [Objects(objects)]) => {
            let exile = ZoneScope::Shared(EventZone::Exile);
            enter(both(
                event_filter(objects)?,
                ObjectFilter::AnyOf(vec![
                    ObjectFilter::EnteredFrom(exile.clone()),
                    ObjectFilter::Cast {
                        by: None,
                        from: Some(exile),
                    },
                ]),
            ))
        }
        ("ASpellOrAbilityWouldCausePlayerToDiscardCard", [causes, Player(player), _]) => discard(
            PlayerSet::One(player.clone()),
            Some(stack_filter(causes)?),
            Some(ObjectFilter::SameObject(ObjectRef::Source)),
            false,
        ),
        ("ASpellOrAbilityWouldCausePlayerToDiscardACard", [causes, Player(player)]) => discard(
            PlayerSet::One(player.clone()),
            Some(stack_filter(causes)?),
            None,
            false,
        ),
        ("AnEffectWouldCausePlayerToDiscardACard", [Player(player)]) => {
            discard(PlayerSet::One(player.clone()), None, None, true)
        }
        ("APlayerWouldRollANumberOfDice", [Players(players)]) => {
            player_event(ReplacedPlayerEvent::RollDice, players.clone())
        }
        ("APlayerWouldRollANumberOfD6", [Players(players)]) => {
            player_event(ReplacedPlayerEvent::RollD6, players.clone())
        }
        ("NextTimePlayerWouldRollDiceThisTurn", [Player(player)]) => {
            player_event(ReplacedPlayerEvent::RollDice, PlayerSet::One(player.clone()))
        }
        ("NextTimePlayerWouldDrawACardThisTurn", [Player(player)]) => {
            player_event(ReplacedPlayerEvent::Draw, PlayerSet::One(player.clone()))
        }
        ("APlayerWouldPlaneswalk", [Players(players)]) => {
            player_event(ReplacedPlayerEvent::Planeswalk, players.clone())
        }
        ("PermanentWouldLeaveTheBattlefield", [Object(object)]) => Some(ReplacementEvent::Object {
            event: ReplacedObjectEvent::LeavesBattlefield,
            objects: ObjectFilter::SameObject(object.clone()),
        }),
        (
            "WouldPutACardInAPlayersGraveyardFromAnywhereOtherThanBattlefield",
            [Filter(cards), Players(players)],
        ) => Some(wider_zone_change(
            ObjectFilter::AllOf(vec![
                cards.clone(),
                ObjectFilter::OwnedBy(players.clone()),
                not(ObjectFilter::InZone(ZoneScope::Shared(EventZone::Battlefield))),
            ]),
            EventZone::Graveyard,
        )),
        (
            "WouldPutAPermanentASpellOrACardIntoAPlayersGraveyard",
            [Objects(objects), Filter(spells), Filter(cards), Players(players)],
        ) => Some(wider_zone_change(
            both(
                ObjectFilter::AnyOf(vec![event_filter(objects)?, spells.clone(), cards.clone()]),
                ObjectFilter::OwnedBy(players.clone()),
            ),
            EventZone::Graveyard,
        )),
        ("AnyNumberOfCountersWouldBePutOnAPlayer", [Players(players)]) => {
            Some(ReplacementEvent::Counters {
                kind: None,
                objects: no_object(),
                players: Some(players.clone()),
                by: None,
                effect_only: false,
            })
        }
        ("AnyNumberOfCountersOfTypeWouldBePutOnAPlayer", [kind, Players(players)]) => {
            Some(ReplacementEvent::Counters {
                kind: Some(counter_kind(kind)?),
                objects: no_object(),
                players: Some(players.clone()),
                by: None,
                effect_only: false,
            })
        }
        _ => None,
    }
}
