//! Rows that decode to a typed form leave no primitive behind.
use mtgish_ast::{ir::*, semantics};

fn filter(name: &str, operands: Vec<Operand>) -> ObjectFilter {
    semantics::reduce_filter(name.to_string(), operands)
}

#[test]
fn object_predicates_are_typed() {
    let reduced =
        |name: &str, operands| !matches!(filter(name, operands), ObjectFilter::Primitive { .. });
    for name in [
        "Permanents::IsARingBearer",
        "Permanents::ProwlCostWasPaid",
        "Spells::AdditionalCostWasPaid",
        "Cards::WasDrawnThisTurn",
        "Permanents::SharesANameOriginallyPrintedInHomelands",
    ] {
        assert!(reduced(name, vec![]), "{name}");
    }
    let player = || Operand::Player(PlayerRef::AbilityController);
    assert!(reduced(
        "Permanents::AttackedDuringLastPlayersTurn",
        vec![player()]
    ));
    assert!(reduced("Cards::WasExiledByPlayer", vec![player()]));
    assert!(reduced(
        "Permanents::SharesANameWithCardInPlayersGraveyard",
        vec![Operand::Filter(ObjectFilter::Any), player()]
    ));
    assert!(reduced(
        "Cards::TheCardsExiledThisWay",
        vec![Operand::Result(ResultId(1))]
    ));
}

#[test]
fn an_unknown_row_stays_primitive() {
    assert!(matches!(
        filter("Permanents::NotARealForm", vec![]),
        ObjectFilter::Primitive { .. }
    ));
}

#[test]
fn numbers_and_conditions_are_typed() {
    let player = || Operand::Player(PlayerRef::AbilityController);
    let filter = || Operand::Objects(mtgish_ast::battlefield_query!(ObjectFilter::Any));
    for (name, operands) in [
        ("GameNumber::PlayersChosenNumber", vec![player()]),
        (
            "GameNumber::MaxPermanentsControlledByAPlayer",
            vec![filter(), Operand::Players(PlayerSet::All)],
        ),
        ("GameNumber::NumColorsManaSpentToCastSelf", vec![]),
    ] {
        assert!(
            !matches!(
                semantics::reduce_number(name.into(), operands),
                Expr::Primitive { .. }
            ),
            "{name}"
        );
    }
    for (name, operands) in [
        ("Condition::IsBeforeCombat", vec![]),
        ("Players::HasMaxSpeed", vec![player()]),
        ("Players::OwnsAPermanent", vec![player(), filter()]),
    ] {
        assert!(
            !matches!(
                semantics::reduce_condition(name.into(), operands),
                Condition::Primitive { .. }
            ),
            "{name}"
        );
    }
    assert!(semantics::reduce_player("PlayerToTheLeftOf", &[player()]).is_some());
}

#[test]
fn pairs_of_mana_count_whole_pairs() {
    let blue = || Operand::Vocabulary {
        family: "ManaProduceSymbol".into(),
        form: "ManaProduceU".into(),
    };
    let pairs = semantics::reduce_number(
        "GameNumber::WhenAPermanentEntersTheBattlefield_AmountOfManaOfTypeSpentToCast".into(),
        vec![blue(), blue()],
    );
    assert!(matches!(pairs, Expr::Divide { .. }));
}

#[test]
fn rules_are_typed() {
    let player = || Operand::Player(PlayerRef::AbilityController);
    let creatures = || Operand::Objects(mtgish_ast::battlefield_query!(ObjectFilter::Any));
    for (name, operands) in [
        ("PlayerEffect::ChoosesWhichCreaturesAttack", vec![]),
        ("PlayerEffect::UnspentManaBecomesColorless", vec![]),
        (
            "PlayerEffect::MustAttackWithEachCreature",
            vec![creatures()],
        ),
        (
            "PlayerEffect::MayAttackOnlyPlayerOrPlaneswalkersControlledBy",
            vec![player()],
        ),
    ] {
        assert!(
            !matches!(
                semantics::reduce_player_rule(name.into(), operands),
                PlayerRule::Primitive { .. }
            ),
            "{name}"
        );
    }
    for (name, operands) in [
        ("PermanentRule::CantBeEquipped", vec![]),
        ("PermanentRule::CantBeTurnedFaceUp", vec![]),
        (
            "PermanentRule::AttackingCausesOthersToAttack",
            vec![creatures()],
        ),
    ] {
        assert!(
            !matches!(
                semantics::reduce_rule(name.into(), operands),
                RuleChange::Primitive { .. }
            ),
            "{name}"
        );
    }
    for name in [
        "Rule::DamageCantBePrevented",
        "Rule::YouAreTheStartingPlayer",
    ] {
        assert!(
            !matches!(
                semantics::reduce_ability(name.into(), vec![]),
                Ability::Primitive { .. }
            ),
            "{name}"
        );
    }
}

