#![cfg(feature = "cantrip")]
//! Forms lane g8 writes: history questions, mana costs, prohibitions as the tables
//! Cantrip reads, payments offered whole, and the cards that waited on them. It reads
//! the corpus and the printed words for these cards alone, and has nothing to say
//! without them.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

/// The card, what its module has to say, and whether it is whole.
const CARDS: &[(&str, &str, bool)] = &[
    // Counters a player put, and combat damage an object dealt, from the history.
    ("Kid Loki", "Event::CountersPut", true),
    ("Ruric Thar, Magecrusher", "kind.is_combat()", true),
    // A card's mana cost, compared and read for its symbols (CR 202.1).
    ("Urza's Saga", "!values.has_no_cost()", true),
    ("Namor the Sub-Mariner", "q.mana_cost(id)", true),
    ("Omnath, Locus of All", "q.mana_cost(", false),
    // Prohibitions are the tables the rules read (CR 101.2).
    ("Drannith Magistrate", "cast_rules: &[", true),
    ("Melira, Sylvok Outcast", "player_counter_bans: &[", true),
    ("Solemnity", "player_counter_bans: &[", true),
    ("The Aetherspark", "Keyword::CantBeAttacked", true),
    ("Marooned", "CostPayment::BeamUp(", true),
    ("Shaman's Trance", "ZoneKinds::of(&[ZoneKind::Graveyard])", false),
    // Cards that share a land type: a set part over the land types.
    ("Myriad Landscape", "SubtypeSet::LAND_TYPES", true),
    // The target of an ability: a stack item that targets it (CR 115.1).
    ("Goblin Artisans", "q.state().targets(e).contains(&Target::Object(id))", true),
    ("Wyll's Reversal", "q.state().targets(i).len() as i64", false),
    // "Until it leaves the battlefield" in a trigger about the source (CR 610.3).
    ("Lumbering Battlement", "g.exile_with(&objects_1, by_1, true)", true),
    // Payments as the ability resolves (CR 118.12): offered whole, chosen, or a counter.
    ("Meanders Guide", "Count::UpTo(", true),
    ("Zoraline, Cosmos Caller", ".and(life(2)), &PAY_QUESTION)", true),
    ("Mister Hyde, Monster Within", "g.remove_a_counter_from_among(", true),
    // Excess damage of a fight, read from its own damage events (CR 120.10).
    ("The Last Agni Kai", "recipient_excess", false),
    // "Another activated ability": the asking one is on the source.
    ("Tazri, Stalwart Survivor", ".count() > usize::from(", false),
    // A card type chosen in the same body.
    ("Season of Gathering", "q.chars(id).is(chosen_", false),
    // Mana another player spent to cast it (assist, CR 702.132a).
    ("Yume, Chronicler of Valor", "Event::ManaSpent { player: payer", false),
    // A permission until its holder's own next end step.
    ("Casey & Raph, Hotheads", "Expiry::UntilYourNextEndStep", false),
    ("Timeline Culler", "warp::way_with_cost(", false),
    // Counters removed: the history has no such record, a hole that says so.
    (
        "Cyclopean Tomb",
        "counters-removed-from-an-object-which-the-history-does-not-record",
        false,
    ),
];

#[test]
fn modules_say_what_their_cards_say() {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let (corpus, oracle) = (data.join("mtgish.lines.ron"), data.join("oracle.json"));
    if !corpus.exists() || !oracle.exists() {
        return;
    }
    let mut names: Vec<String> = CARDS.iter().map(|(name, ..)| name.to_string()).collect();
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
}
