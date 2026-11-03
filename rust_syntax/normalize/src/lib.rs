//! Generic card semantics and compilation. Kept apart from the large parser
//! schema so edits to lowering do not recompile its serialization derives.
#[cfg(test)]
extern crate self as mtgish_ast;
pub use rust_syntax::{corpus, inspect, mtg_types};
pub mod canonical;
pub use mtgish_ir::{counter, ir, ir_schema};
mod complete;
#[cfg(feature = "legacy")]
pub mod lower;
pub mod presentation;
#[cfg(feature = "legacy")]
pub mod report;
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
