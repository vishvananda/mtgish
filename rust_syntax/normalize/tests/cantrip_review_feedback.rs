#![cfg(feature = "cantrip")]
//! Review findings verified against fresh output, including rejected recommendations.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf, sync::OnceLock};

fn modules() -> &'static BTreeMap<String, String> {
    static MODULES: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    MODULES.get_or_init(|| {
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let output = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("generator-review-feedback.jsonl");
        cantrip::planned::tracked(&fixtures.join("generator_review_feedback.ronl"),
                                 &fixtures.join("generator_review_feedback.oracle.json"), &output).unwrap();
        let mut files = BTreeMap::new();
        for line in std::fs::read_to_string(output).unwrap().lines() {
            let row: serde_json::Value = serde_json::from_str(line).unwrap();
            let file = row["file"].as_str().unwrap_or_else(|| panic!("incomplete card: {row}"));
            for face in row["faces"].as_array().unwrap() {
                files.insert(face["name"].as_str().unwrap().to_string(), file.to_owned());
            }
        }
        files
    })
}

#[test]
fn damage_counter_amounts_preserve_the_signed_counter_bound() {
    let file = &modules()["Living Artifact"];
    assert!(file.contains("i16::MAX"), "{file}");
    assert!(!file.contains("i64::from(item.data[0]) as i16;"), "{file}");
}

#[test]
fn colored_protection_is_a_text_aware_removable_static_ability() {
    for name in ["Mountain Yeti", "Black Knight", "White Knight"] {
        let file = &modules()[name];
        assert!(file.contains("protection::from::<"), "{name}: {file}");
        assert!(!file.contains("protection: ColorSet("), "{name}: {file}");
    }
}

#[test]
fn entry_choices_use_the_entering_controller_before_the_object_enters() {
    let file = &modules()["Jihad"];
    let enters = file.split("pub static CARD").next().unwrap().split("entering.chosen_player").next().unwrap();
    assert!(!enters.contains("q.controller("), "{file}");
    assert!(enters.contains("opponents(entering.controller)"), "{file}");
}

#[test]
fn chosen_values_keep_the_distinct_query_and_layer_link_interfaces() {
    let file = &modules()["Jihad"];
    assert!(file.contains("linked_to(asker)"), "{file}");
    assert!(file.contains("linked_to(asker.source)"), "{file}");
}

#[test]
fn a_plain_damage_ability_uses_the_existing_single_target_recipe() {
    let file = &modules()["Prodigal Sorcerer"];
    assert!(file.contains("effects::damage_first_target::<1>"), "{file}");
}

#[test]
fn regeneration_uses_the_keyword_recipe_for_source_and_target() {
    let source = &modules()["Drudge Skeletons"];
    let target = &modules()["Death Ward"];
    assert!(source.contains("regenerate::this(g, item)"), "{source}");
    assert!(target.contains("regenerate::target(g, item, 0)"), "{target}");
    assert!(!source.contains("RegenerationShield") && !target.contains("RegenerationShield"));
}

#[test]
fn common_enchant_and_creature_target_filters_are_shared() {
    for (name, target) in [("Living Artifact", "ARTIFACT"), ("Death Ward", "CREATURE"), ("Fear", "CREATURE")] {
        let file = &modules()[name];
        assert!(file.contains(&format!("targets::{target}")), "{name}: {file}");
    }
}

#[test]
fn graveyard_cards_already_reject_tokens_and_keep_the_requested_types() {
    for name in ["Raise Dead", "Regrowth"] {
        let file = &modules()[name];
        assert!(file.contains("!q.state().obj(id).token") || file.contains("! q.state().obj(id).token"), "{file}");
        assert!(file.contains("!q.state().obj(id).card_copy") || file.contains("! q.state().obj(id).card_copy"), "{file}");
    }
    assert!(modules()["Raise Dead"].contains("CardType::Creature"));
}

#[test]
fn optional_resolution_costs_keep_their_payment_semantics() {
    // CR 118.12: "you may ... if you do" is a cost even though paid during resolution.
    let file = &modules()["Living Artifact"];
    assert!(file.contains(".may_pay("), "{file}");
    assert!(file.contains("remove_counters(CounterKind::Vitality, 1)"), "{file}");
}

#[test]
fn sharing_does_not_remove_existing_saturating_stat_arithmetic() {
    for name in ["Holy Strength", "Weakness"] {
        let file = &modules()[name];
        assert!(file.contains("saturating_add") || file.contains("plus_pt_saturating"), "{name}: {file}");
        assert!(!file.contains("statics::plus_pt::<"), "{name}: {file}");
    }
}

#[test]
fn upkeep_of_an_enchanted_objects_controller_keeps_object_and_player() {
    for name in ["Wanderlust", "Super Intelligence", "Feedback", "Paralyze", "Cursed Land", "Warp Artifact"] {
        let file = &modules()[name];
        assert!(file.contains("Target::Object") && file.contains("u32::from(q.state().turn.active.0)"), "{name}: {file}");
        assert!(!file.contains("let subject = Target::Player(q.state().turn.active)"), "{name}: {file}");
    }
}

#[test]
fn drawing_for_a_recorded_upkeep_player_requires_them_to_remain_in_game() {
    let file = &modules()["Super Intelligence"];
    assert!(file.contains(".in_game("), "{file}");
    assert!(file.contains("item.data[0]"), "{file}");
}

#[test]
fn paralyze_keeps_the_referenced_creature_and_player_after_attachment_changes() {
    let file = &modules()["Paralyze"];
    assert!(file.contains("Some(PlayerId(item.data[0] as u8))"), "{file}");
    assert!(file.split_whitespace().collect::<String>().contains("item.subject.object()"), "{file}");
}
