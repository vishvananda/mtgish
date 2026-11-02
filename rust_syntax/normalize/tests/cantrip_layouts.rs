#![cfg(feature = "cantrip")]
//! A record of any layout as one file: `CARD`, `EXTRA`, and the faces beside them.
use mtgish_ast::{cantrip, corpus, mtg_types::OracleCard, unified};

fn whole(name: &str) -> (cantrip::Whole, Vec<String>) {
    let source: OracleCard = include_str!("fixtures/layouts.ronl")
        .lines()
        .map(|s| ron::from_str(s).unwrap())
        .find(|s| {
            corpus::Identity::of(s)
                .unwrap()
                .names
                .iter()
                .any(|n| n == name)
        })
        .unwrap_or_else(|| panic!("no fixture {name}"));
    let lowered = unified::lower(&source);
    let whole = cantrip::whole(&source, &lowered.card).unwrap();
    let gaps = cantrip::whole_gaps(&source, &lowered.card);
    (whole, gaps.into_iter().map(|g| g.feature).collect())
}

/// The file, and the line that defines its top-level `CARD`.
fn file(name: &str) -> (String, String) {
    let rust = whole(name).0.rust.unwrap();
    let top = rust.split("pub mod ").next().unwrap();
    let card = top
        .rsplit("pub static CARD: CardDef = ")
        .next()
        .unwrap()
        .to_string();
    (rust, card)
}

fn extra(rust: &str) -> &str {
    let at = rust.find("pub static EXTRA").expect("a file lists EXTRA");
    &rust[at..at + rust[at..].find('\n').unwrap()]
}

#[test]
fn a_double_faced_card_is_its_front_with_a_back() {
    let (rust, card) = file("Agadeem's Awakening");
    assert!(card.contains("name: \"Agadeem's Awakening\""));
    assert!(card.contains("back: Some(&back::CARD)"));
    assert!(card.contains("back_land: true"));
    assert!(rust.contains("pub mod back {"));

    let (_, card) = file("Hakka, Whispering Raven");
    assert!(card.contains("back: Some(&back::CARD)"));
    assert!(!card.contains("back_land"));
}

#[test]
fn a_back_face_earthbends_with_the_ability_its_front_lists_first() {
    let (rust, card) = file("Aang, at the Crossroads");
    assert!(card.contains("delayed: &[cantrip_mechanics::earthbend::RETURN,"));
    assert!(rust.contains("earthbend::effect"));
}

#[test]
fn a_split_card_is_both_halves_at_once() {
    let (rust, card) = file("Wear");
    assert!(card.contains("name: \"Wear // Tear\""));
    assert!(card.contains("split: Some((&left::CARD, &right::CARD))"));
    assert!(card.contains("fuse: true"));
    // {1}{R} and {W}: the two costs together (CR 709.4b).
    assert!(card.contains("generic: 1, x: 0, symbols: [1, 0, 0, 1, 0, 0]"));
    assert!(card.contains("types: TypeSet::of(&[CardType::Instant])"));
    assert_eq!(
        extra(&rust),
        "pub static EXTRA: &[&CardDef] = &[&left::CARD,&right::CARD];"
    );
    assert_eq!(rust.matches("pub static EXTRA").count(), 1);
    assert!(whole("Wear").1.is_empty());
}

#[test]
fn aftermath_is_on_the_half_that_has_it() {
    let (rust, _) = file("Cut");
    let right = rust.split("pub mod right {").nth(1).unwrap();
    let left = rust.split("pub mod right {").next().unwrap();
    assert!(right.contains("aftermath: true"));
    assert!(!left.contains("aftermath: true"));
}

#[test]
fn an_inset_face_is_listed_with_what_it_brings() {
    let (rust, card) = file("Bonecrusher Giant");
    assert!(card.contains("adventure: Some(&adventure::CARD)"));
    assert!(extra(&rust).contains("&adventure::CARD"));
    let (rust, card) = file("Scavenger Regent");
    assert!(card.contains("omen: Some(&omen::CARD)"));
    assert!(extra(&rust).contains("&omen::CARD"));
}

