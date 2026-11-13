#![cfg(feature = "cantrip")]
use mtgish_ast::cantrip::planned;
use std::{collections::BTreeMap, path::PathBuf};
#[test]
fn untap_deadlines_and_current_drawn_objects() {
    let f=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let out=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-untap-draw.jsonl");
    planned::tracked(&f.join("early_untap_draw.ronl"),&f.join("early_untap_draw.oracle.json"),&out).unwrap();
    let rows:BTreeMap<String,serde_json::Value>=std::fs::read_to_string(out).unwrap().lines().map(|s| { let v:serde_json::Value=serde_json::from_str(s).unwrap();(v["name"].as_str().unwrap().into(),v) }).collect();
    for name in ["Deep Spawn","Homarid Warrior","Barl's Cage","Krovikan Sorcerer"] {assert!(rows[name]["faces"][0]["gaps"].as_array().unwrap().is_empty(),"{name}: {}",rows[name]);}
    for name in ["Deep Spawn","Homarid Warrior","Reveka, Wizard Savant","Cinder Marsh","Samite Alchemist","Spectral Bears"] {let rust=rows[name]["faces"][0]["rust"].as_str().unwrap();assert!(rust.contains("Duration::DuringNextUntapStepOf") && !rust.contains("Duration::DuringItsControllersNextUntapStep"),"{name}: {rust}");}
    let cage=rows["Barl's Cage"]["faces"][0]["rust"].as_str().unwrap();assert!(cage.contains("Duration::DuringItsControllersNextUntapStep"),"{cage}");
    let farmer=&rows["Orcish Farmer"]["faces"][0];
    assert!(farmer["gaps"].as_array().unwrap().iter().any(|g| g.to_string().contains("current controller's next untap step")), "{farmer}");
    // A named player has a fixed deadline and remains supported.
    let source=std::fs::read_to_string(f.join("early_untap_draw.ronl")).unwrap();
    let farmer=source.lines().find(|line| line.starts_with("Card(Name: \"Orcish Farmer\"")).unwrap();
    let fixed=farmer.replace("UntilPlayersNextUntapStep(ControllerOfPermanent(Ref_TargetPermanent))", "UntilPlayersNextUntapStep(You)");
    assert_ne!(farmer, fixed);
    let input=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("fixed-player-untap.ronl");
    let output=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("fixed-player-untap.jsonl");
    std::fs::write(&input, fixed+"\n").unwrap();
    planned::tracked(&input,&f.join("early_untap_draw.oracle.json"),&output).unwrap();
    let row:serde_json::Value=serde_json::from_str(std::fs::read_to_string(output).unwrap().trim()).unwrap();
    assert!(row["faces"][0]["gaps"].as_array().unwrap().is_empty(),"{row}");
    let rust=row["faces"][0]["rust"].as_str().unwrap();
    assert!(rust.contains("Duration::UntilNextStepOf { step: Step::Untap"),"{rust}");
    let sorcerer=rows["Krovikan Sorcerer"]["faces"][0]["rust"].as_str().unwrap();assert!(sorcerer.contains("zone(Zone::hand(player)).contains(&card)"),"{sorcerer}");
}
