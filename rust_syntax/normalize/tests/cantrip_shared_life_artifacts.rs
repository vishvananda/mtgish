#![cfg(feature = "cantrip")]
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

#[test]
fn all_five_artifacts_use_the_same_upstream_color_and_payment_recipes() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let output = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shared-life-artifacts.jsonl");
    cantrip::planned::tracked(&fixtures.join("shared_life_artifacts.ronl"),
                             &fixtures.join("shared_life_artifacts.oracle.json"), &output).unwrap();
    let rows: BTreeMap<String, serde_json::Value> = std::fs::read_to_string(output).unwrap().lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap())
        .map(|r| (r["faces"][0]["name"].as_str().unwrap().to_owned(), r)).collect();
    for (name, color) in [("Ivory Cup", "White"), ("Crystal Rod", "Blue"), ("Throne of Bone", "Black"),
                          ("Iron Star", "Red"), ("Wooden Sphere", "Green")] {
        let row = &rows[name];
        let file = row["file"].as_str().unwrap_or_else(|| panic!("{name}: {row}"));
        assert!(file.contains("triggers::a_player_casts_a_spell_of_color"), "{name}: {file}");
        assert!(file.contains(&format!("Color::{color}")), "{name}: {file}");
        assert!(file.contains("effects::may_pay_to_gain_life"), "{name}: {file}");
        assert!(!file.contains("fn when_") && !file.contains("PAY_QUESTION"), "{name}: {file}");
    }
    // Farmstead shares the payment but has a different cost and trigger.
    assert!(rows["Farmstead"]["file"].as_str().unwrap().contains("effects::may_pay_to_gain_life"));
    // A free optional life gain must not acquire an optional mana payment.
    let firewalker = rows["Kor Firewalker"]["file"].as_str().unwrap();
    assert!(firewalker.contains("triggers::a_player_casts_a_spell_of_color"));
    assert!(!firewalker.contains("effects::may_pay_to_gain_life"));
}
