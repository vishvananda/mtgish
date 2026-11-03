#![cfg(feature = "cantrip")]
//! Callback citations follow their actual API role, including identical condition signatures.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf, sync::OnceLock};

fn modules() -> &'static BTreeMap<String, String> {
    static MODULES: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    MODULES.get_or_init(|| {
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let output = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("citation-roles.jsonl");
        cantrip::planned::tracked(&fixtures.join("citation_roles.ronl"), &fixtures.join("citation_roles.oracle.json"), &output).unwrap();
        std::fs::read_to_string(output).unwrap().lines().map(|line| {
            let row: serde_json::Value = serde_json::from_str(line).unwrap();
            (row["faces"][0]["name"].as_str().unwrap().to_string(), row["file"].as_str().unwrap().to_string())
        }).collect()
    })
}

fn docs(source: &str, name: impl Fn(&str) -> bool) -> Vec<String> {
    syn::parse_file(source).unwrap().items.iter().filter_map(|item| {
        let (ident, attrs) = match item {
            syn::Item::Fn(f) => (&f.sig.ident, &f.attrs),
            syn::Item::Static(s) => (&s.ident, &s.attrs),
            _ => return None,
        };
        if !name(&ident.to_string()) { return None; }
        Some(attrs.iter().filter_map(|attr| match &attr.meta {
            syn::Meta::NameValue(value) if value.path.is_ident("doc") => match &value.value {
                syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(text), .. }) => Some(text.value()),
                _ => None,
            },
            _ => None,
        }).collect::<Vec<_>>().join("\n"))
    }).collect()
}

#[test]
fn aura_target_callbacks_cite_targeting_instead_of_resolution() {
    for name in ["Divine Transformation", "Fishliver Oil", "Gaseous Form", "Giant Strength", "Immolation", "Seeker"] {
        let found = docs(&modules()[name], |name| name.ends_with("_enchant"));
        if found.is_empty() {
            // Exact filters can already be replaced by a shared target recipe.
            assert!(modules()[name].contains("enchant: Some(targets::"), "{name}: {}", modules()[name]);
            continue;
        }
        for doc in found {
            assert!(doc.contains("CR 115.1"), "{name}: {doc}");
            assert!(!doc.contains("608.2"), "{name}: {doc}");
        }
    }
}

#[test]
fn activation_costs_and_timing_are_not_resolving_effects() {
    for name in ["Angus Mackenzie", "Colossus of Sardia", "Desert", "Dwarven Weaponsmith", "Gate to Phyrexia", "Hell's Caretaker", "Nebuchadnezzar"] {
        let found = docs(&modules()[name], |name| name.ends_with("_if"));
        if name == "Nebuchadnezzar" && found.is_empty() {
            assert!(modules()[name].contains(".timing(Timing::OwnTurn)"));
            continue;
        }
        assert!(!found.is_empty(), "{name}: {}", modules()[name]);
        for doc in found { assert!(doc.contains("CR 602.5"), "{name}: {doc}"); }
    }
    assert!(modules()["Gate to Phyrexia"].contains("CR 118.1"));
    assert!(modules()["Dwarven Weaponsmith"].contains("CR 118.1"));
    let conditions = docs(&modules()["Teleport"], |name| name.ends_with("_if"));
    assert!(!conditions.is_empty());
    for doc in conditions { assert!(doc.contains("CR 601.3"), "{doc}"); }
}

#[test]
fn nonstandard_callbacks_with_ambiguous_names_follow_the_api_position() {
    use cantrip::pr_form::{render, Characteristics};
    let filter = "fn filter(q: &dyn Query, _: &Asker, t: Target) -> bool { matches!(t, Target::Object(id) if q.chars(id).power.unwrap_or(0) > 7) }";
    let raw = format!("use cantrip_api::*; {filter} pub static CARD: CardDef = CardDef {{ enchant: Some(target(filter)), ..CardDef::BLANK }};");
    let output = render(&raw, "Fixture", "Enchant creature with power 8 or greater", Characteristics::Oracle).unwrap();
    assert!(docs(&output, |name| name == "filter")[0].contains("CR 115.1"), "{output}");

    let predicate = "fn predicate(q: &dyn Query, me: &Asker) -> bool { q.obj(me.source).counters(CounterKind::Charge) >= 7 }";
    for (constructor, rule) in [
        ("ability(TAP, action, \"Activate only if this creature has seven or more charge counters.\")", "602.5"),
        ("triggered(THIS_ENTERS, action, \"When this creature enters, if it has seven or more charge counters, you gain 2 life.\")", "603.4"),
    ] {
        let raw = format!("use cantrip_api::*; {predicate} fn action(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {{ g.gain_life(item.controller, 2); Ok(()) }} pub static CARD: CardDef = CardDef {{ abilities: &[{constructor}.condition(predicate)], ..CardDef::BLANK }};");
        let output = render(&raw, "Fixture", "An ability.", Characteristics::Oracle).unwrap();
        assert!(docs(&output, |name| name == "predicate")[0].contains(&format!("CR {rule}")), "{output}");
    }
}

#[test]
fn shared_only_cards_distinguish_spells_from_activated_mana_abilities() {
    for name in ["Boomerang", "Desert Twister"] {
        let found = docs(&modules()[name], |name| name == "CARD");
        assert_eq!(found.len(), 1);
        assert!(found[0].contains("CR 608.2"), "{name}: {}", found[0]);
    }
    for name in ["Princess Lucrezia", "Riven Turnbull", "Sunastian Falconer", "Apprentice Wizard"] {
        assert!(modules()[name].contains("CR 605.1a"), "{name}: {}", modules()[name]);
        assert!(!modules()[name].contains("CR 604.1"), "{name}: {}", modules()[name]);
    }
}

#[test]
fn replacement_callbacks_and_zone_actions_cite_their_own_rules() {
    for name in ["Reverberation", "Quarum Trench Gnomes", "Shimian Night Stalker"] {
        let found = docs(&modules()[name], |name| name.ends_with("_applies") || name.ends_with("_apply"));
        assert!(!found.is_empty());
        for doc in found { assert!(doc.contains("CR 614.1a"), "{name}: {doc}"); }
    }
    assert!(modules()["Recall"].contains("701.13a"));
    assert!(!modules()["Recall"].contains("406.3"));
    for name in ["Ebony Horse", "Hyperion Blacksmith", "Jandor's Saddlebags", "Reset"] {
        assert!(modules()[name].contains("701.26b"), "{name}: {}", modules()[name]);
        assert!(!modules()[name].contains("502.3"), "{name}: {}", modules()[name]);
    }
}
