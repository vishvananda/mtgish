use mtgish_ast::{canonical, corpus, ir::*, mtg_types::OracleCard, unified, validate};
use serde_json::Value;
fn normalized(abilities: &[Ability]) -> Value {
    validate::abilities(abilities).unwrap();
    let mut value = serde_json::to_value(abilities).unwrap();
    canonical::abilities(&mut value).unwrap();
    value
}
#[test]
fn both_entwine_branches_preserve_complete_programs_and_use_the_named_players_step() {
    corpus::on_large_stack(|| {
        let source: OracleCard =
            ron::from_str(include_str!("fixtures/blinding_beam_reference.ronl").trim()).unwrap();
        let card = unified::lower(&source).card.faces.remove(0);
        validate::card(&card).unwrap();
        let expected: Vec<Ability> = serde_json::from_str(include_str!(
            "fixtures/blinding_beam_reference_expected.json"
        ))
        .unwrap();
        assert_eq!(normalized(&card.abilities), normalized(&expected));
        let Ability::CostBranch { paid, unpaid, .. } = &card.abilities[0] else {
            panic!("{:?}", card.abilities)
        };
        for ability in [paid, unpaid] {
            let Ability::Spell { announcement, .. } = ability.as_ref() else {
                panic!()
            };
            let modes = announcement.modes.as_ref().unwrap();
            let Instruction::Effect {
                operation:
                    Operation::CreateContinuousEffect {
                        affected,
                        changes,
                        duration,
                        evaluation,
                        ..
                    },
                ..
            } = &modes.options[1].body.instructions[0]
            else {
                panic!()
            };
            assert!(matches!(
                affected,
                EffectSubjects::Objects(ObjectSet::Zone {
                    filter: ObjectFilter::CardType(_),
                    ..
                })
            ));
            assert!(matches!(
                duration,
                Duration::DuringNextUntapStep(PlayerRef::Target {
                    slot: TargetSlot(0),
                    index: 0
                })
            ));
            assert!(matches!(
                changes.as_slice(),
                [Change::Rule(RuleChange::UntapStep {
                    players: PlayerSet::One(PlayerRef::Target {
                        slot: TargetSlot(0),
                        index: 0
                    }),
                    behavior: UntapBehavior::Skip
                })]
            ));
            assert!(matches!(evaluation, EffectEvaluation::LiveObjects));
        }
    });
}
