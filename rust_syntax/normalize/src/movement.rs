//! Explicit preparation at the execution point of one operation.
use crate::{
    ir::*,
    visit::{VisitorMut, WalkMut},
};
mod transfer;
pub fn normalize_card(mut card: Card) -> Card {
    card.walk_mut(&mut Normalize);
    card
}
pub fn normalize_unified(card: &mut UnifiedCard) {
    card.walk_mut(&mut Normalize);
}
pub fn normalize_program(program: &mut Program) {
    program.walk_mut(&mut Normalize);
}
struct Normalize;
impl VisitorMut for Normalize {
    fn operation_end(&mut self, operation: &mut Operation) {
        if matches!(operation, Operation::DraftChangeZone { .. }) {
            *operation = transfer::canonical_move(operation);
            operation.walk_mut(self);
            return;
        }
        if matches!(operation, Operation::WithSelections { .. }) {
            return;
        }
        if let Operation::Together(actions) = operation {
            if let [first, last] = actions.as_slice() {
                if matches!(
                    first.action(),
                    Operation::Reveal { .. } | Operation::RevealFromHand { .. }
                ) && matches!(last.action(), Operation::DiscardObjects { .. })
                {
                    *operation = Operation::InOrder(std::mem::take(actions));
                }
            }
        }
        if let Operation::Together(actions) = operation {
            let mut prepared = Vec::new();
            for member in actions {
                if let Operation::WithSelections { steps, action } = member {
                    let mut rename = Rename(std::collections::BTreeMap::new());
                    for mut step in std::mem::take(steps) {
                        // Inputs see preceding bindings, including an outer
                        // binding shadowed by this step's own id.
                        step.objects.walk_mut(&mut rename);
                        step.count.walk_mut(&mut rename);
                        step.constraints
                            .iter_mut()
                            .for_each(|c| c.walk_mut(&mut rename));
                        let binding = u32::try_from(prepared.len()).expect("selection id overflow");
                        rename.0.insert(step.binding.0, binding);
                        step.binding = SelectionId(binding);
                        prepared.push(step);
                    }
                    action.walk_mut(&mut rename);
                    *member = (**action).clone();
                }
            }
            if !prepared.is_empty() {
                let action = std::mem::replace(operation, Operation::Together(vec![]));
                *operation = Operation::WithSelections {
                    steps: prepared,
                    action: Box::new(action),
                };
                return;
            }
        }
        let mut extraction = Extract {
            root: true,
            steps: vec![],
        };
        operation.walk_mut(&mut extraction);
        if !extraction.steps.is_empty() {
            let action = std::mem::replace(operation, Operation::Together(vec![]));
            *operation = Operation::WithSelections {
                steps: extraction.steps,
                action: Box::new(action),
            };
        }
    }
}
struct Extract {
    root: bool,
    steps: Vec<SelectionStep>,
}
impl VisitorMut for Extract {
    fn operation(&mut self, _: &mut Operation) -> bool {
        std::mem::replace(&mut self.root, false)
    }
    fn instruction(&mut self, _: &mut Instruction) -> bool {
        false
    }
    fn ability(&mut self, _: &mut Ability) -> bool {
        false
    }
    fn cost(&mut self, _: &mut Cost) -> bool {
        false
    }
    fn announcement(&mut self, _: &mut Announcement) -> bool {
        false
    }
    fn target_spec(&mut self, _: &mut TargetSpec) -> bool {
        false
    }
    fn event_spec(&mut self, _: &mut EventSpec) -> bool {
        false
    }
    fn object_filter(&mut self, _: &mut ObjectFilter) -> bool {
        false
    }
    fn object_set_end(&mut self, objects: &mut ObjectSet) {
        let (chooser, pool, count) = match objects {
            ObjectSet::DraftChosen {
                chooser,
                objects,
                count,
            } => (
                SelectionChooser::Players(chooser.clone()),
                (**objects).clone(),
                count.clone(),
            ),
            ObjectSet::DraftRandom { objects, count } => (
                SelectionChooser::Chance,
                (**objects).clone(),
                Count::AsManyAsCan(count.clone()),
            ),
            _ => return,
        };
        let binding = SelectionId(self.steps.len() as u32);
        *objects = ObjectSet::Prepared(binding.clone());
        self.steps.push(SelectionStep {
            binding,
            chooser,
            objects: pool,
            count,
            constraints: vec![],
        });
    }
}

/// Rename local reads while respecting independently declared and nested scopes.
struct Rename(std::collections::BTreeMap<u32, u32>);
impl VisitorMut for Rename {
    fn instruction(&mut self, _: &mut Instruction) -> bool {
        false
    }
    fn ability(&mut self, _: &mut Ability) -> bool {
        false
    }
    fn cost(&mut self, _: &mut Cost) -> bool {
        false
    }
    fn announcement(&mut self, _: &mut Announcement) -> bool {
        false
    }
    fn target_spec(&mut self, _: &mut TargetSpec) -> bool {
        false
    }
    fn event_spec(&mut self, _: &mut EventSpec) -> bool {
        false
    }
    fn operation(&mut self, op: &mut Operation) -> bool {
        if let Operation::WithSelections { steps, action } = op {
            let outer = self.0.clone();
            for step in steps {
                step.objects.walk_mut(self);
                step.count.walk_mut(self);
                step.constraints.iter_mut().for_each(|c| c.walk_mut(self));
                self.0.remove(&step.binding.0);
            }
            action.walk_mut(self);
            self.0 = outer;
            false
        } else {
            true
        }
    }
    fn object_set_end(&mut self, objects: &mut ObjectSet) {
        if let ObjectSet::Prepared(binding) = objects {
            if let Some(new) = self.0.get(&binding.0) {
                binding.0 = *new;
            }
        }
    }
}
