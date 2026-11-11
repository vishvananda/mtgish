#[path = "support/epoch189.rs"]
mod epoch189;
use mtgish_ast::{canonical, corpus, ir::*, lower, migrate, mtg_types as source, validate};
use serde_json::{json, Value};

fn fixtures() -> impl Iterator<Item = &'static str> {
    include_str!("fixtures/reference_shared_x_20261110.ronl").lines()
        .chain(include_str!("fixtures/reference_target_actor_20261110.ronl").lines())
        .chain(include_str!("fixtures/reference_scope_20261110.ronl").lines())
        .chain(include_str!("fixtures/reference_history_20261110.ronl").lines())
        .chain(include_str!("fixtures/reference_crew_history_20261110.ronl").lines())
}
fn canonical_abilities(mut value: Value) -> Value {
    epoch189::tree(&mut value);
    let abilities: Vec<Ability> = serde_json::from_value(value.clone()).unwrap();
    validate::abilities(&abilities).unwrap();
    canonical::abilities(&mut value).unwrap();
    let abilities: Vec<Ability> = serde_json::from_value(value.clone()).unwrap();
    validate::abilities(&abilities).unwrap();
    value
}
#[test]
fn complete_reviewed_cards_match_explicit_semantic_repairs() {
    corpus::on_large_stack(|| {
        let expected: Value = serde_json::from_str(include_str!("fixtures/reference32_expected_20261110.json")).unwrap();
        let nyla = migrate::read_json(include_str!("fixtures/nyla_reviewed_proposal_v180.json")).unwrap();
        let mjolnir = migrate::read_json(include_str!("fixtures/mjolnir_reviewed_proposal_v180.json")).unwrap();
        let mut names = std::collections::BTreeSet::new();
        let mut mismatches = Vec::new();
        for line in fixtures() {
            let input: source::OracleCard = ron::from_str(line).unwrap();
            let card = lower::card(&input).unwrap();
            names.insert(card.name.clone());
            validate::card(&card).unwrap_or_else(|error| panic!("{}: {error}",card.name));
            let actual = canonical_abilities(serde_json::to_value(&card.abilities).unwrap());
            let want = if card.name == "Nyla, Shirshu Sleuth" {
                serde_json::to_value(&nyla.faces[0].abilities).unwrap()
            } else if card.name == "Mjölnir, Storm Hammer" {
                serde_json::to_value(&mjolnir.faces[0].abilities).unwrap()
            } else { expected[&card.name].clone() };
            let want = canonical_abilities(want);
            if let Ok(dir) = std::env::var("REFERENCE_FIX_EVIDENCE") {
                std::fs::write(format!("{dir}/{}.actual.json",card.name),serde_json::to_vec_pretty(&actual).unwrap()).unwrap();
                std::fs::write(format!("{dir}/{}.expected.json",card.name),serde_json::to_vec_pretty(&want).unwrap()).unwrap();
            }
            if actual != want { mismatches.push(card.name.clone()); }
        }
        assert_eq!(names.len(),32);
        assert!(mismatches.is_empty(),"whole-card differences: {mismatches:?}");
    });
}
#[test]
fn migration_preserves_saved_186_meaning_and_diagnoses_invalid_surveil_receipt() {
    corpus::on_large_stack(|| {
        let mut preserved = 0;
        for line in include_str!("fixtures/reference32_before_v186.jsonl").lines() {
            let mut original: Value = serde_json::from_str(line).unwrap();
        epoch189::tree(&mut original);
            let name = original["faces"][0]["name"].as_str().unwrap().to_owned();
            if name == "Starving Revenant" {
                let error = migrate::mapped_read_json(line).unwrap_err();
                assert!(error.contains("did not surveil"),"{error}");
                continue;
            }
            let mapped = migrate::mapped_read_json(line).unwrap_or_else(|error| panic!("{name}: {error}"));
            assert_eq!(mapped.source_version,186);
            epoch189::assert_origins(&serde_json::from_str(line).unwrap(), &mapped);
            original["schema_version"]=json!(SCHEMA_VERSION);
            for face in original["faces"].as_array_mut().unwrap() { face["schema_version"]=json!(SCHEMA_VERSION); }
            assert_eq!(serde_json::to_value(&mapped.card).unwrap(),original,"{name}: migration changed saved meaning");
            assert!(mapped.map.introduced.is_empty());
            assert!(mapped.map.sites.values().all(|new|new.len() == 1));
            preserved+=1;
        }
        assert_eq!(preserved,31);
    });
}

