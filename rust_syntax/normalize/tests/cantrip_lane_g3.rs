#![cfg(feature = "cantrip")]
//! Lane g3: grants of other permanents' abilities and of keywords, activation
//! triggers, and what an activation paid, on the cards that waited on them; and the
//! cards that hold on a form naming what Cantrip lacks. It reads the corpus and the
//! printed words for these cards alone, and has nothing to say without them.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

/// The card, what its module has to say, and whether it is whole.
const CARDS: &[(&str, &str, bool)] = &[
    // "Creature cards you own that aren't on the battlefield have flash": a flash rule.
    ("Teferi, Mage of Zhalfir", "flash_rules: &[ability_1_0_flash]", true),
    ("Teferi, Mage of Zhalfir", "q.chars(spell).keywords.has(Keyword::Flash)", true),
    // Unleash given: an optional entering replacement and a can't-block static.
    ("Tesak, Judith's Hellhound", ".optional(&ABILITY_1_0_UNLEASH)", true),
    ("Tesak, Judith's Hellhound", "chars.keywords = chars.keywords.with(Keyword::CantBlock)", true),
    // Freerunning and web-slinging given: granted ways from the hand.
    ("Ezio Auditore da Firenze", "kind: WayKind::Freerunning", true),
    ("Ezio Auditore da Firenze", "assassin_dealt_combat_damage(q, asker)", true),
    ("Peter Parker", "kind: WayKind::WebSlinging", true),
    // Activation triggers read the activated ability's stack item.
    ("Ertha Jo, Frontier Mentor", "i.serial == serial && i.kind == StackKind::Activated", true),
    ("Bill Potts", "chosen.iter().all(|t| *t == chosen[0])", true),
    ("Bill Potts", ".only_if(ability_0_shared_limit)", true),
    ("Dynaheir, Invoker Adept", "Event::ManaSpent { on, cast: false, source, mana, .. } if on == id", true),
    ("Vazi, Keen Negotiator", "source != ObjectId::NONE && q.chars(source).has_subtype(", true),
    ("Satoru Umezawa", "d.text.starts_with(\"Ninjutsu\")", true),
    // What was paid to activate it, after the mana abilities paid while activating.
    ("Ashnod the Uncaring", "let sacrificed = paid.iter().any(", true),
    ("Verrak, Warped Sengir", "let data = [id.0, life];", true),
    // A countered activated ability's source, from its stack item.
    ("Interdict", "s.left_ability(serial)", true),
    ("Ouphe Vandals", "s.left_ability(serial)", true),
    // "Do this only once each turn" of a may that chooses first.
    ("Rinoa, Angel Wing", "if !selected_2.is_empty() { g.note_done(", true),
    // "The first activated ability of an artifact you activate each turn".
    ("Tezzeret, Betrayer of Flesh", "Event::AbilityActivated { source, ability, player: p, .. }", true),
    // The activated abilities other permanents have as layer 6 leaves them.
    ("Drana and Linvala", "let other = q.chars(id); for r in other.abilities()", true),
    ("Drana and Linvala", "spend_any_color: &[ability_3_0_spend]", true),
    ("Scheming Fence", "!d.special_action && (true && !d.is_loyalty_ability())", true),
    ("Experiment Kraj", "if id == source || !q.state().obj(id).zone.is(ZoneKind::Battlefield)", true),
    ("Marvin, Murderous Mimic", "let other = q.chars(id);", true),
    ("Robaran Mercenaries", "let other = q.chars(id);", true),
    ("Manascape Refractor", "let other = q.chars(id);", true),
    // Reconfigure at either price: its two abilities at each.
    ("Razorfield Ripper", "\"Reconfigure\u{2014}Pay {E}{E}{E} (Pay {E}{E}{E}: Unattach this permanent.", true),
    ("Razorfield Ripper", "Cost { energy: 3, ..Cost::NONE }), cantrip_mechanics::attach::attach_to_target", true),
    // What Cantrip has no word for.
    ("Dragon's Fire", "planned_value!(\"engine: no reveal cost\", Cost::NONE => Cost::one_of", false),
    ("Sharkey, Tyrant of the Shire", "mana-spent-as-though-of-any-type", false),
    ("Koh, the Face Stealer", "a-chosen-object-outside-its-body", false),
    ("Bullseye, Death Dealer", "a-choice-between-the-costs-of-an-activated-ability", false),
    ("Ion Storm", "a-choice-between-the-costs-of-an-activated-ability", false),
    ("Rift Elemental", "a-choice-between-the-costs-of-an-activated-ability", false),
    ("Kinzu of the Bleak Coven", "toxic-as-a-copy-exception", false),
    ("The Grim Captain's Locker", "escape-given-by-a-resolving-effect", false),
    ("Rhythm of the Wild", "engine-no-riot-granted", false),
    ("Uncivil Unrest", "engine-no-riot-granted", false),
    ("Kaya, Bane of the Dead", "player-rule-targetable-despite", false),
    ("Kaya, Bane of the Dead", "static-rule-targetable-despite", false),
    ("Professor Hojo", "the-targets-of-abilities-activated-earlier-this-turn", false),
    ("Volatile Stormdrake", "hexproof-from-a-kind-of-ability", false),
    ("Ulalek, Fused Atrocity", "copies-of-each-of-several-abilities-on-the-stack", false),
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
        // A form is named by its id in a planned item, by its words in a planned value.
        let named = module.contains(says) || !whole && module.contains(&says.replace('-', " "));
        assert!(named, "{name} should have {says}:\n{module}");
        if *whole {
            assert!(!module.contains("planned"), "{name} should be whole:\n{module}");
        }
    }
}
