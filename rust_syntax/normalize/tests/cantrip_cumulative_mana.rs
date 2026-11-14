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

#[test]
fn finite_recipe_generates_the_whole_unicorn_without_an_opaque_mana_callback() {
    cantrip::planned::with_features(&["cumulative-upkeep-mana", "mana-bundle-choices"], || {
        let card = unicorn();
        validate::card(&card).unwrap();
        assert!(cantrip::gaps(&card).is_empty(), "{:?}", cantrip::gaps(&card));
        let code = cantrip::module(&card).unwrap();
        syn::parse_file(&code).unwrap();
        let dense: String = code.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(dense.contains("add_one_bundle_of(&[ManaPool([0,1,0,0,0,0]),ManaPool([0,1,0,0,0,1])],0).only_for(ManaRestriction::CumulativeUpkeep)"), "{code}");
        assert!(!code.contains("add_by("), "{code}");
        assert!(code.contains("Add {U}"), "{code}");
        assert!(code.contains("Add {U}{C}"), "{code}");
        assert!(!code.contains("planned!"), "{code}");
    });
}

#[test]
fn bundle_labels_stay_distinct_between_abilities() {
    cantrip::planned::with_features(&["cumulative-upkeep-mana", "mana-bundle-choices"], || {
        let mut card = unicorn();
        card.abilities.push(card.abilities[0].clone());
        let code = cantrip::module(&card).unwrap();
        syn::parse_file(&code).unwrap();
        let dense: String = code.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(dense.contains("ManaPool([0,1,0,0,0,1])],0)"), "{code}");
        assert!(dense.contains("ManaPool([0,1,0,0,0,1])],2)"), "{code}");
    });
}

#[test]
fn a_bundle_recipe_does_not_supply_an_unavailable_spending_purpose() {
    cantrip::planned::with_features(&["mana-bundle-choices"], || {
        assert!(!cantrip::gaps(&unicorn()).is_empty());
        assert!(cantrip::module(&unicorn()).is_err());
    });
}

#[test]
fn snowfall_generates_optional_conditional_mana_with_an_unknown_restricted_yield() {
    cantrip::planned::with_features(&["cumulative-upkeep-mana"], || {
        let source: OracleCard = ron::from_str(
            include_str!("fixtures/snowfall_optional_mana.ronl").trim(),
        ).unwrap();
        let card = lower::card(&source).unwrap();
        validate::card(&card).unwrap();
        assert!(cantrip::gaps(&card).is_empty(), "{:?}", cantrip::gaps(&card));
        let code = cantrip::module(&card).unwrap();
        syn::parse_file(&code).unwrap();
        let dense: String = code.chars().filter(|c| !c.is_whitespace()).collect();
        assert!(dense.contains(".mana_ability(TriggeredMana::Unstated)"), "{code}");
        assert!(!dense.contains("TriggeredMana::One(Mana::U)"), "{code}");
        assert!(dense.contains("ManaPool([0,1,0,0,0,0]),ManaPool([0,2,0,0,0,0])"), "{code}");
        assert!(code.contains("cantrip_mechanics::choose::one_of"), "{code}");
        assert!(code.contains("ManaRestriction::CumulativeUpkeep"), "{code}");
        assert!(!code.contains("planned!"), "{code}");
    });
}
