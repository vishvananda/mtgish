#![cfg(feature = "cantrip")]
//! Lane g4's forms: exiled-with links, play permissions, delayed abilities and
//! face-down exile the generator now writes, and the cards it holds on a form that
//! names what Cantrip or the data lacks. It reads the corpus and the printed words
//! for these cards alone, and has nothing to say without them.
use mtgish_ast::cantrip;
#[path = "support/generated_locals.rs"]
mod generated_locals;
use std::{collections::BTreeMap, path::PathBuf};

/// The card, what its module has to say, and whether it is whole.
const CARDS: &[(&str, &str, bool)] = &[
    // "Draw a card for each Aura you controlled that was attached to it": the
    // Auras unattached as it left (CR 603.10a).
    ("Hateful Eidolon", "Event::Unattached { attachment, from } if from == host", true),
    // "They exiled with this artifact this turn": linked to the replacement's text.
    ("Uba Mask", "q.exiled_with(q.linked_to(asker)).contains(&card)", true),
    ("Uba Mask", "let by = g.text_linked(asker.source); g.exile_with(&objects_0, by, false)", true),
    // "For as long as it remains exiled": until it moves (CR 400.7).
    ("Kheru Spellsnatcher", "expiry: Expiry::Never, instead: Some(ManaCost::ZERO)", true),
    // The landwalks of the sacrificed land's types.
    ("Excavator", "filter_map(Keyword::landwalk)", true),
    ("Excavator", "q.cost_objects(item)", true),
    // The exiled card is linked to the ability the token was given (CR 607.5).
    ("Timothar, Baron of Bats", "acquired: Acquired { by: AcquiredBy::Grant, stamp, part: 0 }", true),
    ("Timothar, Baron of Bats", "q.exiled_with(q.linked(item))", true),
    // Exiled face down with a hatching counter, and its looker may go on looking.
    ("The Dragon-Kami Reborn", "g.exile_face_down(&objects_4, None)", true),
    ("The Dragon-Kami Reborn", "g.add_counters(o, CounterKind::Hatching", true),
    ("The Dragon-Kami Reborn", "g.look_at(player_2, &moved_4)", true),
    // "When you play a card exiled with this creature".
    ("Voltaic Visionary", "Event::ZoneChange { old, new, from, to, reason: ZoneReason::PlayLand", true),
    ("Voltaic Visionary", "l.is_by_in(q.state(), q.linked_to(asker))", true),
    // The delayed ability's subject is the opponent; the card is what its source became.
    ("Endless Whispers", "Target::Player(player)); }", true),
    ("Endless Whispers", "q.state().obj(item.object).became.into_iter()", true),
    // Cantrip lacks these.
    ("Dimensional Breach", "a-delayed-ability-at-each-step-for-as-long-as-cards-remain-exiled", false),
    ("Ertai's Meddling", "a-delayed-ability-at-each-step-for-as-long-as-cards-remain-exiled", false),
    ("Bösium Strip", "a-lasting-permission-to-cast-from-the-top-of-a-graveyard", false),
    ("Shared Fate", "the-player-who-exiled-a-card-another-player-owns", false),
    ("Fear of Abduction", "a card exiled to pay a permanent spell's cost, read as exiled with the permanent", false),
    ("Redemptor Dreadnought", "a card exiled to pay a permanent spell's cost, read as exiled with the permanent", false),
    // The data says something other than the Oracle text.
    ("Grave Sifter", "every graveyard where the card says their graveyard, in the data", false),
    ("Expedited Inheritance", "a-may-the-card-prints-read-as-a-must-in-the-data", false),
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
        assert!(generated_locals::contains(module,says), "{name} should have {says}:\n{module}");
        if *whole {
            assert!(!module.contains("planned"), "{name} should be whole:\n{module}");
        }
    }
}
