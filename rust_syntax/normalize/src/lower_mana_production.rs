// Typed bridge; these are production expressions, never sentence-shaped actions.
impl Context {
    fn mana_production(&self, value: &source::ManaProduce) -> Result<ManaProduction> {
        self.reduced_mana_production(value).or_else(|error| {
            let (name, operands) = self.rescue(error, || self.primitive("ManaProduce", value))?;
            if matches!(
                name.as_str(),
                "ManaProduce::ManaOfTheChosenColor" | "ManaProduce::ManaOfAChosenColor"
            ) {
                return Ok(ManaProduction::Amount {
                    color: ManaColor::Chosen,
                    amount: Expr::Constant(1),
                });
            }
            Ok(match (name.as_str(), operands.as_slice()) {
                ("ManaProduce::Trigger_AnyManaTypeProduced", []) => {
                    ManaProduction::SameTypeAsProduced
                }
                ("ManaProduce::AnyManaTypeAPermanentCouldProduce", [Operand::Objects(objects)]) => {
                    ManaProduction::CouldProduce {
                        objects: objects.clone(),
                        colors_only: false,
                    }
                }
                ("ManaProduce::AnyManaColorAPermanentCouldProduce", [Operand::Objects(objects)]) => {
                    ManaProduction::CouldProduce {
                        objects: objects.clone(),
                        colors_only: true,
                    }
                }
                ("ManaProduce::AnyManaColorInCommanderColorIdentity", []) => {
                    ManaProduction::CommanderIdentity
                }
                _ => ManaProduction::Primitive { name, operands },
            })
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
            S::IsNonCardtype(ty) => ObjectFilter::Not(Box::new(ObjectFilter::CardType(ty.clone()))),
            S::IsSupertype(ty) => ObjectFilter::Supertype(ty.clone()),
            S::IsColor(color) => self.color_filter(color)?,
            S::IsNonColor(color) => ObjectFilter::Not(Box::new(self.color_filter(color)?)),
            S::IsColorless => ObjectFilter::Colorless,
            S::ControlledByAPlayer(players) => ObjectFilter::ControlledBy(self.players(players)?),
            S::IsNamed(name) => self.card_filter(&source::Cards::IsNamed(name.clone()))?,
            S::IsNotACommander => self.filter_inner(&source::Permanents::IsNotACommander)?,
            S::IsAnOutlaw => self.filter_inner(&source::Permanents::IsAnOutlaw)?,
            S::IsCreatureTypeVariable(ty) => {
                self.filter_inner(&source::Permanents::IsCreatureTypeVariable(ty.clone()))?
            }
        })
    }
    fn mana_use(&self, value: &source::ManaUseModifier) -> Result<ManaUse> {
        self.reduced_mana_use(value).or_else(|error| {
            self.rescue(error, || {
                self.primitive("ManaUseModifier", value)
                    .map(|(name, operands)| match (name.as_str(), operands.as_slice()) {
                        (
                            "ManaUseModifier::DontLoseAsStepsAndPhasesEnd",
                            [Operand::Duration(until)],
                        ) => ManaUse::Persists(until.clone()),
                        _ => crate::semantics::reduce_mana_use(&name, &operands)
                            .unwrap_or(ManaUse::Primitive { name, operands }),
                    })
            })
        })
    }
    fn reduced_mana_use(&self, value: &source::ManaUseModifier) -> Result<ManaUse> {
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
            S::CreatePermanentSpellLayerEffect(until, spells, changes) => ManaUse::SpentOn {
                spells: self.spell_filter(spells)?,
                changes: self.changes(changes)?,
                // The change is on one object, so leaving the battlefield ends it.
                until: match until {
                    source::Expiration::UntilItLeavesTheBattlefield => Duration::Indefinite,
                    until => self.duration(until)?,
                },
            },
            S::And(v) => ManaUse::All(v.iter().map(|v| self.mana_use(v)).collect::<Result<_>>()?),
            S::Or(v) => ManaUse::Either(v.iter().map(|v| self.mana_use(v)).collect::<Result<_>>()?),
            _ => return Err(unsupported(value)),
        })
    }
}
