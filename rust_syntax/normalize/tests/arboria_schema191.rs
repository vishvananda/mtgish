use mtgish_ast::{ir::*, mtg_types::OracleCard, unified, validate};

#[test]
fn frozen_arboria_matches_the_published_schema191_reading() {
    let source: OracleCard = ron::from_str(include_str!("fixtures/arboria_schema191.ronl").trim()).unwrap();
    let lowered = unified::lower(&source);
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let saved: serde_json::Value = serde_json::from_str(include_str!("fixtures/arboria_schema191.ir.json")).unwrap();
    let wrapper=serde_json::json!({"schema_version":191,"layout":"Card","faces":[saved],"data":[]});
    let expected = mtgish_ast::migrate::read_json(&wrapper.to_string()).unwrap().faces.remove(0);
    validate::card(&expected).unwrap();
    assert_eq!(serde_json::to_value(&lowered.card.faces[0]).unwrap(), serde_json::to_value(expected).unwrap());
}
