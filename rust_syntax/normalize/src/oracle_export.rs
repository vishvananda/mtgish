//! Compare freshly lowered MTGish faces with independently exported Oracle IR.
//! No reading is selected from ambiguity and no disagreement rewrites either side.
use crate::{canonical, corpus, ir, mtg_types::OracleCard, unified, validate};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path};

type Key = (Vec<String>, String);

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub identity: Value,
    pub record_names: Vec<String>,
    pub name: String,
    pub grammar_status: String,
    pub expected: Option<Value>,
}

struct LoweredFace {
    face: ir::Card,
    diagnostics: Vec<String>,
    source_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub format_version: u32,
    pub schema_version: u32,
    pub source_records: usize,
    pub input_faces: usize,
    pub counts: BTreeMap<String, usize>,
    /// Gate covers unique exported faces; unsupported inputs never count as matches.
    pub passed: bool,
    pub complete_agreement: bool,
    pub findings: Vec<Value>,
}

fn key(names: impl Iterator<Item = String>, face: &str) -> Key {
    let mut names: Vec<_> = names.collect();
    names.sort();
    (names, face.to_owned())
}

fn index_record(source: &OracleCard) -> corpus::Result<Vec<(Key, LoweredFace)>> {
    let identity = corpus::Identity::of(source)?;
    let lowered = unified::lower(source);
    let names: Vec<_> = lowered.card.faces.iter().map(|f| f.name.clone()).collect();
    let diagnostics: Vec<_> = lowered
        .diagnostics
        .iter()
        .map(|d| format!("{d:?}"))
        .collect();
    Ok(lowered
        .card
        .faces
        .into_iter()
        .map(|face| {
            (
                key(names.iter().cloned(), &face.name),
                LoweredFace {
                    face,
                    diagnostics: diagnostics.clone(),
                    source_sha256: identity.source_sha256.clone(),
                },
            )
        })
        .collect())
}

fn typed(value: &Value) -> Result<ir::Card, String> {
    let card: ir::Card = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
    if serde_json::to_value(&card).map_err(|e| e.to_string())? != *value {
        return Err("unknown or missing fields in exported card".into());
    }
    if card.schema_version != ir::SCHEMA_VERSION {
        return Err("exported schema differs from compiled model".into());
    }
    validate::card(&card)?;
    Ok(card)
}

fn first_difference(left: &Value, right: &Value, path: &str) -> Option<Value> {
    if left == right {
        return None;
    }
    let child = |name: &str| format!("{path}/{}", name.replace('~', "~0").replace('/', "~1"));
    match (left, right) {
        (Value::Object(a), Value::Object(b)) => {
            let mut names: Vec<_> = a.keys().chain(b.keys()).collect();
            names.sort();
            names.dedup();
            for name in names {
                match (a.get(name), b.get(name)) {
                    (Some(a), Some(b)) => {
                        if let Some(diff) = first_difference(a, b, &child(name)) {
                            return Some(diff);
                        }
                    }
                    (a, b) => {
                        return Some(
                            json!({"path":child(name),"mtgish":a,"exported":b,"missing_field":true}),
                        )
                    }
                }
            }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                if let Some(diff) = first_difference(a, b, &child(&i.to_string())) {
                    return Some(diff);
                }
            }
        }
        _ => {}
    }
    Some(json!({"path":path,"mtgish":left,"exported":right}))
}

