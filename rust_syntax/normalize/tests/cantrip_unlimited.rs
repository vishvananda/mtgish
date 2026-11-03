#![cfg(feature = "cantrip")]
//! Unlimited gaps expressible with the existing Cantrip API.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

fn modules() -> BTreeMap<String, String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let names = ["Chaoslace", "Deathlace", "Lifelace", "Purelace", "Thoughtlace", "Siren's Call", "Nettling Imp", "Earthbind", "Kudzu", "Lich", "Volcanic Eruption"]
        .into_iter().map(str::to_string).collect::<Vec<_>>();
    cantrip::modules_printed(&root.join("unlimited_generator.ronl"), &root.join("unlimited_generator.oracle.json"), &names)
        .unwrap().into_iter().collect()
}

#[test]
fn laces_change_spells_with_the_existing_layered_effect_api() {
    let modules = modules();
    for name in ["Chaoslace", "Deathlace", "Lifelace", "Purelace", "Thoughtlace"] {
        let module = &modules[name];
        assert!(!module.contains("planned"), "{name}: {module}");
        assert!(!module.contains("spell_changes::"), "{name}: {module}");
        assert!(module.contains("EffectKind::SetColors("), "{name}: {module}");
        assert!(module.contains("Duration::Indefinite"), "{name}: {module}");
    }
}

#[test]
fn sirens_call_uses_lasting_rules_and_keeps_each_creatures_history_separate() {
    let modules = cantrip::planned::with_features(&["lasting-rules"], modules);
    let module = &modules["Siren's Call"];
    assert!(!module.contains("planned"), "{module}");
    assert!(module.contains("LastingRuleKind::MustAttack"), "{module}");
    assert!(module.contains("let history_candidate = id;"), "{module}");
    assert!(!module.contains("Some(id) == Some(id)"), "{module}");
}

#[test]
fn nettling_imp_captures_the_same_target_once_for_testing_and_destroying() {
    let modules = modules();
    let module = &modules["Nettling Imp"];
    assert!(!module.contains("planned"), "{module}");
    assert!(module.contains("g.delayed_trigger_about("), "{module}");
    assert!(module.contains("about(q, item, 0)"), "{module}");
    assert!(!module.contains("about(q, item, 1)"), "{module}");
}

#[test]
fn earthbind_grants_an_existing_static_ability_definition() {
    let module = &modules()["Earthbind"];
    assert!(!module.contains("planned"), "{module}");
    assert!(module.contains("granted_static_ability(Layer::Ability"), "{module}");
    assert!(module.contains("EffectKind::AddAbility(AbilityRef::delayed("), "{module}");
}

#[test]
fn kudzu_remembers_the_land_that_triggered_it() {
    let module = &modules()["Kudzu"];
    assert!(module.contains("let objects_0 = { let q: &dyn Query = g; item.subject.object()"), "{module}");
    assert!(module.contains("item.subject.object().map(|o: ObjectId| q.controller(o))"), "{module}");
}

#[test]
fn lich_requires_the_full_number_of_sacrifices() {
    let module = &modules()["Lich"];
    assert!(module.contains("picks_2.len() as u32 == owed_2"), "{module}");
    assert!(!module.contains("let paid_2 = !picks_2.is_empty()"), "{module}");
}

#[test]
fn volcanic_eruption_counts_only_actual_moves_to_a_graveyard() {
    let module = &modules()["Volcanic Eruption"];
    assert!(module.contains("old == o && to.is(ZoneKind::Graveyard)"), "{module}");
}
