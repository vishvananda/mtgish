//! Explicitly reviewed differences between preserved saved cards and today's
//! Source producer. This is test-only snapshot data, not canonicalization or a
//! migration. Every old and new value is asserted; all other fields must match.
use serde_json::Value;

pub fn assert_matches(baseline: &str, saved: &Value, fresh: &Value) {
    let name = saved["faces"][0]["name"].as_str().unwrap_or("");
    assert_named(baseline, name, saved, fresh);
}

pub fn assert_named(baseline: &str, name: &str, saved: &Value, fresh: &Value) {
    let reviewed: Value = serde_json::from_str(include_str!(
        "../fixtures/reference_remaining30_fresh_changes.json"
    ))
    .unwrap();
    let mut expected = saved.clone();
    if let Some(changes) = reviewed[baseline][name].as_array() {
        for change in changes {
            let path = change["path"].as_str().unwrap();
            assert_eq!(
                expected.pointer(path),
                Some(&change["before"]),
                "{baseline}/{name}: original {path}"
            );
            assert_eq!(
                fresh.pointer(path),
                Some(&change["after"]),
                "{baseline}/{name}: current {path}"
            );
            *expected.pointer_mut(path).unwrap() = change["after"].clone();
        }
    }
    assert_eq!(
        &expected, fresh,
        "{baseline}/{name}: unexpected producer change"
    );
}
