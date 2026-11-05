#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, ir::*, lower, mtg_types::OracleCard, validate};

fn cards() -> Vec<Card> {
    include_str!("fixtures/tapped_aura_references.ronl").lines().map(|line| {
        let source: OracleCard = ron::from_str(line).unwrap();
        let card = lower::card(&source).unwrap();
        validate::card(&card).unwrap();
        card
    }).collect()
}

#[test]
fn tapped_aura_references_the_event_land_and_moves_the_source() {
    for card in cards() {
        let Ability::Triggered { body, .. } = card.abilities.iter().find(|a| matches!(a, Ability::Triggered { .. })).unwrap() else { unreachable!() };
        let ir = format!("{body:?}");
        assert!(ir.contains("EventSubject"), "{}: {ir}", card.name);
        assert!(!ir.contains("AttachedTo"), "{}: {ir}", card.name);
        cantrip::planned::with_features(&["non-target-attachment-hosts", "wide-effect-quantities", "snow-mana", "target-original-context"], || {
            let module = cantrip::module(&card).unwrap();
            assert!(module.contains("q.can_attach_to("), "{module}");
            assert!(module.contains("item.subject.object()"), "{module}");
            assert!(module.contains("q.controller(o)"), "{module}");
            assert!(module.contains("asker.source"), "{module}");
            assert!(module.contains("q.is_on_battlefield(a)"), "{module}");
            if let Ok(dir) = std::env::var("TAPPED_AURA_PROOF_DIR") {
                let file = cantrip::pr_form::render(&module, &card.name, "", cantrip::pr_form::Characteristics::Inline).unwrap();
                std::fs::write(std::path::Path::new(&dir).join(card.name.to_ascii_lowercase().replace(' ', "_") + ".rs"), file).unwrap();
            }
            assert!(module.find("g.destroy").unwrap() < module.find("q.can_attach_to").unwrap(), "{module}");
            syn::parse_file(&module).unwrap();
        });
        cantrip::planned::with_features(&[], || {
            assert_eq!(cantrip::module(&card).unwrap_err().feature, "non-target attachment host legality");
        });
    }
}

#[test]
fn source_attachment_choice_must_be_immediate_and_scoped() {
    for escape in [false, true] {
        let mut card = cards().remove(0);
        let Ability::Triggered { body, .. } = card.abilities.iter_mut().find(|a| matches!(a, Ability::Triggered { .. })).unwrap() else { unreachable!() };
        let Instruction::Optional { body: optional, .. } = &mut body.instructions[1] else { panic!() };
        let Instruction::Group(group) = &mut optional.instructions[0] else { panic!() };
        if escape {
            let attach = group.instructions.pop().unwrap();
            optional.instructions.push(attach);
        } else {
            group.instructions.insert(1, Instruction::Effect { result: ResultId(999), operation: Operation::Draw { players: PlayerSet::All, amount: Expr::Constant(1) } });
        }
        validate::card(&card).unwrap();
        cantrip::planned::with_features(&["non-target-attachment-hosts"], || {
            assert_eq!(cantrip::module(&card).unwrap_err().feature, "non-adjacent attachment host choice");
        });
    }
}
