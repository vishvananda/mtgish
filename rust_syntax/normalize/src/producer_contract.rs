//! Fresh producer forms adopted from grammar-v0.2.1. Saved IR is untouched.
use crate::{
    ir::*,
    visit::{VisitorMut, WalkMut},
};

pub(super) struct Normalize;
impl VisitorMut for Normalize {
    fn ability(&mut self, ability: &mut Ability) -> bool {
        if let Ability::Spell { announcement, body } = ability {
            announcement.walk_mut(&mut OwnSpell);
            body.walk_mut(&mut OwnSpell);
            exchange_pair(ability);
        }
        true
    }
}

struct OwnSpell;
impl VisitorMut for OwnSpell {
    fn ability(&mut self, _: &mut Ability) -> bool {
        false
    }
    fn cost(&mut self, _: &mut Cost) -> bool {
        false
    }
    fn operation(&mut self, operation: &mut Operation) -> bool {
        !matches!(
            operation,
            Operation::CreateTrigger { .. }
                | Operation::CreateDelayedTrigger { .. }
                | Operation::ReflexiveTrigger { .. }
                | Operation::CreateReplacement { .. }
        )
    }
    fn expr(&mut self, expression: &mut Expr) -> bool {
        if matches!(
            expression,
            Expr::ObjectProperty {
                object: ObjectRef::Source,
                property: Property::SpellX,
                information: Information::CurrentOrLastKnown,
            }
        ) {
            *expression = Expr::AnnouncedX;
            return false;
        }
        true
    }
}

fn exchange_pair(ability: &mut Ability) {
    let Ability::Spell { announcement, body } = ability else {
        return;
    };
    // Pay serialization costs only for the tiny plain-exchange candidate set.
    if announcement.targets.len() != 2
        || body.instructions.len() != 1
        || !matches!(
            &body.instructions[0],
            Instruction::Effect {
                operation: Operation::Keyword {
                    action: KeywordAction::ExchangeControl { .. },
                    ..
                },
                ..
            }
        )
    {
        return;
    }
    let mut value = serde_json::to_value([&*ability]).expect("typed ability");
    // The owner proves matching domains, filters, distinctness, counts and
    // absence of dependent slot uses. Do not maintain a second such proof.
    if crate::canonical::abilities(&mut value).is_err() {
        return;
    }
    let Ok(
        [Ability::Spell {
            announcement: proof,
            ..
        }],
    ) = serde_json::from_value::<[Ability; 1]>(value)
    else {
        return;
    };
    if proof.targets.len() != 1
        || !matches!(proof.targets[0].count, Count::Exactly(Expr::Constant(2)))
    {
        return;
    }
    let Ability::Spell { announcement, body } = ability else {
        unreachable!()
    };
    let slot = announcement.targets[0].slot.clone();
    announcement.targets.truncate(1);
    announcement.targets[0].count = Count::Exactly(Expr::Constant(2));
    if let Instruction::Effect {
        operation:
            Operation::Keyword {
                action: KeywordAction::ExchangeControl { second, .. },
                ..
            },
        ..
    } = &mut body.instructions[0]
    {
        *second = ObjectRef::Target { slot, index: 1 };
    }
}
