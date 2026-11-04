use mtgish_ast::{ir::*, mtg_types::OracleCard, semantics, unified, validate};

#[test]
fn singleton_replacement_has_the_same_ordered_list_as_multiple_abilities() {
    let ability = Ability::Keyword(Keyword::Flying);
    let changes = semantics::reduce_changes(
        "LayerEffect::AddAbilityAndLoseAllOtherAbilities".into(),
        vec![Operand::Ability(Box::new(ability))],
    );
    assert!(matches!(
        changes.as_slice(),
        [
            Change::RemoveAllAbilities,
            Change::GrantKeyword(Keyword::Flying)
        ]
    ));
}

#[test]
fn real_replacement_records_roundtrip_with_ordered_ordinary_changes() {
    for line in include_str!("fixtures/ability_aliases.ronl").lines() {
        let source: OracleCard = ron::from_str(line).unwrap();
        let lowered = unified::lower(&source);
        validate::unified(&lowered.card).unwrap();
        unified::round_trip(&lowered.card).unwrap();
        let value = serde_json::to_value(&lowered.card).unwrap();
        assert!(!value.to_string().contains("\"OnlyAbility\""));
        assert!(!value.to_string().contains("\"OnlyAbilities\""));
    }
}

#[cfg(feature = "cantrip")]
#[test]
fn ability_loss_and_grants_stay_ordered_for_static_and_resolving_changes() {
    for line in include_str!("fixtures/ability_aliases.ronl").lines() {
        let source: OracleCard = ron::from_str(line).unwrap();
        let lowered = unified::lower(&source);
        let card = &lowered.card.faces[0];
        if card.name == "Darksteel Mutation" {
            let code = mtgish_ast::cantrip::module(card).unwrap();
            assert!(
                code.find("loses_all_abilities(").unwrap()
                    < code.find("\"Static: GrantKeyword.\"").unwrap()
            );
        } else if card.name == "Dragonshift" {
            let code = mtgish_ast::cantrip::module(card).unwrap();
            assert_eq!(code.matches("EffectKind::LoseAllAbilities").count(), 2);
            assert_eq!(
                code.matches("EffectKind::AddKeywords(Keywords::of(&[Keyword::Flying]))")
                    .count(),
                2
            );
            assert!(
                code.find("EffectKind::LoseAllAbilities").unwrap()
                    < code
                        .find("EffectKind::AddKeywords(Keywords::of(&[Keyword::Flying]))")
                        .unwrap()
            );
        } else if card.name == "Bronzehide Lion" {
            let code = mtgish_ast::cantrip::module(card).unwrap();
            assert!(
                code.contains("aura: true, loses_other_abilities: true, abilities: &[0]"),
                "{code}"
            );
            assert!(code.contains("Keyword::Indestructible"));
        }
    }
}
