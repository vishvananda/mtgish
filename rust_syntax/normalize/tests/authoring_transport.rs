use mtgish_ast::{binding_map::SiteMap, canonical, human::document, ir::*};
fn card(body: &str) -> String {
    format!("Card{{name:\"Mapped names\",properties:{{card_types:[Instant]}},abilities:[Spell{{do:[{body}]}}]}}")
}
fn mapped(text: &str) -> (document::Document, canonical::MappedCard) {
    let doc = document::read(text).unwrap();
    let mapped = canonical::mapped_unified(doc.card()).unwrap();
    assert_eq!(
        serde_json::to_value(&mapped.card).unwrap(),
        serde_json::to_value(canonical::unified(doc.card()).unwrap()).unwrap()
    );
    (doc, mapped)
}
#[test]
fn group_splicing_transports_sites_names_and_stable_binding_indices() {
    let(doc,mapped)=mapped(&card("Group([chosen_cards=NoteObjects{objects:One(This)},NoteObjects{objects:Selected(chosen_cards)}])"));
    let index = doc
        .bindings()
        .iter()
        .position(|b| b.name == "chosen_cards")
        .unwrap();
    let moved = doc.uses().iter().find(|u| u.binding == index).unwrap();
    assert!(mapped.map.sites[&moved.path]
        .iter()
        .all(|p| !p.contains("/args/instructions/")));
    let after = doc.transport(&mapped.card, &mapped.map).unwrap();
    assert_eq!(after.bindings()[index].name, "chosen_cards");
    assert!(after.write().unwrap().contains("Selected(chosen_cards)"));
    assert_eq!(doc.card().faces[0].abilities.len(), 1);
}
#[test]
fn event_restructuring_carries_its_player_predicate_binding() {
    let text = r#"Card{name:"Entry names",properties:{card_types:[Creature]},abilities:[Triggered{event:Player{event:EntersUnderControl,players:Matching{binding:controller,condition:PlayerIsActive(Bound(controller))},objects:None},do:[DrawCards{players:You,amount:1}]}]}"#;
    let (doc, mapped) = mapped(text);
    let after = doc.transport(&mapped.card, &mapped.map).unwrap();
    assert!(after.write().unwrap().contains("binding: controller"));
    assert!(
        mapped
            .map
            .sites
            .iter()
            .any(|(p, after)| p.contains("/players/")
                && after.iter().any(|p| p.contains("/objects/")))
    );
}
#[test]
fn simultaneous_selection_hoisting_preserves_each_scoped_name() {
    let text=card("Together([WithSelections{steps:[{binding:first,chooser:Players(You),objects:One(This),count:Exactly(1),constraints:[]}],action:NoteObjects{objects:Prepared(first)}},WithSelections{steps:[{binding:second,chooser:Players(You),objects:One(This),count:Exactly(1),constraints:[]}],action:NoteObjects{objects:Prepared(second)}}])");
    let (doc, mapped) = mapped(&text);
    let after = doc.transport(&mapped.card, &mapped.map).unwrap();
    let code = after.write().unwrap();
    assert!(code.contains("Prepared(first)"));
    assert!(code.contains("Prepared(second)"));
    assert!(mapped
        .map
        .sites
        .iter()
        .any(|(before, after)| after.iter().any(|after| before != after)));
}

