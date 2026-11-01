// Rules about players and objects, by what they change, as Forge's static modes:
// a permission, a cost, a combat requirement, a prohibition. A form that is one
// card's own rule goes on a closed list; anything unmatched stays with its row.

fn color_form(operand: &Operand) -> Option<ManaColor> {
    match vocabulary_form(operand)? {
        "White" => Some(ManaColor::White),
        "Blue" => Some(ManaColor::Blue),
        "Black" => Some(ManaColor::Black),
        "Red" => Some(ManaColor::Red),
        "Green" => Some(ManaColor::Green),
        _ => None,
    }
}

fn top_of_libraries(players: PlayerSet) -> ObjectSet {
    match players {
        PlayerSet::One(_) => ObjectSet::Take {
            objects: Box::new(zone_set(EventZone::Library, players, ObjectFilter::Any)),
            count: Expr::Constant(1),
            from: SequenceEnd::Top,
        },
        players => ObjectSet::LibraryTops(players),
    }
}

fn zone_set(zone: EventZone, players: PlayerSet, filter: ObjectFilter) -> ObjectSet {
    ObjectSet::Zone {
        scope: ZoneScope::Players { zone, players },
        kind: ObjectKind::Card,
        filter,
    }
}

fn you() -> PlayerSet {
    PlayerSet::One(PlayerRef::Affected)
}

fn play_terms() -> PlayTerms {
    PlayTerms {
        limit: None,
        during: None,
        cost: PlayCost::Normal,
        spend_as: None,
        into_exile: false,
        condition: None,
        effects: vec![],
        flash: false,
        may_look: false,
        then: None,
    }
}

fn play_rule(
    cards: ObjectSet,
    from: EventZone,
    action: CardPlayAction,
    filter: ObjectFilter,
    terms: PlayTerms,
) -> PlayerRule {
    PlayerRule::PlayPermissionWith {
        permission: Box::new(PlayerRule::PlayPermission {
            cards,
            from,
            action,
            filter,
        }),
        terms,
    }
}

fn using_keyword(form: &str) -> Option<CostedKeyword> {
    use CostedKeyword as K;
    Some(match form {
        "Blitz" => K::Blitz,
        "Mutate" => K::Mutate,
        "Bestow" => K::Bestow,
        "Warp" => K::Warp,
        _ => return None,
    })
}

fn alternate_first_keyword(form: &str) -> Option<CostedKeyword> {
    use CostedKeyword as K;
    Some(match form {
        "Equip" => K::Equip,
        "PowerUp" => K::PowerUp,
        "Cycling" => K::Cycling,
        "Unearth" => K::Unearth,
        _ => return None,
    })
}

fn special_action(
    body: &Operand,
    cost: Option<&Operand>,
    limit: Option<PlayLimit>,
    during: Option<PlayerSet>,
    instant_speed: bool,
) -> PlayerRule {
    PlayerRule::SpecialAction {
        body: Box::new(body.clone()),
        cost: cost.map(|cost| Box::new(cost.clone())),
        limit,
        during,
        instant_speed,
    }
}

fn keyword_cost(
    keyword: CostedKeyword,
    change: KeywordCostChange,
    per: Option<&Expr>,
    first_each_turn: bool,
) -> PlayerRule {
    PlayerRule::KeywordCost {
        keyword,
        cards: ObjectFilter::Any,
        change,
        per: per.cloned(),
        first_each_turn,
    }
}

/// Costs of keywords: Forge's `ReduceCost` and `RaiseCost` with a `Type$`.
fn reduce_keyword_cost(bare: &str, operands: &[Operand]) -> Option<PlayerRule> {
    use CostedKeyword as K;
    use KeywordCostChange as C;
    let boxed = |operand: &Operand| Box::new(operand.clone());
    if let Some(rest) = bare.strip_prefix("MayPayAlternateCostForFirst") {
        let keyword = alternate_first_keyword(rest.strip_suffix("CostEachTurn")?)?;
        let [cost] = operands else { return None };
        return Some(keyword_cost(keyword, C::Alternate(boxed(cost)), None, true));
    }
    Some(match (bare, operands) {
        ("DecreaseFlashbackCosts", [amount]) => {
            keyword_cost(K::Flashback, C::Reduce(boxed(amount)), None, false)
        }
        ("IncreaseFlashBackCosts", [amount]) => {
            keyword_cost(K::Flashback, C::Increase(boxed(amount)), None, false)
        }
        ("DecreaseCyclingCosts", [amount]) => {
            keyword_cost(K::Cycling, C::Reduce(boxed(amount)), None, false)
        }
        ("DecreaseCostOfForetellingCardsFromHand", [amount]) => {
            keyword_cost(K::Foretell, C::Reduce(boxed(amount)), None, false)
        }
        ("DecreasePlotFromHandCost", [amount]) => {
            keyword_cost(K::Plot, C::Reduce(boxed(amount)), None, false)
        }
        ("DecreaseDashCost", [amount]) => {
            keyword_cost(K::Dash, C::Reduce(boxed(amount)), None, false)
        }
        ("DecreaseNinjutsuAbilityCost", [amount]) => {
            keyword_cost(K::Ninjutsu, C::Reduce(boxed(amount)), None, false)
        }
        ("DecreaseUnlockCost", [amount]) => {
            keyword_cost(K::Unlock, C::Reduce(boxed(amount)), None, false)
        }
        ("DecreaseBoastAbilityCostForEach", [amount, Operand::Number(per)]) => {
            keyword_cost(K::Boast, C::Reduce(boxed(amount)), Some(per), false)
        }
        ("DecreaseBlitzCostsForEach", [amount, Operand::Number(per)]) => {
            keyword_cost(K::Blitz, C::Reduce(boxed(amount)), Some(per), false)
        }
        ("MayPayAlternateCyclingCosts", [cost]) => {
            keyword_cost(K::Cycling, C::Alternate(boxed(cost)), None, false)
        }
        ("ReplaceForetellCostOfFirstCardForetoldEachTurn", [cost]) => {
            keyword_cost(K::Foretell, C::Alternate(boxed(cost)), None, true)
        }
        ("MayPayAlternateEchoCosts", [cost, Operand::Objects(permanents)]) => {
            PlayerRule::KeywordCost {
                keyword: K::Echo,
                cards: event_filter(permanents)?,
                change: C::Alternate(boxed(cost)),
                per: None,
                first_each_turn: false,
            }
        }
        ("DecreaseCostToTurnPermanentsFaceUp", [Operand::Objects(permanents), amount]) => {
            PlayerRule::KeywordCost {
                keyword: K::TurnFaceUp,
                cards: event_filter(permanents)?,
                change: C::Reduce(boxed(amount)),
                per: None,
                first_each_turn: false,
            }
        }
        (
            "MayRemoveACounterOfTypeFromAPermanentToPlayPermanentsCrewCost",
            [kind, Operand::Objects(permanents), Operand::Object(vehicle)],
        ) => PlayerRule::KeywordCost {
            keyword: K::Crew,
            cards: ObjectFilter::SameObject(vehicle.clone()),
            change: C::Alternate(Box::new(Operand::Cost(Cost {
                parts: vec![CostPart::RemoveCounters {
                    removals: vec![CounterRemoval {
                        player: PlayerRef::Affected,
                        recipients: Recipients::Objects(permanents.clone()),
                        kind: Some(counter_kind(kind)?),
                        amount: CounterAmount::AmongThem(Count::Exactly(Expr::Constant(1))),
                        selection: None,
                    }],
                }],
            }))),
            per: None,
            first_each_turn: false,
        },
        _ => return None,
    })
}

