//! Structural cost addresses and cardinality. This is independent of game state;
//! action execution/replacements determine the actual history at runtime.
use crate::{ir::*, number_spec::PaymentAction};

pub fn action(part: &CostPart) -> Option<PaymentAction> {
    match part {
        CostPart::Constrained { part, .. } => action(part),
        CostPart::Sacrifice { .. } => Some(PaymentAction::Sacrifice),
        CostPart::Discard { .. } => Some(PaymentAction::Discard),
        CostPart::ChangeTapState {
            symbol: false,
            tapped,
            ..
        } => Some(if *tapped {
            PaymentAction::Tap
        } else {
            PaymentAction::Untap
        }),
        CostPart::Move {
            destination: Destination::Exile,
            ..
        } => Some(PaymentAction::Exile),
        CostPart::RemoveCounters { .. } => Some(PaymentAction::RemoveCounters),
        _ => None,
    }
}
pub fn component<'a>(cost: &'a Cost, path: &[u32]) -> Option<&'a CostPart> {
    let (index, rest) = path.split_first()?;
    let mut part = cost.parts.get(*index as usize)?;
    while let CostPart::Constrained { part: inner, .. } = part {
        part = inner;
    }
    if rest.is_empty() {
        return action(part).map(|_| part);
    }
    let CostPart::Choice(options) = part else {
        return None;
    };
    let (option, rest) = rest.split_first()?;
    component(options.get(*option as usize)?, rest)
}
pub fn components(cost: &Cost) -> Vec<(PaymentAction, Vec<u32>)> {
    fn walk(cost: &Cost, prefix: &mut Vec<u32>, out: &mut Vec<(PaymentAction, Vec<u32>)>) {
        for (i, part) in cost.parts.iter().enumerate() {
            prefix.push(i as u32);
            let mut part = part;
            while let CostPart::Constrained { part: inner, .. } = part {
                part = inner;
            }
            if let CostPart::Choice(options) = part {
                for (i, option) in options.iter().enumerate() {
                    prefix.push(i as u32);
                    walk(option, prefix, out);
                    prefix.pop();
                }
            } else if let Some(action) = action(part) {
                out.push((action, prefix.clone()));
            }
            prefix.pop();
        }
    }
    let mut out = vec![];
    walk(cost, &mut vec![], &mut out);
    out
}
pub fn singular(part: &CostPart) -> bool {
    fn one(count: &Count) -> bool {
        matches!(
            count,
            Count::Exactly(Expr::Constant(0..=1)) | Count::UpTo(Expr::Constant(0..=1))
        )
    }
    match part {
        CostPart::Constrained { part, .. } => singular(part),
        CostPart::Discard {
            count: Expr::Constant(0..=1),
            ..
        } => true,
        CostPart::Sacrifice { objects, choose } => choose
            .as_ref()
            .map_or(matches!(objects, ObjectSet::One(_)), one),
        CostPart::ChangeTapState { selections, .. } | CostPart::Move { selections, .. } => {
            matches!(selections.as_slice(),[CostObjectSelection {objects,choose}] if choose.as_ref().map_or(matches!(objects,ObjectSet::One(_)),one))
        }
        _ => false,
    }
}
pub fn zone(part: &CostPart, phase: &ObjectPhase) -> Option<EventZone> {
    match part {
        CostPart::Constrained { part, .. } => zone(part, phase),
        CostPart::Sacrifice { .. } if matches!(phase, ObjectPhase::Before) => {
            Some(EventZone::Battlefield)
        }
        CostPart::Discard { .. } if matches!(phase, ObjectPhase::Before) => Some(EventZone::Hand),
        CostPart::ChangeTapState { .. } if matches!(phase, ObjectPhase::Before) => {
            Some(EventZone::Battlefield)
        }
        CostPart::Move {
            destination: Destination::Exile,
            ..
        } if matches!(phase, ObjectPhase::After) => Some(EventZone::Exile),
        _ => None,
    }
}
/// Only unrepeated, directly declared additional costs are currently bound.
/// Alternative/conditional/linked cost declarations need their own namespaces.
pub fn casting_cost(ability: &Ability) -> Option<&Cost> {
    match ability {
        Ability::Casting(CastingRule::AdditionalCost {
            cost,
            repetition: CostRepetition::Once,
            ..
        }) => Some(cost),
        _ => None,
    }
}
