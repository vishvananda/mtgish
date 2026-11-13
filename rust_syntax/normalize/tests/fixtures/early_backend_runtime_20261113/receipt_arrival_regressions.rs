use cantrip_engine::*;
use cantrip_script::{GameScript, run_on};
use std::sync::OnceLock;
#[allow(dead_code, unused_imports)] #[path="fixtures/receipt-arrival/arrival.rs"] mod arrival;
#[allow(dead_code, unused_imports)] #[path="fixtures/receipt-arrival/membership.rs"] mod membership;
fn table() -> &'static CardTable {
    static TABLE: OnceLock<CardTable> = OnceLock::new();
    TABLE.get_or_init(|| CardTable::new(cantrip_cards::all().into_iter().chain([&arrival::CARD, &membership::CARD])))
}
fn play(name: &str, hand: usize) {
    let text = format!("[Bot1 Deck]\nSwamp\nEntomb\n{name}\nSwamp\nSwamp\nSwamp\nSwamp\nGrizzly Bears\nSwamp x12\n[Bot2 Deck]\nIsland x20\n[Actions]\nBot1.Hand.Swamp.Play\nBot1.Battlefield.Swamp.Activate\nBot1.Hand.Entomb.Cast\nBot1.Search(Grizzly Bears)\nBot1.Hand.{name}.Cast\nBot1.Pass\n[Expect]\nBot1.Hand.Grizzly Bears.Colors = Green\nBot1.Hand.Count = {hand}\nExile.Count = 0\n");
    let script=GameScript::parse(&text,name).unwrap();
    if let Err(failure)=run_on(&script,table()) { panic!("{}",failure.report); }
}
#[test] fn past_exile_count_survives_the_later_return_to_hand() { play("Receipt Arrival Test",6); }
#[test] fn still_exiled_does_not_survive_the_later_return_to_hand() { play("Receipt Membership Test",5); }
