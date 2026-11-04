#![cfg(feature = "cantrip")]
//! Choose up to X first. The native placement ceiling applies after replacements.
use std::{collections::BTreeMap, path::PathBuf};

#[test]
fn clockworks_choose_how_many_counters_to_put_including_zero() {
    mtgish_ast::cantrip::planned::with_features(&["wide-effect-quantities", "counter-placement-ceiling", "pt-counters"], || {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let output = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("capped-counter-choices.jsonl");
    mtgish_ast::cantrip::planned::tracked(&fixture.join("capped_counter_choices.ronl"),
        &fixture.join("capped_counter_choices.oracle.json"), &output).unwrap();
    let rows: BTreeMap<String, serde_json::Value> = std::fs::read_to_string(output).unwrap().lines().map(|line| {
        let row: serde_json::Value = serde_json::from_str(line).unwrap();
        (row["name"].as_str().unwrap().to_string(), row)
    }).collect();
    assert_eq!(rows.len(), 5);
    for name in ["Clockwork Avian", "Clockwork Beast", "Clockwork Steed", "Clockwork Swarm"] {
        let row = &rows[name];
        assert!(row["faces"][0]["gaps"].as_array().unwrap().is_empty(), "{name}: {row}");
        let module = row["file"].as_str().unwrap();
        let dense: String = module.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(dense.contains("choose::number("), "{name}: {module}");
        assert!(module.contains("Put counters"), "{name}: {module}");
        assert!(!dense.contains(".min(room)"), "{name}: {module}");
        assert!(dense.contains(".filter(|&object|is_live(g,object))"), "{name}: {module}");
        assert!(dense.contains("g.add_counters_with_limit(object,CounterKind::Plus1Plus0,chosen,"), "{name}: {module}");
        syn::parse_file(module).unwrap();
    }
    assert!(!rows["Clockwork Gnomes"]["file"].as_str().unwrap().contains("Put counters"));
    });
}

#[test]
fn wide_choices_are_not_limited_by_the_counter_ceiling_or_a_byte() {
    mtgish_ast::cantrip::planned::with_features(&["wide-effect-quantities", "counter-placement-ceiling", "pt-counters"], || {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let original = std::fs::read_to_string(fixture.join("capped_counter_choices.ronl")).unwrap();
    let original = original.lines().find(|line| line.contains("Name: \"Clockwork Beast\"")).unwrap();
    let oracle: Vec<serde_json::Value> = serde_json::from_str(&std::fs::read_to_string(fixture.join("capped_counter_choices.oracle.json")).unwrap()).unwrap();
    let oracle = oracle.into_iter().find(|row| row["name"] == "Clockwork Beast").unwrap();
    for (tag, amount) in [("small", 2), ("wide", 512), ("above-i16", 65_535)] {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("capped-choice-{tag}"));
        std::fs::create_dir_all(&dir).unwrap();
        let source = original.replace("UptoNumberCountersOfTypeOnPermanent(ValueX,", &format!("UptoNumberCountersOfTypeOnPermanent(Integer({amount}),"))
            .replace("ThisPermanent, Integer(7))", "ThisPermanent, Integer(512))");
        assert_ne!(source, original);
        let mut oracle = oracle.clone();
        let text = oracle["cards"][0]["text"].as_str().unwrap().replace("up to X", &format!("up to {amount}"))
            .replace("greater than seven", "greater than 512");
        oracle["cards"][0]["text"] = text.into();
        std::fs::write(dir.join("source.ronl"), source + "\n").unwrap();
        std::fs::write(dir.join("oracle.json"), serde_json::to_vec(&vec![oracle]).unwrap()).unwrap();
        mtgish_ast::cantrip::planned::tracked(&dir.join("source.ronl"), &dir.join("oracle.json"), &dir.join("rows.jsonl")).unwrap();
        let row: serde_json::Value = serde_json::from_str(std::fs::read_to_string(dir.join("rows.jsonl")).unwrap().trim()).unwrap();
        let gaps = row["faces"][0]["gaps"].as_array().unwrap();
        assert!(gaps.is_empty(), "{row}");
        let generated = row["file"].as_str().unwrap();
        assert!(generated.contains("choose::number("), "{generated}");
        assert!(generated.contains("add_counters_with_limit("), "{generated}");
        assert!(!generated.contains(".min(room)"), "{generated}");
    }
    });
}

#[test]
fn an_old_engine_without_the_post_replacement_authority_keeps_a_precise_gap() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let output = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("capped-counter-missing-api.jsonl");
    mtgish_ast::cantrip::planned::with_features(&[], || {
        mtgish_ast::cantrip::planned::tracked(&fixture.join("capped_counter_choices.ronl"),
            &fixture.join("capped_counter_choices.oracle.json"), &output).unwrap();
        for row in std::fs::read_to_string(output).unwrap().lines() {
            let row: serde_json::Value = serde_json::from_str(row).unwrap();
            if row["name"] == "Clockwork Gnomes" { continue; }
            let gaps = row["faces"][0]["gaps"].as_array().unwrap();
            assert!(gaps.iter().any(|gap| gap["feature"] == "post-replacement counter placement ceiling"), "{row}");
            assert!(row["file"].as_str().is_none_or(|file| !file.contains(".min(room)")));
        }
    });
}
