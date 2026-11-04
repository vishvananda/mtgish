#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, corpus, lower, mtg_types::OracleCard};

fn card(name: &str) -> mtgish_ast::ir::Card {
    include_str!("fixtures/combat_entry_authority.ronl").lines().find_map(|line| {
        let source: OracleCard = ron::from_str(line).unwrap();
        corpus::Identity::of(&source).unwrap().names.iter().any(|n| n == name)
            .then(|| lower::card(&source).unwrap())
    }).unwrap()
}

fn printed(name: &str) -> String {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    cantrip::modules_printed(&dir.join("combat_entry_authority.ronl"),
        &dir.join("combat_entry_authority.oracle.json"), &[name.into()])
        .unwrap().pop().expect("original printed card").1
}

#[test]
fn departed_combat_sources_use_directional_native_relationships() {
    cantrip::planned::with_features(&["combat-lki", "blocking-status"], || {
        for name in ["Abu Ja'far", "Tinder Wall"] {
            let module = printed(name);
            assert!(module.contains("q.creatures_blocked_by("), "{name}: {module}");
            if name == "Abu Ja'far" {
                assert!(module.contains("q.creatures_blocking("), "{module}");
            }
            syn::parse_file(&module).unwrap();
        }
    });
}

#[test]
fn the_chants_read_the_recorded_putter_instead_of_the_entering_controller() {
    cantrip::planned::with_features(&["entry-actor"], || {
        for name in ["Thelon's Chant", "Tourach's Chant"] {
            let module = printed(name);
            assert!(module.contains("put_by: Some(put_by)"), "{name}: {module}");
            assert!(module.contains("let player = *put_by;"), "{name}: {module}");
            syn::parse_file(&module).unwrap();
        }
    });
}

#[test]
fn unchanged_ordinary_controller_entry_events_keep_their_meaning() {
    use mtgish_ast::ir::*;
    let mut c = card("Thelon's Chant");
    let event = c.abilities.iter_mut().find_map(|ability| match ability {
        Ability::Triggered { event: EventSpec::Player { event, .. }, .. } => Some(event),
        _ => None,
    }).expect("the printed land-put trigger");
    *event = PlayerEvent::EntersUnderControl;
    let old = cantrip::planned::with_features(&["resolving-counter-costs"], || cantrip::module(&c)).unwrap();
    let new = cantrip::planned::with_features(&["entry-actor", "resolving-counter-costs"], || cantrip::module(&c)).unwrap();
    assert_eq!(old, new);
    assert!(new.contains("let player = q.controller(id);"), "{new}");
}
