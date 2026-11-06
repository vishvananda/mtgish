use mtgish_ast::{corpus, ir::*, mtg_types as source, unified, validate};

pub fn card(name: &str) -> Card {
    corpus::on_large_stack(|| {
        let source: source::OracleCard = include_str!("../fixtures/reference_followup.ronl")
            .lines()
            .filter(|line| line.contains(&format!("Name: {name:?}")))
            .map(|line| ron::from_str(line).unwrap())
            .find(|card| {
                corpus::Identity::of(card)
                    .unwrap()
                    .names
                    .iter()
                    .any(|n| n == name)
            })
            .unwrap_or_else(|| panic!("missing fixture {name}"));
        let lowered = unified::lower(&source);
        assert!(
            lowered.diagnostics.is_empty(),
            "{name}: {:?}",
            lowered.diagnostics
        );
        let card = lowered
            .card
            .faces
            .into_iter()
            .find(|c| c.name == name)
            .unwrap();
        validate::card(&card).unwrap();
        card
    })
}
