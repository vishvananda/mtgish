#[path = "support/legacy_entries.rs"]
mod legacy_entries;
use mtgish_ast::{
    canonical,
    human::{
        self,
        document::{self, BindingKind, Document},
    },
    ir::*,
};
use serde_json::{json, Value};
fn card(body: &str) -> String {
    format!("Card{{name:\"Branch names\",properties:{{card_types:[Instant]}},abilities:[Spell{{do:[{body}]}}]}}")
}
fn alternatives() -> [&'static str; 2] {
    [
    "If{condition:PlayerIsActive(You),then:[picked=NoteObjects{objects:One(This)},NoteObjects{objects:Selected(picked)}],otherwise:[picked=NoteObjects{objects:One(This)},NoteObjects{objects:Selected(picked)}]}",
    "ChooseOne{decider:You,options:[[picked=NoteObjects{objects:One(This)},NoteObjects{objects:Selected(picked)}],[picked=NoteObjects{objects:One(This)},NoteObjects{objects:Selected(picked)}]],offered:None}",
]
}
#[test]
fn each_alternative_has_its_own_identity_and_can_be_renamed_independently() {
    for body in alternatives() {
        let mut doc = document::read(&card(body)).unwrap();
        let picked: Vec<_> = doc
            .bindings()
            .iter()
            .enumerate()
            .filter(|(_, b)| b.name == "picked")
            .map(|(i, b)| (i, b.scope))
            .collect();
        assert_eq!(picked.len(), 2);
        assert_ne!(picked[0].1, picked[1].1);
        doc.rename(picked[0].0, "first_picks").unwrap();
        let text = doc.write().unwrap();
        assert_eq!(text.matches("Selected(first_picks)").count(), 1, "{text}");
        assert_eq!(text.matches("Selected(picked)").count(), 1, "{text}");
    }
}
#[test]
fn inherited_names_and_later_ordinary_definitions_keep_their_own_scope() {
    for body in alternatives() {
        let mut doc=document::read(&card(&format!("picked=NoteObjects{{objects:One(This)}},{body},NoteObjects{{objects:Selected(picked)}}"))).unwrap();
        let picked: Vec<_> = doc
            .bindings()
            .iter()
            .enumerate()
            .filter(|(_, b)| b.name == "picked")
            .map(|(i, _)| i)
            .collect();
        assert_eq!(picked.len(), 3);
        doc.rename(picked[0], "outer_picks").unwrap();
        let text = doc.write().unwrap();
        assert_eq!(text.matches("Selected(outer_picks)").count(), 1, "{text}");
        assert_eq!(text.matches("Selected(picked)").count(), 2, "{text}");
    }
}
#[test]
fn branch_locals_do_not_become_ordinary_reads_outside_or_in_siblings() {
    assert!(document::read(&card("If{condition:PlayerIsActive(You),then:[picked=NoteObjects{objects:One(This)}],otherwise:[NoteObjects{objects:Selected(picked)}]}" )).is_err());
    for body in alternatives() {
        assert!(document::read(&card(&format!(
            "{body},NoteObjects{{objects:Selected(picked)}}"
        )))
        .is_err());
    }
}
fn reused_receipts() -> UnifiedCard {
    let mut cases: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/canonical_join_results.json")).unwrap();
    cases.iter_mut().for_each(legacy_entries::json);
    let mut abilities = cases[0]["a"].clone();
    abilities[0]["args"]["body"]["instructions"][0]["args"]["options"][1]["instructions"][0]
        ["args"]["result"] = json!(11);
    abilities[0]["args"]["body"]["instructions"][1]["args"]["operation"]["args"]["results"] =
        json!([11, 11]);
    let mut card = human::read(&card("")).unwrap();
    card.faces[0].abilities = serde_json::from_value(abilities).unwrap();
    card
}
#[test]
fn reused_numeric_receipts_keep_the_original_card_and_independent_names() {
    let card = reused_receipts();
    let mut doc = document::from_card(&card).unwrap();
    assert_eq!(
        serde_json::to_value(doc.card()).unwrap(),
        serde_json::to_value(&card).unwrap()
    );
    let groups: Vec<_> = doc
        .bindings()
        .iter()
        .filter(|b| !b.members.is_empty())
        .collect();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].members.len(), 2);
    let indices = groups[0].members.clone();
    doc.rename(indices[0], "left_receipt").unwrap();
    doc.rename(indices[1], "right_receipt").unwrap();
    let text = doc.write().unwrap();
    let back = human::read(&text).unwrap();
    assert_eq!(
        serde_json::to_value(canonical::unified(&card).unwrap()).unwrap(),
        serde_json::to_value(canonical::unified(&back).unwrap()).unwrap()
    );
    assert_eq!(text.matches("left_receipt").count(), 3, "{text}");
    assert_eq!(text.matches("right_receipt").count(), 3, "{text}");
    Document::from_json(&serde_json::to_string(&doc).unwrap())
        .unwrap()
        .write()
        .unwrap();
}
#[test]
fn symbolic_joins_resolve_each_local_name_without_exposing_it_to_ordinary_reads() {
    let body="ChooseOne{decider:You,options:[[first=NoteObjects{objects:One(This)}],[second=NoteObjects{objects:One(This)}]],offered:None},combined=JoinResults{results:[first,second]},NoteObjects{objects:Transition{result:combined,phase:After}}";
    let mut doc = document::read(&card(body)).unwrap();
    let first = doc
        .bindings()
        .iter()
        .position(|b| b.name == "first")
        .unwrap();
    doc.rename(first, "first_choice").unwrap();
    let text = doc.write().unwrap();
    assert!(text.contains("results: [first_choice, second]"), "{text}");
    assert_eq!(doc.bindings()[first].kind, BindingKind::Result);
    assert!(document::read(&card(&format!(
        "{body},NoteObjects{{objects:Selected(first)}}"
    )))
    .is_err());
}