#[test]
fn a_prepare_spell_is_no_card_of_its_name() {
    let (rust, card) = file("Abigale, Poet Laureate");
    assert!(card.contains("prepare: Some(&prepared::CARD)"));
    assert!(extra(&rust).contains("&prepared::CARD"));
    assert!(card.contains("oracle: true"));
    let prepared = rust.split("pub mod prepared {").nth(1).unwrap();
    assert!(prepared.contains("oracle: false"));
}

#[test]
fn a_room_knows_its_doors() {
    let (rust, card) = file("Unholy Annex");
    assert!(card.contains("split: Some((&left::CARD, &right::CARD))"));
    assert!(card.contains("Subtype::Room"));
    assert!(rust
        .split("pub mod right {")
        .nth(1)
        .unwrap()
        .contains("*door == DOOR_RIGHT"));
}

#[test]
fn a_door_cantrip_would_refuse_is_a_gap() {
    let (_, gaps) = whole("Greenhouse");
    assert!(gaps.iter().any(|g| g == "Room door with static abilities"));
}

#[test]
fn what_cantrip_has_no_model_of_is_a_hole_and_a_requirement() {
    let (rust, _) = file("Budoka Gardener");
    assert!(rust.contains("planned_item!(\"layout-flip\" => flip: Some(&flipped::CARD));"));
    assert!(whole("Budoka Gardener")
        .1
        .contains(&"layout Flip".to_string()));

    for (name, code) in [
        ("Brisela, Voice of Nightmares", "melded: true"),
        ("Akoum", "variant: Some(Variant::Planechase)"),
        (
            "A Reckoning Approaches",
            "variant: Some(Variant::Archenemy)",
        ),
        ("Adriana's Valor", "variant: Some(Variant::ConspiracyDraft)"),
        (
            "Akroma, Angel of Wrath Avatar",
            "variant: Some(Variant::Vanguard { hand: 1, life: 7 })",
        ),
        ("Lost Mine of Phandelver", "dungeon: &[&[1, 2], "),
    ] {
        let (whole, gaps) = whole(name);
        assert!(whole.faces[0].rust.contains(code), "{name}");
        assert!(gaps.iter().any(|g| g.starts_with("layout ")), "{name}");
    }
}

#[test]
fn only_a_gapless_record_has_a_module() {
    let source: OracleCard = include_str!("fixtures/layouts.ronl")
        .lines()
        .map(|s| ron::from_str(s).unwrap())
        .find(|s| corpus::Identity::of(s).unwrap().layout == "Flip")
        .unwrap();
    let lowered = unified::lower(&source);
    assert!(cantrip::whole_module(&source, &lowered.card).is_err());
}

/// Bonecrusher Giant's whole file, with `line` as the printed text of its creature face.
fn printed(line: &str) -> String {
    let source: OracleCard = include_str!("fixtures/layouts.ronl")
        .lines()
        .map(|s| ron::from_str(s).unwrap())
        .find(|s| corpus::Identity::of(s).unwrap().names[0] == "Bonecrusher Giant")
        .unwrap();
    let lowered = unified::lower(&source);
    let oracle = [("Bonecrusher Giant".to_string(), line.to_string())].into();
    cantrip::whole_printed(&source, &lowered.card, Some(&oracle))
        .unwrap()
        .rust
        .unwrap()
}

#[test]
fn an_ability_reads_as_its_card_prints_it_without_reminder_text() {
    let rust = printed(
        "Whenever this creature becomes the target of a spell, it bites that spell's controller. (Ouch.)",
    );
    assert!(rust.contains("\"Whenever this creature becomes the target of a spell, it bites that spell's controller.\""));
    assert!(!rust.contains("Ouch"));
}

#[test]
fn a_line_that_cannot_be_aligned_leaves_the_label_as_it_was() {
    let rust = printed("{T}: Lands are forests.");
    assert!(!rust.contains("Lands are forests."));
    assert_eq!(rust, whole("Bonecrusher Giant").0.rust.unwrap());
}
