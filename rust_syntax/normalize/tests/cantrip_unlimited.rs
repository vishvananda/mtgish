#![cfg(feature = "cantrip")]
//! Unlimited gaps expressible with the existing Cantrip API.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf};

fn modules() -> BTreeMap<String, String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let names = ["Chaoslace", "Deathlace", "Lifelace", "Purelace", "Thoughtlace", "Siren's Call", "Nettling Imp", "Earthbind", "Kudzu", "Lich", "Volcanic Eruption", "Pirate Ship", "Sea Serpent", "Twiddle"]
        .into_iter().map(str::to_string).collect::<Vec<_>>();
    cantrip::modules_printed(&root.join("unlimited_generator.ronl"), &root.join("unlimited_generator.oracle.json"), &names)
        .unwrap().into_iter().collect()
}

#[test]
fn before_attackers_checks_the_first_such_step_in_the_turn() {
    let modules=modules();
    for name in ["Siren's Call", "Nettling Imp"] {
        let module=&modules[name];
        assert!(module.contains("history().this_turn().iter().any"),"{name}: {module}");
        assert!(module.contains("Event::StepBegan { step: Step::DeclareAttackers }"),"{name}: {module}");
    }
}

#[test]
fn defending_player_restrictions_include_a_battles_protector() {
    let modules=modules();
    for name in ["Pirate Ship","Sea Serpent"] {
        assert!(modules[name].contains("defending_player_of(q.state(), defender)"),"{name}: {}",modules[name]);
    }
}

#[test]
fn untapping_by_a_spell_cites_the_untap_action() {
    let fixtures=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let output=PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("unlimited-untap-citation.jsonl");
    cantrip::planned::tracked(&fixtures.join("unlimited_generator.ronl"),
                             &fixtures.join("unlimited_generator.oracle.json"), &output).unwrap();
    let rows=std::fs::read_to_string(output).unwrap();
    let row=rows.lines().map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .find(|row| row["faces"].as_array().unwrap().iter().any(|face| face["name"]=="Twiddle")).unwrap();
    let file=row["file"].as_str().unwrap();
    assert!(file.contains("701.26b") && !file.contains("502.3"),"{file}");
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
    assert!(module.contains("q.controller(item.subject.object().expect(\"event matcher guarantees object subject\"))"), "{module}");
    let printed = cantrip::pr_form::render(module, "Kudzu", "Enchant land. When enchanted land becomes tapped, destroy it. That land's controller may attach this Aura to a land of their choice.", cantrip::pr_form::Characteristics::Inline).unwrap();
    assert!(printed.contains("event matcher guarantees object subject"), "{printed}");
    assert!(!printed.contains("let Some(named_1) = item.subject.object()"), "{printed}");
    if let Ok(path) = std::env::var("CANTRIP_KUDZU_PRINTED") { std::fs::write(path, printed).unwrap(); }
}

#[test]
fn lich_requires_the_full_number_of_sacrifices() {
    let module = &modules()["Lich"];
    assert!(module.contains("picks_2.len() as u32 == owed_2"), "{module}");
    assert!(!module.contains("let paid_2 = !picks_2.is_empty()"), "{module}");
    assert!(!module.contains("Count::Exactly(n) |"), "{module}");
}

#[test]
fn volcanic_eruption_counts_only_actual_moves_to_a_graveyard() {
    let module = &modules()["Volcanic Eruption"];
    assert!(module.contains("old == o && to.is(ZoneKind::Graveyard)"), "{module}");
}
