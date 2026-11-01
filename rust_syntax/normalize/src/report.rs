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
    /// Lowered, validated and round-tripped. Unreduced forms are allowed here.
    #[serde(default)]
    pub unified: bool,
    /// Every unreduced form in this record, including nested ones.
    #[serde(default)]
    pub named: BTreeMap<String, usize>,
    /// Forms that actually failed, with counts. This is the work list: `named`
    /// also holds children that only failed to reduce with their parent.
    #[serde(default)]
    pub blockers: BTreeMap<String, usize>,
    /// Reviewed primitives this record depends on.
    #[serde(default)]
    pub primitives: BTreeMap<String, usize>,
    pub diagnostics: Vec<lower::Diagnostic>,
    pub backend_capable: bool,
    pub backend_diagnostic: Option<BackendDiagnostic>,
}

/// What one worker found for one record. Merged in line order.
struct Checked {
    record: Record,
    forms: crate::inventory::Forms,
    operations: BTreeMap<String, usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    pub schema_version: u32,
    pub corpus_sha256: String,
    pub inputs: BTreeMap<String, String>,
    pub total: usize,
    pub complete: usize,
    /// Complete records that depend on no primitive.
    #[serde(default)]
    pub complete_without_primitives: usize,
    #[serde(default)]
    pub unified: usize,
    /// Unreduced forms over all records, in the order of work for reduction.
    #[serde(default)]
    pub named: BTreeMap<String, usize>,
    /// Records each form appears in. Reduction work ranks by this, not occurrences.
    #[serde(default)]
    pub named_records: BTreeMap<String, usize>,
    /// Records blocked by each form. Rank reduction work by this.
    #[serde(default)]
    pub blocker_records: BTreeMap<String, usize>,
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
            complete_without_primitives: 0,
            unified: 0,
            named: BTreeMap::new(),
            named_records: BTreeMap::new(),
            blocker_records: BTreeMap::new(),
            backend_checked: backend,
            backend_capable: 0,
            backend_unsupported: BTreeMap::new(),
            forms: BTreeMap::new(),
            records: Vec::new(),
            unsupported: BTreeMap::new(),
            operations: BTreeMap::new(),
        };
        let check = |line: usize, source: crate::mtg_types::OracleCard| -> Result<Checked> {
            let identity = Identity::of(&source)?;
            #[allow(unused_mut)]
            let mut backend_diagnostic = None;
            #[allow(unused_mut)]
            let mut backend_capable = false;
            let (mut forms, mut operations) = (BTreeMap::new(), BTreeMap::new());
            let lowered = crate::unified::lower(&source);
            let invalid =
                |e: String| format!("{}:{line}: invalid unified AST: {e}", path.display());
            validate::unified(&lowered.card).map_err(invalid)?;
            crate::unified::round_trip(&lowered.card).map_err(invalid)?;
            let crate::unified::Forms { named, primitives } = crate::unified::forms(&lowered.card);
            let mut blockers = BTreeMap::new();
            for d in &lowered.blockers {
                *blockers
                    .entry(format!("{}::{}", d.source_type, d.form))
                    .or_default() += 1;
            }
            let complete = lowered.diagnostics.is_empty() && named.is_empty();
            if complete {
                for card in &lowered.card.faces {
                    validate::card(card).map_err(|e| {
                        format!("{}:{line}: invalid normalized AST: {e}", path.display())
                    })?;
                    crate::inventory::Inventory(&mut forms).card(card);
                    count_operations(&serde_json::to_value(card)?, &mut operations);
                }
                #[cfg(feature = "cantrip")]
                if backend && crate::cantrip::is_card(&lowered.card) {
                    match crate::cantrip::whole_module(&source, &lowered.card) {
                        Ok(_) => backend_capable = true,
                        Err(d) => backend_diagnostic = Some(d),
                    }
                }
            }
            Ok(Checked {
                record: Record {
                    line,
                    identity,
                    complete,
                    unified: true,
                    named,
                    blockers,
                    primitives,
                    diagnostics: lowered.diagnostics,
                    backend_capable,
                    backend_diagnostic,
                },
                forms,
                operations,
            })
        };
        corpus::map_cards(path, check, |_, checked| {
            let Checked {
                record,
                forms,
                operations,
            } = checked;
            result.total += 1;
            result.unified += 1;
            result.complete += usize::from(record.complete);
            result.complete_without_primitives +=
                usize::from(record.complete && record.primitives.is_empty());
            result.backend_capable += usize::from(record.backend_capable);
            if let Some(d) = &record.backend_diagnostic {
                *result
                    .backend_unsupported
                    .entry(d.feature.clone())
                    .or_default() += 1;
            }
            for (family, counts) in forms {
                let merged = result.forms.entry(family).or_default();
                for (form, count) in counts {
                    *merged.entry(form).or_default() += count;
                }
            }
            for (operation, count) in operations {
                *result.operations.entry(operation).or_default() += count;
            }
            for (form, count) in &record.named {
                *result.named.entry(form.clone()).or_default() += count;
                *result.named_records.entry(form.clone()).or_default() += 1;
            }
            for form in record.blockers.keys() {
                *result.blocker_records.entry(form.clone()).or_default() += 1;
            }
            for d in &record.diagnostics {
                *result
                    .unsupported
                    .entry(format!("{}::{}", d.source_type, d.form))
                    .or_default() += 1;
            }
            result.records.push(record);
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
/// Write every record in the one schema, reduced or not. Unlike `export_ron`,
/// this succeeds on an unreduced corpus. A `.jsonl` destination writes JSON lines.
pub fn export_unified(path: &Path, destination: &Path) -> Result<usize> {
    let json = destination.extension().is_some_and(|e| e == "jsonl");
    let mut output = corpus::Artifact::create(destination)?;
    let mut count = 0;
    corpus::map_cards(
        path,
        |line, source| -> Result<String> {
            let card = crate::unified::lower(&source).card;
            validate::unified(&card)
                .map_err(|e| format!("{}:{line}: invalid unified AST: {e}", path.display()))?;
            Ok(if json {
                serde_json::to_string(&card)?
            } else {
                ron::ser::to_string(&card)?
            })
        },
        |_, text| {
            writeln!(output, "{text}")?;
            count += 1;
            Ok(())
        },
    )?;
    if count == 0 {
        return Err("card corpus is empty".into());
    }
    output.commit()?;
    Ok(count)
}

pub fn export_ron(path: &Path, destination: &Path) -> Result<usize> {
    let mut output = corpus::Artifact::create(destination)?;
    let mut count = 0;
    corpus::map_cards(
        path,
        |line, source| -> Result<String> {
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
            Ok(ron::ser::to_string(&card)?)
        },
        |_, text| {
            writeln!(output, "{text}")?;
            count += 1;
            Ok(())
        },
    )?;
    if count == 0 {
        return Err("card corpus is empty".into());
    }
    output.commit()?;
    Ok(count)
}
