#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, ir::*, lower, mtg_types::OracleCard, validate};

fn card(name: &str) -> Card {
    let source: OracleCard = ron::from_str(include_str!("fixtures/captured_replacements.ronl")
        .lines().find(|line| line.contains(&format!("Name: {name:?}"))).unwrap()).unwrap();
    lower::card(&source).unwrap()
}

#[test]
fn old_targets_retain_the_makers_x_gap() {
    cantrip::planned::with_additional_features(&[], || {
        let gaps = cantrip::gaps(&card("Aladdin's Lamp"));
        assert!(gaps.iter().any(|g| g.feature == "the X of the ability that made a replacement effect"), "{gaps:?}");
    });
}

#[test]
fn lamp_captures_x_at_creation_and_passes_it_to_the_waiting_callbacks() {
    cantrip::planned::with_additional_features(&["captured-replacement-values"], || {
        let card = card("Aladdin's Lamp");
        validate::card(&card).unwrap();
        assert!(cantrip::gaps(&card).is_empty(), "{:?}", cantrip::gaps(&card));
        let code = cantrip::module(&card).unwrap();
        syn::parse_file(&code).unwrap();
        assert!(code.contains("captured_granted_replacement("), "{code}");
        assert!(code.contains("CapturedReplacementCallbacks"), "{code}");
        assert!(code.contains(".show_captured_value()"), "announced X is public: {code}");
        assert!(code.contains("EffectKind::CapturedReplacementRule"), "{code}");
        assert!(code.contains("let created_x ="), "{code}");
        assert!(code.contains("i64::from(item.x)"), "{code}");
        assert!(code.contains("i64::from(created_x)"), "{code}");
        assert!(code.contains("created_x: u32"), "{code}");
        assert!(!code.contains("q.obj(asker.source).x"), "{code}");
        assert!(!code.contains("Purpose::ChooseAsEnters"), "library selection is not entering: {code}");
        assert!(code.contains("Purpose::Choose,"), "ordinary selection: {code}");
        assert!(!code.contains("planned!"), "{code}");
    });
}

#[test]
fn damage_replacement_uses_the_same_capture_contract() {
    cantrip::planned::with_additional_features(&["captured-replacement-values"], || {
        let mut card = card("Taii Wakeen, Perfect Shot");
        // This isolates the shared captured-X prerequisite; it does not claim
        // the unrelated damage-trigger part of Taii's card is generated.
        card.abilities.retain(|a| matches!(a, Ability::Activated { .. }));
        validate::card(&card).unwrap();
        assert!(cantrip::gaps(&card).is_empty(), "{:?}", cantrip::gaps(&card));
        let code = cantrip::module(&card).unwrap();
        syn::parse_file(&code).unwrap();
        assert!(code.contains("EffectKind::CapturedReplacementRule"), "{code}");
        assert!(code.contains("i64::from(created_x)"), "{code}");
        assert!(!code.contains("planned!"), "{code}");
    });
}
