#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, ir::*, lower, mtg_types::OracleCard};

const FEATURES: &[&str] = &["wide-effect-quantities", "simultaneous-replacement-damage"];

fn eye() -> Card {
    let source: OracleCard =
        ron::from_str(include_str!("fixtures/replacement_damage.ronl").trim()).unwrap();
    lower::card(&source).unwrap()
}

// Exercise the printed and granted callback signatures with the same typed
// damage program, without a chosen-source capture masking a diagnostic.
fn printed_replacement() -> Card {
    let mut card = eye();
    let Ability::Spell { body, .. } = &card.abilities[0] else {
        panic!()
    };
    let Instruction::Effect {
        operation: Operation::CreateReplacement { event, body, .. },
        ..
    } = &body.instructions[1]
    else {
        panic!()
    };
    let mut event = event.clone();
    let ReplacementEvent::Damage(damage) = &mut event else {
        panic!()
    };
    damage.sources = ObjectFilter::Any;
    card.abilities = vec![Ability::Replacement {
        active: ReplacementActive::Battlefield,
        event,
        body: body.clone(),
    }];
    card
}

#[test]
fn printed_eye_adds_damage_to_the_replaced_event_in_final_pr_form() {
    cantrip::planned::with_features(FEATURES, || {
        let raw = cantrip::module(&eye()).unwrap();
        let file = cantrip::pr_form::render(
            &raw,
            "Eye for an Eye",
            "",
            cantrip::pr_form::Characteristics::Inline,
        )
        .unwrap();
        assert!(file.contains(".with_damage("), "{file}");
        assert!(file.contains("DamageComponent::Additional"), "{file}");
        assert!(
            file.contains("amount: i64") || file.contains("let amount ="),
            "{file}"
        );
        assert!(!file.contains(".then("), "{file}");
        assert!(!file.contains("g.damage"), "{file}");
        syn::parse_file(&file).unwrap();
        let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join("replacement-damage-generated");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("eye_for_an_eye.rs"), file).unwrap();
    });
}

#[test]
fn wide_damage_alone_does_not_claim_simultaneous_expansion_support() {
    cantrip::planned::with_features(&["wide-effect-quantities"], || {
        assert!(cantrip::module(&eye()).is_err());
        assert_eq!(
            cantrip::module(&printed_replacement()).unwrap_err().feature,
            "simultaneous additional replacement damage"
        );
    });
}

#[test]
fn printed_replacement_has_the_pure_callback_without_an_affected_parameter() {
    cantrip::planned::with_features(FEATURES, || {
        let raw = cantrip::module(&printed_replacement()).unwrap();
        assert!(raw.contains(".with_damage("), "{raw}");
        assert!(!raw.contains("affected: ObjectId"), "{raw}");
        assert!(!raw.contains(".then("), "{raw}");
        syn::parse_file(&raw).unwrap();
    });
}

#[test]
fn mixed_mutating_work_is_not_silently_moved_into_a_pure_component_callback() {
    let mut card = printed_replacement();
    let Ability::Replacement { body, .. } = &mut card.abilities[0] else {
        panic!()
    };
    body.instructions.push(Instruction::Effect {
        result: ResultId(99),
        operation: Operation::Draw {
            players: PlayerSet::All,
            amount: Expr::Constant(1),
        },
    });
    cantrip::planned::with_features(FEATURES, || {
        assert_eq!(
            cantrip::module(&card).unwrap_err().feature,
            "additional replacement damage mixed with other instructions"
        );
    });
}

#[test]
fn a_group_keeps_additional_damage_simultaneous() {
    let mut card = printed_replacement();
    let Ability::Replacement { body, .. } = &mut card.abilities[0] else {
        panic!()
    };
    body.instructions = vec![Instruction::Group(body.clone())];
    cantrip::planned::with_features(FEATURES, || {
        let raw = cantrip::module(&card).unwrap();
        assert!(raw.contains(".with_damage("), "{raw}");
        assert!(!raw.contains(".then("), "{raw}");
    });
}
