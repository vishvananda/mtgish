#[path = "support/fresh_producer.rs"]
mod fresh_producer;
use mtgish_ast::{canonical, corpus, human, ir::*, migrate, mtg_types, unified};
use serde_json::Value;
fn code(modes: &str) -> String {
    format!("Card{{name:\"Mode policies\",properties:{{card_types:[Instant]}},abilities:[Spell{{modes:{modes},do:[]}}]}}")
}
#[test]
fn mode_policies_are_independent_and_defaults_are_ordinary_choice() {
    let text = code("{options:[{weight:1,do:[]},{weight:2,do:[]}]}");
    let card = human::read(&text).unwrap();
    let readable = human::write(&card).unwrap();
    assert!(!readable.contains("repetition"), "{readable}");
    assert!(!readable.contains("mechanism"), "{readable}");
    let all=human::read(&code("{count:{minimum:0,maximum:3},options:[{weight:1,do:[]},{weight:2,do:[]}],rules:{repetition:MayRepeat,availability:Unchosen(ThisTurn),surcharge:Some({parts:[]}),targets:Distinct,acceptance:MayDecline},mechanism:Announced{chooser:OneOf(OpponentsOf(You)),method:Random},budget:Some(5)}")).unwrap();
    let value = serde_json::to_value(&all).unwrap();
    let selection = &value["faces"][0]["abilities"][0]["args"]["announcement"]["modes"];
    assert_eq!(selection["rules"]["repetition"], "MayRepeat");
    assert_eq!(selection["rules"]["targets"], "Distinct");
    assert_eq!(selection["mechanism"]["Announced"]["method"], "Random");
    let bytes = bincode::encode_to_vec(&all, bincode::config::standard()).unwrap();
    let (back, n): (UnifiedCard, usize) =
        bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
    assert_eq!(n, bytes.len());
    assert_eq!(value, serde_json::to_value(back).unwrap());
    for bad in [
        "{options:[],mechanism:Implied{chooser:Controller}}",
        "{options:[],rules:{repetition:true}}",
        "{options:[],implied:false}",
    ] {
        assert!(human::read(&code(bad)).is_err(), "{bad}");
    }
}
#[test]
fn every_captured_mode_shape_migrates_to_exact_fresh_source_with_origins() {
    corpus::on_large_stack(|| {
        let sources: Vec<mtg_types::OracleCard> = include_str!("fixtures/mode_policies.ronl")
            .lines()
            .map(|l| ron::from_str(l).unwrap())
            .collect();
        let mut count = 0;
        for line in include_str!("fixtures/mode_policies_v183.jsonl").lines() {
            let old: Value = serde_json::from_str(line).unwrap();
            let name = old["faces"][0]["name"].as_str().unwrap();
            let source = sources
                .iter()
                .find(|s| {
                    corpus::Identity::of(s)
                        .unwrap()
                        .names
                        .iter()
                        .any(|n| n == name)
                })
                .unwrap();
            let mapped = migrate::mapped_read_json(line).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(mapped.source_version, 183);
            assert!(mapped.map.introduced.is_empty());
            let new = serde_json::to_value(&mapped.card).unwrap();
            fresh_producer::assert_matches("mode_policies_v183.jsonl", &new, &serde_json::to_value(unified::lower(source).card).unwrap());
            for (origin, targets) in mapped.map.sites {
                for target in targets {
                    assert_eq!(
                        old.pointer(&origin),
                        new.pointer(&target),
                        "{name}: {origin} -> {target}"
                    );
                }
            }
            let text = human::write(&mapped.card).unwrap();
            let back = human::read(&text).unwrap_or_else(|e| panic!("{name}: {e}\n{text}"));
            assert_eq!(
                serde_json::to_value(canonical::mapped_unified(&back).unwrap().card).unwrap(),
                serde_json::to_value(canonical::mapped_unified(&mapped.card).unwrap().card)
                    .unwrap(),
                "{name}"
            );
            count += 1;
        }
        assert_eq!(count, 178);
    });
}
#[test]
fn captured_names_and_nested_scope_order_survive_mode_regrouping() {
    corpus::on_large_stack(|| {
        let mut count = 0;
        for old in include_str!("fixtures/mode_documents_v183.jsonl")
            .lines()
            .chain(include_str!("fixtures/mode_context_documents_v183.jsonl").lines())
        {
            let mut value: Value = serde_json::from_str(old).unwrap();
            // Three synthetic scope probes used a one-faced Transforming
            // wrapper to force the old unified writer. That malformed layout
            // is now rejected; exercise their unchanged modes with Card instead.
            if value["card"]["layout"]=="Transforming" && value["card"]["faces"].as_array().unwrap().len()==1 {
                assert!(human::document::Document::from_json(old).unwrap_err().contains("requires 2..=2 faces"));
                value["card"]["layout"]=serde_json::json!("Card");
            }
            let name = &value["card"]["faces"][0]["name"];
            let document = human::document::Document::from_json(&value.to_string()).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(
                serde_json::to_value(document.bindings()).unwrap(),
                value["bindings"],
                "{name}"
            );
            let text = document.write().unwrap();
            let back =
                human::document::read(&text).unwrap_or_else(|e| panic!("{name}: {e}\n{text}"));
            assert_eq!(
                serde_json::to_value(back.bindings()).unwrap(),
                value["bindings"],
                "{name}"
            );
            assert_eq!(
                serde_json::to_value(canonical::mapped_unified(back.card()).unwrap().card).unwrap(),
                serde_json::to_value(canonical::mapped_unified(document.card()).unwrap().card)
                    .unwrap(),
                "{name}"
            );
            count += 1;
        }
        assert_eq!(count, 181);
    });
}
#[test]
fn strict_legacy_modes_refuse_unknown_fields_and_ambiguous_mechanisms() {
    let old: Value = serde_json::from_str(
        include_str!("fixtures/mode_context_documents_v183.jsonl")
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    for field in ["extra", "implied"] {
        let mut card = old["card"].clone();
        card["faces"][0]["abilities"][0]["args"]["announcement"]["modes"][field] =
            Value::Bool(true);
        assert!(
            migrate::read_json(&serde_json::to_string(&card).unwrap()).is_err(),
            "{field}"
        );
    }
    let bad=code("{options:[{weight:1,do:[]}],rules:{availability:Unchosen(LastCombat(Target{slot:t99,index:0}))}}");
    let error = human::read(&bad).unwrap_err();
    assert!(error.contains("unbound target"), "{error}");
}

#[cfg(feature="cantrip")]
#[test]
fn last_combat_history_keeps_additional_mode_policies_in_generated_output() {
    let card=human::read(&code("{options:[{weight:1,do:[DrawCards{players:You,amount:1}]},{weight:2,do:[GainLife{players:You,amount:2}]}],rules:{availability:Unchosen(LastCombat(You)),targets:Distinct},mechanism:Announced{chooser:OneOf(OpponentsOf(You)),method:Random},budget:Some(3)}")).unwrap();
    assert!(mtgish_ast::cantrip::module(&card.faces[0]).is_err());
    let gaps=mtgish_ast::cantrip::gaps(&card.faces[0]);
    for retained in ["modes with distinct targets","modes chosen by another player","modes chosen at random","modes with a budget"] {
        assert!(gaps.iter().any(|gap|gap.feature==retained),"lost {retained}: {gaps:?}");
    }
}
