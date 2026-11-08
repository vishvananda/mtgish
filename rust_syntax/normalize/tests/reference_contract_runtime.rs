#![cfg(feature = "cantrip")]
//! Explicit native regression gate, separate from ordinary AST-only validation.
use mtgish_ast::{cantrip, corpus, lower, mtg_types::OracleCard};
use std::{fs, path::PathBuf, process::Command};
#[test]
#[ignore = "compile and execute generated reference regressions against MTGISH_CANTRIP_DIR"]
fn generated_reference_contracts_execute_in_cantrip() {
    let checkout = PathBuf::from(
        std::env::var_os("MTGISH_CANTRIP_DIR")
            .expect("set MTGISH_CANTRIP_DIR to the Cantrip checkout"),
    );
    let root = std::env::var_os("MTGISH_REFERENCE_PROBE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("mtgish-reference-contracts"));
    fs::create_dir_all(root.join("src")).unwrap();
    let mut manifest="[package]\nname=\"mtgish-reference-contracts\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n".to_string();
    for dep in [
        "cantrip-api",
        "cantrip-mechanics",
        "cantrip-rules",
        "cantrip-state",
    ] {
        manifest.push_str(&format!(
            "{dep}={{path={:?}}}\n",
            checkout.join("crates").join(dep).to_string_lossy()
        ));
    }
    manifest.push_str("[profile.checked]\ninherits=\"release\"\nopt-level=3\ndebug-assertions=true\ncodegen-units=256\nincremental=true\n");
    fs::write(root.join("Cargo.toml"), manifest).unwrap();
    let sources: Vec<OracleCard> = include_str!("fixtures/reference_investigations.ronl")
        .lines()
        .chain(include_str!("fixtures/sacrifice_public_destinations.ronl").lines())
        .chain(include_str!("fixtures/oracle_fixes_20261107.ronl").lines())
        .chain(include_str!("fixtures/joint_blink_reference_fixes.ronl").lines())
        .chain(include_str!("fixtures/return_followup_reference_fixes.ronl").lines())
        .map(|s| ron::from_str(s).unwrap())
        .collect();
    let mut lib = String::new();
    for (module, name) in [
        ("abstract_performance", "Abstract Performance"),
        ("it_that_betrays", "It That Betrays"),
        ("ambergris_balance", "Ambergris, Agent of Balance"),
        ("ambergris__agent_of_law", "Ambergris, Agent of Law"),
        ("arcane_artisan", "Arcane Artisan"),
        ("chimeric_idol", "Chimeric Idol"),
        ("haunted_fengraf", "Haunted Fengraf"),
        ("karn_s_touch", "Karn's Touch"),
        ("laquatus_s_creativity", "Laquatus's Creativity"),
        ("calibrated_blast", "Calibrated Blast"),
        ("yuna_s_whistle", "Yuna's Whistle"),
        ("young_necromancer", "Young Necromancer"),
        ("kroxa_and_kunoros", "Kroxa and Kunoros"),
        ("arashin_sovereign", "Arashin Sovereign"),
        ("avenging_angel", "Avenging Angel"),
        ("ambergris_progress", "Ambergris, Agent of Progress"),
        ("cloudshift", "Cloudshift"),
        ("conjurers_closet", "Conjurer's Closet"),
        ("restoration_angel", "Restoration Angel"),
        ("basalt_golem", "Basalt Golem"),
        ("portal", "Justiciar's Portal"),
        ("flux", "Essence Flux"),
        ("respite", "Hallowed Respite"),
        ("obzedat", "Obzedat, Ghost Council"),
    ] {
        let source = sources
            .iter()
            .find(|s| {
                corpus::Identity::of(s)
                    .unwrap()
                    .names
                    .iter()
                    .any(|n| n == name)
            })
            .unwrap();
        let card = lower::card(source).unwrap();
        lib.push_str(&format!(
            "mod {module} {{\n{}\n",
            cantrip::module(&card).unwrap()
        ));
        if name.starts_with("Ambergris,") {
            lib.push_str(CALLBACK)
        }
        lib.push_str("}\n");
    }
    let legacy = lower::card(
        &ron::from_str(include_str!("fixtures/legacy_compound_payment.ronl").trim()).unwrap(),
    )
    .unwrap();
    lib.push_str(&format!(
        "mod ambergris_legacy {{\n{}\n{CALLBACK}\n}}\n",
        cantrip::module(&legacy).unwrap()
    ));
    let legacy_sacrifice = lower::card(
        &ron::from_str(include_str!("fixtures/legacy_sacrifice_graveyard.ronl").trim()).unwrap(),
    )
    .unwrap();
    lib.push_str(&format!(
        "mod it_that_betrays_legacy {{\n{}\n}}\n",
        cantrip::module(&legacy_sacrifice).unwrap()
    ));
    lib.push_str(include_str!("fixtures/reference_contract_scenarios.rs"));
    lib.push_str(include_str!("fixtures/oracle_fixes_20261107_scenarios.rs"));
    lib.push_str(include_str!("fixtures/joint_blink_reference_scenarios.rs"));
    lib.push_str(include_str!(
        "fixtures/return_followup_reference_scenarios.rs"
    ));
    fs::write(root.join("src/lib.rs"), lib).unwrap();
    let target = std::env::var_os("MTGISH_CANTRIP_TARGET")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"));
    let result = Command::new("cargo")
        .args(["test", "--profile", "checked"])
        .current_dir(&root)
        .env("CARGO_TARGET_DIR", target)
        .env("RUST_MIN_STACK", "33554432")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    println!("{}", String::from_utf8_lossy(&result.stdout));
}
const CALLBACK: &str = "pub fn contract_callback(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> { match CARD.abilities[0] { AbilityDef::Triggered(def) => (def.effect)(g,item), _ => panic!(\"expected attack trigger\") } }\n";
