impl Context {
    fn scoped_ability(&self, value: &source::Rule) -> Result<Ability> {
        use source::Rule as S;
        let (zones, condition, rule) = match value {
            S::FromHand(r) => (vec![EventZone::Hand], None, r),
            S::FromGraveyard(r) => (vec![EventZone::Graveyard], None, r),
            S::FromGraveyardIf(c, r) => (vec![EventZone::Graveyard], Some(self.condition(c)?), r),
            S::FromGraveyardOrBattlefield(r) => {
                (vec![EventZone::Graveyard, EventZone::Battlefield], None, r)
            }
            S::FromStack(r) => (vec![EventZone::Stack], None, r),
            S::FromStackIf(c, r) => (vec![EventZone::Stack], Some(self.condition(c)?), r),
            S::FromExile(r) => (vec![EventZone::Exile], None, r),
            S::FromExileIf(c, r) => (vec![EventZone::Exile], Some(self.condition(c)?), r),
            S::FromExileOrBattlefield(r) => {
                (vec![EventZone::Exile, EventZone::Battlefield], None, r)
            }
            S::FromCommandZone(r) => (vec![EventZone::Command], None, r),
            S::FromCommandZoneOrBattlefield(r) => {
                (vec![EventZone::Command, EventZone::Battlefield], None, r)
            }
            S::FromAnyZone(r) => (
                vec![
                    EventZone::Battlefield,
                    EventZone::Graveyard,
                    EventZone::Hand,
                    EventZone::Library,
                    EventZone::Stack,
                    EventZone::Exile,
                    EventZone::Command,
                    EventZone::Ante,
                ],
                None,
                r,
            ),
            _ => return Err(unsupported(value)),
        };
        with_source_scope(zones, condition, self.fresh().ability(rule)?)
    }
    fn cycling(&mut self, mut cost: Cost) -> Result<Ability> {
        cost.parts.push(CostPart::Discard {
            count: Expr::Constant(1),
            filter: ObjectFilter::SameObject(ObjectRef::Source),
            random: false,
        });
        let body = Program {
            instructions: vec![self.effect(Operation::Draw {
                players: PlayerSet::One(PlayerRef::AbilityController),
                amount: Expr::Constant(1),
            })],
        };
        Ok(Ability::Scoped {
            zones: vec![EventZone::Hand],
            condition: None,
            ability: Box::new(Ability::Activated {
                origin: ActivationOrigin::Cycling { search: None },
                cost,
                limits: ActivationLimits::unrestricted(),
                announcement: Announcement::empty(),
                body,
            }),
        })
    }
    fn stack_effect(
        &self,
        affected: ObjectFilter,
        effects: &[source::SpellEffect],
    ) -> Result<Ability> {
        let mut changes = Vec::new();
        for (i, effect) in effects.iter().enumerate() {
            use source::SpellEffect as S;
            changes.extend(match effect {
                S::CantBeCountered => {
                    vec![Change::Rule(RuleChange::Prohibit(ObjectAction::Counter))]
                }
                S::CantBeCopied => vec![Change::Rule(RuleChange::Prohibit(ObjectAction::Copy))],
                S::Lifelink => vec![Change::GrantKeyword(Keyword::Lifelink)],
                S::Deathtouch => vec![Change::GrantKeyword(Keyword::Deathtouch)],
                S::Wither => vec![Change::GrantKeyword(Keyword::Wither)],
                S::AddAbility(rules) => rules
                    .iter()
                    .map(|r| {
                        self.fresh()
                            .ability(r)
                            .map(|a| Change::GrantAbility(Box::new(a)))
                    })
                    .collect::<Result<_>>()?,
                _ => match self.primitive("SpellEffect", effect) {
                    Some((name, operands)) => crate::semantics::reduce_changes(name, operands),
                    None => {
                        let rule = as_family::<source::Rule>(effect, "Rule");
                        let granted = rule.and_then(|rule| self.fresh().ability(&rule).ok());
                        let granted = granted
                            .ok_or_else(|| unsupported(effect).at(format!("effects[{i}]")))?;
                        vec![Change::GrantAbility(Box::new(granted))]
                    }
                },
            });
        }
        Ok(Ability::Static {
            kind: StaticKind::Ordinary,
            affected: EffectSubjects::Objects(ObjectSet::Zone {
                scope: ZoneScope::Shared(EventZone::Stack),
                kind: ObjectKind::Spell,
                filter: affected,
            }),
            changes,
        })
    }
}

fn with_source_scope(
    mut zones: Vec<EventZone>,
    condition: Option<Condition>,
    ability: Ability,
) -> Result<Ability> {
    let (condition, ability) = match ability {
        Ability::Scoped {
            zones: inner,
            condition: other,
            ability,
        } => {
            zones.retain(|z| inner.contains(z));
            let condition = match (condition, other) {
                (Some(a), Some(b)) => Some(Condition::All(vec![a, b])),
                (a, b) => a.or(b),
            };
            (condition, *ability)
        }
        ability => (condition, ability),
    };
    if zones.is_empty() {
        return Err(invalid("source scopes have no common zone"));
    }
    Ok(Ability::Scoped {
        zones,
        condition,
        ability: Box::new(ability),
    })
}
