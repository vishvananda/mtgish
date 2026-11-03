#![cfg(feature = "cantrip")]
//! Cards of several faces, each file as the tracker writes it, saying what Cantrip's own card says: the forms their
//! scenarios turned up.
use mtgish_ast::cantrip::planned;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::OnceLock;

const NAMES: &[&str] = &[
    "Ajani, Nacatl Pariah",
    "Anaconda",
    "Arc of Fortune",
    "Barracks of the Thousand",
    "Black Panther, Hope Enduring",
    "Brigid, Clachan's Heart",
    "Call to the Kindred",
    "Caravan Escort",
    "Cease",
    "Delver of Secrets",
    "Eiganjo Dynastorian",
    "Fable of the Mirror-Breaker",
    "Great Fierce Bee",
    "Grove's Bounty",
    "Hada Spy Patrol",
    "Hearth Elemental",
    "Hexdrinker",
    "Jade Seedstones",
    "Joraga Treespeaker",
    "Lord of Shatterskull Pass",
    "Revitalize",
    "Selective Adaptation",
    "Sink into Stupor",
    "Stature, Size Shifter",
    "Temple of Power",
    "The Restoration of Eiganjo",
    "Unlicensed Hearse",
    "Valakut Awakening",
    "Vengeful Townsfolk",
    "Watertight Gondola",
    "Wedding Announcement",
];

/// The records of `NAMES` and the Oracle file. None without the data.
fn subset() -> Option<&'static (PathBuf, PathBuf)> {
    static SUBSET: OnceLock<Option<(PathBuf, PathBuf)>> = OnceLock::new();
    SUBSET
        .get_or_init(|| {
            let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
            let (corpus, oracle) = (data.join("mtgish.lines.ron"), data.join("oracle.json"));
            if !corpus.exists() || !oracle.exists() {
                return None;
            }
            let subset = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("faces.ron");
            let lines: Vec<String> = std::fs::read_to_string(&corpus)
                .unwrap()
                .lines()
                .filter(|line| NAMES.iter().any(|name| line.contains(&format!("Name: \"{name}\""))))
                .map(str::to_string)
                .collect();
            std::fs::write(&subset, lines.join("\n") + "\n").unwrap();
            Some((subset, oracle))
        })
        .as_ref()
}

/// Each card's row, by every name it has: its file and its gaps' features. None without the data.
fn rows() -> Option<&'static BTreeMap<String, (String, Vec<String>)>> {
    static ROWS: OnceLock<Option<BTreeMap<String, (String, Vec<String>)>>> = OnceLock::new();
    ROWS.get_or_init(|| {
        let (subset, oracle) = subset()?;
        let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("faces.jsonl");
        planned::tracked(subset, oracle, &out).unwrap();
        let mut rows = BTreeMap::new();
        for line in std::fs::read_to_string(&out).unwrap().lines() {
            let row: serde_json::Value = serde_json::from_str(line).unwrap();
            let file = row["file"].as_str().unwrap_or("").to_string();
            let gaps: Vec<String> = row["faces"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|face| face["gaps"].as_array().unwrap().iter())
                .map(|gap| gap["feature"].as_str().unwrap().to_string())
                .collect();
            for name in row["name"].as_str().unwrap().split(" // ") {
                rows.insert(name.to_string(), (file.clone(), gaps.clone()));
            }
        }
        Some(rows)
    })
    .as_ref()
}

/// The file of a card the tracker writes whole.
fn file(name: &str) -> Option<&'static str> {
    let (file, gaps) = rows()?.get(name).unwrap_or_else(|| panic!("no row for {name}"));
    assert!(gaps.is_empty(), "{name} has gaps {gaps:?}");
    Some(file)
}

fn says(name: &str, code: &[&str]) {
    let Some(file) = file(name) else { return };
    for code in code {
        assert!(file.contains(code), "{name} should say {code}:\n{file}");
    }
}

#[test]
fn up_to_two_cards_from_a_single_graveyard_are_two_slots_of_one_graveyard() {
    for name in ["Cease", "Unlicensed Hearse"] {
        says(name, &["same_graveyard(", "same_graveyard).optional()"]);
    }
}

#[test]
fn x_mana_of_one_color_is_that_color_chosen_on_activation() {
    says("Brigid, Clachan's Heart", &["choose_color().colors(ColorSet(17))"]);
}

#[test]
fn mana_produced_by_this_is_told_apart_from_other_mana() {
    says(
        "Barracks of the Thousand",
        &["fn its_own_mana(_: &dyn Query, asker: &Asker, source: ObjectId) -> bool", "its_own_mana]"],
    );
}

