//! This must pass even when the enclosing test harness has a larger stack.
use rust_syntax::{corpus, inspect, mtg_types::OracleCard};

#[test]
fn nested_source_inspection_is_safe_on_a_two_megabyte_caller_stack() {
    let raw = include_str!("../normalize/tests/fixtures/early_set_generator_gaps.ronl")
        .lines()
        .next()
        .unwrap();
    let expected = corpus::on_large_stack(|| {
        let source: OracleCard = ron::from_str(raw).unwrap();
        (
            inspect::value(&source).unwrap(),
            corpus::Identity::of(&source).unwrap(),
        )
    });
    let actual = std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(move || {
            let source: OracleCard = ron::from_str(raw).unwrap();
            (
                inspect::value(&source).unwrap(),
                corpus::Identity::of(&source).unwrap(),
            )
        })
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn nested_inspection_reuses_the_existing_protected_worker() {
    #[derive(serde::Serialize)]
    struct ThreadWitness(#[serde(serialize_with = "thread_id")] ());
    fn thread_id<S: serde::Serializer>(_: &(), serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("{:?}", std::thread::current().id()))
    }
    corpus::on_large_stack(|| {
        let before = format!("{:?}", std::thread::current().id());
        assert_eq!(
            inspect::value(&ThreadWitness(())).unwrap(),
            serde_json::json!(before)
        );
    });
}
