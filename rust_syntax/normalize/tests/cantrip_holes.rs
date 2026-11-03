#![cfg(feature = "cantrip")]
//! Every record the tracker writes has code, and every part Cantrip can't play yet is a hole in it that says what it waits
//! on: a sticker sheet's stickers, a Room door Cantrip refuses, and a mechanic printed twice that once had none.
use mtgish_ast::cantrip::planned;
use std::path::PathBuf;
use std::sync::OnceLock;

/// The rows of Scurry of Squirrels, Greenhouse // Rickety Gazebo and the sheet with the "Hot Dog" sticker, as
/// (name, code of all faces, file, gaps). None without the data.
fn rows() -> Option<&'static Vec<(String, String, String, Vec<String>)>> {
    static ROWS: OnceLock<Option<Vec<(String, String, String, Vec<String>)>>> = OnceLock::new();
    ROWS.get_or_init(|| {
        let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let (corpus, oracle) = (data.join("mtgish.lines.ron"), data.join("oracle.json"));
        if !corpus.exists() || !oracle.exists() {
            return None;
        }
        let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
        let (subset, out) = (tmp.join("holes.ron"), tmp.join("holes.jsonl"));
        let lines: Vec<String> = std::fs::read_to_string(&corpus)
            .unwrap()
            .lines()
            .filter(|line| {
                line.contains("Name: \"Scurry of Squirrels\"")
                    || line.contains("Name: \"Greenhouse\"")
                    || (line.starts_with("StickerSheet(") && line.contains("NameSticker(\"Hot Dog\")"))
            })
            .map(str::to_string)
            .collect();
        assert_eq!(lines.len(), 3, "{lines:?}");
        std::fs::write(&subset, lines.join("\n") + "\n").unwrap();
        planned::tracked(&subset, &oracle, &out).unwrap();
        let rows = std::fs::read_to_string(&out)
            .unwrap()
            .lines()
            .map(|line| {
                let row: serde_json::Value = serde_json::from_str(line).unwrap();
                let faces = row["faces"].as_array().unwrap();
                let code = row["rust"].as_str().map(str::to_string).unwrap_or_else(|| {
                    faces.iter().map(|face| face["rust"].as_str().unwrap()).collect::<Vec<_>>().join("\n")
                });
                let gaps = faces
                    .iter()
                    .flat_map(|face| face["gaps"].as_array().unwrap().iter())
                    .map(|gap| gap["feature"].as_str().unwrap().to_string())
                    .collect();
                let file = row["file"].as_str().unwrap_or("").to_string();
                (row["name"].as_str().unwrap().to_string(), code, file, gaps)
            })
            .collect();
        Some(rows)
    })
    .as_ref()
}

fn row(name: &str) -> Option<&'static (String, String, String, Vec<String>)> {
    Some(rows()?.iter().find(|row| row.0.starts_with(name)).unwrap_or_else(|| panic!("no row for {name}")))
}

/// "Myriad, myriad": each trigger makes its own delayed trigger from the one first `delayed` ability, so the card is whole.
#[test]
fn a_mechanic_printed_twice_shares_its_first_delayed_ability() {
    let Some((_, _, file, gaps)) = row("Scurry of Squirrels") else { return };
    assert!(gaps.is_empty(), "{gaps:?}");
    assert_eq!(file.matches("myriad::TRIGGER").count(), 2, "{file}");
}

/// A door `CardDef::door_unsupported` refuses is written whole, and the file says that it is held and why.
#[test]
fn a_room_door_cantrip_refuses_is_a_hole_in_the_file() {
    let Some((_, code, _, gaps)) = row("Greenhouse") else { return };
    assert_eq!(gaps, &["Room door with static abilities"]);
    assert!(
        code.contains("planned_item!(\"room-door-with-static-abilities\", \"left::CARD: CardDef::door_unsupported refuses a door with static abilities\");"),
        "{code}"
    );
}

/// A sticker sheet is named for its name stickers, and each sticker is a hole that keeps what it says.
#[test]
fn a_sticker_sheet_is_a_hole_for_each_sticker() {
    let Some((name, code, _, gaps)) = row("Sticker sheet") else { return };
    assert_eq!(name, "Sticker sheet: Ancestral, Hot Dog, Minotaur");
    assert_eq!(gaps, &["engine: no stickers"]);
    assert_eq!(code.matches("planned_item!(\"engine-no-stickers\"").count(), 7, "{code}");
    assert!(code.contains("PowerToughness(tickets:5,power:8,toughness:6)"), "{code}");
}
