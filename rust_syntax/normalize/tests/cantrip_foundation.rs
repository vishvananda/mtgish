#![cfg(feature = "cantrip")]
//! The native early-set foundation is an explicit candidate profile until merged upstream.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf, sync::OnceLock};
const FOUNDATION: &[&str] = &["world-rule", "landwalk-blocking-exceptions", "independent-pt-settings", "no-combat-damage-assignment"];
type Rows = BTreeMap<String,serde_json::Value>;
fn read(path: &std::path::Path) -> Rows {
    std::fs::read_to_string(path).unwrap().lines().map(|line| { let row:serde_json::Value=serde_json::from_str(line).unwrap();(row["name"].as_str().unwrap().to_string(),row) }).collect()
}
fn profiles() -> &'static (Rows,Rows) {
    static ROWS:OnceLock<(Rows,Rows)>=OnceLock::new();
    ROWS.get_or_init(|| {
        let fixtures=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let source=fixtures.join("early_set_foundation.ronl");let oracle=fixtures.join("early_set_foundation.oracle.json");
        let baseline=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-set-foundation-baseline.jsonl");
        let candidate=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-set-foundation-candidate.jsonl");
        cantrip::planned::tracked(&source,&oracle,&baseline).unwrap();
        cantrip::planned::with_additional_features(FOUNDATION, || cantrip::planned::tracked(&source,&oracle,&candidate).unwrap());
        (read(&baseline),read(&candidate))
    })
}
fn gaps(row:&serde_json::Value)->Vec<&str> {
    row["faces"].as_array().unwrap().iter().flat_map(|f|f["gaps"].as_array().unwrap()).map(|g|g["feature"].as_str().unwrap()).collect()
}
fn code(row:&serde_json::Value)->&str { row["faces"][0]["rust"].as_str().unwrap() }
#[test]
fn foundation_removes_only_its_four_requirements_from_the_33_affected_cards() {
    let (baseline,candidate)=profiles();assert_eq!(baseline.len(),33);assert_eq!(candidate.len(),33);
    assert!(baseline.values().all(|r| !gaps(r).is_empty()));
    let still_blocked:Vec<_>=candidate.iter().filter(|(_,r)|!gaps(r).is_empty()).map(|(n,_)|n.as_str()).collect();
    assert_eq!(still_blocked,["Caverns of Despair","Field of Dreams","Land's Edge","Revelation"]);
    for (name,row) in candidate {
        syn::parse_file(code(row)).unwrap_or_else(|e|panic!("{name}: {e}"));
        assert!(!gaps(row).iter().any(|g|matches!(*g,"supertype World"|"static rule BlockedAsThoughLacking"|"engine: no effect sets power or toughness alone"|"continuous rule CombatDamage NoAssignment")),"{name}: {:?}",gaps(row));
    }
}
#[test]
fn candidate_uses_native_rules_and_supplies_the_required_world_field() {
    let (baseline,candidate)=profiles();
    for row in candidate.values() { assert!(code(row).contains("world: ")); }
    assert!(code(&candidate["Arboria"]).contains("world: true"));
    assert!(code(&candidate["Great Wall"]).contains("landwalk_exceptions:"));
    assert!(code(&candidate["Staff of the Ages"]).contains("Keywords::LANDWALKS"));
    assert!(!code(&candidate["Great Wall"]).contains("LoseKeywords"));
    assert!(code(&candidate["Island of Wak-Wak"]).contains("EffectKind::SetPower"));
    assert!(code(&candidate["Sentinel"]).contains("EffectKind::SetToughness"));
    assert!(code(&candidate["Gaze of Pain"]).contains("EffectKind::NoCombatDamageAssignment"));
    assert!(!code(&baseline["Great Wall"]).contains("landwalk_exceptions:"));
    assert!(!code(&baseline["Island of Wak-Wak"]).contains("world: "));
}

