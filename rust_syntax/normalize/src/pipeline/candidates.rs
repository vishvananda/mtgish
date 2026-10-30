//! Bulk mapping preparation from actual Rust enum signatures. Shapes suggest
//! review batches, not semantics; this tool never publishes executable rules.
use super::{registry, report::Report};
use crate::corpus;
use quote::ToTokens;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct Candidate {
    pub source: String,
    pub arity: usize,
    pub occurrences: usize,
    pub record_count: usize,
    pub example_lines: Vec<usize>,
    pub reviewed_action_rule: Option<String>,
    pub reviewed_number_rule: Option<String>,
    pub reviewed_selector_rule: Option<String>,
    pub reviewed_program_rule: Option<String>,
    pub reviewed_operand_rule: Option<String>,
    pub reviewed_mechanic_rule: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Group {
    pub fields: Vec<String>,
    pub occurrences: usize,
    pub candidates: Vec<Candidate>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Candidates {
    pub source_type: String,
    pub source_sha256: String,
    pub corpus_sha256: String,
    pub observed_forms: usize,
    pub occurrences: usize,
    pub groups: Vec<Group>,
}
impl Candidates {
    pub fn from_report(report: &Report, source: &str, source_type: &str) -> corpus::Result<Self> {
        let hash = corpus::digest(source.as_bytes());
        if report.inputs.get("rust_syntax/src/mtg_types.rs") != Some(&hash) {
            return Err("inventory uses another Rust source schema".into());
        }
        for (path, hash) in super::registry_fingerprints() {
            if report.inputs.get(path) != Some(hash) {
                return Err("inventory uses another rewrite registry".into());
            }
        }
        let forms = report
            .source_forms
            .get(source_type)
            .ok_or("source type not observed")?;
        let syntax = syn::parse_file(source)?;
        let definition = syntax
            .items
            .iter()
            .find_map(|item| match item {
                syn::Item::Enum(e) if e.ident == source_type => Some(e),
                _ => None,
            })
            .ok_or("source type is not a Rust enum")?;
        let prefix = format!("{source_type}::");
        let mut membership: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        for (i, record) in report.records.iter().enumerate() {
            for form in record
                .source_forms
                .iter()
                .filter(|f| f.starts_with(&prefix))
            {
                membership.entry(form.as_str()).or_default().push(i);
            }
        }
        let mut groups: BTreeMap<Vec<String>, Vec<Candidate>> = BTreeMap::new();
        for variant in &definition.variants {
            let name = variant.ident.to_string();
            let Some(&occurrences) = forms.get(&name) else {
                continue;
            };
            let fields: Vec<_> = variant
                .fields
                .iter()
                .map(|f| {
                    let ty = f.ty.to_token_stream().to_string().replace(' ', "");
                    f.ident
                        .as_ref()
                        .map_or(ty.clone(), |id| format!("{id}:{ty}"))
                })
                .collect();
            // Whole-tree membership exposes operand occurrences even beneath
            // unsupported parents. No corpus parsing/lowering is necessary.
            let rule = (source_type == "Action")
                .then(|| {
                    registry()
                        .rules
                        .iter()
                        .find(|r| r.source == name)
                        .map(|r| r.id.clone())
                })
                .flatten();
            let key = format!("{source_type}::{name}");
            let records = membership
                .get(key.as_str())
                .map(Vec::as_slice)
                .unwrap_or_default();
            groups.entry(fields.clone()).or_default().push(Candidate {
                source: name,
                arity: fields.len(),
                occurrences,
                record_count: records.len(),
                example_lines: records
                    .iter()
                    .take(3)
                    .map(|&i| report.records[i].line)
                    .collect(),
                reviewed_action_rule: rule,
                reviewed_mechanic_rule: (source_type == "Rule")
                    .then(|| {
                        super::mechanics::registry()
                            .rules
                            .iter()
                            .find(|r| r.source == variant.ident.to_string())
                            .map(|r| r.id.clone())
                    })
                    .flatten(),
                reviewed_operand_rule: super::operands::registry()
                    .rules
                    .iter()
                    .find(|r| {
                        r.domain.name() == source_type && r.source == variant.ident.to_string()
                    })
                    .map(|r| r.id.clone()),
                reviewed_program_rule: super::programs::registry()
                    .rules
                    .iter()
                    .find(|r| {
                        r.domain.name() == source_type && r.source == variant.ident.to_string()
                    })
                    .map(|r| r.id.clone()),
                reviewed_selector_rule: crate::selector_spec::Domain::from_rust_name(source_type)
                    .and_then(|domain| {
                        super::selectors::registry()
                            .rules
                            .iter()
                            .find(|r| r.domain == domain && r.source == variant.ident.to_string())
                            .map(|r| r.id.clone())
                    }),
                reviewed_number_rule: (source_type == "GameNumber")
                    .then(|| {
                        super::numbers::registry()
                            .rules
                            .iter()
                            .find(|r| r.source == variant.ident.to_string())
                            .map(|r| r.id.clone())
                    })
                    .flatten(),
            });
        }
        let found: usize = groups.values().map(Vec::len).sum();
        if found != forms.len() {
            return Err("inventory form missing from Rust source enum".into());
        }
        let mut groups: Vec<_> = groups
            .into_iter()
            .map(|(fields, mut candidates)| {
                candidates.sort_by_key(|c| (std::cmp::Reverse(c.occurrences), c.source.clone()));
                Group {
                    fields,
                    occurrences: candidates.iter().map(|c| c.occurrences).sum(),
                    candidates,
                }
            })
            .collect();
        groups.sort_by_key(|g| (std::cmp::Reverse(g.occurrences), g.fields.clone()));
        Ok(Self {
            source_type: source_type.into(),
            source_sha256: hash,
            corpus_sha256: report.corpus_sha256.clone(),
            observed_forms: found,
            occurrences: forms.values().sum(),
            groups,
        })
    }
}
