#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, corpus, ir::*, lower, mtg_types::OracleCard};

fn card(name: &str) -> Card {
    include_str!("fixtures/native_untap_landwalk.ronl").lines().find_map(|line| {
        let source: OracleCard = ron::from_str(line).unwrap();
        corpus::Identity::of(&source).unwrap().names.iter().any(|n| n == name)
            .then(|| lower::card(&source).unwrap())
    }).unwrap()
}

#[test]
fn source_tapped_durations_use_the_native_interval_authority() {
    for name in ["Mole Worms", "Phyrexian Gremlins", "Ice Floe", "Seasinger", "Spirit Shield", "Tawnos's Weaponry", "Willow Satyr"] {
        let generated = cantrip::module(&card(name)).unwrap();
        assert!(generated.contains("Duration::WhileSourceTapped"), "{name}: {generated}");
        assert!(!generated.contains("Duration::AsLongAs("), "{name}: {generated}");
    }
    let generated = cantrip::module(&card("Rubinia Soulsinger")).unwrap();
    assert!(generated.contains("Duration::WhileSourceTappedAndControlled"), "{generated}");
}

#[test]
fn supported_landwalks_and_references_use_native_keyword_metadata() {
    for (name, keyword) in [("Rime Dryad", "SnowForestwalk"), ("Legions of Lim-Dûl", "SnowSwampwalk"),
        ("Desert Nomads", "Desertwalk"), ("Livonya Silone", "LegendaryLandwalk")] {
        let generated = cantrip::module(&card(name)).unwrap();
        assert!(generated.contains(&format!("Keyword::{keyword}")), "{name}: {generated}");
        assert!(!generated.contains("q.defending_player(attacker)"), "{name}: {generated}");
    }
    let generated = cantrip::module(&card("Merfolk Assassin")).unwrap();
    assert!(generated.contains("Keyword::landwalk(q.words(asker.source).subtype(Subtype::Island))"), "{generated}");
    assert!(generated.contains("keywords.matches(walk)"), "{generated}");
    let generated = cantrip::module(&card("Hammerheim")).unwrap();
    assert!(generated.contains("EffectKind::LoseKeywords(Keywords::LANDWALKS)"), "{generated}");
}

#[test]
fn floating_landwalk_grants_and_removals_read_the_sources_type_word() {
    let generated = cantrip::module(&card("Scarwood Hag")).unwrap();
    assert!(generated.contains("EffectKind::AddKeywords") && generated.contains("EffectKind::LoseKeywords"), "{generated}");
    assert!(generated.matches("q.words(asker.source).subtype(Subtype::Forest)").count() >= 2, "Both effects must follow the printed land type: {generated}");
}

#[test]
fn simultaneous_untap_results_record_actual_untaps() {
    let source: OracleCard = ron::from_str(r#"Card(Name: "Untap probe", Typeline: (Supertypes: [], Cardtypes: [Sorcery], Subtypes: []), Rules: [SpellActions(ActionList([DrawACard]))])"#).unwrap();
    let mut card = lower::card(&source).unwrap();
    card.abilities = vec![Ability::Spell { announcement: Announcement::empty(), body: Program { instructions: vec![
        Instruction::Effect { result: ResultId(0), operation: Operation::Tap { objects: ObjectSet::battlefield(ObjectFilter::Any), tapped: false } },
        Instruction::Effect { result: ResultId(1), operation: Operation::GainLife { players: PlayerSet::One(PlayerRef::AbilityController), amount: Expr::CountObjects(Box::new(ObjectSet::Transition { result: ResultId(0), phase: ObjectPhase::After })) } },
    ] } }];
    let generated = cantrip::module(&card).unwrap();
    assert!(generated.contains("g.untap_together("), "{generated}");
    assert!(generated.contains("Event::Untapped"), "{generated}");
    assert!(!generated.contains("g.untap(object)"), "{generated}");
}
