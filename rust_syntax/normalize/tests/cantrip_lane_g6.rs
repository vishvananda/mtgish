#![cfg(feature = "cantrip")]
//! Lane g6's forms: what the generator now writes for the cards that waited on
//! them, and the cards that hold on a hole naming what Cantrip or the data lacks.
//! It reads the corpus and the printed words for these cards alone, and has
//! nothing to say without them.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

/// The card, what its module has to say, and whether it is whole.
const CARDS: &[(&str, &str, bool)] = &[
    // "Add two mana of different colors": each color among those not yet chosen.
    ("Interplanar Beacon", "left = ColorSet(left.0 & !ColorSet::of(color).0);", true),
    ("Interplanar Beacon", ".yields_any(2)", true),
    // Each opponent chooses among the cards no earlier opponent chose.
    ("Manifold Insights", "unpicked_", true),
    // One card for each color pair.
    ("Niv-Mizzet Reborn", "one_of_each_color_pair", true),
    // A total mana value read as it resolves.
    ("Smelting Vat", "total_mana_value_at_most(most_", true),
    // An empty cost is paid.
    ("Tectonic Giant", "let paid_", true),
    // "You may unattach an Equipment from a creature you control. If you do".
    ("Akiri, Fearless Voyager", "g.unattach(e); unattached_", true),
    // "Move one or more counters": at least one, of whichever kinds.
    ("Goldberry, River-Daughter", "let least = u32::from(done_", true),
    // Another counter of a kind a player has.
    ("Animation Module", "g.give_poison(player, 1)", true),
    ("Animation Module", "labels: &[\"Poison counter\", \"Energy counter\", \"Experience counter\"]", true),
    // "Play lands and cast spells from one of those piles".
    ("Brilliant Ultimatum", "g.could_play_land_during_resolution(c)", true),
    // A Room on the battlefield is named by its unlocked doors.
    ("Central Elevator", "cantrip_mechanics::library::object_names(q, id)", true),
    // Mana equal to a permanent's mana cost, a choice for each hybrid symbol.
    ("Elemental Resonance", "let cost = g.mana_cost(o);", true),
    // Several objects returned tapped and attacking (CR 508.4).
    ("Noctis, Heir Apparent", "g.enter_attacking(&[entered], defender);", true),
    // "Each of them enters with an additional ... counter if it's a creature".
    ("Semester's End", "EffectKind::Replacement { card: item.ability.card, index: 1 }", true),
    // "Sacrifice half the lands you control, rounded up" as its total cost is determined.
    ("Tectonic Split", "cost_of_targets: Some(ability_0_of_board)", true),
    // Hexproof from the chosen color; the block rule reading it is Cantrip's gap.
    ("Skrelv, Defector Mite", "HexproofFrom::color(", false),
    ("Skrelv, Defector Mite", "a-lasting-block-rule-reading-what-its-ability-chose", false),
    ("Sungold Sentinel", "a-lasting-block-rule-reading-what-its-ability-chose", false),
    ("The Black Gate", "a-lasting-block-rule-reading-what-its-ability-chose", false),
    // The data pays for one tied creature and sacrifices another choice.
    ("Tariff", "a mana cost paid for one creature and a sacrifice choosing again", false),
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
