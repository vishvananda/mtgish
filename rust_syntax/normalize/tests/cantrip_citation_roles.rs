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
fn enters_tapped_has_its_own_rule_and_combined_entry_replacements_cite_both() {
    for name in ["Dwarven Ruins", "Ebon Stronghold"] {
        let found = docs(&modules()[name], |name| name.ends_with("_enters"));
        assert_eq!(found.len(), 1, "{name}");
        assert!(found[0].contains("614.1d"), "{name}: {}", found[0]);
        assert!(!found[0].contains("614.1c"), "{name}: {}", found[0]);
    }
    let bog = docs(&modules()["Peat Bog"], |name| name.ends_with("_enters"));
    assert_eq!(bog.len(), 1);
    assert!(bog[0].contains("614.1c, 614.1d"), "{}", bog[0]);
    let depths = docs(&modules()["Dark Depths"], |name| name.ends_with("_enters"));
    assert_eq!(depths.len(), 1);
    assert!(depths[0].contains("614.1c"), "{}", depths[0]);
    assert!(!depths[0].contains("614.1d"), "{}", depths[0]);
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
fn attack_and_resolving_cost_predicates_do_not_claim_to_be_spell_casting_costs() {
    for name in ["Flooded Woodlands", "Leviathan"] {
        let found = docs(&modules()[name], |name| name.contains("rule_cost") || name.starts_with("paid_"));
        assert!(!found.is_empty(), "{name}: {}", modules()[name]);
        for doc in found {
            assert!(doc.contains("CR 118.1"), "{name}: {doc}");
            assert!(!doc.contains("601.2h"), "{name}: {doc}");
        }
    }
}

#[test]
fn granted_static_callbacks_cite_static_abilities() {
    let found = docs(&modules()["Glyph of Delusion"], |name| name.ends_with("_keyword"));
    assert_eq!(found.len(), 1);
    assert!(found[0].contains("CR 604.1"), "{}", found[0]);
    assert!(!found[0].contains("608.2"), "{}", found[0]);
    // The granted upkeep effect still resolves, and must retain its own role.
    let resolving = docs(&modules()["Glyph of Delusion"], |name| name == "granted_3");
    assert_eq!(resolving.len(), 1);
    assert!(resolving[0].contains("CR 608.2"), "{}", resolving[0]);
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
fn duration_conditions_and_open_set_rules_have_different_rules() {
    use cantrip::pr_form::{render, Characteristics};
    let raw = r#"
use cantrip_api::*;
fn predicate(q: &dyn Query, e: &ContinuousEffect) -> bool { q.obj(e.source).tapped }
fn unrelated_event(q: &dyn Query, _: &Asker, _: Target, id: ObjectId) -> bool { q.chars(id).is(CardType::Creature) }
pub static CARD: CardDef = CardDef {
    lasting: &[Lasting { text: "{source} remains tapped", holds: predicate }],
    lasting_rules: &[LastingRule { text: "Creatures can't block.", rule: LastingRuleKind::CantBlock, applies: unrelated_event }],
    ..CardDef::BLANK
};
"#;
    let output = render(raw, "Fixture", "For as long as this artifact remains tapped.", Characteristics::Oracle).unwrap();
    let condition = docs(&output, |name| name == "predicate");
    assert_eq!(condition.len(), 1);
    assert!(condition[0].contains("CR 611.2b"), "{output}");
    assert!(!condition[0].contains("608.2"), "{output}");
    let rule = docs(&output, |name| name == "unrelated_event");
    assert_eq!(rule.len(), 1);
    assert!(rule[0].contains("CR 611.2c"), "{output}");
    assert!(!rule[0].contains("603.2"), "{output}");
}

#[test]
fn permanent_and_card_cost_filters_cite_cost_payment() {
    use cantrip::pr_form::{render, Characteristics};
    for constructor in ["tap_untapped", "return_to_hand", "return_tapped_to_hand", "exile_permanents", "exile_from_hand", "exile_from_graveyard_of", "sacrifice"] {
        let raw = format!(r#"
use cantrip_api::*;
fn predicate(q: &dyn Query, asker: &Asker, id: ObjectId) -> bool {{ q.controller(id) == asker.controller }}
fn action(_: &mut dyn Game, _: &StackItem) -> Result<(), Suspend> {{ Ok(()) }}
pub static CARD: CardDef = CardDef {{ abilities: &[ability({constructor}(1, predicate), action, "An activated ability.")], ..CardDef::BLANK }};
"#);
        let output = render(&raw, "Fixture", "An activated ability.", Characteristics::Oracle).unwrap();
        let filter = docs(&output, |name| name == "predicate");
        assert_eq!(filter.len(), 1, "{constructor}: {output}");
        assert!(filter[0].contains("CR 118.1"), "{constructor}: {output}");
        assert!(!filter[0].contains("601.2h"), "{constructor}: {output}");
        assert!(!filter[0].contains("608.2"), "{constructor}: {output}");
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

#[test]
fn x_announcement_callbacks_follow_casting_and_activation_roles() {
    let found = docs(&modules()["Winter's Chill"], |name| name.ends_with("_x_max"));
    assert_eq!(found.len(), 1);
    assert!(found[0].contains("CR 601.2b"), "{}", found[0]);
    assert!(!found[0].contains("608.2"), "{}", found[0]);
    use cantrip::pr_form::{render, Characteristics};
    // Arbitrary names and identical signatures must still get the API's role.
    let functions = "fn spell_bound(_: &dyn Query, _: &Asker, _: &[Target]) -> u32 { 2 } fn activation_bound(_: &dyn Query, _: &Asker, _: &[Target]) -> u32 { 2 } fn resolve(_: &mut dyn Game, _: &StackItem) -> Result<(), Suspend> { Ok(()) }";
    let raw = format!("use cantrip_api::*; {functions} pub static CARD: CardDef = CardDef {{ x_rule: Some(XRule::AtMost(spell_bound)), abilities: &[ability(cost(\"{{X}}\"), resolve, \"Pay X.\").x_at_most(activation_bound)], ..CardDef::BLANK }};");
    let output = render(&raw, "Fixture", "Do something.", Characteristics::Oracle).unwrap();
    for (name, rule) in [("spell_bound", "601.2b"), ("activation_bound", "602.2b"), ("resolve", "608.2")] {
        let found = docs(&output, |candidate| candidate == name);
        assert_eq!(found.len(), 1, "{output}");
        assert!(found[0].contains(rule), "{name}: {}", found[0]);
    }
}
