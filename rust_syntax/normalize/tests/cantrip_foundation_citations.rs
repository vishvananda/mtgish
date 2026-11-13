#![cfg(feature = "cantrip")]

use mtgish_ast::cantrip;
use std::path::PathBuf;

#[test]
fn foundation_callbacks_cite_the_rules_their_api_positions_implement() {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    if !data.join("mtgish.lines.ron").exists() || !data.join("oracle.json").exists() {
        return;
    }
    let cases = [
        ("Adventurers' Guildhouse", "granted_0_bands_0", "702.22b"),
        ("Cathedral of Serra", "granted_0_bands_0", "702.22b"),
        ("Clergy of the Holy Nimbus", "creature_cant_be_regenerated_only_activators", "602.2"),
        ("Cursed Rack", "as_artifact_enters_choose_opponent_hand_size_0", "402.2"),
        ("Damping Field", "players_cant_untap_more_than_untaps_0", "502.3"),
        ("Damping Field", "players_cant_untap_more_than_untapping_0", "502.3"),
        // The same card's resolving effect must retain its resolution rule.
        ("Clergy of the Holy Nimbus", "creature_cant_be_regenerated_only", "608.2"),
    ];
    let mut names: Vec<String> = cases.iter().map(|(name, _, _)| (*name).into()).collect();
    names.sort();
    names.dedup();
    let modules = cantrip::modules_printed(
        &data.join("mtgish.lines.ron"),
        &data.join("oracle.json"),
        &names,
    ).unwrap();
    let printed: serde_json::Value =
        serde_json::from_slice(&std::fs::read(data.join("oracle.json")).unwrap()).unwrap();
    for (name, function, rule) in cases {
        let module = modules.iter().find(|(n, _)| n == name).unwrap();
        let text = printed.as_array().unwrap().iter()
            .flat_map(|card| card["cards"].as_array().unwrap())
            .find(|face| face["name"] == name).unwrap()["text"].as_str().unwrap();
        let rendered = cantrip::pr_form::render(
            &module.1, name, text, cantrip::pr_form::Characteristics::Oracle,
        ).unwrap();
        let file = syn::parse_file(&rendered).unwrap();
        let function = file.items.iter().find_map(|item| match item {
            syn::Item::Fn(f) if f.sig.ident == function => Some(f),
            _ => None,
        }).unwrap_or_else(|| panic!("{name}: missing {function}"));
        let docs: String = function.attrs.iter().filter_map(|attr| {
            let syn::Meta::NameValue(meta) = &attr.meta else { return None };
            if !meta.path.is_ident("doc") { return None; }
            let syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(value), .. }) = &meta.value else { return None };
            Some(value.value())
        }).collect();
        assert!(docs.contains(&format!("CR {rule}")), "{name}: {docs}");
        if rule != "608.2" {
            assert!(!docs.contains("608.2"), "{name}: {docs}");
        }
    }
}
