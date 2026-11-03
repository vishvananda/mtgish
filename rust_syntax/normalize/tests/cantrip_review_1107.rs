#![cfg(feature = "cantrip")]
//! Regressions from the generated-definition comparisons in Cantrip PR #1107.
use mtgish_ast::cantrip;
use std::{collections::BTreeMap, path::PathBuf, sync::OnceLock};

fn modules() -> &'static BTreeMap<String, String> {
    static MODULES: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    MODULES.get_or_init(|| {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let output = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("avatar-review-1107.jsonl");
        cantrip::planned::tracked(&root.join("generator_review_1107.ronl"),
                                 &root.join("generator_review_1107.oracle.json"), &output).unwrap();
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
fn food_reminder_text_does_not_give_momo_a_food_activation() {
    let module = &modules()["Momo, Playful Pet"];
    assert!(module.contains("tokens::FOOD"));
    assert!(!module.contains("g.gain_life("));
    assert!(!module.contains("SACRIFICE_THIS"));
}

#[test]
fn targeted_optional_draw_has_a_worded_question_and_checks_the_player_first() {
    let (_, module) = modules().iter().find(|(name, _)| name.starts_with("The Legend of Yangchen")).unwrap();
    assert!(module.contains("have the targeted player draw 3 cards?"), "{module}");
    assert!(module.contains("Purpose::Ask(&PLAYER_DRAW_QUESTION_"), "{module}");
    let ask = module.find("effects::ask_may").unwrap();
    assert!(module[..ask].rfind(".is_some()").is_some(), "{module}");
    assert!(!module.contains("Purpose::Optional"));
    // Chapter I asks every player, not only opponents.
    assert!(module.contains("apnap()") && module.contains("rotate_left"));
}

#[test]
fn attack_alone_reuses_the_shared_trigger_and_keeps_its_subject() {
    let module = &modules()["Team Avatar"];
    assert!(module.contains("triggers::CREATURE_YOU_CONTROL_ATTACKS_ALONE"), "{module}");
    assert!(!module.contains("attackers.len()"));
    assert!(module.contains("item.subject.object()"));
}

#[test]
fn activation_during_your_turn_uses_the_existing_timing_rule() {
    let module = &modules()["Path to Redemption"];
    assert!(module.contains(".timing(Timing::OwnTurn)"), "{module}");
    assert!(!module.contains(".condition("));
}

#[test]
fn optional_sacrifice_does_not_keep_the_old_literal_true_scaffold() {
    let module = &modules()["Vengeful Villagers"];
    assert!(!module.contains("if true"));
    assert!(module.contains("can_sacrifice") || module.contains("Sacrifice"));
}

#[test]
fn single_card_library_pick_uses_the_existing_selection_recipe() {
    let module = &modules()["Master Piandao"];
    assert!(module.contains("library::look_at_top_select("), "{module}");
    assert!(!module.contains("library::look_at_top("));
    assert!(module.contains("Subtype::Ally") && module.contains("Subtype::Equipment") && module.contains("Subtype::Lesson"));
}
