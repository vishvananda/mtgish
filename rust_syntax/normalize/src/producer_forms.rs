//! Producer normal forms. These run on newly lowered programs, never on saved
//! cards or comparison operands. Each substitution has a lexical binding proof.
use crate::{
    ir::*,
    visit::{Visitor, VisitorMut, Walk, WalkMut},
};

pub(crate) fn normalize(card: &mut UnifiedCard) {
    for face in &mut card.faces {
        normalize_card(face);
    }
    for field in &mut card.data {
        normalize_value(field);
    }
}
pub(crate) fn normalize_card(card: &mut Card) {
    normalize_value(card);
}
fn normalize_value<T: Walk + WalkMut>(card: &mut T) {
    struct Results(u32);
    impl Visitor for Results {
        fn instruction(&mut self, i: &Instruction) -> bool {
            if let Instruction::Effect { result, .. } = i {
                self.0 = self.0.max(result.0);
            }
            true
        }
    }
    let mut results = Results(0);
    card.walk(&mut results);
    card.walk_mut(&mut Normalize {
        next_result: results.0.checked_add(1).expect("result id overflow"),
    });
}

/// Only object references are substituted. A player's event binding can name
/// a damage recipient even when the object's event binding names its source.
struct Subject(ObjectRef);
impl VisitorMut for Subject {
    fn ability(&mut self, _: &mut Ability) -> bool {
        false
    }
    fn operation(&mut self, op: &mut Operation) -> bool {
        !matches!(
            op,
            Operation::CreateTrigger { .. } | Operation::CreateReplacement { .. }
        )
    }
    fn object_ref(&mut self, object: &mut ObjectRef) -> bool {
        if matches!(object, ObjectRef::EventSubject) {
            *object = self.0.clone();
            false
        } else {
            true
        }
    }
}

fn subject(event: &EventSpec) -> Option<ObjectRef> {
    let filter = match event {
        EventSpec::Object { objects, .. }
        | EventSpec::ZoneChange { objects, .. }
        | EventSpec::BecomesTarget { objects, .. } => objects,
        EventSpec::Damage(damage) if matches!(damage.subject, DamageSubject::Source) => {
            &damage.sources
        }
        EventSpec::Attack { attackers, .. } => attackers,
        EventSpec::Filtered { event, .. } | EventSpec::FirstEachTurn(event) => {
            return subject(event)
        }
        EventSpec::AnyOf(events) => {
            let first = subject(events.first()?)?;
            return events
                .iter()
                .all(|e| subject(e).is_some_and(|s| same_object(&s, &first)))
                .then_some(first);
        }
        _ => return None,
    };
    match filter {
        ObjectFilter::SameObject(object) => Some(object.clone()),
        _ => None,
    }
}
fn same_object(a: &ObjectRef, b: &ObjectRef) -> bool {
    match (a, b) {
        (ObjectRef::Source, ObjectRef::Source) => true,
        (ObjectRef::Target { slot: a, index: i }, ObjectRef::Target { slot: b, index: j }) => {
            a.0 == b.0 && i == j
        }
        _ => false,
    }
}

