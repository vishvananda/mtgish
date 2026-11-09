use mtgish_ast::{human, ir::*, migrate, validate};
use serde_json::{json, Value};

#[test]
fn persisted_entry_flags_upgrade_to_typed_fields() {
    for line in include_str!("fixtures/entry_payloads_v179.jsonl").lines() {
        let old: Value = serde_json::from_str(line).unwrap();
        let card =
            migrate::read_json(line).unwrap_or_else(|e| panic!("{}: {e}", old["faces"][0]["name"]));
        assert_eq!(card.schema_version, SCHEMA_VERSION);
        for face in &card.faces {
            assert_eq!(face.schema_version, SCHEMA_VERSION);
            validate::card(face).unwrap();
        }
        let code = human::write(&card).unwrap();
        assert!(!code.contains("EntersNormally"));
        assert!(!code.contains("EntersTapped"));
        // Other independent permission/effect payloads may still use this
        // vocabulary; this cohort removes grammar entry flags specifically.

        let again = human::read(&code).unwrap();
        for (a, b) in card.faces.iter().zip(&again.faces) {
            assert_eq!(
                serde_json::to_value(mtgish_ast::canonical::card(a).unwrap()).unwrap(),
                serde_json::to_value(mtgish_ast::canonical::card(b).unwrap()).unwrap()
            );
        }
    }
}

#[test]
fn delay_retains_time_counters_and_granted_suspend() {
    let line = include_str!("fixtures/entry_payloads_v179.jsonl")
        .lines()
        .find(|line| line.contains("\"name\":\"Delay\"") || line.contains("\"name\": \"Delay\""))
        .unwrap();
    let card = migrate::read_json(line).unwrap();
    struct Find(bool);
    impl mtgish_ast::visit::Visitor for Find {
        fn operation(&mut self, operation: &Operation) -> bool {
            if let Operation::CounterWith { entry, .. } = operation {
                assert_eq!(entry.counters.len(), 1);
                assert!(matches!(entry.counters[0].1, Expr::Constant(3)));
                assert!(
                    matches!(entry.states.as_slice(), [EntryState::Changes(changes)]
                    if matches!(changes.as_slice(), [Change::GrantAbility(ability)]
                        if matches!(&**ability, Ability::Mechanic {mechanic: Mechanic::BasicSuspend, ..})))
                );
                self.0 = true;
            }
            true
        }
    }
    use mtgish_ast::visit::Walk;
    let mut find = Find(false);
    card.walk(&mut find);
    assert!(find.0);
}

#[test]
fn current_entry_language_rejects_operand_bags() {
    for operation in [
        "MoveTopCard{actor:You,cards:Any,reveal:false,destination:OwnersHand,entry:[Integer(42)]}",
        "Populate{actor:You,entry:[Integer(42)]}",
        "CounterWith{object:This,destination:Exile,entry:[Integer(42)]}",
    ] {
        let text=format!("Card{{name:\"Invalid entry\",properties:{{card_types:[Instant]}},abilities:[Spell{{do:[{operation}]}}]}}");
        assert!(human::read(&text).is_err());
    }
    human::read("Card{name:\"Typed entry\",properties:{card_types:[Instant]},abilities:[Spell{do:[MoveTopCard{actor:You,cards:Any,reveal:false,destination:OwnersBattlefield,entry:{tapped:true}}]}]}").unwrap();
}