#[test]
fn same_spelling_in_a_join_is_qualified_by_readable_lexical_scope() {
    let source=card("ChooseOne{decider:You,options:[[picked=NoteObjects{objects:One(This)}],[picked=NoteObjects{objects:One(This)}]],offered:None},JoinResults{results:[picked]}");
    let doc = document::read(&source).unwrap();
    let text = doc.write().unwrap();
    assert!(text.contains("ReceiptFrom(\"option_1\", picked)"), "{text}");
    assert!(text.contains("ReceiptFrom(\"option_2\", picked)"), "{text}");
    let back = document::read(&text).unwrap();
    assert_eq!(text, back.write().unwrap());
    assert_eq!(
        serde_json::to_value(canonical::unified(doc.card()).unwrap()).unwrap(),
        serde_json::to_value(canonical::unified(back.card()).unwrap()).unwrap()
    );
}

#[test]
fn receipt_groups_transport_through_expansion_and_identity_maps() {
    let mut doc = document::from_card(&reused_receipts()).unwrap();
    let members = doc
        .bindings()
        .iter()
        .find(|b| !b.members.is_empty())
        .unwrap()
        .members
        .clone();
    doc.rename(members[0], "first_objects").unwrap();
    doc.rename(members[1], "second_objects").unwrap();
    let identity = mtgish_ast::binding_map::SiteMap::identity(doc.card()).unwrap();
    let same = doc.transport(doc.card(), &identity).unwrap();
    assert_eq!(same.bindings().len(), doc.bindings().len());
    assert_eq!(same.write().unwrap(), doc.write().unwrap());
    let mapped = canonical::mapped_unified(doc.card()).unwrap();
    let moved = doc.transport(&mapped.card, &mapped.map).unwrap();
    assert_eq!(moved.bindings()[members[0]].name, "first_objects");
    assert_eq!(moved.bindings()[members[1]].name, "second_objects");
    let text = moved.write().unwrap();
    assert_eq!(text.matches("first_objects").count(), 3, "{text}");
    assert_eq!(text.matches("second_objects").count(), 3, "{text}");
}

