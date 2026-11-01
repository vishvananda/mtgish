#![cfg(feature = "cantrip")]
//! What the generator says of operations, modes and cost branches on real cards.
//! It reads the corpus for these cards alone, and has nothing to say without it.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

/// The card, and what its module has to say.
const CARDS: &[(&str, &str)] = &[
    // "Choose one. If this spell was kicked, choose any number instead."
    ("Inscription of Abundance", ".when_paid(0, 3)"),
    // Kicked, any number of targets; not, the one.
    ("Divine Resilience", "if_kicked: Some((0, 4294967292))"),
    // The roll is told to the ability that reads it.
    ("Ancient Bronze Dragon", "reflexive_trigger_counted"),
    // A saga's chapter divides as any triggered ability does.
    (
        "Revival of the Ancestors",
        ".divide(Divide { total: 3, of_x: None })",
    ),
    // Every spell is of a type a spell can be.
    (
        "Borne Upon a Wind",
        "CastAsThoughFlash(TypeSet::of(&[CardType::Artifact",
    ),
    // "Each creature not chosen this way", whether one was chosen or not.
    ("Thunderwave", "!chosen.contains(o)"),
    // A spell from outside the game: a choice among two sets is revealed.
    ("Karn, the Great Creator", "bring_into_hand_revealed"),
    // What goes on is named as a requirement.
    ("Alchemist's Retrieval", "cleave that changes its targets"),
    ("Abzan Charm", "target with a divided amount"),
    ("Rhino, Terrible Trampler", "target with a divided amount"),
    ("Riptide Replicator", "token of the chosen color"),
    (
        "Davriel, Soul Broker",
        "a choice among options offered at random",
    ),
    ("Everdream", "engine: splice onto other than Arcane"),
    (
        "Akroma, Angel of Wrath Avatar",
        "a choice of Ability outside its body",
    ),
    ("Typhoid Mary, Fractured", "modal announcement"),
    ("Diligent Farmhand", "source-zone scope on CountsAsNamed"),
    ("Kari Zev, Crew of Two", "operation Conjure"),
    // A delayed ability's target is not a slot of its own.
    ("Bounty of the Hunt", "target reference"),
    // No stated time for spells cast, so no reading of one: a hole, with its reason.
    ("First Family", "spells cast at no stated time"),
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

/// Nothing whole may hold a read of what failed: a body with a gap in it has one.
#[test]
fn a_binding_out_of_a_test_for_a_player_reaches_the_rest() {
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/mtgish.lines.ron");
    if !corpus.exists() {
        return;
    }
    let names = vec!["Bane of Bala Ged".to_string()];
    let modules: BTreeMap<String, String> = cantrip::modules_named(&corpus, &names)
        .unwrap()
        .into_iter()
        .collect();
    let module = modules.get("Bane of Bala Ged").expect("a module");
    assert!(
        !module.contains("depends on an instruction with a gap"),
        "{module}"
    );
}