#[test]
fn x_keywords_keep_their_mana() {
    let symbol = |form: &str| Operand::Term {
        family: "ManaSymbolX".into(),
        form: form.into(),
        operands: vec![],
    };
    let Ability::Mechanic { mechanic, cost, .. } = semantics::reduce_ability(
        "Rule::MiracleX".into(),
        vec![symbol("ManaCostX"), symbol("ManaCostR")],
    ) else {
        panic!("miracle stays a primitive");
    };
    assert!(matches!(mechanic, Mechanic::Miracle));
    let cost = cost.expect("a cost");
    assert!(matches!(
        cost.parts.as_slice(),
        [CostPart::Mana(symbols)]
            if matches!(symbols.as_slice(), [ManaSymbol::X, ManaSymbol::Color(ManaColor::Red)])
    ));
}

#[test]
fn an_unknown_rule_stays_with_its_row() {
    assert!(matches!(
        semantics::reduce_player_rule("PlayerEffect::NoSuchRule".into(), vec![]),
        PlayerRule::Primitive { .. }
    ));
    assert!(matches!(
        semantics::reduce_cost("Cost::NoSuchCost".into(), vec![]),
        CostPart::Primitive { .. }
    ));
}

#[test]
fn actions_read_as_zone_changes_and_plays() {
    let you = || PlayerSet::One(PlayerRef::AbilityController);
    let player = || Operand::Player(PlayerRef::AbilityController);
    let cards = || Operand::Filter(ObjectFilter::Any);
    let reduced = |name: &str, operands| semantics::reduce_action(you(), name.into(), operands);
    match reduced(
        "Action::ReturnACardFromGraveyardToHandAtRandom",
        vec![cards()],
    ) {
        Operation::ChangeZone {
            random: true,
            destination: Destination::OwnersHand,
            count: Some(Count::Exactly(_)),
            ..
        } => {}
        other => panic!("{other:?}"),
    }
    match reduced(
        "Action::CastASpellFromPlayersGraveyardWithoutPayingIntoExile",
        vec![cards(), player()],
    ) {
        Operation::PlayWith { terms, .. } => {
            assert!(terms.into_exile && matches!(terms.cost, PlayCost::Free));
        }
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        reduced(
            "Action::CreateFutureSpellEffect",
            vec![
                Operand::Term {
                    family: "FutureSpell".into(),
                    form: "TheNextSpellPlayerCastsThisTurn".into(),
                    operands: vec![cards(), player()],
                },
                Operand::List(vec![]),
            ],
        ),
        Operation::EffectOn {
            subject: EffectSubject::NextSpell {
                this_turn: true,
                ..
            },
            until: None,
            ..
        }
    ));
    assert!(matches!(
        reduced("Action::NoSuchAction", vec![]),
        Operation::Primitive { .. }
    ));
}

