impl Context {
    fn keyword(&self, value: &source::Rule) -> Result<Keyword> {
        use source::Rule as S;
        Ok(match value {
            S::Protection(quality) => Keyword::Protection(Box::new(self.protection(quality)?)),
            S::HexproofFrom(quality) => Keyword::HexproofFrom(Box::new(self.protection(quality)?)),
            S::Landwalk(lands) => Keyword::Landwalk(Box::new(self.filter(lands)?)),
            S::Ward(cost) => {
                let mut context = self.fresh();
                context.keyword_host = true;
                context.payer = Some(PlayerRef::WardPayer);
                Keyword::Ward(Box::new(context.cost(cost)?))
            }
            _ => keyword(value)?,
        })
    }

    fn protection(&self, value: &source::Protectable) -> Result<Protection> {
        use source::Protectable as S;
        let objects = |kind, filter| Protection::Objects { kind, filter };
        Ok(match value {
            // These disjunctions come from the grammar's separate printed
            // protection qualities, rather than from a merged object query.
            S::FromTypes(cards) if matches!(cards.as_ref(), source::Cards::Or(_)) => {
                let source::Cards::Or(cards) = cards.as_ref() else {
                    unreachable!()
                };
                let artifacts_and_each_color = matches!(
                    cards.as_slice(),
                    [
                        source::Cards::IsCardtype(source::CardType::Artifact),
                        source::Cards::IsColored
                    ]
                );
                let qualities = cards
                    .iter()
                    .map(|card| {
                        if artifacts_and_each_color && matches!(card, source::Cards::IsColored) {
                            self.protection(&S::FromColor(source::ProtectableColor::AnyColor))
                        } else {
                            self.protection(&S::FromTypes(Box::new(card.clone())))
                        }
                    })
                    .collect::<Result<Vec<_>>>()?;
                if qualities.len() < 2 {
                    return Err(invalid(
                        "protection alternatives require separate qualities",
                    ));
                }
                Protection::Each(qualities)
            }
            S::FromSpells(spells) if matches!(spells.as_ref(), source::Spells::Or(_)) => {
                let source::Spells::Or(spells) = spells.as_ref() else {
                    unreachable!()
                };
                let qualities = spells
                    .iter()
                    .map(|spell| self.protection(&S::FromSpells(Box::new(spell.clone()))))
                    .collect::<Result<Vec<_>>>()?;
                if qualities.len() < 2 {
                    return Err(invalid(
                        "protection alternatives require separate qualities",
                    ));
                }
                Protection::Each(qualities)
            }
            S::FromEverything => objects(ObjectKind::Any, ObjectFilter::Any),
            S::FromTypes(filter) => objects(ObjectKind::Any, self.card_filter(filter)?),
            S::FromPermanents(filter) => objects(ObjectKind::Permanent, self.filter(filter)?),
            S::FromSpells(filter) => objects(ObjectKind::Spell, self.spell_filter(filter)?),
            S::FromPlayers(players) => Protection::Players(self.players(players)?),
            S::FromManaValue(comparison) => {
                let test = self.number_test(comparison)?;
                objects(
                    ObjectKind::Any,
                    ObjectFilter::Property { property: Property::ManaValue, test },
                )
            }
            S::FromCardName(name) => objects(
                ObjectKind::Any,
                self.card_filter(&source::Cards::IsNamed(name.clone()))?,
            ),
            S::FromActivatedAndTriggeredAbilities => objects(
                ObjectKind::Any,
                ObjectFilter::AnyOf(vec![
                    ObjectFilter::OnStack(StackTrait::Activated),
                    ObjectFilter::OnStack(StackTrait::Triggered),
                ]),
            ),
            S::FromColor(colors) => {
                use source::ProtectableColor as C;
                let color = |color| objects(ObjectKind::Any, ObjectFilter::Color(color));
                match colors {
                    C::TheChosenColor => objects(
                        ObjectKind::Any,
                        self.color_filter(&source::Color::TheChosenColor)?,
                    ),
                    C::Colors(colors) => {
                        let mut qualities = colors
                            .iter()
                            .map(|c| self.color_filter(c).map(|f| objects(ObjectKind::Any, f)))
                            .collect::<Result<Vec<_>>>()?;
                        if qualities.is_empty() {
                            return Err(invalid("protection has no color qualities"));
                        }
                        if qualities.len() == 1 {
                            qualities.pop().unwrap()
                        } else {
                            Protection::Each(qualities)
                        }
                    }
                    C::AnyColor => Protection::Each(
                        [
                            ManaColor::White,
                            ManaColor::Blue,
                            ManaColor::Black,
                            ManaColor::Red,
                            ManaColor::Green,
                        ]
                        .into_iter()
                        .map(color)
                        .collect(),
                    ),
                    C::Colored => objects(
                        ObjectKind::Any,
                        ObjectFilter::Not(Box::new(ObjectFilter::Colorless)),
                    ),
                    C::ItsOwnColors => objects(
                        ObjectKind::Any,
                        ObjectFilter::SharesColorWith(ObjectRef::Source),
                    ),
                    C::ColorsOfPermanent(object) => objects(
                        ObjectKind::Any,
                        ObjectFilter::SharesColorWith(self.object(object)?),
                    ),
                    C::ColorsOfPermanents(permanents) => objects(
                        ObjectKind::Any,
                        ObjectFilter::SharesColorWithAny(Box::new(self.objects(permanents)?)),
                    ),
                    C::TheColorsOfSpell(spell) => objects(
                        ObjectKind::Any,
                        ObjectFilter::SharesColorWith(self.spell(spell)?),
                    ),
                    C::Monocolored | C::Multicolored => objects(
                        ObjectKind::Any,
                        ObjectFilter::Property { property: Property::ColorCount, test: NumberTest::Compare { comparison: if matches!(colors, C::Monocolored) {
                                Comparison::Equal
                            } else {
                                Comparison::Greater
                            }, value: Expr::Constant(1) } },
                    ),
                    _ => {
                        let (name, operands) = self
                            .primitive("ProtectableColor", colors)
                            .ok_or_else(|| unsupported(colors))?;
                        objects(
                            ObjectKind::Any,
                            crate::semantics::reduce_filter(name, operands),
                        )
                    }
                }
            }
            _ => return Err(unsupported(value)),
        })
    }
}
