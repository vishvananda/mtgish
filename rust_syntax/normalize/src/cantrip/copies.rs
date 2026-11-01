//! The exceptions a copy states (CR 707.9b), each what `Copiable` calls it.
use super::*;

/// The `Vocabulary` words of `operands`.
fn words(operands: &[Operand]) -> Result<Vec<&str>> {
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
                _ => return Err(gap("copy exception of a power and toughness not fixed")),
            },
            ("RemoveSupertypes", list) if words(list)? == ["Legendary"] => {
                format!("{values}.nonlegendary()")
            }
            ("AddSupertypes", list) if words(list)? == ["Legendary"] => {
                format!("{values}.legendary()")
            }
            ("SetColor", [Operand::Term { form, operands, .. }]) if form == "SimpleColorList" => {
                let set = words(operands)?
                    .iter()
                    .fold("ColorSet::NONE".to_string(), |set, color| {
                        format!("{set}.with(Color::{color})")
                    });
                format!("{values}.colored({set})")
            }
            ("AddCardtypes", list) => words(list)?.iter().fold(values, |values, word| {
                format!("{values}.with_type(CardType::{word})")
            }),
            ("AddCreatureTypes" | "AddArtifactTypes" | "AddLandTypes", list) => {
                words(list)?.iter().fold(values, |values, word| {
                    format!("{values}.with_subtype(Subtype::{word})")
                })
            }
            ("AddAbility", [Operand::Ability(ability)]) => match &**ability {
                Ability::Keyword(k) if keyword(k).is_ok() => {
                    format!("{values}.with_keyword({})", keyword(k)?)
                }
                ability => {
                    let index = self.grant(ability)?;
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
            (other, _) => return Err(gap(format!("copy exception {other}"))),
        })
    }
    /// `Option<Copiable>`: `object`'s values with these exceptions, if it's there.
    pub(super) fn copy_values(&mut self, object: &str, adjustments: &[Operand]) -> Result<String> {
        let values = self.excepted("q.copiable_values(original)".into(), adjustments)?;
        Ok(self.fetch(format!("{object}.map(|original: ObjectId| {values})")))
    }
}
