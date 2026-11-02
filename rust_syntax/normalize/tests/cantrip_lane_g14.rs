#![cfg(feature = "cantrip")]
//! Lane g14: reads that waited on a form the generator now writes, and reads that
//! follow the hole of what made them. It reads the corpus and the printed words for
//! these cards alone, and has nothing to say without them.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

/// The card, what its module has to say, and whether it is whole.
const CARDS: &[(&str, &str, bool)] = &[
    // "For each of those tokens, you may put an Aura card ... attached to it":
    // one Aura for each token, attached to that one.
    ("Liberated Livestock", "for each_object in", true),
    ("Liberated Livestock", "Some(each_object)", true),
    ("Liberated Livestock", "UpTo((1i64).max(0) as u32)", true),
    // A copy's added ability and keyword (CR 707.9b, 707.10).
    ("Choreographed Sparks", ".with_keyword(Keyword::Haste).with_ability(", true),
    ("Omni-Changeling", ".with_keyword(Keyword::Changeling)", true),
    // Escape a permanent gives the cards in your graveyard (CR 702.138a).
    ("Underworld Breach", "cantrip_mechanics::escape::granted(ability_0_escape_cost)", true),
    ("Underworld Breach", "escape::cost(q.card(q.chars(card).card).cost, 3)", true),
    ("The Master of Keys", "q.chars(card).is(CardType::Enchantment)", true),
    // Ward of discard a card or pay {2}.
    ("Titania, Rugged Rumbler", "triggers::ward_discard_or_pay::<2>(", true),
    // "Each opponent's first noncreature spell each turn costs {1} more": of the
    // player casting it.
    ("Heartwood Storyteller Avatar", "Some(player) == Some(cast.player) && !(cast.chars(q).is(CardType::Creature))", false),
    // "That Aura": the event's object, the host its other (Eriette).
    ("Eriette, the Beguiler", "Duration::WhileMatches({ let q: &dyn Query = g; item.subject.object() }", false),
    ("Eriette, the Beguiler", "<= (subject.object().map_or(", false),
    // A block rule's "same sector" is the attacker's.
    ("Space Beleren", "q.in_sector(asker, blocker, Some(attacker))", false),
    // "Its toughness" for each object the perpetual change is on.
    ("Teyo, Aegis Adept", "for &object in &objects_1 { let kind_1_0", false),
    // An effect on a spell as it resolves is handed the spell.
    ("Feather, the Redeemed", "|g: &mut dyn Game, affected: ObjectId|", false),
    // "All activated and triggered abilities of the exiled card".
    ("Idris, Soul of the TARDIS", "matches!(ability, AbilityDef::Triggered(_))", false),
    // A keyword given by an effect is a word of Keywords.
    ("Thundermane Dragon", "Keywords::of(&[Keyword::Haste])", false),
    // The holes these name are Cantrip's.
    ("Gwenom, Remorseless", "a-lasting-rule-to-look-at-the-top-card-of-a-library", false),
    ("Gwenom, Remorseless", "a-lasting-permission-to-play-from-the-top-of-a-library", false),
    ("Raging River", "a-pile-s-name-kept-on-its-objects-for-a-lasting-rule", false),
    ("Pin Collection", "the abilities of ability stickers", false),
    // A choice that reads what a hole made follows that hole.
    ("Hostile Negotiations", "planned!(\"depends-on-an-instruction-with-a-gap\" => cantrip_mechanics::choose::of_piles(", false),
    ("Close Encounter", "planned_value!(\"depends on an instruction with a gap\"", false),
    ("Apostle's Blessing", "planned_value!(\"depends on an instruction with a gap\"", false),
    // Objects chosen as the permanent entered, read later.
    ("Metamorphic Alteration", "q.state().chosen_object(", false),
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
