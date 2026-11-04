#![cfg(feature = "cantrip")]
//! A next-turn deadline must not silently become the next occurrence of upkeep.
use mtgish_ast::cantrip::planned;
use std::{collections::BTreeMap, path::PathBuf};

fn rows(features: &[&str], tag: &str) -> BTreeMap<String, serde_json::Value> {
    fixture_rows(features, tag, "next_turn_delayed")
}

fn fixture_rows(features: &[&str], tag: &str, fixture: &str) -> BTreeMap<String, serde_json::Value> {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let output = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("next-turn-delayed-{tag}.jsonl"));
    planned::with_features(features, || planned::tracked(&fixtures.join(format!("{fixture}.ronl")),
        &fixtures.join(format!("{fixture}.oracle.json")), &output)).unwrap();
    std::fs::read_to_string(output).unwrap().lines().map(|line| {
        let row: serde_json::Value = serde_json::from_str(line).unwrap();
        (row["name"].as_str().unwrap().to_string(), row)
    }).collect()
}

#[test]
fn next_turn_steps_carry_objects_and_select_the_printed_players_turn() {
    let old = fixture_rows(&[], "carried-old", "next_turn_carried");
    let new = fixture_rows(&["next-turn-delayed"], "carried-new", "next_turn_carried");
    assert_eq!(new.len(), 4);
    for (name, row) in &new {
        assert!(!old[name]["faces"][0]["gaps"].as_array().unwrap().is_empty(), "{name}");
        assert!(row["faces"][0]["gaps"].as_array().unwrap().is_empty(), "{name}: {row}");
        let module = row["file"].as_str().unwrap();
        let dense: String = module.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(dense.contains("g.delayed_trigger_about(DelayedWhen::StepOfNextTurn{"), "{name}: {module}");
        let step = match name.as_str() {
            "Meandering Towershell" => "DeclareAttackers",
            _ => "End",
        };
        assert!(dense.contains(&format!("step:Step::{step},")), "{name}: {module}");
        let player = if name == "Suppress" { "of:Some(named_0)" } else { "of:Some(item.controller)" };
        assert!(dense.contains(player), "{name}: {module}");
        if name == "Suppress" {
            assert!(dense.contains("letSome(named_0)=t_0.get(0).copied().and_then(|t:Target|t.player())"), "{module}");
        }
        syn::parse_file(module).unwrap();
    }
}

#[test]
fn next_turn_upkeeps_use_the_native_deadline_and_older_targets_report_the_real_gap() {
    let old = rows(&["regeneration-bans"], "old");
    let new = rows(&["regeneration-bans", "next-turn-delayed"], "new");
    assert_eq!(new.len(), 24);
    for (name, row) in &new {
        if name == "Hazezon Tamar" { continue; }
        let gaps = old[name]["faces"][0]["gaps"].as_array().unwrap();
        assert!(gaps.iter().any(|g| g["feature"] == "delayed trigger starting NextTurn"), "{name}: {gaps:?}");
        assert!(row["faces"][0]["gaps"].as_array().unwrap().is_empty(), "{name}: {row}");
        let module = row["file"].as_str().unwrap();
        let dense: String = module.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(dense.contains("DelayedWhen::StepOfNextTurn{step:Step::Upkeep,of:None"), "{name}: {module}");
        assert!(!dense.contains("DelayedWhen::Step{step:Step::Upkeep"), "{name}: {module}");
        syn::parse_file(module).unwrap();
    }
    let control = new["Hazezon Tamar"]["file"].as_str().unwrap();
    assert_eq!(old["Hazezon Tamar"]["file"], new["Hazezon Tamar"]["file"]);
    let dense: String = control.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(dense.contains("DelayedWhen::Step{step:Step::Upkeep"));
    assert!(!dense.contains("StepOfNextTurn"));
}
