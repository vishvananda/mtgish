use cantrip_engine::*;
use cantrip_script::{GameScript, run_on};
use std::sync::OnceLock;
#[allow(dead_code, unused_imports)]
#[path = "fixtures/defender-history/floral_spuzzem.rs"] mod floral;
#[allow(dead_code, unused_imports)]
#[path = "fixtures/defender-history/necrite.rs"] mod necrite;
fn table() -> &'static CardTable {
    static TABLE: OnceLock<CardTable> = OnceLock::new();
    TABLE.get_or_init(|| CardTable::new(cantrip_cards::all().into_iter()
        .filter(|c| !["Floral Spuzzem", "Necrite"].contains(&c.name))
        .chain([&floral::CARD, &necrite::CARD])))
}
fn play(name: &str, text: &str) {
    let script = GameScript::parse(text, name).unwrap();
    if let Err(failure) = run_on(&script, table()) { panic!("{}", failure.report); }
}
#[test] fn entered_flower_keeps_its_defender_after_dying_in_response() {
    play("floral-defender", include_str!("fixtures/defender-history/floral.cantrip"));
}
#[test] fn entered_necrite_keeps_its_defender_after_regeneration() {
    play("necrite-defender", include_str!("fixtures/defender-history/necrite.cantrip"));
}