#[test]
fn ordered_reveal_policy_matches_the_plain_prepass_gate() {
    // The plain canonicalizer runs movement normalization only when some
    // simultaneous member contains preparation. Tracing uses that same gate.
    for body in [
        "Together([Reveal{objects:One(This)},DiscardObjects{objects:One(This)}])",
        "Together([Reveal{objects:One(This)},DiscardObjects{objects:One(This)}]),Together([WithSelections{steps:[{binding:picked,chooser:Players(You),objects:One(This),count:Exactly(1),constraints:[]}],action:NoteObjects{objects:Prepared(picked)}}])",
    ] {
        let (doc, mapped) = mapped(&card(body));
        doc.transport(&mapped.card, &mapped.map).unwrap();
    }
}
#[test]
fn trigger_merges_report_both_origins_and_do_not_silently_discard_names() {
    let text = r#"Card{name:"Merged names",properties:{card_types:[Creature]},abilities:[Triggered{event:Turn{point:UpkeepBegins,players:All},do:[first=BindValue{value:3}]},Triggered{event:Turn{point:EndStepBegins,players:All},do:[second=BindValue{value:3}]}]}"#;
    let (doc, mapped) = mapped(text);
    let before = doc.write().unwrap();
    assert_eq!(mapped.card.faces[0].abilities.len(), 1);
    let destinations: Vec<_> = mapped.map.sites.values().flat_map(|v| v.iter()).collect();
    assert!(destinations.iter().any(|path| destinations
        .iter()
        .filter(|other| *other == path)
        .count()
        > 1));
    assert!(doc
        .transport(&mapped.card, &mapped.map)
        .unwrap_err()
        .contains("merges bindings"));
    assert_eq!(doc.write().unwrap(), before);
}
#[test]
fn incomplete_wrong_type_and_duplicate_maps_are_rejected_atomically() {
    let (doc, mapped) = mapped(&card(
        "Group([chosen=NoteObjects{objects:One(This)},NoteObjects{objects:Selected(chosen)}])",
    ));
    let before = doc.write().unwrap();
    let mut broken = mapped.map.clone();
    broken.sites.pop_first();
    assert!(doc
        .transport(&mapped.card, &broken)
        .unwrap_err()
        .contains("every input"));
    let mut broken = mapped.map.clone();
    broken
        .sites
        .values_mut()
        .next()
        .unwrap()
        .push("/faces/0/name".into());
    assert!(doc
        .transport(&mapped.card, &broken)
        .unwrap_err()
        .contains("wrong ID type"));
    let mut broken = mapped.map.clone();
    let destinations = broken.sites.values_mut().next().unwrap();
    destinations.push(destinations[0].clone());
    assert!(doc
        .transport(&mapped.card, &broken)
        .unwrap_err()
        .contains("duplicate"));
    let mut changed = mapped.card.clone();
    changed.faces[0].name = "Different meaning".into();
    assert!(doc
        .transport(&changed, &mapped.map)
        .unwrap_err()
        .contains("meaning"));
    assert_eq!(doc.write().unwrap(), before);
}
#[test]
fn empty_documents_accept_an_empty_site_map() {
    // Source identity maps are still required for rewrites that introduce no IDs.
    let doc = document::read(&card("")).unwrap();
    let map = SiteMap::default();
    let after = doc.transport(doc.card(), &map).unwrap();
    assert_eq!(after.write().unwrap(), doc.write().unwrap());
}
#[test]
#[ignore = "explicit full mapped canonical corpus"]
fn all_records_have_complete_canonical_origin_maps() {
    use std::io::BufRead;
    mtgish_ast::corpus::on_large_stack(|| {
        let input = std::fs::File::open(std::env::var("AUTHORING_CORPUS").unwrap()).unwrap();
        let (mut count, mut errors) = (0, vec![]);
        for line in std::io::BufReader::new(input).lines() {
            let card: UnifiedCard = serde_json::from_str(&line.unwrap()).unwrap();
            let check = canonical::mapped_unified(&card).and_then(|mapped| {
                let twice = canonical::unified(&mapped.card)?;
                if serde_json::to_value(twice).unwrap()
                    != serde_json::to_value(mapped.card).unwrap()
                {
                    return Err("canonicalization is not idempotent".into());
                }
                Ok(())
            });
            if let Err(e) = check {
                let error = format!(
                    "{:?}: {e}",
                    card.faces.iter().map(|f| &f.name).collect::<Vec<_>>()
                );
                eprintln!("{error}");
                errors.push(error);
            }
            count += 1;
        }
        assert_eq!(count, 33529);
        assert!(
            errors.is_empty(),
            "{} mapping failures:\n{}",
            errors.len(),
            errors.join("\n")
        );
        println!(
            "Mapped canonicalization agrees with plain canonicalization for all {count} records"
        );
    });
}

