#![cfg(feature = "write_out_json")]

use rust_syntax::mtg_types::{
    Action, CardInHand, CardsInHand, Condition, FutureReplacableEventWouldDealDamage, GameNumber,
    PlaneType, Players, Rule, SpellEffect, Target,
};
use ts_rs::TS;

// Compare generated Rust declarations to the checked-in TypeScript contract,
// without running every generated binding test or compiling the giant corpus in tsc.
#[test]
fn repaired_rust_forms_are_exported_in_typescript() {
    let bindings = include_str!("../../typescript_types/oracle_cards.d.ts");
    for (name, declaration) in [
        ("Condition", Condition::decl()),
        ("Action", Action::decl()),
        ("CardInHand", CardInHand::decl()),
        ("CardsInHand", CardsInHand::decl()),
        ("PlaneType", PlaneType::decl()),
        ("Players", Players::decl()),
        ("Rule", Rule::decl()),
        ("SpellEffect", SpellEffect::decl()),
        ("Target", Target::decl()),
        ("GameNumber", GameNumber::decl()),
        (
            "FutureReplacableEventWouldDealDamage",
            FutureReplacableEventWouldDealDamage::decl(),
        ),
    ] {
        let start = bindings.find(&format!("type {name} =")).unwrap();
        let end = bindings[start..].find(';').unwrap() + start + 1;
        let arms = |s: &str| {
            s.split_once('=')
                .unwrap()
                .1
                .trim_end_matches(';')
                .split('|')
                .map(|arm| {
                    arm.chars()
                        .filter(|c| !c.is_whitespace())
                        .collect::<String>()
                })
                .filter(|arm| !arm.is_empty())
                .collect::<std::collections::BTreeSet<_>>()
        };
        let generated = arms(&declaration);
        let checked_in = arms(&bindings[start..end]);
        assert!(
            generated == checked_in,
            "{name}: missing {:?}; unexpected {:?}",
            generated.difference(&checked_in).collect::<Vec<_>>(),
            checked_in.difference(&generated).collect::<Vec<_>>()
        );
    }
}
