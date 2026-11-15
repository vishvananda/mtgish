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