#[test]
fn captured_names_survive_hoisting_through_nested_and_future_scopes() {
    for body in [
        "WithSelections{steps:[{binding:outer,chooser:Players(You),objects:One(This),count:Exactly(1),constraints:[]}],action:Together([WithSelections{steps:[{binding:inner,chooser:Players(You),objects:Prepared(outer),count:Exactly(1),constraints:[]}],action:NoteObjects{objects:Prepared(inner)}},NoteObjects{objects:Prepared(outer)}])}",
        "Together([WithSelections{steps:[{binding:outer,chooser:Players(You),objects:One(This),count:Exactly(1),constraints:[]}],action:InOrder([WithSelections{steps:[{binding:inner,chooser:Players(You),objects:Prepared(outer),count:Exactly(1),constraints:[]}],action:InOrder([NoteObjects{objects:Prepared(outer)},NoteObjects{objects:Prepared(inner)}])}])}])",
        "Together([WithSelections{steps:[{binding:outer,chooser:Players(You),objects:One(This),count:Exactly(1),constraints:[]}],action:CreateDelayedTrigger{event:Turn{point:EndStepBegins,players:All},start:Immediately,expires:None,intervening_if:None,do:[NoteObjects{objects:Prepared(outer)}]}}])",
        "Together([WithSelections{steps:[{binding:outer,chooser:Players(You),objects:One(This),count:Exactly(1),constraints:[]}],action:WithSelections{steps:[{binding:inner,chooser:Players(You),objects:Prepared(outer),count:Exactly(1),constraints:[]}],action:NoteObjects{objects:Prepared(inner)}}}])",
    ] {
        let (doc, mapped) = mapped(&card(body));
        let after = doc.transport(&mapped.card, &mapped.map).unwrap();
        let code = after.write().unwrap();
        assert!(code.contains("Prepared(outer)"), "{code}");
        if body.contains("binding:inner") {
            assert!(code.contains("Prepared(inner)"), "{code}");
        }
    }
}

#[test]
fn joined_branch_receipts_report_each_cloned_origin() {
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/canonical_join_results.json")).unwrap();
    let mut value = cases[0]["a"].clone();
    value[0]["args"]["body"]["instructions"][0]["args"]["options"][1]["instructions"][0]["args"]
        ["result"] = serde_json::json!(11);
    value[0]["args"]["body"]["instructions"][1]["args"]["operation"]["args"]["results"] =
        serde_json::json!([11, 11]);
    let mut card = document::read(&card("")).unwrap().card().clone();
    card.faces[0].abilities = serde_json::from_value(value).unwrap();
    let mapped = canonical::mapped_unified(&card).unwrap();
    let source = "/faces/0/abilities/0/args/body/instructions/1/args/operation/args/results/0";
    assert_eq!(mapped.map.sites[source].len(), 2);
    assert_ne!(mapped.map.sites[source][0], mapped.map.sites[source][1]);
    let source = source.strip_suffix('0').unwrap().to_owned() + "1";
    assert_eq!(mapped.map.sites[&source].len(), 2);
    let value = serde_json::to_value(&mapped.card).unwrap();
    let results = value["faces"][0]["abilities"][0]["args"]["body"]["instructions"][1]["args"]
        ["operation"]["args"]["results"]
        .as_array()
        .unwrap();
    assert_eq!(results.len(), 4);
    assert_ne!(results[0], results[1]);
    assert_eq!(&results[..2], &results[2..]);
}

#[test]
fn hoisted_selection_choosers_keep_their_earlier_prepared_reference() {
    let text=card("Together([WithSelections{steps:[{binding:s17,chooser:Players(You),objects:One(This),count:Exactly(1),constraints:[]},{binding:s99,chooser:Players(Matching{binding:decider,condition:Compare{left:CountObjects(Prepared(s17)),comparison:Greater,right:0}}),objects:One(This),count:Exactly(1),constraints:[]}],action:NoteObjects{objects:Prepared(s99)}}])");
    let (doc, mapped) = mapped(&text);
    let after = doc.transport(&mapped.card, &mapped.map).unwrap();
    assert!(after.write().unwrap().contains("Prepared(s17)"));
}

#[test]
fn nested_preparations_capture_outer_names_before_local_definitions() {
    let text=card("WithSelections{steps:[{binding:outer,chooser:Players(You),objects:One(This),count:Exactly(1),constraints:[]}],action:WithSelections{steps:[{binding:inner,chooser:Players(Matching{binding:decider,condition:Compare{left:CountObjects(Prepared(outer)),comparison:Greater,right:0}}),objects:Prepared(outer),count:Exactly(1),constraints:[]}],action:NoteObjects{objects:Prepared(inner)}}}");
    let (doc, mapped) = mapped(&text);
    let after = doc.transport(&mapped.card, &mapped.map).unwrap();
    let code = after.write().unwrap();
    assert_eq!(code.matches("Prepared(outer)").count(), 2);
    assert!(code.contains("Prepared(inner)"));
}

