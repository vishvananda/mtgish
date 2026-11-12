//! Cardinality proofs for fresh producer output. Run after movement lowering;
//! never reinterpret a saved card or infer token counts from requested amounts.
use crate::{
    ir::*,
    visit::{VisitorMut, WalkMut},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Default)]
struct Objects {
    cards: bool,
    one: bool,
}
#[derive(Clone, Copy, Default)]
struct Receipt {
    before: Objects,
    after: Objects,
    selected: Objects,
}
#[derive(Clone, Default)]
struct Scope {
    results: BTreeMap<u32, Receipt>,
    written: BTreeSet<u32>,
    targets: BTreeMap<u16, Objects>,
    prepared: BTreeMap<u32, Objects>,
}
fn one(count: &Count) -> bool {
    matches!(
        count,
        Count::Exactly(Expr::Constant(1))
            | Count::UpTo(Expr::Constant(1))
            | Count::AsManyAsCan(Expr::Constant(1))
    )
}
impl Scope {
    fn objects(&self, objects: &ObjectSet) -> Objects {
        match objects {
            ObjectSet::Zone { kind, .. } => Objects {
                cards: *kind == ObjectKind::Card,
                one: false,
            },
            ObjectSet::One(object) => Objects {
                cards: match object {
                    ObjectRef::Target { slot, .. } => {
                        self.targets.get(&slot.0).is_some_and(|p| p.cards)
                    }
                    ObjectRef::Query { objects, .. } => self.objects(objects).cards,
                    ObjectRef::Result { result, phase, .. } => self.transition(result, phase).cards,
                    _ => false,
                },
                one: true,
            },
            ObjectSet::Targets(slot) => self.targets.get(&slot.0).copied().unwrap_or_default(),
            ObjectSet::Take { objects, count, .. } => Objects {
                cards: self.objects(objects).cards,
                one: matches!(count, Expr::Constant(1)) || self.objects(objects).one,
            },
            ObjectSet::Matching { objects, .. } | ObjectSet::InZone { objects, .. } => {
                self.objects(objects)
            }
            ObjectSet::Observed(result) | ObjectSet::Selected(result) => self
                .results
                .get(&result.0)
                .map(|r| r.selected)
                .unwrap_or_default(),
            ObjectSet::Transition { result, phase } => self.transition(result, phase),
            ObjectSet::Prepared(binding) => {
                self.prepared.get(&binding.0).copied().unwrap_or_default()
            }
            ObjectSet::Union(sets) => Objects {
                cards: !sets.is_empty() && sets.iter().all(|s| self.objects(s).cards),
                one: sets.len() == 1 && self.objects(&sets[0]).one,
            },
            _ => Objects::default(),
        }
    }
    fn transition(&self, result: &ResultId, phase: &ObjectPhase) -> Objects {
        self.results
            .get(&result.0)
            .map(|r| match phase {
                ObjectPhase::Before => r.before,
                ObjectPhase::After => r.after,
            })
            .unwrap_or_default()
    }
    fn selection(&mut self, step: &SelectionStep) {
        let pool = self.objects(&step.objects);
        self.prepared.insert(
            step.binding.0,
            Objects {
                cards: pool.cards,
                one: pool.one
                    || (one(&step.count)
                        && matches!(
                            step.chooser,
                            SelectionChooser::Chance | SelectionChooser::Players(PlayerSet::One(_))
                        )),
            },
        );
    }
    fn receipt(&self, operation: &Operation) -> Receipt {
        let mut receipt = Receipt::default();
        match operation {
            Operation::WithSelections { steps, action } => {
                let mut scope = self.clone();
                for step in steps {
                    scope.selection(step);
                }
                return scope.receipt(action);
            }
            Operation::Move {
                objects,
                destination,
                ..
            } => {
                let input = self.objects(objects);
                receipt.before = input;
                receipt.after = Objects {
                    cards: input.cards
                        && !matches!(
                            destination.address(),
                            Destination::Battlefield(_) | Destination::OwnersBattlefield
                        ),
                    one: input.cards && input.one,
                };
            }
            Operation::Destroy { objects, .. } => receipt.before = self.objects(objects),
            Operation::Sacrifice {
                players,
                objects,
                choose,
            } => {
                let input = self.objects(objects);
                receipt.before = Objects {
                    cards: input.cards,
                    one: input.one
                        || (matches!(players, PlayerSet::One(_))
                            && choose.as_ref().is_some_and(one)),
                };
            }
            Operation::ChooseObjects { objects, count, .. } => {
                let input = self.objects(objects);
                receipt.selected = Objects {
                    cards: input.cards,
                    one: input.one || one(count),
                };
            }
            Operation::LookAt { objects, .. }
            | Operation::Reveal { objects }
            | Operation::NoteObjects { objects } => receipt.selected = self.objects(objects),
            Operation::Search { choices, .. } => {
                receipt.selected = Objects {
                    cards: true,
                    one: matches!(choices.as_slice(),[choice] if one(&choice.count)),
                }
            }
            // A requested mill/discard of one can be replaced by several cards.
            Operation::Mill { .. }
            | Operation::Discard { .. }
            | Operation::DiscardObjects { .. } => {
                receipt.before.cards = true;
                receipt.after.cards = true;
            }
            _ => {}
        }
        receipt
    }
}
#[derive(Default)]
pub(super) struct Normalize {
    scope: Scope,
    frames: Vec<Scope>,
}
impl Normalize {
    fn push(&mut self, captures: bool) {
        let mut next = if captures {
            self.scope.clone()
        } else {
            Scope::default()
        };
        next.written.clear();
        self.frames.push(std::mem::replace(&mut self.scope, next));
    }
    fn pop(&mut self) {
        self.scope = self.frames.pop().expect("cardinality scope");
    }
    fn branch(&mut self, body: &mut Program) {
        self.push(true);
        body.walk_mut(self);
        self.pop();
    }
}
fn closure(operation: &Operation) -> bool {
    matches!(
        operation,
        Operation::CreateTrigger { .. }
            | Operation::CreateDelayedTrigger { .. }
            | Operation::CreateReplacement { .. }
            | Operation::ReflexiveTrigger { .. }
            | Operation::WithSelections { .. }
            | Operation::EachPlayer { .. }
    )
}
fn loop_scope(instruction: &Instruction) -> bool {
    matches!(
        instruction,
        Instruction::ForEachPlayer { .. }
            | Instruction::ForEachObject { .. }
            | Instruction::Repeatable(_)
            | Instruction::RepeatableWithLimit { .. }
    )
}
impl VisitorMut for Normalize {
    fn ability(&mut self, _: &mut Ability) -> bool {
        self.push(false);
        true
    }
    fn ability_end(&mut self, _: &mut Ability) {
        self.pop();
    }
    fn mode(&mut self, _: &mut Mode) -> bool {
        self.push(true);
        true
    }
    fn mode_end(&mut self, _: &mut Mode) {
        self.pop();
    }
    fn target_spec(&mut self, target: &mut TargetSpec) -> bool {
        self.scope.targets.insert(target.slot.0,Objects {cards:matches!(&target.kind,TargetKind::Object(domain) if domain.kind==ObjectKind::Card && !domain.zones.contains(&EventZone::Battlefield)),one:one(&target.count)});
        true
    }
    fn instruction(&mut self, instruction: &mut Instruction) -> bool {
        match instruction {
            Instruction::If {
                condition,
                then,
                otherwise,
            } => {
                condition.walk_mut(self);
                self.branch(then);
                self.branch(otherwise);
                false
            }
            Instruction::ChooseOne {
                decider,
                options,
                offered,
            } => {
                decider.walk_mut(self);
                if let Some(n) = offered {
                    n.walk_mut(self);
                }
                for body in options {
                    self.branch(body);
                }
                false
            }
            _ => {
                if loop_scope(instruction) {
                    self.push(true);
                }
                true
            }
        }
    }
    fn instruction_end(&mut self, instruction: &mut Instruction) {
        if loop_scope(instruction) {
            let body = std::mem::take(&mut self.scope);
            self.pop();
            if !matches!(instruction, Instruction::RepeatableWithLimit { .. }) {
                // Immediate loop receipts collect every run. They remain card
                // populations where proven, but cannot keep a singleton proof.
                for id in body.written {
                    let mut receipt = body.results[&id];
                    receipt.before.one = false;
                    receipt.after.one = false;
                    receipt.selected.one = false;
                    self.scope.results.insert(id, receipt);
                    self.scope.written.insert(id);
                }
            }
        }
        if let Instruction::Effect { result, operation } = instruction {
            self.scope.written.insert(result.0);
            self.scope
                .results
                .insert(result.0, self.scope.receipt(operation));
        }
    }
    fn operation(&mut self, operation: &mut Operation) -> bool {
        if closure(operation) {
            self.push(true);
        }
        if let Operation::WithSelections { steps, .. } = operation {
            for step in steps {
                self.scope.selection(step);
            }
        }
        true
    }
    fn operation_end(&mut self, operation: &mut Operation) {
        if closure(operation) {
            self.pop();
        }
    }
    fn condition_end(&mut self, condition: &mut Condition) {
        let Condition::Compare {
            left: Expr::CountObjects(objects),
            test:
                NumberTest::Compare {
                    comparison: Comparison::Greater,
                    value: Expr::Constant(0),
                },
        } = condition
        else {
            return;
        };
        let ObjectSet::Matching { objects, filter } = objects.as_ref() else {
            return;
        };
        let ObjectSet::Transition { result, phase } = objects.as_ref() else {
            return;
        };
        if self.scope.transition(result, phase).one {
            *condition = Condition::ObjectMatches {
                object: ObjectRef::Result {
                    result: result.clone(),
                    phase: phase.clone(),
                    index: 0,
                },
                filter: filter.clone(),
            };
        }
    }
    fn expr_end(&mut self, number: &mut Expr) {
        let Expr::Aggregate {
            objects,
            property,
            aggregation: Aggregation::Sum,
        } = number
        else {
            return;
        };
        let ObjectSet::Transition { result, phase } = objects.as_ref() else {
            return;
        };
        if self.scope.transition(result, phase).one {
            *number = Expr::ObjectProperty {
                object: ObjectRef::Result {
                    result: result.clone(),
                    phase: phase.clone(),
                    index: 0,
                },
                property: property.clone(),
                information: Information::CurrentOrLastKnown,
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mtg_types::CardType;
    fn moved(objects: ObjectSet) -> Instruction {
        Instruction::Effect {
            result: ResultId(0),
            operation: Operation::Move {
                objects,
                destination: MovementDestination::Exile {
                    visibility: ExileVisibility::FaceUp,
                    arrival: Default::default(),
                },
                qualifiers: vec![],
            },
        }
    }
    fn top() -> ObjectSet {
        ObjectSet::Take {
            objects: Box::new(ObjectSet::Zone {
                scope: ZoneScope::Players {
                    zone: EventZone::Library,
                    players: PlayerSet::One(PlayerRef::AbilityController),
                },
                kind: ObjectKind::Card,
                filter: ObjectFilter::Any,
            }),
            count: Expr::Constant(1),
            from: SequenceEnd::Top,
        }
    }
    fn check() -> Instruction {
        Instruction::If {
            condition: Condition::Compare {
                left: Expr::CountObjects(Box::new(ObjectSet::Matching {
                    objects: Box::new(ObjectSet::Transition {
                        result: ResultId(0),
                        phase: ObjectPhase::After,
                    }),
                    filter: ObjectFilter::CardType(CardType::Creature),
                })),
                test: NumberTest::Compare {
                    comparison: Comparison::Greater,
                    value: Expr::Constant(0),
                },
            },
            then: Program {
                instructions: vec![],
            },
            otherwise: Program {
                instructions: vec![],
            },
        }
    }
    fn scalar(instruction: &Instruction) -> bool {
        matches!(
            instruction,
            Instruction::If {
                condition: Condition::ObjectMatches { .. },
                ..
            }
        )
    }
    #[test]
    fn per_iteration_singletons_are_batches_after_a_loop_even_when_ids_shadow() {
        let mut p = Program {
            instructions: vec![
                moved(top()),
                Instruction::ForEachPlayer {
                    players: PlayerSet::All,
                    binding: LoopBinding(0),
                    starting_with: None,
                    in_chosen_direction: false,
                    body: Program {
                        instructions: vec![moved(top()), check()],
                    },
                },
                check(),
            ],
        };
        p.walk_mut(&mut Normalize::default());
        let Instruction::ForEachPlayer { body, .. } = &p.instructions[1] else {
            panic!()
        };
        assert!(scalar(&body.instructions[1]));
        assert!(!scalar(&p.instructions[2]));
    }
    #[test]
    fn alternative_receipts_never_prove_a_sibling_or_outer_result_is_single() {
        let mut p = Program {
            instructions: vec![
                Instruction::If {
                    condition: Condition::Compare {
                        left: Expr::Constant(1),
                        test: NumberTest::Compare {
                            comparison: Comparison::Greater,
                            value: Expr::Constant(0),
                        },
                    },
                    then: Program {
                        instructions: vec![moved(top()), check()],
                    },
                    otherwise: Program {
                        instructions: vec![moved(ObjectSet::One(ObjectRef::Source)), check()],
                    },
                },
                check(),
            ],
        };
        p.walk_mut(&mut Normalize::default());
        let Instruction::If {
            then, otherwise, ..
        } = &p.instructions[0]
        else {
            panic!()
        };
        assert!(scalar(&then.instructions[1]));
        assert!(!scalar(&otherwise.instructions[1]));
        assert!(!scalar(&p.instructions[1]));
    }
    #[test]
    fn a_requested_mill_of_one_keeps_a_batch_receipt() {
        let mut p = Program {
            instructions: vec![
                Instruction::Effect {
                    result: ResultId(0),
                    operation: Operation::Mill {
                        players: PlayerSet::One(PlayerRef::AbilityController),
                        amount: Expr::Constant(1),
                    },
                },
                check(),
            ],
        };
        p.walk_mut(&mut Normalize::default());
        assert!(!scalar(&p.instructions[1]));
    }
}
