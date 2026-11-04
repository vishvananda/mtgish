#![cfg(feature = "cantrip")]
//! Forms lane g7 landed in the generator, each written as Cantrip's own card
//! prints it, on the cards that waited on them. Moritte, Orcish Farmer, Sakashima
//! and Zombie Musher stay held by other forms; their landed part is checked here.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

/// The card, what its module has to say, and whether it is whole.
const CARDS: &[(&str, &str, bool)] = &[
    // "Has annihilator X, where X is ...": the trigger reads X as it resolves.
    ("Ulamog, the Defiler", "cantrip_mechanics::triggers::annihilate_n(g, item, n)", true),
    ("Ulamog, the Defiler", "delayed: &[triggered(cantrip_mechanics::triggers::ANNIHILATOR,", true),
    // "Hexproof from each of its colors", read in layer 6 (CR 702.11d).
    ("Tam, Mindful First-Year", "HexproofFrom(chars.colors.0 as u32)", true),
    // "All colors" is no color word (CR 612.2).
    ("Tam, Mindful First-Year", "EffectKind::SetColors(ColorSet(31))", true),
    ("Scrapbasket", "EffectKind::SetColors(ColorSet(31))", true),
    // Damage dealt as though its source had infect is a rule asked of the player.
    ("Phyrexian Unlife", "infect_damage_to: &[ability_1_0_infect]", true),
    // Planeswalker types: subtypes of the planeswalker kind.
    ("Tam, the Possibility", "kind.card_types().has(CardType::Planeswalker)", true),
    // Beginning the game on the battlefield with terms (CR 103.6a).
    (
        "Gemstone Caverns",
        "opening_hand: Some(OpeningHandAction { condition: ability_0_may_begin, counter: Some((CounterKind::Luck, 1)), exile_from_hand: 1 })",
        true,
    ),
    // "When you do, choose up to that many".
    ("Tranquil Frillback", ".up_to_that_many()", true),
    ("Hawkeye, Master Marksman", ".up_to_that_many()", true),
    // A granted attack limit, asked by the card of what has the grant.
    ("Tomik, Orzhov Lawmage", "attack_limits: &[granted_0_limit_held]", true),
    ("Tomik, Orzhov Lawmage", "q.chars(id).abilities.contains(&Some(granted))", true),
    // Flashback for a price of its own.
    ("Iroh, Grand Lotus", "granted_ways: &[GrantedWay { text: \"Flashback\", flashback: true, cost: ability_1_0_cost, ..GrantedWay::CAST },GrantedWay { text: \"Flashback\", flashback: true, cost: ability_2_0_cost", true),
    // Harness: a designation that stays with the permanent.
    ("The Mind Stone", "g.add_effect(object, object, EffectKind::Harnessed, Duration::Indefinite)", true),
    ("The Soul Stone", "g.add_effect(object, object, EffectKind::Harnessed, Duration::Indefinite)", true),
    // Emerge from artifact.
    ("Crabomination", "filter: cantrip_mechanics::targets::filters::artifact, action: PermanentAction::Sacrifice, tapped: false }), emerge: true", true),
    // Tap creatures rather than pay mana.
    ("Heirloom Epic", ".tap_creatures_for_mana()", true),
    // Held by other forms; the landed part.
    ("Zombie Musher", "Keyword::SnowLandwalk", false),
    ("Moritte of the Frost", "with_keyword(Keyword::Changeling)", false),
    ("Orcish Farmer", "Duration::UntilNextTurnOf(", false),
];

/// Features these cards no longer hold.
const GONE: &[(&str, &str)] = &[
    ("Sakashima of a Thousand Faces", "granted DeckRule"),
    ("Zombie Musher", "landwalk of Supertype"),
    ("Moritte of the Frost", "event change as it enters Copiable GrantAbility"),
    ("Orcish Farmer", "duration UntilNextUntapStep"),
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
        let module = modules
            .get(*name)
            .unwrap_or_else(|| panic!("no module for {name}"));
        assert!(module.contains(says), "{name} should have {says}:\n{module}");
        if *whole {
            assert!(!module.contains("planned"), "{name} should be whole:\n{module}");
        }
    }
    for (name, form) in GONE {
        let module = modules
            .get(*name)
            .unwrap_or_else(|| panic!("no module for {name}"));
        assert!(!module.contains(&format!("\"{form}\"")), "{name} should not hold {form}:\n{module}");
    }
}
