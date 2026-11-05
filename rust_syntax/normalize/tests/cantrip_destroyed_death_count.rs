#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, mtg_types::OracleCard, unified};

#[test]
fn creatures_that_died_this_way_must_have_entered_a_graveyard() {
    let line = include_str!("fixtures/pipeline_generated.ronl")
        .lines()
        .find(|line| line.starts_with("Card(Name: \"Hellfire\""))
        .unwrap();
    let source: OracleCard = ron::from_str(line).unwrap();
    let lowered = unified::lower(&source);
    let oracle = [("Hellfire".to_string(),
        "Destroy all nonblack creatures. Hellfire deals X plus 3 damage to you, where X is the number of creatures that died this way.".to_string())].into();
    let whole = cantrip::whole_printed(&source, &lowered.card, Some(&oracle)).unwrap();
    assert!(whole.faces.iter().all(|face| face.gaps.is_empty()));
    let generated = whole.rust.unwrap_or_else(|| whole.faces.iter().map(|face| face.rust.as_str()).collect());
    assert!(generated.contains("to.is(ZoneKind::Graveyard)"), "{generated}");
    assert!(!generated.contains("became.is_some()"), "{generated}");
    if let Ok(path) = std::env::var("HELLFIRE_DEATH_COUNT_OUTPUT") {
        std::fs::write(path, generated).unwrap();
    }
}
