//! Generic card semantics and compilation. Kept apart from the large parser
//! schema so edits to lowering do not recompile its serialization derives.
#[cfg(test)]
extern crate self as mtgish_ast;
pub use rust_syntax::{corpus, inspect, mtg_types};
pub mod canonical;
pub mod ir;
pub mod ir_schema;
pub mod lower;
pub mod presentation;
pub mod report;
pub mod validate;

pub mod counter;

mod subtype;

#[cfg(feature = "cantrip")]
pub mod cantrip;
#[cfg(feature = "cantrip")]
mod cantrip_vocab;

pub mod inventory;
pub mod mechanic_spec;
pub mod number_spec;
pub mod operand_spec;
pub mod pipeline;
pub mod primitive_spec;
pub mod program_spec;
pub mod render;
pub mod rewrite_spec;
pub mod selector_spec;
pub mod semantics;
pub mod unified;
pub mod value_audit;

pub mod cost_history;
