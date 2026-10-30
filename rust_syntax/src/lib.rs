//! Card syntax, semantic normalization, and corpus validation.
//!
//! `mtg_types` remains the legacy parser input contract. The `mtgish-ast`
//! workspace package supplies semantic normalization and backend compilation.

pub mod corpus;
pub mod inspect;
pub mod mtg_types;
