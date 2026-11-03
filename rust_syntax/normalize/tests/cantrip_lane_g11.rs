#![cfg(feature = "cantrip")]
//! Lane g11: forms lane g8 left for the generator, on the cards that waited on
//! them, and the cards whose data or Cantrip holds them, on a form naming that. It
//! reads the corpus and the printed words for these cards alone, and has nothing to
//! say without them.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

/// The card, what its module has to say, and whether it is whole.
const CARDS: &[(&str, &str, bool)] = &[
    // "At the beginning of your upkeep, you may skip that turn" kept as a rule
    // the turn's beginning asks (CR 614.1b, 500.11).
    ("Time Vault", "TurnBeginRule {", true),
    // The earlier attacks of a turn, read from their own records.
    ("Desert Were-Worm", "Attacked { attacker, known }.chars(q)", true),
    // "Equip abilities you activate that target [this creature] cost {N} less".
    ("Ghostfire Blade", ".cost_change(", true),
    // "Produces mana of a color instead", granted for as long as it lasts.
    ("Quarum Trench Gnomes", "ReplacementKind::ManaProduction", true),
    // "[+2], then": loyalty put on or removed from the source as it resolves.
    ("Comet, Stellar Pup", "g.add_counters(asker.source, CounterKind::Loyalty", false),
    // "An ability that targets only it".
    ("Agrus Kos, Eternal Soldier", "q.state().target(i, k) == Target::Object(it)", false),
    // Exiled until a target enchantment leaves (CR 610.3).
    ("Calix, Destiny's Hand", "DelayedWhen::UntilEvent, holder", true),
    // A cost paid whole at resolution, then the pile on top at random.
    ("Triumph of Saint Katherine", "g.exile_face_down(&objects_0, None)", true),
    ("Triumph of Saint Katherine", "LibraryPlace::Top", true),
    // Two abilities of three chosen as it enters: one set of them.
    ("Greymond, Avacyn's Stalwart", "\"first strike and vigilance\"", true),
    ("Greymond, Avacyn's Stalwart", "Keywords::of(&[Keyword::FirstStrike, Keyword::Lifelink])", true),
    // The one player a spell targets.
    ("Zevlor, Elturel Exile", "let Target::Player(first) = *chosen.next()?", false),
    // A static's subjects of the top card of a library, read as they are.
    ("Fblthp, Lost on the Range", ".take((1i64).max(0) as usize)", false),
    // Cantrip holds these.
    ("Comet Storm", "a-spell-s-target-count-of-times-kicked-beside-an-x-or-added-to", false),
    ("Strength of the Tajuru", "a-spell-s-target-count-of-times-kicked-beside-an-x-or-added-to", false),
    ("Primitive Justice", "a-spell-s-target-count-of-an-additional-cost-paid-any-number-of-times", false),
    ("Fishing Pole", "engine: a cost that taps the permanent that granted the ability", false),
    // The data says something other than the Oracle text.
    (
        "Turtles Forever",
        "the-rest-of-a-search-outside-the-game-shuffled-into-the-library-read-as-a-shuffle-in-the-data",
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
