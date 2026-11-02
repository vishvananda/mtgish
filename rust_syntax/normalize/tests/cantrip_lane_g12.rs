#![cfg(feature = "cantrip")]
//! Lane g12: the cards card lanes left on forms they settled for their own cards,
//! landed or moved onto a form naming what they wait on. It reads the corpus and the
//! printed words for these cards alone, and has nothing to say without them.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

/// The card, what its module has to say, and whether it is whole.
const CARDS: &[(&str, &str, bool)] = &[
    // Modes told apart by what was done to cast it are `ModalDef::allowed`, and
    // the mode a target implies is one slot branched on at resolution (CR 700.2).
    ("Jeskai Revelation", "_implied", true),
    // "If you discarded ...": a flattened choice keeps its outer mode's requirement.
    ("Typhoid Mary, Fractured", ".allowed(", false),
    ("Savior of Ollenbock", "_implied", false),
    // A reflexive ability's modes implied by its target (CR 603.12).
    ("Etched Slith", "_implied", false),
    // "One or more ... that many": the batch's members.
    ("Struggle for Project Purity", "q.event_objects(item).len()", false),
    // "You may suspect a creature" paid as a cost (CR 701.60).
    ("Frantic Scapegoat", "g.suspect(o)", false),
    // `TriggerMembers` is told the event's subject alone.
    ("Arcee, Acrobatic Coupe", "the-members-a-trigger-keeps-read-through-its-source", false),
    // `Game::attach` hosts an object, never a player.
    ("Lynde, Cheerful Tormentor", "an-aura-attached-to-a-player-by-an-effect", false),
    // What Shuriken was unattached from is the creature it gave the ability.
    ("Shuriken", "Subtype::Ninja", false),
    // One search of graveyard, hand and/or library with two find clauses
    // (CR 701.23), the library shuffled only if it was searched.
    ("Agency Outfitter", "Search it too", true),
    ("Agency Outfitter", "one for each thing searched for", true),
    ("Agency Outfitter", "if searched_1", true),
];

#[test]
fn modules_say_what_their_cards_say() {
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
