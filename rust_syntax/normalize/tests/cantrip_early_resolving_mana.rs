#![cfg(feature = "cantrip")]
//! A resolving payment uses its known amount and the existing native mana-cost range.
use mtgish_ast::cantrip::planned;
use std::{collections::BTreeMap, path::PathBuf};

#[test]
fn resolving_mana_uses_announced_x_and_does_not_cap_counter_costs_at_255() {
    let f = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let output = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-resolving-mana.jsonl");
    planned::tracked(
        &f.join("early_resolving_mana.ronl"),
        &f.join("early_resolving_mana.oracle.json"),
        &output,
    )
    .unwrap();
    let rows: BTreeMap<String, serde_json::Value> = std::fs::read_to_string(output)
        .unwrap()
        .lines()
        .map(|line| {
            let row: serde_json::Value = serde_json::from_str(line).unwrap();
            (row["name"].as_str().unwrap().to_string(), row)
        })
        .collect();
    for name in ["Musician", "Power Sink", "Condescend"] {
        assert!(
            rows[name]["faces"][0]["gaps"]
                .as_array()
                .unwrap()
                .is_empty(),
            "{name}: {}",
            rows[name]
        );
    }
    let musician = rows["Musician"]["file"].as_str().unwrap();
    assert!(!musician.contains("clamp(0, 255)"), "{musician}");
    assert!(!musician.contains("generic: 255"), "{musician}");
    for name in ["Power Sink", "Condescend"] {
        let source = rows[name]["file"].as_str().unwrap();
        assert!(!source.contains("cost(\"{X}\")"), "{name}: {source}");
        assert!(source.contains("item.x"), "{name}: {source}");
        assert!(source.contains("x: 0"), "{name}: {source}");
    }
}
