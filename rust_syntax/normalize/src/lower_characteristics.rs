fn symbol_color(color: &source::Color) -> Result<ManaColor> {
    Ok(match color {
        source::Color::White => ManaColor::White,
        source::Color::Blue => ManaColor::Blue,
        source::Color::Black => ManaColor::Black,
        source::Color::Red => ManaColor::Red,
        source::Color::Green => ManaColor::Green,
        source::Color::TheChosenColor => ManaColor::Chosen,
        _ => return Err(unsupported(color)),
    })
}

impl Context {
    fn fixed_colors(&self, color: &source::SettableColor) -> Result<Vec<ManaColor>> {
        use source::SettableColor as S;
        Ok(match color {
            S::Colorless | S::Devoid => vec![],
            S::AllColors => vec![
                ManaColor::White,
                ManaColor::Blue,
                ManaColor::Black,
                ManaColor::Red,
                ManaColor::Green,
            ],
            S::SimpleColorList(colors) => colors
                .iter()
                .map(|c| match c {
                    source::SimpleColor::White => ManaColor::White,
                    source::SimpleColor::Blue => ManaColor::Blue,
                    source::SimpleColor::Black => ManaColor::Black,
                    source::SimpleColor::Red => ManaColor::Red,
                    source::SimpleColor::Green => ManaColor::Green,
                })
                .collect(),
            _ => return Err(unsupported(color)),
        })
    }
    fn characteristic_definition(&self, value: &source::Rule) -> Result<Ability> {
        use source::Rule as S;
        let mut keyword = None;
        let changes = match value {
            S::CDA_Power(power) => vec![Change::SetPower(self.number(power)?)],
            S::CDA_Toughness(toughness) => vec![Change::SetToughness(self.number(toughness)?)],
            S::CDA_PowerToughness(power, toughness) => vec![Change::SetBasePowerToughness {
                power: self.number(power)?,
                toughness: self.number(toughness)?,
            }],
            S::CDA_Color(color) | S::CDA_ColorButNotColorIdentity(color) => {
                if matches!(color, source::SettableColor::Devoid) {
                    keyword = Some(Keyword::Devoid);
                }
                vec![Change::Colors {
                    mode: ChangeMode::Set,
                    colors: self.fixed_colors(color)?,
                }]
            }
            S::CDA_Types(source::CDA_Types::HasAllNonbasicLandTypes) => {
                vec![Change::Primitive {
                    name: "CDA_Types::HasAllNonbasicLandTypes".into(),
                    operands: vec![],
                }]
            }
            S::CDA_Types(types) => vec![match types {
                source::CDA_Types::Changeling | source::CDA_Types::HasAllCreatureTypes => {
                    if matches!(types, source::CDA_Types::Changeling) {
                        keyword = Some(Keyword::Changeling);
                    }
                    Change::CreatureTypes {
                        mode: ChangeMode::Set,
                        types: CreatureTypeSet::All,
                    }
                }
                source::CDA_Types::AddCreatureTypes(types) => Change::CreatureTypes {
                    mode: ChangeMode::Add,
                    types: CreatureTypeSet::Listed(types.clone()),
                },
                source::CDA_Types::AddCreatureTypeVariable(variable) => self
                    .primitive_change(&source::LayerEffect::AddCreatureTypeVariable(
                        variable.clone(),
                    ))
                    .ok_or_else(|| unsupported(types))?,
                _ => return Err(unsupported(types)),
            }],
            _ => return Err(unsupported(value)),
        };
        Ok(Ability::Static {
            kind: StaticKind::CharacteristicDefining {
                keyword,
                contributes_to_color_identity: !matches!(value, S::CDA_ColorButNotColorIdentity(_)),
            },
            affected: EffectSubjects::Objects(ObjectSet::One(ObjectRef::Source)),
            changes,
        })
    }
    fn characteristic_number(&self, value: &source::GameNumber) -> Result<Expr> {
        use source::GameNumber as S;
        Ok(match value {
            S::PlayerDevotionTo(player, color) => Expr::CountManaSymbols {
                objects: Box::new(ObjectSet::Battlefield(ObjectFilter::ControlledBy(
                    PlayerSet::One(self.player(player)?),
                ))),
                colors: vec![symbol_color(color)?],
            },
            S::TheTotalNumberOfColorManaSymbolsInManaCostsOfTheCardsInHandRevealedThisWay(color)
            | S::TheTotalNumberOfColorManaSymbolsInManaCostsOfTheLibraryCardsRevealedThisWay(
                color,
            ) => Expr::CountManaSymbols {
                objects: Box::new(self.revealed_objects()?),
                colors: vec![symbol_color(color)?],
            },
            _ => return Err(unsupported(value)),
        })
    }
}
