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
    format!("Card{{name:\"Named spell\",properties:{{card_types:[Instant]}},abilities:[Spell{{{body}}}]}}")
}
fn meaning(card: &UnifiedCard) -> Value {
    json!(card
        .faces
        .iter()
        .map(|f| canonical::card(f).unwrap())
        .collect::<Vec<_>>())
}
fn roundtrip(text: &str) -> Document {
    let doc = document::read(text).unwrap();
    let out = doc.write().unwrap();
    let back = document::read(&out).unwrap_or_else(|e| panic!("{e}\n{out}"));
    assert_eq!(out, back.write().unwrap());
    assert_eq!(meaning(&doc.card()), meaning(&back.card()));
    doc
}
#[test]
fn targets_and_unused_results_keep_their_names() {
    let doc=roundtrip(&card("targets:[Target{name:victim,select:AnyTarget}],do:[hit=DealDamage{from:This,to:Targets(victim),amount:3}]"));
    let code = doc.write().unwrap();
    assert!(code.contains("name: victim"), "{code}");
    assert!(code.contains("Targets(victim)"), "{code}");
    assert!(code.contains("hit = DealDamage"), "{code}");
    assert_eq!(doc.bindings().iter().filter(|b| b.explicit).count(), 2);
    assert!(!serde_json::to_string(&doc.card())
        .unwrap()
        .contains("victim"));
}
#[test]
fn names_survive_document_serialization_and_rename_updates_all_uses() {
    let doc=roundtrip(&card("do:[chosen_cards=NoteObjects{objects:One(This)},NoteObjects{objects:Selected(chosen_cards)},NoteObjects{objects:Selected(chosen_cards)}]"));
    let mut back: Document = serde_json::from_str(&serde_json::to_string(&doc).unwrap()).unwrap();
    let binding = back
        .bindings()
        .iter()
        .position(|b| b.name == "chosen_cards")
        .unwrap();
    back.rename(binding, "picked_cards").unwrap();
    let code = back.write().unwrap();
    assert_eq!(code.matches("picked_cards").count(), 3);
    assert!(!code.contains("chosen_cards"));
    assert_eq!(meaning(&doc.card()), meaning(&back.card()));
}
#[test]
fn independent_scopes_and_namespaces_can_reuse_a_spelling() {
    let text = r#"Card{name:"Scopes",properties:{card_types:[Instant]},abilities:[
       Spell{targets:[Target{name:chosen,select:Creature}],do:[chosen=NoteObjects{objects:Targets(chosen)},NoteObjects{objects:Selected(chosen)}]},
       Spell{targets:[Target{name:chosen,select:Player}],do:[chosen=NoteObjects{objects:One(This)},NoteObjects{objects:Selected(chosen)}]}
    ]}"#;
    let mut doc = roundtrip(text);
    let results: Vec<_> = doc
        .bindings()
        .iter()
        .enumerate()
        .filter(|(_, b)| b.kind == BindingKind::Result && b.name == "chosen")
        .map(|(i, b)| (i, b.scope))
        .collect();
    assert_eq!(results.len(), 2);
    assert_ne!(results[0].1, results[1].1);
    doc.rename(results[0].0, "first_choices").unwrap();
    let out = doc.write().unwrap();
    assert!(out.contains("name: chosen"));
    assert!(out.contains("Selected(chosen)"));
    assert!(out.contains("Selected(first_choices)"));
}
#[test]
fn names_in_custom_target_filters_are_not_dropped() {
    let text=card("targets:[Target{name:first,select:Creature},Target{name:other,select:Objects{domain:{kind:Permanent,zones:[Battlefield]},filter:Not(SameObject(Target{slot:first,index:0}))}}],do:[NoteObjects{objects:Targets(other)}]");
    let doc = roundtrip(&text);
    let code = doc.write().unwrap();
    assert!(code.contains("slot: first"), "{code}");
}
#[test]
fn invalid_rename_and_annotation_edits_leave_semantics_protected() {
    let mut doc = roundtrip(&card(
        "do:[one=NoteObjects{objects:One(This)},two=NoteObjects{objects:Selected(one)}]",
    ));
    let one = doc.bindings().iter().position(|b| b.name == "one").unwrap();
    let original = doc.write().unwrap();
    assert!(doc.rename(one, "bad name").is_err());
    assert_eq!(original, doc.write().unwrap());
    let two = doc.bindings().iter().position(|b| b.name == "two").unwrap();
    assert!(doc.rename(two, "one").is_err());
    assert_eq!(original, doc.write().unwrap());
    let mut value = serde_json::to_value(&doc).unwrap();
    value["uses"][0]["path"] =
        json!("/faces/0/abilities/0/args/body/instructions/0/args/operation/args/objects/kind");
    assert!(Document::from_json(&value.to_string())
        .unwrap_err()
        .contains("wrong type or path"));
    let mut value = serde_json::to_value(&doc).unwrap();
    value["bindings"][one]["kind"] = json!("Target");
    assert!(Document::from_json(&value.to_string()).is_err());
    let mut value = serde_json::to_value(&doc).unwrap();
    let duplicate = value["uses"][0].clone();
    value["uses"].as_array_mut().unwrap().push(duplicate);
    assert!(Document::from_json(&value.to_string())
        .unwrap_err()
        .contains("duplicate"));
}
#[test]
fn documents_created_from_ir_keep_the_callers_ids() {
    let mut card = human::read(&card(
        "do:[NoteObjects{objects:One(This)},NoteObjects{objects:One(This)}]",
    ))
    .unwrap();
    if let Ability::Spell { body, .. } = &mut card.faces[0].abilities[0] {
        for (i, instruction) in body.instructions.iter_mut().enumerate() {
            if let Instruction::Effect { result, .. } = instruction {
                result.0 = 17 + i as u32;
            }
        }
    }
    let mut doc = document::from_card(&card).unwrap();
    assert_eq!(
        serde_json::to_value(&card).unwrap(),
        serde_json::to_value(&doc.card()).unwrap()
    );
    let binding = doc
        .uses()
        .iter()
        .find(|u| u.path.ends_with("/0/args/result"))
        .unwrap()
        .binding;
    doc.rename(binding, "remembered").unwrap();
    assert!(doc.write().unwrap().contains("remembered ="));
    assert_eq!(
        serde_json::to_value(&card).unwrap(),
        serde_json::to_value(&doc.card()).unwrap()
    );
}
#[test]
fn loops_and_preparations_keep_local_names_and_typed_uses() {
    let text=card("do:[ForEachPlayer{players:All,binding:each,starting_with:None,in_chosen_direction:false,do:[DrawCards{players:One(Bound(each)),amount:1},ForEachObject{objects:One(This),binding:each,do:[NoteObjects{objects:One(Bound(each))}]},DrawCards{players:One(Bound(each)),amount:1}]},ForEachPlayer{players:All,binding:each,starting_with:None,in_chosen_direction:false,do:[DrawCards{players:One(Bound(each)),amount:1}]}]");
    let doc = roundtrip(&text);
    let loops: Vec<_> = doc
        .bindings()
        .iter()
        .enumerate()
        .filter(|(_, b)| b.kind == BindingKind::Loop && b.name == "each")
        .map(|(i, b)| (i, b.scope))
        .collect();
    assert_eq!(loops.len(), 3);
    assert_ne!(loops[0].1, loops[1].1);
    assert_ne!(loops[0].1, loops[2].1);
    let text=card("do:[picked=WithSelections{steps:[{binding:picked,chooser:Players(You),objects:One(This),count:Exactly(1),constraints:[]}],action:NoteObjects{objects:Prepared(picked)}},WithSelections{steps:[{binding:picked,chooser:Players(You),objects:One(This),count:Exactly(1),constraints:[]}],action:NoteObjects{objects:Prepared(picked)}}]");
    let doc = roundtrip(&text);
    assert_eq!(
        doc.bindings()
            .iter()
            .filter(|b| b.kind == BindingKind::Selection && b.name == "picked")
            .count(),
        2
    );
    assert!(document::read(&text.replace(
        "action:NoteObjects{objects:Prepared(picked)}",
        "action:NoteObjects{objects:Prepared(wrong)}"
    ))
    .is_err());
}
#[test]
fn explicit_numeric_looking_result_names_are_preserved() {
    let doc = roundtrip(&card("do:[r123=NoteObjects{objects:One(This)}]"));
    assert!(doc.write().unwrap().contains("r123 ="));
}
#[test]
fn delayed_closures_capture_outer_names_and_keep_local_names_separate() {
    let text=card("do:[saved=NoteObjects{objects:One(This)},CreateDelayedTrigger{event:Turn{point:EndStepBegins,players:All},start:Immediately,expires:None,intervening_if:None,do:[saved=NoteObjects{objects:Selected(saved)},NoteObjects{objects:Selected(saved)}]},NoteObjects{objects:Selected(saved)}]");
    // A result is bound after its operation's inputs are evaluated, as on the
    // ordinary instruction sequence. Use different local names for this probe.
    let text=text.replace("do:[saved=NoteObjects{objects:Selected(saved)},NoteObjects{objects:Selected(saved)}]","do:[captured=NoteObjects{objects:Selected(saved)},NoteObjects{objects:Selected(captured)}]");
    let doc = roundtrip(&text);
    let saved = doc
        .bindings()
        .iter()
        .position(|b| b.name == "saved")
        .unwrap();
    let captured = doc
        .bindings()
        .iter()
        .position(|b| b.name == "captured")
        .unwrap();
    assert_ne!(doc.bindings()[saved].scope, doc.bindings()[captured].scope);
    assert_eq!(doc.uses().iter().filter(|u| u.binding == saved).count(), 3);
    assert_eq!(
        doc.uses().iter().filter(|u| u.binding == captured).count(),
        2
    );
}
#[test]
fn same_spelling_and_numeric_ids_cannot_smuggle_a_reference_across_scopes() {
    let text = r#"Card{name:"Local",properties:{card_types:[Instant]},abilities:[
       Spell{targets:[Target{name:t0,select:Creature}],do:[NoteObjects{objects:Targets(t0)}]},
       Spell{targets:[Target{name:t0,select:Creature}],do:[NoteObjects{objects:Targets(t0)}]}
    ]}"#;
    let doc = roundtrip(text);
    let targets: Vec<_> = doc
        .bindings()
        .iter()
        .enumerate()
        .filter(|(_, b)| b.kind == BindingKind::Target)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(targets.len(), 2);
    let mut value = serde_json::to_value(&doc).unwrap();
    let second = value["uses"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|site| {
            site["binding"] == targets[1]
                && site["path"].as_str().unwrap().ends_with("/objects/args")
        })
        .unwrap();
    second["binding"] = json!(targets[0]);
    assert!(Document::from_json(&value.to_string())
        .unwrap_err()
        .contains("lexical scope"));
}
#[test]
fn replacement_and_optional_bodies_do_not_alias_later_results() {
    let text=card("do:[CreateReplacement{event:Enter{objects:Any},limit:UntilExpired,duration:EndOfTurn,do:[local=NoteObjects{objects:One(This)},NoteObjects{objects:Selected(local)}]},local=NoteObjects{objects:One(This)},NoteObjects{objects:Selected(local)}]");
    let doc = roundtrip(&text);
    let locals: Vec<_> = doc
        .bindings()
        .iter()
        .filter(|b| b.name == "local")
        .collect();
    assert_eq!(locals.len(), 2);
    assert_ne!(locals[0].scope, locals[1].scope);
    let text=card("do:[Optional{decider:You,do:[r0=NoteObjects{objects:One(This)},NoteObjects{objects:Selected(r0)}]},r0=NoteObjects{objects:One(This)},NoteObjects{objects:Selected(r0)}]");
    let mut doc = roundtrip(&text);
    let locals: Vec<_> = doc
        .bindings()
        .iter()
        .enumerate()
        .filter(|(_, b)| b.name == "r0")
        .map(|(i, b)| (i, b.scope))
        .collect();
    assert_eq!(locals.len(), 2);
    assert_ne!(locals[0].1, locals[1].1);
    doc.rename(locals[0].0, "optional_objects").unwrap();
    let code = doc.write().unwrap();
    assert_eq!(code.matches("optional_objects").count(), 2);
    assert!(code.contains("Selected(r0)"));
}
#[test]
fn pure_id_renumbering_preserves_names_and_structural_edits_require_mapping() {
    let doc=roundtrip(&card("targets:[Target{name:target,select:Creature}],do:[saved=NoteObjects{objects:Targets(target)},NoteObjects{objects:Selected(saved)}]"));
    let mut value = serde_json::to_value(doc.card()).unwrap();
    for site in doc.uses() {
        let leaf = value.pointer_mut(&site.path).unwrap();
        let id = leaf.as_u64().unwrap();
        *leaf = json!(if id > u32::MAX as u64 - 100 {
            id - 100
        } else {
            id + 100
        });
    }
    let card: UnifiedCard = serde_json::from_value(value).unwrap();
    let rebound = doc.rebind_ids(&card).unwrap();
    let text = rebound.write().unwrap();
    assert!(text.contains("name: target"));
    assert!(text.contains("Selected(saved)"));
    assert_eq!(meaning(doc.card()), meaning(rebound.card()));
    let mut changed = card.clone();
    changed.faces[0].name = "Structural change".into();
    assert!(doc.rebind_ids(&changed).unwrap_err().contains("structural"));
}
#[test]
fn player_and_object_loops_can_share_raw_ids_without_sharing_names() {
    let text=card("do:[ForEachPlayer{players:All,binding:b0,starting_with:None,in_chosen_direction:false,do:[ForEachObject{objects:One(This),binding:b0,do:[DrawCards{players:One(Bound(b0)),amount:1},NoteObjects{objects:One(Bound(b0))}]}]}]");
    let mut doc = roundtrip(&text);
    let loops: Vec<_> = doc
        .bindings()
        .iter()
        .enumerate()
        .filter(|(_, b)| b.kind == BindingKind::Loop)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(loops.len(), 2);
    doc.rename_many(&[
        (loops[0], "each_player".into()),
        (loops[1], "each_object".into()),
    ])
    .unwrap();
    let code = doc.write().unwrap();
    assert!(code.contains("players: One(Bound(each_player))"), "{code}");
    assert!(code.contains("objects: One(Bound(each_object))"), "{code}");
}
#[test]
fn object_shadowing_can_be_renamed_independently() {
    let text=card("do:[ForEachObject{objects:One(This),binding:b0,do:[ForEachObject{objects:One(Bound(b0)),binding:b0,do:[NoteObjects{objects:One(Bound(b0))}]},NoteObjects{objects:One(Bound(b0))}]}]");
    let mut doc = roundtrip(&text);
    let loops: Vec<_> = doc
        .bindings()
        .iter()
        .enumerate()
        .filter(|(_, b)| b.kind == BindingKind::Loop)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(loops.len(), 2);
    doc.rename_many(&[
        (loops[0], "outer_object".into()),
        (loops[1], "inner_object".into()),
    ])
    .unwrap();
    let code = doc.write().unwrap();
    assert!(code.contains("Bound(outer_object)"));
    assert!(code.contains("Bound(inner_object)"));
}
#[test]
fn cost_and_quoted_programs_keep_their_local_names_separate() {
    let text = r#"Card{name:"Cost scopes",properties:{card_types:[Artifact]},abilities:[Activated{origin:Ordinary,cost:{parts:[PlayerAction{player:You,do:[local=NoteObjects{objects:One(This)},NoteObjects{objects:Selected(local)}]}]},do:[local=NoteObjects{objects:One(This)},NoteObjects{objects:Selected(local)}]}]}"#;
    let doc = roundtrip(text);
    assert_eq!(
        doc.bindings()
            .iter()
            .filter(|b| b.kind == BindingKind::Result && b.name == "local")
            .count(),
        2
    );
    let text=card("do:[MoveTopCard{actor:You,cards:Any,reveal:false,destination:OwnersBattlefield,entry:{states:[RulesUntil{rules:[Program([local=NoteObjects{objects:One(This)},NoteObjects{objects:Selected(local)}])],until:EndOfTurn}]}},local=NoteObjects{objects:One(This)},NoteObjects{objects:Selected(local)}]");
    let doc = roundtrip(&text);
    assert_eq!(
        doc.bindings()
            .iter()
            .filter(|b| b.kind == BindingKind::Result && b.name == "local")
            .count(),
        2
    );
}
#[test]
fn after_event_programs_keep_private_result_names() {
    let text=card("do:[CreateReplacement{event:Enter{objects:Any},limit:UntilExpired,duration:EndOfTurn,do:[ModifyEvent{change:Then([local=NoteObjects{objects:One(This)},NoteObjects{objects:Selected(local)}])},local=NoteObjects{objects:One(This)},NoteObjects{objects:Selected(local)}]}]");
    let doc = roundtrip(&text);
    assert_eq!(
        doc.bindings()
            .iter()
            .filter(|b| b.kind == BindingKind::Result && b.name == "local")
            .count(),
        2
    );
}
#[test]
fn payment_parts_export_the_last_same_id_receipt_after_payment() {
    let text=card("do:[PayCost{player:You,optional:false,cost:{parts:[PlayerAction{player:You,do:[r0=MoveObjects{objects:One(This),destination:OwnersHand}]},PlayerAction{player:You,do:[r0=MoveObjects{objects:One(This),destination:OwnersGraveyard}]}]}},NoteObjects{objects:Transition{result:r0,phase:After}}]");
    let mut doc = roundtrip(&text);
    let receipts: Vec<_> = doc
        .bindings()
        .iter()
        .enumerate()
        .filter(|(_, b)| b.kind == BindingKind::Result && b.name == "r0")
        .map(|(i, _)| i)
        .collect();
    assert_eq!(receipts.len(), 2);
    doc.rename_many(&[
        (receipts[0], "first_paid".into()),
        (receipts[1], "last_paid".into()),
    ])
    .unwrap();
    let code = doc.write().unwrap();
    assert!(code.contains("result: last_paid"), "{code}");
}
#[test]
fn activation_cost_modifiers_keep_their_named_target() {
    let text = r#"Card{name:"Activation targets",properties:{card_types:[Artifact]},abilities:[Activated{origin:Ordinary,cost:{parts:[]},targets:[Target{name:recipient,select:Creature}],limits:{modifiers:[ActivationRule(Cost{modifier:Reduce{reductions:[{color:None,amount:CountCounters{recipients:Targets(recipient),kind:PowerToughness(1,1),information:CurrentOrLastKnown}}],spill_into_generic:true},condition:None})]},do:[] }]}"#;
    let mut doc = roundtrip(text);
    let target = doc
        .bindings()
        .iter()
        .position(|b| b.kind == BindingKind::Target)
        .unwrap();
    doc.rename(target, "equipped_creature").unwrap();
    assert!(doc.write().unwrap().contains("Targets(equipped_creature)"));
}