/// The cards a player may play and on what terms: Forge's `MayPlay` parameters.
fn reduce_play_rule(bare: &str, operands: &[Operand]) -> Option<PlayerRule> {
    use CardPlayAction as A;
    use EventZone as Z;
    use Operand::{Filter, Object, Objects, Player, Players};
    let terms = play_terms();
    let hand = || zone_set(Z::Hand, you(), ObjectFilter::Any);
    let graveyard_of = |players| zone_set(Z::Graveyard, players, ObjectFilter::Any);
    let others = || PlayerSet::OthersThan(PlayerRef::Affected);
    if let Some(rest) = bare.strip_prefix("MayCastGraveyardCardUsing") {
        let keyword = using_keyword(rest.strip_suffix("Ability")?)?;
        let [Object(card)] = operands else { return None };
        return Some(play_rule(
            ObjectSet::One(card.clone()),
            Z::Graveyard,
            A::CastSpell,
            ObjectFilter::Any,
            PlayTerms {
                cost: PlayCost::Using(keyword),
                ..terms
            },
        ));
    }
    Some(match (bare, operands) {
        ("MayCastGraveyardCardAsAnAdventure", [Object(card)]) => play_rule(
            ObjectSet::One(card.clone()),
            Z::Graveyard,
            A::CastSpell,
            ObjectFilter::Any,
            PlayTerms {
                cost: PlayCost::Using(CostedKeyword::Adventure),
                ..terms
            },
        ),
        ("MayCastSpellsFromTopOfLibraryAsThoughTheyHadFlash", [Filter(spells)]) => play_rule(
            top_of_libraries(you()),
            Z::Library,
            A::CastSpell,
            spells.clone(),
            PlayTerms {
                flash: true,
                ..terms
            },
        ),
        ("MayCastSpellsWithoutPayingAndAsThoughTheyHadFlash", [Filter(spells)]) => play_rule(
            hand(),
            Z::Hand,
            A::CastSpell,
            spells.clone(),
            PlayTerms {
                cost: PlayCost::Free,
                flash: true,
                ..terms
            },
        ),
        ("MayCastSpellsForAlternateCostAsThoughTheyHadFlash", [Filter(spells), cost]) => play_rule(
            hand(),
            Z::Hand,
            A::CastSpell,
            spells.clone(),
            PlayTerms {
                cost: PlayCost::Alternate(cost.clone()),
                flash: true,
                ..terms
            },
        ),
        ("MayCastSpellsFromTheTopOfTheirGraveyardIntoExile", [Filter(spells)]) => play_rule(
            ObjectSet::Take {
                objects: Box::new(graveyard_of(you())),
                count: Expr::Constant(1),
                from: SequenceEnd::Top,
            },
            Z::Graveyard,
            A::CastSpell,
            spells.clone(),
            PlayTerms {
                into_exile: true,
                ..terms
            },
        ),
        ("MayPlayTopCardOfLibraryForAlternateCost", [cost]) => play_rule(
            top_of_libraries(you()),
            Z::Library,
            A::Either,
            ObjectFilter::Any,
            PlayTerms {
                cost: PlayCost::Alternate(cost.clone()),
                ..terms
            },
        ),
        ("MayPlayTopCardOfLibraryWithoutPaying", []) => play_rule(
            top_of_libraries(you()),
            Z::Library,
            A::Either,
            ObjectFilter::Any,
            PlayTerms {
                cost: PlayCost::Free,
                ..terms
            },
        ),
        ("MayCastSpellsFromTopOfPlayersLibrary", [Filter(spells), Players(players)]) => play_rule(
            top_of_libraries(players.clone()),
            Z::Library,
            A::CastSpell,
            spells.clone(),
            terms,
        ),
        ("MayPlayLandsFromTopOfPlayersLibrary", [Players(players)]) => play_rule(
            top_of_libraries(players.clone()),
            Z::Library,
            A::PlayLand,
            ObjectFilter::Any,
            terms,
        ),
        ("MayCastExiledCardAndMaySpendColorlessManaAsThoughAnyColorToCast", [Object(card)]) => {
            play_rule(
                ObjectSet::One(card.clone()),
                Z::Exile,
                A::CastSpell,
                ObjectFilter::Any,
                PlayTerms {
                    spend_as: Some(SpendAs::ColorlessAsAnyColor),
                    ..terms
                },
            )
        }
        (
            "MayCastSpellsFromAmongExiledAndMaySpendManaAsThoughAnyTypeToCastAndAsThoughTheyHadFlash",
            [Filter(spells), Objects(cards)],
        ) => play_rule(
            cards.clone(),
            Z::Exile,
            A::CastSpell,
            spells.clone(),
            PlayTerms {
                spend_as: Some(SpendAs::AnyType),
                flash: true,
                ..terms
            },
        ),
        (
            "MayCastSpellsFromAmongExiledAndMaySpendManaFromSnowSourcesAsThoughItWereAnyColorToCast",
            [Objects(cards)],
        ) => play_rule(
            cards.clone(),
            Z::Exile,
            A::CastSpell,
            ObjectFilter::Any,
            PlayTerms {
                spend_as: Some(SpendAs::SnowAsAnyColor),
                ..terms
            },
        ),
        (
            "MayCastASpellOfEachNonlandCardtypeFromAmongExiledCardsWithoutPaying",
            [Objects(cards)],
        ) => play_rule(
            cards.clone(),
            Z::Exile,
            A::CastSpell,
            ObjectFilter::Any,
            PlayTerms {
                cost: PlayCost::Free,
                limit: Some(PlayLimit::OnePerCardType(TypeClass::Nonland)),
                ..terms
            },
        ),
        (
            "MayPlayALandOrCastASpellFromTheirGraveyardOfEachPermanentTypeDuringPlayersTurn",
            [Player(player)],
        ) => play_rule(
            graveyard_of(you()),
            Z::Graveyard,
            A::Either,
            ObjectFilter::Any,
            PlayTerms {
                limit: Some(PlayLimit::OnePerCardType(TypeClass::Permanent)),
                during: Some(PlayerSet::One(player.clone())),
                ..terms
            },
        ),
        (
            "MayCastGraveyardCardIntoExileIfSpellForIncreasedCost",
            [Object(card), Filter(spells), cost],
        ) => play_rule(
            ObjectSet::One(card.clone()),
            Z::Graveyard,
            A::CastSpell,
            spells.clone(),
            PlayTerms {
                cost: PlayCost::Additional(cost.clone()),
                into_exile: true,
                ..terms
            },
        ),
        (
            "MayLookAtAndPlayCardsFromTheTopOfOtherPlayersLibraryAndMaySpendManaAsThoughAnyColorToCast",
            [Player(player)],
        ) => play_rule(
            top_of_libraries(PlayerSet::One(player.clone())),
            Z::Library,
            A::Either,
            ObjectFilter::Any,
            PlayTerms {
                spend_as: Some(SpendAs::AnyColor),
                may_look: true,
                ..terms
            },
        ),
        (
            "MayCastSpellsFromTopOfLibraryAndSpellsOfTypeWithEffect",
            [Filter(spells), Filter(triggering), then],
        ) => play_rule(
            top_of_libraries(you()),
            Z::Library,
            A::CastSpell,
            spells.clone(),
            PlayTerms {
                effects: vec![Filter(triggering.clone()), then.clone()],
                ..terms
            },
        ),
        ("CanCastSpellsFromRevealedHandOfPlayer", [Player(player)]) => play_rule(
            zone_set(Z::Hand, PlayerSet::One(player.clone()), ObjectFilter::Any),
            Z::Hand,
            A::CastSpell,
            ObjectFilter::Any,
            terms,
        ),
        ("CanPlayLandsFromRevealedHandOfPlayer", [Player(player)]) => play_rule(
            zone_set(Z::Hand, PlayerSet::One(player.clone()), ObjectFilter::Any),
            Z::Hand,
            A::PlayLand,
            ObjectFilter::Any,
            terms,
        ),
        ("MayCastSpellsFromOtherPlayersGraveyards", []) => play_rule(
            graveyard_of(others()),
            Z::Graveyard,
            A::CastSpell,
            ObjectFilter::Any,
            terms,
        ),
        ("MayPlaysLandsFromOtherPlayersGraveyards", []) => play_rule(
            graveyard_of(others()),
            Z::Graveyard,
            A::PlayLand,
            ObjectFilter::Any,
            terms,
        ),
        ("MayCastSpellsFromGraveyardUsingTheirSneakAbility", [Filter(spells), Filter(cards)]) => {
            play_rule(
                zone_set(Z::Graveyard, you(), cards.clone()),
                Z::Graveyard,
                A::CastSpell,
                spells.clone(),
                PlayTerms {
                    cost: PlayCost::Using(CostedKeyword::Sneak),
                    ..terms
                },
            )
        }
        (
            "OnceEachTurnMayCastASpellFromAmongCardsInGraveyardsWithEffect",
            [Filter(spells), Filter(cards), then],
        ) => play_rule(
            zone_set(Z::Graveyard, PlayerSet::All, cards.clone()),
            Z::Graveyard,
            A::CastSpell,
            spells.clone(),
            PlayTerms {
                limit: Some(PlayLimit::EachTurn(1)),
                effects: vec![then.clone()],
                ..terms
            },
        ),
        _ => return None,
    })
}

