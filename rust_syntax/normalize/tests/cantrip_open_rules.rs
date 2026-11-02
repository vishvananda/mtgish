#![cfg(feature = "cantrip")]
//! Rules a resolving spell or ability gives an open set of objects (CR 611.2c),
//! and "doesn't untap during its controller's next untap step", written for a
//! Cantrip with `lasting_rules` (`MTGISH_CANTRIP_FEATURES=lasting-rules`). The
//! switch is read once a process, so this file's one test turns it on before
//! it writes anything; the other tests write for the pinned Cantrip. It reads
//! the corpus and the printed words for these cards alone, and has nothing to
//! say without them.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

/// The card, what its module has to say, and whether it is whole.
const CARDS: &[(&str, &str, bool)] = &[
    // "Creatures without flying can't block this turn": a creature that
    // arrives later is in the set too.
    ("Falter", "rule: LastingRuleKind::CantBlock", true),
    ("Falter", "g.add_lasting_rule(item, 0, about_0, until_0);", true),
    ("Goblin Diplomats", "rule: LastingRuleKind::MustAttack", true),
    ("Goblin Diplomats", "text: \"Each creature attacks this turn if able.\"", true),
    // The target opponent is what the rule reads of the resolution, and
    // "their next untap step" is that player's.
    ("Exhaustion", "rule: LastingRuleKind::DoesntUntap", true),
    ("Exhaustion", "about.player() == Some(q.controller(id))", true),
    ("Exhaustion", "Duration::DuringNextUntapStepOf(", true),
    // A rule's words are its sentence of the card: the kicked spell's, the
    // loyalty ability's past its cost, the mode's.
    (
        "Orim's Chant",
        "text: \"If this spell was kicked, creatures can't attack this turn.\"",
        true,
    ),
    (
        "Jace, Arcane Strategist",
        "text: \"Creatures you control can't be blocked this turn.\"",
        true,
    ),
    (
        "Reverse the Polarity",
        "text: \"Creatures can't be blocked this turn.\"",
        true,
    ),
    // "Other creatures": the target creature is the one left out.
    ("Intimidation Bolt", "!(about.object() == Some(id))", true),
    // Attacking creatures are the ones attacking as it resolves (Tangle's
    // ruling), each waiting on its own controller's untap step.
    ("Tangle", "EffectKind::DoesntUntap", true),
    ("Tangle", "Duration::DuringItsControllersNextUntapStep", true),
    // Under a change of control the permanent waits on its new controller's
    // untap step, which exerting wouldn't (Frost Titan's ruling).
    ("Frost Titan", "EffectKind::DoesntUntap", true),
    ("Frost Titan", "Duration::DuringItsControllersNextUntapStep", true),
    // "For as long as you control Icefall Regent": the same rule, lasting.
    ("Icefall Regent", "EffectKind::DoesntUntap", true),
    // A creature the target opponent chose, and that player: two things the
    // ability chose, which a rule's `about` can't both hold.
    ("Mark for Death", "a-lasting-rule-reading-two-things-its-ability-chose", false),
    ("Goblin War Cry", "a-lasting-rule-reading-what-its-ability-chose", false),
];

#[test]
fn modules_say_what_their_cards_say() {
    std::env::set_var("MTGISH_CANTRIP_FEATURES", "lasting-rules");
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