#[test]
#[ignore = "explicit normalized corpus"]
fn all_records_support_the_authoring_document() {
    use std::io::BufRead;
    mtgish_ast::corpus::on_large_stack(|| {
        let file = std::fs::File::open(std::env::var("AUTHORING_CORPUS").unwrap()).unwrap();
        let mut count = 0;
        let mut failures = Vec::new();
        for line in std::io::BufReader::new(file).lines() {
            let card: UnifiedCard = serde_json::from_str(&line.unwrap()).unwrap();
            let result = document::from_card(&card).and_then(|mut doc| {
                let renames: Vec<_> = (0..doc.bindings().len())
                    .map(|i| (i, format!("named_{i}")))
                    .collect();
                doc.rename_many(&renames)
            });
            if let Err(error) = result {
                let message = format!(
                    "{:?}: {error}",
                    card.faces.iter().map(|f| &f.name).collect::<Vec<_>>()
                );
                eprintln!("{message}");
                failures.push(message);
            }
            count += 1;
        }
        assert_eq!(count, 33529);
        assert!(
            failures.is_empty(),
            "{} name-audit failures:\n{}",
            failures.len(),
            failures.join("\n")
        );
        println!("Renamed every binding in all {count} records");
    });
}

#[test]
#[ignore = "explicit normalized scope cohort"]
fn real_control_flow_scopes_support_distinct_names_for_every_binding() {
    use mtgish_ast::visit::{Visitor, Walk};
    use std::io::BufRead;
    struct Scoped(bool);
    impl Visitor for Scoped {
        fn instruction(&mut self, i: &Instruction) -> bool {
            self.0 |= matches!(
                i,
                Instruction::Optional { .. }
                    | Instruction::Times { .. }
                    | Instruction::Repeatable(_)
            );
            true
        }
        fn operation(&mut self, o: &Operation) -> bool {
            self.0 |= matches!(
                o,
                Operation::CreateReplacement { .. }
                    | Operation::CreateTrigger { .. }
                    | Operation::ReflexiveTrigger { .. }
                    | Operation::CreateDelayedTrigger { .. }
            );
            true
        }
    }
    mtgish_ast::corpus::on_large_stack(|| {
        let file = std::fs::File::open(std::env::var("AUTHORING_CORPUS").unwrap()).unwrap();
        let mut count = 0;
        for line in std::io::BufReader::new(file).lines() {
            let card: UnifiedCard = serde_json::from_str(&line.unwrap()).unwrap();
            let mut scoped = Scoped(false);
            card.walk(&mut scoped);
            if !scoped.0 {
                continue;
            }
            let mut doc = document::from_card(&card).unwrap();
            let renames: Vec<_> = (0..doc.bindings().len())
                .map(|i| (i, format!("named_{i}")))
                .collect();
            doc.rename_many(&renames).unwrap_or_else(|e| {
                panic!(
                    "{:?}: {e}",
                    card.faces.iter().map(|f| &f.name).collect::<Vec<_>>()
                )
            });
            count += 1;
        }
        assert!(count > 0);
        println!("Renamed every binding in {count} scope-cohort records");
    });
}
