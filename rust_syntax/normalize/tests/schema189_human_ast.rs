//! A schema change must preserve whole programs and binding origins, while making
//! invalid scalar alternatives unrepresentable. Keep size budgets on hot nodes.
use mtgish_ast::{canonical, corpus, ir::*, migrate, mtg_types::OracleCard, unified, validate};
use serde_json::{json, Value};
fn stable(card: &UnifiedCard) -> Value {
    validate::unified(card).unwrap();
    serde_json::to_value(canonical::unified(card).unwrap()).unwrap()
}
#[test]
fn previous_epoch_controls_preserve_native_programs_and_all_binding_origins() {
    corpus::on_large_stack(|| {
        let expected: Vec<_> = include_str!("fixtures/schema188-human-ast-controls.jsonl")
            .lines()
            .map(|line| migrate::mapped_read_json(line).unwrap())
            .collect();
        let mut seen = 0;
        for line in include_str!("fixtures/schema189-human-ast-controls.ronl").lines() {
            let source: OracleCard = ron::from_str(line).unwrap();
            let actual = unified::lower(&source);
            assert!(actual.diagnostics.is_empty(), "{:?}", actual.diagnostics);
            let name = &actual.card.faces[0].name;
            let migrated = expected
                .iter()
                .find(|m| m.card.faces[0].name == *name)
                .unwrap();
            assert_eq!(migrated.source_version, 188);
            assert!(migrated.map.introduced.is_empty(), "{name}");
            assert_eq!(stable(&actual.card), stable(&migrated.card), "{name}");
            seen += 1;
        }
        assert_eq!(seen, expected.len());
        assert_eq!(seen, 10);
    });
}
#[test]
fn alternatives_cannot_be_used_as_scalar_numbers() {
    assert!(serde_json::from_value::<Expr>(json!({"kind":"AnyOf","args":[]})).is_err());
    assert!(serde_json::from_value::<Expr>(json!({"kind":"Aggregate","args":{
        "objects":{"kind":"One","args":{"kind":"Source"}},"property":"ManaValue","aggregation":"AnyOf"
    }})).is_err());
    let predicate: NumberTest = serde_json::from_value(json!({"kind":"AnyOf","args":[
        {"kind":"Compare","args":{"comparison":"NotEqual","value":{"kind":"Constant","args":1}}},
        {"kind":"Compare","args":{"comparison":"NotEqual","value":{"kind":"Constant","args":2}}}
    ]}))
    .unwrap();
    // x != 1 OR x != 2 must not become x NOT IN {1,2} during canonicalization.
    let mut condition = serde_json::to_value(vec![Ability::Casting(CastingRule::Require(
        Condition::Compare {
            left: Expr::Constant(1),
            test: predicate,
        },
    ))])
    .unwrap();
    canonical::abilities(&mut condition).unwrap();
    let tests = &condition[0]["args"]["Require"]["args"]["test"];
    assert_eq!(tests["kind"], "AnyOf");
    assert_eq!(tests["args"].as_array().unwrap().len(), 2);
}
#[test]
#[cfg(target_pointer_width = "64")]
fn common_node_size_budgets_keep_rare_payloads_out_of_line() {
    assert!(std::mem::size_of::<Expr>() <= 96);
    assert!(std::mem::size_of::<Condition>() <= 384);
    assert!(std::mem::size_of::<ObjectFilter>() <= 168);
    assert!(std::mem::size_of::<Operation>() <= 904);
    assert!(std::mem::size_of::<Instruction>() <= 912);
    assert!(std::mem::size_of::<Ability>() <= 1040);
    assert!(std::mem::size_of::<Announcement>() <= 32);
}

#[test]
fn shared_source_comparisons_keep_alternatives_in_predicate_positions() {
    corpus::on_large_stack(|| {
        let source: OracleCard = ron::from_str("Card(Name:\"Predicate control\",Typeline:(Supertypes:[],Cardtypes:[Instant],Subtypes:[]),Rules:[SpellActions(ActionList([If(PlayerPassesFilter(You,NumCardsInHandIs(OneOf([1,2]))),[GainLife(Integer(1))])]))])").unwrap();
        let lowered = unified::lower(&source);
        assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
        validate::card(&lowered.card.faces[0]).unwrap();
        let Ability::Spell { body, .. } = &lowered.card.faces[0].abilities[0] else {
            panic!()
        };
        let Instruction::If {
            condition:
                Condition::Compare {
                    test: NumberTest::AnyOf(tests),
                    ..
                },
            ..
        } = &body.instructions[0]
        else {
            panic!("{:?}", body)
        };
        assert!(matches!(
            tests.as_slice(),
            [
                NumberTest::Compare {
                    comparison: Comparison::Equal,
                    value: Expr::Constant(1)
                },
                NumberTest::Compare {
                    comparison: Comparison::Equal,
                    value: Expr::Constant(2)
                }
            ]
        ));
    });
}
