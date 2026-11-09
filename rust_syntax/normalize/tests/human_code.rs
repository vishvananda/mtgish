use mtgish_ast::{canonical, corpus, human, ir::*, mtg_types as source};
use serde_json::Value;
fn meaning(v: &UnifiedCard) -> Value {
    let mut v = serde_json::to_value(v).unwrap();
    for face in v["faces"].as_array_mut().unwrap() {
        let c: Card = serde_json::from_value(face.clone()).unwrap();
        *face = serde_json::to_value(canonical::card(&c).unwrap()).unwrap();
    }
    fn metadata(v: &mut Value) {
        match v {
            Value::Object(fields) => {
                for (key, value) in fields {
                    if key == "abilities" {
                        canonical::abilities(value).unwrap();
                    } else {
                        metadata(value);
                    }
                }
            }
            Value::Array(items) => {
                for v in items {
                    metadata(v);
                }
            }
            _ => (),
        }
    }
    metadata(&mut v["data"]);
    v
}
#[test]
fn bolt_has_one_readable_action_and_safe_defaults() {
    let code = r#"Card {
        name: "Lightning Bolt",
        properties: {card_types: [Instant]},
        abilities: [Spell {
            targets: [Target {name: victim, select: AnyTarget}],
            do: [DealDamage {amount: 3, from: This, to: Targets(victim)}]
        }]
    }"#;
    let card = human::read(code).unwrap();
    let text = human::write(&card).unwrap();
    assert!(!text.contains("result"));
    assert!(!text.contains("Effect"));
    assert!(!text.contains("preventable"));
    assert!(!text.contains("schema"));
    assert!(text.contains("AnyTarget"));
    assert_eq!(meaning(&card), meaning(&human::read(&text).unwrap()));
    let code = code.replace("amount: 3,", "amount: 3, unpreventable: true,");
    let card = human::read(&code).unwrap();
    assert_eq!(
        serde_json::to_value(&card).unwrap()["faces"][0]["abilities"][0]["args"]["body"]
            ["instructions"][0]["args"]["operation"]["args"]["preventable"],
        false
    );
    assert!(human::write(&card).unwrap().contains("unpreventable: true"));
}
#[test]
fn symbolic_results_survive_anonymous_actions_and_checked_scope() {
    let text = r#"Card { name:"Example", properties:{card_types:[Instant]}, abilities:[Spell {
        do:[DrawCards{players:You,amount:1},saved = NoteObjects{objects:One(This)},
            NoteObjects{objects:Selected(saved)}]
    }] }"#;
    let card = human::read(text).unwrap();
    assert_eq!(
        meaning(&card),
        meaning(&human::read(&human::write(&card).unwrap()).unwrap())
    );
    assert!(human::read(&text.replace("Selected(saved)", "Selected(missing)")).is_err());
    assert!(human::read(
        &text
            .replace("saved = NoteObjects", "r4294967295 = NoteObjects")
            .replace("Selected(saved)", "Selected(r4294967295)")
    )
    .is_ok());
    assert!(human::read(&text.replace("amount:1", "amount:9223372036854775807")).is_ok());
    assert!(human::read(&text.replace("amount:1", "amount:9223372036854775808")).is_err());
}
#[test]
fn omissions_preserve_none_zero_empty_and_reject_typos_or_duplicate_aliases() {
    let code = r#"Card {name:"Zero",properties:{card_types:[Creature],mana_cost:[],power_toughness:[0,0]}}"#;
    let card = human::read(code).unwrap();
    let output = human::write(&card).unwrap();
    assert!(output.contains("mana_cost: []"));
    assert!(output.contains("[0, 0]"));
    assert!(card.faces[0]
        .characteristics
        .mana_cost
        .as_ref()
        .is_some_and(Vec::is_empty));
    let absent = human::read(&code.replace(",mana_cost:[]", "")).unwrap();
    assert!(absent.faces[0].characteristics.mana_cost.is_none());
    assert!(human::read(&code.replace("properties:", "propertiez:")).is_err());
    assert!(
        human::read(&code.replace("name:\"Zero\"", "name:\"Zero\",name:\"Duplicate\"")).is_err()
    );
    assert!(human::read(&format!("{code} trailing")).is_err());
}
#[test]
fn complete_cards_roundtrip_across_receipts_modes_and_multiple_faces() {
    corpus::on_large_stack(|| {
        let lines = include_str!("fixtures/joint_blink_reference_fixes.ronl")
            .lines()
            .chain(include_str!("fixtures/return_followup_reference_fixes.ronl").lines())
            .chain(include_str!("fixtures/reference_actors_20261108.ronl").lines());
        for line in lines {
            let source: source::OracleCard = ron::from_str(line).unwrap();
            let card = mtgish_ast::unified::lower(&source).card;
            let text = human::write(&card).unwrap();
            let back = human::read(&text)
                .unwrap_or_else(|e| panic!("{}: {e}\n{text}", card.faces[0].name));
            assert_eq!(meaning(&card), meaning(&back), "{}", card.faces[0].name);
        }
    });
}
#[test]
fn bindings_and_names_are_local_and_boolean_state_changes_stay_explicit() {
    let code = r#"Card {name:"Scopes",properties:{card_types:[Creature]},abilities:[
        Spell {do:[DrawCards{players:You,amount:1}]},
        Triggered {event:Turn{point:EndStepBegins,players:All},do:[
            r0=NoteObjects{objects:One(This)},NoteObjects{objects:Selected(r0)}
        ]}
    ]}"#;
    let card = human::read(code).unwrap();
    let mut value = serde_json::to_value(card).unwrap();
    value["faces"][0]["abilities"][0]["args"]["body"]["instructions"][0]["args"]["result"] =
        serde_json::json!(0);
    let card: UnifiedCard = serde_json::from_value(value).unwrap();
    let text = human::write(&card).unwrap();
    assert_eq!(text.matches(" = ").count(), 1, "{text}");
    assert_eq!(meaning(&card), meaning(&human::read(&text).unwrap()));
    // Symbolic names cannot leak into another ability, even when raw numeric
    // IDs are present in that ability's explicit representation.
    let leak = r#"Card {name:"Scopes",properties:{card_types:[Creature]},abilities:[
        Spell{do:[saved=NoteObjects{objects:One(This)}]},
        Spell{do:[Effect{result:0,operation:NoteObjects{objects:One(This)}},
            NoteObjects{objects:Selected(saved)}]}
    ]}"#;
    assert!(human::read(leak).is_err());
    let change = r#"Card {name:"Tap",properties:{card_types:[Instant]},abilities:[Spell{do:[SetTapped{objects:One(This),tapped:false}]}]}"#;
    assert!(human::write(&human::read(change).unwrap())
        .unwrap()
        .contains("tapped: false"));
    assert!(human::read(&change.replace(",tapped:false", "")).is_err());
    let meta = human::editor_metadata();
    assert_eq!(meta["actions"]["DealDamage"]["category"], "Damage & life");
    assert_eq!(
        meta["references"]["PlayerRef"]["EventSubject"],
        "EventPlayer"
    );
    assert_eq!(
        meta["references"]["ObjectRef"]["EventSubject"],
        "EventObject"
    );
}
#[test]
#[ignore = "explicit current normalized corpus; full corpus is not a unit-test fixture"]
fn entire_current_corpus_roundtrips_through_card_code() {
    corpus::on_large_stack(|| {
        use std::io::BufRead;
        let file = std::env::var("HUMAN_CODE_CORPUS").unwrap();
        let mut count = 0;
        for line in std::io::BufReader::new(std::fs::File::open(file).unwrap()).lines() {
            let card: UnifiedCard = serde_json::from_str(&line.unwrap()).unwrap();
            let text = human::write(&card).unwrap_or_else(|e| {
                panic!(
                    "{:?}: {e}",
                    card.faces.iter().map(|f| &f.name).collect::<Vec<_>>()
                )
            });
            let back = human::read(&text).unwrap_or_else(|e| {
                panic!(
                    "{:?}: {e}\n{text}",
                    card.faces.iter().map(|f| &f.name).collect::<Vec<_>>()
                )
            });
            assert_eq!(
                meaning(&card),
                meaning(&back),
                "{:?}\n{text}",
                card.faces.iter().map(|f| &f.name).collect::<Vec<_>>()
            );
            count += 1;
            if count % 5000 == 0 {
                println!("Human code: {count} records");
            }
        }
        assert_eq!(count, 33529);
        println!("Human code: all {count} records valid and semantically equal");
    });
}
