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

fn copy_of(object: ObjectRef, effects: &Operand) -> Change {
    Change::CopyOf {
        object,
        effects: Box::new(effects.clone()),
    }
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
            Change::AddNames(ObjectFilter::All(vec![
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
        ("AddAbilityAndLoseAllOtherAbilities", abilities) if abilities.len() > 1 => {
            Change::OnlyAbilities(ability_list(abilities)?)
        }
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
        ),
        ("IsACopyOfPermanentSpell", [Operand::Object(object), effects]) => {
            copy_of(object.clone(), effects)
        }
        ("IsACopyOfTheRevealedCard", [effects]) => {
            copy_of(earlier_object(EarlierObjects::Revealed), effects)
        }
        ("IsACopyOf_TheObjectChosenToCopy", [effects]) => {
            copy_of(earlier_object(EarlierObjects::Selected), effects)
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
            &one(&ObjectRef::Named {
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
        _ => return None,
    })
}

/// Spells cast from a zone, with whatever else limits them.
fn cast_limited(spells: &ObjectFilter, from: ObjectFilter) -> ManaUse {
    ManaUse::CastSpell(match spells {
        ObjectFilter::Any => from,
        spells => ObjectFilter::All(vec![spells.clone(), from]),
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
        _ => return None,
    })
}
