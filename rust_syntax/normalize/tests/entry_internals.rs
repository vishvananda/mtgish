use mtgish_ast::{canonical, human, migrate, mtg_types, unified, validate};
use serde_json::{json, Value};

fn card(operation: &str) -> String {
    format!("Card{{name:\"Entry values\",properties:{{card_types:[Instant]}},abilities:[Spell{{do:[{operation}]}}]}}")
}
#[test]
fn entry_values_use_typed_expressions_abilities_and_programs() {
    let text=card("size=BindValue{value:5},TurnFaceDownAs{objects:One(This),as_creature:{power:BoundValue(size),toughness:3,creature_type:Some(Zombie),artifact:true}}");
    let doc = human::document::read(&text).unwrap();
    assert!(doc.write().unwrap().contains("BoundValue(size)"));
    for wrong in [
        "TurnFaceDownAs{objects:One(This),as_creature:{power_toughness:Integer(42),creature_type:None,artifact:false}}",
        "MoveObjects{objects:One(This),destination:OwnersBattlefield,entry:{states:[FaceDownLand(Objects(Targets(t99)))]}}",
        "MoveObjects{objects:One(This),destination:OwnersBattlefield,entry:{states:[Optional([Integer(42)])]}}",
        "MoveObjects{objects:One(This),destination:Exile,entry:{states:[ExiledForWith{until:EndOfTurn,objects:One(This),do:Integer(42)}]}}",
        "MoveObjects{objects:One(This),destination:OwnersBattlefield,entry:{states:[Draft{name:\"EnterFlag::EntersFaceDown\",operands:[]}]}}",
    ] { assert!(human::read(&card(wrong)).is_err(),"{wrong}"); }
    let quoted=card("size=BindValue{value:4},MoveObjects{objects:One(This),destination:OwnersBattlefield,entry:{states:[FaceDownWith{face_down:{power:3,toughness:3,artifact:false},abilities:[Spell{do:[DrawCards{players:You,amount:BoundValue(size)}]}],noted:None}]}}");
    assert!(human::read(&quoted).is_err());
    human::read(&quoted.replace("BoundValue(size)", "2")).unwrap();
}

#[test]
fn all_captured_entry_payloads_upgrade_exactly_to_fresh_source() {
    mtgish_ast::corpus::on_large_stack(|| {
        let mut count = 0;
        for old in include_str!("fixtures/entry_internals_v181.jsonl").lines() {
            // Source capture order is not a semantic key; use its recorded name.
            let before: Value = serde_json::from_str(old).unwrap();
            let name = before["faces"][0]["name"].as_str().unwrap();
            let source = include_str!("fixtures/entry_internals.ronl")
                .lines()
                .find(|line| line.contains(&format!("Name: {}", json!(name))))
                .unwrap();
            let source: mtg_types::OracleCard = ron::from_str(source).unwrap();
            let expected = unified::lower(&source).card;
            let mapped = migrate::mapped_read_json(old).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(mapped.source_version, 181);
            assert!(mapped.map.introduced.is_empty());
            let after = serde_json::to_value(&mapped.card).unwrap();
            assert_eq!(after, serde_json::to_value(&expected).unwrap(), "{name}");
            for (origin, targets) in &mapped.map.sites {
                for target in targets {
                    assert_eq!(
                        before.pointer(origin),
                        after.pointer(target),
                        "{name}: {origin} -> {target}"
                    );
                }
            }
            validate::unified(&mapped.card).unwrap();
            let code = human::write(&mapped.card).unwrap();
            let read = human::read(&code).unwrap();
            assert_eq!(
                serde_json::to_value(canonical::mapped_unified(&read).unwrap().card).unwrap(),
                serde_json::to_value(canonical::mapped_unified(&mapped.card).unwrap().card)
                    .unwrap(),
                "{name}"
            );
            count += 1;
        }
        assert_eq!(count, 16);
    });
}

#[test]
fn every_captured_named_document_retains_its_identities_and_scope_names() {
    mtgish_ast::corpus::on_large_stack(|| {
        for old in include_str!("fixtures/entry_documents_v181.jsonl").lines() {
            let value: Value = serde_json::from_str(old).unwrap();
            let name = &value["card"]["faces"][0]["name"];
            let doc =
                human::document::Document::from_json(old).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(
                serde_json::to_value(doc.bindings()).unwrap(),
                value["bindings"],
                "{name}"
            );
            let text = doc.write().unwrap();
            let back = human::document::read(&text).unwrap();
            assert_eq!(
                serde_json::to_value(canonical::mapped_unified(doc.card()).unwrap().card).unwrap(),
                serde_json::to_value(canonical::mapped_unified(back.card()).unwrap().card).unwrap()
            );
            let saved = serde_json::to_string(&doc).unwrap();
            assert_eq!(
                human::document::Document::from_json(&saved)
                    .unwrap()
                    .write()
                    .unwrap(),
                text
            );
        }
    });
}