#[test]
fn newly_constructed_references_must_be_accounted_for_explicitly() {
    let doc = document::read(&card(
        "chosen=NoteObjects{objects:One(This)},NoteObjects{objects:Selected(chosen)}",
    ))
    .unwrap();
    let mut map = SiteMap::identity(doc.card()).unwrap();
    let reference = doc
        .uses()
        .iter()
        .find(|u| u.path.ends_with("/objects/args"))
        .unwrap()
        .path
        .clone();
    map.sites.insert(reference.clone(), vec![]);
    assert!(doc
        .transport(doc.card(), &map)
        .unwrap_err()
        .contains("every output"));
    map.introduced.push(reference);
    let after = doc.transport(doc.card(), &map).unwrap();
    assert!(after.write().unwrap().contains("Selected(chosen)"));
    let mut bad = map.clone();
    bad.introduced.push(bad.introduced[0].clone());
    assert!(doc
        .transport(doc.card(), &bad)
        .unwrap_err()
        .contains("introduced"));
}
#[test]
fn composed_maps_follow_names_through_two_structural_rewrites() {
    let doc=document::read(&card("Group([Group([chosen=NoteObjects{objects:One(This)},NoteObjects{objects:Selected(chosen)}])])")).unwrap();
    let mut value = serde_json::to_value(doc.card()).unwrap();
    let inner = value["faces"][0]["abilities"][0]["args"]["body"]["instructions"][0]["args"]
        ["instructions"]
        .take();
    value["faces"][0]["abilities"][0]["args"]["body"]["instructions"] = inner;
    let middle: UnifiedCard = serde_json::from_value(value).unwrap();
    let mut first = SiteMap::identity(doc.card()).unwrap();
    for destinations in first.sites.values_mut() {
        for p in destinations {
            *p = p.replacen("/instructions/0/args/instructions", "/instructions", 1);
        }
    }
    doc.transport(&middle, &first).unwrap();
    let last = canonical::mapped_unified(&middle).unwrap();
    let composed = first.then(&last.map).unwrap();
    let direct = canonical::mapped_unified(doc.card()).unwrap();
    assert_eq!(composed, direct.map);
    let after = doc.transport(&last.card, &composed).unwrap();
    assert!(after.write().unwrap().contains("Selected(chosen)"));
    let mut wrong = last.map;
    wrong.sites.pop_first();
    assert!(first.then(&wrong).unwrap_err().contains("intermediate"));
}

#[test]
fn metadata_only_single_abilities_use_the_same_typed_canonicalizer() {
    let face = document::read(&card(
        "Group([chosen=NoteObjects{objects:One(This)},NoteObjects{objects:Selected(chosen)}])",
    ))
    .unwrap();
    let card = UnifiedCard {
        schema_version: SCHEMA_VERSION,
        layout: "StickerSheet".into(),
        faces: vec![],
        data: vec![Field {
            name: "Ability".into(),
            value: Operand::Ability(Box::new(face.card().faces[0].abilities[0].clone())),
        }],
    };
    let mut doc = document::from_card(&card).unwrap();
    let binding = doc
        .bindings()
        .iter()
        .enumerate()
        .find(|(i, b)| {
            b.kind == document::BindingKind::Result
                && doc.uses().iter().filter(|u| u.binding == *i).count() > 1
        })
        .map(|(i, _)| i)
        .unwrap();
    doc.rename(binding, "sticker_objects").unwrap();
    let mapped = canonical::mapped_unified(doc.card()).unwrap();
    let after = doc.transport(&mapped.card, &mapped.map).unwrap();
    assert!(after.write().unwrap().contains("Selected(sticker_objects)"));
    let value = serde_json::to_value(&mapped.card).unwrap();
    assert_eq!(
        value["data"][0]["value"]["args"]["args"]["body"]["instructions"][0]["kind"],
        "Effect"
    );
    let json = serde_json::to_string(&mapped).unwrap();
    canonical::MappedCard::from_json(&json).unwrap();
    let mut bad = serde_json::to_value(mapped).unwrap();
    bad["card"]["unknown"] = serde_json::json!(true);
    assert!(canonical::MappedCard::from_json(&bad.to_string())
        .unwrap_err()
        .contains("unknown"));
}
