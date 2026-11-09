//! Frozen normalized IR types and their wire/schema contracts.
//! Consumers pin this package independently of changing canonicalization,
//! validation and source-lowering code, so its serialization derives stay cached.
pub use rust_syntax::mtg_types;
pub mod counter;
pub mod ir;
pub mod ir_schema;
pub mod visit;

pub mod targets;
