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

/// The integer a form holds directly.
fn integer(value: &Value) -> Option<i64> {
    match value.get("variant")?.as_str()? {
        "Integer" => value.get("args")?.as_i64(),
        _ => None,
    }
}

/// `Integer` arguments and fieldless forms of a source tree. Lowering folds a
/// sum of two integers, so the sum stands for its terms.
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
                (Some("Plus"), Some(Value::Array(terms))) => {
                    if let [Some(a), Some(b)] = terms.iter().map(integer).collect::<Vec<_>>()[..] {
                        integers.insert((a + b).unsigned_abs());
                        return;
                    }
                }
                // Two piles are what `SeparatePiles` means.
                (Some("SeparatePermanentsIntoNumberPiles"), Some(Value::Array(terms))) => {
                    if let [objects, count] = &terms[..] {
                        if integer(count) == Some(2) {
                            source_values(objects, integers, forms);
                            return;
                        }
                    }
                }
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
            words.extend(text.rsplit("::").next().map(str::to_string));
            words.insert(text.clone());
        }
        Value::Number(n) => integers.extend(n.as_i64().map(i64::unsigned_abs)),
        _ => {}
    }
}

/// Slots read under `value`. A target's own `slot` field announces it.
fn slot_reads(value: &Value, reads: &mut BTreeSet<u64>) {
    match value {
        Value::Array(items) => items.iter().for_each(|item| slot_reads(item, reads)),
        Value::Object(fields) => fields.iter().for_each(|(name, field)| match name.as_str() {
            "slot" | "Targets" if field.is_u64() => reads.extend(field.as_u64()),
            "args" if fields.get("kind").is_some_and(|kind| kind == "Targets") => {
                reads.extend(field.as_u64())
            }
            "targets" => {
                for spec in field.as_array().into_iter().flatten() {
                    for (name, part) in spec.as_object().into_iter().flatten() {
                        if name != "slot" {
                            slot_reads(part, reads);
                        }
                    }
                }
            }
            _ => slot_reads(field, reads),
        }),
        _ => {}
    }
}

/// Announced target slots nothing in their ability reads. A distributed slot is
/// read through its shares.
fn unread_targets(value: &Value, unread: &mut Vec<u64>) {
    match value {
        Value::Array(items) => items.iter().for_each(|item| unread_targets(item, unread)),
        Value::Object(fields) => {
            let specs = fields.get("announcement").and_then(|a| a.get("targets"));
            if let Some(specs) = specs.and_then(Value::as_array) {
                let mut reads = BTreeSet::new();
                slot_reads(value, &mut reads);
                unread.extend(
                    specs
                        .iter()
                        .filter(|spec| spec["distribute"].is_null())
                        .filter_map(|spec| spec["slot"].as_u64())
                        .filter(|slot| !reads.contains(slot)),
                );
            }
            fields
                .values()
                .for_each(|field| unread_targets(field, unread));
        }
        _ => {}
    }
}

type Audited = (String, Vec<u64>, BTreeMap<String, bool>, Vec<u64>);

fn record(source: &source::OracleCard) -> Result<Option<Audited>> {
    let lowered = crate::unified::lower(source);
    if !lowered.diagnostics.is_empty() || !crate::unified::forms(&lowered.card).named.is_empty() {
        return Ok(None);
    }
    let (mut integers, mut forms) = (BTreeSet::new(), BTreeSet::new());
    source_values(&inspect::value(source)?, &mut integers, &mut forms);
    let (mut kept_integers, mut words) = (BTreeSet::new(), BTreeSet::new());
    let tree = serde_json::to_value(&lowered.card)?;
    output_values(&tree, &mut kept_integers, &mut words);
    let mut unread = Vec::new();
    unread_targets(&tree, &mut unread);
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
    Ok(Some((name, lost, forms.collect(), unread)))
}

/// Findings not listed in `reviewed`, which maps a form, or `target:` and a card name,
/// to why its absence is right.
pub fn audit(corpus_path: &Path, reviewed: &Path) -> Result<Vec<String>> {
    let reviewed: BTreeMap<String, String> =
        serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(reviewed)?))?;
    let mut findings = Vec::new();
    let mut forms: BTreeMap<String, Tally> = BTreeMap::new();
    corpus::map_cards(
        corpus_path,
        |_, source| record(&source),
        |_, audited| {
            let Some((name, lost, seen, unread)) = audited else {
                return Ok(());
            };
            findings.extend(
                lost.iter()
                    .map(|n| format!("{name}: integer {n} is not in the AST")),
            );
            if !reviewed.contains_key(&format!("target:{name}")) {
                findings.extend(
                    unread
                        .iter()
                        .map(|n| format!("{name}: target slot {n} is never read")),
                );
            }
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
