impl Context {
    fn permanent_activations(
        &self,
        value: &source::ActivatedAbilities,
    ) -> Result<(Option<ObjectFilter>, ActivationFilter)> {
        use source::ActivatedAbilities as S;
        Ok(match value {
            S::AbilityOfAPermanent(p) => (Some(self.filter(p)?), ActivationFilter::Any),
            S::AbilityOfPermanent(p) => (
                Some(ObjectFilter::SameObject(self.object(p)?)),
                ActivationFilter::Any,
            ),
            S::And(items) => {
                let mut sources = Vec::new();
                let mut filters = Vec::new();
                for item in items {
                    let (source, filter) = self.permanent_activations(item)?;
                    if let Some(source) = source {
                        sources.push(source);
                    }
                    if !matches!(filter, ActivationFilter::Any) {
                        filters.push(filter);
                    }
                }
                let source = match sources.len() {
                    0 => None,
                    1 => sources.pop(),
                    _ => Some(ObjectFilter::All(sources)),
                };
                let filter = match filters.len() {
                    0 => ActivationFilter::Any,
                    1 => filters.pop().unwrap(),
                    _ => ActivationFilter::All(filters),
                };
                (source, filter)
            }
            _ => (None, self.activation_filter(value)?),
        })
    }
    fn activation_filter(&self, value: &source::ActivatedAbilities) -> Result<ActivationFilter> {
        use source::ActivatedAbilities as S;
        Ok(match value {
            S::AnyAbility => ActivationFilter::Any,
            S::ManaAbility => ActivationFilter::Mana,
            S::LoyaltyAbility => ActivationFilter::Is(StackTrait::Loyalty),
            S::NonLoyaltyAbility => {
                ActivationFilter::Not(Box::new(ActivationFilter::Is(StackTrait::Loyalty)))
            }
            S::HasTapSelfInCost => ActivationFilter::Is(StackTrait::TapSymbolInCost),
            S::AbilityOfAPermanent(sources) => {
                ActivationFilter::Of(Box::new(self.filter(sources)?))
            }
            S::AbilityOfPermanent(source) => ActivationFilter::Of(Box::new(
                ObjectFilter::SameObject(self.object(source)?),
            )),
            S::AbilityOfASource(sources) => {
                ActivationFilter::Of(Box::new(self.mana_sources(sources)?))
            }
            S::NonManaAbility => ActivationFilter::Not(Box::new(ActivationFilter::Mana)),
            S::And(v) => ActivationFilter::All(
                v.iter()
                    .map(|a| self.activation_filter(a))
                    .collect::<Result<_>>()?,
            ),
            S::Or(v) => ActivationFilter::Either(
                v.iter()
                    .map(|a| self.activation_filter(a))
                    .collect::<Result<_>>()?,
            ),
            _ => return Err(unsupported(value)),
        })
    }

    fn rule_changes(&self, values: &[source::PermanentRule]) -> Result<Vec<Change>> {
        let mut context = self.clone();
        context.affected = true;
        let mut result = Vec::new();
        for (i, value) in values.iter().enumerate() {
            result.extend(
                context
                    .rule_change(value)
                    .map_err(|e| e.at(format!("rules[{i}]")))?
                    .into_iter()
                    .map(Change::Rule),
            );
        }
        Ok(result)
    }

