//! Typed traversal generated from the portable IR's own declarations. New IR
//! fields are included automatically; no Debug or serialization shape is read.
use crate::ir::*;

/// Hooks return false to stop traversal beneath that node. This lets a query
/// distinguish a lexical closure from an independently declared ability.
pub trait Visitor {
    fn ability(&mut self, _: &Ability) -> bool {
        true
    }
    fn instruction(&mut self, _: &Instruction) -> bool {
        true
    }
    fn operation(&mut self, _: &Operation) -> bool {
        true
    }
    fn object_filter(&mut self, _: &ObjectFilter) -> bool {
        true
    }
    fn object_set(&mut self, _: &ObjectSet) -> bool {
        true
    }
    fn object_ref(&mut self, _: &ObjectRef) -> bool {
        true
    }
    fn expr(&mut self, _: &Expr) -> bool {
        true
    }
}

pub trait Walk {
    fn walk<V: Visitor>(&self, visitor: &mut V);
}

include!(concat!(env!("OUT_DIR"), "/ir_visit.rs"));
