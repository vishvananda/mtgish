use mtgish_ast::{
    canonical, corpus,
    human::{self, document::Document},
    ir::*,
    migrate, mtg_types, unified, validate,
};
use serde_json::{json, Value};

#[test]
fn authentic_api_and_all_layout_examples_migrate_to_exact_source_with_complete_origins() {
    corpus::on_large_stack(|| {
        let sources: Vec<mtg_types::OracleCard> = include_str!("fixtures/api_layouts.ronl")
            .lines()
            .map(|l| ron::from_str(l).unwrap())
            .collect();
        let old: Vec<_> = include_str!("fixtures/api_layouts_v184.jsonl")
            .lines()
            .collect();
        assert_eq!(old.len(), 29);
        assert_eq!(sources.len(), old.len());
        let mut layouts = std::collections::BTreeSet::new();
        for (line, source) in old.into_iter().zip(sources) {
            let before: Value = serde_json::from_str(line).unwrap();
            let mapped = migrate::mapped_read_json(line)
                .unwrap_or_else(|e| panic!("{}: {e}", before["faces"]));
            assert_eq!(mapped.source_version, 184);
            assert!(mapped.map.introduced.is_empty());
            let fresh = unified::lower(&source);
            assert!(fresh.diagnostics.is_empty(), "{:?}", fresh.diagnostics);
            let after = serde_json::to_value(&mapped.card).unwrap();
            let mut expected = after.clone();
            if mapped.card.faces.first().is_some_and(|face| face.name == "Aragorn, Company Leader") {
                let condition=&mut expected["faces"][0]["abilities"][0]["args"]["intervening_if"];
                assert_eq!(condition["args"]["object"]["kind"], "RingBearer");
                *condition=json!({"kind":"RingBearerChosen","args":condition["args"]["filter"].clone()});
            }
            if mapped.card.faces.first().is_some_and(|face| face.name == "Alora, Cheerful Scout") {
                // Saved184 retains its old unreduced term. Fresh Source now
                // publishes the exact post-cost receipt and typed perpetual.
                // Only this declared semantic fix differs; migration/origins
                // below still compare the untouched historical representation.
                let op = &mut expected["faces"][0]["abilities"][0]["args"]["body"]["instructions"][1]["args"]["operation"]["args"]["body"]["instructions"][1]["args"]["then"]["instructions"][0]["args"]["operation"];
                assert_eq!(op["kind"], "EffectOn");
                *op = json!({"kind":"CreatePerpetualEffect","args":{
                    "objects":{"kind":"CostObjects","args":{"payment":{"Resolution":1},"component":[0],"view":"Performed","phase":"After"}},
                    "changes":[{"kind":"Pump","args":{"power":{"kind":"Constant","args":1},"toughness":{"kind":"Constant","args":1}}}]}});
            }
            assert_eq!(expected, serde_json::to_value(fresh.card).unwrap());
            for (from, to) in mapped.map.sites {
                for to in to {
                    assert_eq!(before.pointer(&from), after.pointer(&to), "{from} -> {to}");
                }
            }
            layouts.insert(mapped.card.layout.to_string());
            let text = human::write(&mapped.card).unwrap();
            let back = human::read(&text).unwrap();
            assert_eq!(
                serde_json::to_value(canonical::unified(&mapped.card).unwrap()).unwrap(),
                serde_json::to_value(canonical::unified(&back).unwrap()).unwrap()
            );
        }
        assert_eq!(layouts.len(), 18);
    });
}
#[test]
fn original_names_and_scope_identities_survive_role_and_counter_source_moves() {
    corpus::on_large_stack(|| {
        let mut count = 0;
        for line in include_str!("fixtures/api_layout_documents_v184.jsonl").lines() {
            let old: Value = serde_json::from_str(line).unwrap();
            let doc = Document::from_json(line)
                .unwrap_or_else(|e| panic!("{}: {e}", old["card"]["faces"]));
            assert_eq!(
                serde_json::to_value(doc.bindings()).unwrap(),
                old["bindings"]
            );
            let text = doc.write().unwrap();
            let back = mtgish_ast::human::document::read(&text).unwrap();
            let names = |d: &Document| {
                d.bindings()
                    .iter()
                    .map(|b| (&b.kind, &b.scope_name, &b.name, b.explicit))
                    .map(|b| format!("{b:?}"))
                    .collect::<Vec<_>>()
            };
            assert_eq!(names(&doc), names(&back));
            count += 1;
        }
        assert_eq!(count, 29);
    });
}
fn blank() -> UnifiedCard {
    human::read("Card{name:\"Layout contract\",properties:{card_types:[Instant]},abilities:[]}")
        .unwrap()
}
#[test]
fn known_layouts_require_their_faces_and_metadata_without_becoming_extensions() {
    let c = blank();
    for layout in Layout::KNOWN {
        let name = layout.known_name().unwrap();
        assert_eq!(Layout::from_name(name).unwrap(), layout);
        assert_eq!(serde_json::to_value(&layout).unwrap(), json!(name));
    }
    assert!(Layout::from_name("Transformng").is_err());
    let mut forged: Value = serde_json::from_str(
        include_str!("fixtures/api_layouts_v184.jsonl")
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    forged["layout"] = json!({"Extension":{"namespace":"org.example","name":"Card"}});
    assert!(migrate::read_json(&forged.to_string())
        .unwrap_err()
        .contains("legacy layout requires a string"));
    let mut v = serde_json::to_value(&c).unwrap();
    v["layout"] = json!("Transformng");
    assert!(migrate::read_json(&v.to_string()).is_err());
    for (layout, faces, data) in [
        (Layout::Transforming, vec![c.faces[0].clone()], vec![]),
        (Layout::Card, vec![c.faces[0].clone(); 2], vec![]),
        (Layout::Vanguard, c.faces.clone(), vec![]),
        (
            Layout::MeldPiece,
            c.faces.clone(),
            vec![Field {
                name: "MeldsInto".into(),
                value: Operand::Integer(2),
            }],
        ),
        (
            Layout::StickerSheet,
            vec![],
            vec![Field {
                name: "Stickers".into(),
                value: Operand::List(vec![]),
            }],
        ),
    ] {
        assert!(validate::unified(&UnifiedCard {
            schema_version: SCHEMA_VERSION,
            layout,
            faces,
            data
        })
        .is_err());
    }
}
#[test]
fn namespaced_extensions_are_explicit_and_do_not_impersonate_builtins() {
    let mut c = blank();
    c.layout = Layout::Extension {
        namespace: "org.example".into(),
        name: "Card".into(),
    };
    validate::unified(&c).unwrap();
    assert!(c.layout.known_name().is_none());
    let text = human::write(&c).unwrap();
    assert!(text.contains("Extension"));
    let back = human::read(&text).unwrap();
    assert_eq!(back.layout, c.layout);
    for namespace in ["", "builtin", "org..example", ".org", "org/example"] {
        c.layout = Layout::Extension {
            namespace: namespace.into(),
            name: "Example".into(),
        };
        assert!(validate::unified(&c).is_err());
    }
    c.layout = Layout::Extension {
        namespace: "org.example".into(),
        name: "Example".into(),
    };
    c.data = vec![
        Field {
            name: "property".into(),
            value: Operand::Integer(1)
        };
        2
    ];
    assert!(validate::unified(&c).is_err());
}
#[test]
fn current_names_are_readable_while_old_text_aliases_only_affect_input() {
    let text="Card{name:\"Roles\",properties:{card_types:[Instant]},abilities:[Spell{do:[Tap{objects:One(This),tapped:true}]}]}";
    let c = human::read(&text).unwrap();
    let new = human::write(&c).unwrap();
    assert!(new.contains("SetTapped"));
    assert!(!new.contains("operation:Tap"));
    assert!(
        matches!(&c.faces[0].abilities[0],Ability::Spell {body,..} if matches!(body.instructions[0],Instruction::Effect {operation:Operation::SetTapped {..},..}))
    );
}
#[test]
fn an_event_counter_source_cannot_be_a_damage_recipient_or_read_an_unrelated_event() {
    let c = blank();
    let mut v = serde_json::to_value(&c).unwrap();
    let spell = json!({"kind":"Spell","args":{"announcement":{"targets":[],"modes":null},"body":{"instructions":[{"kind":"Effect","args":{"result":0,"operation":{"kind":"DuplicateCounters","args":{"from":"EventPut","to":{"kind":"Objects","args":{"kind":"One","args":{"kind":"Source"}}},"copies":"All","kind":null}}}}]}}});
    v["faces"][0]["abilities"] = json!([spell]);
    let bad: UnifiedCard = serde_json::from_value(v.clone()).unwrap();
    assert!(validate::unified(&bad)
        .unwrap_err()
        .contains("counter-placement event"));
    v["faces"][0]["abilities"][0]["args"]["body"]["instructions"][0]["args"]["operation"] = json!({"kind":"DealDamage","args":{"source":{"kind":"Source"},"recipients":{"kind":"CountersTheEventPut"},"amount":{"kind":"Constant","args":1},"preventable":true}});
    assert!(serde_json::from_value::<UnifiedCard>(v).is_err());
}
