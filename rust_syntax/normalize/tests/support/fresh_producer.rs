//! Explicitly reviewed differences between preserved saved cards and today's
//! Source producer. This is test-only snapshot data, not canonicalization or a
//! migration. Every old and new value is asserted; all other fields must match.
use serde_json::Value;

pub fn assert_matches(baseline: &str, saved: &Value, fresh: &Value) {
    let name = saved["faces"][0]["name"].as_str().unwrap_or("");
    assert_named(baseline, name, saved, fresh);
}

pub fn assert_named(baseline: &str, name: &str, saved: &Value, fresh: &Value) {
    let mut expected = saved.clone();
    for data in [include_str!("../fixtures/reference_remaining30_fresh_changes.json"),
        include_str!("../fixtures/target_populations_fresh_changes_20261112.json"),
        include_str!("../fixtures/random_damage_fresh_changes_20261112.json"),
        include_str!("../fixtures/reference_followups_fresh_changes_20261112.json"),
        include_str!("../fixtures/ordered_zones_fresh_changes_20261113.json"),
        include_str!("../fixtures/history_threshold_fresh_changes_20261113.json"),
        include_str!("../fixtures/combat_source_history_fresh_changes_20261113.json")] {
        let reviewed: Value=serde_json::from_str(data).unwrap();
        if let Some(changes) = reviewed[baseline][name].as_array() {
            for change in changes {
                let path=change["path"].as_str().unwrap();
                assert_eq!(expected.pointer(path),Some(&change["before"]),"{baseline}/{name}: original {path}");
                // A later reviewed correction can refine a field within this
                // historical replacement. Check the complete final value below.
                *expected.pointer_mut(path).unwrap()=change["after"].clone();
            }
        }
    }
    assert_eq!(
        &expected, fresh,
        "{baseline}/{name}: unexpected producer change"
    );
}
