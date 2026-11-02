#![cfg(feature = "cantrip")]
//! What a resolving effect says of real cards. It reads the corpus for these
//! cards alone, and has nothing to say without it.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

/// The card, and what its module has to say.
const CARDS: &[(&str, &str)] = &[
    // "Up to that many target": the slot is counted, and the ability is told.
    ("Miasma Demon", "reflexive_trigger_counted"),
    ("Miasma Demon", "between(0, TargetSpec::COUNTED)"),
    // What paid is what was picked, so "for each creature tapped this way" reads it.
    ("Ledev Champion", "Purpose::Tap"),
    // A choice of a counter is a choice of a kind.
    ("Ichormoon Gauntlet", "Label::Counter(k)"),
    // A chosen permanent stays with the text that asked.
    ("Dauntless Bodyguard", "object_as_enters"),
    // A countered spell is the card it became.
    ("Spelljack", "countered_"),
    // "When you do" of what happens whatever the game's state.
    ("Prishe's Wanderings", "reflexive_trigger"),
    // What Cantrip doesn't have yet is named as a requirement.
    ("Midnight Crusader Shuttle", "villainous choice"),
    // "That many" of one or more counters at once: their total (CR 603.2c).
    ("Bioessence Hydra", ".sums_per_subject()"),
];

#[test]
fn modules_say_what_their_cards_say() {
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/mtgish.lines.ron");
    if !corpus.exists() {
        return;
    }
    let names: Vec<String> = CARDS.iter().map(|(name, _)| name.to_string()).collect();
    let modules: BTreeMap<String, String> = cantrip::modules_named(&corpus, &names)
        .unwrap()
        .into_iter()
        .collect();
    for (name, says) in CARDS {
        let module = modules
            .get(*name)
            .unwrap_or_else(|| panic!("no module for {name}"));
        assert!(
            module.contains(says),
            "{name} should have {says}:\n{module}"
        );
    }
}
