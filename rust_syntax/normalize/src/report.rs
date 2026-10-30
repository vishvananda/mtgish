//! Corpus accounting is independent of backend support and export success.

use crate::{
    corpus::{self, Identity, Manifest, Result},
    ir, lower, validate,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BackendDiagnostic {
    pub path: String,
    pub feature: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Record {
    pub line: usize,
    pub identity: Identity,
    pub complete: bool,
    pub diagnostics: Vec<lower::Diagnostic>,
    pub backend_capable: bool,
    pub backend_diagnostic: Option<BackendDiagnostic>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    pub schema_version: u32,
    pub corpus_sha256: String,
    pub inputs: BTreeMap<String, String>,
    pub total: usize,
    pub complete: usize,
    #[serde(default)]
    pub backend_checked: bool,
    pub backend_capable: usize,
    pub backend_unsupported: BTreeMap<String, usize>,
    pub forms: crate::inventory::Forms,
    pub records: Vec<Record>,
    pub unsupported: BTreeMap<String, usize>,
    pub operations: BTreeMap<String, usize>,
}

/// Complete cards that changed state between two reports. Cards are matched by
/// name, not source hash, so a repaired card still compares with its old record.
#[derive(Debug, Default, PartialEq, Eq, Serialize)]
pub struct CoverageDelta {
    pub gained: Vec<String>,
    pub lost: Vec<String>,
}

impl CoverageDelta {
    pub fn between(before: &Report, after: &Report) -> Self {
        let complete = |report: &Report| {
            report
                .records
                .iter()
                .map(|record| (record.identity.names.join(" // "), record.complete))
                .collect::<BTreeMap<_, _>>()
        };
        let (before, after) = (complete(before), complete(after));
        let mut delta = Self::default();
        for (name, now) in after {
            match (before.get(&name).copied().unwrap_or(false), now) {
                (false, true) => delta.gained.push(name),
                (true, false) => delta.lost.push(name),
                _ => {}
            }
        }
        delta
    }

    /// Reviewed losses map a card to the reason, such as a dropped form the repair restored.
    pub fn unreviewed_losses(&self, reviewed: &BTreeMap<String, String>) -> Vec<&str> {
        self.lost
            .iter()
            .filter(|name| !reviewed.contains_key(*name))
            .map(String::as_str)
            .collect()
    }
}

impl Report {
    pub fn check(path: &Path, root: &Path, baseline: Option<&Manifest>) -> Result<Self> {
        Self::check_options(path, root, baseline, false)
    }
    pub fn check_with_backend(
        path: &Path,
        root: &Path,
        baseline: Option<&Manifest>,
    ) -> Result<Self> {
        if !cfg!(feature = "cantrip") {
            return Err("backend checking requires --features cantrip".into());
        }
        Self::check_options(path, root, baseline, true)
    }
    fn check_options(
        path: &Path,
        root: &Path,
        baseline: Option<&Manifest>,
        backend: bool,
    ) -> Result<Self> {
        let mut result = Self {
            schema_version: ir::SCHEMA_VERSION,
            corpus_sha256: corpus::digest(&std::fs::read(path)?),
            inputs: crate::pipeline::input_fingerprints(root)?,
            total: 0,
            complete: 0,
            backend_checked: backend,
            backend_capable: 0,
            backend_unsupported: BTreeMap::new(),
            forms: BTreeMap::new(),
            records: Vec::new(),
            unsupported: BTreeMap::new(),
            operations: BTreeMap::new(),
        };
        corpus::for_each_card(path, |line, source| {
            let identity = Identity::of(&source)?;
            #[allow(unused_mut)]
            let mut backend_diagnostic = None;
            #[allow(unused_mut)]
            let mut backend_capable = false;
            let diagnostics = match lower::card(&source) {
                Ok(card) => {
                    validate::card(&card).map_err(|e| {
                        format!("{}:{line}: invalid normalized AST: {e}", path.display())
                    })?;
                    crate::inventory::Inventory(&mut result.forms).card(&card);
                    count_operations(&serde_json::to_value(&card)?, &mut result.operations);
                    #[cfg(feature = "cantrip")]
                    if backend {
                        match crate::cantrip::module(&card) {
                            Ok(_) => {
                                backend_capable = true;
                                result.backend_capable += 1;
                            }
                            Err(d) => {
                                *result
                                    .backend_unsupported
                                    .entry(d.feature.clone())
                                    .or_default() += 1;
                                backend_diagnostic = Some(d);
                            }
                        }
                    }
                    result.complete += 1;
                    Vec::new()
                }
                Err(errors) => errors,
            };
            for d in &diagnostics {
                *result
                    .unsupported
                    .entry(format!("{}::{}", d.source_type, d.form))
                    .or_default() += 1;
            }
            result.total += 1;
            result.records.push(Record {
                line,
                identity,
                complete: diagnostics.is_empty(),
                diagnostics,
                backend_capable,
                backend_diagnostic,
            });
            Ok(())
        })?;
        if result.total == 0 {
            return Err("card corpus is empty".into());
        }
        if let Some(baseline) = baseline {
            let current = Manifest {
                manifest_version: 1,
                corpus_sha256: result.corpus_sha256.clone(),
                inputs: result.inputs.clone(),
                records: result.records.iter().map(|r| r.identity.clone()).collect(),
                forms: BTreeMap::new(),
            };
            current.check_identities(baseline)?;
        }
        Ok(result)
    }
}

fn count_operations(value: &serde_json::Value, counts: &mut BTreeMap<String, usize>) {
    match value {
        serde_json::Value::Object(fields) => {
            if let Some(name) = fields
                .get("operation")
                .and_then(|v| v.get("kind"))
                .and_then(|v| v.as_str())
            {
                *counts.entry(name.into()).or_default() += 1;
            }
            for child in fields.values() {
                count_operations(child, counts);
            }
        }
        serde_json::Value::Array(items) => {
            for child in items {
                count_operations(child, counts);
            }
        }
        _ => {}
    }
}

/// A partial migration can produce a coverage report, but cannot publish a
/// normalized corpus. The destination is unchanged if even its last card fails.
pub fn export_ron(path: &Path, destination: &Path) -> Result<usize> {
    let mut output = corpus::Artifact::create(destination)?;
    let mut count = 0;
    corpus::for_each_card(path, |line, source| {
        let card = lower::card(&source).map_err(|errors| {
            let detail = errors
                .iter()
                .map(|e| format!("{} {}::{}", e.path, e.source_type, e.form))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "{}:{line}: incomplete normalization: {detail}",
                path.display()
            )
        })?;
        validate::card(&card)?;
        writeln!(output, "{}", ron::ser::to_string(&card)?)?;
        count += 1;
        Ok(())
    })?;
    if count == 0 {
        return Err("card corpus is empty".into());
    }
    output.commit()?;
    Ok(count)
}
