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
