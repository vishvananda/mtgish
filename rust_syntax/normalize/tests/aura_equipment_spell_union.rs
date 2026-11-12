use mtgish_ast::{
    corpus,
    ir::*,
    mtg_types::{OracleCard, SubType},
    unified, validate,
};

const FIXTURE: &str = include_str!("fixtures/aura_equipment_spell_union.ronl");

fn assert_union(filter: &ObjectFilter) {
    let ObjectFilter::AnyOf(alternatives) = filter else {
        panic!("Aura and Equipment must be alternatives: {filter:?}")
    };
    assert_eq!(alternatives.len(), 2);
    assert!(matches!(
        alternatives[0],
        ObjectFilter::Subtype(SubType::Aura)
    ));
    assert!(matches!(
        alternatives[1],
        ObjectFilter::Subtype(SubType::Equipment)
    ));
}

#[test]
fn mana_cost_and_timing_rules_accept_either_spell_class() {
    corpus::on_large_stack(|| {
        let mut checked = 0;
        for line in FIXTURE.lines() {
            let source: OracleCard = ron::from_str(line).unwrap();
            let lowered = unified::lower(&source);
            assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
            validate::unified(&lowered.card).unwrap();
            unified::round_trip(&lowered.card).unwrap();
            let card = &lowered.card.faces[0];
            match card.name.as_str() {
                "Codsworth, Handy Helper" => {
                    let Ability::Activated { body, .. } = &card.abilities[1] else {
                        panic!()
                    };
                    let Instruction::Effect {
                        operation:
                            Operation::AddMana {
                                restriction: Some(ManaUse::CastSpell(filter)),
                                ..
                            },
                        ..
                    } = &body.instructions[0]
                    else {
                        panic!()
                    };
                    assert_union(filter);
                    checked += 1;
                }
                "Danitha Capashen, Paragon" | "Golden-Tail Trainer" | "Sigarda's Aid" => {
                    let changes = card
                        .abilities
                        .iter()
                        .find_map(|ability| match ability {
                            Ability::Static { changes, .. } => Some(changes),
                            _ => None,
                        })
                        .unwrap();
                    let [Change::PlayerRule(rule)] = changes.as_slice() else {
                        panic!()
                    };
                    let filter = match rule {
                        PlayerRule::SpellCost { spells, .. }
                        | PlayerRule::SpellTiming { spells, .. } => spells,
                        _ => panic!("{rule:?}"),
                    };
                    assert_union(filter);
                    checked += 1;
                }
                _ => {} // Complete matching controls are checked by the parser regression.
            }
        }
        assert_eq!(checked, 4);
    });
}

#[path = "support/source_json.rs"]
mod source_json;

#[test]
fn tagged_source_json_matches_the_complete_source_records() {
    corpus::on_large_stack(|| source_json::assert_fixture_matches(FIXTURE));
}
