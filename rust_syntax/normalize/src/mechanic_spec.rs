//! Reviewed aliases for reusable mechanic definitions. New mechanics require a
//! shared semantic recipe; rows never embed executable Rust or Oracle strings.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    pub version: u32,
    pub rules: Vec<Rule>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub id: String,
    pub source: String,
    pub arity: usize,
    pub mechanic: Mechanic,
    pub threshold: usize,
    pub once_each_turn: bool,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Mechanic {
    Crew,
}
impl Registry {
    pub fn parse(text: &str) -> Result<Self, String> {
        let r: Self = serde_json::from_str(text).map_err(|e| e.to_string())?;
        r.validate()?;
        Ok(r)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("unsupported mechanic registry version".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        let mut sources = std::collections::BTreeSet::new();
        for r in &self.rules {
            if r.id.is_empty() || !ids.insert(&r.id) || !sources.insert(&r.source) {
                return Err("duplicate/empty mechanic rule".into());
            }
            if r.source.is_empty()
                || !r.source.chars().enumerate().all(|(i, c)| {
                    c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit())
                })
            {
                return Err("invalid mechanic source identifier".into());
            }
            if r.arity != 1 || r.threshold != 0 {
                return Err("unconsumed/reused mechanic fields".into());
            }
        }
        Ok(())
    }
    pub fn generate(&self) -> String {
        let mut out="// Generated from rewrites/mechanics.json5.\nfn generated_mechanic(value:&source::Rule)->Option<MechanicRecipe> {Some(match value {\n".to_string();
        for r in &self.rules {
            out.push_str(&format!("source::Rule::{}(threshold)=>MechanicRecipe {{mechanic:Mechanic::{:?},threshold:i64::from(*threshold),once_each_turn:{}}},\n",r.source,r.mechanic,r.once_each_turn));
        }
        out.push_str("_=>return None,\n})}\n");
        out
    }
}
