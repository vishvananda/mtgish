#![cfg(feature = "cantrip")]
//! Lane g13: the last cards of forms that still computed to the generator, landed or
//! moved onto a form naming what they wait on. It reads the corpus and the printed
//! words for these cards alone, and has nothing to say without them.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

/// The card, what its module has to say, and whether it is whole.
const CARDS: &[(&str, &str, bool)] = &[
    // "Exile it with N time counters", N read of each card exiled.
    ("The Wedding of River Song", "g.add_counters_together(&puts)", false),
    ("The Wedding of River Song", "|&moved_o|", false),
    // A cost paid instead of a card's mana cost that reads that card.
    ("Kentaro, the Smiling Cat", "\"a cost reading the card it is paid for\"", false),
    // "For as long as ... remain exiled" on a play permission (CR 400.7).
    ("Extraordinary Journey", "Duration::Indefinite", false),
    ("Durnan of the Yawning Portal", "Duration::Indefinite", false),
    // A permanent spell's own exile cost is linked to the permanent (CR 607.2q).
    ("Gorex, the Tombshell", "a card exiled to pay a permanent spell's cost", false),
    // Exile piles: face down and face up.
    ("Abstract Performance", "g.exile_face_down(&objects_0, None)", false),
    ("Abstract Performance", "g.exile_all(&objects_1)", false),
    // Per-player token creations no gather makes as one batch.
    ("Mardu Siegebreaker", "per-player-tapped-token-copies-of-several-objects", false),
    ("Peacekeeper Avatar", "per-player-tokens-entering-attached", false),
    // Each player choosing among actions.
    ("Nicol Bolas, Dragon-God", "per-player-choice-of-what-to-exile-exiled-together", false),
    ("Eumidian Wastewaker", "per-player-choice-among-different-actions-done-at-once", false),
    // An embalm or eternalize ability by its printed word.
    ("Vizier of the Anointed", "d.text.starts_with(\"Eternalize\")", false),
    ("Vizier of the Anointed", "d.text.starts_with(\"Embalm\")", false),
    ("Market Gnome", "d.text.starts_with(\"Craft with\")", false),
    // The target of an activated ability: the newest item of that source.
    ("Professor Hojo", "i.kind == StackKind::Activated", false),
    // The players an instant that deals damage once dealt damage to.
    ("Aurelia's Fury", "target: Target::Player(p)", false),
    // Control gained until end of turn.
    ("Seize the Spotlight", "Duration::EndOfTurn); } }); }", false),
    // Aura cards that were attached: the cards those Auras became.
    ("Cass, Hand of Vengeance", "new == id && was.contains(&old)", false),
    // "Do this only once each turn" shared by a sentence's two abilities.
    ("Glava, Five-Advents Mage", "g.note_done(asker.source, AbilityRef { card: g.chars(asker.source).card, index: 2", false),
    // What each player exiled from their own library, by owner.
    ("Rocco, Street Chef", "Some(q.state().obj(o).owner) == Some(bound_0)", false),
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

/// Glava's two abilities spend one key: a thing done once each turn (603.2h).
#[test]
fn a_sentence_s_abilities_share_one_do_once_key() {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let (corpus, oracle) = (data.join("mtgish.lines.ron"), data.join("oracle.json"));
    if !corpus.exists() || !oracle.exists() {
        return;
    }
    let names = vec!["Glava, Five-Advents Mage".to_string()];
    let module = cantrip::modules_printed(&corpus, &oracle, &names).unwrap().remove(0).1;
    let keys: std::collections::BTreeSet<&str> = module
        .match_indices("g.note_done(asker.source, AbilityRef { card: g.chars(asker.source).card, index: ")
        .map(|(at, found)| &module[at + found.len()..at + found.len() + 3])
        .collect();
    assert_eq!(keys.len(), 1, "{module}");
    assert!(!module.contains("limit_shared_with"), "{module}");
}
