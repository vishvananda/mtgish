#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, corpus, lower, mtg_types::OracleCard};

#[test]
fn printed_relative_prevention_and_damage_replacement_keep_their_distinct_meanings() {
    for line in include_str!("fixtures/quantity_prevention.ronl").lines() {
        let source: OracleCard = ron::from_str(line).unwrap();
        let name = corpus::Identity::of(&source).unwrap().names[0].clone();
        let card = lower::card(&source).unwrap();
        cantrip::planned::with_features(&[], || {
            let old = cantrip::module(&card).unwrap();
            assert!(!old.contains("PreventionAmount::"), "{name}: {old}");
        });
        cantrip::planned::with_features(&["wide-effect-quantities"], || {
            let raw = cantrip::module(&card).unwrap();
            if name == "Benevolent Unicorn" {
                // Its current words reduce the damage instead; this is a
                // replacement effect, not a fixed prevention allowance.
                assert!(!raw.contains("PreventionAmount::"), "{raw}");
                assert!(raw.contains("e.amount = "), "{raw}");
                assert!(!raw.contains("as u16"), "{raw}");
                return;
            }
            let variant = "Relative";
            assert!(
                raw.contains(&format!("PreventionAmount::{variant}")),
                "{name}: {raw}"
            );
            assert!(raw.contains("-> i64"), "{name}: {raw}");
            let file = cantrip::pr_form::render(
                &raw,
                &name,
                "",
                cantrip::pr_form::Characteristics::Inline,
            )
            .unwrap();
            assert!(
                file.contains(&format!("PreventionAmount::{variant}")),
                "{name}: {file}"
            );
            assert!(!file.contains("-> u16"), "{name}: {file}");
            syn::parse_file(&file).unwrap();
        });
    }
}

#[test]
fn exact_damage_capture_survives_pr_form_for_a_printed_damage_trigger() {
    let line = include_str!("fixtures/damage.ronl")
        .lines()
        .find(|line| line.starts_with("Card(Name: \"Spitemare\""))
        .unwrap();
    let source: OracleCard = ron::from_str(line).unwrap();
    let card = lower::card(&source).unwrap();
    cantrip::planned::with_features(&["wide-effect-quantities"], || {
        let raw = cantrip::module(&card).unwrap();
        assert!(raw.contains(".damage_amount()"), "{raw}");
        assert!(raw.contains("item.matched_damage_amount()"), "{raw}");
        let file = cantrip::pr_form::render(
            &raw,
            "Spitemare",
            "",
            cantrip::pr_form::Characteristics::Inline,
        )
        .unwrap();
        assert!(file.contains(".damage_amount()"), "{file}");
        assert!(file.contains("item.matched_damage_amount()"), "{file}");
        assert!(!file.contains("-> u16"), "{file}");
        syn::parse_file(&file).unwrap();
    });
}

#[test]
fn printed_life_delta_uses_the_exact_bounded_legacy_payload() {
    let source: OracleCard =
        ron::from_str(include_str!("fixtures/quantity_life.ronl").trim()).unwrap();
    let card = lower::card(&source).unwrap();
    cantrip::planned::with_features(&["wide-effect-quantities"], || {
        let raw = cantrip::module(&card).unwrap();
        assert!(raw.contains("u32::try_from(delta.unsigned_abs())"), "{raw}");
        assert!(raw.contains("i64::from(item.data[0])"), "{raw}");
        assert!(!raw.contains("matched_damage_amount()"), "{raw}");
        assert!(!raw.contains("as i32"), "{raw}");
        let file = cantrip::pr_form::render(
            &raw,
            "Exquisite Blood",
            "",
            cantrip::pr_form::Characteristics::Inline,
        )
        .unwrap();
        assert!(!file.contains("planned"), "{file}");
        assert!(!file.contains("as i32"), "{file}");
        syn::parse_file(&file).unwrap();
    });
}
