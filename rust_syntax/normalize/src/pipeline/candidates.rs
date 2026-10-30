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

/// Oracle text by face name, lowercased, without reminder text.
pub fn oracle_text(path: &std::path::Path) -> corpus::Result<BTreeMap<String, String>> {
    let cards: serde_json::Value =
        serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(path)?))?;
    let mut faces = BTreeMap::new();
    for card in cards.as_array().ok_or("oracle data is not a list")? {
        for face in card["cards"].as_array().into_iter().flatten() {
            let (Some(name), text) = (face["name"].as_str(), face["text"].as_str()) else {
                continue;
            };
            let mut plain = String::new();
            let mut depth = 0;
            for c in text.unwrap_or_default().to_lowercase().chars() {
                match c {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ if depth == 0 => plain.push(c),
                    _ => {}
                }
            }
            faces.insert(name.to_string(), plain);
        }
    }
    Ok(faces)
}

/// Space-separated words must all appear; `|` separates alternatives for one word.
pub fn text_matches(expected: &str, oracle: &str) -> bool {
    expected
        .split_whitespace()
        .all(|word| word.split('|').any(|choice| oracle.contains(choice)))
}

fn record_text(names: &[String], oracle: &BTreeMap<String, String>) -> String {
    names
        .iter()
        .filter_map(|name| oracle.get(name))
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
}

/// A blocker form with its signature and a first guess at its Oracle words.
#[derive(Debug, Serialize, Deserialize)]
pub struct Draft {
    pub source: String,
    pub signature: String,
    pub text: String,
    pub records: usize,
    /// Share of those records whose Oracle text holds `text`. Evidence for review
    /// only: a high score does not prove the form is atomic.
    pub oracle_hits: usize,
    pub example_lines: Vec<usize>,
}

/// Words of a variant name that say what it does, dropping schema vocabulary.
fn draft_text(variant: &str) -> String {
    const NOISE: &[&str] = &[
        "a", "an", "the", "of", "to", "from", "on", "in", "into", "and", "or", "with", "its",
        "their", "your", "this", "that", "each", "all", "any", "permanent", "permanents", "card",
        "cards", "player", "players", "action", "actions", "until", "for", "at", "by", "when",
        "whenever", "creature", "creatures", "is", "are", "number", "nth", "number",
    ];
    let mut words: Vec<String> = vec![];
    for c in variant.chars() {
        if c.is_uppercase() || words.is_empty() {
            words.push(String::new());
        }
        if c != '_' {
            words.last_mut().unwrap().push(c.to_ascii_lowercase());
        }
    }
    words.retain(|w| !w.is_empty() && !NOISE.contains(&w.as_str()));
    // Oracle text conjugates by subject, so match "attack" in "attacks".
    for word in &mut words {
        if word.len() > 3 && word.ends_with('s') && !word.ends_with("ss") {
            word.pop();
        }
    }
    words.join(" ")
}

pub fn draft_rows(
    report: &crate::report::Report,
    schema: &str,
    source_type: &str,
    oracle: &BTreeMap<String, String>,
) -> corpus::Result<Vec<Draft>> {
    let syntax = syn::parse_file(schema)?;
    let definition = syntax
        .items
        .iter()
        .find_map(|item| match item {
            syn::Item::Enum(e) if e.ident == source_type => Some(e),
            _ => None,
        })
        .ok_or("source type is not a Rust enum")?;
    let mut drafts = vec![];
    for variant in &definition.variants {
        let source = format!("{source_type}::{}", variant.ident);
        if crate::primitive_spec::lookup(&source).is_some() {
            continue;
        }
        let records: Vec<_> = report
            .records
            .iter()
            .filter(|r| r.blockers.contains_key(&source))
            .collect();
        if records.is_empty() {
            continue;
        }
        let text = draft_text(&variant.ident.to_string());
        drafts.push(Draft {
            signature: variant
                .fields
                .iter()
                .map(|f| f.ty.to_token_stream().to_string().replace(' ', ""))
                .collect::<Vec<_>>()
                .join(","),
            oracle_hits: records
                .iter()
                .filter(|r| text_matches(&text, &record_text(&r.identity.names, oracle)))
                .count(),
            records: records.len(),
            example_lines: records.iter().take(3).map(|r| r.line).collect(),
            source,
            text,
        });
    }
    drafts.sort_by(|a, b| (&a.signature, b.records).cmp(&(&b.signature, a.records)));
    Ok(drafts)
}

/// Every use of a reviewed primitive on a card whose Oracle text lacks its words.
pub fn audit_primitives(
    report: &crate::report::Report,
    oracle: &BTreeMap<String, String>,
) -> Vec<String> {
    let mut findings = vec![];
    for record in &report.records {
        let text = record_text(&record.identity.names, oracle);
        for name in record.primitives.keys() {
            let rule = crate::primitive_spec::lookup(name).expect("validated primitive");
            if !text.is_empty() && !text_matches(&rule.text, &text) {
                findings.push(format!(
                    "{name} (\"{}\") on {}",
                    rule.text,
                    record.identity.names.join(" // ")
                ));
            }
        }
    }
    findings
}
