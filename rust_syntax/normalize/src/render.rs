//! Does a complete record mean what its Oracle text means? The record renders to
//! the words of its identifiers, with a primitive's reviewed text, and the Oracle
//! text to its words and word pairs. Mining finds the pairs that hold across
//! nearly every card, so no table of synonyms is needed. A card breaking one has
//! either lost a clause, or says something its text does not, or the pair was only
//! a habit of the wording. This is a report to read, not a gate: most pairs are the
//! last kind, and a pair's review names its cards so a new card reopens it.
use crate::corpus::{self, Result};
use crate::mtg_types as source;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

const LEAST_CARDS: u32 = 20;
const LEAST_CARDS_FOR_PAIRS: u32 = 40;
const CONFIDENCE: f64 = 0.92;
const LIFT: f64 = 4.0;
const SHOWN: usize = 12;

fn stem(word: &str) -> String {
    let word = word.trim_end_matches("'s").trim_end_matches('\'');
    let word = match word {
        "one" => "1",
        "two" | "twice" => "2",
        "three" => "3",
        "four" => "4",
        "five" => "5",
        "six" => "6",
        "seven" => "7",
        "eight" => "8",
        "nine" => "9",
        "ten" => "10",
        other => other,
    };
    if word.len() > 3 && word.ends_with("ies") {
        return format!("{}y", &word[..word.len() - 3]);
    }
    if word.len() > 3 && word.ends_with('s') && !word.ends_with("ss") {
        return word[..word.len() - 1].to_string();
    }
    word.to_string()
}

/// `DealDamage`, `Action::Mill` and `Ref_Target` become lowercase words.
fn identifier_words(text: &str, out: &mut BTreeSet<String>) {
    let mut word = String::new();
    let mut flush = |word: &mut String| {
        if !word.is_empty() {
            out.insert(stem(&word.to_lowercase()));
            word.clear();
        }
    };
    let mut previous_upper = false;
    for c in text.chars() {
        if !c.is_alphanumeric() {
            flush(&mut word);
            previous_upper = false;
            continue;
        }
        if c.is_uppercase() && !previous_upper {
            flush(&mut word);
        }
        previous_upper = c.is_uppercase();
        word.push(c);
    }
    flush(&mut word);
}

fn rendered(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::Array(items) => items.iter().for_each(|item| rendered(item, out)),
        Value::Object(fields) => fields.iter().for_each(|(name, field)| {
            identifier_words(name, out);
            if name == "name" {
                // A name is either a card's, which Oracle text drops, or a primitive's.
                if let Some(rule) = field.as_str().and_then(crate::primitive_spec::lookup) {
                    identifier_words(&rule.text.replace('|', " "), out);
                    identifier_words(field.as_str().unwrap_or_default(), out);
                }
                return;
            }
            rendered(field, out);
        }),
        Value::String(text) => identifier_words(text, out),
        Value::Number(n) => {
            out.insert(n.to_string().trim_start_matches('-').to_string());
        }
        _ => {}
    }
}

fn oracle_words(text: &str, names: &[String], out: &mut BTreeSet<String>) {
    // The words before a dash name an ability; they have no rules meaning (207.2c).
    let mut text = text
        .lines()
        .map(|line| line.split_once(" — ").map_or(line, |(_, rest)| rest))
        .collect::<Vec<_>>()
        .join("\n");
    for name in names {
        let name = name.to_lowercase();
        // A card can call itself by the part of its name before the comma.
        for name in [name.as_str(), name.split(',').next().unwrap_or_default()] {
            text = text.replace(name, " ");
        }
    }
    let mut previous: Option<String> = None;
    for token in text.split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == '{' || c == '}')) {
        if token.is_empty() {
            previous = None;
            continue;
        }
        let word = stem(token);
        if let Some(previous) = &previous {
            out.insert(format!("{previous} {word}"));
        }
        out.insert(word.clone());
        previous = Some(word);
    }
}

/// Why these cards may break a mined pair.
#[derive(serde::Deserialize)]
pub struct Reviewed {
    pub reason: String,
    pub cards: BTreeSet<String>,
}

