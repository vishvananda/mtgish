// Typed bridge; these are production expressions, never sentence-shaped actions.
impl Context {
    fn mana_production(&self, value: &source::ManaProduce) -> Result<ManaProduction> {
        self.reduced_mana_production(value).or_else(|error| {
            let (name, operands) = self.primitive("ManaProduce", value).ok_or(error)?;
            Ok(ManaProduction::Primitive { name, operands })
        })
    }

    fn reduced_mana_production(&self, value: &source::ManaProduce) -> Result<ManaProduction> {
        use source::ManaProduce as S;
        let color = match value {
            S::ManaProduceW => Some(ManaColor::White),
            S::ManaProduceU => Some(ManaColor::Blue),
            S::ManaProduceB => Some(ManaColor::Black),
            S::ManaProduceR => Some(ManaColor::Red),
            S::ManaProduceG => Some(ManaColor::Green),
            S::ManaProduceC => Some(ManaColor::Colorless),
            _ => None,
        };
        if let Some(color) = color {
            return Ok(ManaProduction::Amount {
                color,
                amount: Expr::Constant(1),
            });
        }
        Ok(match value {
            S::And(parts) => ManaProduction::All(
                parts
                    .iter()
                    .map(|p| self.mana_production(p))
                    .collect::<Result<_>>()?,
            ),
            S::Or(parts) => ManaProduction::ChooseOne(
                parts
                    .iter()
                    .map(|p| self.mana_production(p))
                    .collect::<Result<_>>()?,
            ),
            S::AnyManaColor => ManaProduction::ChooseOne(
                [
                    ManaColor::White,
                    ManaColor::Blue,
                    ManaColor::Black,
                    ManaColor::Red,
                    ManaColor::Green,
                ]
                .into_iter()
                .map(|color| ManaProduction::Amount {
                    color,
                    amount: Expr::Constant(1),
                })
                .collect(),
            ),
            _ => return Err(unsupported(value)),
        })
    }
    fn mana_sources(&self, value: &source::AbilitySources) -> Result<ObjectFilter> {
        use source::AbilitySources as S;
        Ok(match value {
            S::And(v) => ObjectFilter::All(
                v.iter()
                    .map(|v| self.mana_sources(v))
                    .collect::<Result<_>>()?,
            ),
            S::Or(v) => ObjectFilter::Either(
                v.iter()
                    .map(|v| self.mana_sources(v))
                    .collect::<Result<_>>()?,
            ),
            S::IsCardtype(ty) => ObjectFilter::CardType(ty.clone()),
            S::IsCreatureType(ty) => ObjectFilter::Subtype(crate::subtype::creature_type(ty)),
            _ => return Err(unsupported(value)),
        })
    }
    fn mana_use(&self, value: &source::ManaUseModifier) -> Result<ManaUse> {
        use source::ManaUseModifier as S;
        Ok(match value {
            S::CanOnlySpendOnSpells(spells) => ManaUse::CastSpell(self.spell_filter(spells)?),
            S::CantSpendOnSpells(spells) => {
                ManaUse::Not(Box::new(ManaUse::CastSpell(self.spell_filter(spells)?)))
            }
            S::CanOnlySpendToActivateAbilities => ManaUse::ActivateAbility {
                sources: ObjectFilter::Any,
                permanents_only: false,
            },
            S::CanOnlySpendToActivateAbilitiesOfPermanents(p) => ManaUse::ActivateAbility {
                sources: self.filter(p)?,
                permanents_only: true,
            },
            S::CanOnlySpendToActivateAbilitiesOfSources(p) => ManaUse::ActivateAbility {
                sources: self.mana_sources(p)?,
                permanents_only: false,
            },
            S::And(v) => ManaUse::All(v.iter().map(|v| self.mana_use(v)).collect::<Result<_>>()?),
            S::Or(v) => ManaUse::Either(v.iter().map(|v| self.mana_use(v)).collect::<Result<_>>()?),
            _ => return Err(unsupported(value)),
        })
    }
}
