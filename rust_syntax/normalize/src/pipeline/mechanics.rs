//! Parameterized keyword recipes stay independent of parent ability/layout.
pub use crate::mechanic_spec::Mechanic;
use crate::{mechanic_spec::Registry, mtg_types as source};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MechanicRecipe {
    pub mechanic: Mechanic,
    pub threshold: i64,
    pub once_each_turn: bool,
}
pub fn registry() -> &'static Registry {
    static REGISTRY: std::sync::OnceLock<Registry> = std::sync::OnceLock::new();
    REGISTRY.get_or_init(|| {
        Registry::parse(include_str!("../../rewrites/mechanics.json5"))
            .expect("validated mechanic registry")
    })
}
pub fn rewrite(value: &source::Rule) -> Option<MechanicRecipe> {
    generated_mechanic(value)
}
pub fn source_key(value: &source::Rule) -> String {
    serde_json::to_string(value).expect("typed mechanic source")
}
include!(concat!(env!("OUT_DIR"), "/pipeline_mechanics.rs"));
