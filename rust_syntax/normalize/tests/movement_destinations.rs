#[path = "support/fresh_producer.rs"]
mod fresh_producer;
use mtgish_ast::{
    canonical, corpus,
    human::{self, document::Document},
    migrate, mtg_types, unified,
};
use serde_json::Value;

#[test]
fn authentic_movement_records_upgrade_to_exact_source_and_preserve_all_binding_origins() {
    corpus::on_large_stack(|| {
        let sources = include_str!("fixtures/movement_destinations.ronl").lines();
        let old = include_str!("fixtures/movement_destinations_v185.jsonl").lines();
        assert_eq!(sources.clone().count(), 87);
        for (source, old) in sources.zip(old) {
            let source: mtg_types::OracleCard = ron::from_str(source).unwrap();
            let before: Value = serde_json::from_str(old).unwrap();
            let mapped = migrate::mapped_read_json(old)
                .unwrap_or_else(|e| panic!("{}: {e}", before["faces"]));
            let fresh = unified::lower(&source);
            assert!(fresh.diagnostics.is_empty(), "{:?}", fresh.diagnostics);
            let after = serde_json::to_value(&mapped.card).unwrap();
            let mut historical_source = serde_json::to_value(&fresh.card).unwrap();
            if fresh.card.faces[0].name == "Takklemaggot" {
                // Schema188 fixes the newly produced card's inverse enchant
                // relation. Migrating a saved185 card preserves its old meaning;
                // migration must not silently apply this separate reference fix.
                fn original_relation(value: &mut Value) -> usize {
                    match value {
                        Value::Object(fields) if fields.get("kind").and_then(Value::as_str) == Some("CouldBeEnchantedBy") => {
                            assert_eq!(fields["args"], serde_json::json!({"kind":"AuraAfterEnchantedLeft","args":{"aura":{"kind":"Source"},"host":{"kind":"EventSubject"}}}));
                            *value = serde_json::json!({"kind":"CouldEnchant","args":{"kind":"One","args":{"kind":"Source"}}});
                            1
                        },
                        Value::Object(fields) => fields.values_mut().map(original_relation).sum(),
                        Value::Array(items) => items.iter_mut().map(original_relation).sum(),
                        _ => 0,
                    }
                }
                assert_eq!(original_relation(&mut historical_source), 1);
            }
            fresh_producer::assert_matches("movement_destinations_v185.jsonl", &after, &historical_source);
            assert!(mapped.map.introduced.is_empty());
            for (from, to) in mapped.map.sites {
                for to in to {
                    assert_eq!(before.pointer(&from), after.pointer(&to), "{from} -> {to}");
                }
            }
            let text = human::write(&fresh.card).unwrap();
            let back = human::read(&text).unwrap();
            assert_eq!(
                serde_json::to_value(canonical::unified(&fresh.card).unwrap()).unwrap(),
                serde_json::to_value(canonical::unified(&back).unwrap()).unwrap()
            );
        }
    });
}
#[test]
fn authentic_named_documents_retain_explicit_names_and_scope_keys() {
    corpus::on_large_stack(|| {
        for old in include_str!("fixtures/movement_documents_v185.jsonl").lines() {
            let before: Value = serde_json::from_str(old).unwrap();
            let doc = Document::from_json(old).unwrap();
            assert_eq!(
                serde_json::to_value(doc.bindings()).unwrap(),
                before["bindings"]
            );
            let back = human::document::read(&doc.write().unwrap()).unwrap();
            let names = |d: &Document| {
                d.bindings()
                    .iter()
                    .map(|b| format!("{:?} {:?} {} {}", b.kind, b.scope_name, b.name, b.explicit))
                    .collect::<Vec<_>>()
            };
            assert_eq!(names(&doc), names(&back));
        }
    });
}

#[test]
fn an_unannounced_player_cannot_hide_in_a_hand_destination() {
    let mut old: Value = serde_json::from_str(
        include_str!("fixtures/movement_destinations_v185.jsonl")
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    old["faces"][0]["abilities"] = serde_json::json!([{"kind":"Spell","args":{"announcement":{"targets":[],"modes":null},"body":{"instructions":[{"kind":"Effect","args":{"result":0,"operation":{"kind":"Move","args":{"objects":{"kind":"One","args":{"kind":"Source"}},"destination":{"Hand":{"kind":"Target","args":{"slot":99,"index":0}}},"entry":{"controller":null,"tapped":false,"counters":[],"states":[]},"face_down":false,"terms":{"order":"DestinationDefault","placement":"DestinationDefault","qualifiers":[]}}}}}]}}}]);
    let error = migrate::read_json(&old.to_string()).unwrap_err();
    assert!(error.contains("target slot 99"), "{error}");
}

#[test]
fn legacy_card_code_moves_names_to_the_typed_destination_in_their_own_scopes() {
    let ability = r#"Spell{targets:[Target{name:victim,select:Creature}],do:[
        remembered=NoteObjects{objects:Targets(victim)},
        banished=Move{objects:Selected(remembered),destination:Exile,entry:{states:[
            ExiledForWith{until:EndOfTurn,objects:Selected(remembered),body:[
                remembered=NoteObjects{objects:One(This)},Reveal{objects:Selected(remembered)}
            ]}
        ]}}
    ]}"#;
    let text=format!("Card{{name:\"Names\",properties:{{card_types:[Instant]}},abilities:[{ability},{ability}]}}");
    let doc = human::document::read(&text).unwrap();
    let code = doc.write().unwrap();
    let back = human::document::read(&code).unwrap();
    assert_eq!(code, back.write().unwrap());
    let remembered = doc
        .bindings()
        .iter()
        .filter(|b| b.name == "remembered")
        .collect::<Vec<_>>();
    assert_eq!(remembered.len(), 4);
    assert_eq!(
        remembered
            .iter()
            .map(|b| b.scope)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        4
    );
    assert!(doc.uses().iter().any(|u| u
        .path
        .contains("/destination/Exile/arrival/states/0/ExiledForWith/objects/")));
    assert!(code.contains("banished = Move"), "{code}");
}

#[test]
fn this_way_reads_retain_the_players_actual_battlefield_receipts() {
    corpus::on_large_stack(|| {
        let source = include_str!("fixtures/pipeline_generated.ronl")
            .lines()
            .find(|s| s.contains("Guild Feud"))
            .unwrap();
        let source: mtg_types::OracleCard = ron::from_str(source).unwrap();
        let fresh = unified::lower(&source);
        assert!(fresh.diagnostics.is_empty(), "{:?}", fresh.diagnostics);
        let old =
            migrate::mapped_read_json(include_str!("fixtures/movement_receipt_v185.json")).unwrap();
        assert_eq!(
            serde_json::to_value(&old.card).unwrap(),
            serde_json::to_value(&fresh.card).unwrap()
        );
        let code = human::write(&fresh.card).unwrap();
        human::read(&code).unwrap();
    });
}