fn compare_face(request: &Request, index: &BTreeMap<Key, Vec<LoweredFace>>) -> Value {
    let mut row = json!({"identity":request.identity,"name":request.name});
    let fail = |mut row: Value, status: &str, message: String| {
        row["status"] = json!(status);
        row["message"] = json!(message);
        row
    };
    match request.grammar_status.as_str() {
        "refused" => {
            return fail(
                row,
                "grammar_refused",
                "No exported AST; not an agreement".into(),
            )
        }
        "limit" => {
            return fail(
                row,
                "grammar_limit",
                "Native export exhausted a budget".into(),
            )
        }
        "ambiguous" => {
            return fail(
                row,
                "grammar_ambiguous",
                "No unique exported value; no reading selected".into(),
            )
        }
        "unique" => {}
        _ => return fail(row, "invalid_export", "Unknown grammar outcome".into()),
    }
    let Some(expected) = &request.expected else {
        return fail(row, "invalid_export", "Unique export has no value".into());
    };
    let expected = match typed(expected) {
        Ok(card) if card.name == request.name => card,
        Ok(_) => {
            return fail(
                row,
                "invalid_export",
                "Face name differs from exported value".into(),
            )
        }
        Err(why) => return fail(row, "invalid_export", why),
    };
    let lookup = key(request.record_names.iter().cloned(), &request.name);
    let Some(candidates) = index.get(&lookup) else {
        return fail(
            row,
            "mtgish_missing",
            "Record/face absent: parser refusal, exclusion, or changed source identity".into(),
        );
    };
    if candidates.len() != 1 {
        return fail(
            row,
            "identity_collision",
            "Multiple Source occurrences have this record/face identity".into(),
        );
    }
    let local = &candidates[0];
    row["source_sha256"] = json!(local.source_sha256);
    if !local.diagnostics.is_empty() {
        row["diagnostics"] = json!(local.diagnostics);
        return fail(
            row,
            "mtgish_incomplete",
            "Strict lowering rejected part of this record".into(),
        );
    }
    if let Err(why) = validate::card(&local.face) {
        return fail(row, "mtgish_invalid", why);
    }
    let result = (|| -> Result<_, String> {
        let raw_local = serde_json::to_value(&local.face).map_err(|e| e.to_string())?;
        let raw_expected = serde_json::to_value(&expected).map_err(|e| e.to_string())?;
        let a = canonical::card(&local.face)?;
        let b = canonical::card(&expected)?;
        validate::card(&a)?;
        validate::card(&b)?;
        let a = serde_json::to_value(a).map_err(|e| e.to_string())?;
        let b = serde_json::to_value(b).map_err(|e| e.to_string())?;
        Ok((raw_local, raw_expected, a, b))
    })();
    let (raw_local, raw_expected, local, expected) = match result {
        Ok(values) => values,
        Err(why) => return fail(row, "canonicalization_error", why),
    };
    row["canonical_equal"] = json!(local == expected);
    row["exact_equal"] = json!(raw_local == raw_expected);
    let status = if raw_local == raw_expected {
        "equal"
    } else if local == expected {
        "canonical_only"
    } else {
        "mismatch"
    };
    row["status"] = json!(status);
    if status != "equal" {
        row["difference"] = first_difference(&raw_local, &raw_expected, "").unwrap_or(Value::Null);
        row["mtgish"] = raw_local;
        row["exported"] = raw_expected;
    }
    row
}

fn finish(index: &BTreeMap<Key, Vec<LoweredFace>>, records: usize, requests: &[Request]) -> Report {
    let findings: Vec<_> = requests.iter().map(|r| compare_face(r, index)).collect();
    let mut counts = BTreeMap::new();
    for row in &findings {
        *counts
            .entry(
                row["status"]
                    .as_str()
                    .unwrap_or("invalid_export")
                    .to_owned(),
            )
            .or_insert(0) += 1;
    }
    let equal = counts.get("equal").copied().unwrap_or(0);
    let refused = counts.get("grammar_refused").copied().unwrap_or(0);
    Report {
        format_version: 1,
        schema_version: ir::SCHEMA_VERSION,
        source_records: records,
        input_faces: requests.len(),
        passed: equal > 0 && equal + refused == requests.len(),
        complete_agreement: equal > 0 && equal == requests.len(),
        counts,
        findings,
    }
}

pub fn compare(sources: &[OracleCard], requests: &[Request]) -> corpus::Result<Report> {
    let mut index: BTreeMap<Key, Vec<LoweredFace>> = BTreeMap::new();
    for source in sources {
        for (key, face) in index_record(source)? {
            index.entry(key).or_default().push(face);
        }
    }
    Ok(finish(&index, sources.len(), requests))
}

/// The wrapper verifies bundle/source hashes and supplies a fresh parser output.
pub fn run(source: &Path, requests: &Path, destination: &Path) -> corpus::Result<()> {
    let requests: Vec<Request> =
        serde_json::from_value(crate::json_input::value(&fs::read_to_string(requests)?)?)?;
    let mut index: BTreeMap<Key, Vec<LoweredFace>> = BTreeMap::new();
    let mut records = 0;
    corpus::map_cards(
        source,
        |_, source| index_record(&source),
        |_, faces| {
            records += 1;
            for (key, face) in faces {
                index.entry(key).or_default().push(face);
            }
            Ok(())
        },
    )?;
    let report = finish(&index, records, &requests);
    let mut artifact = corpus::Artifact::create(destination)?;
    serde_json::to_writer_pretty(&mut artifact, &report)?;
    artifact.commit()?;
    if !report.passed {
        return Err("Oracle export comparison failed; inspect the comparison report".into());
    }
    Ok(())
}
