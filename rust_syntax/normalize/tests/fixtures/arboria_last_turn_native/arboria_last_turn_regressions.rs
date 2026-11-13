use cantrip_engine::*;
use cantrip_script::{GameScript, run_on};
use std::sync::OnceLock;
#[allow(dead_code,unused_imports)] #[path="fixtures/arboria-last-turn/arboria.rs"] mod arboria;
fn table() -> &'static CardTable {
    static TABLE: OnceLock<CardTable> = OnceLock::new();
    TABLE.get_or_init(|| CardTable::new(cantrip_cards::all().into_iter()
        .filter(|c| c.name != "Arboria").chain([&arboria::CARD])))
}
#[test]
fn arboria_credits_the_putter_in_the_players_last_completed_turn() {
    let script=GameScript::parse(include_str!("fixtures/arboria-last-turn/arboria-credits-the-putter-instead-of-the-entering-controller.cantrip"),"arboria-putter").unwrap();
    if let Err(e)=run_on(&script,table()) { panic!("{}",e.report); }
}
