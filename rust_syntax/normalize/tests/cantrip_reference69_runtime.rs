#![cfg(feature="cantrip")]
use mtgish_ast::{cantrip,ir::*,mtg_types::OracleCard,unified};
use std::{fs,path::PathBuf,process::Command};
#[test]
#[ignore="compile and execute against the exact pinned MTGISH_CANTRIP_DIR"]
fn pool_exclusion_and_infinity_presence_execute_in_cantrip() {
    let checkout=PathBuf::from(std::env::var_os("MTGISH_CANTRIP_DIR").unwrap());
    let root=PathBuf::from(std::env::var_os("MTGISH_REFERENCE69_PROBE").unwrap());
    fs::create_dir_all(root.join("src")).unwrap();
    let mut manifest="[package]\nname=\"mtgish-reference69-runtime\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n".to_string();
    for dep in ["cantrip-api","cantrip-state","cantrip-mechanics","cantrip-rules"] {
        manifest.push_str(&format!("{dep}={{path={:?}}}\n",checkout.join("crates").join(dep)));
    }
    manifest.push_str("[profile.checked]\ninherits=\"release\"\nopt-level=3\ndebug-assertions=true\ncodegen-units=256\nincremental=true\n");
    fs::write(root.join("Cargo.toml"),manifest).unwrap();
    let mut cards=vec![];
    for line in include_str!("fixtures/reference69_20261110.ronl").lines() {
        let source:OracleCard=ron::from_str(line).unwrap();cards.extend(unified::lower(&source).card.faces);
    }
    let mut lib=String::new();
    for (module,name) in [("knowledge_pool","Knowledge Pool"),("soul_stone","The Soul Stone"),("mind_stone","The Mind Stone"),("bombshell","Bronze Bombshell")] {
        let card:&Card=cards.iter().find(|c|c.name==name).unwrap();
        lib.push_str(&format!("mod {module} {{\n{}\n}}\n",cantrip::module(card).unwrap()));
    }
    lib.push_str(include_str!("fixtures/reference69_runtime_scenarios.rs"));
    fs::write(root.join("src/lib.rs"),lib).unwrap();
    let result=Command::new("cargo").args(["test","--profile","checked"]).current_dir(&root)
        .env("CARGO_TARGET_DIR",std::env::var_os("MTGISH_CANTRIP_TARGET").unwrap())
        .env("RUST_MIN_STACK","33554432").output().unwrap();
    assert!(result.status.success(),"{}\n{}",String::from_utf8_lossy(&result.stdout),String::from_utf8_lossy(&result.stderr));
    println!("{}",String::from_utf8_lossy(&result.stdout));
}
