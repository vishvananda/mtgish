use mtgish_ast::{corpus, mtg_types::OracleCard, unified, validate};
use serde_json::Value;

fn operations<'a>(value: &'a Value, kind: &str, found: &mut Vec<&'a Value>) {
    if value["kind"] == kind {
        found.push(value);
    }
    match value {
        Value::Object(fields) => fields.values().for_each(|v| operations(v, kind, found)),
        Value::Array(items) => items.iter().for_each(|v| operations(v, kind, found)),
        _ => {}
    }
}

#[test]
fn land_difference_searches_preserve_fail_to_find_and_the_correct_numeric_operands() {
    corpus::on_large_stack(|| {
        for line in include_str!("fixtures/rejected_172.ronl").lines() {
            assert!(line.contains("FindNumberCardsOfType("));
            assert!(!line.contains("FindUptoNumberCardsOfType("));
            let source: OracleCard = ron::from_str(line).unwrap();
            let lowered = unified::lower(&source);
            assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
            validate::unified(&lowered.card).unwrap();
            unified::round_trip(&lowered.card).unwrap();
            let face = &lowered.card.faces[0];
            let value = serde_json::to_value(&face.abilities).unwrap();
            let mut searches = vec![];
            operations(&value, "Search", &mut searches);
            assert_eq!(searches.len(), 1, "{}", face.name);
            let count = &searches[0]["args"]["choices"][0]["count"];
            assert!(count.get("UpTo").is_some(), "{}: {count}", face.name);
            let difference = &count["UpTo"];
            assert_eq!(difference["kind"], "Subtract");
            let operands = difference["args"].as_array().unwrap();
            if face.name == "Boreas Charger" {
                for operand in operands {
                    assert_eq!(operand["kind"], "CountObjects");
                    let zone = &operand["args"];
                    assert_eq!(zone["kind"], "Zone");
                    assert_eq!(zone["args"]["scope"]["Shared"], "Battlefield");
                    assert_eq!(
                        zone["args"]["filter"]["args"][0],
                        serde_json::json!({"kind":"CardType","args":"Land"})
                    );
                }
                let owner =
                    |index: usize| &operands[index]["args"]["args"]["filter"]["args"][1]["args"];
                assert_eq!(
                    owner(1),
                    &serde_json::json!({"kind":"One","args":{"kind":"AbilityController"}})
                );
                assert_ne!(owner(0), owner(1));
            } else {
                assert_eq!(
                    operands[0],
                    serde_json::json!({"kind":"BoundValue","args":0})
                );
                assert_eq!(operands[1]["kind"], "CountObjects");
                assert_eq!(
                    operands[1]["args"]["args"]["filter"]["args"][1]["args"],
                    serde_json::json!({"kind":"One","args":{"kind":"Bound","args":0}})
                );
            }
        }
    });
}

#[test]
fn fail_to_find_does_not_make_an_unqualified_find_optional() {
    corpus::on_large_stack(|| {
        for (find, count_kind) in [
            ("FindNumberGenericCards(Integer(2))", "AsManyAsCan"),
            (
                "FindNumberCardsOfType(Integer(2), IsCardtype(Creature))",
                "UpTo",
            ),
        ] {
            let source = format!("Card(Name: \"Search control\", Typeline: (Supertypes: [], Cardtypes: [Sorcery], Subtypes: []), Rules: [SpellActions(ActionList([SearchLibrary([{find}, PutFoundCardsIntoHand, Shuffle])]))])");
            let source: OracleCard = ron::from_str(&source).unwrap();
            let lowered = unified::lower(&source);
            assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
            validate::unified(&lowered.card).unwrap();
            let value = serde_json::to_value(&lowered.card).unwrap();
            let mut searches = vec![];
            operations(&value, "Search", &mut searches);
            assert!(searches[0]["args"]["choices"][0]["count"]
                .get(count_kind)
                .is_some());
        }
    });
}
