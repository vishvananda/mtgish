//! Generic card semantics and compilation. Kept apart from the large parser
//! schema so edits to lowering do not recompile its serialization derives.
#[cfg(test)]
extern crate self as mtgish_ast;
pub use rust_syntax::{corpus, inspect, mtg_types};
pub mod binding_map;
mod binding_roles;
pub mod canonical;
pub use mtgish_ir::{battlefield_query, counter, declarations, ir, ir_schema, visit};
pub mod change_lists;
mod complete;
pub mod copying;
pub mod json_input;
#[cfg(feature = "legacy")]
pub mod lower;
pub mod migrate;
pub mod movement;
pub mod presentation;
#[cfg(feature = "legacy")]
pub mod report;
pub mod result_reads;
mod source_values;
pub mod keyword_actions;
pub mod mode_policies;
pub mod validate;

mod subtype;

#[cfg(feature = "cantrip")]
pub mod cantrip;
#[cfg(feature = "cantrip")]
mod cantrip_vocab;

#[cfg(feature = "legacy")]
pub mod inventory;
#[cfg(feature = "legacy")]
pub mod mechanic_spec;
pub mod number_spec;
#[cfg(feature = "legacy")]
pub mod operand_spec;
#[cfg(feature = "legacy")]
pub mod pipeline;
pub mod primitive_spec;
#[cfg(feature = "legacy")]
pub mod program_spec;
#[cfg(feature = "legacy")]
pub mod render;
#[cfg(feature = "legacy")]
pub mod rewrite_spec;
#[cfg(feature = "legacy")]
pub mod selector_spec;
pub mod semantics;
#[cfg(feature = "legacy")]
pub mod unified;
#[cfg(feature = "legacy")]
pub mod value_audit;

pub mod cost_history;
pub use mtgish_ir::targets;
pub mod human;