/// Who attacks whom and who decides it.
fn reduce_combat_rule(bare: &str, operands: &[Operand]) -> Option<PlayerRule> {
    use AttackDefenders as D;
    use Operand::{Comparison as Compare, Condition as Cond, Object, Objects, Player};
    let walkers = |player: &PlayerRef| D::PlayersAndPlaneswalkers(PlayerSet::One(player.clone()));
    Some(match (bare, operands) {
        ("ChoosesHowCreaturesBlock", [Objects(objects)]) => {
            PlayerRule::Decides(GameDecision::HowCreaturesBlock(event_filter(objects)?))
        }
        ("ChoosesWhichCreaturesBlockAndHowTheyBlock", []) => {
            PlayerRule::Decides(GameDecision::WhichCreaturesBlock)
        }
        ("ChoosesWhichCreaturesAttack", []) => {
            PlayerRule::Decides(GameDecision::WhichCreaturesAttack)
        }
        ("ChoosesHowPlayersVote", []) => PlayerRule::Decides(GameDecision::HowPlayersVote),
        ("AssignsCombatDamageOfAttackingCreatures", [Objects(objects)]) => {
            PlayerRule::Decides(GameDecision::CombatDamageOf(event_filter(objects)?))
        }
        ("ControlsPlayersWhileTheyAreSearchingLibraries", [Operand::Players(players)]) => {
            PlayerRule::Decides(GameDecision::SearchesOf(players.clone()))
        }
        ("MustAttackWithEachCreature", [Objects(objects)]) => PlayerRule::MustAttack {
            attackers: objects_filter(objects),
            defenders: D::Any,
            count: None,
        },
        ("MustAttackPlaneswalkerWithEachCreature", [Object(walker), Objects(objects)]) => {
            PlayerRule::MustAttack {
                attackers: objects_filter(objects),
                defenders: D::Objects(ObjectFilter::SameObject(walker.clone())),
                count: None,
            }
        }
        ("MustAttackWithANumberOfCreatures", [Compare(comparison, value), Objects(objects)]) => {
            PlayerRule::MustAttack {
                attackers: objects_filter(objects),
                defenders: D::Any,
                count: Some((comparison.clone(), value.clone())),
            }
        }
        ("MustAttackPlayerOrPlaneswalkersControlledBy", [Player(player)]) => {
            PlayerRule::MustAttack {
                attackers: ObjectFilter::Any,
                defenders: walkers(player),
                count: None,
            }
        }
        ("MayAttackOnlyPlayerOrPlaneswalkersControlledBy", [Player(player)]) => {
            PlayerRule::AttackOnly(walkers(player))
        }
        ("CantAttackPlayerOrPlaneswalkerControlledBy", [Player(player)]) => {
            PlayerRule::Prohibit(PlayerAction::AttackDefender(walkers(player)))
        }
        ("CantBeAttackedExceptBy", [Objects(objects)]) => {
            PlayerRule::AttackedOnlyBy(event_filter(objects)?)
        }
        ("AttackingDoesntCauseCreaturesToTapIf", [Objects(objects), Cond(condition)]) => {
            PlayerRule::AttackWithoutTapping {
                attackers: event_filter(objects)?,
                condition: condition.clone(),
            }
        }
        ("CantBlockWithMoreThanOneCreature", []) => PlayerRule::BlockerLimit(Expr::Constant(1)),
        _ => return None,
    })
}

/// A set of permanents as a predicate; a chosen set stays that set.
fn objects_filter(objects: &ObjectSet) -> ObjectFilter {
    event_filter(objects).unwrap_or_else(|| ObjectFilter::InSet(Box::new(objects.clone())))
}

/// Mana: what it can be spent as, what it pays for, and what happens to it.
fn reduce_mana_rule(bare: &str, operands: &[Operand]) -> Option<PlayerRule> {
    let color_as = |color: &Operand, as_color, others_colorless, on| {
        Some(PlayerRule::SpendColorAs {
            color: color_form(color)?,
            as_color,
            others_colorless,
            on,
        })
    };
    match (bare, operands) {
        ("UnspentManaBecomesColorless", []) => Some(PlayerRule::UnspentManaBecomes(None)),
        ("UnspentManaBecomesColor", [color]) => {
            Some(PlayerRule::UnspentManaBecomes(Some(color_form(color)?)))
        }
        ("MaySpendColoredManaAsThoughItWereAnyColor", [color]) => {
            color_as(color, None, false, ManaPayment::Anything)
        }
        ("MaySpendColoredManaAsThoughItWereAnyColorToPayForAbilities", [color, abilities]) => {
            color_as(
                color,
                None,
                false,
                ManaPayment::Abilities(stack_filter(abilities)?),
            )
        }
        ("MaySpendColoredManaAsThoughItWereAnotherColor", [color, other]) => color_as(
            color,
            Some(color_form(other)?),
            false,
            ManaPayment::Anything,
        ),
        (
            "MaySpendColoredManaAsThoughItWereAnyColorAndMaySpendOtherManaOnlyAsThoughItWereColorless",
            [color],
        ) => color_as(color, None, true, ManaPayment::Anything),
        ("MaySpendManaAsThoughItWasAnyTypeToActivateAbilities", [abilities]) => {
            Some(PlayerRule::SpendManaAs {
                spend_as: SpendAs::AnyType,
                on: ManaPayment::Abilities(stack_filter(abilities)?),
            })
        }
        ("MayPayLifeRatherThanMana", [Operand::Number(life), symbol]) => {
            Some(PlayerRule::LifeForMana {
                life: life.clone(),
                symbol: Box::new(symbol.clone()),
            })
        }
        ("MayPayLifeToProduceManaAsManaAbility", [Operand::Number(life), mana]) => {
            Some(PlayerRule::LifeForManaAbility {
                life: life.clone(),
                mana: Box::new(mana.clone()),
            })
        }
        ("MayTapPermanentsTheyDontControlForManaWithModifiers", [Operand::Objects(set), modifier]) => {
            Some(PlayerRule::TapOthersForMana {
                permanents: event_filter(set)?,
                modifier: Box::new(modifier.clone()),
            })
        }
        ("AsLoseUnspentMana", [mana, then]) => Some(PlayerRule::AsManaEmpties {
            mana: Box::new(mana.clone()),
            then: Box::new(then.clone()),
        }),
        _ => None,
    }
}

/// Activated abilities: when they can be used and how often.
fn reduce_activation_rule(bare: &str, operands: &[Operand]) -> Option<PlayerRule> {
    use ActivationGrant as G;
    use Operand::{Number, Object, Objects, Players};
    let is = |trait_| ObjectFilter::OnStack(trait_);
    let of = |set: &ObjectSet| Some(ObjectFilter::AbilityOf(Box::new(event_filter(set)?)));
    let own = |object: &ObjectRef| {
        ObjectFilter::AbilityOf(Box::new(ObjectFilter::SameObject(object.clone())))
    };
    let loyalty = |source| ObjectFilter::All(vec![is(StackTrait::Loyalty), source]);
    let grant = |abilities, grant| PlayerRule::ActivationGrant { abilities, grant };
    let keyword = |word| is(StackTrait::Keyword(word));
    Some(match (bare, operands) {
        ("MayActivateAbilitiesOfCreaturesAsThoughTheyHadHaste", [Objects(set)]) => {
            grant(of(set)?, G::AsThoughHaste)
        }
        (
            "MayActivateLoyaltyAbilityOfPlaneswalkersDuringEachPlayersTurnAndAnyTimeTheyCouldCastAnInstant",
            [Objects(set), Players(players)],
        ) => grant(loyalty(of(set)?), G::InstantSpeedDuring(players.clone())),
        (
            "MayActivateLoyaltyAbilityOfPlaneswalkerDuringEachPlayersTurnAndAnyTimeTheyCouldCastAnInstant",
            [Object(object), Players(players)],
        ) => grant(loyalty(own(object)), G::InstantSpeedDuring(players.clone())),
        ("MayActivateLoyaltyAbilityOfPlaneswalkerAnyTimeTheyCouldCastAnInstant", [Object(object)]) => {
            grant(loyalty(own(object)), G::InstantSpeed)
        }
        ("MayActivateEquipAbilitiesAnyTimeTheyCouldCastAnInstant", []) => {
            grant(keyword(AbilityKeyword::Equip), G::InstantSpeed)
        }
        ("MayActivateLoyaltyAbilitiesTwiceEachTurn", [Objects(set)]) => grant(
            loyalty(of(set)?),
            G::AdditionalTimes(Expr::Constant(1)),
        ),
        ("MayActivateLoyaltyAbilitiesOfPlanewalkerTwice", [Object(object)])
        | ("MayActivateLoyaltyAbilityOfPlanewalkerAnAdditionalTime", [Object(object)]) => grant(
            loyalty(own(object)),
            G::AdditionalTimes(Expr::Constant(1)),
        ),
        ("MayActivateLoyaltyAbilityOfPlanewalkerAnAdditionalNumberTimes", [Object(object), Number(n)]) => {
            grant(loyalty(own(object)), G::AdditionalTimes(n.clone()))
        }
        ("MayActivateLoyaltyAbilityOfPlanewalkersAnAdditionalTime", [Objects(set)]) => grant(
            loyalty(of(set)?),
            G::AdditionalTimes(Expr::Constant(1)),
        ),
        ("MayActivatePowerUpAbilitiesOfPermanentsAnAdditionalTime", [Objects(set)]) => grant(
            ObjectFilter::All(vec![keyword(AbilityKeyword::PowerUp), of(set)?]),
            G::AdditionalTimes(Expr::Constant(1)),
        ),
        ("MayActivateExhaustAbilitiesAsThoughTheyHaventBeenActivated", []) => {
            grant(keyword(AbilityKeyword::Exhaust), G::AsThoughNotActivated)
        }
        ("CanActivateAbilitiesOnlyDuringTheirTurn", []) => {
            PlayerRule::ActivationTiming(SpellTiming::OwnTurnOnly)
        }
        _ => return None,
    })
}

