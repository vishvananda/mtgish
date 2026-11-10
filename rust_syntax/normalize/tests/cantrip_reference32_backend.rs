#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, corpus, ir::*, lower, mtg_types::OracleCard};

fn card(name: &str) -> Card {
    let source: OracleCard = include_str!("fixtures/reference_history_20261110.ronl")
        .lines()
        .chain(include_str!("fixtures/reference_crew_history_20261110.ronl").lines())
        .map(|s| ron::from_str(s).unwrap())
        .find(|s| corpus::Identity::of(s).unwrap().names.iter().any(|n| n == name))
        .unwrap();
    lower::card(&source).unwrap()
}
fn generated(name: &str) -> String {
    let code = cantrip::module(&card(name)).unwrap_or_else(|e| panic!("{name}: {e:?}"));
    syn::parse_file(&code).unwrap();
    if let Ok(dir) = std::env::var("MTGISH_REFERENCE32_MODULES") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(std::path::Path::new(&dir).join(format!("{}.rs", name.replace(' ', "_").to_lowercase())), &code).unwrap();
    }
    code
}
#[test]
fn draw_cost_keeps_requested_count_separate_from_actual_draws() {
    let code = generated("Horrid Shadowspinner");
    assert!(code.contains("requested_draw_"), "{code}");
    assert!(code.contains("cant_draw(payer_"), "{code}");
    assert!(code.contains("i64::from(draw_count_"), "{code}");
    assert!(!code.contains("history().drawn("), "{code}");
}
#[test]
fn historical_crew_count_deduplicates_incarnations_without_a_live_zone_filter() {
    let code = generated("Luxurious Locomotive");
    assert!(code.contains("Event::Crewed { creature, vehicle }"), "{code}");
    assert!(code.contains("sources.dedup()"), "{code}");
    assert!(!code.contains("history measure DistinctSources"), "{code}");
    assert!(code.contains("\"Crew 1\").once_each_turn()"), "{code}");
}
#[test]
fn draw_receipt_does_not_fall_back_to_another_component() {
    let mut source = card("Horrid Shadowspinner");
    let Ability::Triggered { body, .. } = &mut source.abilities[1] else { panic!() };
    let Instruction::If { then, .. } = &mut body.instructions[1] else { panic!() };
    let Instruction::Effect { operation: Operation::Discard { amount, .. }, .. } = &mut then.instructions[0] else { panic!() };
    *amount = Expr::RequestedDrawCount { payment: PaymentRef::Resolution(ResultId(0)), component: vec![1] };
    assert_eq!(cantrip::module(&source).unwrap_err().feature, "invalid AST");
}

