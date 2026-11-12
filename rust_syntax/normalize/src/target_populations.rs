//! Optional targets as populations, independent of the consuming operation.
//! Scalar reference fields keep their explicit identity/existence semantics.
use crate::ir::*;

#[derive(Default)]
pub(crate) struct TargetPopulations {
    bindings: Vec<(u16, bool)>,
    frames: Vec<(usize, usize, usize)>,
    floor: usize,
    pub(crate) cost_depth: usize,
}
impl TargetPopulations {
    pub(crate) fn push(&mut self, captures: bool) {
        self.frames
            .push((self.bindings.len(), self.floor, self.cost_depth));
        self.cost_depth = 0;
        if !captures {
            self.floor = self.bindings.len();
        }
    }
    pub(crate) fn pop(&mut self) {
        let (len, floor, cost_depth) = self.frames.pop().expect("target population scope");
        self.bindings.truncate(len);
        self.floor = floor;
        self.cost_depth = cost_depth;
    }
    pub(crate) fn declare(&mut self, spec: &TargetSpec) {
        self.bindings.push((
            spec.slot.0,
            matches!(spec.count, Count::UpTo(Expr::Constant(1))),
        ));
    }
    fn optional(&self, slot: &TargetSlot, index: u16) -> bool {
        index == 0
            && self.bindings[self.floor..]
                .iter()
                .rev()
                .find(|b| b.0 == slot.0)
                .is_some_and(|b| b.1)
    }
    /// A payment of one required object must not become a payment of the
    /// empty target set. Give quantified costs an explicit exactly-one count.
    /// Cost forms without a count retain their required scalar/existence guard.
    pub(crate) fn cost_part(&self, part: &mut CostPart) {
        let selection = |objects: &mut ObjectSet, choose: &mut Option<Count>| {
            if let ObjectSet::One(ObjectRef::Target { slot, index }) = objects {
                if self.optional(slot, *index) {
                    if choose.is_none() {
                        *choose = Some(Count::Exactly(Expr::Constant(1)));
                    }
                    *objects = ObjectSet::Targets(slot.clone());
                }
            }
        };
        match part {
            CostPart::Move { selections, .. } | CostPart::ChangeTapState { selections, .. } => {
                for item in selections {
                    selection(&mut item.objects, &mut item.choose);
                }
            }
            CostPart::Sacrifice { objects, choose } => selection(objects, choose),
            _ => {}
        }
    }
    pub(crate) fn objects(&self, objects: &mut ObjectSet) {
        if self.cost_depth > 0 {
            return;
        }
        if let ObjectSet::One(ObjectRef::Target { slot, index }) = objects {
            if self.optional(slot, *index) {
                *objects = ObjectSet::Targets(slot.clone());
            }
        }
    }
    pub(crate) fn players(&self, players: &mut PlayerSet) {
        if self.cost_depth > 0 {
            return;
        }
        if let PlayerSet::One(PlayerRef::Target { slot, index }) = players {
            if self.optional(slot, *index) {
                *players = PlayerSet::Targets(slot.clone());
            }
        }
    }
    pub(crate) fn recipients(&self, recipients: &mut Recipients) {
        if self.cost_depth > 0 {
            return;
        }
        if let Recipients::Target { slot, index } = recipients {
            if self.optional(slot, *index) {
                *recipients = Recipients::Targets(slot.clone());
            }
        }
    }
}
