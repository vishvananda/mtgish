#![cfg(feature = "cantrip")]
//! Piles, choices made outside a body, a chosen direction, what players do, per-player
//! walks and coin flips: forms the generator now writes on the cards that waited on
//! them, and the cards that hold on a form naming what Cantrip lacks or what the data
//! says. It reads the corpus and the printed words for these cards alone, and has
//! nothing to say without them.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

/// The card, what its module has to say, and whether it is whole.
const CARDS: &[(&str, &str, bool)] = &[
    // Piles one player separates and another chooses from (CR 700.3).
    ("Bend or Break", "choose::separate(g, Some(separator_1), asker.source, rest_1)?", true),
    ("Make an Example", "choose::separate(", true),
    ("Phyrexian Portal", "FaceDownPile", true),
    // A pile chosen at random.
    ("Whims of the Fates", "g.choose_at_random(row.1.len())", true),
    // What each player gains control of is what the next controlled as it began.
    ("Aminatou, the Fateshifter", "snap_7[index_6]", true),
    // "Choose left or right": turn order, or its reverse after the first.
    ("Order of Succession", "players_1[1..].reverse();", true),
    // Players paying repeatedly, and what went unpaid.
    ("Fade Away", "g.may_pay_times(", true),
    // Flipping until a flip is lost or the player stops.
    ("Fiery Gambit", "flip again?", true),
    ("Negative Zone Portal", "flip_coin(player_2)", true),
    // A token copy of one of the creatures each player chose.
    ("Zndrsplt's Judgment", "Purpose::CopyWhich", true),
    // An emblem of the creature type chosen as it resolved.
    ("Oko, Lorwyn Liege", "create_emblem_choosing_subtype(", true),
    // What each player put onto the battlefield this way, under their control.
    ("Guild Feud", "Some(q.state().obj(id).controller) == (", true),
    // The players' cards shuffled in together, and each draws their own share.
    ("The Great Aurora", "q.state().obj(id).owner == bound_0", true),
    // "Another player of your choice may gain control of that permanent."
    ("Stardew Valley", "ask_may(g, decider_", true),
    // Each opponent's process in turn order, that player paying.
    ("Protection Racket", "g.may_pay(payer_", true),
    ("Sivriss, Nightmare Speaker", "for bound_0 in players_0.iter().copied() {", true),
    // Each opponent's card offered by your "may", all of them entering together.
    ("Sepulchral Primordial", "gathered_1.extend(offered_2)", true),
    // Tokens of the chosen color and creature type, named for that type.
    ("Volrath's Laboratory", ".named(kind_", true),
    ("Riptide Replicator", ".named(kind_", true),
    ("Sarpadian Empires, Vol. VII", "(Color::Blue, Subtype::Camarid)", true),
    // Each player puts what they turned face up onto the battlefield.
    ("Pyxis of Pandemonium", "g.turn_face_up(", true),
    // An explicitly ordered auction takes and moves one card before the next
    // player chooses; the remaining exiled pool controls repetition.
    ("Thieves' Auction", "g.put_all_onto_battlefield_as", true),
    // What Cantrip lacks, and what the data says.
    ("Kozilek, the Broken Reality", "per-player-manifest", false),
    ("Leori, Sparktouched Hunter", "a-resolution-s-choice-read-by-the-trigger-it-creates", false),
    ("Zombie Boa", "a-resolution-s-choice-read-by-the-trigger-it-creates", false),
    ("Phyrexian Splicer", "a-value-chosen-as-an-activation-cost-read-by-its-targets", false),
    ("Infernal Denizen", "operation-players-do", false),
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
