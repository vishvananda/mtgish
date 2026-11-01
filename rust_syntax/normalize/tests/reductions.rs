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