/// A filter's Candidate becomes its enclosing matched object when a free
/// condition is lifted out. Nested filters introduce their own Candidate.
struct Candidate {
    object: ObjectRef,
    filter_depth: usize,
    unresolved: bool,
}
impl VisitorMut for Candidate {
    fn object_filter(&mut self, _: &mut ObjectFilter) -> bool {
        self.filter_depth += 1;
        true
    }
    fn object_filter_end(&mut self, _: &mut ObjectFilter) {
        self.filter_depth -= 1;
    }
    fn object_ref(&mut self, object: &mut ObjectRef) -> bool {
        if matches!(object, ObjectRef::Candidate) {
            if self.filter_depth == 0 {
                *object = self.object.clone();
            } else {
                self.unresolved = true;
            }
            false
        } else {
            true
        }
    }
}
fn condition(c: &mut Condition) {
    match c {
        Condition::AllOf(cs) | Condition::AnyOf(cs) => cs.iter_mut().for_each(condition),
        Condition::Not(c) => condition(c),
        Condition::ObjectMatches { object, filter } => {
            // Optional targets, empty queries and unsuccessful move results
            // need the existence test supplied by ObjectMatches. A source or
            // established event/iteration binding has an original object even
            // after departure, so lifting does not discard that requirement.
            if !matches!(
                object,
                ObjectRef::Source
                    | ObjectRef::EventSubject
                    | ObjectRef::EventOther
                    | ObjectRef::Candidate
                    | ObjectRef::Bound(_)
            ) {
                return;
            }
            fn lift(object: &ObjectRef, filter: ObjectFilter) -> Condition {
                match filter {
                    ObjectFilter::Condition(mut c) => {
                        let original = c.clone();
                        let mut binding = Candidate {
                            object: object.clone(),
                            filter_depth: 0,
                            unresolved: false,
                        };
                        c.walk_mut(&mut binding);
                        // History predicates and nested queries can still read
                        // the enclosing candidate. Keep that binding wrapper
                        // when lifting would leave an unresolved capture.
                        if binding.unresolved {
                            return Condition::ObjectMatches {
                                object: object.clone(),
                                filter: ObjectFilter::Condition(original),
                            };
                        }
                        condition(&mut c);
                        *c
                    }
                    ObjectFilter::AllOf(fs) => {
                        let mut conditions: Vec<_> =
                            fs.into_iter().map(|f| lift(object, f)).collect();
                        if conditions.len() == 1 {
                            conditions.pop().unwrap()
                        } else {
                            Condition::AllOf(conditions)
                        }
                    }
                    f => Condition::ObjectMatches {
                        object: object.clone(),
                        filter: f,
                    },
                }
            }
            let liftable = matches!(filter, ObjectFilter::Condition(_))
                || matches!(filter, ObjectFilter::AllOf(fs) if fs.iter().any(|f| matches!(f, ObjectFilter::Condition(_))));
            if liftable {
                *c = lift(object, filter.clone());
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_missing_optional_target_keeps_its_object_match_guard() {
        let mut value = Condition::ObjectMatches {
            object: ObjectRef::Target {
                slot: TargetSlot(0),
                index: 0,
            },
            filter: ObjectFilter::Condition(Box::new(Condition::PlayerIsActive(
                PlayerRef::AbilityController,
            ))),
        };
        condition(&mut value);
        assert!(matches!(value, Condition::ObjectMatches { .. }));
    }

    #[test]
    fn a_nested_candidate_keeps_its_original_enclosing_binding() {
        let mut value = Condition::ObjectMatches {
            object: ObjectRef::Source,
            filter: ObjectFilter::Condition(Box::new(Condition::Compare {
                left: Expr::CountObjects(Box::new(ObjectSet::Zone {
                    scope: ZoneScope::Shared(EventZone::Battlefield),
                    kind: ObjectKind::Permanent,
                    filter: ObjectFilter::SameObject(ObjectRef::Candidate),
                })),
                test: NumberTest::Compare {
                    comparison: Comparison::Greater,
                    value: Expr::Constant(0),
                },
            })),
        };
        let before = serde_json::to_value(&value).unwrap();
        condition(&mut value);
        assert_eq!(serde_json::to_value(value).unwrap(), before);
    }
}

/// Reflexive instructions inherit their creator's object frame. The payment
/// event does not establish a new object. Ordinary created event triggers do.
struct ReflexiveSubject(ObjectRef);
impl VisitorMut for ReflexiveSubject {
    fn ability(&mut self, _: &mut Ability) -> bool {
        false
    }
    fn operation(&mut self, op: &mut Operation) -> bool {
        match op {
            Operation::ReflexiveTrigger {
                announcement,
                body,
                condition,
                ..
            } => {
                let mut replace = Subject(self.0.clone());
                announcement.walk_mut(&mut replace);
                body.walk_mut(&mut replace);
                if let Some(c) = condition {
                    c.walk_mut(&mut replace);
                }
                false
            }
            Operation::CreateTrigger { .. } | Operation::CreateReplacement { .. } => false,
            _ => true,
        }
    }
}

/// Follow-ups which operate exclusively on the newly entered/created objects.
/// A failed entry produces no applicable recipient, unlike an independent draw
/// or life gain. Keep aggregate token results intact (replacement multiplicity).
fn follows(objects: &ObjectSet, first: &ResultId) -> bool {
    match objects {
        ObjectSet::Created(result) => result.0 == first.0,
        ObjectSet::One(ObjectRef::Result {
            result,
            phase: ObjectPhase::After,
            ..
        })
        | ObjectSet::Transition {
            result,
            phase: ObjectPhase::After,
        } => result.0 == first.0,
        _ => false,
    }
}
fn dependent(i: &Instruction, first: &ResultId) -> bool {
    match i {
        Instruction::Effect { operation, .. } => match operation {
            Operation::CreateContinuousEffect {
                affected: EffectSubjects::Objects(objects),
                ..
            }
            | Operation::Move { objects, .. }
            | Operation::Sacrifice { objects, .. } => follows(objects, first),
            Operation::CreateDelayedTrigger {
                body,
                intervening_if: None,
                ..
            } => {
                !body.instructions.is_empty()
                    && body.instructions.iter().all(|i| dependent(i, first))
            }
            _ => false,
        },
        _ => false,
    }
}

struct Normalize {
    next_result: u32,
}
impl VisitorMut for Normalize {
    fn instruction_end(&mut self, i: &mut Instruction) {
        if let Instruction::If { condition: c, .. } = i {
            condition(c);
        }
        if let Instruction::Optional { decider, body } = i {
            let Some(Instruction::Effect { result, operation }) = body.instructions.first() else {
                return;
            };
            if body.instructions.len() < 2
                || !matches!(
                    operation.action(),
                    Operation::Move {
                        destination: MovementDestination::Battlefield { .. },
                        ..
                    } | Operation::CreateTokens { .. }
                )
                || !body.instructions[1..].iter().all(|i| dependent(i, result))
            {
                return;
            }
            let result = ResultId(self.next_result);
            self.next_result = self.next_result.checked_add(1).expect("result id overflow");
            let then = Program {
                instructions: body.instructions.split_off(1),
            };
            let payment = Operation::PayCost {
                player: decider.clone(),
                optional: true,
                cost: Cost {
                    parts: vec![CostPart::PlayerAction {
                        player: decider.clone(),
                        body: std::mem::replace(
                            body,
                            Program {
                                instructions: vec![],
                            },
                        ),
                    }],
                },
            };
            *i = Instruction::Group(Program {
                instructions: vec![
                    Instruction::Effect {
                        result: result.clone(),
                        operation: payment,
                    },
                    Instruction::If {
                        condition: Condition::PaymentChosen(result),
                        then,
                        otherwise: Program {
                            instructions: vec![],
                        },
                    },
                ],
            });
        }
    }
    fn operation_end(&mut self, op: &mut Operation) {
        if let Operation::CreateTrigger {
            event,
            body,
            intervening_if,
            ..
        } = op
        {
            fn stable_subject(event: &EventSpec) -> Option<ObjectRef> {
                match event {
                    // The matcher can test an old object but bind its actual
                    // new arrival. Captured targets retain the old incarnation.
                    EventSpec::ZoneChange { .. } => None,
                    EventSpec::Filtered { event, .. } | EventSpec::FirstEachTurn(event) => {
                        stable_subject(event)
                    }
                    EventSpec::AnyOf(events) => {
                        let first = stable_subject(events.first()?)?;
                        events
                            .iter()
                            .all(|e| stable_subject(e).is_some_and(|s| same_object(&first, &s)))
                            .then_some(first)
                    }
                    _ => subject(event),
                }
            }
            if let Some(object @ ObjectRef::Target { .. }) = stable_subject(event) {
                body.walk_mut(&mut Subject(object.clone()));
                if let Some(c) = intervening_if {
                    c.walk_mut(&mut Subject(object));
                }
            }
            if let Some(c) = intervening_if {
                condition(c);
            }
        }
        if let Operation::Keyword {
            actor,
            action: KeywordAction::Connive { object, .. },
        } = op
        {
            *actor = PlayerSet::One(PlayerRef::ControllerOf(Box::new(object.clone())));
        }
    }
    fn ability_end(&mut self, ability: &mut Ability) {
        if let Ability::Triggered {
            event,
            body,
            intervening_if,
            ..
        } = ability
        {
            if let Some(ObjectRef::Source) = subject(event) {
                body.walk_mut(&mut ReflexiveSubject(ObjectRef::Source));
                // Connive is performed by the named object's current/LKI
                // controller, not the controller of the trigger on the stack.
                struct ConniveSource;
                impl VisitorMut for ConniveSource {
                    fn ability(&mut self, _: &mut Ability) -> bool {
                        false
                    }
                    fn operation(&mut self, op: &mut Operation) -> bool {
                        if let Operation::Keyword {
                            actor,
                            action: KeywordAction::Connive { object, .. },
                        } = op
                        {
                            if matches!(object, ObjectRef::EventSubject) {
                                *object = ObjectRef::Source;
                            }
                            *actor =
                                PlayerSet::One(PlayerRef::ControllerOf(Box::new(object.clone())));
                        }
                        !matches!(
                            op,
                            Operation::CreateTrigger { .. } | Operation::CreateReplacement { .. }
                        )
                    }
                }
                body.walk_mut(&mut ConniveSource);
            }
            if let Some(c) = intervening_if {
                condition(c);
            }
        }
    }
}
