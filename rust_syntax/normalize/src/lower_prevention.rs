/// One side of a would-be damage event. Every source form spells this pair in
/// its variant name, so the arms below only sort operands by type.
enum DamageFrom<'a> {
    Anything,
    Permanents(&'a source::Permanents),
    Permanent(&'a source::Permanent),
    Sources(&'a source::DamageSources),
    Source(&'a source::SingleDamageSource),
    Spells(&'a source::Spells),
    Spell(&'a source::Spell),
}
enum DamageTo<'a> {
    Anything,
    List(&'a source::DamageRecipientsList),
    One(&'a source::SingleDamageRecipient),
}

impl Context {
    fn damage_shape(
        &self,
        from: DamageFrom<'_>,
        to: DamageTo<'_>,
        kind: DamageKind,
    ) -> Result<DamageEvent> {
        // The prevented recipient is the event's subject, as for "that creature".
        let (source_kind, sources) = match from {
            DamageFrom::Anything => (ObjectKind::Any, ObjectFilter::Any),
            DamageFrom::Permanents(p) => (ObjectKind::Permanent, self.filter(p)?),
            DamageFrom::Permanent(p) => (
                ObjectKind::Permanent,
                ObjectFilter::SameObject(self.object(p)?),
            ),
            DamageFrom::Sources(s) => (ObjectKind::Any, self.damage_sources(s)?),
            DamageFrom::Source(source::SingleDamageSource::TheChosenDamageSource) => (
                ObjectKind::Any,
                ObjectFilter::SameObject(ObjectRef::Result {
                    result: self
                        .chosen_damage_source
                        .clone()
                        .ok_or_else(|| invalid("no damage source was chosen in this scope"))?,
                    phase: ObjectPhase::Before,
                    index: 0,
                }),
            ),
            DamageFrom::Spells(s) => (ObjectKind::Spell, self.spell_filter(s)?),
            DamageFrom::Spell(s) => (ObjectKind::Spell, ObjectFilter::SameObject(self.spell(s)?)),
        };
        let recipients = match to {
            DamageTo::Anything => DamageRecipients::Any,
            DamageTo::List(source::DamageRecipientsList::APermanent(p)) => {
                DamageRecipients::Objects(self.filter(p)?)
            }
            DamageTo::List(source::DamageRecipientsList::APlayer(p)) => {
                DamageRecipients::Players(self.players(p)?)
            }
            DamageTo::List(source::DamageRecipientsList::APlayerOrAPermanent(players, objects)) => {
                DamageRecipients::ObjectsAndPlayers {
                    objects: self.filter(objects)?,
                    players: self.players(players)?,
                }
            }
            DamageTo::One(one) => self.single_recipient(one)?,
        };
        Ok(DamageEvent {
            source_kind,
            sources,
            recipients,
            kind,
            grouping: DamageGrouping::SourceRecipient,
            subject: DamageSubject::Recipient,
        })
    }

    fn single_recipient(&self, value: &source::SingleDamageRecipient) -> Result<DamageRecipients> {
        use source::SingleDamageRecipient as S;
        let any_target = |index: u16| -> Result<DamageRecipients> {
            Ok(DamageRecipients::Target {
                slot: self.target(TargetKind::DamageRecipient, 0, index)?,
                index,
            })
        };
        match value {
            S::Player(p) => Ok(DamageRecipients::Players(PlayerSet::One(self.player(p)?))),
            S::Permanent(p) => Ok(DamageRecipients::Objects(ObjectFilter::SameObject(
                self.object(p)?,
            ))),
            S::Ref_AnyTarget | S::Ref_AnyTargets_1 | S::Ref_TargetPlayerOrPermanent => {
                any_target(0)
            }
            S::Ref_AnyTargets_2 => any_target(1),
            S::DistributedAnyTarget => Err(unsupported(value)),
        }
    }

    fn prevented_event(&self, value: &source::EventPreventDamage) -> Result<ReplacementEvent> {
        use source::EventPreventDamage as S;
        use DamageFrom as F;
        use DamageTo as T;
        if let S::Or(items) = value {
            return Ok(ReplacementEvent::Either(
                items
                    .iter()
                    .map(|item| self.prevented_event(item))
                    .collect::<Result<_>>()?,
            ));
        }
        let kind = match value {
            S::CombatDamageWouldBeDealt
            | S::CombatDamageWouldBeDealtByACreature(..)
            | S::CombatDamageWouldBeDealtByACreatureToASetOfRecipients(..)
            | S::CombatDamageWouldBeDealtByACreatureToARecipient(..)
            | S::CombatDamageWouldBeDealtByACreatureToRecipient(..)
            | S::CombatDamageWouldBeDealtByCreature(..)
            | S::CombatDamageWouldBeDealtByCreatureToARecipient(..)
            | S::CombatDamageWouldBeDealtToARecipient(..)
            | S::CombatDamageWouldBeDealtToRecipient(..) => DamageKind::Combat,
            S::NoncombatDamageWouldBeDealtToARecipient(..)
            | S::NoncombatDamageWouldBeDealtToRecipient(..) => DamageKind::Noncombat,
            _ => DamageKind::Any,
        };
        let (from, to) = match value {
            S::CombatDamageWouldBeDealt => (F::Anything, T::Anything),
            S::CombatDamageWouldBeDealtByACreature(p) | S::DamageWouldBeDealtByAPermanent(p) => {
                (F::Permanents(p), T::Anything)
            }
            S::CombatDamageWouldBeDealtByACreatureToARecipient(p, r)
            | S::DamageWouldBeDealtByAPermanentToARecipient(p, r) => {
                (F::Permanents(p), T::List(r))
            }
            S::CombatDamageWouldBeDealtByACreatureToRecipient(p, r)
            | S::DamageWouldBeDealtByAPermanentToRecipient(p, r) => (F::Permanents(p), T::One(r)),
            S::CombatDamageWouldBeDealtByCreature(p) | S::DamageWouldBeDealtByPermanent(p) => {
                (F::Permanent(p), T::Anything)
            }
            S::CombatDamageWouldBeDealtByCreatureToARecipient(p, r)
            | S::DamageWouldBeDealtByPermanentToARecipient(p, r) => (F::Permanent(p), T::List(r)),
            S::DamageWouldBeDealtByPermanentToRecipient(p, r) => (F::Permanent(p), T::One(r)),
            S::DamageWouldBeDealtByASource(s) => (F::Sources(s), T::Anything),
            S::DamageWouldBeDealtByASourceToARecipient(s, r) => (F::Sources(s), T::List(r)),
            S::DamageWouldBeDealtByASourceToRecipient(s, r) => (F::Sources(s), T::One(r)),
            S::DamageWouldBeDealtByASpell(s) => (F::Spells(s), T::Anything),
            S::DamageWouldBeDealtByASpellToARecipient(s, r) => (F::Spells(s), T::List(r)),
            S::DamageWouldBeDealtByASpellToRecipient(s, r) => (F::Spells(s), T::One(r)),
            S::DamageWouldBeDealtBySpell(s) => (F::Spell(s), T::Anything),
            S::DamageWouldBeDealtBySource(s) => (F::Source(s), T::Anything),
            S::DamageWouldBeDealtBySourceToRecipient(s, r) => (F::Source(s), T::One(r)),
            S::CombatDamageWouldBeDealtToARecipient(r)
            | S::DamageWouldBeDealtToARecipient(r)
            | S::NoncombatDamageWouldBeDealtToARecipient(r) => (F::Anything, T::List(r)),
            S::CombatDamageWouldBeDealtToRecipient(r)
            | S::DamageWouldBeDealtToRecipient(r)
            | S::NoncombatDamageWouldBeDealtToRecipient(r) => (F::Anything, T::One(r)),
            _ => return Err(unsupported(value)),
        };
        self.damage_shape(from, to, kind).map(ReplacementEvent::Damage)
    }

    fn future_prevention(
        &self,
        value: &source::FutureEventPreventDamage,
    ) -> Result<(ReplacementEvent, ReplacementLimit)> {
        use source::FutureEventPreventDamage as S;
        use DamageFrom as F;
        use DamageTo as T;
        let limit = match value {
            S::NextAmountOfDamageThatWouldBeDealtThisTurnByPermanent(n, ..)
            | S::NextAmountOfDamageThatWouldBeDealtThisTurnBySourceToARecipient(n, ..)
            | S::NextAmountOfDamageThatWouldBeDealtThisTurnBySourceToRecipient(n, ..)
            | S::NextAmountOfDamageThatWouldBeDealtThisTurnBySpellToRecipient(n, ..)
            | S::NextAmountOfDamageThatWouldBeDealtThisTurnToARecipient(n, ..)
            | S::NextAmountOfDamageThatWouldBeDealtThisTurnToRecipient(n, ..) => {
                ReplacementLimit::TotalDamage(self.number(n)?)
            }
            _ => ReplacementLimit::NextOccurrence,
        };
        let kind = match value {
            S::NextTimeCombatDamageWouldBeDealtThisTurnByCreature(..)
            | S::NextTimeCombatDamageWouldBeDealtThisTurnByCreatureToRecipient(..) => {
                DamageKind::Combat
            }
            _ => DamageKind::Any,
        };
        let (from, to) = match value {
            S::NextAmountOfDamageThatWouldBeDealtThisTurnByPermanent(_, p)
            | S::NextTimeCombatDamageWouldBeDealtThisTurnByCreature(p)
            | S::NextTimeDamageWouldBeDealtThisTurnByPermanent(p) => (F::Permanent(p), T::Anything),
            S::NextTimeCombatDamageWouldBeDealtThisTurnByCreatureToRecipient(p, r)
            | S::NextTimeDamageWouldBeDealtThisTurnByPermanentToRecipient(p, r) => {
                (F::Permanent(p), T::One(r))
            }
            S::NextTimeDamageWouldBeDealtThisTurnByAPermanentToRecipient(p, r) => {
                (F::Permanents(p), T::One(r))
            }
            S::NextAmountOfDamageThatWouldBeDealtThisTurnBySourceToARecipient(_, s, r)
            | S::NextTimeDamageWouldBeDealtThisTurnBySourceToARecipient(s, r) => {
                (F::Source(s), T::List(r))
            }
            S::NextAmountOfDamageThatWouldBeDealtThisTurnBySourceToRecipient(_, s, r)
            | S::NextTimeDamageWouldBeDealtThisTurnBySourceToRecipient(s, r) => {
                (F::Source(s), T::One(r))
            }
            S::NextTimeDamageWouldBeDealtThisTurnBySource(s) => (F::Source(s), T::Anything),
            S::NextAmountOfDamageThatWouldBeDealtThisTurnBySpellToRecipient(_, s, r) => {
                (F::Spell(s), T::One(r))
            }
            S::NextAmountOfDamageThatWouldBeDealtThisTurnToARecipient(_, r) => {
                (F::Anything, T::List(r))
            }
            S::NextAmountOfDamageThatWouldBeDealtThisTurnToRecipient(_, r)
            | S::NextTimeDamageWouldBeDealtThisTurnToRecipient(r) => (F::Anything, T::One(r)),
            _ => return Err(unsupported(value)),
        };
        Ok((
            ReplacementEvent::Damage(self.damage_shape(from, to, kind)?),
            limit,
        ))
    }

    fn prevention_actions(&mut self, values: &[source::ActionPreventDamage]) -> Result<Program> {
        let mut instructions = Vec::new();
        for (i, value) in values.iter().enumerate() {
            instructions.push(
                self.prevention_action(value)
                    .map_err(|e| e.at(format!("prevention[{i}]")))?,
            );
        }
        Ok(Program { instructions })
    }

    fn prevention_action(&mut self, value: &source::ActionPreventDamage) -> Result<Instruction> {
        use source::ActionPreventDamage as S;
        let change = match value {
            S::RemoveCounters(_) | S::PutCounters(_) => {
                let mut inspected = crate::inspect::value(value)
                    .map_err(|_| invalid("prevention action does not serialize"))?;
                retag(&mut inspected, &|name: &str| match name {
                    "ActionPreventDamage" => Some("Action"),
                    "ActionPreventDamagePutCounters" => Some("PutCountersAction"),
                    "ActionPreventDamageRemoveCounters" => Some("RemoveCountersAction"),
                    _ => None,
                });
                let action: source::Action = crate::pipeline::source_tree::decode(&inspected)
                    .map_err(|_| unsupported(value))?;
                return self.action(&action, &PlayerSet::One(PlayerRef::AbilityController));
            }
            S::PreventThatDamage => EventChange::PreventDamage,
            S::PreventSomeOfThatDamage(n) => EventChange::PreventSomeDamage(self.number(n)?),
            S::PreventAllButSomeOfThatDamage(n) => {
                EventChange::PreventAllButDamage(self.number(n)?)
            }
            S::If(condition, then) | S::Unless(condition, then) => {
                let mut condition = self.condition(condition)?;
                if matches!(value, S::Unless(..)) {
                    condition = Condition::Not(Box::new(condition));
                }
                return self.conditional(
                    condition,
                    |c| c.prevention_actions(then),
                    |_| Ok(Program::empty()),
                );
            }
            S::IfElse(condition, then, otherwise) => {
                let condition = self.condition(condition)?;
                return self.conditional(
                    condition,
                    |c| c.prevention_actions(then),
                    |c| c.prevention_actions(otherwise),
                );
            }
            S::PermanentDealsDamage(p, n, r) => {
                return Ok(self.effect(Operation::DealDamage {
                    source: self.object(p)?,
                    recipients: self.recipients(r)?,
                    amount: self.number(n)?,
                    preventable: true,
                }))
            }
            S::SpellDealsDamage(s, n, r) => {
                return Ok(self.effect(Operation::DealDamage {
                    source: self.spell(s)?,
                    recipients: self.recipients(r)?,
                    amount: self.number(n)?,
                    preventable: true,
                }))
            }
            S::GainLife(n) => {
                return Ok(self.effect(Operation::GainLife {
                    players: PlayerSet::One(self.player(&source::Player::You)?),
                    amount: self.number(n)?,
                }))
            }
            S::DrawNumberCards(n) => {
                return Ok(self.effect(Operation::Draw {
                    players: PlayerSet::One(self.player(&source::Player::You)?),
                    amount: self.number(n)?,
                }))
            }
            _ => {
                let action: source::Action =
                    as_family(value, "Action").ok_or_else(|| unsupported(value))?;
                let actor = PlayerSet::One(self.player(&source::Player::You)?);
                return self.action(&action, &actor);
            }
        };
        Ok(self.effect(Operation::ModifyEvent { change }))
    }

    fn prevention_body(
        &mut self,
        event: &ReplacementEvent,
        actions: &[source::ActionPreventDamage],
    ) -> Result<Program> {
        let mut child = self.clone();
        child.event = crate::semantics::replacement_bindings(event);
        let body = child.prevention_actions(actions)?;
        self.result = child.result;
        Ok(body)
    }
}
