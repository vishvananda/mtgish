// Shared set qualifications for search/resolution choices and strict costs.
impl Context {
    fn group_constraints(&self, value: &source::GroupFilter) -> Result<Vec<SelectionConstraint>> {
        use source::GroupFilter as S;
        Ok(vec![match value {
            S::DifferentNames => SelectionConstraint::DifferentNames,
            S::SharesANameWithEachPermanent(objects) => {
                SelectionConstraint::NamedAfterOneEach(self.objects(objects)?)
            }
            S::DifferentManaValues => SelectionConstraint::DifferentProperty(Property::ManaValue),
            S::DifferentPowers => SelectionConstraint::DifferentProperty(Property::Power),
            S::ShareACreatureType | S::ShareACreatureTypeOfChoice => {
                SelectionConstraint::CommonProperty(DistinctProperty::CreatureTypes)
            }
            S::SharesNoCreatureTypes => {
                SelectionConstraint::NoCommonProperty(DistinctProperty::CreatureTypes)
            }
            S::EachBasicLandType => SelectionConstraint::OneForEach(
                [
                    source::SubType::Plains,
                    source::SubType::Island,
                    source::SubType::Swamp,
                    source::SubType::Mountain,
                    source::SubType::Forest,
                ]
                .map(ObjectFilter::Subtype)
                .to_vec(),
            ),
            S::ShareACardType => SelectionConstraint::CommonProperty(DistinctProperty::CardTypes),
            S::ShareAColor => SelectionConstraint::CommonProperty(DistinctProperty::Colors),
            S::SameNames => SelectionConstraint::CommonProperty(DistinctProperty::Names),
            S::ShareALandType => SelectionConstraint::CommonProperty(DistinctProperty::LandTypes),
            S::ShareAGraveyard => SelectionConstraint::CommonProperty(DistinctProperty::Owners),
            S::ControlledByTheSamePlayer => {
                SelectionConstraint::CommonProperty(DistinctProperty::Controllers)
            }
            S::SameToughness => SelectionConstraint::CommonProperty(DistinctProperty::Toughness),
            S::ControlledByDifferentPlayers | S::DifferentControllers => {
                SelectionConstraint::DifferentControllers
            }
            S::TotalPowerIs(c) | S::TotalManaValueIs(c) => {
                let (comparison, amount) = self.comparison(c)?;
                SelectionConstraint::TotalProperty {
                    property: if matches!(value, S::TotalPowerIs(_)) {
                        Property::Power
                    } else {
                        Property::ManaValue
                    },
                    comparison,
                    amount,
                }
            }
            S::ANumberOfDifferentCardTypes(c) => {
                let (comparison, amount) = self.comparison(c)?;
                SelectionConstraint::DistinctCount {
                    property: DistinctProperty::CardTypes,
                    comparison,
                    amount,
                }
            }
            S::HasNumberSymbolsAmongCosts(c, symbol) => {
                use source::ManaProduceSymbol as M;
                let (comparison, amount) = self.comparison(c)?;
                SelectionConstraint::TotalManaSymbols {
                    color: match symbol {
                        M::ManaProduceW => ManaColor::White,
                        M::ManaProduceU => ManaColor::Blue,
                        M::ManaProduceB => ManaColor::Black,
                        M::ManaProduceR => ManaColor::Red,
                        M::ManaProduceG => ManaColor::Green,
                        M::ManaProduceC => return Err(unsupported(value)),
                    },
                    comparison,
                    amount,
                }
            }
            _ => return Err(unsupported(value)),
        }])
    }
}
