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
    // A later slot's rule reads what an earlier slot chose.
    ("Band Together", "q.chosen_targets()"),
    // "Whenever you cast a spell with one or more targets": how many it has.
    ("Voracious Bibliophile", "q.state().targets(i).len()"),
    // "Each token ... is created under your control instead".
    ("Crafty Cutpurse", "t.controller ="),
    // "That player or a planeswalker that player controls": chosen as it resolves.
    ("Vial Smasher the Fierce", "candidates: candidates_"),
    // What Cantrip doesn't have yet is named as a requirement.
    ("Land Equilibrium", "event change then"),
    ("Twinning Staff", "replacement event CopySpell"),
    ("Vedalken Squirrel-Whacker", "replacement of RollD6"),
    ("Grifter's Blade", "what a permanent could be attached to"),
    // The amount follows the event Cantrip lacks.
    ("\"Lifetime\" Pass Holder", "trigger event Player RollsToVisit"),
    // "If those permanents are exchanged this way".
    ("Gauntlets of Chaos", "traded_"),
    // "For each counter you put on a creature this way": once per counter.
    ("Shambling Swarm", "std::iter::repeat(o).take(share)"),
    ("Bounty of the Hunt", "std::iter::repeat(o).take(share)"),
    // A reflexive ability's damage comes from a target of what made it.
    ("Thorin, Mountain-king", "item.subject.object()"),
    // "That permanent or player": the two are read from different places.
    ("Ghyrson Starn, Kelermorph", "ability_1_or_1_event"),
    // "The damage dealt this way", of a batch gathered one player at a time.
    ("Spiteful Repossession", "let done_"),
    // What Cantrip doesn't have yet is named as a requirement.
    (
        "Sacred Boon",
        "damage prevented by an effect of this ability",
    ),
    // The sentence's limit, shared by its two abilities' triggerings.
    ("Bill Potts", ".only_if(ability_0_shared_limit)"),
    ("Market Gnome", "trigger event WhileActivating"),
    (
        "Divine Deflection",
        "target reference outside a resolving ability",
    ),
    // "Each other creature and each player" (#156): creatures, not every permanent.
    (
        "Arcbond",
        "q.chars(id).is(CardType::Creature) && !(Some(asker.source) == Some(id))",
    ),
    (
        "Undiscovered Paradise",
        "replacement made outside a resolving ability",
    ),
    ("The Mimeoplasm", "event change as it enters Entry CopyOf"),
    ("Martyrdom", "replacement made by a granted ability"),
    // A spell as a source of damage is an object on the stack.
    ("Suffocation", "q.state().obj(source).zone.is(ZoneKind::Stack)"),
    // Its as-enters exile reads its cast record: only the play is left.
    ("Primeval Spawn", "feature: \"operation PlayWith\""),
    ("Quarum Trench Gnomes", "ReplacementKind::ManaProduction"),
    // A granted exalted is the word its grant prints.
    ("First Sliver's Chosen", "subject_gets_plus_one, \"Exalted\")"),
];

/// Cards whose source lost a word, so code from it would say what the card does
/// not: "if it isn't a mana ability" reaches the generator as "if it is", and "a
/// spell" as anything. Each keeps its gap.
const LOST: &[(&str, &str)] = &[
    ("Harsh Mentor", "a mana ability test that lost its negation"),
    ("Minds Aglow", "an amount read by each player as their own"),
];

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

#[test]
fn excess_trigger_payload_is_not_confused_with_the_damage_amount() {
    for (features, expected) in [
        (&[][..], "recipient_excess"),
        (&["wide-effect-quantities"][..], "exact wide event quantity outside its local callback"),
    ] {
        let files = cantrip::planned::with_features(features, || modules(vec!["Fall of Cair Andros".to_owned()]));
        let Some(files) = files else { return };
        let module = &files["Fall of Cair Andros"];
        assert!(module.contains(expected), "{module}");
        if !features.is_empty() {
            assert!(module.contains("// gaps:"), "exact excess payload capture is not claimed implemented: {module}");
        }
    }
}
