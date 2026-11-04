#![cfg(feature = "cantrip")]
use mtgish_ast::cantrip::planned;
use std::{collections::BTreeMap, path::PathBuf};

fn rows(features: &[&str], tag: &str) -> BTreeMap<String, serde_json::Value> {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let output = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("regeneration-bans-{tag}.jsonl"));
    planned::with_features(features, || planned::tracked(
        &fixtures.join("regeneration_bans.ronl"), &fixtures.join("regeneration_bans.oracle.json"), &output)).unwrap();
    std::fs::read_to_string(output).unwrap().lines().map(|line| {
        let row: serde_json::Value = serde_json::from_str(line).unwrap();
        (row["name"].as_str().unwrap().to_owned(), row)
    }).collect()
}

#[test]
fn regeneration_prohibitions_are_held_on_old_targets_and_use_native_rules_when_available() {
    let old = rows(&[], "old");
    let new = rows(&["regeneration-bans"], "new");
    for name in ["Clergy of the Holy Nimbus", "Disintegrate", "Gravebind", "Hurr Jackal", "Incinerate",
        "Lim-Dûl's Cohort", "Orcish Healer", "Whippoorwill"] {
        assert!(old[name]["faces"][0]["gaps"].as_array().unwrap().iter()
            .any(|gap| gap["feature"].as_str().unwrap().contains("Prohibit Regenerate")), "{name}: {}", old[name]);
        let gaps = new[name]["faces"][0]["gaps"].as_array().unwrap();
        assert!(!gaps.iter().any(|gap| gap["feature"].as_str().unwrap().contains("Prohibit Regenerate")), "{name}: {gaps:?}");
        let code = new[name]["faces"][0]["rust"].as_str().unwrap();
        assert!(code.contains("EffectKind::CantBeRegenerated"), "{name}: {code}");
        match name {
            "Clergy of the Holy Nimbus" => assert!(gaps.iter().any(|gap| gap["feature"] == "engine: no activation by other players")),
            "Whippoorwill" => assert!(gaps.iter().any(|gap| gap["feature"] == "continuous rule Unique")),
            _ => assert!(gaps.is_empty(), "{name}: {}", new[name]),
        }
    }
}

#[test]
fn bone_shaman_grants_a_real_static_ban_with_its_printed_text() {
    let old = rows(&[], "old-grant");
    let new = rows(&["regeneration-bans"], "new-grant");
    assert!(!old["Bone Shaman"]["faces"][0]["gaps"].as_array().unwrap().is_empty());
    let row = &new["Bone Shaman"];
    let code = row["file"].as_str().unwrap();
    assert!(row["faces"][0]["gaps"].as_array().unwrap().is_empty(), "{row}");
    assert!(code.contains("granted_regeneration_ban("), "{code}");
    assert!(code.contains("AbilityRef::delayed("), "{code}");
    assert!(code.contains("EffectKind::AddAbility("), "{code}");
    assert!(code.contains("Creatures dealt damage by this creature this turn can't be regenerated this turn."), "{code}");
    assert!(!code.contains("granted_static_rules"), "{code}");
    let parsed = syn::parse_file(code).unwrap();
    let predicate_docs = parsed.items.iter().filter_map(|item| match item {
        syn::Item::Fn(f) if f.sig.inputs.iter().any(|arg| quote::quote!(#arg).to_string().contains("ObjectId"))
            && f.sig.inputs.iter().any(|arg| quote::quote!(#arg).to_string().contains("Asker")) => Some(&f.attrs),
        _ => None,
    }).map(|attrs| quote::quote!(#(#attrs)*).to_string()).collect::<Vec<_>>();
    assert_eq!(predicate_docs.len(), 1, "{code}");
    assert!(predicate_docs[0].contains("604.1"));
    assert!(!predicate_docs[0].contains("{B}"), "static predicate must quote only the granted ability: {}", predicate_docs[0]);
}
