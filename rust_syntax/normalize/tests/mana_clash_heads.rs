use mtgish_ast::{canonical, corpus, human, ir::*, lower, mtg_types as source, unified, validate};
use serde_json::{json, Value};
fn source(rule: &str) -> source::OracleCard {
    ron::from_str(&format!("Card(Name:\"Coin batch\",Typeline:(Supertypes:[],Cardtypes:[Instant],Subtypes:[]),Rules:[{rule}])")).unwrap()
}
fn batch() -> &'static str {
    "EachPlayerAction(Or([SinglePlayer(You),SinglePlayer(Ref_TargetPlayer)]),FlipACoin)"
}
fn all_heads() -> Condition {
    Condition::All(vec![
        Condition::CoinCameUp {
            player: Some(PlayerRef::AbilityController),
            outcome: CoinOutcome::Heads,
        },
        Condition::CoinCameUp {
            player: Some(PlayerRef::Target {
                slot: TargetSlot(0),
                index: 0,
            }),
            outcome: CoinOutcome::Heads,
        },
    ])
}

#[test]
fn mana_clash_changes_only_its_collective_head_check_and_keeps_private_flip_receipts() {
    corpus::on_large_stack(|| {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/mtgish.lines.ron");
        let s = corpus::read_cards(&path)
            .unwrap()
            .into_iter()
            .find(|s| {
                corpus::Identity::of(s)
                    .unwrap()
                    .names
                    .contains(&"Mana Clash".to_string())
            })
            .unwrap();
        let card = unified::lower(&s).card;
        validate::unified(&card).unwrap();
        validate::card(&card.faces[0]).unwrap();
        let mut expected: Value =
            serde_json::from_str(include_str!("fixtures/mana_clash_v184_before.json")).unwrap();
        let ptr =
            "/faces/0/abilities/0/args/body/instructions/0/args/instructions/2/args/condition";
        assert_eq!(
            expected.pointer(ptr).unwrap(),
            &json!({"kind":"Not","args":{"kind":"Compare","args":{"left":{"kind":"CoinFlipsWon","args":false},"comparison":"Equal","right":{"kind":"Constant","args":0}}}})
        );
        // Upgrade the authentic capture structurally; retain its original,
        // separately corrected heads/wins condition until the assertion below.
        expected = serde_json::to_value(mtgish_ast::migrate::read_json(&expected.to_string()).unwrap()).unwrap();
        *expected.pointer_mut(ptr).unwrap() =
            serde_json::to_value(Condition::Not(Box::new(all_heads()))).unwrap();
        assert_eq!(serde_json::to_value(&card).unwrap(), expected);
        let mut named = human::document::from_card(&card).unwrap();
        let names: Vec<_> = named
            .bindings()
            .iter()
            .enumerate()
            .map(|(i, _)| (i, format!("coin_name_{i}")))
            .collect();
        named.rename_many(&names).unwrap();
        let text = named.write().unwrap();
        assert!(!text.contains("CoinFlipsWon"));
        let back = human::document::read(&text).unwrap();
        assert_eq!(
            serde_json::to_value(named.bindings()).unwrap(),
            serde_json::to_value(back.bindings()).unwrap()
        );
        assert_eq!(
            serde_json::to_value(canonical::mapped_unified(&card).unwrap().card).unwrap(),
            serde_json::to_value(canonical::mapped_unified(back.card()).unwrap().card).unwrap()
        );
    });
}
#[test]
fn absent_or_replaced_batches_cannot_supply_a_collective_head_condition() {
    for middle in [
        "",
        "FlipACoin,",
        "FlipCoins(Integer(2)),",
        "FlipACoinUntilLose,",
    ] {
        let actions = if middle.is_empty() {
            "If(AllCoinsCameUpHeads,[DrawACard])".to_string()
        } else {
            format!("{},{}If(AllCoinsCameUpHeads,[DrawACard])", batch(), middle)
        };
        let s = source(&format!(
            "SpellActions(Targeted([TargetPlayer(Opponent)],ActionList([{actions}])))"
        ));
        let error = lower::card(&s).unwrap_err();
        assert!(
            error
                .iter()
                .any(|e| e.message.contains("one-flip-per-player batch")),
            "{}",
            serde_json::to_string(&error).unwrap()
        );
    }
    for action in ["FlipCoins(Integer(2))", "PlayerAction(You,FlipACoin)"] {
        let s=source(&format!("SpellActions(Targeted([TargetPlayer(Opponent)],ActionList([EachPlayerAction(Or([SinglePlayer(You),SinglePlayer(Ref_TargetPlayer)]),{action}),If(AllCoinsCameUpHeads,[DrawACard])])))"));
        assert!(lower::card(&s).is_err(), "unproven batch {action}");
    }
    let s=source(&format!("SpellActions(Targeted([TargetPlayer(Opponent)],ActionList([{},MayAction(FlipACoin),If(AllCoinsCameUpHeads,[DrawACard])])))",batch()));
    assert!(lower::card(&s).is_err());
    let s=source(&format!("SpellActions(Targeted([TargetPlayer(Opponent)],ActionList([{}]))),SpellActions(ActionList([If(AllCoinsCameUpHeads,[DrawACard])]))",batch()));
    assert!(lower::card(&s).is_err());
}
#[test]
fn heads_predicate_uses_both_participants_and_ignores_unrelated_flips() {
    // Independently interpret the existing conjunction and per-player outcome
    // vocabulary, without the lowerer's proof bookkeeping or player enumeration.
    fn value(condition: &Condition, heads: u8) -> bool {
        match condition {
            Condition::All(conditions) => conditions.iter().all(|c| value(c, heads)),
            Condition::CoinCameUp {
                player: Some(PlayerRef::AbilityController),
                outcome: CoinOutcome::Heads,
            } => heads & 1 != 0,
            Condition::CoinCameUp {
                player:
                    Some(PlayerRef::Target {
                        slot: TargetSlot(0),
                        index: 0,
                    }),
                outcome: CoinOutcome::Heads,
            } => heads & 2 != 0,
            _ => panic!("unexpected condition {condition:?}"),
        }
    }
    let s=source(&format!("SpellActions(Targeted([TargetPlayer(Opponent)],ActionList([{},DrawACard,If(AllCoinsCameUpHeads,[DrawACard])])))",batch()));
    let card = lower::card(&s).unwrap();
    let Ability::Spell { body, .. } = &card.abilities[0] else {
        panic!()
    };
    let Instruction::If { condition, .. } = &body.instructions[2] else {
        panic!()
    };
    for heads in 0..8 {
        assert_eq!(
            value(condition, heads),
            heads & 3 == 3,
            "heads bits {heads:03b}"
        );
    }
}

#[test]
fn heads_checks_cannot_hide_their_participants_in_primitives_or_terms() {
    for (condition,message) in [
        ("Primitive{name:\"Condition::AllCoinsCameUpHeads\",operands:[]}","requires explicit participants"),
        ("Primitive{name:\"Condition::NumCoinFlipsWonIs\",operands:[Term{family:\"Condition\",form:\"AllCoinsCameUpHeads\",operands:[]}]}","names something in scope"),
    ] {
        let code=format!("Card{{name:\"Coin check\",properties:{{card_types:[Instant]}},abilities:[Spell{{do:[If{{condition:{condition},then:[],otherwise:[]}}]}}]}}");
        let error=human::read(&code).unwrap_err();assert!(error.contains(message),"{error}");
    }
}
