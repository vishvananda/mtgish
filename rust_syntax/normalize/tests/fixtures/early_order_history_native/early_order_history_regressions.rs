use cantrip_engine::*;
use cantrip_script::{GameScript, run_on};
use std::sync::OnceLock;
#[allow(dead_code,unused_imports)] #[path="fixtures/early-order-history/eureka.rs"] mod eureka;
#[allow(dead_code,unused_imports)] #[path="fixtures/early-order-history/rapid_fire.rs"] mod rapid_fire;
#[allow(dead_code,unused_imports)] #[path="fixtures/early-order-history/seraph.rs"] mod seraph;
#[allow(dead_code,unused_imports)] #[path="fixtures/early-order-history/whirling_dervish.rs"] mod whirling_dervish;
#[allow(dead_code,unused_imports)] #[path="fixtures/early-order-history/total_war.rs"] mod total_war;
#[allow(dead_code,unused_imports)] #[path="fixtures/early-order-history/shapeshifter.rs"] mod shapeshifter;
// Match the native Oracle build of printed */7-*: neither is a literal i32.
static PRINTED_SHAPESHIFTER: CardDef = CardDef { power: Some(0), toughness: Some(0), ..shapeshifter::CARD };
#[allow(dead_code,unused_imports)] #[path="fixtures/early-order-history/thieves_auction.rs"] mod thieves_auction;
fn table() -> &'static CardTable {
static TABLE: OnceLock<CardTable> = OnceLock::new();
TABLE.get_or_init(|| CardTable::new(cantrip_cards::all().into_iter().filter(|c| !["Eureka", "Rapid Fire", "Seraph", "Whirling Dervish", "Total War", "Shapeshifter", "Thieves' Auction"].contains(&c.name)).chain([&eureka::CARD, &rapid_fire::CARD, &seraph::CARD, &whirling_dervish::CARD, &total_war::CARD, &PRINTED_SHAPESHIFTER, &thieves_auction::CARD])))
}
fn play(name: &str, text: &str) { let script=GameScript::parse(text,name).unwrap(); if let Err(failure)=run_on(&script,table()) { panic!("{}",failure.report); } }
#[test] fn rapid_fire_cannot_be_cast_in_an_additional_combat() { play("rapid-fire-cannot-be-cast-in-an-additional-combat.cantrip", include_str!("fixtures/early-order-history/rapid-fire-cannot-be-cast-in-an-additional-combat.cantrip")); }
#[test] fn eureka_players_put_in_turn() { play("eureka-players-put-in-turn.cantrip", include_str!("fixtures/early-order-history/eureka-players-put-in-turn.cantrip")); }
#[test] fn seraph_leaves_after_return() { play("seraph-leaves-after-return.cantrip", include_str!("fixtures/early-order-history/seraph-leaves-after-return.cantrip")); }
#[test] fn seraph_simultaneous_deaths() { play("seraph-simultaneous-deaths.cantrip", include_str!("fixtures/early-order-history/seraph-simultaneous-deaths.cantrip")); }
#[test] fn total_war_additional_combat() { play("total-war-additional-combat.cantrip", include_str!("fixtures/early-order-history/total-war-additional-combat.cantrip")); }
#[test] fn total_war_destroys_nonattackers() { play("total-war-destroys-nonattackers.cantrip", include_str!("fixtures/early-order-history/total-war-destroys-nonattackers.cantrip")); }
#[test] fn total_war_multiple_attackers_and_tapped() { play("total-war-multiple-attackers-and-tapped.cantrip", include_str!("fixtures/early-order-history/total-war-multiple-attackers-and-tapped.cantrip")); }
#[test] fn shapeshifter_characteristic_defining_in_hand() { play("shapeshifter-characteristic-defining-in-hand.cantrip", include_str!("fixtures/early-order-history/shapeshifter-characteristic-defining-in-hand.cantrip")); }
#[test] fn whirling_dervish_damage_to_opponent_who_leaves() { play("whirling-dervish-damage-to-opponent-who-leaves.cantrip", include_str!("fixtures/early-order-history/whirling-dervish-damage-to-opponent-who-leaves.cantrip")); }

#[test] fn eureka_all_decline_ends_the_process() { play("eureka-all-decline-ends-the-process.cantrip",include_str!("fixtures/early-order-history/eureka-all-decline-ends-the-process.cantrip")); }

#[test] fn thieves_auction_ordered_entries_and_completion() { play("thieves-auction-ordered-entries-and-completion.cantrip",include_str!("fixtures/early-order-history/thieves-auction-ordered-entries-and-completion.cantrip")); }
