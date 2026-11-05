#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, ir::*, lower, mtg_types::OracleCard};

fn cards() -> Vec<Card> {
    include_str!("fixtures/attachment_hosts.ronl").lines().map(|s| {
        let source: OracleCard = ron::from_str(s).unwrap();
        lower::card(&source).unwrap()
    }).collect()
}

#[test]
fn printed_hosts_are_filtered_and_capability_is_required() {
    for card in cards() {
        cantrip::planned::with_features(&[], || {
            assert_eq!(cantrip::module(&card).unwrap_err().feature, "non-target attachment host legality");
        });
        cantrip::planned::with_features(&["non-target-attachment-hosts"], || {
            let raw = cantrip::module(&card).unwrap();
            assert!(raw.contains("q.can_attach_to("), "{raw}");
            let select = raw.find("cantrip_mechanics::choose::select").unwrap();
            assert!(raw.find("q.can_attach_to(").unwrap() < select);
            syn::parse_file(&raw).unwrap();
        });
    }
}

#[test]
fn nonadjacent_host_choice_is_an_explicit_gap() {
    let mut card = cards().remove(1);
    let Ability::Activated { body, .. } = &mut card.abilities[0] else { panic!() };
    let Instruction::Group(group) = &mut body.instructions[0] else { panic!() };
    group.instructions.insert(1, Instruction::Effect { result: ResultId(999), operation: Operation::Draw { players: PlayerSet::All, amount: Expr::Constant(1) } });
    cantrip::planned::with_features(&["non-target-attachment-hosts"], || {
        assert_eq!(cantrip::module(&card).unwrap_err().feature, "non-adjacent attachment host choice");
    });
}

#[test]
fn final_pr_modules_preserve_caller_filters() {
    let names = ["Aura Graft", "Crown of the Ages", "Enchantment Alteration"];
    for (card, name) in cards().into_iter().zip(names) {
        cantrip::planned::with_features(&["non-target-attachment-hosts"], || {
            let raw = cantrip::module(&card).unwrap();
            let file = cantrip::pr_form::render(&raw, name, "", cantrip::pr_form::Characteristics::Inline).unwrap();
            assert!(file.contains("q.can_attach_to("), "{file}");
            assert!(file.contains("g.attach("), "{file}");
            assert!(file.contains("attached_to"), "{file}");
            if name == "Crown of the Ages" || name == "Enchantment Alteration" {
                assert!(file.contains("CardType::Creature"), "{file}");
            }
            if name == "Enchantment Alteration" {
                assert!(file.contains("[CardType::Creature, CardType::Land]") && !file.contains("a.types.iter()"), "{file}");
            }
            if name == "Aura Graft" {
                assert!(file.find("EffectKind::Control").unwrap() < file.find("q.can_attach_to(").unwrap(), "{file}");
            }
            syn::parse_file(&file).unwrap();
            if let Ok(dir) = std::env::var("ATTACHMENT_PROOF_DIR") {
                std::fs::create_dir_all(&dir).unwrap();
                let filename = name.to_ascii_lowercase().replace(' ', "_") + ".rs";
                std::fs::write(std::path::Path::new(&dir).join(filename), file).unwrap();
            }
        });
    }
}

#[test]
fn multiple_attachments_are_not_treated_as_one() {
    let mut card = cards().remove(1);
    let Ability::Activated { body, .. } = &mut card.abilities[0] else { panic!() };
    let Instruction::Group(group) = &mut body.instructions[0] else { panic!() };
    let Instruction::Effect { operation: Operation::Attach { attachments, .. }, .. } = &mut group.instructions[1] else { panic!() };
    *attachments = ObjectSet::Zone { scope: ZoneScope::Shared(EventZone::Battlefield), kind: ObjectKind::Permanent, filter: ObjectFilter::Any };
    cantrip::planned::with_features(&["non-target-attachment-hosts"], || {
        assert_eq!(cantrip::module(&card).unwrap_err().feature, "non-adjacent attachment host choice");
    });
}

#[test]
fn multiple_targets_are_not_rechecked_mid_resolution() {
    let mut card = cards().remove(1);
    let Ability::Activated { announcement, .. } = &mut card.abilities[0] else { panic!() };
    let mut second = announcement.targets[0].clone();
    second.slot = Slot(1);
    announcement.targets.push(second);
    cantrip::planned::with_features(&["non-target-attachment-hosts"], || {
        assert_eq!(cantrip::module(&card).unwrap_err().feature, "attachment host choice with multiple or optional targets");
    });
}

#[test]
fn group_choice_used_by_enclosing_attachment_is_a_gap() {
    let mut card = cards().remove(1);
    let Ability::Activated { body, .. } = &mut card.abilities[0] else { panic!() };
    let Instruction::Group(group) = &mut body.instructions[0] else { panic!() };
    let attach = group.instructions.pop().unwrap();
    body.instructions.push(attach);
    mtgish_ast::validate::card(&card).unwrap();
    cantrip::planned::with_features(&["non-target-attachment-hosts"], || {
        let error = cantrip::module(&card).unwrap_err();
        assert_eq!(error.feature, "non-adjacent attachment host choice");
    });
}

#[test]
fn group_choice_used_inside_later_in_order_is_a_gap() {
    let mut card = cards().remove(1);
    let Ability::Activated { body, .. } = &mut card.abilities[0] else { panic!() };
    let Instruction::Group(group) = &mut body.instructions[0] else { panic!() };
    let Instruction::Effect { result, operation } = group.instructions.pop().unwrap() else { panic!() };
    body.instructions.push(Instruction::Effect { result, operation: Operation::InOrder(vec![operation]) });
    mtgish_ast::validate::card(&card).unwrap();
    cantrip::planned::with_features(&["non-target-attachment-hosts"], || {
        assert_eq!(cantrip::module(&card).unwrap_err().feature, "non-adjacent attachment host choice");
    });
}

#[test]
fn group_choice_used_inside_later_prepared_selection_is_a_gap() {
    let mut card = cards().remove(1);
    let Ability::Activated { body, .. } = &mut card.abilities[0] else { panic!() };
    let Instruction::Group(group) = &mut body.instructions[0] else { panic!() };
    let Instruction::Effect { operation: Operation::ChooseObjects { objects, .. }, .. } = &group.instructions[0] else { panic!() };
    let pool = objects.clone();
    let Instruction::Effect { result, operation } = group.instructions.pop().unwrap() else { panic!() };
    body.instructions.push(Instruction::Effect { result, operation: Operation::WithSelections {
        steps: vec![SelectionStep { binding: SelectionId(99), chooser: SelectionChooser::Players(PlayerSet::One(PlayerRef::AbilityController)), objects: pool, count: Count::Exactly(Expr::Constant(1)), constraints: vec![] }],
        action: Box::new(operation),
    }});
    mtgish_ast::validate::card(&card).unwrap();
    cantrip::planned::with_features(&["non-target-attachment-hosts"], || {
        assert_eq!(cantrip::module(&card).unwrap_err().feature, "non-adjacent attachment host choice");
    });
}
