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
    let filter = || Operand::Objects(ObjectSet::Battlefield(ObjectFilter::Any));
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
    let creatures = || Operand::Objects(ObjectSet::Battlefield(ObjectFilter::Any));
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
    let creatures = || Operand::Objects(ObjectSet::Battlefield(ObjectFilter::Any));
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
        Operation::ChooseValueWith {
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
