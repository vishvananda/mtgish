#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, corpus, lower, mtg_types as source, validate};
fn module(name: &str) -> String {
    let source: source::OracleCard = include_str!("fixtures/receipt_arrival_counts.ronl")
        .lines()
        .map(|line| ron::from_str(line).unwrap())
        .find(|card| {
            corpus::Identity::of(card)
                .unwrap()
                .names
                .iter()
                .any(|n| n == name)
        })
        .unwrap();
    let card = lower::card(&source).unwrap();
    validate::card(&card).unwrap();
    let module = cantrip::module(&card).unwrap();
    syn::parse_file(&module).unwrap();
    module
}
#[test]
fn past_exile_count_reads_recorded_arrival_zone_after_a_later_move() {
    let module = module("Receipt Arrival Test");
    let count = module.split("let amount_").last().unwrap();
    assert!(count.contains("ZoneKind::Exile"), "{module}");
    assert!(
        !count
            .split("g.draw")
            .next()
            .unwrap()
            .contains("became.is_none"),
        "a past receipt survives a later move: {module}"
    );
}
#[test]
fn explicit_still_exiled_membership_keeps_its_live_identity_guard() {
    let module = module("Receipt Membership Test");
    assert!(
        module.contains("became.is_none()") && module.contains("ZoneKind::Exile"),
        "{module}"
    );
}
