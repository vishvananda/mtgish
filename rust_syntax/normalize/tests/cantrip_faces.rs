#![cfg(feature = "cantrip")]
//! Cards of several faces, each file as the tracker writes it, saying what Cantrip's own card says: the forms their
//! scenarios turned up.
use mtgish_ast::cantrip::planned;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::OnceLock;

const NAMES: &[&str] = &[
    "Ajani, Nacatl Pariah",
    "Arc of Fortune",
    "Barracks of the Thousand",
    "Black Panther, Hope Enduring",
    "Brigid, Clachan's Heart",
    "Cease",
    "Delver of Secrets",
    "Grove's Bounty",
    "Jade Seedstones",
    "Sink into Stupor",
    "Temple of Power",
    "Unlicensed Hearse",
    "Valakut Awakening",
    "Watertight Gondola",
    "Wedding Announcement",
];

/// Each card's row, by every name it has: its file and its gaps' features. None without the data.
fn rows() -> Option<&'static BTreeMap<String, (String, Vec<String>)>> {
    static ROWS: OnceLock<Option<BTreeMap<String, (String, Vec<String>)>>> = OnceLock::new();
    ROWS.get_or_init(|| {
        let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let (corpus, oracle) = (data.join("mtgish.lines.ron"), data.join("oracle.json"));
        if !corpus.exists() || !oracle.exists() {
            return None;
        }
        let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
        let (subset, out) = (tmp.join("faces.ron"), tmp.join("faces.jsonl"));
        let lines: Vec<String> = std::fs::read_to_string(&corpus)
            .unwrap()
            .lines()
            .filter(|line| NAMES.iter().any(|name| line.contains(&format!("Name: \"{name}\""))))
            .map(str::to_string)
            .collect();
        std::fs::write(&subset, lines.join("\n") + "\n").unwrap();
        planned::tracked(&subset, &oracle, &out).unwrap();
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
}

#[test]
fn the_data_s_wrong_reads_are_planned_holes() {
    let Some(rows) = rows() else { return };
    for (name, feature) in [
        ("Ajani, Nacatl Pariah", "one or more that die read as each in the data"),
        ("Sink into Stupor", "an opponent's permanent read as anyone's in the data"),
    ] {
        let (_, gaps) = &rows[name];
        assert!(gaps.iter().any(|gap| gap == feature), "{name}: {gaps:?}");
    }
}
