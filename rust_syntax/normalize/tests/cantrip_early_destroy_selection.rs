#![cfg(feature = "cantrip")]
use mtgish_ast::cantrip::{planned, pr_form::{render, Characteristics}};
use std::{collections::BTreeMap, path::PathBuf};

#[test]
fn direct_destroy_filters_only_after_its_printed_pool_is_computed() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-destroy-selection.jsonl");
    planned::tracked(&fixtures.join("early_destroy_choice.ronl"), &fixtures.join("early_destroy_choice.oracle.json"), &out).unwrap();
    let rows: BTreeMap<String, serde_json::Value> = std::fs::read_to_string(out).unwrap().lines().map(|line| { let row: serde_json::Value = serde_json::from_str(line).unwrap(); (row["name"].as_str().unwrap().into(), row) }).collect();
    for name in ["Drop of Honey", "Porphyry Nodes", "Imperial Edict", "Frankenstein's Monster", "Wand of Ith"] {
        assert!(rows[name]["faces"][0]["gaps"].as_array().unwrap().is_empty(), "{name}: {}", rows[name]);
    }
    for name in ["Drop of Honey", "Porphyry Nodes"] {
        let raw = rows[name]["faces"][0]["rust"].as_str().unwrap();
        let text = "At the beginning of your upkeep, destroy the creature with the least power.";
        let printed = render(raw, name, text, Characteristics::Oracle).unwrap();
        let minimum = printed.find(".min()").unwrap();
        let exclusion = printed.find("q.can_destroy(").unwrap();
        assert!(!printed.contains("keywords.has(Keyword::Indestructible)"), "the core owns destruction permission: {printed}");
        assert!(minimum < exclusion, "{printed}");
    }
    let monster = rows["Frankenstein's Monster"]["faces"][0]["rust"].as_str().unwrap();
    assert!(monster.contains("AsManyAsCan") && monster.contains("ZoneKind::Exile") && !monster.contains("payment_count_"), "{monster}");
    let wand = rows["Wand of Ith"]["faces"][0]["rust"].as_str().unwrap();
    assert!(wand.contains("!(q.chars(id).is(CardType::Land))"), "{wand}");
    let independent = rows["Imperial Edict"]["faces"][0]["rust"].as_str().unwrap();
    assert!(!independent.contains("Indestructible"), "{independent}");
}