#[test]
#[ignore = "compile and execute fresh generated cards against MTGISH_CANTRIP_DIR"]
fn generated_draw_cost_and_crew_history_execute_in_cantrip() {
    use std::{fs, path::PathBuf, process::Command};
    let checkout = PathBuf::from(std::env::var_os("MTGISH_CANTRIP_DIR").expect("set MTGISH_CANTRIP_DIR"));
    let root = PathBuf::from(std::env::var_os("MTGISH_REFERENCE32_PROBE").expect("set MTGISH_REFERENCE32_PROBE"));
    fs::create_dir_all(root.join("src")).unwrap();
    let mut manifest = "[package]\nname=\"mtgish-reference32-backend\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n".to_string();
    for dep in ["cantrip-api", "cantrip-state", "cantrip-mechanics", "cantrip-rules"] {
        manifest.push_str(&format!("{dep}={{path={:?}}}\n", checkout.join("crates").join(dep)));
    }
    manifest.push_str("[profile.checked]\ninherits=\"release\"\nopt-level=3\ndebug-assertions=true\ncodegen-units=256\nincremental=true\n");
    fs::write(root.join("Cargo.toml"), manifest).unwrap();
    let mut lib = String::new();
    for (module, name) in [("horrid", "Horrid Shadowspinner"), ("locomotive", "Luxurious Locomotive")] {
        lib.push_str(&format!("mod {module} {{\n{}\npub fn callback(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {{ let mut event = item.clone(); event.subject = Target::Object(g.state().battlefield().iter().copied().find(|&id| g.chars(id).card == g.card_id(&CARD)).expect(\"source fixture\")); match CARD.abilities[0] {{ AbilityDef::Triggered(def) => (def.effect)(g, &event), _ => panic!() }} }}\n}}\n", generated(name)));
    }
    let mut zero = card("Horrid Shadowspinner");
    let Ability::Triggered { body, .. } = &mut zero.abilities[1] else { panic!() };
    let Instruction::Effect { operation: Operation::PayCost { cost, .. }, .. } = &mut body.instructions[0] else { panic!() };
    cost.parts[0] = CostPart::Draw { count: Expr::Constant(0) };
    lib.push_str(&format!("mod zero {{\n{}\npub fn callback(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {{ match CARD.abilities[0] {{ AbilityDef::Triggered(def) => (def.effect)(g,item), _ => panic!() }} }}\n}}\n", cantrip::module(&zero).unwrap()));
    lib.push_str(include_str!("fixtures/reference32_backend_scenarios.rs"));
    fs::write(root.join("src/lib.rs"), lib).unwrap();
    let result = Command::new("cargo").arg("test").arg("--manifest-path").arg(root.join("Cargo.toml"))
        .arg("--profile").arg("checked")
        .env("CARGO_TARGET_DIR", std::env::var_os("MTGISH_CANTRIP_TARGET").map(PathBuf::from).unwrap_or_else(|| root.join("target")))
        .env("CANTRIP_VERIFY", "1").output().unwrap();
    assert!(result.status.success(), "{}\n{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
    println!("{}", String::from_utf8_lossy(&result.stdout));
}

#[test]
fn planned_loyalty_and_creation_interfaces_unlock_only_their_cards() {
    for (name, feature, call) in [
        ("Deification", "damage-loyalty-floor", ".leave_loyalty("),
        ("Hofri Ghostforge", "creation-exile-links", ".create_token_copies_linked_to_exiled("),
    ] {
        let card = card(name);
        let old = cantrip::module(&card).unwrap_err();
        let enabled = cantrip::planned::with_additional_features(&[feature], || cantrip::module(&card))
            .unwrap_or_else(|error| panic!("{name}: {error:?}; old: {old:?}"));
        syn::parse_file(&enabled).unwrap();
        assert!(enabled.contains(call), "{enabled}");
        assert!(!enabled.contains("planned_value!"), "{enabled}");
        let unrelated = if feature == "damage-loyalty-floor" { "creation-exile-links" } else { "damage-loyalty-floor" };
        assert!(cantrip::planned::with_additional_features(&[unrelated], || cantrip::module(&card)).is_err());
        if let Ok(dir) = std::env::var("MTGISH_REFERENCE32_MODULES") {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(std::path::Path::new(&dir).join(format!("{}.rs", name.replace(' ', "_").to_lowercase())), &enabled).unwrap();
        }
    }
}

fn planned_rows(tag: &str, features: &[&str]) -> std::collections::BTreeMap<String, serde_json::Value> {
    use std::{collections::BTreeMap, path::PathBuf};
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let output = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("reference32-planned-{tag}.jsonl"));
    cantrip::planned::with_additional_features(features, || {
        cantrip::planned::tracked(&fixtures.join("planned_reference32.ronl"), &fixtures.join("planned_reference32.oracle.json"), &output)
    }).unwrap();
    std::fs::read_to_string(output).unwrap().lines().map(|line| {
        let row: serde_json::Value = serde_json::from_str(line).unwrap();
        (row["name"].as_str().unwrap().to_string(), row)
    }).collect::<BTreeMap<_, _>>()
}