#[test]
fn reused_labels_cannot_forge_reference_identity_or_receipt_membership() {
    let mut doc = document::from_card(&reused_receipts()).unwrap();
    let group = doc
        .bindings()
        .iter()
        .position(|b| !b.members.is_empty())
        .unwrap();
    let before = doc.write().unwrap();
    assert!(doc
        .rename(group, "fake_variable")
        .unwrap_err()
        .contains("expression"));
    assert_eq!(before, doc.write().unwrap());
    let mut value = serde_json::to_value(&doc).unwrap();
    let members = doc.bindings()[group].members.clone();
    let use_ = value["uses"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|u| u["binding"] == members[1])
        .unwrap();
    use_["binding"] = json!(members[0]);
    assert!(Document::from_json(&value.to_string())
        .unwrap_err()
        .contains("lexical scope"));
    let mut value = serde_json::to_value(&doc).unwrap();
    value["bindings"][group]["members"] = json!([members[0], members[0]]);
    assert!(Document::from_json(&value.to_string())
        .unwrap_err()
        .contains("distinct result"));
    let mut value = serde_json::to_value(&doc).unwrap();
    value["bindings"][members[0]]["scope_name"] = json!("wrong/branch");
    assert!(Document::from_json(&value.to_string())
        .unwrap_err()
        .contains("scope metadata"));
}

#[test]
fn saved_v1_branch_documents_upgrade_names_without_changing_the_card() {
    for input in [
        include_str!("fixtures/authoring-v1-if.json"),
        include_str!("fixtures/authoring-v1-choice.json"),
    ] {
        let original: Value = serde_json::from_str(input).unwrap();
        let mut doc = Document::from_json(input).unwrap();
        assert_eq!(doc.version, 2);
        assert_eq!(
            serde_json::to_value(doc.card()).unwrap(),
            serde_json::to_value(
                mtgish_ast::migrate::read_json(&original["card"].to_string()).unwrap()
            )
            .unwrap()
        );
        let picked: Vec<_> = doc
            .bindings()
            .iter()
            .enumerate()
            .filter(|(_, b)| b.name == "picked")
            .map(|(i, _)| i)
            .collect();
        assert_eq!(picked.len(), 2);
        doc.rename(picked[0], "one_branch").unwrap();
        let text = doc.write().unwrap();
        assert_eq!(text.matches("Selected(one_branch)").count(), 1, "{text}");
        assert_eq!(text.matches("Selected(picked)").count(), 1, "{text}");
        Document::from_json(&serde_json::to_string(&doc).unwrap()).unwrap();
        let mut forged = original;
        forged["uses"][0]["path"] = json!("/faces/0/name");
        assert!(Document::from_json(&forged.to_string()).is_err());
    }
}

#[test]
fn all_native_join_regressions_support_distinct_names_for_actual_definitions() {
    let mut cases: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/canonical_join_results.json")).unwrap();
    cases.iter_mut().for_each(legacy_entries::json);
    for case in cases {
        for side in ["a", "b"] {
            let mut c = human::read(&card("")).unwrap();
            c.faces[0].abilities = serde_json::from_value(case[side].clone()).unwrap();
            let mut doc = document::from_card(&c).unwrap();
            let names: Vec<_> = doc
                .bindings()
                .iter()
                .enumerate()
                .filter(|(_, b)| b.members.is_empty())
                .map(|(i, _)| (i, format!("named_{i}")))
                .collect();
            doc.rename_many(&names)
                .unwrap_or_else(|e| panic!("{} {side}: {e}", case["name"]));
            Document::from_json(&serde_json::to_string(&doc).unwrap())
                .unwrap()
                .write()
                .unwrap();
        }
    }
}

#[test]
fn private_optional_results_do_not_join_a_sibling_with_the_same_numeric_label() {
    let text=card("ChooseOne{decider:You,options:[[Optional{decider:You,do:[r11=BindValue{value:3}]}],[r11=MoveObjects{objects:One(This),destination:Exile}]],offered:None},JoinResults{results:[r11,r11]}");
    let doc = document::read(&text).unwrap();
    assert!(doc.bindings().iter().all(|b| b.members.is_empty()));
    let output = doc.write().unwrap();
    let back = document::read(&output).unwrap();
    assert_eq!(
        serde_json::to_value(canonical::unified(doc.card()).unwrap()).unwrap(),
        serde_json::to_value(canonical::unified(back.card()).unwrap()).unwrap()
    );
    assert!(
        output.contains("ReceiptFrom(\"option_2\", r11)"),
        "{output}"
    );
}

