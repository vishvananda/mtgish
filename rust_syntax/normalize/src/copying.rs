//! Source/legacy boundary for copy exceptions. Backends consume `CopyException`
//! directly; source family/form strings never escape into its typed payloads.
use crate::ir::*;
use serde::de::DeserializeOwned;
use serde_json::json;

type Result<T> = std::result::Result<T, String>;

/// Retained typed subtrees, relative to the source operand list and output
/// exception list. A migration composes these moves with nested migrations;
/// compiler numbers are never used to guess an origin.
#[derive(Default)]
pub(crate) struct Conversion {
    pub exceptions: Vec<CopyException>,
    pub moves: Vec<(String, String)>,
    work: usize,
}

pub fn from_source(operands: &[Operand]) -> Result<Vec<CopyException>> {
    Ok(convert(operands)?.exceptions)
}
pub(crate) fn convert(operands: &[Operand]) -> Result<Conversion> {
    let mut c = Conversion::default();
    for (i, operand) in operands.iter().enumerate() {
        c.exception(operand, &format!("/{i}"), "", 0)?;
    }
    Ok(c)
}

fn term(operand: &Operand) -> Result<(&str, &str, &[Operand])> {
    match operand {
        Operand::Term {
            family,
            form,
            operands,
        } => Ok((family, form, operands)),
        Operand::Vocabulary { family, form } => Ok((family, form, &[])),
        _ => Err("copy exception requires a reviewed descriptive source form".into()),
    }
}
fn word<T: DeserializeOwned>(operand: &Operand, family: &str) -> Result<T> {
    let Operand::Vocabulary {
        family: given,
        form,
    } = operand
    else {
        return Err(format!("copy exception requires {family} vocabulary"));
    };
    if given != family {
        return Err(format!("expected {family}, found {given}"));
    }
    serde_json::from_value(json!(form)).map_err(|e| e.to_string())
}
fn words<T: DeserializeOwned>(operands: &[Operand], family: &str) -> Result<Vec<T>> {
    operands.iter().map(|o| word(o, family)).collect()
}
fn child(path: &str, i: usize) -> String {
    format!("{path}/args/operands/{i}")
}

/// Reviewed source rows must not bypass typed copy fields if a new or malformed
/// payload cannot be reduced. Total lowering can retain them as explicitly
/// unreduced source instead of claiming a complete generic copy implementation.
pub(crate) fn requires_typed_payload(row: &str) -> bool {
    matches!(
        row.split_once("::"),
        Some((
            "Action",
            "CopySpellWithModifiers"
                | "CopySpellAndMayChooseNewTargetsWithEffects"
                | "PutExiledCardOnStackAsCopyOfSpell"
        )) | Some((
            "LayerEffect" | "StaticLayerEffect",
            "IsACopyOfExiledCard"
                | "IsACopyOfLibraryCard"
                | "IsACopyOfGraveyardCard"
                | "IsACopyOfPermanent"
                | "IsACopyOfPermanentSpell"
                | "IsACopyOfTheRevealedCard"
                | "IsACopyOf_TheObjectChosenToCopy"
        )) | Some((
            "ReplacementActionWouldEnter",
            "EnterAsACopyOfAPermanent"
                | "EnterAsACopyOfPermanent"
                | "EnterAsACopyOfACardInAPlayersGraveyard"
                | "EnterAsACopyOfACardInExile"
                | "EnterAsCopyOfExiled"
                | "EnterAsACopyOfAPermanentUntil"
        ))
    )
}