struct Card {
    name: String,
    oracle: BTreeSet<String>,
    ast: BTreeSet<String>,
}

fn card(source: &source::OracleCard, oracle: &BTreeMap<String, String>) -> Result<Option<Card>> {
    let lowered = crate::unified::lower(source);
    if !lowered.diagnostics.is_empty() || !crate::unified::forms(&lowered.card).named.is_empty() {
        return Ok(None);
    }
    let names: Vec<String> = lowered.card.faces.iter().map(|f| f.name.clone()).collect();
    let text = names
        .iter()
        .filter_map(|name| oracle.get(name))
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    if text.is_empty() {
        return Ok(None);
    }
    let (mut oracle_bag, mut ast) = (BTreeSet::new(), BTreeSet::new());
    oracle_words(&text, &names, &mut oracle_bag);
    let mut value = serde_json::to_value(&lowered.card)?;
    // Printed characteristics say nothing about what the rules text does.
    for face in value["faces"].as_array_mut().into_iter().flatten() {
        if let Some(fields) = face.as_object_mut() {
            fields.remove("characteristics");
            fields.remove("schema_version");
        }
    }
    rendered(&value, &mut ast);
    Ok(Some(Card {
        name: names.join(" // "),
        oracle: oracle_bag,
        ast,
    }))
}

fn ids(bags: impl Iterator<Item = String>, least: impl Fn(&str) -> u32) -> HashMap<String, usize> {
    let mut counts: HashMap<String, u32> = HashMap::new();
    for word in bags {
        *counts.entry(word).or_default() += 1;
    }
    let mut kept: Vec<String> = counts
        .into_iter()
        .filter(|(word, count)| *count >= least(word))
        .map(|(word, _)| word)
        .collect();
    kept.sort();
    kept.into_iter().enumerate().map(|(i, w)| (w, i)).collect()
}

/// One direction: for each `from`, the `to` that follows it, and the cards without it.
fn mine(
    cards: &[Card],
    from: impl Fn(&Card) -> &BTreeSet<String>,
    to: impl Fn(&Card) -> &BTreeSet<String>,
    arrow: &str,
    reviewed: &BTreeMap<String, Reviewed>,
) -> Vec<(usize, String)> {
    let left = ids(cards.iter().flat_map(|c| from(c).iter().cloned()), |w| {
        if w.contains(' ') {
            LEAST_CARDS_FOR_PAIRS
        } else {
            LEAST_CARDS
        }
    });
    let right = ids(cards.iter().flat_map(|c| to(c).iter().cloned()), |_| {
        LEAST_CARDS
    });
    let (width, height) = (right.len(), left.len());
    let mut both = vec![0u32; width * height];
    let (mut left_count, mut right_count) = (vec![0u32; height], vec![0u32; width]);
    for card in cards {
        let l: Vec<usize> = from(card)
            .iter()
            .filter_map(|w| left.get(w).copied())
            .collect();
        let r: Vec<usize> = to(card)
            .iter()
            .filter_map(|w| right.get(w).copied())
            .collect();
        r.iter().for_each(|j| right_count[*j] += 1);
        for i in &l {
            left_count[*i] += 1;
            let row = &mut both[i * width..(i + 1) * width];
            r.iter().for_each(|j| row[*j] += 1);
        }
    }
    let names: BTreeMap<usize, &String> = right.iter().map(|(w, j)| (*j, w)).collect();
    let total = cards.len() as f64;
    let mut findings = vec![];
    for (word, i) in &left {
        let row = &both[i * width..(i + 1) * width];
        let seen = left_count[*i] as f64;
        let best = (0..width)
            .filter(|j| {
                let confidence = row[*j] as f64 / seen;
                confidence >= CONFIDENCE
                    && confidence / (right_count[*j] as f64 / total) >= LIFT
                    && row[*j] < left_count[*i]
            })
            .max_by_key(|j| row[*j]);
        let Some(j) = best else { continue };
        let token = names[&j];
        let pair = format!("{word} {arrow} {token}");
        let accepted = reviewed.get(&pair).map(|review| &review.cards);
        let missing: Vec<&str> = cards
            .iter()
            .filter(|c| from(c).contains(word) && !to(c).contains(token))
            .filter(|c| accepted.is_none_or(|accepted| !accepted.contains(&c.name)))
            .map(|c| c.name.as_str())
            .collect();
        if missing.is_empty() {
            continue;
        }
        findings.push((
            missing.len(),
            format!(
                "{pair} ({} of {}): {}",
                missing.len(),
                left_count[*i],
                missing[..missing.len().min(SHOWN)].join(", ")
            ),
        ));
    }
    findings
}