#[test]
fn set_and_stack_actions_are_typed() {
    let you = || PlayerSet::One(PlayerRef::AbilityController);
    let creatures = || Operand::Objects(mtgish_ast::battlefield_query!(ObjectFilter::Any));
    let reduced = |name: &str, operands| semantics::reduce_action(you(), name.into(), operands);
    assert!(matches!(
        reduced("Action::DestroyAPermanentAtRandom", vec![creatures()]),
        Operation::Destroy {
            objects: ObjectSet::Random { .. },
            regeneration: true,
        }
    ));
    assert!(matches!(
        reduced("Action::PhaseOutAnyNumberOfPermanents", vec![creatures()]),
        Operation::KeywordEach {
            action: KeywordAction::PhaseOut,
            objects: ObjectSet::Chosen { .. },
            ..
        }
    ));
    match reduced(
        "Action::SimultaneouslyTapEachPermanentAndUntapEachPermanent",
        vec![creatures(), creatures()],
    ) {
        Operation::Together(steps) => assert_eq!(steps.len(), 2),
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        reduced(
            "Action::SecretlyChooseANumberBetween",
            vec![Operand::Integer(1), Operand::Integer(3)]
        ),
        Operation::ChooseValue {
            kind: ValueKind::Number,
            terms: ChoiceTerms { secret: true, .. },
            ..
        }
    ));
    assert!(matches!(
        reduced("Action::ExchangeHandAndGraveyard", vec![]),
        Operation::Exchange {
            kind: ExchangeKind::Zones(EventZone::Hand, EventZone::Graveyard),
            ..
        }
    ));
    assert!(matches!(
        reduced("Action::RollTwoD6AndChooseAnOrder", vec![]),
        Operation::Keyword {
            action: KeywordAction::RollTwoInOrder(6),
            ..
        }
    ));
}

#[test]
fn numbers_and_conditions_read_what_happened() {
    assert!(matches!(
        semantics::reduce_number("GameNumber::PowerOfTheRevealedCard".into(), vec![]),
        Expr::Earlier {
            what: EarlierObjects::Revealed,
            measure: ObjectMeasure::Of(Property::Power),
            ..
        }
    ));
    assert!(matches!(
        semantics::reduce_number("GameNumber::TheTotalOfTheDiceResults".into(), vec![]),
        Expr::Dice(DiceMeasure::Total)
    ));
    assert!(matches!(
        semantics::reduce_condition("Condition::FreerunningCostWasPaid".into(), vec![]),
        Condition::Outcome(PriorOutcome::FreerunningPaid)
    ));
    assert!(matches!(
        semantics::reduce_event(
            "Trigger::WhenACardIsPutIntoExile".into(),
            vec![Operand::Filter(ObjectFilter::Any)],
        ),
        EventSpec::ZoneChange {
            from: None,
            to: EventZone::Exile,
            ..
        }
    ));
    assert!(matches!(
        semantics::reduce_event(
            "Trigger::WhenASpellOrAbilityCausesItsControllerToShuffleTheirLibrary".into(),
            vec![Operand::Term {
                family: "SpellsAndAbilities".into(),
                form: "AnySpellOrAbility".into(),
                operands: vec![],
            }],
        ),
        EventSpec::Caused { .. }
    ));
}

#[test]
fn references_name_what_they_read() {
    assert!(matches!(
        semantics::reduce_player_ref("Player::WinningBidder".into(), vec![]),
        PlayerRef::Prior(PriorPlayer::WinningBidder)
    ));
    assert!(matches!(
        semantics::reduce_object("CardInExile::TheLastExiledCard".into(), vec![]),
        ObjectRef::Earlier {
            what: EarlierObjects::LastExiled,
            by: None
        }
    ));
}

#[test]
fn continuous_effects_say_what_they_change() {
    assert!(matches!(
        semantics::reduce_change("SpellEffect::HasAllCreatureTypes".into(), vec![]),
        Change::CreatureTypes {
            types: CreatureTypeSet::All,
            ..
        }
    ));
    assert!(matches!(
        semantics::reduce_change(
            "LayerEffect::AddLandTypeVariable".into(),
            vec![Operand::Term {
                family: "LandTypeVariable".into(),
                form: "EachBasicLandType".into(),
                operands: vec![],
            }],
        ),
        Change::LandTypes {
            scope: LandTypeScope::Basic,
            mode: ChangeMode::Add,
        }
    ));
    assert!(matches!(
        semantics::reduce_duration("Expiration::UntilEndOfTheNextTurn", &[]),
        Some(Duration::ThroughNextTurn)
    ));
    assert!(matches!(
        semantics::reduce_mana_production("ManaProduce::AnyTwoDifferentManaColors", &[]),
        Some(ManaProduction::DifferentColors(2))
    ));
    assert!(matches!(
        semantics::reduce_mana_use("ManaUseModifier::CanOnlySpendToCastForetoldSpells", &[]),
        Some(ManaUse::CastSpell(ObjectFilter::State(
            ObjectState::Foretold
        )))
    ));
}

