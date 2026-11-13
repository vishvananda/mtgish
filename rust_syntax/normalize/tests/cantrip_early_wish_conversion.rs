#![cfg(feature = "cantrip")]
use mtgish_ast::cantrip::{planned, pr_form::{render, Characteristics}};
use std::{collections::BTreeMap, path::PathBuf};
#[test]
fn numeric_result_conversion_and_mandatory_wish_are_preserved() {
    let f=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let out=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("early-wish-conversion.jsonl");
    planned::tracked(&f.join("early_wish_conversion.ronl"),&f.join("early_wish_conversion.oracle.json"),&out).unwrap();
    let rows:BTreeMap<String,serde_json::Value>=std::fs::read_to_string(out).unwrap().lines().map(|s| {let v:serde_json::Value=serde_json::from_str(s).unwrap();(v["name"].as_str().unwrap().into(),v)}).collect();
    for name in ["Nameless Race","Ring of Ma'rûf","Death Wish","North Wind Avatar","Enduring Renewal"] {assert!(rows[name]["faces"][0]["gaps"].as_array().unwrap().is_empty(),"{name}: {}",rows[name]);}
    let race=rows["Nameless Race"]["faces"][0]["rust"].as_str().unwrap();
    let rendered=render(race, "Nameless Race", "As this creature enters, pay any amount of life.", Characteristics::Oracle).unwrap();
    std::fs::write(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("nameless-rendered.rs"), &rendered).unwrap();
    assert!(rendered.contains(".try_into()") && rendered.contains(".expect(\"payable life fits the native life total\")") && !rendered.contains("let Some(named_"), "{rendered}");
    let ring=rows["Ring of Ma'rûf"]["faces"][0]["rust"].as_str().unwrap();
    let ring_rendered=render(ring, "Ring of Ma'rûf", "The next time you would draw a card this turn, instead put a card you own from outside the game into your hand.", Characteristics::Oracle).unwrap();
    std::fs::write(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ring-rendered.rs"), &ring_rendered).unwrap();
    assert!(ring.contains("Purpose::PutIntoHand") && ring.contains(", 1, false)"),"{ring}");
    let end=rows["Enduring Renewal"]["faces"][0]["rust"].as_str().unwrap();
    let end_rendered=render(end, "Enduring Renewal", "Play with your hand revealed.\nIf you would draw a card, reveal the top card of your library instead. If it is a creature card, put it into your graveyard. Otherwise, draw a card.", Characteristics::Oracle).unwrap();
    std::fs::write(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("enduring-rendered.rs"), &end_rendered).unwrap();
    let parsed=syn::parse_file(&end_rendered).unwrap();
    let docs=|name:&str| parsed.items.iter().find_map(|item| match item {
        syn::Item::Fn(f) if f.sig.ident == name => Some(f.attrs.iter().filter_map(|a| match &a.meta {
            syn::Meta::NameValue(v) => match &v.value {syn::Expr::Lit(v) => match &v.lit {syn::Lit::Str(v) => Some(v.value()), _ => None}, _ => None}, _ => None
        }).collect::<Vec<_>>().join(" ")), _=>None
    }).unwrap();
    assert!(docs("hand_revealed_0").contains("604.1") && !docs("hand_revealed_0").contains("608.2"));
    assert!(docs("if_would_draw_card_reveal_then_steps").contains("614.1a") && !docs("if_would_draw_card_reveal_then_steps").contains("608.2"));
    assert!(docs("if_would_draw_card_reveal").contains("608.2"));
    for name in ["Death Wish","North Wind Avatar"] {let rust=rows[name]["faces"][0]["rust"].as_str().unwrap();assert!(rust.contains("Count::UpTo"),"{name}: {rust}");}
}