/// Each pair some card breaks, leaving out the cards `reviewed` accepts for that pair.
pub fn audit(corpus_path: &Path, oracle: &Path, reviewed: &Path) -> Result<Vec<String>> {
    let reviewed: BTreeMap<String, Reviewed> =
        serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(reviewed)?))?;
    let oracle = crate::pipeline::candidates::oracle_text(oracle)?;
    let mut cards = vec![];
    corpus::map_cards(
        corpus_path,
        |_, source| card(&source, &oracle),
        |_, card| {
            cards.extend(card);
            Ok(())
        },
    )?;
    let mut findings = mine(&cards, |c| &c.oracle, |c| &c.ast, "->", &reviewed);
    findings.extend(mine(&cards, |c| &c.ast, |c| &c.oracle, "<-", &reviewed));
    findings.sort();
    Ok(findings.into_iter().map(|(_, text)| text).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bag(words: &[&str]) -> BTreeSet<String> {
        words.iter().map(|word| word.to_string()).collect()
    }

    fn cards(both: u32, broken: &[&str]) -> Vec<Card> {
        let kept = (0..both).map(|i| Card {
            name: format!("Kept {i}"),
            oracle: bag(&["mill"]),
            ast: bag(&["mill"]),
        });
        let unrelated = (0..both * 5).map(|i| Card {
            name: format!("Other {i}"),
            oracle: bag(&["draw"]),
            ast: bag(&["draw"]),
        });
        let broken = broken.iter().map(|name| Card {
            name: name.to_string(),
            oracle: bag(&["mill"]),
            ast: bag(&["draw"]),
        });
        kept.chain(unrelated).chain(broken).collect()
    }

    fn found(cards: &[Card], reviewed: &BTreeMap<String, Reviewed>) -> Vec<String> {
        mine(cards, |c| &c.oracle, |c| &c.ast, "->", reviewed)
            .into_iter()
            .map(|(_, text)| text)
            .collect()
    }

    #[test]
    fn identifiers_and_oracle_words_meet_in_one_spelling() {
        let mut words = BTreeSet::new();
        identifier_words("Action::MillCards", &mut words);
        identifier_words("Ref_TargetPermanent", &mut words);
        assert_eq!(
            words,
            bag(&["action", "card", "mill", "permanent", "ref", "target"])
        );
        assert_eq!(stem("libraries"), "library");
        assert_eq!(stem("twice"), "2");
    }

    #[test]
    fn oracle_words_drop_ability_words_and_the_cards_own_name() {
        let mut words = BTreeSet::new();
        let names = vec!["Boon, the Kind".to_string()];
        oracle_words("landfall — boon draws two cards.", &names, &mut words);
        assert!(words.contains("draw 2"), "{words:?}");
        assert!(
            !words.contains("landfall") && !words.contains("boon"),
            "{words:?}"
        );
    }

    #[test]
    fn a_held_pair_reports_the_card_that_breaks_it() {
        let lines = found(&cards(40, &["Broken"]), &BTreeMap::new());
        assert_eq!(lines, ["mill -> mill (1 of 41): Broken"]);
    }

    #[test]
    fn a_review_accepts_its_cards_and_a_new_card_reopens_the_pair() {
        let reviewed = BTreeMap::from([(
            "mill -> mill".to_string(),
            Reviewed {
                reason: "wording".to_string(),
                cards: bag(&["Accepted"]),
            },
        )]);
        assert!(found(&cards(40, &["Accepted"]), &reviewed).is_empty());
        let lines = found(&cards(40, &["Accepted", "New"]), &reviewed);
        assert_eq!(lines, ["mill -> mill (1 of 42): New"]);
    }
}
