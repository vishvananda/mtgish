#![cfg(feature = "cantrip")]
//! Forms Cantrip could already express that the generator now writes (lane g5:
//! filters, searches and events), and the cards that hold on a form naming what
//! Cantrip or the data lacks. It reads the corpus and the printed words for these
//! cards alone, and has nothing to say without them.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

/// The card, what its module has to say, and whether it is whole.
const CARDS: &[(&str, &str, bool)] = &[
    // "A name originally printed in the Homelands expansion": the names, each.
    ("Apocalypse Chime", "\"Aysen Abbey\" |", true),
    ("Golgothian Sylex", "\"Mishra's Factory\" |", true),
    // The rest of a prevention for each source it prevented damage from.
    ("Comeuppance", "find(|k| k.source == p.source)", true),
    // "Of an opponent's choice": the controller says which opponent chooses, among
    // the lands they control.
    ("Demonic Hordes", "cantrip_mechanics::choose::player(g, asker.controller", true),
    ("Demonic Hordes", "Some(asker.controller) == Some(q.controller(id))", true),
    // "Each foe chooses one untapped creature they control, then taps the rest."
    ("Regna's Sanction", "Purpose::Keep, selected_", true),
    // "If you controlled ... as you cast this spell": kept as it is cast.
    ("Flame Discharge", "g.history().fact_as_cast(item.object) != 0", true),
    ("Flame Discharge", "cast_fact: Some(kept_as_cast)", true),
    ("Steer Clear", "fact_as_cast(item.object)", true),
    ("Faerie Fencing", "fact_as_cast(item.object)", true),
    // A search of library and graveyard for several things, one choice of both.
    ("Journey for the Elixir", "g.searched(searcher_", true),
    ("Nissa's Encouragement", "clauses_", true),
    // "A land card of each basic land type": a clause for each type.
    ("Gaea's Balance", "Subtype::Mountain", true),
    ("Gaea's Balance", "SearchDestination::Battlefield", true),
    // What Cantrip has no read for.
    ("Bold Plagiarist", "counters-the-event-put", false),
    ("Captain Marvel, Apex Avenger", "counters-the-event-put", false),
    ("Confusion in the Ranks", "event value in a target filter beyond its subject", false),
    ("Venerable Warsinger", "event value in a target filter beyond its subject", false),
    ("Rootwater Shaman", "object filter CouldEnchant", false),
    ("Tallowisp", "object filter CouldEnchant", false),
    ("Hordewing Skaab", "the players a one-or-more damage trigger's batch dealt damage to", false),
    ("Malcolm, Keen-Eyed Navigator", "the players a one-or-more damage trigger's batch dealt damage to", false),
    ("Heroism", "damage-prevented-by-what-the-effect-asks-as-it-would-be-dealt", false),
    // The data says something other than the Oracle text.
    (
        "Ecological Appreciation",
        "the chosen cards of a search of more than the library shuffled into it, read as a shuffle in the data",
        false,
    ),
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