fn shared_size() -> (Value, String) {
    let mut card: Value = include_str!("fixtures/entry_internals_v181.jsonl")
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .find(|v| v["faces"][0]["name"] == "Cyber Conversion")
        .unwrap();
    let body = card["faces"][0]["abilities"][0]["args"]["body"]["instructions"]
        .as_array_mut()
        .unwrap();
    body.insert(0,json!({"kind":"Effect","args":{"result":17,"operation":{"kind":"BindValue","args":{"value":{"kind":"Constant","args":5}}}}}));
    let path =
        "/faces/0/abilities/0/args/body/instructions/1/args/operation/args/as_creature".to_string();
    card.pointer_mut(&path).unwrap()["power_toughness"] = json!({"kind":"Term","args":{"family":"PT","form":"PTX","operands":[{"kind":"Vocabulary","args":{"family":"PTXValue","form":"X"}},{"kind":"Vocabulary","args":{"family":"PTXValue","form":"X"}},{"kind":"Number","args":{"kind":"BoundValue","args":17}}]}});
    (card, path)
}
#[test]
fn shared_face_down_expressions_get_explicit_cloned_origins() {
    let (old, path) = shared_size();
    let mapped = migrate::mapped_read_json(&old.to_string()).unwrap();
    let after = serde_json::to_value(mapped.card).unwrap();
    let origin = format!("{path}/power_toughness/args/operands/2/args/args");
    assert_eq!(mapped.map.sites[&origin].len(), 2);
    for target in &mapped.map.sites[&origin] {
        assert_eq!(after.pointer(target), Some(&json!(17)));
    }
    assert_eq!(
        after.pointer(&format!("{path}/power")),
        Some(&json!({"kind":"BoundValue","args":17}))
    );
    assert_eq!(
        after.pointer(&format!("{path}/toughness")),
        Some(&json!({"kind":"BoundValue","args":17}))
    );
}

#[test]
fn optional_entries_preserve_numeric_reads_and_only_the_final_controller_origin() {
    let (mut old, _) = shared_size();
    let announcement = &mut old["faces"][0]["abilities"][0]["args"]["announcement"];
    announcement["targets"][0]["kind"] = json!("Player");
    announcement["targets"][0]["filter"] = json!({"kind":"Any"});
    let mut second = announcement["targets"][0].clone();
    second["slot"] = json!(1);
    announcement["targets"].as_array_mut().unwrap().push(second);
    let controller = |slot| json!({"kind":"Term","args":{"family":"EnterFlag","form":"EntersUnderPlayersControl","operands":[{"kind":"Player","args":{"kind":"Target","args":{"slot":slot,"index":0}}}]}});
    old["faces"][0]["abilities"][0]["args"]["body"]["instructions"][1]["args"]["operation"] = json!({"kind":"Move","args":{"objects":{"kind":"One","args":{"kind":"Source"}},"destination":"OwnersBattlefield","face_down":false,"terms":{"order":"DestinationDefault","placement":"DestinationDefault","qualifiers":[]},"entry":{"controller":null,"tapped":false,"counters":[],"states":[{"May":[controller(0),controller(1),{"kind":"Term","args":{"family":"EnterFlag","form":"EntersWithNumberCounters","operands":[{"kind":"Number","args":{"kind":"BoundValue","args":17}},{"kind":"Vocabulary","args":{"family":"CounterType","form":"SilverCounter"}}]}}]}]}}});
    let mapped = migrate::mapped_read_json(&old.to_string()).unwrap();
    let after = serde_json::to_value(mapped.card).unwrap();
    let at = "/faces/0/abilities/0/args/body/instructions/1/args/operation/args/entry/states/0/May";
    assert!(mapped.map.sites[&format!("{at}/0/args/operands/0/args/args/slot")].is_empty());
    for source in [
        format!("{at}/1/args/operands/0/args/args/slot"),
        format!("{at}/2/args/operands/0/args/args"),
    ] {
        assert_eq!(mapped.map.sites[&source].len(), 1);
        assert_eq!(
            old.pointer(&source),
            after.pointer(&mapped.map.sites[&source][0])
        );
    }
}

#[test]
fn entry_upgrade_refuses_wrong_families_mixed_epochs_and_unknown_fields() {
    let (old, path) = shared_size();
    let mut extra = old.clone();
    extra.pointer_mut(&path).unwrap()["mystery"] = json!(true);
    assert!(migrate::read_json(&extra.to_string()).is_err());
    let mut mixed = old.clone();
    mixed["faces"][0]["schema_version"] = json!(180);
    assert!(migrate::read_json(&mixed.to_string()).is_err());
    let mut wrong = old;
    wrong.pointer_mut(&path).unwrap()["creature_type"] =
        json!({"kind":"Vocabulary","args":{"family":"CounterType","form":"Cyberman"}});
    assert!(migrate::read_json(&wrong.to_string()).is_err());
}

#[test]
#[ignore = "explicit complete before/current corpora"]
fn every_record_upgrades_to_exact_fresh_output_with_complete_origins() {
    use std::io::BufRead;
    mtgish_ast::corpus::on_large_stack(|| {
        let old = std::io::BufReader::new(
            std::fs::File::open(std::env::var("ENTRY_OLD_CORPUS").unwrap()).unwrap(),
        );
        let new = std::io::BufReader::new(
            std::fs::File::open(std::env::var("ENTRY_NEW_CORPUS").unwrap()).unwrap(),
        );
        let mut count = 0;
        let mut old = old.lines();
        let mut new = new.lines();
        loop {
            match (old.next(), new.next()) {
                (None, None) => break,
                (Some(old), Some(new)) => {
                    let old = old.unwrap();
                    let before: Value = serde_json::from_str(&old).unwrap();
                    let mapped = migrate::mapped_read_json(&old)
                        .unwrap_or_else(|e| panic!("{}: {e}", before["faces"][0]["name"]));
                    let after = serde_json::to_value(mapped.card).unwrap();
                    assert_eq!(
                        after,
                        serde_json::from_str::<Value>(&new.unwrap()).unwrap(),
                        "{}",
                        before["faces"][0]["name"]
                    );
                    for (origin, targets) in mapped.map.sites {
                        for target in targets {
                            assert_eq!(before.pointer(&origin), after.pointer(&target));
                        }
                    }
                    count += 1;
                }
                _ => panic!("corpus lengths differ"),
            }
        }
        assert_eq!(count, 33529);
        println!(
            "Upgraded all {count} records with exact fresh-source output and complete origins"
        );
    });
}
