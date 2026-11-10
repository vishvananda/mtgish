#[path = "support/legacy_entries.rs"]
mod legacy_entries;
use mtgish_ast::{canonical, ir::Ability, validate};
use serde_json::{json, Value};
fn valid(v: &Value) -> bool {
    let a: Vec<Ability> = serde_json::from_value(v.clone()).unwrap();
    validate::abilities(&a).is_ok()
}
fn normalized(mut v: Value) -> Value {
    assert!(valid(&v));
    canonical::abilities(&mut v).unwrap();
    assert!(valid(&v));
    let mut twice = v.clone();
    canonical::abilities(&mut twice).unwrap();
    assert_eq!(v, twice);
    v
}
fn cases() -> Vec<Value> {
    let mut cases: Value =
        serde_json::from_str(include_str!("fixtures/canonical_join_results.json")).unwrap();
    legacy_entries::json(&mut cases);
    serde_json::from_value(cases).unwrap()
}
#[test]
fn explicit_joins_retain_renamed_branch_receipts_and_restore_native_matches() {
    for c in cases() {
        assert_eq!(
            normalized(c["a"].clone()),
            normalized(c["b"].clone()),
            "{}",
            c["name"]
        );
    }
}
#[test]
fn joins_reject_dangling_and_future_receipts() {
    for ids in [[11, 999], [33, 22]] {
        let mut v = cases()[0]["a"].clone();
        v[0]["args"]["body"]["instructions"][1]["args"]["operation"]["args"]["results"] =
            json!(ids);
        assert!(!valid(&v));
        canonical::abilities(&mut v).unwrap();
        assert!(!valid(&v));
    }
}
#[test]
fn joins_support_reused_branch_labels_without_leaking_normal_reads() {
    let mut v = cases()[0]["a"].clone();
    v[0]["args"]["body"]["instructions"][0]["args"]["options"][1]["instructions"][0]["args"]
        ["result"] = json!(11);
    v[0]["args"]["body"]["instructions"][1]["args"]["operation"]["args"]["results"] =
        json!([11, 11]);
    let v = normalized(v);
    let steps = &v[0]["args"]["body"]["instructions"];
    let a = &steps[0]["args"]["options"][0]["instructions"][0]["args"]["result"];
    let b = &steps[0]["args"]["options"][1]["instructions"][0]["args"]["result"];
    assert_eq!(
        steps[1]["args"]["operation"]["args"]["results"],
        json!([a, b, a, b])
    );
    let mut leaking = cases()[0]["a"].clone();
    leaking[0]["args"]["body"]["instructions"][2]["args"]["operation"]["args"]["objects"]["args"]
        ["result"] = json!(11);
    assert!(!valid(&leaking));
}

#[test]
fn joins_cannot_mix_receipts_from_independent_choices() {
    let mut v = cases()[0]["a"].clone();
    let steps = v[0]["args"]["body"]["instructions"].as_array_mut().unwrap();
    let mut second = steps[0].clone();
    for (i, id) in [44, 55].into_iter().enumerate() {
        second["args"]["options"][i]["instructions"][0]["args"]["result"] = json!(id);
    }
    steps.insert(1, second);
    steps[2]["args"]["operation"]["args"]["results"] = json!([11, 55]);
    assert!(!valid(&v));
    canonical::abilities(&mut v).unwrap();
    assert!(!valid(&v));
}

#[test]
fn later_ordinary_definition_shadows_an_older_branch_label() {
    let mut v = cases()[0]["a"].clone();
    let steps = v[0]["args"]["body"]["instructions"].as_array_mut().unwrap();
    let later = steps[0]["args"]["options"][0]["instructions"][0].clone();
    steps.insert(1, later);
    let mut renamed = v.clone();
    renamed[0]["args"]["body"]["instructions"][1]["args"]["result"] = json!(44);
    renamed[0]["args"]["body"]["instructions"][2]["args"]["operation"]["args"]["results"] =
        json!([44, 22]);
    let v = normalized(v);
    assert_eq!(v, normalized(renamed));
    let steps = &v[0]["args"]["body"]["instructions"];
    assert_eq!(
        steps[2]["args"]["operation"]["args"]["results"],
        json!([
            steps[1]["args"]["result"],
            steps[0]["args"]["options"][1]["instructions"][0]["args"]["result"]
        ])
    );
}

#[test]
fn excessive_join_work_is_refused_without_changing_the_input() {
    let mut v = cases()[0]["a"].clone();
    let steps = v[0]["args"]["body"]["instructions"].as_array_mut().unwrap();
    let option = steps[0]["args"]["options"][0].clone();
    steps[0]["args"]["options"] = Value::Array(vec![option; 300]);
    steps[1]["args"]["operation"]["args"]["results"] = json!(vec![11; 256]);
    assert!(valid(&v));
    let before = v.clone();
    let error = canonical::abilities(&mut v).unwrap_err();
    assert_eq!(error, "branch receipt join work limit exceeded");
    assert_eq!(v, before);
}

#[test]
fn a_granted_abilitys_branch_wins_over_an_enclosing_ordinary_label() {
    use mtgish_ast::ir::*;
    let base = cases()[0]["a"][0]["args"]["body"].clone();
    let outer: Instruction = serde_json::from_value(
        base["instructions"][0]["args"]["options"][0]["instructions"][0].clone(),
    )
    .unwrap();
    let inner = Ability::Activated {
        origin: ActivationOrigin::Ordinary,
        cost: Cost { parts: vec![] },
        limits: ActivationLimits::unrestricted(),
        announcement: Announcement {
            targets: vec![],
            modes: None,
        },
        body: serde_json::from_value(base).unwrap(),
    };
    let ability = Ability::Spell {
        announcement: Announcement {
            targets: vec![],
            modes: None,
        },
        body: Program {
            instructions: vec![
                outer,
                Instruction::Effect {
                    result: ResultId(50),
                    operation: Operation::CreateContinuousEffect {
                        source: ObjectRef::Source,
                        affected: EffectSubjects::Objects(ObjectSet::One(ObjectRef::Source)),
                        changes: vec![Change::GrantAbility(Box::new(inner))],
                        duration: Duration::EndOfTurn,
                        evaluation: EffectEvaluation::Snapshot,
                    },
                },
            ],
        },
    };
    let v = normalized(serde_json::to_value(vec![ability]).unwrap());
    let steps = &v[0]["args"]["body"]["instructions"];
    let body = &steps[1]["args"]["operation"]["args"]["changes"][0]["args"]["args"]["body"]
        ["instructions"];
    let a = &body[0]["args"]["options"][0]["instructions"][0]["args"]["result"];
    let b = &body[0]["args"]["options"][1]["instructions"][0]["args"]["result"];
    assert_eq!(
        body[1]["args"]["operation"]["args"]["results"],
        json!([a, b])
    );
    assert_ne!(a, &steps[0]["args"]["result"]);
}
