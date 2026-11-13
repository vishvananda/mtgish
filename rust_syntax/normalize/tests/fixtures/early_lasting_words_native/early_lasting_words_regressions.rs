use cantrip_engine::*;
use cantrip_script::{GameScript, run_on};
use std::sync::OnceLock;
#[allow(dead_code,unused_imports)] #[path="fixtures/early-lasting-words/island_sanctuary.rs"] mod island_sanctuary;
#[allow(dead_code,unused_imports)] #[path="fixtures/early-lasting-words/tower_of_coireall.rs"] mod tower_of_coireall;
fn table() -> &'static CardTable {
    static TABLE: OnceLock<CardTable> = OnceLock::new();
    TABLE.get_or_init(|| CardTable::new(cantrip_cards::all().into_iter()
        .filter(|c| !["Island Sanctuary", "Tower of Coireall"].contains(&c.name))
        .chain([&island_sanctuary::CARD, &tower_of_coireall::CARD])))
}
fn check(script: &str, name: &str) {
    let script=GameScript::parse(script,name).unwrap();
    if let Err(e)=run_on(&script,table()) { panic!("{}",e.report); }
}

#[test]
fn lasting_words_0() { check(include_str!("fixtures/early-lasting-words/island-sanctuary-restriction-keeps-its-original-text.cantrip"),"island-sanctuary-restriction-keeps-its-original-text.cantrip"); }

#[test]
fn lasting_words_1() { check(include_str!("fixtures/early-lasting-words/tower-of-coireall-can-target-opponents-creature.cantrip"),"tower-of-coireall-can-target-opponents-creature.cantrip"); }

#[test]
fn lasting_words_2() { check(include_str!("fixtures/early-lasting-words/tower-of-coireall-text-change-to-elf.cantrip"),"tower-of-coireall-text-change-to-elf.cantrip"); }

#[test]
fn lasting_words_3() { check(include_str!("fixtures/early-lasting-words/tower-of-coireall-wall-restriction-expires.cantrip"),"tower-of-coireall-wall-restriction-expires.cantrip"); }
