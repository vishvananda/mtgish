#![cfg(feature = "cantrip")]
//! What events, triggers and replacements say of real cards. It reads the corpus
//! for these cards alone, and has nothing to say without it.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

/// The card, and what its module has to say.
const CARDS: &[(&str, &str)] = &[
    // A surveil reports the cards it put in the graveyard.
    ("Enlightened Confidant", "surveil_into_graveyard"),
    // Excess damage is the event's own number.
    ("Fall of Cair Andros", "recipient_excess"),
    // A later slot's rule reads what an earlier slot chose.
    ("Band Together", "q.chosen_targets()"),
    // "Whenever you cast a spell with one or more targets": how many it has.
    ("Voracious Bibliophile", "q.state().targets(i).len()"),
    // "Each token ... is created under your control instead".
    ("Crafty Cutpurse", "t.controller ="),
    // What Cantrip doesn't have yet is named as a requirement.
    ("Vial Smasher the Fierce", "a choice of Recipient"),
    ("Land Equilibrium", "event change then"),
    ("Twinning Staff", "replacement event CopySpell"),
    ("Vedalken Squirrel-Whacker", "replacement of RollD6"),
    ("Grifter's Blade", "what a permanent could be attached to"),
    (
        "\"Lifetime\" Pass Holder",
        "amount of an event its trigger does not carry",
    ),
];

/// Cards whose source lost a word, so code from it would say the opposite of the
/// card: "if it isn't a mana ability" reaches the generator as "if it is". Each
/// keeps its gap.
const LOST: &[(&str, &str)] = &[("Harsh Mentor", "event object this trigger does not bind")];

fn modules(names: Vec<String>) -> Option<BTreeMap<String, String>> {
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/mtgish.lines.ron");
    if !corpus.exists() {
        return None;
    }
    Some(
        cantrip::modules_named(&corpus, &names)
            .unwrap()
            .into_iter()
            .collect(),
    )
}

#[test]
fn modules_say_what_their_cards_say() {
    let names = CARDS.iter().map(|(name, _)| name.to_string()).collect();
    let Some(modules) = modules(names) else {
        return;
    };
    let silent: Vec<String> = CARDS
        .iter()
        .filter(|(name, says)| !modules.get(*name).is_some_and(|m| m.contains(says)))
        .map(|(name, says)| {
            let start = modules
                .get(*name)
                .map_or("no module", |m| &m[..m.len().min(200)]);
            format!("{name} does not say {says}: {start}")
        })
        .collect();
    assert!(silent.is_empty(), "{}", silent.join("\n"));
}

#[test]
fn a_filter_that_lost_its_negation_stays_a_hole() {
    let names = LOST.iter().map(|(name, _)| name.to_string()).collect();
    let Some(modules) = modules(names) else {
        return;
    };
    for (name, gap) in LOST {
        let module = modules
            .get(*name)
            .unwrap_or_else(|| panic!("no module for {name}"));
        assert!(module.contains(gap), "{name} has lost its gap:\n{module}");
    }
}
