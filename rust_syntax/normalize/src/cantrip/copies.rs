//! Compile typed copy-time exceptions (CR 707.9), never grammar terms.
use super::*;

fn list<T: std::fmt::Debug>(prefix: &str, values: &[T]) -> String {
    values
        .iter()
        .map(|v| format!("{prefix}::{}", name(v)))
        .collect::<Vec<_>>()
        .join(", ")
}

impl Compiler {
    pub(super) fn excepted(
        &mut self,
        mut values: String,
        exceptions: &[CopyException],
    ) -> Result<String> {
        let mut known_types: Option<std::collections::BTreeSet<String>> = None;
        for exception in exceptions {
            // Compare type state, not substrings in emitted Rust. An intervening
            // type addition must not be hidden by an earlier full replacement.
            if let CopyException::SetCardTypes(types) = exception {
                let next = types.iter().map(name).collect();
                if known_types.as_ref() == Some(&next) {
                    continue;
                }
            }
            values = self.except(values, exception)?;
            match exception {
                CopyException::SetTypes { card_types, .. }
                | CopyException::SetCardTypes(card_types) => {
                    known_types = Some(card_types.iter().map(name).collect())
                }
                CopyException::AddCardTypes(types) => {
                    if let Some(known) = &mut known_types {
                        known.extend(types.iter().map(name));
                    }
                }
                CopyException::MergeTypeline
                | CopyException::IfPermanentSpell(_)
                | CopyException::AddAbilities(_)
                | CopyException::AddAbilityIfAbsent { .. }
                | CopyException::AddCurrentAbility
                | CopyException::AddChosenAbilities
                | CopyException::AddAbilitiesFrom { .. } => known_types = None,
                _ => {}
            }
        }
        Ok(values)
    }
    fn except(&mut self, values: String, exception: &CopyException) -> Result<String> {
        use CopyException as E;
        let planned = |feature: &str, code: String| planned::planned_value(feature, &values, &code);
        Ok(match exception {
            E::RemoveSupertypes(types) if matches!(types.as_slice(),[crate::mtg_types::SuperType::Legendary])=>format!("{values}.nonlegendary()"),
            E::AddSupertypes(types) if matches!(types.as_slice(),[crate::mtg_types::SuperType::Legendary])=>format!("{values}.legendary()"),
            E::RemoveSupertypes(types)=>planned("copy exception RemoveSupertypes",format!("{values}.without_supertypes(&[{}])",list("Supertype",types))),
            E::AddSupertypes(types)=>planned("copy exception AddSupertypes",format!("{values}.with_supertypes(&[{}])",list("Supertype",types))),
            E::AddCardTypes(types)=>types.iter().fold(values,|v,t|format!("{v}.with_type(CardType::{})",name(t))),
            E::SetCardTypes(types)=>planned("copy exception SetCardtypes",format!("{values}.typed_as(TypeSet::of(&[{}]))",list("CardType",types))),
            E::AddCreatureTypes(types)=>self.copy_subtypes(values,&types.iter().map(crate::subtype::creature_type).collect::<Vec<_>>()),
            E::AddArtifactTypes(types)=>self.copy_subtypes(values,&types.iter().map(crate::subtype::artifact_type).collect::<Vec<_>>()),
            E::AddLandTypes(types)=>self.copy_subtypes(values,&types.iter().map(crate::subtype::land_type).collect::<Vec<_>>()),
            E::SetTypes{card_types,subtypes}=>{
                let subtypes=subtypes.iter().map(|t|format!("q.words(asker.source).subtype(Subtype::{})",name(t))).collect::<Vec<_>>().join(", ");
                format!("{values}.typed(TypeSet::of(&[{}]), SubtypeSet::of(&[{subtypes}]))",list("CardType",card_types))
            }
            E::SetArtifactTypes(types)=>planned("copy exception SetArtifactTypes",format!("{values}.subtyped_as(CardType::Artifact, SubtypeSet::of(&[{}]))",list("Subtype",types))),
            E::Colors{mode,colors:CopyColors::Listed(colors)}=>match mode {
                ChangeMode::Set=>{
                    let set=colors.iter().fold("ColorSet::NONE".into(),|set,c|format!("{set}.with(q.words(asker.source).color(Color::{}))",name(c)));
                    format!("{values}.colored({set})")
                }
                ChangeMode::Add=>lane_o::added_colors(&values,&list("Color",colors)),
            },
            E::Colors{mode,..}=>return Err(gap(if matches!(mode,ChangeMode::Set){"copy exception SetColor"}else{"copy exception AddColor"})),
            E::KeepColors=>format!("{{ let own = q.copiable_values(asker.source); {values}.colored(own.colors.unwrap_or(q.card(own.card).colors())) }}"),
            E::SetName(text)=>format!("{values}.named({})",quoted(text)),
            E::KeepName=>format!("{{ let own = q.copiable_values(asker.source); {values}.named(own.name.unwrap_or(q.card(own.card).name)) }}"),
            E::SetPowerToughness{power:Expr::Constant(p),toughness:Expr::Constant(t)}=>{
                let(_,low,high)=quantity::pt_kind();
                format!("{values}.sized({}, {})",i128::from(*p).clamp(low,high),i128::from(*t).clamp(low,high))
            }
            E::SetPowerToughness{power,toughness}=>format!("{values}.sized({}, {})",self.copy_size(power)?,self.copy_size(toughness)?),
            E::KeepPowerToughness=>format!("{{ let own = q.card(q.chars(asker.source).card); {values}.sized(own.power.unwrap_or(0), own.toughness.unwrap_or(0)) }}"),
            E::SetLoyalty(Expr::Constant(n)) if {let(_,low,high)=quantity::pt_kind();(low..=high).contains(&i128::from(*n))}=>format!("{values}.with_loyalty({n})"),
            E::SetLoyalty(n)=>planned("copy exception SetLoyalty",format!("{values}.with_loyalty({})",self.copy_size(n)?)),
            E::RemoveManaCost=>format!("{values}.costless()"),
            E::AddAbilities(abilities)=>return self.copy_abilities(values,abilities),
            E::AddCurrentAbility if self.stack_item=>format!("{values}.with_ability(item.ability)"),
            E::AddCurrentAbility|E::AddChosenAbilities=>return Err(gap("copy exception AddAbilityVariable")),
            E::RemoveCurrentAbility=>planned("copy exception RemoveThisAbility",format!("{values}.without_ability(item.ability)")),
            E::RemoveAbilities(pattern)=>planned("copy exception LosesAbility",format!("{values}.losing({})",self.copy_pattern(pattern)?)),
            E::AddAbilityIfAbsent{ability,..}=>{
                // Preserve the existing capability gap until the engine exposes
                // a predicate for matching an existing ability on a copy.
                self.granted_refs(ability)?;
                return Err(gap("copy exception AddAbilityIfItDoesntHaveAbility"));
            }
            E::IfPermanentSpell(exceptions)=>format!("{{ let plain = {values}; if cantrip_mechanics::targets::is_permanent_card(&q.chars(original)) {{ {} }} else {{ plain }} }}",self.excepted("plain".into(),exceptions)?),
            E::MergeTypeline=>return Err(gap("copy exception MergeTypeline")),
            E::AddAbilitiesFrom{..}=>return Err(gap("copy exception AddAbilityFromEachExiledHasable")),
        })
    }
    fn copy_subtypes(&self, values: String, types: &[crate::mtg_types::SubType]) -> String {
        types.iter().fold(values, |v, t| {
            format!(
                "{v}.with_subtype(q.words(asker.source).subtype(Subtype::{}))",
                name(t)
            )
        })
    }
    fn copy_size(&self, n: &Expr) -> Result<String> {
        let (pt, _, _) = quantity::pt_kind();
        Ok(format!(
            "({}).clamp({pt}::MIN.into(), {pt}::MAX.into()) as {pt}",
            self.number(n)?.code
        ))
    }
    fn copy_abilities(&mut self, values: String, abilities: &[Ability]) -> Result<String> {
        if let [ability] = abilities {
            match ability {
                Ability::Keyword(k) if keyword(k).is_ok() => {
                    return Ok(format!("{values}.with_keyword({})", keyword(k)?))
                }
                Ability::DeckRule(rule)
                    if !matches!(
                        rule,
                        DeckRule::Banned
                            | DeckRule::RemoveUnlessPlayingForAnte
                            | DeckRule::Partner(PartnerKind::With(_))
                    ) =>
                {
                    return Ok(values)
                }
                ability if lane_j::granted_keyword(ability).is_some() => {
                    return Ok(format!(
                        "{values}.with_keyword({})",
                        lane_j::granted_keyword(ability).unwrap()
                    ))
                }
                ability if lane_n::toxic_of(ability).is_some() => {
                    return Err(gap("toxic as a copy exception"))
                }
                ability => {
                    if let Ok(index) = self.grant(ability) {
                        return Ok(format!(
                            "{values}.with_ability(AbilityRef::delayed({}, {index}))",
                            self.source_card()
                        ));
                    }
                }
            }
        }
        let mut code = values;
        for ability in abilities {
            let keyword = match ability {
                Ability::Keyword(k)
                | Ability::Static {
                    kind:
                        StaticKind::CharacteristicDefining {
                            keyword: Some(k), ..
                        },
                    ..
                } => keyword(k).ok(),
                _ => None,
            };
            if let Some(keyword) = keyword {
                code = format!("{code}.with_keyword({keyword})");
                continue;
            }
            if lane_n::toxic_of(ability).is_some() {
                return Err(gap("toxic as a copy exception"));
            }
            let refs = self.granted_refs(ability).or_else(|failed| {
                let Ability::Mechanic {
                    mechanic,
                    amount: Some(amount),
                    ..
                } = ability
                else {
                    return Err(failed);
                };
                Ok(vec![format!(
                    "AbilityRef::mechanic(Mechanic::{mechanic:?}, {})",
                    self.fetch(self.number(amount)?.amount(u32::MAX.into(), "u32")?)
                )])
            })?;
            for r in refs {
                code = format!("{code}.with_ability({r})");
            }
        }
        Ok(code)
    }
    fn copy_pattern(&mut self, pattern: &AbilityPattern) -> Result<String> {
        Ok(match pattern {
            AbilityPattern::Keyword(keyword)
                if serde_json::to_value(keyword)
                    .map_err(|_| gap("copy ability pattern"))?
                    .get("args")
                    .is_none() =>
            {
                format!("Lost::{}", form(keyword))
            }
            AbilityPattern::Mechanic(mechanic) => format!("Lost::{mechanic:?}"),
            AbilityPattern::KeywordFamily(family) => format!("Lost::Any{family:?}"),
            AbilityPattern::Category(kind) => format!(
                "Lost::{}",
                match kind {
                    AbilityCategory::Activated => "ActivatedAbility",
                    AbilityCategory::Triggered => "TriggeredAbility",
                    AbilityCategory::Loyalty => "LoyaltyAbility",
                    AbilityCategory::Mana => "ManaAbility",
                    AbilityCategory::NonMana => "NonManaAbility",
                }
            ),
            AbilityPattern::Current => "Lost::ThisAbility".into(),
            AbilityPattern::OtherThanCurrent => "Lost::OtherThanThisAbility".into(),
            AbilityPattern::Sticker => "Lost::StickerAbility".into(),
            AbilityPattern::AbilitySticker => "Lost::AbilityStickerAbility".into(),
            AbilityPattern::HasTapInCost => "Lost::HasTapSelfInCost".into(),
            AbilityPattern::Exhaust => "Lost::ExhaustAbility".into(),
            AbilityPattern::AnyProtectionFromColor => "Lost::AnyProtectionFromColor".into(),
            AbilityPattern::ProtectionFromColor(color) => {
                format!("Lost::ProtectionFromColor(Color::{color:?})")
            }
            AbilityPattern::Chosen => "Lost::TheChosenAbility".into(),
            AbilityPattern::Eachable => "Lost::EachableAbility".into(),
            AbilityPattern::All(patterns) => format!(
                "Lost::And(&[{}])",
                patterns
                    .iter()
                    .map(|p| self.copy_pattern(p))
                    .collect::<Result<Vec<_>>>()?
                    .join(", ")
            ),
            _ => return Err(gap(format!("copy ability pattern {}", form(pattern)))),
        })
    }
    pub(super) fn copy_values(
        &mut self,
        object: &str,
        exceptions: &[CopyException],
    ) -> Result<String> {
        let values = self.excepted("q.copiable_values(original)".into(), exceptions)?;
        Ok(self.fetch(format!("{object}.map(|original: ObjectId| {values})")))
    }
    /// Planned callers receive a typed transformation, not source enum words.
    pub(super) fn copy_callback(&mut self, exceptions: &[CopyException]) -> Result<String> {
        let values = self.excepted("values".into(), exceptions)?;
        Ok(format!(
            "|q: &dyn Query, asker: &Asker, original: ObjectId, values: Copiable| {values}"
        ))
    }
}
