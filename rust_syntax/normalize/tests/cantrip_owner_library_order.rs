#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, lower, mtg_types::OracleCard};

#[test]
fn target_players_artifacts_are_ordered_by_their_owner_before_one_batch_move() {
    let source: OracleCard = ron::from_str(
        include_str!("fixtures/owner_library_order.ronl")
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    let card = lower::card(&source).unwrap();
    let generated = cantrip::module(&card).unwrap();
    syn::parse_file(&generated).unwrap();
    if let Ok(path) = std::env::var("OWNER_LIBRARY_PRINTED") {
        let printed = cantrip::pr_form::render(&generated, "Drafna's Restoration", "Put any number of target artifact cards from target player's graveyard on top of their library in any order.", cantrip::pr_form::Characteristics::Inline).unwrap();
        std::fs::write(path, printed).unwrap();
    }
    let compact: String = generated.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(compact.contains("g.state().apnap()"), "{generated}");
    assert!(
        compact.contains("ask_library_order(g,owner_"),
        "{generated}"
    );
    assert!(!compact.contains("owner==asker.controller"), "{generated}");
    assert_eq!(
        compact.matches("g.put_into_library(").count(),
        1,
        "{generated}"
    );
}

#[test]
fn opponent_graveyard_cards_also_use_their_owners_order() {
    let source: OracleCard = ron::from_str(
        include_str!("fixtures/owner_library_order.ronl")
            .lines()
            .nth(1)
            .unwrap(),
    )
    .unwrap();
    let card = lower::card(&source).unwrap();
    let generated = cantrip::module(&card).unwrap();
    let compact: String = generated.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(
        compact.contains("ask_library_order(g,owner_"),
        "{generated}"
    );
    assert_eq!(
        compact.matches("g.put_into_library(").count(),
        1,
        "{generated}"
    );
}

#[test]
fn different_owners_are_grouped_before_the_single_simultaneous_move() {
    let source: OracleCard = ron::from_str(
        include_str!("fixtures/owner_library_order.ronl")
            .lines()
            .nth(2)
            .unwrap(),
    )
    .unwrap();
    let card = lower::card(&source).unwrap();
    let generated = cantrip::module(&card).unwrap();
    let compact: String = generated.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(compact.contains("g.state().apnap()"), "{generated}");
    assert!(compact.contains(".owner==owner_"), "{generated}");
    assert_eq!(
        compact.matches("g.put_into_library(").count(),
        1,
        "{generated}"
    );
}
