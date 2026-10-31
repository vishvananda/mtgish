//! Rank backend gaps by what each one unlocks. `check-cantrip` stops at a card's
//! first gap, so it cannot say which card needs only one thing.
use super::Diagnostic;
use crate::corpus;
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

/// Every gap of one complete, primitive-free, single-face record.
#[derive(Serialize)]
pub struct Blocked {
    pub line: usize,
    pub name: String,
    pub gaps: Vec<Diagnostic>,
}

#[derive(Serialize)]
pub struct Step {
    pub feature: String,
    /// Cards this gap alone finishes, once every earlier step is done.
    pub unlocks: usize,
    /// Cards it appears on at all, for ties and for gaps that never finish one.
    pub appears_on: usize,
}

#[derive(Default, Serialize)]
pub struct Survey {
    pub total: usize,
    pub complete: usize,
    pub complete_without_primitives: usize,
    /// Primitive-free complete records of another layout, which the generator
    /// has no face model for and the scan does not attempt.
    pub other_layouts: usize,
    pub capable: usize,
    /// Every gap the capable records would otherwise show, by record count.
    pub features: BTreeMap<String, usize>,
    /// Records that also depend on a primitive, by that primitive.
    pub primitive_records: BTreeMap<String, usize>,
    pub greedy: Vec<Step>,
    pub blocked: Vec<Blocked>,
}

enum Outcome {
    Incomplete,
    Primitive(Vec<String>),
    OtherLayout,
    Compiled(Blocked),
}

/// Repeatedly take the gap finishing the most remaining cards. Ties go to the
/// gap on more cards, so progress continues where no single gap finishes one.
fn greedy(blocked: &[Blocked]) -> Vec<Step> {
    let mut remaining: Vec<BTreeSet<&str>> = blocked
        .iter()
        .map(|b| b.gaps.iter().map(|g| g.feature.as_str()).collect())
        .collect();
    let mut steps = Vec::new();
    while remaining.iter().any(|gaps| !gaps.is_empty()) {
        let mut scores: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
        for gaps in &remaining {
            for gap in gaps {
                let score = scores.entry(gap).or_default();
                score.0 += usize::from(gaps.len() == 1);
                score.1 += 1;
            }
        }
        let (&feature, &(unlocks, appears_on)) = scores
            .iter()
            .max_by_key(|(feature, score)| (**score, std::cmp::Reverse(**feature)))
            .expect("some gap remains");
        let chosen = feature.to_string();
        for gaps in &mut remaining {
            gaps.remove(chosen.as_str());
        }
        remaining.retain(|gaps| !gaps.is_empty());
        steps.push(Step {
            feature: chosen,
            unlocks,
            appears_on,
        });
    }
    steps
}

pub fn survey(input: &Path) -> corpus::Result<Survey> {
    let mut survey = Survey::default();
    corpus::map_cards(
        input,
        |line, source| -> corpus::Result<Outcome> {
            let identity = corpus::Identity::of(&source)?;
            let lowered = crate::unified::lower(&source);
            let forms = crate::unified::forms(&lowered.card);
            if !(lowered.diagnostics.is_empty() && forms.named.is_empty()) {
                return Ok(Outcome::Incomplete);
            }
            if !forms.primitives.is_empty() {
                return Ok(Outcome::Primitive(forms.primitives.into_keys().collect()));
            }
            if lowered.card.layout != "Card" {
                return Ok(Outcome::OtherLayout);
            }
            let gaps = lowered.card.faces.iter().flat_map(super::gaps).collect();
            Ok(Outcome::Compiled(Blocked {
                line,
                name: identity.names.join(" // "),
                gaps,
            }))
        },
        |_, outcome| {
            survey.total += 1;
            match outcome {
                Outcome::Incomplete => {}
                Outcome::Primitive(names) => {
                    survey.complete += 1;
                    for name in names {
                        *survey.primitive_records.entry(name).or_default() += 1;
                    }
                }
                Outcome::OtherLayout => {
                    survey.complete += 1;
                    survey.complete_without_primitives += 1;
                    survey.other_layouts += 1;
                }
                Outcome::Compiled(card) => {
                    survey.complete += 1;
                    survey.complete_without_primitives += 1;
                    if card.gaps.is_empty() {
                        survey.capable += 1;
                    } else {
                        let features: BTreeSet<_> =
                            card.gaps.iter().map(|g| g.feature.as_str()).collect();
                        for feature in features {
                            *survey.features.entry(feature.into()).or_default() += 1;
                        }
                        survey.blocked.push(card);
                    }
                }
            }
            Ok(())
        },
    )?;
    survey.greedy = greedy(&survey.blocked);
    Ok(survey)
}
