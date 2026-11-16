//! Source spell paragraphs share one announcement and resolution scope.
use crate::mtg_types::{Action, Actions, AdditionalCostOption, CastEffect, Condition, Rule, Spell, Spells};

/// A single optional additional cost and a sole paid/unpaid resolution branch
/// have the same contract as AdditionalCost_Modal. Convert before allocating
/// payment addresses, so paid-object references bind to the branch's cost.
pub(crate) fn branch_optional_cost(rules: &[Rule]) -> Option<Vec<Rule>> {
    let [Rule::CastEffect(casting), Rule::SpellActions(actions)] = rules else { return None };
    let CastEffect::OptionalAdditionalCastingCost(cost) = casting else { return None };
    fn branches(actions: &Actions) -> Option<(Actions, Actions)> {
        match actions {
            Actions::Targeted(targets, body) => {
                let (paid, unpaid) = branches(body)?;
                Some((Actions::Targeted(targets.clone(), Box::new(paid)),
                      Actions::Targeted(targets.clone(), Box::new(unpaid))))
            }
            Actions::ActionList(actions) => {
                let [Action::IfElse(condition, paid, unpaid)] = actions.as_slice() else { return None };
                let filter = match condition {
                    Condition::CastSpellPassesFilter(filter) => filter,
                    Condition::SpellPassesFilter(spell, filter) if matches!(spell.as_ref(), Spell::ThisSpell) => filter,
                    _ => return None,
                };
                if !matches!(filter.as_ref(), Spells::AdditionalCostWasPaid) { return None }
                Some((Actions::ActionList(paid.clone()), Actions::ActionList(unpaid.clone())))
            }
            _ => None,
        }
    }
    let (paid, unpaid) = branches(actions)?;
    Some(vec![Rule::SpellActions(Box::new(Actions::AdditionalCost_Modal(vec![
        AdditionalCostOption::AdditionalCost(cost.clone(), Box::new(paid)),
        AdditionalCostOption::NoAdditionalCost(Box::new(unpaid)),
    ])))])
}

/// Join ordinary action-list paragraphs before allocating receipt IDs. Keep
/// modal/targeted wrappers independent until their announcements can be merged
/// explicitly. Casting declarations after a spell paragraph are also a barrier:
/// their existing structural payment addresses must not be shifted here.
pub(crate) fn join(rules: &[Rule]) -> Option<Vec<Rule>> {
    let mut spells = rules
        .iter()
        .enumerate()
        .filter_map(|(i, rule)| matches!(rule, Rule::SpellActions(_)).then_some(i));
    let first = spells.next()?;
    spells.next()?;
    if rules[first..].iter().any(|rule| matches!(rule, Rule::CastEffect(_)))
        || rules.iter().any(|rule| {
            matches!(rule, Rule::SpellActions(actions) if !matches!(actions.as_ref(), Actions::ActionList(_)))
        })
    {
        return None;
    }
    let mut actions = Vec::new();
    for rule in rules {
        if let Rule::SpellActions(body) = rule {
            let Actions::ActionList(paragraph) = body.as_ref() else {
                unreachable!()
            };
            actions.extend(paragraph.iter().cloned());
        }
    }
    let mut joined = Vec::with_capacity(rules.len() - 1);
    for (i, rule) in rules.iter().enumerate() {
        if i == first {
            joined.push(Rule::SpellActions(Box::new(Actions::ActionList(
                std::mem::take(&mut actions),
            ))));
        } else if !matches!(rule, Rule::SpellActions(_)) {
            joined.push(rule.clone());
        }
    }
    Some(joined)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(text: &str) -> Rule {
        ron::from_str(text).unwrap()
    }

    #[test]
    fn ordinary_paragraphs_keep_order_and_other_abilities() {
        let rules = [
            rule("Flying"),
            rule("SpellActions(ActionList([DrawACard]))"),
            rule("SpellActions(ActionList([GainLife(Integer(2))]))"),
        ];
        let joined = join(&rules).unwrap();
        assert_eq!(joined.len(), 2);
        assert_eq!(
            ron::ser::to_string(&joined[1]).unwrap(),
            "SpellActions(ActionList([DrawACard,GainLife(Integer(2))]))"
        );
        assert!(join(&joined).is_none());
    }

    #[test]
    fn unresolved_announcements_and_later_casting_addresses_are_barriers() {
        for other in [
            "SpellActions(Targeted([TargetPlayer(AnyPlayer)],ActionList([DrawACard])))",
            "CastEffect(AdditionalCastingCost(PayLife(Integer(1))))",
        ] {
            let rules = [
                rule("SpellActions(ActionList([DrawACard]))"),
                rule(other),
                rule("SpellActions(ActionList([GainLife(Integer(2))]))"),
            ];
            assert!(join(&rules).is_none(), "{other}");
        }
    }
}
