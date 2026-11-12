//! Printed definitions and bounds for X are evaluated by the core at announcement.
//! A card supplies the value; it does not impose the rule during resolution.
use super::*;

pub(super) const FEATURE: &str = "computed-x";

impl Compiler {
    pub(super) fn x_value_callback(&mut self, value: &Expr, label: &str) -> Result<String> {
        let mut asked = self.clone();
        (asked.reader, asked.event_at) = (Reader::Query, EventAt::Nowhere);
        asked.costing_targets = true;
        let number = asked.number(value)?.code;
        writeln!(self.functions, "#[allow(unused_variables)] fn {label}(q: &dyn Query, asker: &Asker, targets: &[Target]) -> u32 {{ ({number}).clamp(0, i64::from(u32::MAX)) as u32 }}").unwrap();
        Ok(label.into())
    }

    /// A single chosen player's count (Spoils of War). Preserve the choice as
    /// part of defining X; it is not a target or a fresh choice at resolution.
    fn x_player_count(&mut self, value: &Expr, label: &str) -> Result<Option<String>> {
        let Expr::CountObjects(objects) = value else {
            return Ok(None);
        };
        let ObjectSet::Zone {
            scope:
                ZoneScope::Players {
                    players: PlayerSet::One(PlayerRef::OneOf(among)),
                    ..
                },
            ..
        } = objects.as_ref()
        else {
            return Ok(None);
        };
        let mut number = value.clone();
        let Expr::CountObjects(objects) = &mut number else {
            unreachable!()
        };
        let ObjectSet::Zone {
            scope: ZoneScope::Players { players, .. },
            ..
        } = objects.as_mut()
        else {
            unreachable!()
        };
        *players = PlayerSet::One(PlayerRef::Affected);
        let mut asked = self.clone();
        (asked.reader, asked.event_at) = (Reader::Query, EventAt::Nowhere);
        asked.rule_player = true;
        let member = asked.player_member(among)?;
        let number = asked.number(&number)?.code;
        let players = format!("{label}_players");
        let amount = format!("{label}_value");
        writeln!(self.functions, "#[allow(unused_variables)] fn {players}(q: &dyn Query, asker: &Asker, player: PlayerId) -> bool {{ {member} }}\n#[allow(unused_variables)] fn {amount}(q: &dyn Query, asker: &Asker, player: PlayerId) -> u32 {{ ({number}).clamp(0, i64::from(u32::MAX)) as u32 }}").unwrap();
        Ok(Some(format!(
            "XRule::OfPlayer {{ players: {players}, value: {amount} }}"
        )))
    }

    pub(super) fn printed_spell_x(
        &mut self,
        required: &Condition,
        label: &str,
    ) -> Result<Option<String>> {
        let Condition::Compare {
            left: Expr::AnnouncedX,
            test: NumberTest::Compare { comparison, value },
        } = required
        else {
            return Ok(None);
        };
        match comparison {
            Comparison::Equal => {
                if let Some(rule) = self.x_player_count(value, label)? {
                    return Ok(Some(rule));
                }
                let callback = self.x_value_callback(value, &format!("{label}_x"))?;
                Ok(Some(format!("XRule::Is({callback})")))
            }
            Comparison::LessOrEqual => {
                let callback = self.x_value_callback(value, &format!("{label}_x_max"))?;
                Ok(Some(format!("XRule::AtMost({callback})")))
            }
            _ => Ok(None),
        }
    }
}

/// The payment freezes X (601.2f); a text-defined effect value can remain live
/// (107.3c). Until effect lowering substitutes that definition, do not publish
/// a body that would instead read the frozen payment's `item.x`.
pub(super) fn unresolved_effect_x(limits: &ActivationLimits, body: &Program) -> bool {
    let dynamic = limits.modifiers.iter().any(|modifier| matches!(modifier,
        Change::ActivationRule(ActivationRule::XIs(NumberTest::Compare { comparison: Comparison::Equal, value }))
        if !matches!(value, Expr::Constant(_))));
    if !dynamic {
        return false;
    }
    struct Reads(bool);
    impl crate::visit::Visitor for Reads {
        fn ability(&mut self, _: &Ability) -> bool {
            false
        }
        fn expr(&mut self, value: &Expr) -> bool {
            self.0 |= matches!(value, Expr::AnnouncedX);
            !self.0
        }
    }
    let mut reads = Reads(false);
    crate::visit::Walk::walk(body, &mut reads);
    reads.0
}
