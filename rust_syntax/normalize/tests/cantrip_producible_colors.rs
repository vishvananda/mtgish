#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, corpus, mtg_types::OracleCard, unified};

fn source(name: &str) -> OracleCard {
    include_str!("fixtures/producible_mana_20261114.ronl")
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

fn render(source: &OracleCard) -> cantrip::Whole {
    cantrip::whole(source, &unified::lower(source).card).unwrap()
}

#[test]
fn color_capabilities_use_native_query_without_including_colorless() {
    cantrip::planned::with_features(&["producible-mana-types"], || {
        for name in ["Fellwar Stone", "Exotic Orchard"] {
            let result = render(&source(name));
            assert!(
                result.faces[0].gaps.is_empty(),
                "{name}: {:?}",
                result.faces[0].gaps
            );
            let code = &result.faces[0].rust;
            syn::parse_file(code).unwrap();
            assert!(code.contains("q.producible_types(object).0.0"), "{code}");
            assert!(code.contains("q.state().opponents(p)"), "{code}");
            assert!(code.contains("CardType::Land"), "{code}");
            assert!(!code.contains("mana_could_produce"), "{code}");
        }
    });
}

#[test]
fn identical_opponent_land_abilities_use_the_shared_recipe() {
    cantrip::planned::with_features(&["opponents-land-colors"], || {
        for name in ["Fellwar Stone", "Exotic Orchard"] {
            let result = render(&source(name));
            assert!(
                result.faces[0].gaps.is_empty(),
                "{name}: {:?}",
                result.faces[0].gaps
            );
            let code = &result.faces[0].rust;
            assert!(
                code.contains(".colors_from(cantrip_mechanics::mana::opponents_land_colors)"),
                "{code}"
            );
            assert!(!code.contains("q.producible_types"), "{code}");
        }
    });
}

#[test]
fn the_opponent_recipe_does_not_replace_a_different_controller_filter() {
    let text = include_str!("fixtures/producible_mana_20261114.ronl")
        .lines()
        .find(|line| line.contains("Name: \"Fellwar Stone\""))
        .unwrap()
        .replace(
            "ControlledByAPlayer(Opponent)",
            "ControlledByAPlayer(SinglePlayer(You))",
        );
    let own: OracleCard = ron::from_str(&text).unwrap();
    cantrip::planned::with_features(&["producible-mana-types", "opponents-land-colors"], || {
        let result = render(&own);
        assert!(
            result.faces[0].gaps.is_empty(),
            "{:?}",
            result.faces[0].gaps
        );
        let code = &result.faces[0].rust;
        assert!(!code.contains("opponents_land_colors"), "{code}");
        assert!(code.contains("q.producible_types(object).0.0"), "{code}");
    });
}

#[test]
fn old_profiles_and_any_type_queries_keep_their_unmet_contracts() {
    cantrip::planned::with_features(&[], || {
        let result = render(&source("Fellwar Stone"));
        assert!(result.faces[0]
            .gaps
            .iter()
            .any(|gap| gap.feature.contains("could produce")));
    });
    cantrip::planned::with_features(&["producible-mana-types", "opponents-land-colors"], || {
        // Reflecting Pool may produce colorless. A ColorSet-only callback
        // cannot implement that whole Oracle instruction.
        let result = render(&source("Reflecting Pool"));
        assert!(result.faces[0]
            .gaps
            .iter()
            .any(|gap| gap.feature.contains("could produce")));
        assert!(!result.faces[0].rust.contains("opponents_land_colors"));
    });
}
