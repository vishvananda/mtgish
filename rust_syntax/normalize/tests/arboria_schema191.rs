use mtgish_ast::{ir::*, mtg_types::OracleCard, unified, validate};

#[test]
fn frozen_arboria_matches_the_published_schema191_reading() {
    let source: OracleCard = ron::from_str(include_str!("fixtures/arboria_schema191.ronl").trim()).unwrap();
    let lowered = unified::lower(&source);
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let expected: Card = serde_json::from_str(include_str!("fixtures/arboria_schema191.ir.json")).unwrap();
    validate::card(&expected).unwrap();
    assert_eq!(serde_json::to_value(&lowered.card.faces[0]).unwrap(), serde_json::to_value(expected).unwrap());
}

#[cfg(feature = "cantrip")]
#[test]
fn arboria_event_history_reads_the_putting_player_in_the_last_completed_turn() {
    let source: OracleCard = ron::from_str(include_str!("fixtures/arboria_schema191.ronl").trim()).unwrap();
    let lowered = unified::lower(&source);
    let generated = mtgish_ast::cantrip::whole(&source, &lowered.card).unwrap();
    assert!(generated.faces[0].gaps.is_empty(), "{:?}", generated.faces[0].gaps);
    let code = &generated.faces[0].rust;
    assert!(code.contains("put_by: Some(put_by)"), "{code}");
    assert!(code.contains("r.turn < q.state().turn.number"), "{code}");
    assert!(code.contains("Some(r.turn) == turn"), "{code}");
}