#[test]
fn optional_move_receipts_can_be_joined_even_when_an_outer_name_was_shadowed() {
    let text=card("picked=BindValue{value:3},ChooseOne{decider:You,options:[[Optional{decider:You,do:[picked=MoveObjects{objects:One(This),destination:Exile}]}],[other=MoveObjects{objects:One(This),destination:Exile}]],offered:None},JoinResults{results:[ReceiptFrom(\"spell_1/chooseone_1/option_1/optional_1\",picked),other]}");
    let mut doc = document::read(&text).unwrap();
    let picked = doc
        .bindings()
        .iter()
        .enumerate()
        .find(|(_, b)| b.name == "picked" && b.scope_name.ends_with("optional_1"))
        .map(|(i, _)| i)
        .unwrap();
    doc.rename(picked, "optional_objects").unwrap();
    let output = doc.write().unwrap();
    assert!(
        output.contains("results: [optional_objects, other]"),
        "{output}"
    );
}

#[test]
fn ambiguous_short_qualifiers_require_the_branch_path() {
    let body="ChooseOne{decider:You,options:[[ChooseOne{decider:You,options:[[picked=MoveObjects{objects:One(This),destination:Exile}],[another=MoveObjects{objects:One(This),destination:Exile}]],offered:None}],[ChooseOne{decider:You,options:[[picked=MoveObjects{objects:One(This),destination:Exile}],[other=MoveObjects{objects:One(This),destination:Exile}]],offered:None}]],offered:None},JoinResults{results:[picked]}";
    let doc = document::read(&card(body)).unwrap();
    let output = doc.write().unwrap();
    assert!(
        output.contains("ReceiptFrom(\"option_1/chooseone_1/option_1\", picked)"),
        "{output}"
    );
    assert!(
        output.contains("ReceiptFrom(\"option_2/chooseone_1/option_1\", picked)"),
        "{output}"
    );
    let ambiguous = output
        .replace("\"option_1/chooseone_1/option_1\"", "\"option_1\"")
        .replace("\"option_2/chooseone_1/option_1\"", "\"option_1\"");
    assert!(document::read(&ambiguous)
        .unwrap_err()
        .contains("several alternatives"));
    assert_eq!(document::read(&output).unwrap().write().unwrap(), output);
}

#[test]
fn a_v1_join_alias_splits_into_declarations_and_a_receipt_expression() {
    let current = document::from_card(&reused_receipts()).unwrap();
    let alias = current
        .bindings()
        .iter()
        .position(|b| !b.members.is_empty())
        .unwrap();
    let mut combined = current.bindings()[alias].members.clone();
    combined.push(alias);
    let mut value = serde_json::to_value(&current).unwrap();
    let mut bindings =
        vec![json!({"kind":"Result","scope":1,"name":"legacy_receipt","explicit":true})];
    let mut indices = std::collections::BTreeMap::new();
    for (i, binding) in value["bindings"].as_array().unwrap().iter().enumerate() {
        let index = if combined.contains(&i) {
            0
        } else {
            let index = bindings.len();
            let mut b = binding.clone();
            b.as_object_mut().unwrap().remove("scope_name");
            b.as_object_mut().unwrap().remove("members");
            bindings.push(b);
            index
        };
        indices.insert(i, index);
    }
    value["version"] = json!(1);
    value["bindings"] = json!(bindings);
    for use_ in value["uses"].as_array_mut().unwrap() {
        use_["binding"] = json!(indices[&(use_["binding"].as_u64().unwrap() as usize)]);
    }
    let mut upgraded = Document::from_json(&value.to_string()).unwrap();
    assert_eq!(
        serde_json::to_value(upgraded.card()).unwrap(),
        value["card"]
    );
    let declarations: Vec<_> = upgraded
        .bindings()
        .iter()
        .enumerate()
        .filter(|(_, b)| b.name == "legacy_receipt" && b.members.is_empty())
        .map(|(i, _)| i)
        .collect();
    assert_eq!(declarations.len(), 2);
    assert_eq!(
        upgraded
            .bindings()
            .iter()
            .filter(|b| !b.members.is_empty())
            .count(),
        1
    );
    upgraded
        .rename(declarations[0], "first_legacy_receipt")
        .unwrap();
    assert_eq!(
        upgraded
            .write()
            .unwrap()
            .matches("first_legacy_receipt")
            .count(),
        3
    );
}

