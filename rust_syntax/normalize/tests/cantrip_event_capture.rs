#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, lower, mtg_types::OracleCard};

#[test]
fn one_shot_trigger_watches_source_and_carries_an_independent_object() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/mtgish.lines.ron");
    let Ok(corpus) = std::fs::read_to_string(path) else { return };
    let line = corpus.lines().find(|line| line.starts_with("Card(Name: \"Necromancy\"")).unwrap();
    let source: OracleCard = ron::from_str(line).unwrap();
    let card = lower::card(&source).unwrap();
    let raw = cantrip::planned::with_additional_features(&[], || cantrip::module(&card)).unwrap();
    assert!(raw.contains("g.delayed_trigger_about(DelayedWhen::WhenEvent"), "{raw}");
    assert!(raw.contains("q.event_objects(item)"), "{raw}");
    assert!(!raw.contains("DelayedWhen::UntilEvent"), "{raw}");
    let printed = cantrip::pr_form::render(&raw, "Necromancy", "When this enchantment leaves the battlefield, that creature's controller sacrifices it.", cantrip::pr_form::Characteristics::Inline).unwrap();
    syn::parse_file(&printed).unwrap();
    if let Ok(path) = std::env::var("CANTRIP_NECROMANCY_PRINTED") { std::fs::write(path, printed).unwrap(); }
}
