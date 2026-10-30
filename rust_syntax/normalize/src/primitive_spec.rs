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
}

#[derive(Debug, Deserialize)]
pub struct Registry {
    pub version: u32,
    /// Fieldless forms of mixed enums that are still plain vocabulary.
    pub vocabularies: Vec<String>,
    pub rules: Vec<Rule>,
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
