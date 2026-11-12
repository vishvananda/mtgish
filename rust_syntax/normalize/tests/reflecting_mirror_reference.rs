use mtgish_ast::{ir::*,mtg_types::OracleCard,unified,validate};
#[test]
fn mirror_target_does_not_equate_mana_value_with_twice_that_value() {
    let source:OracleCard=ron::from_str(include_str!("fixtures/reflecting_mirror_20261112.ronl").trim()).unwrap();
    let result=unified::lower(&source);assert!(result.diagnostics.is_empty());
    let face=&result.card.faces[0];validate::card(face).unwrap();
    let Ability::Activated {announcement,limits,body,..}=&face.abilities[0] else {panic!()};
    assert_eq!(announcement.targets.len(),1);
    assert!(matches!(announcement.targets[0].filter,ObjectFilter::OnStack(StackTrait::SingleTarget)));
    let [Change::ActivationRule(ActivationRule::XIs(NumberTest::Compare {comparison:Comparison::Equal,value:Expr::Multiply(a,b)}))]=limits.modifiers.as_slice() else {panic!("{:?}",limits)};
    assert!(matches!(**a,Expr::Constant(2)));
    assert!(matches!(**b,Expr::ObjectProperty {object:ObjectRef::Target {index:0,..},property:Property::ManaValue,..}));
    let body=serde_json::to_string(body).unwrap();assert!(body.contains("TargetsAny"));assert!(body.contains("Retarget"));
}
