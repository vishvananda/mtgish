// Shared set qualifications for search/resolution choices and strict costs.
impl Context {
    fn group_constraints(&self, value: &source::GroupFilter) -> Result<Vec<SelectionConstraint>> {
        use source::GroupFilter as S;
        Ok(vec![match value {
            S::DifferentNames => SelectionConstraint::DifferentNames,
            S::DifferentManaValues => SelectionConstraint::DifferentProperty(Property::ManaValue),
            S::DifferentPowers => SelectionConstraint::DifferentProperty(Property::Power),
            S::ShareACreatureType => {
                SelectionConstraint::CommonProperty(DistinctProperty::CreatureTypes)
            }
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
            _ => return Err(unsupported(value)),
        }])
    }
}