#[test]
fn filters_read_what_they_ask_about() {
    assert!(matches!(
        semantics::reduce_filter(
            "Permanents::IsPlaneswalkerTypeVariable".into(),
            vec![Operand::Vocabulary {
                family: "PlaneswalkerTypeVariable".into(),
                form: "TheChosenPlaneswalkerType".into(),
            }],
        ),
        ObjectFilter::Chosen(ValueKind::PlaneswalkerType)
    ));
    assert!(matches!(
        semantics::reduce_filter(
            "Permanents::IsTheSecondChosenPermanentFilter".into(),
            vec![],
        ),
        ObjectFilter::NthChosen(1)
    ));
    assert!(matches!(
        semantics::reduce_filter(
            "Spells::PlayerRevealedACardAsCast".into(),
            vec![
                Operand::Player(PlayerRef::AbilityController),
                Operand::Filter(ObjectFilter::Any),
            ],
        ),
        ObjectFilter::Condition(_)
    ));
    assert!(matches!(
        semantics::reduce_player_ref("Player::Trigger_ThatPlayer".into(), vec![]),
        PlayerRef::Prior(PriorPlayer::That)
    ));
}

#[test]
fn counters_and_would_events_say_what_happens() {
    let plus = Operand::Vocabulary {
        family: "CounterType".into(),
        form: "ChargeCounter".into(),
    };
    assert!(matches!(
        semantics::reduce_action(
            PlayerSet::All,
            "PutCountersAction::DistributeNumberCountersOfTypeAmongPermanents".into(),
            vec![
                Operand::Number(Expr::Constant(3)),
                plus,
                Operand::Objects(mtgish_ast::battlefield_query!(ObjectFilter::Any)),
            ],
        ),
        Operation::DistributeCounters {
            any_number: false,
            ..
        }
    ));
    assert!(matches!(
        semantics::reduce_replacement(
            "ReplacableEventWouldEnter::APermanentWouldEnterTheBattlefieldAndWasntCast".into(),
            vec![Operand::Objects(mtgish_ast::battlefield_query!(ObjectFilter::Any))],
        ),
        ReplacementEvent::Enter { .. }
    ));
    assert!(matches!(
        semantics::reduce_replacement(
            "FutureReplacableEventWouldDraw::NextTimePlayerWouldDrawACardThisTurn".into(),
            vec![Operand::Player(PlayerRef::AbilityController)],
        ),
        ReplacementEvent::Player {
            event: ReplacedPlayerEvent::Draw,
            ..
        }
    ));
}

#[test]
fn actions_keep_what_they_do() {
    assert!(matches!(
        semantics::reduce_action(
            PlayerSet::All,
            "Action::ConjureCardOrCardIntoHand".into(),
            vec![Operand::Text("A".into()), Operand::Text("B".into())],
        ),
        Operation::Conjure {
            card: ConjuredCard::OneOf(_),
            ..
        }
    ));
    assert!(matches!(
        semantics::reduce_action(
            PlayerSet::All,
            "Action::SeparatePermanentsIntoNumberPiles".into(),
            vec![
                Operand::Objects(mtgish_ast::battlefield_query!(ObjectFilter::Any)),
                Operand::Number(Expr::Constant(3)),
            ],
        ),
        Operation::Piles { count: Some(_), .. }
    ));
    assert!(matches!(
        semantics::reduce_action(
            PlayerSet::All,
            "Action::NoteNumber".into(),
            vec![Operand::Number(Expr::Constant(3))],
        ),
        Operation::Note(NoteKind::Number(_))
    ));
    assert!(matches!(
        semantics::reduce_action(
            PlayerSet::All,
            "Action::StartBiddingWarAmongPlayersAtNumber".into(),
            vec![
                Operand::Players(PlayerSet::All),
                Operand::Number(Expr::Constant(1)),
            ],
        ),
        Operation::Bid { start: Some(_), .. }
    ));
}

