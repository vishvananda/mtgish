//! Generic card semantics and compilation. Kept apart from the large parser
//! schema so edits to lowering do not recompile its serialization derives.
#[cfg(test)]
extern crate self as mtgish_ast;
pub use mtgish_ir::{battlefield_query, declarations, ir, ir_schema, visit};
pub use oracle_ir_contract::binding_map;
pub use oracle_ir_contract::canonical;
pub use oracle_ir_contract::change_lists;
pub use rust_syntax::{corpus, inspect, mtg_types};
pub mod coin_policies;
use oracle_ir_contract::complete;
pub use oracle_ir_contract::copying;
pub use oracle_ir_contract::json_input;
pub use oracle_ir_contract::keyword_actions;
#[cfg(feature = "legacy")]
pub mod lower;
pub use oracle_ir_contract::migrate;
pub use oracle_ir_contract::mode_policies;
pub use oracle_ir_contract::movement;
pub use oracle_ir_contract::presentation;
#[cfg(feature = "legacy")]
pub mod report;
#[cfg(feature = "legacy")]
pub mod oracle_export;
pub use oracle_ir_contract::result_reads;
use oracle_ir_contract::source_values;
#[cfg(feature = "legacy")]
mod producer_forms;
pub use oracle_ir_contract::validate;

use oracle_ir_contract::subtype;
#[cfg(feature = "legacy")]
mod spell_paragraphs;

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

pub use mtgish_ir::targets;
pub use oracle_ir_contract::cost_history;
pub use oracle_ir_contract::human;

mod target_populations;

pub mod counter;
