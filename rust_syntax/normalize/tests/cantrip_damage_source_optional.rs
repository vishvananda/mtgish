#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, lower, mtg_types::OracleCard};

const ENCHANTED: &str = "battlefield-source-enchantment";
const OPTIONAL: &str = "optional-lasting-replacements";

fn card(line: &str) -> mtgish_ast::ir::Card {
    let source: OracleCard = ron::from_str(line).unwrap();
    lower::card(&source).unwrap()
}
fn named(fixture: &str, name: &str) -> mtgish_ast::ir::Card {
    card(fixture.lines().find(|l| l.starts_with(&format!("Card(Name: \"{name}\""))).unwrap())
}

#[test]
fn permanent_damage_source_enchantment_needs_the_battlefield_lki_query() {
    let wall = card(include_str!("fixtures/battlefield_damage_source_wall.ronl").trim());
    let old = cantrip::planned::without_features(&[ENCHANTED], || cantrip::module(&wall)).unwrap_err();
    assert!(old.feature.contains("last-known battlefield source enchantment"), "{old:?}");
    let new = cantrip::planned::with_additional_features(&[ENCHANTED], || cantrip::module(&wall)).unwrap();
    syn::parse_file(&new).unwrap();
    assert!(new.contains("q.battlefield_source_is_enchanted(source)"), "{new}");
    assert!(!new.contains("attached_to == Some(source)"), "{new}");
}

#[test]
fn optional_global_damage_replacement_needs_its_own_interface() {
    let blood = named(include_str!("fixtures/early_choice_regressions.ronl"), "Blood of the Martyr");
    let old = cantrip::planned::without_features(&[OPTIONAL], || cantrip::module(&blood)).unwrap_err();
    assert!(old.feature.contains("optional granted replacement needs a choice"), "{old:?}");
    let new = cantrip::planned::with_additional_features(&[OPTIONAL], || cantrip::module(&blood)).unwrap();
    syn::parse_file(&new).unwrap();
    assert!(new.contains(".optional(&INSTEAD_"), "{new}");
    assert!(new.contains("EffectKind::ReplacementRule"), "{new}");
    assert!(new.contains("g.add_player_effect(item.controller, item.object"), "{new}");
    assert!(new.contains("Some(asker.controller)"), "{new}");
    assert!(!new.contains("g.add_effect(item.object, about, EffectKind::Replacement"), "{new}");
}

#[test]
fn ordinary_enchanted_target_still_reads_live_attachments() {
    let source = r#"Card(Name: "Test", Typeline: (Supertypes: [], Cardtypes: [Instant], Subtypes: []), ManaCost: [ManaCostW], Rules: [SpellActions(Targeted([TargetPermanent(And([IsCardtype(Creature), IsEnchanted]))], ActionList([DestroyPermanent(Ref_TargetPermanent)])))])"#;
    let card = card(source);
    let code = cantrip::planned::with_additional_features(&[ENCHANTED], || cantrip::module(&card)).unwrap();
    assert!(code.contains("attached_to == Some("), "{code}");
    assert!(!code.contains("battlefield_source_is_enchanted"), "{code}");
}

#[test]
fn mandatory_replacements_do_not_gain_an_optional_choice() {
    let source = include_str!("fixtures/citation_roles.ronl");
    let card = named(source, "Reverberation");
    let old = cantrip::planned::without_features(&[OPTIONAL], || cantrip::module(&card)).unwrap();
    let new = cantrip::planned::with_additional_features(&[OPTIONAL], || cantrip::module(&card)).unwrap();
    assert_eq!(old, new);
    assert!(!new.contains(".optional(&INSTEAD_"), "{new}");
}

#[test]
fn optional_once_only_replacements_remain_held() {
    use mtgish_ast::ir::{Ability, Instruction, Operation, Program, ReplacementLimit};
    fn next(program: &mut Program) -> bool {
        for instruction in &mut program.instructions {
            match instruction {
                Instruction::Effect { operation: Operation::CreateReplacement { limit, .. }, .. } => {
                    *limit = ReplacementLimit::NextOccurrence;
                    return true;
                }
                Instruction::Group(group) => { if next(group) { return true; } }
                _ => {}
            }
        }
        false
    }
    let mut blood = named(include_str!("fixtures/early_choice_regressions.ronl"), "Blood of the Martyr");
    let Ability::Spell { body, .. } = &mut blood.abilities[0] else { panic!("spell") };
    assert!(next(body));
    let error = cantrip::planned::with_additional_features(&[OPTIONAL], || cantrip::module(&blood)).unwrap_err();
    assert!(error.feature.contains("optional granted replacement needs a choice"), "{error:?}");
}

#[test]
fn enchanted_damage_sources_outside_the_battlefield_view_remain_held() {
    fn change(value: &mut serde_json::Value) -> bool {
        match value {
            serde_json::Value::Object(map) => {
                if map.get("source_kind").and_then(|v| v.as_str()) == Some("Permanent") {
                    map.insert("source_kind".into(), "Card".into());
                    true
                } else { map.values_mut().any(change) }
            }
            serde_json::Value::Array(items) => items.iter_mut().any(change),
            _ => false,
        }
    }
    let wall = card(include_str!("fixtures/battlefield_damage_source_wall.ronl").trim());
    let mut value = serde_json::to_value(wall).unwrap();
    assert!(change(&mut value));
    let altered = serde_json::from_value(value).unwrap();
    let error = cantrip::planned::with_additional_features(&[ENCHANTED], || cantrip::module(&altered)).unwrap_err();
    assert!(error.feature.contains("outside battlefield"), "{error:?}");
}
