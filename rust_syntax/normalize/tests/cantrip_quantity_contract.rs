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

#[test]
fn printed_wide_history_queries_keep_exact_amounts_and_snapshot_filters() {
    for (name, marker) in [
        ("Ojer Axonil, Deepest Might", "let dealt = DamageDealt {"),
        ("Impact Resonance", "Vec<(Target, u32, i64)>"),
        ("Enlightened Confidant", "Option<(Fired, i64)>"),
        ("Reverse Polarity", "saturating_mul"),
    ] {
        let line = include_str!("fixtures/pipeline_generated.ronl").lines()
            .find(|line| line.contains(&format!("Name: \"{name}\""))).unwrap();
        let source: OracleCard = ron::from_str(line).unwrap();
        let card = mtgish_ast::unified::lower(&source).card;
        for wide in [false, true] {
            cantrip::planned::with_features(if wide { &["wide-effect-quantities"] } else { &[] }, || {
                let raw = if name == "Impact Resonance" {
                    let whole = cantrip::whole(&source, &card).unwrap();
                    let gaps: Vec<_> = whole.faces.iter().flat_map(|f| &f.gaps).map(|g| g.feature.as_str()).collect();
                    assert_eq!(gaps, ["target with a divided amount read as chosen"], "{name} wide={wide}");
                    whole.faces[0].rust.clone()
                } else {
                    cantrip::whole_module(&source, &card).unwrap_or_else(|e| panic!("{name} wide={wide}: {e:?}"))
                };
                if wide {
                    assert!(raw.contains(marker), "{name}: {raw}");
                    assert!(raw.contains("Option<(Fired, i64)>"), "{name}: {raw}");
                    assert!(raw.contains("saturating_add"), "{name}: {raw}");
                } else {
                    assert!(!raw.contains("Option<(Fired, i64)>"), "{name}: {raw}");
                }
                syn::parse_file(&raw).unwrap();
            });
        }
    }
}


