//! The exceptions a copy states (CR 707.9b), each what `Copiable` calls it.
use super::*;

/// The `Vocabulary` words of `operands`.
pub(super) fn words(operands: &[Operand]) -> Result<Vec<&str>> {
    operands
        .iter()
        .map(|operand| match operand {
            Operand::Vocabulary { form, .. } => Ok(form.as_str()),
            other => Err(gap(format!("copy exception of {}", form(other)))),
        })
        .collect()
}

impl Compiler {
    /// `values`, a `Copiable`, with what `adjustments` state.
    pub(super) fn excepted(&mut self, values: String, adjustments: &[Operand]) -> Result<String> {
        adjustments
            .iter()
            .try_fold(values, |values, adjustment| self.except(values, adjustment))
    }
    fn except(&mut self, values: String, exception: &Operand) -> Result<String> {
        let Operand::Term {
            form: name,
            operands,
            ..
        } = exception
        else {
            return Err(gap(format!("copy exception {}", form(exception))));
        };
        Ok(match (name.as_str(), operands.as_slice()) {
            ("NoTokenCopyEffects" | "NoCopyEffects", []) => values,
            ("TokenCopyEffects" | "CopyEffects", list) => self.excepted(values, list)?,
            ("SetPT", [Operand::Term { operands, .. }]) => match operands.as_slice() {
                [Operand::Integer(power), Operand::Integer(toughness)] => {
                    format!("{values}.sized({power}, {toughness})")
                }
                _ => return self.stated_size(values, operands),
            },
            ("RemoveSupertypes", list) if words(list)? == ["Legendary"] => {
                format!("{values}.nonlegendary()")
            }
            ("AddSupertypes", list) if words(list)? == ["Legendary"] => {
                format!("{values}.legendary()")
            }
            ("SetColor", [Operand::Term { form, .. }]) if form == "Colorless" => {
                format!("{values}.colored(ColorSet::NONE)")
            }
            ("SetColor", [Operand::Term { form, operands, .. }]) if form == "SimpleColorList" => {
                let set = words(operands)?
                    .iter()
                    .fold("ColorSet::NONE".to_string(), |set, color| {
                        format!("{set}.with(Color::{color})")
                    });
                format!("{values}.colored({set})")
            }
            ("AddCardtypes" | "AddCardtype", list) => words(list)?.iter().fold(values, |values, word| {
                format!("{values}.with_type(CardType::{word})")
            }),
            ("AddCreatureType" | "AddCreatureTypes" | "AddArtifactTypes" | "AddLandTypes", list) => {
                words(list)?.iter().fold(values, |values, word| {
                    format!("{values}.with_subtype(Subtype::{word})")
                })
            }
            // "Except it's a 4/4 black Zombie": that is what it is (CR 707.9b).
            ("SetCreatureTypes", list) => {
                let types = words(list)?
                    .iter()
                    .map(|word| format!("Subtype::{word}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{values}.typed(TypeSet::of(&[CardType::Creature]), SubtypeSet::of(&[{types}]))")
            }
            ("SetCardtypes", list)
                if words(list)? == ["Creature"]
                    && values.contains(".typed(TypeSet::of(&[CardType::Creature])") =>
            {
                values
            }
            ("AddAbility", [Operand::Ability(ability)]) => match &**ability {
                Ability::Keyword(k) if keyword(k).is_ok() => {
                    format!("{values}.with_keyword({})", keyword(k)?)
                }
                ability if lane_j::granted_keyword(ability).is_some() => {
                    let keyword = lane_j::granted_keyword(ability).expect("just seen");
                    format!("{values}.with_keyword({keyword})")
                }
                ability => {
                    let Ok(index) = self.grant(ability) else {
                        return self.stated_exception(values, "AddAbility", operands);
                    };
                    format!(
                        "{values}.with_ability(AbilityRef::delayed({}, {index}))",
                        self.source_card()
                    )
                }
            },
            ("AddAbilityVariable", list) if self.stack_item && words(list)? == ["ThisAbility"] => {
                format!("{values}.with_ability(item.ability)")
            }
            ("SetName", [Operand::Text(text)]) => format!("{values}.named({})", quoted(text)),
            ("SetLoyalty", [Operand::Integer(n)]) => format!("{values}.with_loyalty({n})"),
            ("HasNoManaCost", []) => format!("{values}.costless()"),
            // A spell's copy, read while `original` names that spell.
            ("IfPermanentSpell", list) => format!(
                "{{ let plain = {values}; if cantrip_mechanics::targets::is_permanent_card(&q.chars(original)) {{ {} }} else {{ plain }} }}",
                self.excepted("plain".into(), list)?
            ),
            (other, list) => return self.stated_exception(values, other, list),
        })
    }
    /// `Option<Copiable>`: `object`'s values with these exceptions, if it's there.
    pub(super) fn copy_values(&mut self, object: &str, adjustments: &[Operand]) -> Result<String> {
        let values = self.excepted("q.copiable_values(original)".into(), adjustments)?;
        Ok(self.fetch(format!("{object}.map(|original: ObjectId| {values})")))
    }
}