/// What the player can't do, or can't be made to do.
fn reduce_prohibition(bare: &str, operands: &[Operand]) -> Option<PlayerRule> {
    use Operand::Objects;
    let prohibit = |action| Some(PlayerRule::Prohibit(action));
    let caused = |action, by: &Operand| {
        Some(PlayerRule::CantBeCaused {
            action,
            by: stack_filter(by)?,
        })
    };
    match (bare, operands) {
        ("CanOnlyCastSpellsFromThierHand", []) => {
            prohibit(PlayerAction::CastOutside(EventZone::Hand))
        }
        ("CantActivateAbilitiesOfCardsInGraveyards", []) => {
            prohibit(PlayerAction::Activate(ActivationFilter::Of(Box::new(
                ObjectFilter::InZone(ZoneScope::Players {
                    zone: EventZone::Graveyard,
                    players: PlayerSet::All,
                }),
            ))))
        }
        ("CantPlayCardInHand", [Operand::Object(card)]) => prohibit(PlayerAction::PlayCard(
            ObjectFilter::SameObject(card.clone()),
        )),
        ("CantPlayCardsFromHand", []) => prohibit(PlayerAction::PlayCard(ObjectFilter::InZone(
            ZoneScope::Players {
                zone: EventZone::Hand,
                players: you(),
            },
        ))),
        ("CantVentureIntoTheDungeonMoreThanOnceEachTurn", []) => Some(PlayerRule::LimitPerTurn {
            action: PlayerAction::VentureIntoDungeon,
            amount: Expr::Constant(1),
        }),
        ("CantBeTheTargetOfSpellsOrAbilities", [sources]) => {
            prohibit(PlayerAction::BeTargeted(stack_filter(sources)?))
        }
        ("CantBeCausedToSacrificePermanentsBySpellAndAbilities", [Objects(set), by])
        | ("CantBeCausedToSacrificePermanentsByAbilities", [Objects(set), by]) => caused(
            PlayerAction::Sacrifice {
                objects: event_filter(set)?,
                paying: None,
            },
            by,
        ),
        ("CantBeCausedToExilePermanentsByAbilities", [Objects(set), by]) => {
            caused(PlayerAction::ExileObjects(event_filter(set)?), by)
        }
        ("CantBeCausedToDiscardCardsBySpellAndAbilities", [by]) => {
            caused(PlayerAction::DiscardCards, by)
        }
        ("SpellsAndAbilitiesTheyCantCantCauseThemToSearchTheirLibrary", []) => {
            Some(PlayerRule::CantBeCaused {
                action: PlayerAction::SearchLibraries,
                by: ObjectFilter::Not(Box::new(ObjectFilter::ControlledBy(you()))),
            })
        }
        _ => None,
    }
}

/// Everything else a static effect can give a player.
fn reduce_player_static(bare: &str, operands: &[Operand]) -> Option<PlayerRule> {
    use Operand::{Number, Objects, Players};
    let each_turn = || Some(PlayLimit::EachTurn(1));
    if let Some(rule) = reduce_keyword_cost(bare, operands)
        .or_else(|| reduce_play_rule(bare, operands))
        .or_else(|| reduce_combat_rule(bare, operands))
        .or_else(|| reduce_mana_rule(bare, operands))
        .or_else(|| reduce_activation_rule(bare, operands))
        .or_else(|| reduce_prohibition(bare, operands))
    {
        return Some(rule);
    }
    Some(match (bare, operands) {
        ("OnceEachTurnMayAction", [body]) => {
            special_action(body, None, each_turn(), None, false)
        }
        ("MayActionOnce", [body]) => {
            special_action(body, None, Some(PlayLimit::Total(1)), None, false)
        }
        ("OnceDuringEachPlayersTurnMayAction", [Operand::Player(player), body]) => special_action(
            body,
            None,
            each_turn(),
            Some(PlayerSet::One(player.clone())),
            false,
        ),
        ("MayPayManaAnyTimeTheyCouldCastAnInstant", [cost, body]) => {
            special_action(body, Some(cost), None, None, true)
        }
        ("MayDiscardCardAnyTimeTheyCouldCastAnInstant", [_]) => {
            PlayerRule::Unique(UniquePlayerRule::DiscardThisAtInstantSpeed)
        }
        ("GetsAnOptionalAdditionalVote", []) => PlayerRule::AdditionalVote { optional: true },
        ("GetsAnAdditionalVote", []) => PlayerRule::AdditionalVote { optional: false },
        ("RevealsEachCardDrawn", []) => PlayerRule::RevealDraws {
            first_during: None,
            optional: false,
        },
        ("MayRevealFirstCardDrawnDuringEachPlayersTurn", [Players(players)]) => {
            PlayerRule::RevealDraws {
                first_during: Some(players.clone()),
                optional: true,
            }
        }
        ("RevealFirstCardDrawnDuringEachPlayersTurn", [Players(players)]) => {
            PlayerRule::RevealDraws {
                first_during: Some(players.clone()),
                optional: false,
            }
        }
        ("RevealFirstCardDrawnDuringPlayersTurn", [Operand::Player(player)]) => {
            PlayerRule::RevealDraws {
                first_during: Some(PlayerSet::One(player.clone())),
                optional: false,
            }
        }
        ("PlaysWithCardInHandRevealed", [Operand::Object(card)]) => PlayerRule::Visibility {
            objects: ObjectSet::One(card.clone()),
            audience: VisibilityAudience::Everyone,
        },
        ("WhileChoosingTargetsMustChooseAtLeastOnePermanentIfAble", [Objects(set)]) => {
            PlayerRule::TargetsMustInclude(event_filter(set)?)
        }
        ("CanBeTheTargetOfSpellsAndAbilitiesAsThoughTheyDidntHaveHexproof", [sources]) => {
            PlayerRule::TargetableDespite {
                sources: stack_filter(sources)?,
                keyword: TargetingKeyword::Hexproof,
            }
        }
        ("DrawsACardDuringEachPlayersDrawStep", [Players(players)]) => {
            PlayerRule::DrawsDuringDrawStepOf(players.clone())
        }
        ("UntapsPermanentsDuringEachPlayersUntapSteps", [Objects(set), Players(players)]) => {
            PlayerRule::UntapsDuringUntapStepOf {
                permanents: event_filter(set)?,
                players: players.clone(),
            }
        }
        ("CanOnlyUntapCardsOfTypeOfTheirChoiceDuringTheirUntapStep", [Operand::List(types)]) => {
            PlayerRule::UntapOnlyOneOfTypes(types.clone())
        }
        ("OnceEachTurnMayPayToRerollAnyNumberOfDiceRolled", [cost]) => PlayerRule::DiceChange {
            cost: Box::new(cost.clone()),
            limit: each_turn(),
            reroll: true,
        },
        ("OnceEachTurnMayPayToIncrementOrDecrementADiceRoll", [cost]) => PlayerRule::DiceChange {
            cost: Box::new(cost.clone()),
            limit: each_turn(),
            reroll: false,
        },
        ("MayPayToIncrementOrDecrementADiceRoll", [cost]) => PlayerRule::DiceChange {
            cost: Box::new(cost.clone()),
            limit: None,
            reroll: false,
        },
        ("MayLookAtAnAdditionalNumberCardsAsTheySurveil", [Number(n)]) => {
            PlayerRule::SurveilBonus(n.clone())
        }
        ("IncreaseDevotionToColorAndColorCombinationsByNumber", [Number(n)]) => {
            PlayerRule::DevotionBonus(n.clone())
        }
        (
            "AsTheyCascadeTheyMayPutACardFromAmongExiledCardsOnBattlefiled",
            [Operand::Filter(filter), flags],
        ) => PlayerRule::CascadePut {
            filter: filter.clone(),
            flags: Box::new(flags.clone()),
        },
        ("MayPayToReduceColoredManaCostOfSpells", [cost, mana, Operand::Filter(spells)]) => {
            PlayerRule::SpellCost {
                spells: spells.clone(),
                modifier: SpellCostModifier::PayToReduce {
                    cost: Box::new(cost.clone()),
                    mana: Box::new(mana.clone()),
                },
            }
        }
        ("IncreaseSpellCostForEach", [Operand::Filter(spells), cost, Number(per)]) => {
            PlayerRule::SpellCost {
                spells: spells.clone(),
                modifier: SpellCostModifier::Scaled {
                    change: ActivationCostChange::Increase(Box::new(cost.clone())),
                    per: CostScale::Times(per.clone()),
                },
            }
        }
        ("DecreaseSpellCostForEachTarget", [Operand::Filter(spells), amount]) => {
            PlayerRule::SpellCost {
                spells: spells.clone(),
                modifier: SpellCostModifier::Scaled {
                    change: ActivationCostChange::Reduce(Box::new(amount.clone())),
                    per: CostScale::EachTarget,
                },
            }
        }
        ("IncreaseSpellCostForEachTarget", [Operand::Filter(spells), cost]) => {
            PlayerRule::SpellCost {
                spells: spells.clone(),
                modifier: SpellCostModifier::Scaled {
                    change: ActivationCostChange::Increase(Box::new(cost.clone())),
                    per: CostScale::EachTarget,
                },
            }
        }
        ("MayCastASpellForAlternateCostOnceDuringEachPlayersTurn", [Operand::Filter(spells), cost, Players(players)]) => {
            PlayerRule::SpellCost {
                spells: spells.clone(),
                modifier: SpellCostModifier::AlternateDuring {
                    cost: PlayCost::Alternate(cost.clone()),
                    limit: each_turn(),
                    during: players.clone(),
                },
            }
        }
        ("MayPayAdditionalCostToCastSpellsForEffect", [Operand::Filter(spells), cost, effects]) => {
            PlayerRule::SpellCost {
                spells: spells.clone(),
                modifier: SpellCostModifier::OptionalAdditional {
                    cost: Box::new(cost.clone()),
                    effects: Box::new(effects.clone()),
                },
            }
        }
        ("SpellsCastFromExileHaveAbility", [Operand::Filter(spells), list]) => {
            PlayerRule::SpellsHaveAbilities {
                spells: spells.clone(),
                from: Some(EventZone::Exile),
                nth_each_turn: None,
                abilities: Box::new(list.clone()),
            }
        }
        ("SpellsCastGainAbility", [Operand::Filter(spells), list]) => {
            PlayerRule::SpellsHaveAbilities {
                spells: spells.clone(),
                from: None,
                nth_each_turn: None,
                abilities: Box::new(list.clone()),
            }
        }
        ("MayPlotCardsFromTheTopOfTheirLibrary", [Operand::Filter(cards)]) => {
            PlayerRule::KeywordAccess {
                keyword: CostedKeyword::Plot,
                cards: cards.clone(),
                from: EventZone::Library,
                during: None,
            }
        }
        ("MaySuspendCardsFromHand", [Operand::Filter(cards)]) => PlayerRule::KeywordAccess {
            keyword: CostedKeyword::Suspend,
            cards: cards.clone(),
            from: EventZone::Hand,
            during: None,
        },
        ("CanForetellCardsDuringEachPlayersTurn", [Players(players)]) => {
            PlayerRule::KeywordAccess {
                keyword: CostedKeyword::Foretell,
                cards: ObjectFilter::Any,
                from: EventZone::Hand,
                during: Some(players.clone()),
            }
        }
        ("FirstTwoCoinFlipsEachTurnAreHeadsAndYouWin", []) => {
            PlayerRule::Unique(UniquePlayerRule::FirstTwoCoinFlipsWin)
        }
        ("DrawsCardsFromBottomOfTheirLibrary", []) => {
            PlayerRule::Unique(UniquePlayerRule::DrawsFromBottomOfLibrary)
        }
        ("GainsLifeRatherThanLoseLifeFromRadiation", []) => {
            PlayerRule::Unique(UniquePlayerRule::RadiationGainsLife)
        }
        ("MayPlayColoredCardsFromHandAsRandomBasicLandOfThatCouldProduceOneOfThoseColors", []) => {
            PlayerRule::Unique(UniquePlayerRule::ColoredCardsAsRandomBasicLands)
        }
        ("NoteManaValueOfExiledCard", []) => {
            PlayerRule::Unique(UniquePlayerRule::NoteExiledManaValue)
        }
        _ => return None,
    })
}

