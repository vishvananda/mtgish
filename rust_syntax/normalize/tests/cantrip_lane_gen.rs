#![cfg(feature = "cantrip")]
//! Forms Cantrip could already express that the generator now writes, on the cards
//! that waited on them, and the cards whose data says something other than their
//! Oracle text, which hold on a form naming that. It reads the corpus and the printed
//! words for these cards alone, and has nothing to say without them.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

/// The card, what its module has to say, and whether it is whole.
const CARDS: &[(&str, &str, bool)] = &[
    // The object chosen as it entered, linked to the static ability (CR 607.2a).
    (
        "Tyrannical Pitlord",
        "q.state().chosen_object(q.linked_to(asker.source)) == Some(",
        true,
    ),
    // "You may activate each of those abilities only once each turn" (CR 602.5b).
    ("Mairsil, the Pretender", "granted_once_each_turn: Some(granted_once)", true),
    ("Mairsil, the Pretender", "grant_activated_abilities_of(q, &cards, chars)", true),
    // The cards used to craft it are linked to the card that waited in exile.
    ("Locus of Enlightenment", "used_to_craft(q.state(), source)", true),
    // Read ahead granted is the keyword the rules read as the Saga enters.
    ("Barbara Wright", "Keyword::ReadAhead", true),
    // A spell as a source of damage is an object on the stack (CR 400.7).
    ("Aegar, the Freezing Flame", "zone.is(ZoneKind::Stack)", true),
    ("Suffocation", "zone.is(ZoneKind::Stack)", true),
    // "For as long as they control it" and "for as long as that creature is
    // enchanted": the card's own lasting conditions (CR 611.2b).
    ("Vislor Turlough", "Duration::AsLongAs(", true),
    ("Vislor Turlough", "EffectKind::Control(p) => Some((f.timestamp, p))", true),
    ("Rootwater Matriarch", "Duration::AsLongAs(", true),
    ("Rootwater Matriarch", "attached_to == Some(e.affected)", true),
    // "Choose up to X": a triggered ability's count as it goes on the stack.
    ("Bumi, King of Three Trials", ".up_to_that_many()", true),
    ("Bumi, King of Three Trials", ".counted_by(ability_0_count)", true),
    ("The Ruinous Wrecking Crew", ".up_to_that_many()", true),
    // Only a triggered ability has a count of modes.
    ("Doomsday Confluence", "a-count-of-modes-a-spell-or-an-activated-ability-reads", false),
    // "If you control five or more untapped lands, this enters with ...".
    ("Dust Animus", "CounterKind::Lifelink", true),
    // Color pairs of the permanents that are exactly two colors.
    ("Niv-Mizzet, Guildpact", "value.iter().count() == 2 && !seen.contains(&value)", true),
    // "During your last combat", and a random choice among no one is no choice.
    ("Territorial Hellkite", "EventQuery::kind(EventMask::STEP_BEGAN).step(Step::BeginCombat)", true),
    ("Territorial Hellkite", "break 'paid_1;", true),
    // "If it doesn't have [this ability], it gains that ability".
    ("Musician", "g.chars(object).abilities.contains(&Some(a))", true),
    // "Put counters ... If you do": that a counter was put.
    ("Thought Gorger", "let paid_0 = !applied_1.is_empty();", true),
    // "From your hand": the hand of the player the rule is asked of.
    ("Experimental Frenzy", "q.obj(spell).owner == player", true),
    // The data says something other than the Oracle text.
    ("Redeem", "damage-prevented-to-its-targets-read-in-the-data-as-by-them", false),
    ("Karona, False God", "a-chosen-card-type-where-its-body-chose-a-creature-type-in-the-data", false),
    ("Whim of Volrath", "a-text-change-s-duration-given-to-one-of-its-options-in-the-data", false),
    ("Bison Whistle", "an-option-the-card-says-is-otherwise-free-in-the-data", false),
    ("Ravenous Vampire", "a-mandatory-cost-the-card-s-if-you-don-t-is-read-after-in-the-data", false),
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
