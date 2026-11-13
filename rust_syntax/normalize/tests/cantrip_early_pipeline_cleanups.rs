#![cfg(feature = "cantrip")]
use mtgish_ast::cantrip::{planned,pr_form::{render,Characteristics}};
use std::{collections::BTreeMap,path::PathBuf};
#[test]
fn generated_files_avoid_unneeded_history_and_checked_answer_panics() {
    let f=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let out=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-pipeline-cleanups.jsonl");
    planned::tracked(&f.join("early_pipeline_cleanups.ronl"),&f.join("early_pipeline_cleanups.oracle.json"),&out).unwrap();
    let rows:BTreeMap<String,serde_json::Value>=std::fs::read_to_string(out).unwrap().lines().map(|s| {let v:serde_json::Value=serde_json::from_str(s).unwrap();(v["name"].as_str().unwrap().into(),v)}).collect();
    for name in ["Power Sink","Dream Coat","Nameless Race"] {assert!(rows[name]["faces"][0]["gaps"].as_array().unwrap().is_empty(),"{name}: {}",rows[name]);}
    let printed=|name:&str,text:&str| render(rows[name]["faces"][0]["rust"].as_str().unwrap(),name,text,Characteristics::Oracle).unwrap();
    let power=printed("Power Sink","Counter target spell unless its controller pays {X}. If they don't, tap all lands with mana abilities that player controls and they lose all unspent mana.");
    assert!(power.contains("tap_together") && !power.contains("history()") && !power.contains("hoisted_"),"{power}");
    let coat=printed("Dream Coat","{0}: Enchanted creature becomes the color or colors of your choice. Activate only once each turn.");
    assert!(!coat.contains("unreachable!") && coat.contains(".attached_to(o)") && coat.contains(".or_else(||") && coat.contains("is_live(q, host)"),"{coat}");
    let race=printed("Nameless Race","As this creature enters, pay any amount of life.");
    assert!(race.contains("life: selected_0") && !race.contains(".expect("),"{race}");
}

#[test]
fn result_collectors_remain_when_read_or_when_their_input_has_effects() {
    let raw = r#"
use cantrip_api::*;
fn ability(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {
    let objects: Vec<ObjectId> = vec![item.object];
    let mut hoisted_0: Vec<ObjectId> = Vec::new();
    let mut hoisted_1: Vec<ObjectId> = Vec::new();
    if g.is_on_battlefield(item.object) {
        hoisted_0.extend(objects.clone());
        hoisted_1.extend(g.exile_all(&objects));
    }
    g.tap_together(&hoisted_0);
    Ok(())
}
pub static CARD: CardDef = CardDef { spell: spell(&[], ability), ..CardDef::BLANK };
"#;
    let result=render(raw,"Collector Probe","Exile and tap objects.",Characteristics::Oracle).unwrap();
    assert!(result.contains("hoisted_0") && result.contains("tap_together"),"{result}");
    assert!(result.contains("exile_all"),"{result}");
}