/// Rules on a permanent, past the prohibitions `reduce_rule` already reads.
fn reduce_object_static(bare: &str, operands: &[Operand]) -> Option<RuleChange> {
    use ObjectAction as A;
    use Operand::{Condition as Cond, Number, Objects, Players};
    use RuleChange as R;
    let prohibit = |action| Some(R::Prohibit(action));
    let attached = |attachments, keep_existing| {
        Some(R::Prohibit(A::BeAttachedBy {
            attachments,
            keep_existing,
        }))
    };
    let despite = |attackers, evasion| Some(R::BlocksDespite { attackers, evasion });
    let unique = |rule| Some(R::Unique(rule));
    let with_keyword = |form| ObjectFilter::Subtype(form);
    match (bare, operands) {
        ("CanBeTheTargetOfSpellsAndAbilitiesAsThoughTheyDidntHaveHexproof", [sources]) => {
            Some(R::TargetableDespite {
                sources: stack_filter(sources)?,
                keyword: TargetingKeyword::Hexproof,
            })
        }
        ("CanBeTheTargetOfSpellsOrAbilitiesAsThoughItDidntHaveShroud", [sources]) => {
            Some(R::TargetableDespite {
                sources: stack_filter(sources)?,
                keyword: TargetingKeyword::Shroud,
            })
        }
        ("CanBeAttachedOnlyToAPermanent", [Objects(set)]) => {
            Some(R::AttachOnlyTo(event_filter(set)?))
        }
        ("CantBlockUnlessAttackingPlayer", [Cond(condition)]) => {
            Some(R::BlocksOnlyIf(condition.clone()))
        }
        ("CantBeBlockedUnlessDefendingPlayer", [Players(players)]) => Some(R::BlockedByDefender {
            players: players.clone(),
            unless: true,
        }),
        ("CantBeBlockedIfDefendingPlayer", [Players(players)]) => Some(R::BlockedByDefender {
            players: players.clone(),
            unless: false,
        }),
        ("AttackingCausesOthersToAttack", [Objects(set)]) => {
            Some(R::AttackDrags(event_filter(set)?))
        }
        ("MustAttackIfAnotherCreatureAttacks", [Objects(set)]) => {
            Some(R::AttacksWhenAttacking(event_filter(set)?))
        }
        ("CanBlockCreaturesWithShadowAsThoughTheyDidntHaveShadow", [])
        | ("CanBlockCreaturesWithShadowAsThoughItHadShadow", []) => {
            despite(ObjectFilter::Any, BlockEvasion::Shadow)
        }
        ("CanBlockCreaturesWithFlyingAsThoughItHadReach", [Objects(set)]) => {
            despite(event_filter(set)?, BlockEvasion::Flying)
        }
        ("CanBlockCreaturesWithLandwalkAbilitiesAsThoughTheyDidntHaveThem", []) => {
            despite(ObjectFilter::Any, BlockEvasion::Landwalk)
        }
        ("CantBeEnchantedByAnEnchantment", [Objects(set)]) => {
            attached(event_filter(set)?, false)
        }
        ("CantBeEnchantedAndDoesntRemove", [Objects(set)]) => attached(event_filter(set)?, true),
        ("CantBeEnchanted", []) => attached(with_keyword(crate::mtg_types::SubType::Aura), false),
        ("CantBeEquipped", []) => attached(with_keyword(crate::mtg_types::SubType::Equipment), false),
        ("CrewsVehiclesUsingToughnessRatherThanPower", []) => {
            Some(R::TapCostUsesToughness(TapPowerCost::Crew))
        }
        ("SaddlesMountsUsingToughnessRatherThanPower", []) => {
            Some(R::TapCostUsesToughness(TapPowerCost::Saddle))
        }
        ("StationsPermanentsUsingToughnessRatherThanPower", []) => {
            Some(R::TapCostUsesToughness(TapPowerCost::Station))
        }
        ("StationsPermanentsAsThoughPowerWereGreater", [Number(bonus)]) => Some(R::TapPowerBonus {
            cost: TapPowerCost::Station,
            bonus: bonus.clone(),
        }),
        ("CantBeTurnedFaceUp", []) => prohibit(A::TurnFaceUp),
        ("CantBecomeSuspected", []) => prohibit(A::BecomeSuspected),
        ("CantBeBeamedUp", []) => prohibit(A::BeBeamedUp),
        ("CantBeGainedControlOf", []) => prohibit(A::ChangeControl),
        ("PlaneswalkerCantBeAttacked", []) => prohibit(A::BeAttacked),
        ("CantHaveCountersOfTypeRemovedFromIt", [kind]) => {
            prohibit(A::LoseCounters(counter_kind(kind)?))
        }
        ("CantHaveMoreThanNumberCountersOfType", [Number(max), kind]) => Some(R::CounterLimit {
            kind: counter_kind(kind)?,
            max: max.clone(),
        }),
        ("CanBoastTwice", []) => Some(R::ActivationsEachTurn {
            abilities: ObjectFilter::OnStack(StackTrait::Keyword(AbilityKeyword::Boast)),
            times: Expr::Constant(2),
        }),
        ("DecreaseEquipAbilityCostWhenTargetingAPermanent", [amount, Objects(set)]) => {
            Some(R::OwnActivationCost {
                abilities: ObjectFilter::All(vec![
                    ObjectFilter::OnStack(StackTrait::Keyword(AbilityKeyword::Equip)),
                    ObjectFilter::TargetsAny(Box::new(Recipients::Objects(set.clone()))),
                ]),
                change: ActivationCostChange::Reduce(Box::new(amount.clone())),
            })
        }
        (
            "MayAssignCombatDamageDividedAsYouChooseAmongPlayerOrCreaturesAndPlaneswalkers",
            [Operand::Player(player), Objects(set)],
        ) => Some(R::AssignDamageTo {
            players: Some(player.clone()),
            permanents: event_filter(set)?,
        }),
        ("MayAssignCombatDamageToAPermanent", [Objects(set)]) => Some(R::AssignDamageTo {
            players: None,
            permanents: event_filter(set)?,
        }),
        ("MustBeBlockedByExactlyOneDefender", [Objects(_)]) => {
            unique(UniqueObjectRule::BlockedByExactlyOne)
        }
        ("IsAColorlessSourceOfDamage", []) => unique(UniqueObjectRule::ColorlessSourceOfDamage),
        ("CantBecomeTappedUnlessItIsBeingDeclaredAsAnAttacker", []) => {
            unique(UniqueObjectRule::TapsOnlyToAttack)
        }
        ("CantBeBlockedUnlessAllDefendersBlockIt", []) => {
            unique(UniqueObjectRule::BlockedOnlyByAll)
        }
        ("CanOnlyBeDestroyedByLethalDamageFromASingleSource", []) => {
            unique(UniqueObjectRule::DestroyedOnlyBySingleSource)
        }
        ("ArentPutIntoGraveyardForHaving0Loyalty", []) => {
            unique(UniqueObjectRule::SurvivesZeroLoyalty)
        }
        ("DamageDealtToItCantBePreventedOrRedirected", []) => {
            unique(UniqueObjectRule::DamageToItUnpreventable)
        }
        ("DetermineLethalDamageUsingPowerRatherThanToughness", []) => {
            unique(UniqueObjectRule::LethalDamageUsesPower)
        }
        ("AssignsCombatDamageAsThoughPowerWerePositive", []) => {
            unique(UniqueObjectRule::AssignsAsThoughPowerPositive)
        }
        _ => None,
    }
}

