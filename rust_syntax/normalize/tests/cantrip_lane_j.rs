#![cfg(feature = "cantrip")]
//! What the reads and triggers of the Cantrip backend say of real cards. It reads
//! the corpus for these cards alone, and has nothing to say without it.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

/// The card, and what its module has to say.
const CARDS: &[(&str, &str)] = &[
    (
        "Sheltering Ancient",
        "Payment::PutPlusOnePlusOneCounterOnAnOpposingCreature",
    ),
    ("Wall of Shards", "Payment::OpponentGainsLife(1)"),
    ("Polar Kraken", "Payment::Sacrifice("),
    ("Lethal Sting", "cantrip_mechanics::blight::cost(1)"),
    ("Sanctuary Wall", "ask_may"),
    ("Supper for Spiders", "put_all_onto_battlefield_becoming"),
    (
        "Silver Surfer, Cosmic Voyager",
        "put_all_onto_battlefield_as",
    ),
    ("Keruga, the Macrosage", "deck.cards.iter().all"),
    ("Yorion, Sky Nomad", "deck.minimum_size"),
    ("Thieving Skydiver", "x_cant_be_zero: true"),
    ("Constant Mists", "Kicker { cost: COST.mana"),
    (
        "The Scarab God",
        ".typed(TypeSet::of(&[CardType::Creature])",
    ),
    ("Excruciator", "source == asker.source"),
    ("Basalt Ravager", "s.card_types().has(CardType::Creature)"),
    ("Endless Atlas", "cantrip_mechanics::same_name(q, a, b)"),
    ("Bogardan Hellkite", "damage_divided(g, item)"),
    ("Patrician Geist", "zone_cast_from(q, spell)"),
    // What Cantrip doesn't have yet is named as a requirement.
    ("Alabaster Leech", "a spell costing colored mana more"),
    ("Disa the Restless", "a token that is a named card"),
    (
        "Mantle of the Ancients",
        "several objects arriving attached",
    ),
    (
        "Semester's End",
        "what enters having counters by what it is",
    ),
    (
        "Psychic Vortex",
        "cumulative upkeep paid other than in mana or life",
    ),
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
