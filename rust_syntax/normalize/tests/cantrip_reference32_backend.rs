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