#[test]
fn upgrades_never_drop_unknown_fields_or_unreviewed_flags() {
    let original: Value = serde_json::from_str(
        include_str!("fixtures/entry_payloads_v179.jsonl")
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    let mut extra = original.clone();
    extra["unexpected"] = json!(true);
    assert!(migrate::read_json(&extra.to_string()).is_err());
    let mut mixed = original.clone();
    mixed["faces"][0]["schema_version"] = json!(0);
    assert!(migrate::read_json(&mixed.to_string()).is_err());
    let mut epoch = original;
    epoch["schema_version"] = json!(0);
    assert!(migrate::read_json(&epoch.to_string()).is_err());

    let mut invalid = json!({"schema_version":179,"layout":"Card","data":[],"faces":[{
        "schema_version":179,"name":"Invalid legacy arrival",
        "characteristics":{"supertypes":[],"card_types":["Instant"],"subtypes":[],"mana_cost":null,
            "color_indicator":[],"power_toughness":null,"loyalty":null,"defense":null},
        "abilities":[{"kind":"Spell","args":{"announcement":{"targets":[],"modes":null},
            "body":{"instructions":[{"kind":"Effect","args":{"result":0,"operation":{
                "kind":"Populate","args":{"actor":{"kind":"One","args":{"kind":"AbilityController"}},
                    "entry":[{"kind":"Integer","args":42}]}}}}]}}}]}]});
    assert!(migrate::read_json(&invalid.to_string()).is_err());
    invalid["faces"][0]["abilities"][0]["args"]["body"]["instructions"][0]["args"]["operation"]
        ["args"]["entry"] = json!([]);
    migrate::read_json(&invalid.to_string()).unwrap();
}

#[test]
fn typed_controller_overrides_have_the_correct_reference_scope() {
    let text = "Card{name:\"Controller\",properties:{card_types:[Instant]},abilities:[Spell{do:[Populate{actor:You,entry:{controller:Player(Target{slot:t99,index:0})}}]}]}";
    assert!(human::read(text)
        .unwrap_err()
        .contains("unbound target slot 99"));
    human::read(&text.replace("Target{slot:t99,index:0}", "You")).unwrap();
}

#[test]
#[ignore = "explicit before/current full normalized corpora"]
fn upgrade_matches_current_source_lowering_for_every_record() {
    use std::io::BufRead;
    mtgish_ast::corpus::on_large_stack(|| {
        let before = std::fs::File::open(std::env::var("ENTRY_BEFORE_CORPUS").unwrap()).unwrap();
        let after = std::fs::File::open(std::env::var("ENTRY_CURRENT_CORPUS").unwrap()).unwrap();
        let mut old = std::io::BufReader::new(before).lines();
        let mut new = std::io::BufReader::new(after).lines();
        let mut count = 0;
        loop {
            match (old.next(), new.next()) {
                (None, None) => break,
                (Some(a), Some(b)) => {
                    let a = migrate::read_json(&a.unwrap()).unwrap();
                    let b: UnifiedCard = serde_json::from_str(&b.unwrap()).unwrap();
                    validate::unified(&b).unwrap();
                    for face in &b.faces {
                        validate::card(face).unwrap();
                    }
                    assert_eq!(
                        serde_json::to_value(&a).unwrap(),
                        serde_json::to_value(&b).unwrap(),
                        "{:?}",
                        b.faces.iter().map(|f| &f.name).collect::<Vec<_>>()
                    );
                    count += 1;
                }
                _ => panic!("corpus length changed"),
            }
        }
        assert_eq!(count, 33529);
        println!("All {count} upgrades exactly match current lowering");
    });
}

#[test]
fn top_card_destinations_are_checked_in_the_current_scope() {
    let code = "Card{name:\"Destination\",properties:{card_types:[Instant]},abilities:[Spell{do:[MoveTopCard{actor:You,cards:Any,reveal:false,destination:Battlefield(Target{slot:t99,index:0}),entry:{}}]}]}";
    assert!(human::read(code)
        .unwrap_err()
        .contains("unbound target slot 99"));
    human::read(&code.replace("Target{slot:t99,index:0}", "You")).unwrap();
}

#[test]
fn migration_accepts_browser_depth_and_rejects_excess_nesting() {
    let mut card = human::read("Card{name:\"Deep value\",properties:{card_types:[Instant]},abilities:[Spell{do:[DrawCards{players:You,amount:1}]}]}").unwrap();
    let Ability::Spell { body, .. } = &mut card.faces[0].abilities[0] else {
        panic!()
    };
    let Instruction::Effect {
        operation: Operation::Draw { amount, .. },
        ..
    } = &mut body.instructions[0]
    else {
        panic!()
    };
    for _ in 0..65 {
        *amount = Expr::Add(Box::new(amount.clone()), Box::new(Expr::Constant(0)));
    }
    let text = serde_json::to_string(&card).unwrap();
    // The default JSON recursion limit is below the supported browser depth.
    assert!(serde_json::from_str::<Value>(&text).is_err());
    migrate::read_json(&text).unwrap();
    let excessive = "[".repeat(256) + &"]".repeat(256);
    assert!(migrate::read_json(&excessive)
        .unwrap_err()
        .contains("255 levels"));
}