#[test]
#[ignore="execute the candidate profile against MTGISH_CANTRIP_DIR; use the foundation branch"]
fn foundation_generated_cards_execute_in_cantrip() {
    use std::{fs,process::Command};
    let checkout=PathBuf::from(std::env::var_os("MTGISH_CANTRIP_DIR").expect("foundation checkout"));
    let root=PathBuf::from(std::env::var_os("MTGISH_FOUNDATION_PROBE").unwrap_or_else(||PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("foundation-runtime").into_os_string()));
    fs::create_dir_all(root.join("src")).unwrap();
    let fixture=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let old=root.join("earlier-candidate.jsonl");
    cantrip::planned::with_additional_features(FOUNDATION,||cantrip::planned::tracked(&fixture.join("early_set_generator_gaps.ronl"),&fixture.join("early_set_generator_gaps.oracle.json"),&old).unwrap());
    let mut cards=read(&old);cards.extend(profiles().1.clone());
    // Modules actually used by these scenarios. All 33 foundation modules have a separate compile probe;
    // Delif's Cube's standard counter-name prerequisite is intentionally separate from these runtime cases.
    let modules=["Orcish Farmer","War Barge","Merieke Ri Berit","Ray of Command","Dreams of the Dead","Mirror Universe","Necropolis","Illusionary Terrain","Nameless Race","Forgotten Lore","Infernal Darkness","Tetravus","Frankenstein's Monster","Game of Chaos","Clockwork Beast","Anti-Magic Aura","Soul Exchange","Vision Charm","Island Sanctuary","Great Wall","Gaze of Pain","Arboria","Concordant Crossroads","Island of Wak-Wak"];
    let mut lib=String::from("#![allow(dead_code,unused_imports,unused_variables,unused_mut)]\n");
    for name in modules {
        let row=&cards[name];assert!(gaps(row).is_empty(),"{name}: {:?}",gaps(row));
        let stem:String=name.chars().map(|c|if c.is_ascii_alphanumeric(){c.to_ascii_lowercase()}else{'_'}).collect();
        let stem=stem.split('_').filter(|s|!s.is_empty()).collect::<Vec<_>>().join("_");
        lib.push_str(&format!("mod {stem} {{\n{}\n}}\n",code(row)));
    }
    lib.push_str(include_str!("fixtures/early_set_generator_scenarios.rs"));
    lib.push_str(include_str!("fixtures/early_set_foundation_scenarios.rs"));
    fs::write(root.join("src/lib.rs"),lib).unwrap();
    let mut manifest=String::from("[package]\nname=\"mtgish-foundation-runtime\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n");
    for name in ["cantrip-api","cantrip-state","cantrip-mechanics","cantrip-rules"] {manifest.push_str(&format!("{name}={{path={:?}}}\n",checkout.join("crates").join(name)));}
    fs::write(root.join("Cargo.toml"),manifest).unwrap();
    let result=Command::new("cargo").arg("test").current_dir(&root).env("CARGO_TARGET_DIR",std::env::var_os("MTGISH_CANTRIP_TARGET").expect("separate native target")).env("CANTRIP_VERIFY","1").env("RUST_MIN_STACK","33554432").output().unwrap();
    assert!(result.status.success(),"{}\n{}",String::from_utf8_lossy(&result.stdout),String::from_utf8_lossy(&result.stderr));
    println!("{}",String::from_utf8_lossy(&result.stdout));
}

#[test]
fn a_landwalk_exception_does_not_unlock_ignoring_an_unrelated_ability() {
    use mtgish_ast::{corpus,ir::*,mtg_types::OracleCard,unified};
    let source=include_str!("fixtures/early_set_foundation.ronl").lines().map(|line|ron::from_str::<OracleCard>(line).unwrap())
        .find(|s|corpus::Identity::of(s).unwrap().names.iter().any(|n|n=="Great Wall")).unwrap();
    let mut lowered=unified::lower(&source);
    let Ability::Static {changes,..}=&mut lowered.card.faces[0].abilities[0] else {panic!()};
    let Change::Rule(RuleChange::BlockedAsThoughLacking(ability))=&mut changes[0] else {panic!()};
    *ability=Box::new(Operand::Vocabulary {family:"CheckHasable".into(),form:"Flying".into()});
    cantrip::planned::with_additional_features(FOUNDATION,|| {
        let generated=cantrip::whole(&source,&lowered.card).unwrap();
        assert!(generated.faces.iter().any(|f|f.gaps.iter().any(|g|g.feature=="static rule BlockedAsThoughLacking")));
    });
}
