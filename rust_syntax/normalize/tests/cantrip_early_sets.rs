#![cfg(feature = "cantrip")]
//! Regression probes for old set cards whose native interfaces already exist.
use mtgish_ast::{cantrip, corpus, ir::*, mtg_types::OracleCard, unified};
use std::{collections::BTreeMap, path::PathBuf, sync::OnceLock};

fn rows() -> &'static BTreeMap<String, serde_json::Value> {
    static ROWS: OnceLock<BTreeMap<String, serde_json::Value>> = OnceLock::new();
    ROWS.get_or_init(|| {
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-set-generator-gaps.jsonl");
        cantrip::planned::tracked(&fixtures.join("early_set_generator_gaps.ronl"),
            &fixtures.join("early_set_generator_gaps.oracle.json"), &out).unwrap();
        std::fs::read_to_string(out).unwrap().lines().map(|line| {
            let row: serde_json::Value = serde_json::from_str(line).unwrap();
            (row["name"].as_str().unwrap().to_string(), row)
        }).collect()
    })
}
fn ready(name: &str) -> String {
    let row = &rows()[name];
    for face in row["faces"].as_array().unwrap() {
        assert!(face["gaps"].as_array().unwrap().is_empty(), "{name}: {}", face["gaps"]);
    }
    let code = row["faces"][0]["rust"].as_str().unwrap();
    syn::parse_file(code).unwrap();
    assert!(!code.contains("planned!(") && !code.contains("planned_value!(") && !code.contains("planned_item!("), "{name}: {code}");
    code.to_string()
}
#[test]
fn native_land_type_and_trigger_interfaces_publish_complete_cards() {
    for name in ["Gaea's Liege", "Orcish Farmer", "Thelonite Monk", "Merieke Ri Berit",
        "Krovikan Vampire", "Seraph", "Glyph of Doom", "Phantasmal Mount", "Venomous Breath", "War Barge"] {
        ready(name);
    }
    assert!(ready("Gaea's Liege").contains("EffectKind::SetLandType"));
    assert!(ready("Merieke Ri Berit").contains("Event::Untapped"));
    assert!(ready("Krovikan Vampire").contains("from: player"));
    assert!(ready("Glyph of Doom").contains("delayed_trigger_about"));
}
#[test]
fn clockworks_offer_full_x_then_convert_at_the_counter_boundary() {
    for name in ["Clockwork Beast", "Clockwork Avian"] {
        let code = ready(name);
        assert!(code.contains("Put counters"), "{code}");
        // The choice bound is unsigned X. Its result alone crosses the signed
        // native counter boundary; available room is not the prompt's bound.
        assert!(code.contains("i64::from(picked_"), "{code}");
        assert!(code.contains("min(i32::MAX.into()) as i32"), "{code}");
        assert!(code.contains("add_counters_with_limit"), "{code}");
        assert!(!code.contains(".min(room)"), "{code}");
    }
}
#[test]
fn last_removed_uses_the_events_total_and_shared_counter_mask() {
    // The printed card additionally names the remover. Test an unqualified
    // removal explicitly; never erase the named-remover requirement or fake it.
    let source = include_str!("fixtures/early_set_generator_gaps.ronl").lines()
        .map(|line| ron::from_str::<OracleCard>(line).unwrap())
        .find(|source| corpus::Identity::of(source).unwrap().names.iter().any(|n| n == "Divine Intervention")).unwrap();
    let mut lowered = unified::lower(&source);
    let Ability::Triggered { event: EventSpec::Counters { by, .. }, .. } = &mut lowered.card.faces[0].abilities[2] else { panic!() };
    *by = None;
    let whole = cantrip::whole(&source, &lowered.card).unwrap();
    let code = &whole.faces[0].rust;
    assert!(code.contains("Event::CountersRemoved { object, kind, amount, total }"), "{code}");
    assert!(code.contains("*total == 0"), "{code}");
    assert!(!code.contains("COUNTERS_REMOVED"), "{code}");
    assert!(!code.contains("no event for a counter being removed"), "{code}");
}

