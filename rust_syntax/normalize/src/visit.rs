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
    fn player_ref(&mut self, _: &PlayerRef) -> bool {
        true
    }
    fn expr(&mut self, _: &Expr) -> bool {
        true
    }
    fn cost(&mut self, _: &Cost) -> bool {
        true
    }
    fn announcement(&mut self, _: &Announcement) -> bool {
        true
    }
    fn target_spec(&mut self, _: &TargetSpec) -> bool {
        true
    }
    fn event_spec(&mut self, _: &EventSpec) -> bool {
        true
    }
}

pub trait Walk {
    fn walk<V: Visitor>(&self, visitor: &mut V);
}
impl<T: Walk> Walk for Vec<T> {
    fn walk<V: Visitor>(&self, visitor: &mut V) {
        self.iter().for_each(|value| value.walk(visitor));
    }
}
impl<T: Walk> Walk for [T] {
    fn walk<V: Visitor>(&self, visitor: &mut V) {
        self.iter().for_each(|value| value.walk(visitor));
    }
}

/// Mutable hooks bracket each node in declaration order. Returning false skips
/// its children; the postorder hook still runs and may replace the node.
pub trait VisitorMut {
    fn ability(&mut self, _: &mut Ability) -> bool {
        true
    }
    fn ability_end(&mut self, _: &mut Ability) {}
    fn instruction(&mut self, _: &mut Instruction) -> bool {
        true
    }
    fn instruction_end(&mut self, _: &mut Instruction) {}
    fn operation(&mut self, _: &mut Operation) -> bool {
        true
    }
    fn operation_end(&mut self, _: &mut Operation) {}
    fn object_filter(&mut self, _: &mut ObjectFilter) -> bool {
        true
    }
    fn object_filter_end(&mut self, _: &mut ObjectFilter) {}
    fn object_set(&mut self, _: &mut ObjectSet) -> bool {
        true
    }
    fn object_set_end(&mut self, _: &mut ObjectSet) {}
    fn object_ref(&mut self, _: &mut ObjectRef) -> bool {
        true
    }
    fn object_ref_end(&mut self, _: &mut ObjectRef) {}
    fn player_ref(&mut self, _: &mut PlayerRef) -> bool {
        true
    }
    fn player_ref_end(&mut self, _: &mut PlayerRef) {}
    fn expr(&mut self, _: &mut Expr) -> bool {
        true
    }
    fn expr_end(&mut self, _: &mut Expr) {}
    fn cost(&mut self, _: &mut Cost) -> bool {
        true
    }
    fn cost_end(&mut self, _: &mut Cost) {}
    fn announcement(&mut self, _: &mut Announcement) -> bool {
        true
    }
    fn announcement_end(&mut self, _: &mut Announcement) {}
    fn target_spec(&mut self, _: &mut TargetSpec) -> bool {
        true
    }
    fn target_spec_end(&mut self, _: &mut TargetSpec) {}
    fn event_spec(&mut self, _: &mut EventSpec) -> bool {
        true
    }
    fn event_spec_end(&mut self, _: &mut EventSpec) {}
}
pub trait WalkMut {
    fn walk_mut<V: VisitorMut>(&mut self, visitor: &mut V);
}

include!(concat!(env!("OUT_DIR"), "/ir_visit.rs"));
