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
    serde_json::from_str(include_str!("fixtures/canonical_join_results.json")).unwrap()
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
    assert_eq!(normalized(v), normalized(cases()[0]["a"].clone()));
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
