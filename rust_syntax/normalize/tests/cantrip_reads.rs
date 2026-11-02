#![cfg(feature = "cantrip")]
//! What the generator reads of statics, triggers, delayed and reflexive abilities,
//! on the cards whose form isn't obvious from the Oracle text.
//! It reads the corpus for these cards alone, and has nothing to say without it.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

/// The card, and what its module has to say.
const CARDS: &[(&str, &str)] = &[
    // "You may have target player mill": you are asked, and they do it.
    ("Cloudhoof Kirin", "ask_may(g, decider_"),
    // "Unless any player pays": the first who does ends the asking, and what no
    // one paid for goes at once, after every payment.
    ("Cleansing", "unpaid_0.push(bound_0)"),
    // All a player was dealt at once, whatever dealt it.
    ("Quartzwood Crasher", ".sums_per_subject()"),
    // Fixed as it triggers, not read back from the spell.
    ("Storm, Windrider", ".members_of("),
    // Without the printed line nothing says these are the spells "you cast".
    ("Sami, Wildcat Captain", "engine: no Affinity granted"),
    // A creature card in hand is no creature: it may still cycle.
    ("Damping Matrix", "q.is_on_battlefield(id)"),
    // Tokens have no event of their own: what was resolving as they entered.
    ("Arcane Artisan", "Event::Entered"),
    // Suspended as the upkeep reads it.
    ("Time Beetle", "fn time_travel_for"),
    // A replacement's `then` has no stack item to be linked by.
    ("Asmodeus the Archfiend", "g.text_linked(asker.source)"),
    // Holes that stay, each for what the form can't say.
    (
        "Necromancy",
        "an Aura that enchants what it put onto the battlefield",
    ),
    ("Headliner Scarlett", "a card a player may look at for a while"),
    ("Bane Alley Broker", "cards a player may look at"),
    ("Matopi Golem", "operation WithReflexive"),
];

/// A half's meld is its partner's instruction: nothing of it is on this card.
#[test]
fn a_passive_meld_half_is_an_ordinary_card() {
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/mtgish.lines.ron");
    if !corpus.exists() {
        return;
    }
    let names = vec!["Bruna, the Fading Light".to_string()];
    let modules: BTreeMap<String, String> = cantrip::modules_named(&corpus, &names)
        .unwrap()
        .into_iter()
        .collect();
    let module = modules.get("Bruna, the Fading Light").expect("a module");
    assert!(!module.contains("melds_into"), "{module}");
}

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