#[test]
fn triggers_watch_by_shape() {
    let creatures = || Operand::Objects(mtgish_ast::battlefield_query!(ObjectFilter::Any));
    assert!(matches!(
        semantics::reduce_event(
            "Trigger::WhenANumberOfCreaturesAttacksAPlayerAndArentBlocked".into(),
            vec![
                Operand::Comparison(Comparison::GreaterOrEqual, Expr::Constant(1)),
                creatures(),
                Operand::Players(PlayerSet::All),
            ],
        ),
        EventSpec::AttackUnblocked { count: Some(_), .. }
    ));
    assert!(matches!(
        semantics::reduce_event(
            "Trigger::WhenAPlayerCastsASpellThatTargetsAnyNumberOfPermanents".into(),
            vec![
                Operand::Players(PlayerSet::All),
                Operand::Filter(ObjectFilter::Any),
                creatures(),
            ],
        ),
        EventSpec::SpellTargeting {
            targets: Some(_),
            ..
        }
    ));
    assert!(matches!(
        semantics::reduce_event(
            "Trigger::WhenAPermanentHasPower".into(),
            vec![
                creatures(),
                Operand::Comparison(Comparison::GreaterOrEqual, Expr::Constant(7)),
            ],
        ),
        EventSpec::ObjectBecomes(_)
    ));
    assert!(matches!(
        semantics::reduce_event(
            "Trigger::WhenAPermanentEnteringTheBattlefieldCausesAnAbilityToTrigger".into(),
            vec![creatures()],
        ),
        EventSpec::CausesTrigger {
            own_ability: false,
            ..
        }
    ));
}

#[test]
fn numbers_and_conditions_read_what_the_game_recorded() {
    assert!(matches!(
        semantics::reduce_condition(
            "Condition::NoPermanentsLeftTheBattlefieldThisTurn".into(),
            vec![Operand::Objects(mtgish_ast::battlefield_query!(ObjectFilter::Any))],
        ),
        Condition::Compare { .. }
    ));
    assert!(matches!(
        semantics::reduce_number(
            "GameNumber::TheNumberOfTimesModeChosenForSpell".into(),
            vec![Operand::Object(ObjectRef::EventSubject)],
        ),
        Expr::ModesChosen(_)
    ));
    assert!(matches!(
        semantics::reduce_condition(
            "Players::CouldMulligan".into(),
            vec![Operand::Player(PlayerRef::AbilityController)],
        ),
        Condition::PlayerState {
            state: PlayerState::CouldMulligan,
            ..
        }
    ));
}

#[test]
fn abilities_change_their_own_activation() {
    assert!(matches!(
        semantics::reduce_activation(
            "ActivateModifier::ReduceManaCostForEachAlternateCost".into(),
            vec![Operand::Cost(Cost { parts: vec![] })],
        ),
        Change::ActivationRule(ActivationRule::ManaInstead(_))
    ));
}

#[test]
fn reflexive_triggers_and_perpetual_terms_keep_their_shape() {
    let you = || PlayerSet::One(PlayerRef::AbilityController);
    let body = || {
        Operand::Program(Program {
            instructions: vec![],
        })
    };
    assert!(matches!(
        semantics::reduce_action(
            you(),
            "Action::Reflexive_CollectEvidenceAnyX_WhenYouDo".into(),
            vec![body()],
        ),
        Operation::WithReflexive {
            when: ReflexiveWhen::YouDo,
            ..
        }
    ));
    let adjust = Operand::Term {
        family: "PerpetualEffect".into(),
        form: "AdjustPT".into(),
        operands: vec![Operand::Integer(1), Operand::Integer(1)],
    };
    let top_creature = Operand::Term {
        family: "CardInLibrary".into(),
        form: "TheTopCardOfTypeInPlayersLibrary".into(),
        operands: vec![
            Operand::Filter(ObjectFilter::Any),
            Operand::Player(PlayerRef::AbilityController),
        ],
    };
    assert!(matches!(
        semantics::reduce_action(
            you(),
            "Action::CreatePerpetualCardInLibraryEffect".into(),
            vec![top_creature, Operand::List(vec![adjust])],
        ),
        Operation::CreatePerpetualEffect {
            objects: ObjectSet::Take { .. },
            ..
        }
    ));
    assert!(matches!(
        semantics::reduce_action(you(), "Action::SetSchemeInMotion".into(), vec![]),
        Operation::Keyword {
            action: KeywordAction::SetSchemeInMotion,
            ..
        }
    ));
}

