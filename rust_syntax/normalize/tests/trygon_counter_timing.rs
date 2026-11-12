#[path = "support/fresh_producer.rs"]
mod fresh_producer;
use mtgish_ast::{canonical, corpus, ir::*, mtg_types::OracleCard, unified, validate};
use serde_json::Value;
fn card() -> Card {
    let source: OracleCard =
        ron::from_str(include_str!("fixtures/trygon_counter_timing.ronl").trim()).unwrap();
    unified::lower(&source).card.faces.remove(0)
}
fn normalized(abilities: &[Ability]) -> Value {
    validate::abilities(abilities).unwrap();
    let mut v = serde_json::to_value(abilities).unwrap();
    canonical::abilities(&mut v).unwrap();
    v
}
#[test]
fn trygon_keeps_one_batch_and_the_complete_independent_target_followup() {
    corpus::on_large_stack(|| {
        let card = card();
        validate::card(&card).unwrap();
        let expected: Vec<Ability> =
            serde_json::from_str(include_str!("fixtures/trygon_counter_timing_expected.json"))
                .unwrap();
        fresh_producer::assert_named("trygon_counter_timing_expected.json", &card.name,
            &normalized(&expected), &normalized(&card.abilities));
        let Ability::Triggered {
            announcement, body, ..
        } = &card.abilities[0]
        else {
            panic!()
        };
        assert_eq!(announcement.targets.len(), 1);
        assert!(matches!(
            announcement.targets[0].count,
            Count::UpTo(Expr::Constant(1))
        ));
        let [Instruction::Effect {
            operation: Operation::PutCounters { placements },
            ..
        }, Instruction::Effect {
            operation:
                Operation::CreateContinuousEffect {
                    affected, changes, ..
                },
            ..
        }] = body.instructions.as_slice()
        else {
            panic!("{body:?}")
        };
        assert_eq!(placements.len(), 2);
        assert!(placements
            .iter()
            .all(|p| matches!(p.amount, Expr::Constant(1))));
        assert!(matches!(
            affected,
            EffectSubjects::Objects(ObjectSet::Targets(TargetSlot(0)))
        ));
        assert!(matches!(
            changes.as_slice(),
            [Change::Rule(RuleChange::Prohibit(ObjectAction::BeBlocked(
                ObjectFilter::Any
            )))]
        ));
        // Unblockability reads the original chosen target, independently of
        // whether counters were actually put on it (e.g. under Solemnity).
        assert!(!format!("{affected:?}").contains("Selected"));
    });
}
