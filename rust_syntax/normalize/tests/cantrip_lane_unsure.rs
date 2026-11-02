#![cfg(feature = "cantrip")]
//! Forms the audit had left unsure that Cantrip turned out to express already, which
//! the generator now writes, on the cards that waited on them. It reads the corpus and
//! the printed words for these cards alone, and has nothing to say without them.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

/// The card, what its module has to say, and whether it is whole.
const CARDS: &[(&str, &str, bool)] = &[
    // A card put into a library arrives on top unless it is shuffled in: Cantrip's
    // move inserts it at the library's first index, the top.
    (
        "Gravebane Zombie",
        "z.to = Zone::library(q.state().obj(z.object).owner); z.shuffle = false;",
        true,
    ),
    (
        "Nephalia Academy",
        "z.to = Zone::library(q.state().obj(z.object).owner); z.shuffle = false;",
        false,
    ),
    // "For each other land ..., add one mana of any type that land produced": one
    // mana at a time, each of the types the tap produced (the card's rulings).
    (
        "Sasaya's Essence",
        "cantrip_mechanics::mana::add_one_mana_of_any_type_that_land_produced(g, item)?; }\n}",
        false,
    ),
    // A player who can't draw can't take an offer to draw (CR 121.3).
    ("Tempt with Bunnies", "!cantrip_api::cant_draw(q, player, 1)", true),
    // The defending players of a combat: the active player's opponents (CR 506.2).
    (
        "Stand or Fall",
        "q.state().opponents(q.state().turn.active).any(|o| o == player)",
        false,
    ),
];

#[test]
fn unsure_forms_cantrip_expresses() {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let (corpus, oracle) = (data.join("mtgish.lines.ron"), data.join("oracle.json"));
    if !corpus.exists() || !oracle.exists() {
        return;
    }
    let mut names: Vec<String> = CARDS.iter().map(|(name, ..)| name.to_string()).collect();
    names.dedup();
    let modules: BTreeMap<String, String> = cantrip::modules_printed(&corpus, &oracle, &names)
        .unwrap()
        .into_iter()
        .collect();
    for (name, says, whole) in CARDS {
        let module = modules
            .get(*name)
            .unwrap_or_else(|| panic!("no module for {name}"));
        assert!(module.contains(says), "{name} should have {says}:\n{module}");
        if *whole {
            assert!(!module.contains("planned"), "{name} should be whole:\n{module}");
        }
    }
}
