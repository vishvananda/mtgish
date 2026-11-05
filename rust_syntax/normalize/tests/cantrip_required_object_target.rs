#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, lower, mtg_types::OracleCard};

#[test]
fn sole_required_object_target_does_not_emit_impossible_empty_branch() {
    let corpus = include_str!("fixtures/required_object_target.ronl");
    for name in ["Twiddle", "Elder Druid", "Vesuvan Doppelganger"] {
        let prefix = format!("Card(Name: {name:?}");
        let line = corpus.lines().find(|line| line.starts_with(&prefix)).unwrap();
        let source: OracleCard = ron::from_str(line).unwrap();
        let raw = cantrip::module(&lower::card(&source).unwrap()).unwrap();
        let text = match name {
            "Twiddle" => "You may tap or untap target artifact, creature, or land.",
            "Elder Druid" => "{3}{G}, {T}: You may tap or untap target artifact, creature, or land.",
            _ => "You may have this creature enter as a copy of any creature on the battlefield, except it doesn't copy that creature's color and it has \\\"At the beginning of your upkeep, you may have this creature become a copy of target creature, except it doesn't copy that creature's color and it has this ability.\\\"",
        };
        let printed = cantrip::pr_form::render(&raw, name, text, cantrip::pr_form::Characteristics::Inline).unwrap();
        assert!(!printed.contains("if let Some(object)"), "{name}: {printed}");
        if name == "Vesuvan Doppelganger" { assert!(!printed.contains("if let Some(values)"), "{name}: {printed}"); }
        assert!(!printed.contains(".expect("), "{name}: {printed}");
        assert!(!printed.contains(".unwrap("), "{name}: {printed}");
        if name != "Vesuvan Doppelganger" {
            assert!(printed.contains("choose::may_tap_or_untap_target(g, item, card)"), "{name}: {printed}");
            assert!(printed.contains("labels: choose::TAP_OR_UNTAP"), "{name}: {printed}");
            assert!(!printed.contains("choose::one_of("), "{name}: {printed}");
        }
    }
}

#[test]
fn optional_and_multimode_targets_keep_their_guards() {
    let corpus = include_str!("fixtures/required_object_target.ronl");
    let line = corpus.lines().find(|line| line.starts_with("Card(Name: \"Twiddle\"")).unwrap();
    let source: OracleCard = ron::from_str(line).unwrap();
    let raw = cantrip::module(&lower::card(&source).unwrap()).unwrap();
    let optional = raw.replace("target(ability_0_target_0)]", "target(ability_0_target_0).between(0, 1)]");
    assert_ne!(optional, raw);
    let printed = cantrip::pr_form::render(&optional, "Twiddle", "You may tap or untap target artifact, creature, or land.", cantrip::pr_form::Characteristics::Inline).unwrap();
    assert!(printed.contains("if let Some(object)"), "{printed}");
    let player = raw.replace("Target::Object(id) if cantrip_mechanics::targets::permanent(q, id)", "Target::Player(id) if player_filter(q, id)");
    assert_ne!(player, raw);
    let printed = cantrip::pr_form::render(&player, "Twiddle", "You may tap or untap target artifact, creature, or land.", cantrip::pr_form::Characteristics::Inline).unwrap();
    assert!(printed.contains("if let Some(object)"), "{printed}");
    let multimodal = format!("{raw}\nstatic OTHER: () = dummy().choose(1, 2);\n");
    let printed = cantrip::pr_form::render(&multimodal, "Twiddle", "You may tap or untap target artifact, creature, or land.", cantrip::pr_form::Characteristics::Inline).unwrap();
    assert!(printed.contains("if let Some(object)"), "{printed}");
    for mechanic in ["bestow", "mutate", "arcane"] {
        let alternate_resolution = format!("{raw}\nstatic OTHER: () = {mechanic}();\n");
        let printed = cantrip::pr_form::render(&alternate_resolution, "Twiddle", "You may tap or untap target artifact, creature, or land.", cantrip::pr_form::Characteristics::Inline).unwrap();
        assert!(printed.contains("if let Some(object)"), "{mechanic}: {printed}");
    }
}
