#![cfg(feature = "cantrip")]
//! Combat capabilities stay held for older trees and use the common engine/keyword recipes when present.
use mtgish_ast::cantrip::planned;
use std::{collections::BTreeMap, path::PathBuf};

fn rows(features: &[&str], tag: &str) -> BTreeMap<String, serde_json::Value> {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let output = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("combat-transitions-{tag}.jsonl"));
    planned::with_features(features, || planned::tracked(&fixtures.join("combat_transitions.ronl"), &fixtures.join("combat_transitions.oracle.json"), &output)).unwrap();
    std::fs::read_to_string(output).unwrap().lines().map(|line| {
        let row: serde_json::Value = serde_json::from_str(line).unwrap();
        (row["faces"][0]["name"].as_str().unwrap().to_string(), row)
    }).collect()
}

#[test]
fn rampage_is_shared_and_only_enabled_for_trees_with_the_keyword() {
    let old = rows(&[], "old-rampage");
    let new = rows(&["rampage"], "new-rampage");
    for (name, n) in [("Aerathi Berserker", 3), ("Chromium", 2), ("Craw Giant", 2), ("Frost Giant", 2),
                      ("Hunding Gjornersen", 1), ("Marhault Elsdragon", 1), ("Wolverine Pack", 2)] {
        let held = old[name]["faces"][0]["rust"].as_str().unwrap();
        let enabled = new[name]["file"].as_str().unwrap();
        assert!(held.contains("engine-no-rampage"), "{name}: {held}");
        assert!(!enabled.contains("engine-no-rampage"), "{name}: {enabled}");
        assert!(enabled.contains(&format!("rampage::trigger::<{n}>")), "{name}: {enabled}");
        assert!(new[name]["faces"][0]["gaps"].as_array().unwrap().is_empty(), "{name}: {}", new[name]);
    }
    let rapid = new["Rapid Fire"]["faces"][0]["rust"].as_str().unwrap();
    assert!(rapid.contains("rampage::trigger::<2>"), "{rapid}");
    assert!(!rapid.contains("engine-no-rampage"), "{rapid}");
    assert!(new["Rapid Fire"]["faces"][0]["gaps"].as_array().unwrap().iter().any(|g| g["feature"] == "object filter HasKeywordFamily"));
}

#[test]
fn combat_operations_use_the_engine_and_the_printed_unblocking_recipe() {
    let old = rows(&[], "old-removal");
    let new = rows(&["combat-removal", "blocked-status"], "new-removal");
    for name in ["Disharmony", "Imprison", "Mijae Djinn", "Ydwen Efreet", "False Orders"] {
        let held = old[name]["faces"][0]["rust"].as_str().unwrap();
        let enabled = new[name]["faces"][0]["rust"].as_str().unwrap();
        assert!(held.contains("engine-no-effect-removes-from-combat"), "{name}: {held}");
        assert!(!enabled.contains("engine-no-effect-removes-from-combat"), "{name}: {enabled}");
        let call = if matches!(name, "Imprison" | "False Orders" | "Ydwen Efreet") { "combat::remove_from_combat_and_unblock(g, object)" }
                   else { "g.remove_from_combat(object)" };
        assert!(enabled.contains(call), "{name}: {enabled}");
    }
    for name in ["Dazzling Beauty", "Curtain of Light"] {
        let enabled = new[name]["faces"][0]["rust"].as_str().unwrap();
        assert!(enabled.contains("g.become_blocked(object)"), "{name}: {enabled}");
        assert!(!enabled.contains("engine-no-effect-makes-a-creature-blocked"), "{name}: {enabled}");
    }
}

#[test]
fn becoming_blocked_uses_a_transition_but_blocked_by_a_creature_keeps_each_relationship() {
    let old = rows(&[], "old-transition");
    let new = rows(&["blocked-status"], "new-transition");
    let previous = old["Karn, Silver Golem"]["faces"][0]["rust"].as_str().unwrap();
    let current = new["Karn, Silver Golem"]["faces"][0]["rust"].as_str().unwrap();
    assert!(!previous.contains("Event::BecameBlocked"), "{previous}");
    assert!(current.contains("Event::BecameBlocked { attacker }"), "{current}");
    assert!(current.contains("EventMask::BECAME_BLOCKED"), "{current}");
    let pair = new["Cockatrice"]["faces"][0]["rust"].as_str().unwrap();
    assert!(pair.contains("Event::Blocks { blocker, attacker, .. }"), "{pair}");
    assert!(!pair.contains("Event::BecameBlocked"), "{pair}");
}
