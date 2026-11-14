#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, corpus, ir::*, lower, mtg_types::OracleCard, unified};

// Capabilities used by this card on the reviewed upstream-based engine packet.
// Do not inherit the private foundation's unrelated world-rule capability.
const STAGG_TARGET: &[&str] = &[
    "group-departure-delays", "event-delayed-object-lists", "32-bit-effect-quantities",
];

fn source(name: &str) -> OracleCard {
    include_str!("fixtures/group_departure_delays.ronl")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| ron::from_str::<OracleCard>(line).unwrap())
        .find(|card| {
            corpus::Identity::of(card)
                .unwrap()
                .names
                .iter()
                .any(|n| n == name)
        })
        .unwrap()
}

#[test]
fn stangg_creates_one_group_watcher_not_one_trigger_per_token() {
    cantrip::planned::with_additional_features(&["group-departure-delays"], || {
        let source = source("Stangg");
        let generated = cantrip::whole(&source, &unified::lower(&source).card).unwrap();
        let face = &generated.faces[0];
        assert!(face.gaps.is_empty(), "{:?}", face.gaps);
        syn::parse_file(&face.rust).unwrap();
        assert_eq!(
            face.rust.matches("DelayedWhen::AnyOfObjectsLeaves").count(),
            1
        );
        assert_eq!(face.rust.matches("g.delayed_trigger_about(").count(), 2);
        assert!(face.rust.contains("DelayedWhen::WhenEvent"));
        assert!(!face.rust.contains("q.trigger_about"));
        assert!(!face.rust.contains("DelayedWhen::UntilEvent"));
        assert!(!face.rust.contains("for each in"));
    });
}

#[test]
fn both_delayed_abilities_keep_their_own_printed_suffix_trigger() {
    use std::path::PathBuf;
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let output = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("group-departure-printed.jsonl");
    cantrip::planned::with_features(STAGG_TARGET, || {
        cantrip::planned::tracked(
            &fixtures.join("group_departure_delays.ronl"),
            &fixtures.join("group_departure_delays.oracle.json"),
            &output,
        ).unwrap();
    });
    let row: serde_json::Value = std::fs::read_to_string(output).unwrap().lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .find(|row| row["name"] == "Stangg").unwrap();
    assert!(row["faces"][0]["gaps"].as_array().unwrap().is_empty(), "{row}");
    let code = row["faces"][0]["rust"].as_str().unwrap();
    syn::parse_file(code).unwrap();
    assert!(code.contains("\"Exile that token when Stangg leaves the battlefield.\""), "{code}");
    assert!(code.contains("\"Sacrifice Stangg when that token leaves the battlefield.\""), "{code}");
    assert!(!code.contains("delayed_trigger(delayed_6, \"\")"), "{code}");
    assert!(!code.contains("world:"), "the actual native target has no World field: {code}");
    if let Some(path) = std::env::var_os("MTGISH_GROUP_DELAY_MODULE") {
        std::fs::write(path, code).unwrap();
    }
}

#[test]
fn an_explicit_world_capability_is_retained_for_targets_that_have_it() {
    let mut features = STAGG_TARGET.to_vec();
    features.push("world-rule");
    cantrip::planned::with_features(&features, || {
        let source = source("Stangg");
        let generated = cantrip::whole(&source, &unified::lower(&source).card).unwrap();
        assert!(generated.faces[0].gaps.is_empty(), "{:?}", generated.faces[0].gaps);
        assert_eq!(generated.faces[0].rust.matches("world: false").count(), 2);
    });
}

#[test]
fn old_targets_keep_the_missing_group_matcher_requirement() {
    cantrip::planned::without_features(&["group-departure-delays"], || {
        let source = source("Stangg");
        let generated = cantrip::whole(&source, &unified::lower(&source).card).unwrap();
        assert!(generated.faces[0]
            .gaps
            .iter()
            .any(|g| g.feature == "event trigger carrying values"));
        assert!(!generated.faces[0].rust.contains("AnyOfObjectsLeaves"));
    });
}

#[test]
fn watching_tokens_does_not_supply_an_unrelated_body_capture() {
    let mut card = lower::card(&source("Stangg")).unwrap();
    let Ability::Triggered { body, .. } = &mut card.abilities[0] else {
        panic!()
    };
    // A separate creation has a different receipt, even with the same token
    // recipe. It cannot occupy the member span already used for the watched set.
    let mut extra = body.instructions[0].clone();
    let Instruction::Effect { result, .. } = &mut extra else {
        panic!()
    };
    *result = ResultId(90);
    body.instructions.insert(1, extra);
    let effect = body
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            Instruction::Effect {
                operation: Operation::CreateDelayedTrigger { event, body, .. },
                ..
            } if matches!(
                event.as_ref(),
                EventSpec::Object {
                    objects: ObjectFilter::InSet(_),
                    ..
                }
            ) =>
            {
                Some(body)
            }
            _ => None,
        })
        .unwrap();
    let Instruction::Effect {
        operation: Operation::Sacrifice { objects, .. },
        ..
    } = &mut effect.instructions[0]
    else {
        panic!()
    };
    *objects = ObjectSet::Created(ResultId(90));
    cantrip::planned::with_additional_features(&["group-departure-delays"], || {
        assert!(cantrip::gaps(&card)
            .iter()
            .any(|g| g.feature == "group departure delay with additional closed-over values"));
    });
}

#[test]
fn the_primitive_does_not_claim_all_mechtitan_capture_requirements() {
    cantrip::planned::with_additional_features(&["group-departure-delays"], || {
        let source = source("Mechtitan Core");
        let generated = cantrip::whole(&source, &unified::lower(&source).card).unwrap();
        assert!(!generated.faces[0].gaps.is_empty());
    });
}
