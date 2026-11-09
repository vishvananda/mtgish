//! Explicit preparation at the execution point of one operation.
use crate::{
    ir::*,
    visit::{Visitor, VisitorMut, Walk, WalkMut},
};
use std::collections::{BTreeMap, BTreeSet};
mod transfer;

/// Grammar entry flags end here. Validated operations carry only `EntrySpec`.
/// Unknown source material remains an unreviewed primitive instead of being
/// discarded or accepted as a complete typed arrival.
pub fn entry_from_source(flags: &[Operand]) -> EntrySpec {
    transfer::entry_from_source(flags)
}
pub fn normalize_card(mut card: Card) -> Card {
    card.walk_mut(&mut Normalize);
    card
}
pub fn normalize_unified(card: &mut UnifiedCard) {
    card.walk_mut(&mut Normalize);
}
pub fn normalize_abilities(abilities: &mut [Ability]) {
    for ability in abilities {
        ability.walk_mut(&mut Normalize);
    }
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
        if prepare_group(operation, false).is_some()
            && matches!(operation, Operation::WithSelections { .. })
        {
            return;
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
/// Prefix moves produced by the same typed rewrite used in ordinary lowering.
/// Bookkeeping is optional so the ordinary path allocates no provenance strings.
pub(crate) struct PreparationMap {
    pub prefixes: Vec<(String, String)>,
}
pub(crate) fn prepare_group(operation: &mut Operation, record: bool) -> Option<PreparationMap> {
    let Operation::Together(actions) = operation else {
        return None;
    };
    let mut prefixes = vec![];
    if let [first, last] = actions.as_slice() {
        if matches!(
            first.action(),
            Operation::Reveal { .. } | Operation::RevealFromHand { .. }
        ) && matches!(last.action(), Operation::DiscardObjects { .. })
        {
            *operation = Operation::InOrder(std::mem::take(actions));
            if record {
                prefixes.push((String::new(), String::new()));
            }
            return Some(PreparationMap { prefixes });
        }
    }
    if !actions
        .iter()
        .any(|member| matches!(member, Operation::WithSelections { .. }))
    {
        return None;
    }
    let hoist = actions.iter().any(|mut member| {
        while let Operation::WithSelections { steps, action } = member {
            if !steps.is_empty() {
                return true;
            }
            member = action;
        }
        false
    });
    // A sibling may read a selection captured from outside this group. New
    // shared declarations must not shadow that read, even if it appears only
    // inside a future program or a later preparation's inputs.
    let mut captures = FreeSelections::default();
    if hoist {
        actions.walk(&mut captures);
    }
    let mut allocated = captures.free;
    let mut changed = false;
    let mut prepared = vec![];
    for (index, member) in actions.iter_mut().enumerate() {
        let destination = if !record {
            String::new()
        } else if hoist {
            format!("/args/action/args/{index}")
        } else {
            format!("/args/{index}")
        };
        if !matches!(member, Operation::WithSelections { .. }) {
            if record {
                prefixes.push((format!("/args/{index}"), destination));
            }
            continue;
        }
        let mut source = if record {
            format!("/args/{index}")
        } else {
            String::new()
        };
        let mut rename = Rename::new(member);
        // Unwrap the whole consecutive chain. Unwrapping just one layer leaves
        // a newly exposed preparation for the next canonicalization pass.
        while let Operation::WithSelections { steps, action } = member {
            changed = true;
            for (step_index, mut step) in std::mem::take(steps).into_iter().enumerate() {
                step.chooser.walk_mut(&mut rename);
                step.objects.walk_mut(&mut rename);
                step.count.walk_mut(&mut rename);
                step.constraints
                    .iter_mut()
                    .for_each(|c| c.walk_mut(&mut rename));
                let binding = fresh_selection(&mut allocated);
                rename.used.insert(binding);
                rename.names.insert(step.binding.0, binding);
                step.binding = SelectionId(binding);
                if record {
                    prefixes.push((
                        format!("{source}/args/steps/{step_index}"),
                        format!("/args/steps/{}", prepared.len()),
                    ));
                }
                prepared.push(step);
            }
            *member = std::mem::replace(action.as_mut(), Operation::Together(vec![]));
            if record {
                source.push_str("/args/action");
            }
        }
        member.walk_mut(&mut rename);
        if record {
            prefixes.push((source, destination));
        }
    }
    if !prepared.is_empty() {
        let action = std::mem::replace(operation, Operation::Together(vec![]));
        *operation = Operation::WithSelections {
            steps: prepared,
            action: Box::new(action),
        };
    }
    changed.then_some(PreparationMap { prefixes })
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

fn fresh_selection(used: &mut BTreeSet<u32>) -> u32 {
    let mut next = 0;
    for id in used.iter() {
        if *id > next {
            break;
        }
        next = next.checked_add(1).expect("selection id overflow");
    }
    used.insert(next);
    next
}

/// Identify inherited reads using the same lexical lifetime as validation:
/// each preparation's inputs precede its local declaration.
#[derive(Default)]
struct FreeSelections {
    bound: BTreeSet<u32>,
    free: BTreeSet<u32>,
}
impl Visitor for FreeSelections {
    fn ability(&mut self, _: &Ability) -> bool {
        false
    }
    fn operation(&mut self, operation: &Operation) -> bool {
        let Operation::WithSelections { steps, action } = operation else {
            return true;
        };
        let outer = self.bound.clone();
        for step in steps {
            step.chooser.walk(self);
            step.objects.walk(self);
            step.count.walk(self);
            step.constraints.walk(self);
            self.bound.insert(step.binding.0);
        }
        action.walk(self);
        self.bound = outer;
        false
    }
    fn object_set(&mut self, objects: &ObjectSet) -> bool {
        if let ObjectSet::Prepared(binding) = objects {
            if !self.bound.contains(&binding.0) {
                self.free.insert(binding.0);
            }
        }
        true
    }
}

/// Rename captured reads through lexical programs. Independent abilities do not
/// capture selections; a nested preparation can shadow one after its inputs.
struct Rename {
    names: BTreeMap<u32, u32>,
    used: BTreeSet<u32>,
}
impl Rename {
    fn new(operation: &Operation) -> Self {
        #[derive(Default)]
        struct Labels(BTreeSet<u32>);
        impl Visitor for Labels {
            fn selection_id(&mut self, id: &SelectionId) -> bool {
                self.0.insert(id.0);
                false
            }
        }
        let mut labels = Labels::default();
        operation.walk(&mut labels);
        Self {
            names: BTreeMap::new(),
            used: labels.0,
        }
    }
}
impl VisitorMut for Rename {
    fn ability(&mut self, _: &mut Ability) -> bool {
        false
    }
    fn operation(&mut self, op: &mut Operation) -> bool {
        if let Operation::WithSelections { steps, action } = op {
            let outer = self.names.clone();
            for step in steps {
                step.chooser.walk_mut(self);
                step.objects.walk_mut(self);
                step.count.walk_mut(self);
                step.constraints.iter_mut().for_each(|c| c.walk_mut(self));
                let old = step.binding.0;
                self.names.remove(&old);
                if self.names.values().any(|new| *new == old) {
                    let fresh = fresh_selection(&mut self.used);
                    step.binding.0 = fresh;
                    self.names.insert(old, fresh);
                }
            }
            action.walk_mut(self);
            self.names = outer;
            false
        } else {
            true
        }
    }
    fn object_set_end(&mut self, objects: &mut ObjectSet) {
        if let ObjectSet::Prepared(binding) = objects {
            if let Some(new) = self.names.get(&binding.0) {
                binding.0 = *new;
            }
        }
    }
}
