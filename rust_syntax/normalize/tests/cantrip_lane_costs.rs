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
    // A cost only in that "if you do" reads it, paid as it resolves.
    ("Moonring Mirror", "g.exile_face_down(&cards_"),
    ("Lat-Nam's Legacy", "LibraryPlace::ShuffledIn"),
    // "You may sacrifice one or more creatures. When you do, copy this spell for each."
    ("Plumb the Forbidden", "VariableCost::SacrificeOneOrMore"),
    // Who a payment or a choice ended up with, and who was asked to pay.
    ("Sin Prodder", "who_"),
    ("Seizures", "Some(payer_"),
    // The player who cast the spell that gave this ability.
    ("Hellish Rebuke", "granted_by(q, asker)"),
    // "Any time they could cast a sorcery", asked before the spell is on the stack.
    ("Teferi, Time Raveler", "cast_rules: &["),
    // Who a resolving effect may not make discard or sacrifice.
    ("Sigarda, Host of Herons", "cause_rules: &["),
    // "An opponent does this": the doer is "you" in what they do.
    ("Ingenious Mastery", "&Asker::new(asker.source, doer_"),
    // What a copy keeps of the object that copied.
    ("Vesuvan Doppelganger", "own.colors.unwrap_or("),
    ("Hulking Metamorph", "own.power.unwrap_or(0)"),
    // These hold, as what they want the engine has no way to say.
    ("Ominous Lockbox", "a value chosen in secret"),
    ("Doomsday", "cards found in two zones put on top of a library"),
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

/// As `CARDS`, of forms decided by the card's printed line.
const PRINTED: &[(&str, &str)] = &[
    // A spell leaves the stack without being countered.
    ("Spell Queller", "g.remove_spell(spell, CounterTo::Exile)"),
    // One card is asked of, and more are picked from to the last (CR 101.4c).
    ("Spell Queller", "let among_"),
    // "If it dies this way": destroyed and sent elsewhere it didn't die.
    ("Saw in Half", "to.is(ZoneKind::Graveyard)"),
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

#[test]
fn printed_lines_say_what_their_cards_say() {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let (corpus, oracle) = (data.join("mtgish.lines.ron"), data.join("oracle.json"));
    if !corpus.exists() || !oracle.exists() {
        return;
    }
    let names: Vec<String> = PRINTED.iter().map(|(name, _)| name.to_string()).collect();
    let modules: BTreeMap<String, String> = cantrip::modules_printed(&corpus, &oracle, &names)
        .unwrap()
        .into_iter()
        .collect();
    for (name, says) in PRINTED {
        let module = modules
            .get(*name)
            .unwrap_or_else(|| panic!("no module for {name}"));
        assert!(
            module.contains(says),
            "{name} should have {says}:\n{module}"
        );
    }
}

/// A cast trigger an optional cost adds goes in the card's one ability list (Plumb the Forbidden), and what each
/// player chose as a permanent entered is a local of that ability, which no later ability can read (Null Chamber).
#[test]
fn one_ability_list_and_no_local_of_another_ability() {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let (corpus, oracle) = (data.join("mtgish.lines.ron"), data.join("oracle.json"));
    if !corpus.exists() || !oracle.exists() {
        return;
    }
    let names = vec!["Plumb the Forbidden".to_string(), "Null Chamber".to_string()];
    let modules: BTreeMap<String, String> = cantrip::modules_printed(&corpus, &oracle, &names)
        .unwrap()
        .into_iter()
        .collect();
    let plumb = &modules["Plumb the Forbidden"];
    assert_eq!(plumb.matches(" abilities: &[").count(), 1, "{plumb}");
    assert!(plumb.contains("CAST_THIS_SPELL"), "{plumb}");
    let chamber = &modules["Null Chamber"];
    let enters = chamber.find("fn ability_1").unwrap_or(chamber.len());
    assert!(!chamber[enters..].contains("each_chosen"), "{chamber}");
    assert!(chamber.contains("read-by-another-ability"), "{chamber}");
}
