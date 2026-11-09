use mtgish_ast::{canonical, corpus, ir::*, mtg_types, unified, validate};
use serde_json::Value;
#[test]
fn both_heads_and_both_tails_read_the_flip_receipt_without_winner_aliases() {
    corpus::on_large_stack(|| {
        let source: mtg_types::OracleCard =
            ron::from_str(include_str!("fixtures/coin_heads_reference.ronl").trim()).unwrap();
        let card = unified::lower(&source).card;
        validate::unified(&card).unwrap();
        let face = &card.faces[0];
        validate::card(face).unwrap();
        let Ability::Triggered { body, .. } = &face.abilities[0] else {
            panic!("trigger")
        };
        let Instruction::Effect { result, .. } = &body.instructions[0] else {
            panic!("flip")
        };
        for (index, expected, keyword) in [(1, 2, Keyword::DoubleStrike), (2, 0, Keyword::Menace)] {
            let Instruction::If {
                condition:
                    Condition::Compare {
                        left: Expr::HeadsFlipped(receipt),
                        comparison: Comparison::Equal,
                        right: Expr::Constant(n),
                    },
                then,
                ..
            } = &body.instructions[index]
            else {
                panic!("{:?}", body.instructions[index])
            };
            assert_eq!(receipt.0, result.0);
            assert_eq!(*n, expected);
            let Instruction::Effect {
                operation: Operation::CreateContinuousEffect { changes, .. },
                ..
            } = &then.instructions[0]
            else {
                panic!("grant")
            };
            assert_eq!(
                serde_json::to_value(changes).unwrap(),
                serde_json::json!([{"kind":"GrantKeyword","args":keyword}])
            );
        }
        let mut once = serde_json::to_value(face.abilities.clone()).unwrap();
        canonical::abilities(&mut once).unwrap();
        let mut twice = once.clone();
        canonical::abilities(&mut twice).unwrap();
        assert_eq!(once, twice);
        let text = mtgish_ast::human::write(&card).unwrap();
        let reread = mtgish_ast::human::read(&text).unwrap();
        let current: Value =
            serde_json::to_value(canonical::mapped_unified(&card).unwrap().card).unwrap();
        assert_eq!(
            current,
            serde_json::to_value(canonical::mapped_unified(&reread).unwrap().card).unwrap()
        );
        #[cfg(feature = "cantrip")]
        {
            let oracle: Vec<Value> = serde_json::from_reader(
                std::fs::File::open(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/oracle.json"),
                )
                .unwrap(),
            )
            .unwrap();
            let text = oracle
                .iter()
                .find(|c| c["name"] == "Two-Headed Giant")
                .unwrap()["cards"][0]["text"]
                .as_str()
                .unwrap();
            let printed = std::collections::BTreeMap::from([(face.name.clone(), text.to_string())]);
            let whole = mtgish_ast::cantrip::whole_printed(&source, &card, Some(&printed)).unwrap();
            assert!(whole.faces.iter().all(|f| f.gaps.is_empty()));
            let module = &whole.faces[0].rust;
            assert!(module.contains("flip_coin_for_heads"));
            assert!(!module.contains("g.flip_coin("));
        }
    });
}