#[test]
fn a_prevention_is_hinted_as_what_it_does() {
    says(
        "Black Panther, Hope Enduring",
        &["\"prevents all damage that would be dealt to Black Panther\""],
    );
}

#[test]
fn counters_distributed_among_any_number_of_targets_may_have_none() {
    says("Grove's Bounty", &["between(0, TargetSpec::COUNTED)"]);
}

#[test]
fn the_cards_used_to_craft_a_permanent_are_crafts() {
    says("Jade Seedstones", &["craft::exiled_cards_used_to_craft(q, o).contains(&id)"]);
}

#[test]
fn a_trigger_reading_its_subject_is_not_a_shared_one() {
    let Some(file) = file("Jade Seedstones") else { return };
    let front = file.split("pub mod back").next().unwrap();
    assert!(front.contains("Fired::about(subject)"), "{file}");
}

#[test]
fn damage_this_turn_is_of_its_source_as_it_dealt_it() {
    says("Temple of Power", &["let dealt = DamageDealt {"]);
}

#[test]
fn creatures_attacked_with_are_counted_once_each() {
    says("Wedding Announcement", &["attackers.dedup()"]);
}

#[test]
fn a_player_who_cant_draw_isnt_offered_a_may_that_draws() {
    says("Arc of Fortune", &["cant_draw(g, bound_0, 7)"]);
}

#[test]
fn cards_put_on_the_bottom_in_any_order_have_their_order_asked() {
    says("Valakut Awakening", &["library::ask_library_order("]);
}

#[test]
fn a_reveal_of_the_top_card_is_not_offered_from_an_empty_library() {
    says("Delver of Secrets", &[".is_empty()\n        && effects::ask_may("]);
}

#[test]
fn cant_be_blocked_as_long_as_is_the_keyword_while_it_holds() {
    says(
        "Watertight Gondola",
        &["chars.keywords = chars.keywords.with(Keyword::CantBeBlocked);"],
    );
    // Power is of a later layer than the keyword's, and a level's lines are its stage's: both stay rules.
    for name in ["Stature, Size Shifter", "Hada Spy Patrol"] {
        let Some(file) = file(name) else { return };
        assert!(file.contains("block_rules: &["), "{name}:\n{file}");
        assert!(!file.contains("Keyword::CantBeBlocked);"), "{name}:\n{file}");
    }
}

#[test]
fn the_data_s_repaired_reads_are_written_whole() {
    // "Whenever one or more other ... die" is a batch in the data again (#177): it triggers once (CR 603.2c).
    for name in ["Ajani, Nacatl Pariah", "Great Fierce Bee", "Vengeful Townsfolk"] {
        says(name, &[".once_per_batch()"]);
    }
    // "Target spell or nonland permanent an opponent controls" (#178): the permanent is an opponent's too.
    let Some(file) = file("Sink into Stupor") else { return };
    assert_eq!(file.matches("opponents(asker.controller)").count(), 2, "{file}");
}

#[test]
fn the_lane_s_rows_are_the_tracker_s_files_of_every_layout() {
    let (Some((subset, oracle)), Some(rows)) = (subset(), rows()) else { return };
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("faces-lane.jsonl");
    planned::lane_rows(subset, oracle, &out, None).unwrap();
    let mut seen = BTreeMap::new();
    for line in std::fs::read_to_string(&out).unwrap().lines() {
        let row: serde_json::Value = serde_json::from_str(line).unwrap();
        let name = row["name"].as_str().unwrap().to_string();
        let faces: Vec<&str> = row["faces"].as_array().unwrap().iter().map(|f| f.as_str().unwrap()).collect();
        // The faces in the name's order, the front face first.
        assert_eq!(faces.join(" // "), name, "{row}");
        let (file, _) = &rows[faces[0]];
        assert_eq!(row["rust"].as_str().unwrap_or(""), file, "{name}");
        seen.insert(name, (row["layout"].as_str().unwrap().to_string(), row["rust"].is_string(), row["gap"].clone()));
    }
    // A card of several faces is written whole, its layout named.
    let fable = &seen["Fable of the Mirror-Breaker // Reflection of Kiki-Jiki"];
    assert_eq!((fable.0.as_str(), fable.1, fable.2.is_null()), ("Transforming", true, true));
    // A card whose content is all of it has neither a file nor a gap.
    let anaconda = &seen["Anaconda"];
    assert_eq!((anaconda.0.as_str(), anaconda.1, anaconda.2.is_null()), ("Card", false, true));
}