#[test]
fn private_control_flow_receipts_cannot_replace_an_enclosing_choice_group() {
    let choice="ChooseOne{decider:You,options:[[r11=MoveObjects{objects:One(This),destination:Exile}],[r11=MoveObjects{objects:One(This),destination:Exile}]],offered:None}";
    for control in [
        format!(
            "ForEachPlayer{{players:All,binding:each,in_chosen_direction:false,do:[{choice}]}}"
        ),
        format!("ForEachObject{{objects:One(This),binding:item,do:[{choice}]}}"),
        format!("Optional{{decider:You,do:[{choice}]}}"),
        "RepeatableWithLimit{amount:1,do:[r11=BindValue{value:3}]}".into(),
    ] {
        let doc = document::read(&card(&format!(
            "{choice},{control},JoinResults{{results:[r11,r11]}}"
        )))
        .unwrap();
        doc.write().unwrap_or_else(|e| panic!("{control}: {e}"));
    }
}

#[test]
fn qualified_exported_results_preserve_shadowed_names_and_transport() {
    let source=card("picked=BindValue{value:3},Optional{decider:You,do:[picked=MoveObjects{objects:One(This),destination:Exile}]},Group([NoteObjects{objects:Transition{result:ReceiptFrom(\"optional_1\",picked),phase:After}}])");
    let doc = document::read(&source).unwrap();
    assert!(doc.uses().iter().any(|u| u.qualified));
    let text = doc.write().unwrap();
    assert!(
        text.contains("ReceiptFrom(\"optional_1\", picked)"),
        "{text}"
    );
    let saved = Document::from_json(&serde_json::to_string(&doc).unwrap()).unwrap();
    assert_eq!(saved.write().unwrap(), text);
    let mapped = canonical::mapped_unified(doc.card()).unwrap();
    let moved = doc.transport(&mapped.card, &mapped.map).unwrap();
    assert!(moved.uses().iter().any(|u| u.qualified));
    assert!(moved
        .write()
        .unwrap()
        .contains("ReceiptFrom(\"optional_1\", picked)"));
    let private=card("ChooseOne{decider:You,options:[[picked=NoteObjects{objects:One(This)}],[picked=NoteObjects{objects:One(This)}]],offered:None},NoteObjects{objects:Selected(ReceiptFrom(\"option_1\",picked))}");
    assert!(document::read(&private)
        .unwrap_err()
        .contains("ordinary scope"));
}

#[test]
fn only_a_root_pay_cost_instruction_publishes_payment_results() {
    let choice="ChooseOne{decider:You,options:[[r11=MoveObjects{objects:One(This),destination:Exile}],[r11=MoveObjects{objects:One(This),destination:Exile}]],offered:None}";
    let payment="PayCost{player:You,optional:false,cost:{parts:[PlayerAction{player:You,do:[r11=MoveObjects{objects:One(This),destination:OwnersHand}]}]}}";
    for (operation, groups) in [
        (payment.to_owned(), 0),
        (
            format!("Together([{payment},DrawCards{{players:You,amount:1}}])"),
            1,
        ),
    ] {
        let doc = document::read(&card(&format!(
            "{choice},{operation},JoinResults{{results:[r11,r11]}}"
        )))
        .unwrap();
        assert_eq!(
            doc.bindings()
                .iter()
                .filter(|b| !b.members.is_empty())
                .count(),
            groups
        );
        doc.write().unwrap_or_else(|e| panic!("{operation}: {e}"));
    }
}