#[test]
fn final_pr_form_actual_life_matcher_compiles_and_preserves_full_magnitude() {
    use quote::ToTokens;
    use std::{fs, process::Command};
    let line = include_str!("fixtures/pipeline_generated.ronl").lines()
        .find(|line| line.contains("Name: \"Oath of Lim-Dûl\"" )).unwrap();
    let source: OracleCard = ron::from_str(line).unwrap();
    let card = lower::card(&source).unwrap();
    let raw = cantrip::planned::with_features(&["wide-effect-quantities"], || cantrip::module(&card).unwrap());
    let file = cantrip::pr_form::render(&raw, "Oath of Lim-Dûl", "Whenever you lose life, for each 1 life you lost, sacrifice a permanent other than this enchantment unless you discard a card.", cantrip::pr_form::Characteristics::Inline).unwrap();
    let parsed = syn::parse_file(&file).unwrap();
    let matcher = parsed.items.iter().find_map(|item| match item {
        syn::Item::Fn(f) if f.sig.output.to_token_stream().to_string().contains("Option < Fired >") => Some(f),
        _ => None,
    }).unwrap();
    let name = &matcher.sig.ident;
    // Compile the exact matcher AST taken AFTER final PR-form rewrites. These
    // types match the final native life-event boundary, not an alternate callback.
    let witness = format!(r#"
        #![allow(dead_code, unused_variables)]
        type PlayerId = u8;
        trait Query {{}}
        struct Q; impl Query for Q {{}}
        struct Asker {{ controller: PlayerId }}
        #[derive(Debug, PartialEq)] enum Target {{ Player(PlayerId) }}
        enum Event {{ LifeChange {{ player: PlayerId, delta: i64 }}, Other }}
        struct Fired {{ subject: Target, data: [u32; 2] }}
        {matcher}
        fn main() {{
            let q = Q; let asker = Asker {{ controller: 1 }};
            for magnitude in [1, i64::from(u32::MAX)] {{
                let fired = {name}(&q, &asker, &Event::LifeChange {{ player: 1, delta: -magnitude }}).unwrap();
                assert_eq!(fired.subject, Target::Player(1)); assert_eq!(i64::from(fired.data[0]), magnitude);
                assert!({name}(&q, &asker, &Event::LifeChange {{ player: 1, delta: magnitude }}).is_none());
                assert!({name}(&q, &asker, &Event::LifeChange {{ player: 0, delta: -magnitude }}).is_none());
            }}
            assert!({name}(&q, &asker, &Event::Other).is_none());
        }}"#, matcher = matcher.to_token_stream());
    let dir = std::env::temp_dir().join(format!("mtgish-final-pr-life-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap(); fs::write(dir.join("main.rs"), &witness).unwrap();
    let built = Command::new("rustc").args(["--edition=2021", "-C", "overflow-checks=on"]).arg(dir.join("main.rs")).arg("-o").arg(dir.join("witness")).output().unwrap();
    assert!(built.status.success(), "{}\n{file}", String::from_utf8_lossy(&built.stderr));
    assert!(Command::new(dir.join("witness")).status().unwrap().success());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn printed_32_bit_prevention_and_damage_triggers_use_bounded_native_shapes() {
    cantrip::planned::with_features(&["32-bit-effect-quantities"], || {
        for line in include_str!("fixtures/quantity_prevention.ronl").lines() {
            let source: OracleCard = ron::from_str(line).unwrap();
            let name = corpus::Identity::of(&source).unwrap().names[0].clone();
            let raw = cantrip::module(&lower::card(&source).unwrap()).unwrap();
            assert!(!raw.contains("PreventionAmount::"), "{name}: {raw}");
            if name != "Benevolent Unicorn" {
                assert!(raw.contains("-> u32"), "{name}: {raw}");
            }
            syn::parse_file(&raw).unwrap();
        }
        let line = include_str!("fixtures/damage.ronl").lines()
            .find(|line| line.starts_with("Card(Name: \"Spitemare\"")).unwrap();
        let source: OracleCard = ron::from_str(line).unwrap();
        let raw = cantrip::module(&lower::card(&source).unwrap()).unwrap();
        assert!(!raw.contains("matched_damage_amount()"), "{raw}");
        assert!(!raw.contains(".damage_amount()"), "{raw}");
        assert!(raw.contains("item.data[0]"), "{raw}");
        syn::parse_file(&raw).unwrap();
    });
}

#[test]
fn printed_bolt_and_ballista_generate_for_bounded_32_and_legacy_profiles() {
    for (line, name) in [
        (include_str!("fixtures/backend.ronl").lines().find(|l| l.starts_with("Card(Name: \"Lightning Bolt\"")).unwrap(), "Lightning Bolt"),
        (include_str!("fixtures/entry_costs.ronl").lines().find(|l| l.starts_with("Card(Name: \"Walking Ballista\"")).unwrap(), "Walking Ballista"),
    ] {
        let source: OracleCard = ron::from_str(line).unwrap();
        let card = lower::card(&source).unwrap();
        for features in [&[][..], &["32-bit-effect-quantities"][..]] {
            let generated = cantrip::planned::with_features(features, || cantrip::module(&card))
                .unwrap_or_else(|e| panic!("{name} {features:?}: {e:?}"));
            syn::parse_file(&generated).unwrap();
            if name == "Lightning Bolt" {
                let damage = if features.is_empty() { "u16" } else { "u32" };
                assert!(generated.contains(&format!("Vec<(ObjectId, Target, {damage})>")), "{generated}");
            }
        }

        let combined = cantrip::planned::with_additional_features(&["32-bit-effect-quantities"], || cantrip::module(&card))
            .unwrap_or_else(|e| panic!("{name} combined 32 profile: {e:?}"));
        if name == "Lightning Bolt" {
            assert!(combined.contains("Vec<(ObjectId, Target, u32)>"), "{combined}");
        }
        syn::parse_file(&combined).unwrap();
    }
}

#[test]
fn printed_32_bit_typed_prevention_keeps_relative_semantics() {
    cantrip::planned::with_features(&["32-bit-effect-quantities", "typed-prevention-amount"], || {
        for line in include_str!("fixtures/quantity_prevention.ronl").lines() {
            let source: OracleCard = ron::from_str(line).unwrap();
            let name = corpus::Identity::of(&source).unwrap().names[0].clone();
            let raw = cantrip::module(&lower::card(&source).unwrap()).unwrap();
            if name != "Benevolent Unicorn" {
                assert!(raw.contains("PreventionAmount::Relative"), "{name}: {raw}");
                assert!(raw.contains("-> u32"), "{name}: {raw}");
            }
            syn::parse_file(&raw).unwrap();
        }
    });
}

#[test]
fn printed_divided_cards_follow_detected_native_divide_shape() {
    for name in ["Fiery Justice", "Meteor Shower"] {
        let line = include_str!("fixtures/pipeline_generated.ronl").lines()
            .find(|line| line.contains(&format!("Name: \"{name}\""))).unwrap();
        let source: OracleCard = ron::from_str(line).unwrap();
        let card = mtgish_ast::unified::lower(&source).card;
        for new_shape in [false, true] {
            let features: &[&str] = if new_shape { &["32-bit-effect-quantities", "divide-reads-context"] } else { &[] };
            let raw = cantrip::planned::with_features(features, || cantrip::whole_module(&source, &card))
                .unwrap_or_else(|e| panic!("{name} new_shape={new_shape}: {e:?}"));
            let field = if new_shape { "reads:" } else { "of_x:" };
            assert!(raw.contains(field), "{name}: {raw}");
            if name == "Meteor Shower" && new_shape {
                assert!(raw.contains("_: &dyn Query, _: &Asker, x: u32"), "{raw}");
            }
            syn::parse_file(&raw).unwrap();
        }
    }
}