fn mana_form(form: &str, operands: &[Operand]) -> Option<ManaSymbol> {
    use ManaColor::{Black, Blue, Colorless, Green, Red, White};
    let hybrid = |a, b| Some(ManaSymbol::Hybrid(a, b));
    let color = |c| Some(ManaSymbol::Color(c));
    match (form, operands) {
        ("ManaCostGeneric", [Operand::Integer(n)]) => {
            Some(ManaSymbol::Generic((*n).try_into().ok()?))
        }
        ("ManaCostX", []) => Some(ManaSymbol::X),
        ("ManaCostW", []) => color(White),
        ("ManaCostU", []) => color(Blue),
        ("ManaCostB", []) => color(Black),
        ("ManaCostR", []) => color(Red),
        ("ManaCostG", []) => color(Green),
        ("ManaCostC", []) => color(Colorless),
        ("ManaCostS", []) => Some(ManaSymbol::Snow),
        ("ManaCostWU", []) => hybrid(White, Blue),
        ("ManaCostUB", []) => hybrid(Blue, Black),
        ("ManaCostBR", []) => hybrid(Black, Red),
        ("ManaCostRG", []) => hybrid(Red, Green),
        ("ManaCostGW", []) => hybrid(Green, White),
        ("ManaCostWB", []) => hybrid(White, Black),
        ("ManaCostUR", []) => hybrid(Blue, Red),
        ("ManaCostBG", []) => hybrid(Black, Green),
        ("ManaCostRW", []) => hybrid(Red, White),
        ("ManaCostGU", []) => hybrid(Green, Blue),
        _ => None,
    }
}

/// Mana symbols a row left as terms, flat or in lists.
fn mana_terms(operands: &[Operand]) -> Option<Vec<ManaSymbol>> {
    let mut symbols = vec![];
    for operand in operands {
        match operand {
            Operand::List(items) => symbols.extend(mana_terms(items)?),
            Operand::Term { form, operands, .. } => symbols.push(mana_form(form, operands)?),
            Operand::Vocabulary { form, .. } => symbols.push(mana_form(form, &[])?),
            _ => return None,
        }
    }
    Some(symbols)
}

fn mana_cost(operands: &[Operand]) -> Option<Cost> {
    Some(Cost {
        parts: vec![CostPart::Mana(mana_terms(operands)?)],
    })
}

fn vocabulary_value<T: serde::de::DeserializeOwned>(operand: &Operand) -> Option<T> {
    serde_json::from_value(serde_json::Value::String(vocabulary_form(operand)?.into())).ok()
}

/// A color a card effect sets or adds, chosen or listed.
fn settable_color(operand: &Operand, mode: ChangeMode) -> Option<Change> {
    let Operand::Term { form, operands, .. } = operand else {
        return None;
    };
    match (form.as_str(), operands.as_slice()) {
        ("TheChosenColor", []) => Some(Change::ChosenColor { mode }),
        ("Colorless", []) => Some(Change::Colors {
            mode,
            colors: vec![],
        }),
        ("SimpleColorList", colors) => Some(Change::Colors {
            mode,
            colors: colors.iter().map(color_form).collect::<Option<_>>()?,
        }),
        _ => None,
    }
}

/// One card effect: what a zone-wide or self rule changes on a card.
fn card_change(form: &str, operands: &[Operand]) -> Option<Change> {
    use Change as C;
    let unique = |rule| Some(C::Rule(RuleChange::Unique(rule)));
    match (form, operands) {
        ("AddCardtype", [kind]) => Some(C::AddCardType(vocabulary_value(kind)?)),
        ("AddLandType", [kind]) => Some(C::AddLandType(vocabulary_value(kind)?)),
        ("AddSupertype", [kind]) => Some(C::AddSupertype(vocabulary_value(kind)?)),
        ("AddCreatureType", [kind]) => Some(C::CreatureTypes {
            mode: ChangeMode::Add,
            types: CreatureTypeSet::Listed(vec![vocabulary_value(kind)?]),
        }),
        ("HasAllCreatureTypes", []) => Some(C::CreatureTypes {
            mode: ChangeMode::Add,
            types: CreatureTypeSet::All,
        }),
        ("AddCreatureTypeVariable", [variable]) => Some(C::ChosenType {
            kind: chosen_kind(variable)?,
            mode: ChangeMode::Add,
        }),
        ("SetCreatureTypeVariable", [variable]) => Some(C::ChosenType {
            kind: chosen_kind(variable)?,
            mode: ChangeMode::Set,
        }),
        ("AddColor", [color]) => settable_color(color, ChangeMode::Add),
        ("SetColor", [color]) => settable_color(color, ChangeMode::Set),
        ("AdjustPT", [Operand::Integer(power), Operand::Integer(toughness)]) => Some(C::Pump {
            power: Expr::Constant(*power),
            toughness: Expr::Constant(*toughness),
        }),
        ("SetPT", [Operand::Term { operands, .. }]) => match operands.as_slice() {
            [Operand::Integer(power), Operand::Integer(toughness)] => {
                Some(C::SetBasePowerToughness {
                    power: Expr::Constant(*power),
                    toughness: Expr::Constant(*toughness),
                })
            }
            _ => None,
        },
        ("CantBePlayed", []) => Some(C::Rule(RuleChange::Prohibit(ObjectAction::BePlayed))),
        ("CantBeTheTargetOfSpellsOrAbilities", [sources]) => Some(C::Rule(RuleChange::Prohibit(
            ObjectAction::BeTargeted(stack_filter(sources)?),
        ))),
        ("CountersRemainOnCardAsItMovesBetweenZonesExceptforHandAndLibrary", []) => {
            unique(UniqueObjectRule::CountersFollowBetweenZones)
        }
        ("MayCastFromLibraryWhileSearchingLibrary", []) => {
            unique(UniqueObjectRule::CastWhileSearched)
        }
        _ => None,
    }
}

fn abilities_of(operands: &[Operand]) -> Option<Vec<Ability>> {
    ability_list(operands)
}

fn keyword_for(mechanic: Mechanic, cost: Option<Cost>, amount: Option<Expr>) -> Ability {
    Ability::Mechanic {
        mechanic,
        cost,
        amount,
        filter: None,
    }
}

fn modified(ability: Ability, modifier: &Operand) -> Ability {
    Ability::Modified {
        ability: Box::new(ability),
        modifier: Box::new(modifier.clone()),
    }
}

/// Keywords with an {X} in their cost, or a modifier on it.
fn reduce_x_keyword(bare: &str, operands: &[Operand]) -> Option<Ability> {
    use Mechanic as M;
    let plain = |mechanic| Some(keyword_for(mechanic, Some(mana_cost(operands)?), None));
    match (bare, operands) {
        ("MiracleX", _) => plain(M::Miracle),
        ("MorphX", _) => plain(M::Morph),
        ("KickerX", _) => plain(M::Kicker),
        ("HarmonizeX", _) => plain(M::Harmonize),
        ("DisguiseX", _) => plain(M::Disguise),
        ("ReinforceX", _) => Some(keyword_for(
            M::Reinforce,
            Some(mana_cost(operands)?),
            Some(Expr::AnnouncedX),
        )),
        ("SuspendX", [symbols, modifier]) => Some(modified(
            keyword_for(
                M::Suspend,
                Some(mana_cost(std::slice::from_ref(symbols))?),
                Some(Expr::AnnouncedX),
            ),
            modifier,
        )),
        ("KickerXWithModifiers", [symbols, modifier]) => Some(modified(
            keyword_for(M::Kicker, Some(mana_cost(std::slice::from_ref(symbols))?), None),
            modifier,
        )),
        ("FlashbackWithModifier", [Operand::Cost(cost), modifier]) => Some(modified(
            keyword_for(M::Flashback, Some(cost.clone()), None),
            modifier,
        )),
        ("SpecializeWithModifiers", [Operand::Cost(cost), modifier]) => Some(modified(
            keyword_for(M::Specialize, Some(cost.clone()), None),
            modifier,
        )),
        _ => None,
    }
}