#[test]
fn aggregate_reference_expansion_is_bounded_before_allocating_all_candidates() {
    let options = vec!["[picked=MoveObjects{objects:One(This),destination:Exile}]"; 100].join(",");
    let reads = vec!["picked"; 1000].join(",");
    let source=card(&format!("ChooseOne{{decider:You,options:[{options}],offered:None}},JoinResults{{results:[{reads}]}}"));
    assert!(document::read(&source)
        .unwrap_err()
        .contains("join work limit exceeded"));
}

#[test]
fn canonicalization_keeps_nested_payment_labels_private() {
    let choice="ChooseOne{decider:You,options:[[r11=MoveObjects{objects:One(This),destination:Exile}],[r11=MoveObjects{objects:One(This),destination:Exile}]],offered:None}";
    let payment="PayCost{player:You,optional:false,cost:{parts:[PlayerAction{player:You,do:[r11=MoveObjects{objects:One(This),destination:OwnersHand}]}]}}";
    let normalized = |operation: &str| {
        let a = document::read(&card(&format!(
            "{choice},{operation},JoinResults{{results:[r11,r11]}}"
        )))
        .unwrap();
        serde_json::to_value(canonical::unified(a.card()).unwrap()).unwrap()
    };
    assert_eq!(
        normalized(&format!(
            "Together([{payment},DrawCards{{players:You,amount:1}}])"
        )),
        normalized(&format!(
            "Together([{},DrawCards{{players:You,amount:1}}])",
            payment.replace("r11=", "r77=")
        ))
    );
    assert_ne!(
        normalized(payment),
        normalized(&payment.replace("r11=", "r77="))
    );
}

#[test]
fn future_numeric_declarations_do_not_capture_automatically_allocated_names() {
    let source=card("ChooseOne{decider:You,options:[[picked=NoteObjects{objects:One(This)}],[Effect{result:0,operation:NoteObjects{objects:One(This)}}]],offered:None},JoinResults{results:[picked,0]}");
    let doc = document::read(&source).unwrap();
    let value = serde_json::to_value(doc.card()).unwrap();
    let options =
        &value["faces"][0]["abilities"][0]["args"]["body"]["instructions"][0]["args"]["options"];
    assert_ne!(
        options[0]["instructions"][0]["args"]["result"],
        options[1]["instructions"][0]["args"]["result"]
    );
    doc.write().unwrap();
    let source = r#"Card{name:"Numeric target",properties:{card_types:[Instant]},abilities:[Spell{targets:[Target{name:victim,select:Creature},Target{name:0,select:Player}],do:[DealDamage{from:This,to:Targets(victim),amount:3}]}]}"#;
    document::read(source).unwrap().write().unwrap();
    document::read(&card("NoteObjects{objects:One(This)},Effect{result:4294967295,operation:NoteObjects{objects:One(This)}}")).unwrap().write().unwrap();
}

#[test]
fn numeric_references_use_the_actual_named_declaration_in_each_namespace() {
    for source in [
        card("NoteObjects{objects:One(This)},NoteObjects{objects:Selected(4294967295)}"),
        card("WithSelections{steps:[{binding:picked,chooser:Players(You),objects:One(This),count:Exactly(1),constraints:[]}],action:NoteObjects{objects:Prepared(0)}}"),
        card("ForEachPlayer{players:All,binding:each,in_chosen_direction:false,do:[DrawCards{players:One(Bound(0)),amount:1}]}"),
        r#"Card{name:"Numeric target use",properties:{card_types:[Instant]},abilities:[Spell{targets:[Target{name:victim,select:AnyTarget}],do:[DealDamage{from:This,to:Targets(0),amount:3}]}]}"#.into(),
        card("WithSelections{steps:[{binding:0,chooser:Players(You),objects:One(This),count:Exactly(1),constraints:[]}],action:WithSelections{steps:[{binding:0,chooser:Players(You),objects:Prepared(0),count:Exactly(1),constraints:[]}],action:NoteObjects{objects:Prepared(0)}}}"),
    ] {
        document::read(&source).unwrap().write().unwrap_or_else(|e|panic!("{source}: {e}"));
    }
}
