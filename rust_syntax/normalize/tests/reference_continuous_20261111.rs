use mtgish_ast::{canonical, corpus, ir::Ability, mtg_types::OracleCard, unified, validate};
use serde_json::Value;

fn normalized(abilities: &[Ability]) -> Value {
    validate::abilities(abilities).unwrap();
    let mut value = serde_json::to_value(abilities).unwrap();
    canonical::abilities(&mut value).unwrap();
    value
}

#[test]
fn four_repairs_and_four_controls_preserve_complete_programs() {
    corpus::on_large_stack(|| {
        let expected: Value = serde_json::from_str(include_str!(
            "fixtures/reference_continuous_20261111_expected.json"
        ))
        .unwrap();
        let mut seen = 0;
        for line in include_str!("fixtures/reference_continuous_20261111.ronl").lines() {
            let source: OracleCard = ron::from_str(line).unwrap();
            let result = unified::lower(&source);
            assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
            let card = &result.card.faces[0];
            validate::card(card).unwrap();
            let wanted: Vec<Ability> =
                serde_json::from_value(expected[&card.name].clone()).unwrap();
            assert_eq!(
                normalized(&card.abilities),
                normalized(&wanted),
                "{}",
                card.name
            );
            seen += 1;
        }
        assert_eq!(seen, 8);
    });
}

#[path = "support/source_json.rs"]
mod source_json;

#[test]
fn tagged_source_and_ron_keep_the_same_complete_cards() {
    corpus::on_large_stack(|| {
        source_json::assert_fixture_matches(include_str!(
            "fixtures/reference_continuous_20261111.ronl"
        ));
        source_json::assert_fixture_matches(include_str!("fixtures/reference_kogla_20261111.ronl"));
    });
}

#[test]
fn discarded_source_shuffle_uses_exact_cost_output_and_keeps_absent_branch() {
    corpus::on_large_stack(|| {
        let source: OracleCard =
            ron::from_str(include_str!("fixtures/reference_kogla_20261111.ronl")).unwrap();
        let result = unified::lower(&source);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        validate::unified(&result.card).unwrap();
        // Preserve the frozen189 fixture and use its explicit typed migration.
        let expected = mtgish_ast::migrate::read_json(include_str!(
            "fixtures/reference_kogla_20261111_expected.json"
        ))
        .unwrap();
        validate::unified(&expected).unwrap();
        let actual = canonical::mapped_unified(&result.card).unwrap();
        let wanted = canonical::mapped_unified(&expected).unwrap();
        assert_eq!(
            serde_json::to_value(&actual.card).unwrap(),
            serde_json::to_value(&wanted.card).unwrap()
        );
    });
}

#[test]
fn discarded_card_reference_rejects_plural_or_ambiguous_costs() {
    corpus::on_large_stack(|| {
        for cost in [
            "DiscardNumberCards(Integer(2))",
            "And([DiscardACard, DiscardACard])",
        ] {
            let text = format!(
                "Card(Name: \"Discard probe\", Typeline: (Supertypes: [], Cardtypes: [Creature], Subtypes: []), Rules: [FromHand(Activated({cost}, ActionList([ShuffleGraveyardCardIntoLibrary(TheCardDiscardedThisWay)])))])"
            );
            let source: OracleCard = ron::from_str(&text).unwrap();
            assert!(mtgish_ast::lower::card(&source).is_err(), "{cost}");
        }
    });
}

#[test]
fn resolution_discards_keep_the_existing_action_receipt() {
    corpus::on_large_stack(|| {
        let expected: Value = serde_json::from_str(include_str!(
            "fixtures/discard_action_controls_20261111_expected.json"
        ))
        .unwrap();
        for line in include_str!("fixtures/discard_action_controls_20261111.ronl").lines() {
            let source: OracleCard = ron::from_str(line).unwrap();
            let lowered = unified::lower(&source);
            assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
            let card = &lowered.card.faces[0];
            let wanted: Vec<Ability> =
                serde_json::from_value(expected[&card.name].clone()).unwrap();
            assert_eq!(
                normalized(&card.abilities),
                normalized(&wanted),
                "{}",
                card.name
            );
        }
    });
}
