//! Copy-time expressions use the creating scope; quoted rules are definitions.
use super::*;

impl Scope<'_> {
    pub(super) fn copy_exceptions(&self, exceptions: &[CopyException]) -> Result {
        exceptions.iter().try_for_each(|e| match e {
            CopyException::Colors { colors, .. } => match colors {
                CopyColors::Listed(colors) if colors.iter().any(|c| matches!(c, ManaColor::Chosen | ManaColor::Colorless)) => Err("copy color lists contain only the five colors; use an empty list for colorless or ChosenColor".into()),
                CopyColors::ManaFrom(id) if !matches!(self.results.get(&id.0),Some(ResultKind::Mana)) => Err(format!("copy color result {} does not provide produced mana in this scope",id.0)),
                _ => Ok(()),
            },
            CopyException::SetPowerToughness { power, toughness } => {self.expr(power)?;self.expr(toughness)},
            CopyException::SetLoyalty(n) => self.expr(n),
            CopyException::AddAbilities(abilities) => abilities.iter().try_for_each(validate_ability),
            CopyException::AddAbilityIfAbsent { ability, matching } => {validate_ability(ability)?;self.ability_pattern(matching)},
            CopyException::RemoveAbilities(pattern) => self.ability_pattern(pattern),
            CopyException::AddAbilitiesFrom { objects, kinds } => {self.objects(objects)?;kinds.iter().try_for_each(|p|self.ability_pattern(p))},
            CopyException::IfPermanentSpell(exceptions) => self.copy_exceptions(exceptions),
            CopyException::SetName(name) if name.is_empty() => Err("copy name cannot be empty".into()),
            CopyException::SetTypes { card_types, subtypes } => {
                for subtype in subtypes {
                    let value=serde_json::to_value(subtype).map_err(|e|e.to_string())?;
                    let fits=card_types.iter().any(|t| match t {
                        crate::mtg_types::CardType::Creature | crate::mtg_types::CardType::Kindred => serde_json::from_value::<crate::mtg_types::CreatureType>(value.clone()).is_ok(),
                        crate::mtg_types::CardType::Artifact => serde_json::from_value::<crate::mtg_types::ArtifactType>(value.clone()).is_ok(),
                        crate::mtg_types::CardType::Land => serde_json::from_value::<crate::mtg_types::LandType>(value.clone()).is_ok(),
                        crate::mtg_types::CardType::Enchantment => serde_json::from_value::<crate::mtg_types::EnchantmentType>(value.clone()).is_ok(),
                        crate::mtg_types::CardType::Planeswalker => serde_json::from_value::<crate::mtg_types::PlaneswalkerType>(value.clone()).is_ok(),
                        crate::mtg_types::CardType::Instant | crate::mtg_types::CardType::Sorcery => serde_json::from_value::<crate::mtg_types::SpellType>(value.clone()).is_ok(),
                        crate::mtg_types::CardType::Battle => matches!(subtype,crate::mtg_types::SubType::Siege),
                        crate::mtg_types::CardType::Dungeon => matches!(subtype,crate::mtg_types::SubType::Undercity),
                        crate::mtg_types::CardType::Plane => serde_json::from_value::<crate::mtg_types::PlaneType>(value.clone()).is_ok(),
                        _=>false,
                    });
                    if !fits{return Err("copy subtype has no corresponding card type".into());}
                }
                Ok(())
            },
            CopyException::AddSupertypes(_) | CopyException::RemoveSupertypes(_)
            | CopyException::AddCardTypes(_) | CopyException::SetCardTypes(_)
            | CopyException::AddCreatureTypes(_) | CopyException::AddArtifactTypes(_)
            | CopyException::SetArtifactTypes(_) | CopyException::AddLandTypes(_)
            | CopyException::MergeTypeline | CopyException::KeepColors | CopyException::SetName(_)
            | CopyException::KeepName | CopyException::KeepPowerToughness | CopyException::RemoveManaCost
            | CopyException::AddCurrentAbility | CopyException::AddChosenAbilities
            | CopyException::RemoveCurrentAbility => Ok(()),
        })
    }
    fn ability_pattern(&self, pattern: &AbilityPattern) -> Result {
        match pattern {
            AbilityPattern::Keyword(keyword) => self.keyword(keyword),
            AbilityPattern::All(patterns) => {
                patterns.iter().try_for_each(|p| self.ability_pattern(p))
            }
            AbilityPattern::Rule(ability) => validate_ability(ability),
            AbilityPattern::Enchant(filter) | AbilityPattern::Landwalk(filter) => {
                self.filter(filter)
            }
            _ => Ok(()),
        }
    }
}