#[test]
fn single_cards_with_a_general_shape_reduce() {
    let you = || PlayerSet::One(PlayerRef::AbilityController);
    let reduced = |name: &str, operands: Vec<Operand>| {
        !matches!(
            semantics::reduce_action(you(), name.into(), operands),
            Operation::Primitive { .. }
        )
    };
    assert!(reduced(
        "Action::MillNumberCardsForEach",
        vec![
            Operand::Number(Expr::Constant(3)),
            Operand::Number(Expr::Constant(2))
        ]
    ));
    assert!(reduced("Action::RollTwoD6", vec![]));
    assert!(matches!(
        semantics::reduce_action(
            you(),
            "Action::EachPlayerRevealsCardsFromTheTopOfLibraryUntilANumberOfCardsOfTypeAreRevealed"
                .into(),
            vec![
                Operand::Players(PlayerSet::All),
                Operand::Number(Expr::AnnouncedX),
                Operand::Filter(ObjectFilter::Any),
            ],
        ),
        Operation::EachPlayer { .. }
    ));
    assert!(matches!(
        semantics::reduce_replacement(
            "ReplacableEventWouldFlipACoin::PlayerWouldFlipACoin".into(),
            vec![Operand::Player(PlayerRef::AbilityController)],
        ),
        ReplacementEvent::Player {
            event: ReplacedPlayerEvent::FlipCoin,
            ..
        }
    ));
    assert!(matches!(
        semantics::reduce_filter("CardsInExile::WasExiledThisWay".into(), vec![]),
        ObjectFilter::InSet(_)
    ));
}

#[test]
fn entry_flags_and_routed_fallbacks_reduce() {
    let spawn = semantics::reduce_replacement(
        "ReplacableEventWouldEnter::PermanentWouldEnterTheBattlefieldAndWasntCastOrNoManaWasSpentToCast"
            .into(),
        vec![Operand::Object(ObjectRef::Source)],
    );
    assert!(matches!(spawn, ReplacementEvent::Enter { .. }));
    let shares = semantics::reduce_filter(
        "DamageSources::SharesAColorWithExiledCard".into(),
        vec![Operand::Object(ObjectRef::Source)],
    );
    assert!(matches!(
        shares,
        ObjectFilter::SharesColorWith(ObjectRef::Source)
    ));
}

#[test]
fn audited_forms_keep_every_clause() {
    let you = || PlayerSet::One(PlayerRef::AbilityController);
    let copy = semantics::reduce_action(
        you(),
        "Action::CopyEachSpellAndMayChooseNewTargets".into(),
        vec![Operand::Filter(ObjectFilter::Any)],
    );
    assert!(format!("{copy:?}").contains("OnStack(Spell)"), "{copy:?}");
    let drain = semantics::reduce_action(
        you(),
        "Action::ActivateAManaAbilityOfEachPermanentAndLoseUnspentMana".into(),
        vec![Operand::Objects(mtgish_ast::battlefield_query!(ObjectFilter::Any))],
    );
    assert!(
        format!("{drain:?}").contains("LoseUnspentMana"),
        "{drain:?}"
    );
    assert!(matches!(
        semantics::reduce_number(
            "GameNumber::TheClampedAmountOfDamageDealtThisWay".into(),
            vec![Operand::Result(ResultId(0))],
        ),
        Expr::ClampedDamage(_)
    ));
}