impl Conversion {
    fn charge(&mut self, depth: usize) -> Result<()> {
        self.work += 1;
        if depth > 128 || self.work > 65_536 {
            return Err("copy exception conversion limit exceeded".into());
        }
        Ok(())
    }
    fn keep(&mut self, source: String, target: String) {
        self.moves.push((source, target));
    }
    fn number(&mut self, operand: &Operand, source: &str, target: &str) -> Result<Expr> {
        match operand {
            Operand::Integer(n) => Ok(Expr::Constant(*n)),
            Operand::Number(n) => {
                self.keep(format!("{source}/args"), target.into());
                Ok(n.clone())
            }
            _ => Err("copy characteristic needs a numeric expression".into()),
        }
    }
    fn colors(&mut self, operand: &Operand, source: &str, target: &str) -> Result<CopyColors> {
        let (family, form, args) = term(operand)?;
        if family != "SettableColor" {
            return Err("copy color requires SettableColor source vocabulary".into());
        }
        Ok(match (form, args) {
            ("Colorless", []) => CopyColors::Listed(vec![]),
            ("AllColors", []) => CopyColors::Listed(vec![
                ManaColor::White,
                ManaColor::Blue,
                ManaColor::Black,
                ManaColor::Red,
                ManaColor::Green,
            ]),
            ("SimpleColorList", colors) => CopyColors::Listed(words(colors, "SimpleColor")?),
            ("TheChosenColor", []) => CopyColors::ChosenColor,
            ("TheChosenColors", []) => CopyColors::ChosenColors,
            ("TheManaColorChosenThisWay", [Operand::Result(id)]) => {
                self.keep(
                    format!("{}/args", child(source, 0)),
                    format!("{target}/args"),
                );
                CopyColors::ManaFrom(id.clone())
            }
            _ => return Err(format!("unsupported or unbound copy color {form}")),
        })
    }
    fn pattern(
        &mut self,
        operand: &Operand,
        source: &str,
        target: &str,
        depth: usize,
    ) -> Result<AbilityPattern> {
        self.charge(depth)?;
        if let Operand::Ability(ability) = operand {
            self.keep(format!("{source}/args"), format!("{target}/args"));
            return Ok(AbilityPattern::Rule(ability.clone()));
        }
        let (family, form, args) = term(operand)?;
        if family != "CheckHasable" {
            return Err("ability pattern requires CheckHasable source vocabulary".into());
        }
        Ok(match (form, args) {
            ("And", list) => AbilityPattern::All(
                list.iter()
                    .enumerate()
                    .map(|(i, o)| {
                        self.pattern(
                            o,
                            &child(source, i),
                            &format!("{target}/args/{i}"),
                            depth + 1,
                        )
                    })
                    .collect::<Result<_>>()?,
            ),
            ("ThisAbility", []) => AbilityPattern::Current,
            ("OtherThanThisAbility", []) => AbilityPattern::OtherThanCurrent,
            ("StickerAbility", []) => AbilityPattern::Sticker,
            ("AbilityStickerAbility", []) => AbilityPattern::AbilitySticker,
            ("HasTapSelfInCost", []) => AbilityPattern::HasTapInCost,
            ("ExhaustAbility", []) => AbilityPattern::Exhaust,
            ("TheChosenAbility", []) => AbilityPattern::Chosen,
            ("EachableAbility", []) => AbilityPattern::Eachable,
            ("ActivatedAbility", []) => AbilityPattern::Category(AbilityCategory::Activated),
            ("TriggeredAbility", []) => AbilityPattern::Category(AbilityCategory::Triggered),
            ("LoyaltyAbility", []) => AbilityPattern::Category(AbilityCategory::Loyalty),
            ("ManaAbility", []) => AbilityPattern::Category(AbilityCategory::Mana),
            ("NonManaAbility", []) => AbilityPattern::Category(AbilityCategory::NonMana),
            ("AnyProtectionFromColor", []) => AbilityPattern::AnyProtectionFromColor,
            ("ProtectionFromColor", [color]) => {
                AbilityPattern::ProtectionFromColor(word(color, "Color")?)
            }
            (
                "Enchant" | "Landwalk",
                [Operand::Objects(ObjectSet::Zone {
                    scope: ZoneScope::Shared(EventZone::Battlefield),
                    kind: ObjectKind::Permanent,
                    filter,
                })],
            ) => {
                self.keep(
                    format!("{}/args/args/filter", child(source, 0)),
                    format!("{target}/args"),
                );
                if form == "Enchant" {
                    AbilityPattern::Enchant(filter.clone())
                } else {
                    AbilityPattern::Landwalk(filter.clone())
                }
            }
            (name, []) => {
                if let Some(name) = name.strip_prefix("Any") {
                    AbilityPattern::KeywordFamily(
                        serde_json::from_value(json!(name))
                            .map_err(|_| format!("unsupported ability family {name}"))?,
                    )
                } else if let Ok(keyword) = serde_json::from_value(json!({"kind":name})) {
                    AbilityPattern::Keyword(keyword)
                } else if let Ok(mechanic) = serde_json::from_value(json!(name)) {
                    AbilityPattern::Mechanic(mechanic)
                } else {
                    AbilityPattern::KeywordFamily(
                        serde_json::from_value(json!(name))
                            .map_err(|_| format!("unsupported ability pattern {name}"))?,
                    )
                }
            }
            _ => return Err(format!("unsupported ability pattern {form}")),
        })
    }
    fn pt(&mut self, operand: &Operand, source: &str, target: &str) -> Result<(Expr, Expr)> {
        let size = crate::source_values::power_toughness(operand, source, target)?;
        self.moves.extend(size.moves);
        Ok((size.power, size.toughness))
    }
    fn exception(
        &mut self,
        operand: &Operand,
        source: &str,
        target: &str,
        depth: usize,
    ) -> Result<()> {
        self.charge(depth)?;
        let (family, form, args) = term(operand)?;
        if matches!(
            (family, form),
            ("TokenCopyEffects", "NoTokenCopyEffects")
                | ("CopyEffects", "NoCopyEffects")
                | ("StaticCopyEffects", "NoStaticCopyEffects")
                | ("SpellCopyEffects", "NoSpellCopyEffects")
        ) && args.is_empty()
        {
            return Ok(());
        }
        if matches!(
            (family, form),
            ("TokenCopyEffects", "TokenCopyEffects")
                | ("CopyEffects", "CopyEffects")
                | ("StaticCopyEffects", "StaticCopyEffects")
        ) {
            let expected = match family {
                "TokenCopyEffects" => "TokenCopyEffect",
                "CopyEffects" => "CopyEffect",
                "StaticCopyEffects" => "StaticCopyEffect",
                _ => unreachable!(),
            };
            for (i, o) in args.iter().enumerate() {
                if term(o)?.0 != expected {
                    return Err(format!("{family} requires {expected} children"));
                }
                self.exception(o, &child(source, i), target, depth + 1)?;
            }
            return Ok(());
        }
        let known = match family {
            "TokenCopyEffect" => matches!(
                form,
                "AddSupertypes"
                    | "RemoveSupertypes"
                    | "AddCardtypes"
                    | "SetCardtypes"
                    | "AddCreatureTypes"
                    | "AddArtifactTypes"
                    | "SetArtifactTypes"
                    | "SetCreatureTypes"
                    | "AddAbility"
                    | "LosesAbility"
                    | "RemoveThisAbility"
                    | "AddAbilityFromEachExiledHasable"
                    | "AddColor"
                    | "SetColor"
                    | "SetName"
                    | "SetPT"
                    | "SetLoyalty"
            ),
            "CopyEffect" => matches!(
                form,
                "AddSupertypes"
                    | "RemoveSupertypes"
                    | "AddCardtypes"
                    | "SetCardtypes"
                    | "AddCreatureTypes"
                    | "AddArtifactTypes"
                    | "SetArtifactTypes"
                    | "SetCreatureTypes"
                    | "AddLandTypes"
                    | "AddAbilityVariable"
                    | "AddAbility"
                    | "AddAbilityIfItDoesntHaveAbility"
                    | "KeepColor"
                    | "AddColor"
                    | "SetColor"
                    | "KeepName"
                    | "SetName"
                    | "KeepPT"
                    | "SetPT"
                    | "HasNoManaCost"
            ),
            "StaticCopyEffect" => matches!(
                form,
                "AddSupertypes"
                    | "RemoveSupertypes"
                    | "AddCardtypes"
                    | "SetCardtypes"
                    | "AddCreatureTypes"
                    | "AddArtifactTypes"
                    | "SetArtifactTypes"
                    | "AddLandTypes"
                    | "MergeTypeline"
                    | "HasNoManaCost"
                    | "KeepColor"
                    | "AddColor"
                    | "SetColor"
                    | "KeepName"
                    | "SetName"
                    | "AddAbility"
                    | "AddAbilityIfItDoesntHaveAbility"
                    | "KeepPT"
                    | "SetPT"
            ),
            "SpellEffect" => matches!(
                form,
                "IfPermanentSpell"
                    | "AddColor"
                    | "SetColor"
                    | "AddCreatureType"
                    | "AddAbility"
                    | "SetPT"
                    | "AddCardtype"
                    | "RemoveSupertypes"
            ),
            _ => return Err(format!("{family} is not a copy-exception family")),
        };
        if !known {
            return Err(format!("{family}::{form} is not a reviewed copy exception"));
        }
        let at = format!("{target}/{}", self.exceptions.len());
        let exception = match (form, args) {
            ("AddSupertypes", list) => CopyException::AddSupertypes(words(list, "SuperType")?),
            ("RemoveSupertypes", list) => {
                CopyException::RemoveSupertypes(words(list, "SuperType")?)
            }
            ("AddCardtypes" | "AddCardtype", list) => {
                CopyException::AddCardTypes(words(list, "CardType")?)
            }
            ("SetCardtypes", list) => CopyException::SetCardTypes(words(list, "CardType")?),
            ("AddCreatureTypes" | "AddCreatureType", list) => {
                CopyException::AddCreatureTypes(words(list, "CreatureType")?)
            }
            ("SetCreatureTypes", list) => CopyException::SetTypes {
                card_types: vec![crate::mtg_types::CardType::Creature],
                subtypes: words::<crate::mtg_types::CreatureType>(list, "CreatureType")?
                    .iter()
                    .map(crate::subtype::creature_type)
                    .collect(),
            },
            ("AddArtifactTypes", list) => {
                CopyException::AddArtifactTypes(words(list, "ArtifactType")?)
            }
            ("SetArtifactTypes", list) => {
                CopyException::SetArtifactTypes(words(list, "ArtifactType")?)
            }
            ("AddLandTypes", list) => CopyException::AddLandTypes(words(list, "LandType")?),
            ("MergeTypeline", []) => CopyException::MergeTypeline,
            ("SetColor" | "AddColor", [color]) => CopyException::Colors {
                mode: if form == "SetColor" {
                    ChangeMode::Set
                } else {
                    ChangeMode::Add
                },
                colors: self.colors(color, &child(source, 0), &format!("{at}/args/colors"))?,
            },
            ("KeepColor", []) => CopyException::KeepColors,
            ("SetName", [Operand::Text(name)]) => CopyException::SetName(name.clone()),
            ("KeepName", []) => CopyException::KeepName,
            ("SetPT", [pt]) => {
                let (power, toughness) = self.pt(pt, &child(source, 0), &format!("{at}/args"))?;
                CopyException::SetPowerToughness { power, toughness }
            }
            ("KeepPT", []) => CopyException::KeepPowerToughness,
            ("SetLoyalty", [n]) => CopyException::SetLoyalty(self.number(
                n,
                &child(source, 0),
                &format!("{at}/args"),
            )?),
            ("HasNoManaCost", []) => CopyException::RemoveManaCost,
            ("AddAbility", list) => CopyException::AddAbilities(
                list.iter()
                    .enumerate()
                    .map(|(i, o)| {
                        let Operand::Ability(a) = o else {
                            return Err("copy-granted rule is not a typed ability".into());
                        };
                        self.keep(
                            format!("{}/args", child(source, i)),
                            format!("{at}/args/{i}"),
                        );
                        Ok((**a).clone())
                    })
                    .collect::<Result<_>>()?,
            ),
            ("AddAbilityVariable", [Operand::Vocabulary { family, form }])
                if family == "AbilityVariable" && form == "ThisAbility" =>
            {
                CopyException::AddCurrentAbility
            }
            ("AddAbilityVariable", [Operand::Vocabulary { family, form }])
                if family == "AbilityVariable"
                    && matches!(form.as_str(), "TheChosenAbility" | "TheChosenAbilities") =>
            {
                CopyException::AddChosenAbilities
            }
            ("AddAbilityIfItDoesntHaveAbility", [Operand::Ability(ability), kind]) => {
                self.keep(
                    format!("{}/args", child(source, 0)),
                    format!("{at}/args/ability"),
                );
                CopyException::AddAbilityIfAbsent {
                    ability: ability.clone(),
                    matching: self.pattern(
                        kind,
                        &child(source, 1),
                        &format!("{at}/args/matching"),
                        depth + 1,
                    )?,
                }
            }
            ("LosesAbility", [kind]) => CopyException::RemoveAbilities(self.pattern(
                kind,
                &child(source, 0),
                &format!("{at}/args"),
                depth + 1,
            )?),
            ("RemoveThisAbility", []) => CopyException::RemoveCurrentAbility,
            (
                "AddAbilityFromEachExiledHasable",
                [Operand::Objects(objects), Operand::List(kinds)],
            ) => {
                self.keep(
                    format!("{}/args", child(source, 0)),
                    format!("{at}/args/objects"),
                );
                CopyException::AddAbilitiesFrom {
                    objects: objects.clone(),
                    kinds: kinds
                        .iter()
                        .enumerate()
                        .map(|(i, o)| {
                            self.pattern(
                                o,
                                &format!("{}/args/{i}", child(source, 1)),
                                &format!("{at}/args/kinds/{i}"),
                                depth + 1,
                            )
                        })
                        .collect::<Result<_>>()?,
                }
            }
            ("IfPermanentSpell", list) if family == "SpellEffect" => {
                // A private output vector makes its indices local to the guard.
                let outer = std::mem::take(&mut self.exceptions);
                for (i, o) in list.iter().enumerate() {
                    self.exception(o, &child(source, i), &format!("{at}/args"), depth + 1)?;
                }
                let inner = std::mem::replace(&mut self.exceptions, outer);
                CopyException::IfPermanentSpell(inner)
            }
            _ => {
                return Err(format!(
                    "{family}::{form} is not a typed copy exception with these operands"
                ))
            }
        };
        self.exceptions.push(exception);
        Ok(())
    }
}
