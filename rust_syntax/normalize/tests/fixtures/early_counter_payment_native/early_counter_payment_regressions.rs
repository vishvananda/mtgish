use cantrip_engine::*;
use cantrip_script::{GameScript, run_on};
use std::sync::OnceLock;
#[allow(dead_code,unused_imports)] #[path="fixtures/early-counter-payment/tetravus.rs"] mod tetravus;
fn table() -> &'static CardTable {
    static TABLE: OnceLock<CardTable> = OnceLock::new();
    TABLE.get_or_init(|| CardTable::new(cantrip_cards::all().into_iter()
        .filter(|c| c.name!="Tetravus" && !tetravus::EXTRA.iter().any(|t|t.name==c.name))
        .chain([&tetravus::CARD]).chain(tetravus::EXTRA.iter().copied())))
}
#[test]
fn tetravus_rejects_excess_counters_then_creates_and_reclaims_the_paid_number() {
    let script=GameScript::parse(include_str!("fixtures/early-counter-payment/tetravus-makes-and-reclaims-tetravites.cantrip"),"tetravus-payment").unwrap();
    if let Err(e)=run_on(&script,table()) { panic!("{}",e.report); }
}
