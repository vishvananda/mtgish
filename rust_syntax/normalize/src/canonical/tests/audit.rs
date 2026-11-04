//! Explicit unsealed corpus audit for iterating on the fork without cold git builds.
use crate::{canonical, ir::Card, validate};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, BufWriter, Write};

fn audit() {
    let gold = std::env::var("CANONICAL_AUDIT_GOLD").expect("explicit gold path");
    let slice = std::env::var("CANONICAL_AUDIT_SLICE").expect("explicit unsealed slice");
    let output = std::env::var("CANONICAL_AUDIT_OUTPUT").expect("explicit report path");
    let slice: Value = serde_json::from_reader(std::fs::File::open(slice).unwrap()).unwrap();
    let wanted: BTreeSet<_> = slice["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| name.as_str().unwrap().to_owned())
        .collect();
    let mut failures =
        BufWriter::new(std::fs::File::create(format!("{output}.failures.jsonl")).unwrap());
    let mut faces = 0;
    let mut source_valid = 0;
    let mut canonical_valid = 0;
    let mut idempotent = 0;
    let mut reasons = BTreeMap::<String, usize>::new();
    for line in BufReader::new(std::fs::File::open(gold).unwrap()).lines() {
        let record: Value = serde_json::from_str(&line.unwrap()).unwrap();
        for face in record["faces"].as_array().unwrap().iter().filter(|face| {
            face["name"]
                .as_str()
                .is_some_and(|name| wanted.contains(name))
        }) {
            faces += 1;
            let mut face = face.clone();
            face["schema_version"] = json!(crate::ir::SCHEMA_VERSION);
            let source: Card = serde_json::from_value(face).unwrap();
            source_valid += usize::from(validate::card(&source).is_ok());
            let canonical = canonical::card(&source).unwrap();
            let first = serde_json::to_value(&canonical).unwrap();
            idempotent += usize::from(
                first == serde_json::to_value(canonical::card(&canonical).unwrap()).unwrap(),
            );
            match validate::card(&canonical) {
                Ok(()) => canonical_valid += 1,
                Err(reason) => {
                    *reasons.entry(reason.clone()).or_default() += 1;
                    writeln!(failures, "{}", json!({"name":source.name,"why":reason})).unwrap();
                }
            }
        }
    }
    failures.flush().unwrap();
    std::fs::write(output, serde_json::to_vec_pretty(&json!({
        "schema":crate::ir::SCHEMA_VERSION,"faces":faces,"source_valid":source_valid,
        "canonical_valid":canonical_valid,"idempotent":idempotent,"reasons":reasons,
        "scope":"Explicit unsealed population; schema header updated only for operand compatibility. No parsing or printing."
    })).unwrap()).unwrap();
    println!("source valid {source_valid}/{faces}; canonical valid {canonical_valid}; idempotent {idempotent}");
    assert_eq!(source_valid, faces);
    assert_eq!(canonical_valid, faces);
    assert_eq!(idempotent, faces);
}

#[test]
#[ignore = "explicit unsealed inputs and output required; full corpus is not a unit-test fixture"]
fn unsealed_corpus_stays_valid_and_idempotent() {
    crate::corpus::on_large_stack(audit);
}