    fn rule_change(&self, value: &source::PermanentRule) -> Result<Vec<RuleChange>> {
        self.reduced_rule_change(value).or_else(|error| {
            self.rescue(error, || {
                self.primitive("PermanentRule", value)
                    .map(|(name, operands)| vec![crate::semantics::reduce_rule(name, operands)])
            })
        })
    }
    fn reduced_rule_change(&self, value: &source::PermanentRule) -> Result<Vec<RuleChange>> {
        use source::PermanentRule as S;
        let prohibit = |action| RuleChange::Prohibit(action);
        let require = |action, amount| RuleChange::RequireCombat { action, amount };
        let one = || RequirementAmount::AtLeast(Expr::Constant(1));
        let group = |role, participants, others_only, comparison, amount| RuleChange::CombatGroup {
            role,
            participants,
            others_only,
            comparison,
            amount,
        };
        let taxed = |action, cost: &source::Cost| -> Result<_> {
            let payer = PlayerRef::DeclaringPlayer;
            Ok(RuleChange::CombatCost {
                action,
                cost: self.cost_for(cost, &payer)?,
                payer,
            })
        };
        Ok(vec![match value {
            S::CantAttack => prohibit(ObjectAction::Attack(AttackDefenders::Any)),
            S::CantAttackUnlessDefendingPlayer(condition) => {
                let binding = Binding(self.binding_depth);
                let mut child = self.clone();
                child.defending = Some(PlayerRef::Bound(binding.clone()));
                child.binding_depth += 1;
                prohibit(ObjectAction::Attack(AttackDefenders::DefendingPlayers(
                    PlayerSet::Matching {
                        binding,
                        condition: Box::new(Condition::Not(Box::new(child.condition(condition)?))),
                    },
                )))
            }
            S::CantBlock => prohibit(ObjectAction::Block(ObjectFilter::Any)),
            S::CantBeBlocked => prohibit(ObjectAction::BeBlocked(ObjectFilter::Any)),
            S::CantBeRegenerated => prohibit(ObjectAction::Regenerate),
            S::CantBeSacrificed => prohibit(ObjectAction::Sacrifice),
            S::CantBecomeUntapped => prohibit(ObjectAction::Untap),
            S::AbilitiesCantBeActivated => prohibit(ObjectAction::Activate(ActivationFilter::Any)),
            S::AbilitiesOfTypeCantBeActivated(filter) => {
                prohibit(ObjectAction::Activate(self.activation_filter(filter)?))
            }
            S::CantAttackAPermanent(p) => prohibit(ObjectAction::Attack(AttackDefenders::Objects(
                self.filter(p)?,
            ))),
            S::CantAttackAPlayer(p) => prohibit(ObjectAction::Attack(AttackDefenders::Players(
                self.players(p)?,
            ))),
            S::CantAttackPlayer(p) => prohibit(ObjectAction::Attack(AttackDefenders::Players(
                PlayerSet::One(self.player(p)?),
            ))),
            S::CantAttackAnyPlayerOrPlaneswalkerControlledBy(p) => prohibit(ObjectAction::Attack(
                AttackDefenders::PlayersAndPlaneswalkers(self.players(p)?),
            )),
            S::CantAttackPlayerOrPlaneswalkerControlledBy(p) => prohibit(ObjectAction::Attack(
                AttackDefenders::PlayersAndPlaneswalkers(PlayerSet::One(self.player(p)?)),
            )),
            S::CantBlockAttacker(p) => prohibit(ObjectAction::Block(ObjectFilter::SameObject(
                self.object(p)?,
            ))),
            S::CantBlockAttackers(p) => prohibit(ObjectAction::Block(self.filter(p)?)),
            S::CanBlockOnly(p) => prohibit(ObjectAction::Block(ObjectFilter::Not(Box::new(
                self.filter(p)?,
            )))),
            S::CantBeBlockedByDefenders(p) => prohibit(ObjectAction::BeBlocked(self.filter(p)?)),
            S::CantBeBlockedExceptByDefenders(p) => prohibit(ObjectAction::BeBlocked(
                ObjectFilter::Not(Box::new(self.filter(p)?)),
            )),
            S::MustAttack => require(ObjectAction::Attack(AttackDefenders::Any), one()),
            S::MustAttackPlayer(p) => require(
                ObjectAction::Attack(AttackDefenders::Players(PlayerSet::One(self.player(p)?))),
                one(),
            ),
            S::MustAttackAPlayer(p) => require(
                ObjectAction::Attack(AttackDefenders::Players(self.players(p)?)),
                one(),
            ),
            S::MustAttackPlaneswalker(p) => require(
                ObjectAction::Attack(AttackDefenders::Objects(ObjectFilter::SameObject(
                    self.object(p)?,
                ))),
                one(),
            ),
            S::MustBlock => require(ObjectAction::Block(ObjectFilter::Any), one()),
            S::MustBlockAttacker(p) => require(
                ObjectAction::Block(ObjectFilter::SameObject(self.object(p)?)),
                one(),
            ),
            S::MustBlockEachAttacker => require(
                ObjectAction::Block(ObjectFilter::Any),
                RequirementAmount::Each,
            ),
            S::MustBeBlocked => require(ObjectAction::BeBlocked(ObjectFilter::Any), one()),
            S::MustBeBlockedByADefender(p) => {
                require(ObjectAction::BeBlocked(self.filter(p)?), one())
            }
            S::MustBeBlockedByAtLeastNumberDefenders(n) => require(
                ObjectAction::BeBlocked(ObjectFilter::Any),
                RequirementAmount::AtLeast(self.number(n)?),
            ),
            S::AllCreaturesMustBlockIt(p) | S::MustBeBlockedByEachDefender(p) => require(
                ObjectAction::BeBlocked(self.filter(p)?),
                RequirementAmount::Each,
            ),
            S::CantAttackAlone => group(
                CombatRole::Attacker,
                ObjectFilter::Any,
                false,
                Comparison::Greater,
                Expr::Constant(1),
            ),
            S::CanOnlyAttackAlone => group(
                CombatRole::Attacker,
                ObjectFilter::Any,
                false,
                Comparison::Equal,
                Expr::Constant(1),
            ),
            S::CantBlockAlone => group(
                CombatRole::Blocker,
                ObjectFilter::Any,
                false,
                Comparison::Greater,
                Expr::Constant(1),
            ),
            S::CantBeBlockedByMoreThanOne => group(
                CombatRole::BlockedAttacker,
                ObjectFilter::Any,
                false,
                Comparison::LessOrEqual,
                Expr::Constant(1),
            ),
            S::CantAttackUnlessAnotherCreatureAttacks(p) => group(
                CombatRole::Attacker,
                self.filter(p)?,
                true,
                Comparison::Greater,
                Expr::Constant(0),
            ),
            S::CantBlockUnlessAnotherDefender(p) => group(
                CombatRole::Blocker,
                self.filter(p)?,
                true,
                Comparison::Greater,
                Expr::Constant(0),
            ),
            S::CantAttackUnlessANumberOfOtherCreatureAttacks(c, p)
            | S::CantBlockUnlessOtherDefenders(c, p) => {
                let (comparison, amount) = self.comparison(c)?;
                group(
                    if matches!(value, S::CantAttackUnlessANumberOfOtherCreatureAttacks(..)) {
                        CombatRole::Attacker
                    } else {
                        CombatRole::Blocker
                    },
                    self.filter(p)?,
                    true,
                    comparison,
                    amount,
                )
            }
            S::CantBeBlockedExceptByMultipleDefenders(c, p) => {
                let filter = self.filter(p)?;
                let (comparison, amount) = self.comparison(c)?;
                return Ok(vec![
                    prohibit(ObjectAction::BeBlocked(ObjectFilter::Not(Box::new(
                        filter.clone(),
                    )))),
                    group(
                        CombatRole::BlockedAttacker,
                        filter,
                        false,
                        comparison,
                        amount,
                    ),
                ]);
            }
            S::CantAttackUnlessCost(c) => taxed(ObjectAction::Attack(AttackDefenders::Any), c)?,
            S::CantBlockUnlessCost(c) => taxed(ObjectAction::Block(ObjectFilter::Any), c)?,
            S::CantBeBlockedUnlessCost(c) => taxed(ObjectAction::BeBlocked(ObjectFilter::Any), c)?,
            S::CantAttackAPermanentUnlessCost(p, c) => taxed(
                ObjectAction::Attack(AttackDefenders::Objects(self.filter(p)?)),
                c,
            )?,
            S::CantAttackPlayerUnlessCost(p, c) => taxed(
                ObjectAction::Attack(AttackDefenders::Players(PlayerSet::One(self.player(p)?))),
                c,
            )?,
            S::CantAttackPlayerOrPlaneswalkerControlledByUnlessCost(p, c) => taxed(
                ObjectAction::Attack(AttackDefenders::PlayersAndPlaneswalkers(PlayerSet::One(
                    self.player(p)?,
                ))),
                c,
            )?,
            S::CantBlockAttackersUnlessCost(p, c) => {
                taxed(ObjectAction::Block(self.filter(p)?), c)?
            }
            S::CanAttackAsThoughItDidntHaveDefender => RuleChange::AttackException {
                ignore: AttackCheck::Defender,
                defenders: AttackDefenders::Any,
            },
            S::CanAttackAsThoughItHadHaste => RuleChange::AttackException {
                ignore: AttackCheck::SummoningSickness,
                defenders: AttackDefenders::Any,
            },
            S::CanAttackAPlayerAsThoughItDidntHaveDefender(p) => RuleChange::AttackException {
                ignore: AttackCheck::Defender,
                defenders: AttackDefenders::Players(self.players(p)?),
            },
            S::CanAttackPlayersAndPlaneswalkersAsThoughItHadHaste(p) => {
                RuleChange::AttackException {
                    ignore: AttackCheck::SummoningSickness,
                    defenders: AttackDefenders::PlayersAndPlaneswalkers(self.players(p)?),
                }
            }
            S::CanBlockAsThoughUntapped => RuleChange::BlockWhileTapped,
            S::CanBlockAnAdditionalCreature => {
                RuleChange::BlockCapacity(BlockCapacity::Additional(Expr::Constant(1)))
            }
            S::CanBlockAnAdditionalNumberCreatures(n) => {
                RuleChange::BlockCapacity(BlockCapacity::Additional(self.number(n)?))
            }
            S::CanBlockAnyNumberOfCreatures => RuleChange::BlockCapacity(BlockCapacity::Unlimited),
            S::AssignsNoCombatDamage => RuleChange::CombatDamage(CombatDamageRule::NoAssignment),
            S::AssignsToughnessCombatDamage => {
                RuleChange::CombatDamage(CombatDamageRule::UseToughness)
            }
            S::AssignsCombatDamageAsThoughNotBlocked
            | S::MayAssignCombatDamageAsThoughNotBlocked => {
                RuleChange::CombatDamage(CombatDamageRule::AsUnblocked {
                    optional: matches!(value, S::MayAssignCombatDamageAsThoughNotBlocked),
                })
            }
            S::DoesntUntapDuringControllersUntap | S::MayChooseNotToUntapDuringUntap => {
                RuleChange::UntapStep {
                    players: PlayerSet::One(PlayerRef::ControllerOf(Box::new(ObjectRef::Affected))),
                    behavior: if matches!(value, S::DoesntUntapDuringControllersUntap) {
                        UntapBehavior::Skip
                    } else {
                        UntapBehavior::Optional
                    },
                }
            }
            S::UntapsDuringOtherPlayersUntapSteps(players) => RuleChange::UntapStep {
                players: self.players(players)?,
                behavior: UntapBehavior::Additional,
            },
            _ => return Err(unsupported(value)),
        }])
    }

