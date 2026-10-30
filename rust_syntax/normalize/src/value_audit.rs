//! Does a complete record still say what its source says? Two values survive any
//! reduction under their own spelling: integers, and fieldless vocabulary forms. A
//! form usually kept but missing from one record was dropped on that path. A form
//! that is never kept was renamed, so this says nothing about it.
use crate::corpus::{self, Result};
use crate::{inspect, mtg_types as source};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Default)]
struct Tally {
    seen: u32,
    missing: Vec<String>,
}

/// `Integer` arguments and fieldless forms of a source tree.
fn source_values(value: &Value, integers: &mut BTreeSet<u64>, forms: &mut BTreeSet<String>) {
    match value {
        Value::Array(items) => items
            .iter()
            .for_each(|item| source_values(item, integers, forms)),
        Value::Object(fields) => {
            match (
                fields.get("variant").and_then(Value::as_str),
                fields.get("args"),
            ) {
                (Some("Integer"), Some(Value::Number(n))) => {
                    integers.extend(n.as_i64().map(i64::unsigned_abs).filter(|n| *n > 1));
                }
                (Some(form), Some(Value::Null)) => {
                    forms.insert(form.into());
                }
                _ => {}
            }
            fields
                .values()
                .for_each(|field| source_values(field, integers, forms));
        }
        _ => {}
    }
}

/// Every word and number in a lowered tree, field names included.
fn output_values(value: &Value, integers: &mut BTreeSet<u64>, words: &mut BTreeSet<String>) {
    match value {
        Value::Array(items) => items
            .iter()
            .for_each(|item| output_values(item, integers, words)),
        Value::Object(fields) => fields.iter().for_each(|(name, field)| {
            words.insert(name.clone());
            output_values(field, integers, words);
        }),
        Value::String(text) => {
            words.insert(text.clone());
        }
        Value::Number(n) => integers.extend(n.as_i64().map(i64::unsigned_abs)),
        _ => {}
    }
}

fn record(
    source: &source::OracleCard,
) -> Result<Option<(String, Vec<u64>, BTreeMap<String, bool>)>> {
    let lowered = crate::unified::lower(source);
    if !lowered.diagnostics.is_empty() || !crate::unified::forms(&lowered.card).named.is_empty() {
        return Ok(None);
    }
    let (mut integers, mut forms) = (BTreeSet::new(), BTreeSet::new());
    source_values(&inspect::value(source)?, &mut integers, &mut forms);
    let (mut kept_integers, mut words) = (BTreeSet::new(), BTreeSet::new());
    output_values(
        &serde_json::to_value(&lowered.card)?,
        &mut kept_integers,
        &mut words,
    );
    let name = lowered
        .card
        .faces
        .first()
        .map_or(String::new(), |f| f.name.clone());
    let lost = integers.difference(&kept_integers).copied().collect();
    let forms = forms.into_iter().map(|form| {
        let kept = words.contains(&form);
        (form, kept)
    });
    Ok(Some((name, lost, forms.collect())))
}

/// Findings not listed in `reviewed`, which maps a form to why its absence is right.
pub fn audit(corpus_path: &Path, reviewed: &Path) -> Result<Vec<String>> {
    let reviewed: BTreeMap<String, String> =
        serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(reviewed)?))?;
    let mut findings = Vec::new();
    let mut forms: BTreeMap<String, Tally> = BTreeMap::new();
    corpus::map_cards(
        corpus_path,
        |_, source| record(&source),
        |_, audited| {
            let Some((name, lost, seen)) = audited else {
                return Ok(());
            };
            findings.extend(
                lost.iter()
                    .map(|n| format!("{name}: integer {n} is not in the AST")),
            );
            for (form, kept) in seen {
                let tally = forms.entry(form).or_default();
                tally.seen += 1;
                if !kept {
                    tally.missing.push(name.clone());
                }
            }
            Ok(())
        },
    )?;
    for (form, tally) in &forms {
        let missing = tally.missing.len() as u32;
        // Mostly missing means renamed, which this cannot judge.
        if missing > 0 && missing * 2 < tally.seen && !reviewed.contains_key(form) {
            findings.push(format!(
                "{form}: missing from {missing} of {} records: {}",
                tally.seen,
                tally.missing.join(", ")
            ));
        }
    }
    Ok(findings)
}