#[test]
fn old_set_effects_use_existing_native_payments_and_rules() {
    for name in ["Anti-Magic Aura", "Ashen Ghoul", "Chaos Moon", "Dreams of the Dead",
        "Forgotten Lore", "Frankenstein's Monster", "Game of Chaos", "Giant Slug",
        "Illusionary Terrain", "Infernal Darkness", "Island Sanctuary", "Magus of the Unseen",
        "Mirror Universe", "Nameless Race", "Necropolis", "Ray of Command", "Soul Exchange",
        "Tetravus", "Vision Charm"] {
        ready(name);
    }
    assert!(ready("Tetravus").contains("g.may_pay_x"));
    assert!(ready("Tetravus").contains("counters_paid_1.unwrap_or(0)"));
    assert!(ready("Necropolis").contains("CostPart::GraveyardExile"));
    assert!(ready("Frankenstein's Monster").contains("map(Label::Counter)"));
    assert!(ready("Island Sanctuary").contains("LastingRuleKind::CantAttackYou"));
}

#[test]
fn unavailable_native_interfaces_keep_their_explicit_holes() {
    for name in ["Gaea's Liege", "Merieke Ri Berit", "War Barge", "Dreams of the Dead",
        "Mirror Universe", "Infernal Darkness", "Anti-Magic Aura", "Tetravus", "Necropolis", "Soul Exchange"] {
        let source = include_str!("fixtures/early_set_generator_gaps.ronl").lines()
            .map(|line| ron::from_str::<OracleCard>(line).unwrap())
            .find(|source| corpus::Identity::of(source).unwrap().names.iter().any(|n| n == name)).unwrap();
        let lowered = unified::lower(&source);
        cantrip::planned::with_features(&[], || {
            let generated = cantrip::whole(&source, &lowered.card).unwrap();
            assert!(generated.faces.iter().any(|face| !face.gaps.is_empty()), "{name} lost its native requirement");
        });
    }
}

#[test]
#[ignore = "execute against MTGISH_CANTRIP_DIR using MTGISH_CANTRIP_TARGET"]
fn early_set_generated_effects_execute_in_cantrip() {
    use std::{fs, process::Command};
    let checkout = PathBuf::from(std::env::var_os("MTGISH_CANTRIP_DIR").expect("native Cantrip checkout"));
    let root = PathBuf::from(std::env::var_os("MTGISH_EARLY_SET_PROBE")
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-set-runtime").into_os_string()));
    fs::create_dir_all(root.join("src")).unwrap();
    let mut manifest = "[package]\nname=\"mtgish-early-set-runtime\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n".to_string();
    for dep in ["cantrip-api", "cantrip-state", "cantrip-mechanics", "cantrip-rules"] {
        manifest.push_str(&format!("{dep}={{path={:?}}}\n", checkout.join("crates").join(dep)));
    }
    fs::write(root.join("Cargo.toml"), manifest).unwrap();
    let mut lib = String::from("#![allow(dead_code,unused_imports,unused_variables,unused_mut)]\n");
    for (name, row) in rows() {
        if row["faces"].as_array().unwrap().iter().any(|f| !f["gaps"].as_array().unwrap().is_empty()) { continue; }
        let stem: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect();
        let stem = stem.split('_').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("_");
        assert_eq!(row["faces"].as_array().unwrap().len(), 1);
        lib.push_str(&format!("mod {stem} {{\n{}\n}}\n", row["faces"][0]["rust"].as_str().unwrap()));
    }
    lib.push_str(include_str!("fixtures/early_set_generator_scenarios.rs"));
    fs::write(root.join("src/lib.rs"), lib).unwrap();
    let result = Command::new("cargo").arg("test").current_dir(&root)
        .env("CARGO_TARGET_DIR", std::env::var_os("MTGISH_CANTRIP_TARGET").expect("separate native target"))
        .env("CANTRIP_VERIFY", "1").env("RUST_MIN_STACK", "33554432").output().unwrap();
    assert!(result.status.success(), "{}\n{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
    println!("{}", String::from_utf8_lossy(&result.stdout));
}

#[test]
fn ability_choice_can_grant_rampage_while_keeping_the_missing_upkeep_duration() {
    let row = &rows()["Gabriel Angelfire"];
    let gaps: Vec<_> = row["faces"][0]["gaps"].as_array().unwrap().iter().map(|g| g["feature"].as_str().unwrap()).collect();
    assert_eq!(gaps, ["duration UntilNext upkeep"]);
    let code = row["faces"][0]["rust"].as_str().unwrap();
    assert!(code.contains("rampage 3"), "{code}");
    assert!(code.contains("EffectKind::AddAbility"), "{code}");
}