#[test]
fn one_permanent_may_be_chosen_for_each_of_its_types() {
    // Ajani's -4, like Cataclysm: a permanent of two types may be the choice for both, as their rulings say.
    let Some(file) = file("Ajani, Nacatl Pariah") else { return };
    assert!(file.contains("picked_") && !file.contains(".filter(|id| !picked_"), "{file}");
    // A card of each color, or a party, is of distinct cards: one already chosen is out (CR 700.8).
    says("Selective Adaptation", &[".filter(|id| !picked_"]);
}

#[test]
fn a_may_of_up_to_some_cards_is_asked_as_the_choice_of_none() {
    // Fable's chapter II asks once: choosing no card is declining.
    let Some(file) = file("Fable of the Mirror-Breaker") else { return };
    let ii = file.split("fn ii_discard_up_two_cards(").nth(1).unwrap().split("\n}\n").next().unwrap();
    assert!(ii.contains("select::Count::UpTo(2)") && !ii.contains("ask_may"), "{ii}");
    // Having looked first, the may stays: choosing none would still have looked.
    says("Call to the Kindred", &["effects::ask_may(g, item.controller, item.object, Purpose::Optional)?"]);
}

#[test]
fn a_chapter_is_named_and_quoted_as_printed() {
    says(
        "Fable of the Mirror-Breaker",
        &[
            "/// \"II — You may discard up to two cards. If you do, draw that many cards.\"",
            "fn ii_discard_up_two_cards(",
            "fn iii_exile_saga_return_battlefield(",
        ],
    );
}

#[test]
fn a_cost_reduction_is_counted_in_u32() {
    says("Hearth Elemental", &["CostDelta::less(", ".len() as i64) as u32,"]);
}

#[test]
fn a_face_named_as_another_card_is_quoted_as_its_own_card_prints_it() {
    // Eiganjo Dynastorian's prepare spell is Replenish, whose card of its own has reminder text the spell does not.
    let Some(file) = file("Eiganjo Dynastorian") else { return };
    assert!(file.contains("/// \"Return all enchantment cards from your graveyard to the battlefield.\" (CR"), "{file}");
    assert!(!file.contains("Auras with nothing to enchant"), "{file}");
}

#[test]
fn gaining_life_cites_the_rule_of_gaining_life() {
    says("Revitalize", &["(CR 608.2, 121.1, 119.3)."]);
}

#[test]
fn a_level_s_lines_are_quoted_with_their_level() {
    // As Cantrip's own Hexdrinker: each static of a level says its level, and its power and toughness its numbers.
    says(
        "Caravan Escort",
        &["\"LEVEL 1-4: 2/2\"", "\"LEVEL 5+: 5/5\"", "\"LEVEL 5+: First strike\""],
    );
    says(
        "Hexdrinker",
        &[
            "\"LEVEL 3-7: 4/4\"",
            "\"LEVEL 3-7: Protection from instants\"",
            "\"LEVEL 8+: 6/6\"",
            "\"LEVEL 8+: Protection from everything\"",
        ],
    );
    let Some(file) = file("Caravan Escort") else { return };
    assert!(!file.contains("\"LEVEL 1-4\"") && !file.contains("Level up {2} ("), "{file}");
}

#[test]
fn a_level_s_granted_ability_is_said_without_its_level_and_granted_with_it() {
    // As Adagia's "12+ | {3}{W}, {T}: ..." static and its ability.
    let line = "Whenever this creature attacks, it deals 6 damage to each creature defending player controls.";
    says("Lord of Shatterskull Pass", &[&format!("\"LEVEL 6+: {line}\""), &format!("\"{line}\"")]);
    // What a level's line quotes is the text of what it grants.
    let Some(file) = file("Joraga Treespeaker") else { return };
    assert_eq!(file.matches("\"{T}: Add {G}{G}.\")").count(), 2, "{file}");
    assert_eq!(file.matches("            \"LEVEL 5+: 1/4\",").count(), 1, "{file}");
}

#[test]
fn restoration_uses_the_same_plain_spirit_for_attacking_and_blocking() {
    let Some(file) = file("The Restoration of Eiganjo") else { return };
    assert!(!file.contains("pub static SPIRIT"), "{file}");
    assert_eq!(file.matches("g.card_id(&cantrip_mechanics::tokens::SPIRIT_1_1_COLORLESS)").count(), 1, "{file}");
    assert!(file.contains("triggers::THIS_ATTACKS_OR_BLOCKS"), "{file}");
}
