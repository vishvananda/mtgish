use mtgish_ast::{
    corpus,
    ir::*,
    lower,
    mtg_types::OracleCard,
    unified, validate,
    visit::{Visitor, Walk},
};
use std::collections::BTreeMap;
const SOURCE: &str = include_str!("fixtures/reference_other_targets_20261112.ronl");
fn cards() -> BTreeMap<String, Card> {
    SOURCE
        .lines()
        .flat_map(|l| {
            let source: OracleCard = ron::from_str(l).unwrap();
            let result = unified::lower(&source);
            assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
            validate::unified(&result.card).unwrap();
            unified::round_trip(&result.card).unwrap();
            let strict = lower::card(&source).unwrap();
            assert_eq!(
                serde_json::to_value(&strict.abilities).unwrap(),
                serde_json::to_value(&result.card.faces[0].abilities).unwrap()
            );
            result.card.faces
        })
        .map(|c| (c.name.clone(), c))
        .collect()
}
fn trigger(card: &Card) -> (&Announcement, &Program) {
    card.abilities
        .iter()
        .find_map(|a| {
            if let Ability::Triggered {
                announcement, body, ..
            } = a
            {
                Some((announcement, body))
            } else {
                None
            }
        })
        .unwrap()
}
fn refs(filter: &ObjectFilter) -> Vec<String> {
    struct V(Vec<String>);
    impl Visitor for V {
        fn object_ref(&mut self, r: &ObjectRef) -> bool {
            self.0.push(format!("{r:?}"));
            true
        }
    }
    let mut v = V(vec![]);
    filter.walk(&mut v);
    v.0
}
#[test]
fn other_creature_targets_exclude_the_named_creature_source_in_all_four_families() {
    corpus::on_large_stack(|| {
        let cards = cards();
        assert_eq!(cards.len(), 10);
        for name in [
            "Jaheira, Heroic Harper",
            "Kinetic Ooze",
            "Rhino, Terrible Trampler",
            "Amy Rose",
        ] {
            let (a, _) = trigger(&cards[name]);
            let second = &a.targets[1];
            let bound = refs(&second.filter);
            assert_eq!(bound, ["Source"], "{name}: {:?}", second.filter);
            assert!(format!("{:?}", second.filter).contains("Not(SameObject(Source))"));
        }
    });
}
#[test]
fn optional_equipment_populations_are_zero_or_one_without_changing_the_host_reference() {
    corpus::on_large_stack(|| {
        let cards = cards();
        let (a, p) = trigger(&cards["Amy Rose"]);
        assert!(matches!(a.targets[0].count, Count::UpTo(Expr::Constant(1))));
        let Instruction::Effect {
            operation: Operation::Attach { attachments, host },
            ..
        } = &p.instructions[0]
        else {
            panic!("{p:?}")
        };
        assert!(matches!(attachments, ObjectSet::Targets(TargetSlot(0))));
        assert!(matches!(host, ObjectRef::EventSubject | ObjectRef::Source));
    });
}
#[test]
fn explicit_other_target_contrasts_and_independent_repeated_targets_are_preserved() {
    corpus::on_large_stack(|| {
        let cards = cards();
        let band = serde_json::to_string(&cards["Band Together"]).unwrap();
        assert!(band.contains("InSet"));
        assert!(band.contains("Targets"));
        let bathe = serde_json::to_string(&cards["Bathe in Light"]).unwrap();
        assert!(bathe.contains("SameObject"));
        assert!(bathe.contains("Target"));
        let common = &cards["Common Bond"];
        let Ability::Spell { announcement, .. } = &common.abilities[0] else {
            panic!()
        };
        assert_eq!(announcement.targets.len(), 2);
        for t in &announcement.targets {
            assert!(matches!(t.count, Count::Exactly(Expr::Constant(1))));
            assert!(refs(&t.filter).is_empty());
        }
        let (a, _) = trigger(&cards["Guardian Gladewalker"]);
        assert!(refs(&a.targets[0].filter).is_empty());
    });
}
#[path = "support/source_json.rs"]
mod source_json;
#[test]
fn json_and_ron_source_agree_on_the_complete_other_target_packet() {
    corpus::on_large_stack(|| source_json::assert_fixture_matches(SOURCE));
}
