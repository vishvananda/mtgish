#![cfg(feature = "cantrip")]
//! Lane g16: forms the generator now writes for its cards. It reads the corpus and
//! the printed words for these cards alone, and has nothing to say without them.
#[path="support/generated_locals.rs"]
mod generated_locals;
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

/// The card, what its module has to say, and whether it is whole.
const CARDS: &[(&str, &str, bool)] = &[
    // "Exile ... until an opponent becomes the monarch" (CR 610.3): a delayed
    // return on becoming the monarch, and nothing exiled if that already happened
    // after the source entered (CR 610.3b).
    ("Palace Jailer", "g.delayed_trigger(DelayedWhen::UntilEvent, item.object", true),
    ("Palace Jailer", "when_asked(EventMask::BECAME_MONARCH, until_", true),
    ("Palace Jailer", "Event::BecameMonarch { player } if q.state().opponents(asker.controller).any(|o| o == *player)", true),
    ("Palace Jailer", "let moved_1 = match over_1 { false =>", true),
    // "Reveal exactly two cards ... with different names from outside the game":
    // chosen with different names, revealed, two or nothing; the opponent's pick is
    // the card put into hand.
    ("Extrapolate the Impossible", ".with(cantrip_mechanics::select::DIFFERENT_NAMES))?", true),
    ("Extrapolate the Impossible", "if picked_1.len() == 2 { picked_1 } else { Vec::new() }", true),
    ("Extrapolate the Impossible", "g.reveal(owner, &cards)", true),
    ("Extrapolate the Impossible", "(carried_5_0,) = (selected_4.clone(),);", true),
    ("Extrapolate the Impossible", "g.bring_into_hand_revealed(&outside)", true),
    // "Then puts all cards revealed this way into their graveyard": every
    // opponent's revealed cards, moved together once.
    ("Mind Grind", "g.move_together(&moves, MoveReason::Other)", false),
    // "Choose left or right" as it enters, and the nearest opponent that way.
    ("Pramikon, Sky Rampart", "choose::option_as_enters(g, entering, &CARD, 0..2)", false),
    ("Pramikon, Sky Rampart", "q.state().opponents(p).into_iter().collect()", false),
    // A face-down and a face-up pile exiled whole, an opponent chooses one
    // (CR 700.3b-d), and "the other pile" is the one not chosen.
    ("Abstract Performance", "choose::face_down_or_face_up_pile(g, chooser_3, asker.source, moved_0.clone(), moved_1.clone())", true),
    ("Abstract Performance", "chosen_3.clone(), other_3.clone()", true),
];

/// Forms these cards no longer hold.
const GONE: &[(&str, &str)] = &[
    ("Mind Grind", "per-player-change-zone"),
    ("Stronghold Gambit", "per-player-change-zone"),
    ("Pramikon, Sky Rampart", "a-choice-of-direction"),
    ("Abstract Performance", "result_3).contains"),
];

#[test]
fn modules_say_what_their_cards_say() {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let (corpus, oracle) = (data.join("mtgish.lines.ron"), data.join("oracle.json"));
    if !corpus.exists() || !oracle.exists() {
        return;
    }
    let mut names: Vec<String> = CARDS
        .iter()
        .map(|(name, ..)| name.to_string())
        .chain(GONE.iter().map(|(name, _)| name.to_string()))
        .collect();
    names.sort();
    names.dedup();
    let modules: BTreeMap<String, String> = cantrip::modules_printed(&corpus, &oracle, &names)
        .unwrap()
        .into_iter()
        .collect();
    for (name, says, whole) in CARDS {
        let module = modules.get(*name).unwrap_or_else(|| panic!("no module for {name}"));
        assert!(generated_locals::contains(&module,says), "{name} should have {says}:\n{module}");
        if *whole {
            assert!(!module.contains("planned"), "{name} should be whole:\n{module}");
        }
    }
    for (name, gone) in GONE {
        let module = modules.get(*name).unwrap_or_else(|| panic!("no module for {name}"));
        assert!(!module.contains(gone), "{name} should no longer have {gone}:\n{module}");
    }
}
