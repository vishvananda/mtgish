use mtgish_ast::{ir::*, mtg_types::OracleCard, semantics, unified, validate};
use serde_json::Value;

fn count_plural(value: &Value) -> usize {
    match value {
        Value::Object(fields) => {
            usize::from(fields.get("kind").and_then(Value::as_str) == Some("OnlyAbilities"))
                + fields.values().map(count_plural).sum::<usize>()
        }
        Value::Array(values) => values.iter().map(count_plural).sum(),
        _ => 0,
    }
}

#[test]
fn singleton_replacement_has_the_same_ordered_list_as_multiple_abilities() {
    let ability = Ability::Keyword(Keyword::Flying);
    let change = semantics::reduce_change(
        "LayerEffect::AddAbilityAndLoseAllOtherAbilities".into(),
        vec![Operand::Ability(Box::new(ability.clone()))],
    );
    let Change::OnlyAbilities(abilities) = change else {
        panic!("{change:?}")
    };
    assert_eq!(abilities.len(), 1);
    assert_eq!(
        serde_json::to_value(&abilities[0]).unwrap(),
        serde_json::to_value(ability).unwrap()
    );
}

#[test]
fn real_replacement_records_roundtrip_with_one_ability_list_form() {
    for line in include_str!("fixtures/ability_aliases.ronl").lines() {
        let source: OracleCard = ron::from_str(line).unwrap();
        let lowered = unified::lower(&source);
        validate::unified(&lowered.card).unwrap();
        unified::round_trip(&lowered.card).unwrap();
        let value = serde_json::to_value(&lowered.card).unwrap();
        assert!(!value.to_string().contains("\"OnlyAbility\""));
        assert_eq!(
            count_plural(&value),
            line.matches("AddAbilityAndLoseAllOtherAbilities(").count(),
            "{line}"
        );
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
