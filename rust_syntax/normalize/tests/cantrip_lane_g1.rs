#![cfg(feature = "cantrip")]
//! The X a card announces and the up-to counts it reads: what the generator now writes
//! for the cards that waited on them, and the forms the rest hold on, each one a read
//! Cantrip has no place for. It reads the corpus and the printed words for these cards
//! alone, and has nothing to say without them.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

/// The card, what its module has to say, and whether it is whole.
const CARDS: &[(&str, &str, bool)] = &[
    // "Each with mana value less than or equal to the result" of a die roll: the
    // reflexive ability's told amount, judged on the targets together.
    ("Ancient Brass Dragon", "total_mana_value_at_most(TargetSpec::COUNTED)", true),
    // "When turned face up, ... up to X target creatures": the X paid to turn it up.
    ("Aurelia's Vindicator", ".counted_by(", true),
    ("Aurelia's Vindicator", ".x)", true),
    // "X plus one damage divided as you choose among any number of targets".
    ("Meteor Shower", "of_x: Some(", true),
    ("Meteor Shower", "TargetSpec::ANY_NUMBER", true),
    ("Shatterskull Smashing", "of_x: Some(", true),
    // A planeswalker with loyalty X enters with X loyalty counters (CR 306.5b).
    ("Nissa, Steward of Elements", "enters_with_x_loyalty", true),
    // Removing X counters to activate a mana ability: only {X} is announced there.
    ("Calciform Pools", "removing X counters to activate a mana ability", false),
    ("Petalmane Baku", "removing X counters to activate a mana ability", false),
    ("Haruspex", "removing X counters to activate a mana ability", false),
    // A card with mana value X as a cost: a cost's filter has no X to read.
    ("Blazing Shoal", "a cost's filter reading the X announced", false),
    ("Scrap Welder", "a cost's filter reading the X announced", false),
    ("Kozilek, the Great Distortion", "a cost's filter reading the X announced", false),
    // The replacement an X ability made, applied later.
    ("Aladdin's Lamp", "the X of the ability that made a replacement effect", false),
    // "When it becomes monstrous, ... up to X": the event keeps no X.
    ("Death Kiss", "the-x-of-the-monstrosity-a-becomes-monstrous-trigger-reads", false),
    ("Vitality Hunter", "the-x-of-the-monstrosity-a-becomes-monstrous-trigger-reads", false),
    // An activated ability's up-to count read from the game.
    (
        "Agility Bobblehead",
        "an activated ability's target count read as it is activated",
        false,
    ),
    // "Up to X target cards from that opponent's graveyard": read from another target.
    ("Reap", "a target count read from another target", false),
    // "Flash as long as X is ...": judged before X is announced.
    ("Silver Scrutiny", "a-limit-on-the-x-announced", false),
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
