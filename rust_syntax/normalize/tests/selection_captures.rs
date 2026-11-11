//! Preparation inputs read earlier selections before introducing local names.
use mtgish_ast::{canonical, human::document, validate};
fn card(body: &str) -> String {
    format!("Card{{name:\"Preparation captures\",properties:{{card_types:[Instant]}},abilities:[Spell{{do:[{body}]}}]}}")
}
#[test]
fn chooser_references_follow_their_prepared_value_when_hoisted() {
    let text=card("Together([WithSelections{steps:[{binding:s17,chooser:Players(You),objects:One(This),count:Exactly(1),constraints:[]},{binding:s99,chooser:Players(Matching{binding:decider,condition:Compare{left:CountObjects(Prepared(s17)),test:Compare{comparison:Greater,value:0}}}),objects:One(This),count:Exactly(1),constraints:[]}],action:NoteObjects{objects:Prepared(s99)}}])");
    let doc = document::read(&text).unwrap();
    let normalized = canonical::card(&doc.card().faces[0]).unwrap();
    validate::card(&normalized).unwrap();
    let value = serde_json::to_value(normalized).unwrap();
    let steps = &value["abilities"][0]["args"]["body"]["instructions"][0]["args"]["operation"]
        ["args"]["steps"];
    assert_eq!(steps[0]["binding"], 0);
    assert_eq!(
        steps[1]["chooser"]["Players"]["args"]["condition"]["args"]["left"]["args"]["args"],
        0
    );
}
#[test]
fn a_shadowing_preparation_reads_outer_names_in_its_inputs() {
    let text=card("WithSelections{steps:[{binding:picked,chooser:Players(You),objects:One(This),count:Exactly(1),constraints:[]}],action:WithSelections{steps:[{binding:picked,chooser:Players(You),objects:Prepared(picked),count:Exactly(1),constraints:[]}],action:NoteObjects{objects:Prepared(picked)}}}");
    let mut doc = document::read(&text).unwrap();
    let bindings: Vec<_> = doc
        .bindings()
        .iter()
        .enumerate()
        .filter(|(_, b)| b.kind == document::BindingKind::Selection)
        .map(|(i, b)| (i, b.scope))
        .collect();
    assert_eq!(bindings.len(), 2);
    assert_ne!(bindings[0].1, bindings[1].1);
    doc.rename(bindings[0].0, "outer_cards").unwrap();
    doc.rename(bindings[1].0, "inner_cards").unwrap();
    let code = doc.write().unwrap();
    assert!(code.contains("objects: Prepared(outer_cards)"));
    assert!(code.contains("objects: Prepared(inner_cards)"));
}

#[test]
fn hoisting_avoids_capturing_an_inherited_selection_in_a_sibling() {
    let text=card("WithSelections{steps:[{binding:s0,chooser:Players(You),objects:One(This),count:Exactly(1),constraints:[]}],action:Together([WithSelections{steps:[{binding:s17,chooser:Players(You),objects:Prepared(s0),count:Exactly(1),constraints:[]}],action:NoteObjects{objects:Prepared(s17)}},NoteObjects{objects:Prepared(s0)}])}");
    let doc = document::read(&text).unwrap();
    let normalized = canonical::card(&doc.card().faces[0]).unwrap();
    validate::card(&normalized).unwrap();
    let code = document::from_card(&mtgish_ast::ir::UnifiedCard {
        faces: vec![normalized],
        ..doc.card().clone()
    })
    .unwrap()
    .write()
    .unwrap();
    assert!(code.contains("objects: Prepared(s0)"), "{code}");
    assert!(code.contains("objects: Prepared(s1)"), "{code}");
    assert_eq!(code.matches("Prepared(s0)").count(), 2, "{code}");
}

#[test]
fn hoisting_renames_outer_captures_without_capturing_nested_local_reads() {
    let text=card("Together([WithSelections{steps:[{binding:s17,chooser:Players(You),objects:One(This),count:Exactly(1),constraints:[]}],action:InOrder([WithSelections{steps:[{binding:s0,chooser:Players(You),objects:Prepared(s17),count:Exactly(1),constraints:[]}],action:InOrder([NoteObjects{objects:Prepared(s17)},NoteObjects{objects:Prepared(s0)}])}])}])");
    let doc = document::read(&text).unwrap();
    let normalized = canonical::card(&doc.card().faces[0]).unwrap();
    validate::card(&normalized).unwrap();
    let code = document::from_card(&mtgish_ast::ir::UnifiedCard {
        faces: vec![normalized],
        ..doc.card().clone()
    })
    .unwrap()
    .write()
    .unwrap();
    assert_eq!(code.matches("Prepared(s0)").count(), 2, "{code}");
    assert_eq!(code.matches("Prepared(s1)").count(), 1, "{code}");
}

#[test]
fn nested_hoisting_finishes_in_one_canonicalization() {
    let text=card("Together([WithSelections{steps:[{binding:first,chooser:Players(You),objects:One(This),count:Exactly(1),constraints:[]}],action:WithSelections{steps:[{binding:second,chooser:Players(You),objects:Prepared(first),count:Exactly(1),constraints:[]}],action:NoteObjects{objects:Prepared(second)}}}])");
    let doc = document::read(&text).unwrap();
    let first = canonical::card(&doc.card().faces[0]).unwrap();
    validate::card(&first).unwrap();
    let second = canonical::card(&first).unwrap();
    assert_eq!(
        serde_json::to_value(first).unwrap(),
        serde_json::to_value(second).unwrap()
    );
}

#[test]
fn hoisting_keeps_selection_captures_in_a_delayed_program() {
    let text=card("Together([WithSelections{steps:[{binding:s17,chooser:Players(You),objects:One(This),count:Exactly(1),constraints:[]}],action:CreateDelayedTrigger{event:Turn{point:EndStepBegins,players:All},start:Immediately,expires:None,intervening_if:None,do:[NoteObjects{objects:Prepared(s17)}]}}])");
    let doc = document::read(&text).unwrap();
    let normalized = canonical::card(&doc.card().faces[0]).unwrap();
    validate::card(&normalized).unwrap();
}
