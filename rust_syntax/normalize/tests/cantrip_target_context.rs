#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, ir::*, lower, mtg_types::OracleCard};

#[test]
fn ordinary_recipient_choices_compile_for_both_target_request_shapes() {
    let mut card = include_str!("fixtures/core.ronl")
        .lines()
        .map(|line| ron::from_str::<OracleCard>(line).unwrap())
        .map(|source| lower::card(&source).unwrap())
        .find(|card| card.name == "Lightning Bolt")
        .unwrap();
    let Ability::Spell { body, .. } = &mut card.abilities[0] else { panic!() };
    body.instructions = vec![Instruction::Effect {
        result: ResultId(0),
        operation: Operation::ChooseValue {
            choosers: PlayerSet::One(PlayerRef::AbilityController),
            kind: ValueKind::Recipient,
            terms: ChoiceTerms::ordinary(1, vec![Operand::Recipients(Recipients::Players(PlayerSet::All))]),
        },
    }];
    for modern in [false, true] {
        let features: &[&str] = if modern {
            &["snow-mana", "target-original-context"]
        } else {
            &["snow-mana"]
        };
        let module = cantrip::planned::with_features(features, || cantrip::module(&card)).unwrap();
        syn::parse_file(&module).unwrap();
        assert!(module.contains("RequestKind::ChooseTarget"), "{module}");
        assert_eq!(module.contains("replacing: None"), modern, "{module}");
        assert!(module.contains("optional: false") && module.contains("least: 1"), "{module}");
        if let Ok(folder) = std::env::var("TARGET_CONTEXT_PROOF_DIR") {
            let path = std::path::Path::new(&folder);
            std::fs::create_dir_all(path).unwrap();
            std::fs::write(path.join(if modern { "modern.rs" } else { "legacy.rs" }), module).unwrap();
        }
    }
}
