#![cfg(feature = "cantrip")]
//! Lane g9: forms the generator now writes in Cantrip's idiom, on the cards that
//! waited on them, and the cards that moved to a form naming what still stops
//! them. It reads the corpus and the printed words for these cards alone, and has
//! nothing to say without them.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

/// The card, what its module has to say, and whether it is whole.
const CARDS: &[(&str, &str, bool)] = &[
    // "As this enters, choose a card type other than creature": the other types,
    // kept as its chosen option.
    (
        "Stenn, Paranoid Partisan",
        "choose::option_as_enters(g, entering, &CARD, 0..7)",
        true,
    ),
    // "Your choice of a counter from among first strike, vigilance, and lifelink".
    ("Aragorn, Company Leader", "\"First strike\"", false),
    // A bestow cost of mana and more: the more is the Aura's additional cost.
    ("Detective's Phoenix", "additional_cost: Some(", true),
    // "You may behold a [quality]" as an ability resolves, or as it enters.
    ("Elven Passage", "behold::on_resolution", true),
    ("Sarkhan, Dragon Ascendant", "behold::on_resolution", true),
    ("Theorist's Sanctum", "behold::as_enters", true),
    // A mana ability's "becomes that color".
    ("Foraging Wickermaw", "EffectKind::SetColors", true),
    // Ward—Get five poison counters.
    ("The Serpent Society", "ward_poison::<5>", true),
    // Enchant creature put onto the battlefield with it, and the put that does it.
    (
        "Necromancy",
        "becomes_aura_enchant: Some(cantrip_mechanics::targets::CREATURE_PUT_ONTO_THE_BATTLEFIELD_WITH_THIS)",
        true,
    ),
    ("Necromancy", "g.put_onto_battlefield_with_aura(o, player_1, asker.source)", true),
    // A granted miracle whose cost is the card's mana cost reduced by {4}.
    ("Aminatou, Veil Piercer", "granted_miracles: &[GrantedMiracle {", true),
    ("Aminatou, Veil Piercer", "less: 4", true),
    // The loyalty counters removed to activate it, X for a -X.
    ("Way of the Mind Sculptor", "Some(cost) if cost == LOYALTY_MINUS_X", true),
    // The creature returned for web-slinging, from the records before the cast.
    ("Scarlet Spider, Ben Reilly", "from.is(ZoneKind::Battlefield) && to.is(ZoneKind::Hand)", true),
    // Collect evidence of the total mana value of the permanents it targets.
    ("Urgent Necropsy", "cost_of_targets: Some(ability_0_of_targets)", true),
    ("Urgent Necropsy", "chosen.dedup()", true),
    // "At the beginning of your next main phase".
    ("Mana Drain", "Step::PostcombatMain } else { Step::PrecombatMain }", true),
    ("Mana Sculpt", "Step::PostcombatMain } else { Step::PrecombatMain }", true),
    // The damage dealt, no more than the recipient's life, loyalty or toughness.
    ("Drain Life", "clamped_damage_0 += dealt.iter().zip(&caps)", false),
    // The player who revealed the card with the greatest mana value.
    ("Psychic Battle", "let extreme = those.iter().map(|&o| q.mana_value(o)).max();", false),
    // What the data says other than the Oracle text, and what Cantrip lacks.
    (
        "Cloak and Dagger, Entwined",
        "an-exile-until-this-leaves-the-battlefield-dropped-from-a-choice-in-the-data",
        false,
    ),
    ("Winter's Chill", "a-limit-on-the-x-announced", false),
    (
        "Jhoira, Weatherlight Corsair",
        "the-life-you-lose-read-as-the-revealing-player-s-in-the-data",
        false,
    ),
    ("Apex Observatory", "a-card-type-shared-among-the-cards-used-to-craft-it", false),
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