fn source_card(name: &str) -> Card {
    let line = fixtures().find(|line| line.contains(&format!("Name: {name:?}"))).unwrap();
    lower::card(&ron::from_str::<source::OracleCard>(line).unwrap()).unwrap()
}
fn reject_edit(name: &str, edit: impl FnOnce(&mut Value), message: &str) {
    let card = source_card(name);
    let mut value = serde_json::to_value(card).unwrap();
    edit(&mut value);
    let card: Card = serde_json::from_value(value).unwrap();
    let error = validate::card(&card).unwrap_err();
    assert!(error.contains(message),"{name}: {error}");
}
#[test]
fn recorded_facts_require_their_exact_event_or_producer_family() {
    corpus::on_large_stack(|| {
        reject_edit("Aragorn, Company Leader", |v| {
            v["abilities"][0]["args"]["event"]["Player"]["event"] = json!("Draws");
        }, "Ring temptation");
        reject_edit("Horrid Shadowspinner", |v| {
            let count = v["abilities"][1]["args"]["body"]["instructions"][0]["args"]["operation"]["args"]["cost"]["parts"][0]["Draw"]["count"].clone();
            v["abilities"][1]["args"]["body"]["instructions"][0]["args"]["operation"]["args"]["cost"]["parts"][0] = json!({"Mill":{"count":count}});
        }, "draw cost component");
        reject_edit("Horrid Shadowspinner", |v| {
            v["abilities"][1]["args"]["body"]["instructions"][1]["args"]["then"]["instructions"][0]["args"]["operation"]["args"]["amount"]["args"]["component"] = json!([9]);
        }, "invalid component path");
        reject_edit("Starving Revenant", |v| {
            v["abilities"][0]["args"]["body"]["instructions"][1]["args"]["operation"]["args"]["amount"] = json!({"kind":"SurveilKeptOnTop","args":1});
        }, "did not surveil");
        reject_edit("Hofri Ghostforge", |v| {
            v["abilities"][1]["args"]["body"]["instructions"][1]["args"]["then"]["instructions"][0]["args"]["operation"]["args"]["entry"]["states"][0] = json!({"LinkExiledCards":{"kind":"Selected","args":999}});
        }, "999");
        // The creation link belongs to arrival history, not the token's copy
        // recipe. Its quoted return ability contains only its own Source.
        let hofri=serde_json::to_value(source_card("Hofri Ghostforge")).unwrap();
        let create=&hofri["abilities"][1]["args"]["body"]["instructions"][1]["args"]["then"]["instructions"][0]["args"]["operation"]["args"];
        assert!(create["entry"]["states"][0].get("LinkExiledCards").is_some());
        let recipe=serde_json::to_string(&create["tokens"][0]["recipe"]).unwrap();
        assert!(recipe.contains("CreationLinkedCards"));
        assert!(!recipe.contains("CostObjects"));
        assert!(!recipe.contains("LinkExiledCards"));
        let ghoulcaller=serde_json::to_value(source_card("Veteran Ghoulcaller")).unwrap();
        assert_eq!(ghoulcaller["abilities"][1]["args"]["body"]["instructions"][0]["args"]["operation"]["args"]["card"],json!({"DuplicateLastKnown":{"kind":"EventSubject"}}));
    });
}

#[test]
fn adjacent_cards_using_the_same_templates_preserve_all_other_operands() {
    corpus::on_large_stack(|| {
        let expected: Value = serde_json::from_str(include_str!("fixtures/reference_adjacent_expected_20261110.json")).unwrap();
        let mut count=0;
        for line in include_str!("fixtures/reference_adjacent_20261110.ronl").lines() {
            let card=lower::card(&ron::from_str::<source::OracleCard>(line).unwrap()).unwrap();
            validate::card(&card).unwrap();
            assert_eq!(canonical_abilities(serde_json::to_value(&card.abilities).unwrap()),canonical_abilities(expected[&card.name].clone()),"{}",card.name);
            count+=1;
        }
        assert_eq!(count,7);
    });
}