#[test]
fn tracked_cards_keep_complete_future_code_but_are_not_publishable_on_the_old_api() {
    let held = planned_rows("held", &[]);
    let enabled = planned_rows("enabled", &["damage-loyalty-floor", "creation-exile-links"]);
    for (name, feature, calls) in [
        ("Deification", "damage loyalty floor", &[".leave_loyalty("][..]),
        ("Hofri Ghostforge", "noncopiable creation exile links", &[".creation_linked_exiled(", ".create_token_copies_linked_to_exiled("][..]),
    ] {
        let old = &held[name]; let new = &enabled[name];
        assert!(old["file"].is_null(), "{old}");
        assert!(old["faces"][0]["gaps"].as_array().unwrap().iter().any(|gap| gap["feature"] == feature), "{old}");
        let old_code = old["faces"][0]["rust"].as_str().unwrap();
        let new_code = new["faces"][0]["rust"].as_str().unwrap();
        syn::parse_file(old_code).unwrap(); syn::parse_file(new_code).unwrap();
        assert!(old_code.contains("planned_value!"), "{old_code}");
        assert!(new["faces"][0]["gaps"].as_array().unwrap().is_empty(), "{new}");
        assert!(new["file"].is_string(), "{new}");
        for call in calls { assert!(old_code.contains(call), "{old_code}"); assert!(new_code.contains(call), "{new_code}"); }
        if name == "Deification" { assert!(!new_code.contains("e.amount ="), "{new_code}"); }
        if name == "Hofri Ghostforge" {
            assert!(!new_code.contains("g.link_exiled("));
            assert!(!new_code.contains("remember_chosen_object("));
            assert!(new_code.contains("moved_0.clone()"), "{new_code}");
        }
        if let Ok(dir) = std::env::var("MTGISH_REFERENCE32_MODULES") {
            let stem = name.replace(' ', "_").to_lowercase();
            std::fs::write(std::path::Path::new(&dir).join(format!("{stem}-held.rs")), old_code).unwrap();
            std::fs::write(std::path::Path::new(&dir).join(format!("{stem}-enabled.rs")), new_code).unwrap();
        }
    }
}

#[test]
#[ignore = "type-check planned signatures against MTGISH_CANTRIP_DIR; no engine behavior is simulated"]
fn planned_interfaces_typecheck_as_complete_modules() {
    use std::{fs, path::PathBuf, process::Command};
    let checkout = PathBuf::from(std::env::var_os("MTGISH_CANTRIP_DIR").expect("set MTGISH_CANTRIP_DIR"));
    let root = PathBuf::from(std::env::var_os("MTGISH_REFERENCE32_CONTRACT_PROBE").expect("set MTGISH_REFERENCE32_CONTRACT_PROBE"));
    fs::create_dir_all(root.join("src")).unwrap();
    let mut manifest = "[package]\nname=\"mtgish-reference32-planned-contract\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n".to_string();
    for dep in ["cantrip-api", "cantrip-mechanics"] {
        manifest.push_str(&format!("{dep}={{path={:?}}}\n", checkout.join("crates").join(dep)));
    }
    manifest.push_str("[profile.checked]\ninherits=\"release\"\nopt-level=3\ndebug-assertions=true\ncodegen-units=256\nincremental=true\n");
    fs::write(root.join("Cargo.toml"), manifest).unwrap();
    let mut lib = "#![allow(dead_code, unused_imports, unused_variables, unused_mut, unused_parens)]\nmacro_rules! planned_value { ($feature:literal, $placeholder:expr => $($future:tt)*) => { $placeholder }; }\n".to_string();
    lib.push_str(include_str!("fixtures/reference32_planned_contract.rs"));
    for (profile, features) in [("held", &[][..]), ("enabled", &["damage-loyalty-floor", "creation-exile-links"][..])] {
        let rows = planned_rows(&format!("compile-{profile}"), features);
        for (name, row) in rows {
            let module = name.replace(' ', "_").to_lowercase();
            let code = row["faces"][0]["rust"].as_str().unwrap();
            let imports = if profile == "enabled" { "use crate::contract::*;" } else { "" };
            lib.push_str(&format!("mod {module}_{profile} {{ {imports}\n{code}\n}}\n"));
        }
    }
    fs::write(root.join("src/lib.rs"), lib).unwrap();
    let output = Command::new("cargo").arg("check").arg("--lib").arg("--manifest-path").arg(root.join("Cargo.toml"))
        .arg("--profile").arg("checked")
        .env("CARGO_TARGET_DIR", std::env::var_os("MTGISH_CANTRIP_TARGET").map(PathBuf::from).unwrap_or_else(|| root.join("target")))
        .output().unwrap();
    assert!(output.status.success(), "{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    println!("{}", String::from_utf8_lossy(&output.stderr));
}
