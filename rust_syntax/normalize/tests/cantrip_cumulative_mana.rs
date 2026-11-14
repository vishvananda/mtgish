#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, ir::*, lower, mtg_types::OracleCard, validate};

fn unicorn() -> Card {
    let source: OracleCard = ron::from_str(
        include_str!("fixtures/cumulative_mana.ronl").lines()
            .find(|line| line.contains("Name: \"Adarkar Unicorn\"")).unwrap(),
    ).unwrap();
    lower::card(&source).unwrap()
}

// Isolate the restriction contract from the separately unsupported {U}/{C}{U}
// bundle choice. This test must not claim the entire printed card is ready.
fn one_bundle(purpose: ManaPurpose) -> Card {
    let mut card = unicorn();
    let Ability::Activated { body, .. } = &mut card.abilities[0] else { panic!() };
    let Instruction::Effect {
        operation: Operation::AddMana { mana, restriction, .. }, ..
    } = &mut body.instructions[0] else { panic!() };
    let ManaProduction::ChooseOne(options) = mana else { panic!() };
    *mana = options[0].clone();
    *restriction = Some(ManaUse::Purpose(purpose));
    validate::card(&card).unwrap();
    card
}

#[test]
fn cumulative_restriction_uses_the_native_payment_purpose_capability() {
    cantrip::planned::with_features(&["cumulative-upkeep-mana"], || {
        let card = one_bundle(ManaPurpose::CumulativeUpkeep);
        assert!(cantrip::gaps(&card).is_empty(), "{:?}", cantrip::gaps(&card));
        let code = cantrip::module(&card).unwrap();
        syn::parse_file(&code).unwrap();
        assert!(code.contains("ManaRestriction::CumulativeUpkeep"), "{code}");
        assert!(!code.contains("planned!"), "{code}");
    });
}

#[test]
fn an_old_target_cannot_claim_restricted_cumulative_mana() {
    cantrip::planned::with_features(&[], || {
        let card = one_bundle(ManaPurpose::CumulativeUpkeep);
        assert!(!cantrip::gaps(&card).is_empty());
        let error = cantrip::module(&card).unwrap_err();
        assert_eq!(error.feature, "mana spending purpose");
    });
}

#[test]
fn cumulative_support_does_not_unlock_other_payment_purposes_or_bundle_choices() {
    cantrip::planned::with_features(&["cumulative-upkeep-mana"], || {
        let other = one_bundle(ManaPurpose::GenericCost);
        assert!(!cantrip::gaps(&other).is_empty());
        let error = cantrip::module(&other).unwrap_err();
        assert_eq!(error.feature, "mana spending purpose");
        assert!(cantrip::gaps(&unicorn()).iter().any(|gap|
            gap.feature == "mana choice among different bundles"));
    });
}
