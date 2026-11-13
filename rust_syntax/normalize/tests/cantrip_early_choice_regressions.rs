#![cfg(feature = "cantrip")]
use mtgish_ast::cantrip::planned;
use std::{collections::BTreeMap,path::PathBuf};
#[test]
fn early_set_choices_keep_ties_subjects_and_optional_replacement() {
    let fixtures=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let out=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-choice-regressions.jsonl");
    planned::tracked(&fixtures.join("early_choice_regressions.ronl"),&fixtures.join("early_choice_regressions.oracle.json"),&out).unwrap();
    let rows: BTreeMap<String,serde_json::Value>=std::fs::read_to_string(out).unwrap().lines().map(|s| {let v:serde_json::Value=serde_json::from_str(s).unwrap();(v["name"].as_str().unwrap().into(),v)}).collect();
    for name in ["Call to Arms","Cleansing"] {assert!(rows[name]["faces"][0]["gaps"].as_array().unwrap().is_empty(),"{name}: {}",rows[name]);}
    let call=rows["Call to Arms"]["faces"][0]["rust"].as_str().unwrap();
    assert_eq!(call.matches("count(c) == best).count() == 1").count(),2,"{call}");
    let trigger=call.split("fn ability_2_state").nth(1).expect("state-trigger matcher");
    assert!(trigger.contains("-> bool { !({ let those:"),"{trigger}");
    let cleansing=rows["Cleansing"]["faces"][0]["rust"].as_str().unwrap();
    assert!(cleansing.contains("g.may_pay(payer_2, asker.source, bound_0,"),"{cleansing}");
    assert!(cleansing.contains("pay {cost} for {subject}?"),"{cleansing}");
    let gaps=rows["Blood of the Martyr"]["faces"][0]["gaps"].as_array().unwrap();
    assert!(gaps.iter().any(|g| g.to_string().contains("optional granted replacement needs a choice")),"{:?}",rows["Blood of the Martyr"]);
}
