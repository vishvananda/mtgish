use mtgish_ast::{
    change_lists,
    ir::*,
    mtg_types::{CreatureType, OracleCard},
    unified, validate,
};
use serde_json::json;

fn add(types: &[CreatureType]) -> Change {
    Change::CreatureTypes {
        mode: ChangeMode::Add,
        types: CreatureTypeSet::Listed(types.to_vec()),
    }
}

#[test]
fn adjacent_additions_preserve_first_seen_order_and_are_idempotent() {
    use CreatureType::*;
    let normalized = change_lists::coalesce_creature_type_additions(vec![
        add(&[Phyrexian, Phyrexian]),
        add(&[Blinkmoth, Phyrexian]),
        add(&[Insect]),
    ]);
    let expected = vec![add(&[Phyrexian, Blinkmoth, Insect])];
    assert_eq!(
        serde_json::to_value(&normalized).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
    assert_eq!(
        serde_json::to_value(change_lists::coalesce_creature_type_additions(
            normalized.clone()
        ))
        .unwrap(),
        serde_json::to_value(normalized).unwrap()
    );
}

#[test]
fn set_all_removal_and_intervening_changes_are_barriers() {
    use CreatureType::*;
    let barriers = vec![
        Change::CreatureTypes {
            mode: ChangeMode::Set,
            types: CreatureTypeSet::Listed(vec![Monk]),
        },
        Change::CreatureTypes {
            mode: ChangeMode::Set,
            types: CreatureTypeSet::All,
        },
        Change::CreatureTypes {
            mode: ChangeMode::Add,
            types: CreatureTypeSet::All,
        },
        Change::RemoveSubtype(mtgish_ast::mtg_types::SubType::Insect),
        Change::Pump {
            power: Expr::Constant(1),
            toughness: Expr::Constant(1),
        },
    ];
    for barrier in barriers {
        let changes = vec![add(&[Insect]), barrier, add(&[Monk])];
        let expected = serde_json::to_value(&changes).unwrap();
        assert_eq!(
            serde_json::to_value(change_lists::coalesce_creature_type_additions(changes)).unwrap(),
            expected
        );
    }
}

#[test]
fn nested_changes_normalize_independently_without_crossing_effect_boundaries() {
    use CreatureType::*;
    let changes = vec![
        add(&[Insect]),
        Change::Copiable(vec![add(&[Monk]), add(&[Insect])]),
        Change::Continuing {
            changes: vec![add(&[Phyrexian]), add(&[Blinkmoth])],
            until: Duration::EndOfTurn,
        },
        add(&[Monk]),
    ];
    let expected = vec![
        add(&[Insect]),
        Change::Copiable(vec![add(&[Monk, Insect])]),
        Change::Continuing {
            changes: vec![add(&[Phyrexian, Blinkmoth])],
            until: Duration::EndOfTurn,
        },
        add(&[Monk]),
    ];
    assert_eq!(
        serde_json::to_value(change_lists::coalesce_creature_type_additions(changes)).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
}

#[test]
fn inkmoth_and_nantuko_publish_combined_additions_without_other_tree_changes() {
    for (line, before) in include_str!("fixtures/adjacent_type_additions.ronl")
        .lines()
        .zip(include_str!("fixtures/adjacent_type_additions.before.ronl").lines())
    {
        let source: OracleCard = ron::from_str(line).unwrap();
        let lowered = unified::lower(&source);
        assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
        validate::unified(&lowered.card).unwrap();
        unified::round_trip(&lowered.card).unwrap();
        let json = serde_json::to_value(&lowered.card).unwrap();
        let changes = json
            .pointer("/faces/0/abilities/1/args/body/instructions/0/args/operation/args/changes")
            .unwrap()
            .as_array()
            .unwrap();
        let additions: Vec<_> = changes
            .iter()
            .filter(|c| c["kind"] == "CreatureTypes" && c["args"]["mode"] == "Add")
            .collect();
        assert_eq!(additions.len(), 1);
        let expected = if line.contains("Inkmoth Nexus") {
            json!({"Listed":["Phyrexian", "Blinkmoth"]})
        } else {
            json!({"Listed":["Insect", "Monk"]})
        };
        assert_eq!(additions[0]["args"]["types"], expected);
        // Compare the entire schema-168 producer tree. Only the schema stamp
        // and these two adjacent Add nodes change; costs, references, timing,
        // creature/card types and all other animation properties stay exact.
        let before: UnifiedCard = ron::from_str(before).unwrap();
        let mut expected = serde_json::to_value(before).unwrap();
        expected["schema_version"] = json!(SCHEMA_VERSION);
        expected["faces"][0]["schema_version"] = json!(SCHEMA_VERSION);
        let changes = expected
            .pointer_mut(
                "/faces/0/abilities/1/args/body/instructions/0/args/operation/args/changes",
            )
            .unwrap()
            .as_array_mut()
            .unwrap();
        let index = changes
            .iter()
            .position(|c| c["kind"] == "CreatureTypes" && c["args"]["mode"] == "Add")
            .unwrap();
        let second = changes.remove(index + 1);
        let additions = second["args"]["types"]["Listed"].as_array().unwrap();
        changes[index]["args"]["types"]["Listed"]
            .as_array_mut()
            .unwrap()
            .extend(additions.iter().cloned());
        assert_eq!(json, expected);
    }
}

#[test]
fn static_layer_lists_share_the_same_canonicalization() {
    let source: OracleCard = ron::from_str(r#"Card(Name: "Static type additions", Typeline: (Supertypes: [], Cardtypes: [Creature], Subtypes: []), Rules: [PermanentLayerEffect(ThisPermanent, [AddCreatureType(Insect), AddCreatureType(Monk)])], CardPT: (Power: 1, Toughness: 1))"#).unwrap();
    let card = unified::lower(&source);
    assert!(card.diagnostics.is_empty(), "{:?}", card.diagnostics);
    let value = serde_json::to_value(card.card).unwrap();
    let changes = value
        .pointer("/faces/0/abilities/0/args/changes")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(
        changes[0]["args"]["types"],
        json!({"Listed":["Insect", "Monk"]})
    );
}
