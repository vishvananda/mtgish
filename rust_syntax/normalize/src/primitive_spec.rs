//! Reviewed atomic forms. A row says a source form is one rules-defined action with
//! no decomposition into generic operations, and gives the words Oracle text uses
//! for it, so `ast primitive-audit` can check the row against every card using it.
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Deserialize)]
pub struct Rule {
    /// `Type::Form` in the source schema.
    pub source: String,
    /// Words that must appear in the Oracle text of cards using this form.
    pub text: String,
    /// `EventBindings` fields a `Trigger` row supplies to references in its body.
    #[serde(default)]
    pub binds: Vec<String>,
    /// What a later "this way" in the same scope names after an `Action` row:
    /// `observed`, `exiled`, `entered` or `hand`.
    #[serde(default)]
    pub yields: Option<String>,
    /// An `Action` row that moves nothing and pays nothing, so earlier
    /// antecedents survive it. Rows holding nested actions never keep.
    #[serde(default)]
    pub keeps: bool,
    /// The cards this row was reviewed for, when it names something it cannot
    /// resolve in its own scope. A row that lists cards applies to no other,
    /// so a new card with no antecedent still fails instead of matching it.
    #[serde(default)]
    pub cards: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Registry {
    pub version: u32,
    /// Fieldless forms of mixed enums that are still plain vocabulary.
    pub vocabularies: Vec<String>,
    /// Descriptive enums a primitive may hold as bound `Operand::Term`s.
    #[serde(default)]
    pub terms: Vec<String>,
    pub rules: Vec<Rule>,
}

/// Whether a row was reviewed for reading outside its scope on this card.
pub fn reads_outside_scope(card: &str) -> bool {
    static CARDS: OnceLock<std::collections::HashSet<&'static str>> = OnceLock::new();
    CARDS
        .get_or_init(|| {
            let rules = &registry().rules;
            rules
                .iter()
                .flat_map(|rule| rule.cards.iter().map(String::as_str))
                .collect()
        })
        .contains(card)
}

pub const SOURCE: &str = include_str!("../rewrites/primitives.json5");

pub fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| serde_json::from_str(SOURCE).expect("valid primitive registry"))
}

pub fn lookup(source: &str) -> Option<&'static Rule> {
    static RULES: OnceLock<HashMap<&'static str, &'static Rule>> = OnceLock::new();
    RULES
        .get_or_init(|| {
            let rules = &registry().rules;
            let by_source: HashMap<_, _> = rules.iter().map(|r| (r.source.as_str(), r)).collect();
            assert_eq!(by_source.len(), rules.len(), "duplicate primitive rows");
            by_source
        })
        .get(source)
        .copied()
}
