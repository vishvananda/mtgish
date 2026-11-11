//! Source spell paragraphs share one announcement and resolution scope.
use crate::mtg_types::{Actions, Rule};

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
