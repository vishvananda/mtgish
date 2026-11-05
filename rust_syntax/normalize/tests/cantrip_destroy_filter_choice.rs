#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, lower, mtg_types::OracleCard};

#[test]
fn destruction_options_describe_actions() {
    let corpus = include_str!("fixtures/pipeline_generated.ronl");
    let line = corpus.lines().find(|line| line.starts_with("Card(Name: \"Essence Filter\"")).unwrap();
    let source: OracleCard = ron::from_str(line).unwrap();
    let card = lower::card(&source).unwrap();
    let code = cantrip::planned::with_additional_features(&[], || cantrip::module(&card)).unwrap();
    assert!(code.contains("Destroy all enchantments"), "{code}");
    assert!(code.contains("Destroy all nonwhite enchantments"), "{code}");
    assert!(code.contains("Label::Card(item.ability.card"), "{code}");
}

#[test]
fn other_filter_choices_keep_word_options() {
    let corpus = include_str!("fixtures/pipeline_generated.ronl");
    let line = corpus.lines().find(|line| line.starts_with("Card(Name: \"Essence Filter\"")).unwrap();
    let line = line.replace("DestroyEachPermanent(IsTheChosenPermanentFilter)", "PhaseOutEachPermanent(IsTheChosenPermanentFilter)");
    let source: OracleCard = ron::from_str(&line).unwrap();
    let card = lower::card(&source).unwrap();
    let code = cantrip::planned::with_additional_features(&[], || cantrip::module(&card)).unwrap();
    assert!(code.contains("choose::option("), "{code}");
    assert!(!code.contains("Destroy all"), "{code}");
}
