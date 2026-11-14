#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, corpus, ir::*, lower, mtg_types::OracleCard, unified};

fn source(name: &str) -> OracleCard {
    include_str!("fixtures/variable_sacrifice_activation.ronl")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| ron::from_str::<OracleCard>(line).unwrap())
        .find(|card| {
            corpus::Identity::of(card)
                .unwrap()
                .names
                .iter()
                .any(|n| n == name)
        })
        .unwrap()
}

#[test]
fn existing_native_activation_costs_cover_any_number_x_and_one_or_more() {
    cantrip::planned::with_additional_features(&["variable-sacrifice-activations"], || {
        for (name, kind) in [
            ("Sword of the Ages", "SacrificeX("),
            ("Krav, the Unredeemed", "SacrificeX("),
            ("Radiant Lotus", "SacrificeOneOrMore("),
        ] {
            let source = source(name);
            let result = cantrip::whole(&source, &unified::lower(&source).card).unwrap();
            let face = &result.faces[0];
            assert!(
                !face
                    .gaps
                    .iter()
                    .any(|gap| gap.feature.starts_with("sacrificing ")),
                "{name}: {:?}",
                face.gaps
            );
            assert!(face.rust.contains(kind), "{name}: {}", face.rust);
            if name == "Sword of the Ages" {
                assert!(face.gaps.is_empty(), "{:?}", face.gaps);
                assert!(!face.rust.contains("SacrificeAnyNumber"), "{}", face.rust);
                assert!(face.rust.contains("CostPart::Variable"), "{}", face.rust);
            }
        }
    });
}

#[test]
fn independent_mana_x_does_not_become_the_number_sacrificed() {
    let mut card = lower::card(&source("Sword of the Ages")).unwrap();
    let cost = card
        .abilities
        .iter_mut()
        .find_map(|ability| match ability {
            Ability::Activated { cost, .. } => Some(cost),
            _ => None,
        })
        .unwrap();
    cost.parts.push(CostPart::Mana(vec![ManaSymbol::X]));
    cantrip::planned::with_additional_features(&["variable-sacrifice-activations"], || {
        assert!(cantrip::gaps(&card).iter().any(
            |gap| gap.feature == "sacrificing any number of permanents to activate an ability"
        ));
    });
}

#[test]
fn older_activation_profiles_keep_explicit_cost_requirements() {
    cantrip::planned::without_features(&["variable-sacrifice-activations"], || {
        let source = source("Sword of the Ages");
        let result = cantrip::whole(&source, &unified::lower(&source).card).unwrap();
        assert!(result.faces[0].gaps.iter().any(
            |gap| gap.feature == "sacrificing any number of permanents to activate an ability"
        ));
    });
}