    fn has_keyword(&self, value: &source::CheckHasable) -> Result<ObjectFilter> {
        use source::CheckHasable as S;
        match value {
            S::Devoid => return Ok(ObjectFilter::HasKeyword(Keyword::Devoid)),
            S::AnyWard => return Ok(ObjectFilter::HasKeywordFamily(KeywordFamily::Ward)),
            S::AnyProtection => {
                return Ok(ObjectFilter::HasKeywordFamily(KeywordFamily::Protection))
            }
            S::AnyLandwalk => return Ok(ObjectFilter::HasKeywordFamily(KeywordFamily::Landwalk)),
            S::AnyHexproof => return Ok(ObjectFilter::HasKeywordFamily(KeywordFamily::Hexproof)),
            S::AnyToxic => return Ok(ObjectFilter::HasKeywordFamily(KeywordFamily::Toxic)),
            S::AnyPartner => return Ok(ObjectFilter::HasKeywordFamily(KeywordFamily::Partner)),
            S::Landwalk(lands) => {
                return Ok(ObjectFilter::HasKeyword(Keyword::Landwalk(Box::new(
                    self.filter(lands)?,
                ))))
            }
            S::ProtectionFromColor(color) => {
                return Ok(ObjectFilter::HasKeyword(Keyword::Protection(Box::new(
                    self.protection(&source::Protectable::FromColor(
                        source::ProtectableColor::Colors(vec![color.clone()]),
                    ))?,
                ))))
            }
            S::AnyCycling => {
                return Ok(ObjectFilter::HasActivatedAbility(ActivationFilter::Cycling))
            }
            S::ActivatedAbility => {
                return Ok(ObjectFilter::HasActivatedAbility(ActivationFilter::Any))
            }
            _ => {}
        }
        if let S::And(v) = value {
            return Ok(ObjectFilter::All(
                v.iter()
                    .map(|k| self.has_keyword(k))
                    .collect::<Result<_>>()?,
            ));
        }
        if let Some(filter) = self.primitive_predicate("CheckHasable", value) {
            return Ok(filter);
        }
        macro_rules! keyword_filter {
            ($($k:ident),* $(,)?)=> { match value { $(S::$k => ObjectFilter::HasKeyword(Keyword::$k),)* _=>return Err(unsupported(value)), } };
        }
        Ok(keyword_filter!(
            Flying,
            Trample,
            Lifelink,
            Deathtouch,
            Vigilance,
            Haste,
            Reach,
            FirstStrike,
            DoubleStrike,
            Indestructible,
            Shroud,
            Menace,
            Defender,
            Flash,
            Fear,
            Infect,
            Flanking,
            Horsemanship,
            Shadow,
            Skulk
        ))
    }
}