#[test]
#[ignore = "explicit complete saved-186 corpus identity and binding-origin audit"]
fn all_saved_186_records_preserve_meaning_or_diagnose_wrong_surveil_family() {
    use std::io::BufRead;
    corpus::on_large_stack(|| {
        let path=std::env::var("REFERENCE_186_CORPUS").unwrap();
        let input=std::io::BufReader::new(std::fs::File::open(path).unwrap());
        let mut count=0;
        let mut invalid=Vec::new();
        for line in input.lines() {
            let line=line.unwrap();let mut old:Value=serde_json::from_str(&line).unwrap();
        epoch189::tree(&mut old);
            match migrate::mapped_read_json(&line) {
                Ok(mapped)=> {
                    old["schema_version"]=json!(SCHEMA_VERSION);
                    for face in old["faces"].as_array_mut().unwrap() {face["schema_version"]=json!(SCHEMA_VERSION);}
                    assert_eq!(serde_json::to_value(&mapped.card).unwrap(),old);
                    assert!(mapped.map.introduced.is_empty());
                    assert!(mapped.map.sites.values().all(|new|new.len() == 1));
                },
                Err(error)=> {
                    assert!(error.contains("did not surveil"),"{error}");
                    invalid.push(old["faces"][0]["name"].as_str().unwrap().to_owned());
                }
            }
            count+=1;
        }
        assert_eq!(count,33529);
        assert_eq!(invalid,vec!["Starving Revenant"]);
        println!("33528 saved186 records preserve exact meanings and origins;1 known invalid surveil receipt diagnosed");
    });
}

#[test]
fn a_private_look_also_supplies_the_observed_subset_to_a_later_optional_choice() {
    corpus::on_large_stack(|| {
        let source:source::OracleCard=ron::from_str(include_str!("fixtures/reference_look_choice_20261110.ronl").trim()).unwrap();
        let card=lower::card(&source).unwrap();
        validate::card(&card).unwrap();
        let expected:Value=serde_json::from_str(include_str!("fixtures/reference_look_choice_expected_20261110.json")).unwrap();
        assert_eq!(canonical_abilities(serde_json::to_value(&card.abilities).unwrap()),canonical_abilities(expected));
        #[cfg(feature="cantrip")]
        assert!(mtgish_ast::cantrip::module(&card).is_ok());
    });
}

#[test]
fn revealing_a_library_after_a_hand_does_not_replace_the_hand_receipt() {
    corpus::on_large_stack(|| {
        let input:source::OracleCard=ron::from_str(include_str!("fixtures/reference_two_observations_20261110.ronl").trim()).unwrap();
        let card=lower::card(&input).unwrap();validate::card(&card).unwrap();
        let expected:Value=serde_json::from_str(include_str!("fixtures/reference_two_observations_expected_20261110.json")).unwrap();
        assert_eq!(canonical_abilities(serde_json::to_value(&card.abilities).unwrap()),canonical_abilities(expected));
        #[cfg(feature="cantrip")]
        assert!(mtgish_ast::cantrip::module(&card).is_ok());
    });
}

#[test]
fn prepared_186_named_documents_use_the_same_explicit_epoch_migration() {
    corpus::on_large_stack(|| {
        let cases:Value=serde_json::from_str(include_str!("../tools/blockly/fixtures/workspaces-local-v6-schema186.json")).unwrap();
        for case in cases.as_array().unwrap() {
            let old=&case["document"];
            let doc=mtgish_ast::human::document::Document::from_json(&serde_json::to_string(old).unwrap()).unwrap();
            let current=serde_json::to_value(&doc).unwrap();
            assert_eq!(current["card"]["schema_version"],json!(SCHEMA_VERSION));
            assert_eq!(current["bindings"],old["bindings"]);
            let mut expected=old["card"].clone();epoch189::tree(&mut expected);expected["schema_version"]=json!(SCHEMA_VERSION);
            for face in expected["faces"].as_array_mut().unwrap() {face["schema_version"]=json!(SCHEMA_VERSION);}
            assert_eq!(current["card"],expected);
        }
    });
}
