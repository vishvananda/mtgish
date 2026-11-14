use mtgish_ast::{corpus, mtg_types::OracleCard, unified, validate};
use serde_json::Value;

#[test]
fn all_published_021_amendment_operands_are_produced_and_validate() {
    corpus::on_large_stack(|| {
        let changes: Value =
            serde_json::from_str(include_str!("fixtures/grammar_021_changes.json")).unwrap();
        let mut checked = 0;
        let mut faces = 0;
        let mut errors = Vec::new();
        for line in include_str!("fixtures/grammar_021.ronl").lines() {
            let source: OracleCard = ron::from_str(line).unwrap();
            let lowered = unified::lower(&source);
            assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
            validate::unified(&lowered.card).unwrap();
            unified::round_trip(&lowered.card).unwrap();
            for face in &lowered.card.faces {
                let abilities = serde_json::to_value(&face.abilities).unwrap();
                let before = checked;
                for group in changes.as_array().unwrap() {
                    for change in group["changes"].as_array().unwrap() {
                        if change["face"] != face.name {
                            continue;
                        }
                        let mut actual = &abilities;
                        for part in change["path"].as_array().unwrap() {
                            actual = if let Some(index) = part.as_u64() {
                                &actual[index as usize]
                            } else {
                                &actual[part.as_str().unwrap()]
                            };
                        }
                        if actual != &change["to"] {
                            errors.push(format!("{}: {} {:?}: {actual:?} != {:?}",
                                face.name, group["id"], change["path"], change["to"]));
                        }
                        checked += 1;
                    }
                }
                faces += usize::from(checked > before);
            }
        }
        assert_eq!((faces, checked), (106, 122));
        assert!(errors.is_empty(), "{}", errors.join("\n"));
    });
}

#[test]
fn bison_whistle_keeps_the_any_card_graveyard_choice_after_declining_other_options() {
    corpus::on_large_stack(|| {
        let source: OracleCard =
            ron::from_str(include_str!("fixtures/bison_whistle_169.ronl").trim()).unwrap();
        let lowered = unified::lower(&source);
        assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
        validate::unified(&lowered.card).unwrap();
        unified::round_trip(&lowered.card).unwrap();
        let ability = serde_json::to_value(&lowered.card.faces[0].abilities[0]).unwrap();
        let body = &ability["args"]["body"]["instructions"];
        assert_eq!(body[0]["args"]["operation"]["kind"], "LookAt");
        assert_eq!(body[1]["kind"], "ChooseOne");
        let choice = &body[1]["args"];
        assert!(choice["offered"].is_null());
        let options = choice["options"].as_array().unwrap();
        assert_eq!(options.len(), 4);
        let operation = |index: usize| &options[index]["instructions"][0]["args"]["operation"];
        assert_eq!(
            operation(0)["args"]["cards"],
            serde_json::json!({"kind":"Subtype","args":"Bison"})
        );
        assert_eq!(
            operation(1)["args"]["cards"],
            serde_json::json!({"kind":"CardType","args":"Creature"})
        );
        assert_eq!(operation(2)["kind"], "Move");
        assert_eq!(
            operation(2)["args"]["objects"]["args"]["objects"]["args"]["filter"]["kind"],
            "Any"
        );
        assert!(operation(2)["args"]["destination"]
            .get("OwnersGraveyard")
            .is_some());
        assert_eq!(options[3]["instructions"][0]["kind"], "Group");
    });
}