/// Rules of a whole card or zone, and things done as an event happens.
fn reduce_rule_ability(name: &str, operands: &[Operand]) -> Option<Ability> {
    use Operand::{Cost as CostOf, Filter, Number, Object, Objects, Player, Players, Text};
    let bare = name.strip_prefix("Rule::")?;
    if let Some(ability) = reduce_x_keyword(bare, operands) {
        return Some(ability);
    }
    let everywhere = |filter: ObjectFilter, changes| Ability::Static {
        kind: StaticKind::Ordinary,
        affected: EffectSubjects::Objects(ObjectSet::AnyZone(filter)),
        changes,
    };
    let source = || ObjectFilter::SameObject(ObjectRef::Source);
    let unpreventable = |sources, combat_only| {
        Some(Ability::DamageCantBePrevented {
            sources,
            combat_only,
        })
    };
    let cant_enter = |cards: &ObjectFilter, from| {
        Some(Ability::CantEnter {
            cards: cards.clone(),
            from,
        })
    };
    let from_zone = |zone, players: &PlayerSet| ZoneScope::Players {
        zone,
        players: players.clone(),
    };
    let as_event = |object: &ObjectRef, event, actions: &Operand| {
        Some(Ability::AsEvent {
            object: object.clone(),
            event,
            actions: Box::new(actions.clone()),
        })
    };
    let before_game = |when, body: &[Operand]| {
        Some(Ability::BeforeGame {
            when,
            body: Box::new(Operand::List(body.to_vec())),
        })
    };
    let unique = |rule| Some(Ability::Unique(rule));
    let keyword_cost_rule = |keyword, change| Ability::Static {
        kind: StaticKind::Ordinary,
        affected: EffectSubjects::Players(PlayerSet::All),
        changes: vec![Change::PlayerRule(PlayerRule::KeywordCost {
            keyword,
            cards: ObjectFilter::Any,
            change,
            per: None,
            first_each_turn: false,
        })],
    };
    match (bare, operands) {
        ("CardEffect", [Filter(cards), Operand::List(effects)]) => {
            Some(everywhere(cards.clone(), graveyard_changes(effects)?))
        }
        ("SelfEffect", effects) => Some(everywhere(source(), graveyard_changes(effects)?)),
        ("SelfEffect_NonBattlefield", effects) => Some(Ability::Scoped {
            zones: vec![
                EventZone::Hand,
                EventZone::Library,
                EventZone::Graveyard,
                EventZone::Exile,
                EventZone::Stack,
                EventZone::Command,
            ],
            condition: None,
            ability: Box::new(everywhere(source(), graveyard_changes(effects)?)),
        }),
        (
            "EachCardInEachPlayersHandEffect",
            [Filter(cards), Players(owners), Operand::List(effects)],
        ) => Some(zone_static(
            from_zone(EventZone::Hand, owners),
            cards,
            graveyard_changes(effects)?,
        )),
        (
            "EachCardInPlayersLibraryEffect",
            [Filter(cards), Player(owner), Operand::List(effects)],
        ) => Some(zone_static(
            from_zone(EventZone::Library, &PlayerSet::One(owner.clone())),
            cards,
            graveyard_changes(effects)?,
        )),
        (
            "TopCardOfPlayersLibraryEffect",
            [Filter(cards), Player(owner), Operand::List(effects)],
        ) => Some(Ability::Static {
            kind: StaticKind::Ordinary,
            affected: EffectSubjects::Objects(ObjectSet::Matching {
                objects: Box::new(top_of_libraries(PlayerSet::One(owner.clone()))),
                filter: cards.clone(),
            }),
            changes: graveyard_changes(effects)?,
        }),
        ("DamageCantBePrevented", []) => unpreventable(None, false),
        ("CombatDamageCantBePrevented", []) => unpreventable(None, true),
        ("DamageFromPermanentCantBePrevented", [Object(object)]) => {
            unpreventable(Some(ObjectSet::One(object.clone())), false)
        }
        ("CombatDamageOfCreaturesCantBePrevented", [Objects(objects)]) => {
            unpreventable(Some(objects.clone()), true)
        }
        ("CardsCantEnterTheBattlefield", [Filter(cards)]) => cant_enter(cards, None),
        ("CardsInEachPlayersGraveyardsCantEnterTheBattlefield", [Filter(cards), Players(owners)]) => {
            cant_enter(cards, Some(from_zone(EventZone::Graveyard, owners)))
        }
        ("CardsInEachPlayersLibrariesCantEnterTheBattlefield", [Filter(cards), Players(owners)]) => {
            cant_enter(cards, Some(from_zone(EventZone::Library, owners)))
        }
        ("PermanentsCantPhaseIn", [Objects(objects)]) => Some(Ability::Static {
            kind: StaticKind::Ordinary,
            affected: EffectSubjects::Objects(objects.clone()),
            changes: vec![Change::Rule(RuleChange::Prohibit(ObjectAction::PhaseIn))],
        }),
        ("EnchantGraveyardCard", [Filter(cards), Players(owners)]) => Some(Ability::Enchant {
            hosts: EnchantHosts::GraveyardCards {
                filter: cards.clone(),
                owners: owners.clone(),
            },
        }),
        ("NoMoreThanNumberCreaturesCanAttackPlayer", [Number(amount), Player(player)]) => {
            Some(Ability::AttackCap {
                defender: Recipients::Players(PlayerSet::One(player.clone())),
                amount: amount.clone(),
            })
        }
        ("NoMoreThanNumberCreaturesCanAttackPermanent", [Number(amount), Object(object)]) => {
            Some(Ability::AttackCap {
                defender: Recipients::Objects(ObjectSet::One(object.clone())),
                amount: amount.clone(),
            })
        }
        ("AsPermanentTransforms", [Object(object), actions]) => {
            as_event(object, AsEvent::Transforms, actions)
        }
        ("AsPermanentEntersOrIsTurnedFaceUp", [Object(object), actions]) => {
            as_event(object, AsEvent::EntersOrTurnsFaceUp, actions)
        }
        ("AsPermanentIsTurnedFaceUp", [Object(object), actions]) => {
            as_event(object, AsEvent::TurnsFaceUp, actions)
        }
        ("AsPermanentEscapes", [Object(object), actions]) => {
            as_event(object, AsEvent::Escapes, actions)
        }
        ("AsPermanentBecomesAttachedToAPermanent", [Object(object), Objects(hosts), actions]) => {
            as_event(
                object,
                AsEvent::BecomesAttachedTo(event_filter(hosts)?),
                actions,
            )
        }
        ("AsGameBegins", body) => before_game(BeforeGameMoment::GameBegins, body),
        ("BeforeDrawingOpeningHand", body) => before_game(BeforeGameMoment::OpeningHand, body),
        ("AsSchemeIsSetInMotion", [_, actions]) => Some(Ability::BeforeGame {
            when: BeforeGameMoment::SetInMotion,
            body: Box::new(actions.clone()),
        }),
        ("ConspiracyDeck", rules) => Some(Ability::DraftRule(rules.to_vec())),
        ("StartingHandSizeIs", [Number(size)]) => Some(Ability::StartingHandSize(size.clone())),
        ("DrawAnAdditionalHandBeforeMulligans", []) => {
            unique(UniqueAbility::AdditionalOpeningHand)
        }
        ("YouAreTheStartingPlayer", []) => unique(UniqueAbility::StartingPlayer),
        ("ItCantBecomeNight", []) => unique(UniqueAbility::NeverNight),
        ("AllDamageIsDealtAsThoughItsSourceHadWither", []) => {
            unique(UniqueAbility::AllDamageWithers)
        }
        ("CantVentureIntoThisDungeonUnlessNamed", []) => {
            unique(UniqueAbility::VentureOnlyByName)
        }
        ("APlayerFacingAVillainousChoiceFacesItAnAdditionalTime", [Players(players)]) => {
            Some(Ability::VillainousChoiceTwice(players.clone()))
        }
        (
            "WhilePlayersAreSearchingTheirLibraryTheyExileEachCardTheyFindAndPlayerMayPlayThoseCardsWhileTheyRemainExiledAndMaySpendManaAsThoughItWereAnyColor",
            [Players(searching), Player(player)],
        ) => Some(Ability::ExilesSearchedCards {
            searching: searching.clone(),
            player: player.clone(),
        }),
        ("SpellActions_Tiered", tiers) => Some(Ability::Tiered {
            modes: tiers
                .iter()
                .map(|tier| match tier {
                    Operand::Term { operands, .. } => match operands.as_slice() {
                        [CostOf(cost), ability] => Some(SpreeMode {
                            cost: cost.clone(),
                            ability: outcome(ability)?,
                        }),
                        _ => None,
                    },
                    _ => None,
                })
                .collect::<Option<_>>()?,
        }),
        ("SpellActions_MadnessX", [CostOf(cost), paid, unpaid]) => Some(Ability::CostBranch {
            keyword: "Madness".into(),
            cost: cost.clone(),
            paid: outcome(paid)?,
            unpaid: outcome(unpaid)?,
        }),
        ("SpellActions_AdditionalCostOptions", options) => Some(Ability::CostOptions {
            keyword: "AdditionalCostOption".into(),
            options: options
                .iter()
                .map(|option| match option {
                    Operand::Term { operands, .. } => match operands.as_slice() {
                        [CostOf(cost), ability] => Some((cost.clone(), *outcome(ability)?)),
                        _ => None,
                    },
                    _ => None,
                })
                .collect::<Option<_>>()?,
        }),
        ("KickerForSpellAbility", [CostOf(cost), ability]) => Some(Ability::KickedHas {
            cost: cost.clone(),
            ability: outcome(ability)?,
        }),
        ("FromTopOfLibrary_Digital", abilities) => {
            Some(Ability::WhileOnTopOfLibrary(abilities_of(abilities)?))
        }
        ("CountsAsACardWithNameForSpellsNamed", [Text(name), Text(spells)]) => {
            Some(Ability::CountsAsNamed {
                name: name.clone(),
                for_spells: spells.clone(),
            })
        }
        ("CraftWithCraftables", [materials, cost]) => Some(Ability::Craft {
            materials: Box::new(materials.clone()),
            count: None,
            cost: Box::new(cost.clone()),
        }),
        (
            "CraftWithANumberOfGroupCraftables",
            [Operand::Comparison(comparison, value), materials, group, cost],
        ) => Some(Ability::Craft {
            materials: Box::new(Operand::List(vec![materials.clone(), group.clone()])),
            count: Some((comparison.clone(), value.clone())),
            cost: Box::new(cost.clone()),
        }),
        (
            "PlayerEffect_PlayerMayPayToIgnoreEffectUntil",
            [Players(players), rules, CostOf(cost), Operand::Duration(until)],
        ) => Some(Ability::PayToIgnore {
            subjects: EffectSubjects::Players(players.clone()),
            rules: Box::new(rules.clone()),
            payer: None,
            cost: cost.clone(),
            until: until.clone(),
        }),
        (
            "PermanentRuleEffect_PlayerMayPayToIgnoreEffectUntil",
            [Object(object), rules, Player(payer), CostOf(cost), Operand::Duration(until)],
        ) => Some(Ability::PayToIgnore {
            subjects: EffectSubjects::Objects(ObjectSet::One(object.clone())),
            rules: Box::new(rules.clone()),
            payer: Some(payer.clone()),
            cost: cost.clone(),
            until: until.clone(),
        }),
        ("IncreaseMorphCosts", [amount]) => Some(keyword_cost_rule(
            CostedKeyword::Morph,
            KeywordCostChange::Increase(Box::new(amount.clone())),
        )),
        ("DecreaseBuybackCosts", [amount]) => Some(keyword_cost_rule(
            CostedKeyword::Buyback,
            KeywordCostChange::Reduce(Box::new(amount.clone())),
        )),
        ("WardAbilitiesOfPermanentsDontTrigger", [Objects(objects)]) => {
            Some(Ability::TriggerRule {
                cause: None,
                abilities: Some(ObjectFilter::All(vec![
                    ObjectFilter::OnStack(StackTrait::Triggered),
                    ObjectFilter::AbilityOf(Box::new(event_filter(objects)?)),
                    ObjectFilter::HasKeywordFamily(KeywordFamily::Ward),
                ])),
                rule: TriggerRule::DoesntTrigger,
            })
        }
        ("TriggerMayOnceEachTurnI", [Operand::Event(event), Operand::Condition(condition), Operand::Program(body)]) => {
            Some(Ability::Triggered {
                event: (**event).clone(),
                intervening_if: Some(condition.clone()),
                limits: TriggerLimits {
                    per_turn: Some(1),
                    total: None,
                },
                announcement: Announcement {
                    targets: vec![],
                    modes: None,
                },
                body: body.clone(),
            })
        }
        _ => None,
    }
}

