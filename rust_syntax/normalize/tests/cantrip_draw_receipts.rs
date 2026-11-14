#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, corpus, ir::*, lower, mtg_types::OracleCard};

const FEATURE: &str = "draw-result-receipts";

fn source(name: &str) -> OracleCard {
    include_str!("fixtures/draw_followup_receipts.ronl").lines()
        .map(|line| ron::from_str(line).unwrap())
        .find(|card| corpus::Identity::of(card).unwrap().names.iter().any(|n| n == name))
        .unwrap()
}

#[test]
fn draw_and_reveal_cards_require_the_original_draw_receipt() {
    for name in ["Sindbad", "Fa'adiyah Seer"] {
        let source = source(name);
        let card = lower::card(&source).unwrap();
        let missing = cantrip::planned::without_features(&[FEATURE], || cantrip::module(&card)).unwrap_err();
        assert!(missing.feature.contains("original-draw-card-receipts"), "{name}: {missing:?}");
        let generated = cantrip::planned::with_additional_features(&[FEATURE], || cantrip::module(&card)).unwrap();
        syn::parse_file(&generated).unwrap();
        assert!(generated.contains("g.draw_with_receipt("), "{name}: {generated}");
        assert!(!generated.contains("let held =") && !generated.contains("!held.contains"), "{name}: {generated}");
        if let Ok(folder) = std::env::var("MTGISH_DRAW_RECEIPT_OUTPUT") {
            let unified = mtgish_ast::unified::lower(&source).card;
            let whole = cantrip::planned::with_additional_features(&[FEATURE], || cantrip::whole_module(&source, &unified)).unwrap();
            std::fs::write(std::path::Path::new(&folder).join(format!("{}.rs", name.replace(' ', "_"))), whole).unwrap();
        }
    }
}

#[test]
fn an_unconsumed_draw_does_not_require_the_new_native_interface() {
    let mut card = lower::card(&source("Sindbad")).unwrap();
    let Ability::Activated { body, .. } = &mut card.abilities[0] else { panic!() };
    body.instructions.truncate(1);
    let generated = cantrip::planned::without_features(&[FEATURE], || cantrip::module(&card)).unwrap();
    assert!(generated.contains("g.draw("), "{generated}");
    assert!(!generated.contains("draw_with_receipt"), "{generated}");
}

#[test]
fn a_draw_count_keeps_actual_draw_history_without_requiring_card_receipts() {
    let mut card = lower::card(&source("Sindbad")).unwrap();
    let Ability::Activated { body, .. } = &mut card.abilities[0] else { panic!() };
    let result = match &body.instructions[0] {
        Instruction::Effect { result, .. } => result.clone(),
        _ => panic!(),
    };
    body.instructions.truncate(1);
    body.instructions.push(Instruction::Effect {
        result: ResultId(1),
        operation: Operation::GainLife {
            players: PlayerSet::One(PlayerRef::AbilityController),
            amount: Expr::Performed(result),
        },
    });
    let generated = cantrip::planned::without_features(&[FEATURE], || cantrip::module(&card)).unwrap();
    assert!(generated.contains("history().drawn(player)"), "{generated}");
    assert!(!generated.contains("draw_with_receipt") && !generated.contains("let held ="), "{generated}");
}
