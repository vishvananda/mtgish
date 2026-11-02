#![cfg(feature = "cantrip")]
//! What the generator says of costs, casting and play permissions on real cards.
//! It reads the corpus for these cards alone, and has nothing to say without it.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

/// The card, and what its module has to say.
const CARDS: &[(&str, &str)] = &[
    // "Kicker {1}{U} and/or {B}": one is the kicker, the other an optional cost.
    ("Ana Battlemage", "OptionalCost::new(Cost::mana("),
    // A kicker or a squad that is no mana.
    ("Phyrexian Scuta", "kicker::non_mana("),
    ("Thrill-Kill Disciple", "multikicker_cost("),
    // "You may cast this as though it had flash if you pay {2} more": a rider sees it
    // cast out of its time, by the mark of its own flash.
    ("Asinine Antics", "marks.untimely_by_own_flash"),
    // The cost of the card's own ability, not a way of casting it.
    ("Masked Bandits", "land_until_cast::ability"),
    // Read as it was cast, and in the order of the casts.
    ("Lethal Exploit", "fact_as_cast"),
    ("Sentinel Tower", "q.history().casts().take_while"),
    // Once each turn, of a spell alternative.
    ("Darksteel Monolith", "permission_used_this_turn"),
    // Which of a card's ways was taken.
    ("Baleful Mastery", "item.way == 1"),
    // The mana value of the objects, a spell's with its X.
    ("Counterbalance", "q.mana_value(o)"),
    // Flashback a resolving effect gives, at a price of its own.
    ("Archmage's Newt", "flashback: true"),
    // A rule of the combat.
    ("Crawlspace", "attack_limits"),
    // These hold, as what they want the engine has no way to say.
    (
        "Alien Symbiosis",
        "engine: a card's own terms to cast it from its graveyard",
    ),
    (
        "Earthlore",
        "engine: a cost that taps what its payer may not control",
    ),
    ("Urianger Augurelt", "engine: a play permission that costs less"),
    (
        "Berg Strider",
        "engine: snow mana told apart as a spell is paid for",
    ),
    (
        "The Fugitive Doctor",
        "engine: a two-word subtype in a card's type line",
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