/// A cost a row names, past the ones `reduce_cost` already reads.
fn reduce_cost_part(bare: &str, operands: &[Operand]) -> Option<CostPart> {
    use Operand::{Filter, List, Number, Object, Objects, Player, Players};
    let bottom = |objects: ObjectSet, choose| CostPart::Move {
        selections: vec![CostObjectSelection {
            objects,
            choose: Some(choose),
        }],
        destination: Destination::OwnersLibraryBottom,
        entry: EntrySpec::default(),
        face_down: false,
    };
    let exactly = |n: &Expr| Count::Exactly(n.clone());
    let one = || Count::Exactly(Expr::Constant(1));
    let graveyard = |players: &PlayerSet| zone_set(EventZone::Graveyard, players.clone(), ObjectFilter::Any);
    let object_cost = |objects: &ObjectSet, action| {
        Some(CostPart::ObjectAction {
            objects: objects.clone(),
            action,
        })
    };
    Some(match (bare, operands) {
        ("PayLifeEqualToItsManaValue", []) => CostPart::LifeEqualToOwnManaValue,
        ("PayManaAnyNumberOfTimes", symbols) => CostPart::ManaAnyNumberOfTimes(mana_terms(symbols)?),
        ("ItsManaCostReducedBy", [reduction]) => CostPart::ManaCostReduced {
            object: None,
            reduction: Box::new(reduction.clone()),
        },
        ("PayManaCostOfPermanentReducedBy", [Object(object), reduction]) => {
            CostPart::ManaCostReduced {
                object: Some(object.clone()),
                reduction: Box::new(reduction.clone()),
            }
        }
        ("PutANumberOfExiledCardsIntoOwnersGraveyard", [Number(times), Objects(cards)]) => {
            CostPart::Repeated {
                part: Box::new(CostPart::ExiledToGraveyard(cards.clone())),
                times: times.clone(),
            }
        }
        ("RevealTheChosenPlayer", []) => CostPart::RevealChoice(ValueKind::Player),
        ("RevealTheSecretlyChosenCreatureType", []) => {
            CostPart::RevealChoice(ValueKind::CreatureType)
        }
        ("PayManaAnyXRestricted", [List(symbols), Operand::Comparison(comparison, value)]) => {
            CostPart::ManaXBounded {
                mana: mana_terms(symbols)?,
                comparison: comparison.clone(),
                value: value.clone(),
            }
        }
        ("UnattachAPermanentFromAPermanent", [Objects(attachments), Objects(hosts)]) => {
            CostPart::UnattachFrom {
                attachments: attachments.clone(),
                hosts: hosts.clone(),
            }
        }
        ("PutAGraveyardCardOnTheBottomOfItsOwnersLibrary", [Filter(cards)]) => bottom(
            zone_set(EventZone::Graveyard, PlayerSet::All, cards.clone()),
            one(),
        ),
        ("PutNumberCardsFromPlayersGraveyardOnBottomOfLibrary", [Number(n), Player(player)]) => {
            bottom(graveyard(&PlayerSet::One(player.clone())), exactly(n))
        }
        ("PutNumberCardsFromASinglePlayersGraveyardOnBottomOfLibrary", [Number(n), Players(players)]) => {
            CostPart::SinglePlayersCards(Box::new(bottom(graveyard(players), exactly(n))))
        }
        ("SacrificeUptoNumberPermanents", [Number(n), Objects(objects)]) => CostPart::Sacrifice {
            objects: objects.clone(),
            choose: Some(Count::UpTo(n.clone())),
        },
        ("PayOneOrMoreEnergy", []) => CostPart::EnergyOneOrMore,
        ("HaveEachPlayerTakeAction", [Players(players), action]) => CostPart::EachPlayerAction {
            players: players.clone(),
            action: Box::new(action.clone()),
        },
        ("PayManaReduceForEach", [List(symbols), reduction, Number(per)]) => {
            CostPart::ManaReducedForEach {
                mana: mana_terms(symbols)?,
                reduction: Box::new(reduction.clone()),
                per: per.clone(),
            }
        }
        ("SuspectAPermanent", [Objects(objects)]) => {
            return object_cost(objects, CostObjectAction::Suspect)
        }
        ("AnteAPermanent", [Objects(objects)]) => {
            return object_cost(objects, CostObjectAction::Ante)
        }
        ("GainControlOfAPermanent", [Objects(objects)]) => {
            return object_cost(objects, CostObjectAction::GainControl)
        }
        ("AttachAPermanentToAPlayer", [Objects(objects), Players(players)]) => {
            return object_cost(objects, CostObjectAction::AttachToPlayer(players.clone()))
        }
        ("PayManaCostOfAPermanent", [Objects(objects)]) => {
            return object_cost(objects, CostObjectAction::PayManaCost)
        }
        ("PayItsSuspendCost", []) => CostPart::Keyword {
            action: CostKeyword::Suspend,
            amount: None,
        },
        ("PayMana_OnlyProducedByTreasure", symbols) => {
            CostPart::ManaFromTreasure(mana_terms(symbols)?)
        }
        ("ShuffleACardFromHandIntoLibrary", []) => CostPart::ShuffleFromHand,
        ("PlayALandFromTopOfLibrary", [Filter(cards)]) => CostPart::PlayFromTop(cards.clone()),
        _ => return None,
    })
}
