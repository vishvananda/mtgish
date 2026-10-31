impl Context {
    fn damage_sources(&self, value: &source::DamageSources) -> Result<ObjectFilter> {
        use source::DamageSources as S;
        Ok(match value {
            S::AnyDamageSource => ObjectFilter::Any,
            S::Other(other) => {
                let other = match other {
                    source::DamageSource::Trigger_ThatPermanent => {
                        self.object(&source::Permanent::Trigger_ThatPermanent)?
                    }
                    source::DamageSource::ThisDamageSource => ObjectRef::Source,
                };
                ObjectFilter::Not(Box::new(ObjectFilter::InSet(Box::new(ObjectSet::One(
                    other,
                )))))
            }
            S::And(v) => ObjectFilter::All(
                v.iter()
                    .map(|f| self.damage_sources(f))
                    .collect::<Result<_>>()?,
            ),
            S::Or(v) => ObjectFilter::Either(
                v.iter()
                    .map(|f| self.damage_sources(f))
                    .collect::<Result<_>>()?,
            ),
            S::ControlledByAPlayer(p) => ObjectFilter::ControlledBy(self.players(p)?),
            S::IsCardtype(t) => ObjectFilter::CardType(t.clone()),
            S::IsNonCardtype(t) => ObjectFilter::Not(Box::new(ObjectFilter::CardType(t.clone()))),
            S::IsCreatureType(t) => ObjectFilter::Subtype(crate::subtype::creature_type(t)),
            S::IsNonCreatureType(t) => ObjectFilter::Not(Box::new(ObjectFilter::Subtype(
                crate::subtype::creature_type(t),
            ))),
            S::IsColor(c) => self.color_filter(c)?,
            S::IsNotPermanentSource(p) => {
                ObjectFilter::Not(Box::new(ObjectFilter::SameObject(self.object(p)?)))
            }
            S::ManaValueIs(c) => {
                let (comparison, value) = self.comparison(c)?;
                ObjectFilter::Property {
                    property: Property::ManaValue,
                    comparison,
                    value,
                }
            }
            S::IsNamed(n) => self.card_filter(&source::Cards::IsNamed(n.clone()))?,
            _ => {
                let (name, operands) = self
                    .primitive("DamageSources", value)
                    .ok_or_else(|| unsupported(value))?;
                ObjectFilter::Primitive { name, operands }
            }
        })
    }
    fn damage_trigger(&self, value: &source::Trigger) -> Result<EventSpec> {
        use source::Trigger as S;
        // Filters apply to event participants, not to a fresh battlefield query.
        let mut event = DamageEvent {
            source_kind: ObjectKind::Any,
            sources: ObjectFilter::Any,
            recipients: DamageRecipients::Any,
            kind: DamageKind::Any,
            grouping: DamageGrouping::SourceRecipient,
            subject: DamageSubject::Source,
        };
        let mut amount = None;
        match value {
            S::WhenACreatureDealsCombatDamageToAPlayer(p, players)
            | S::WhenAPermanentDealsDamageToAPlayer(p, players)
            | S::WhenAnyNumberOfCreaturesDealCombatDamageToAPlayer(p, players)
            | S::WhenAnyNumberOfCreaturesDealDamageToAPlayer(p, players)
            | S::WhenAnyNumberOfCreaturesDealCombatDamageToAnyNumberOfPlayers(p, players)
            | S::WhenAnyNumberOfPermanentsDealDamageToAnyNumberOfPlayers(p, players)
            | S::WhenAPermanentDealsDamageToAnyNumberOfPlayersForTheFirstTimeEachTurn(p, players) =>
            {
                // First-time history is not an ordinary once-per-turn limit.
                if matches!(
                    value,
                    S::WhenAPermanentDealsDamageToAnyNumberOfPlayersForTheFirstTimeEachTurn(..)
                ) {
                    return Err(unsupported(value));
                }
                event.source_kind = ObjectKind::Permanent;
                event.sources = self.filter(p)?;
                event.recipients = DamageRecipients::Players(self.players(players)?);
                event.kind = if matches!(
                    value,
                    S::WhenACreatureDealsCombatDamageToAPlayer(..)
                        | S::WhenAnyNumberOfCreaturesDealCombatDamageToAPlayer(..)
                        | S::WhenAnyNumberOfCreaturesDealCombatDamageToAnyNumberOfPlayers(..)
                ) {
                    DamageKind::Combat
                } else {
                    DamageKind::Any
                };
                event.grouping = match value {
                    S::WhenAnyNumberOfCreaturesDealCombatDamageToAPlayer(..)
                    | S::WhenAnyNumberOfCreaturesDealDamageToAPlayer(..) => {
                        DamageGrouping::Recipient
                    }
                    S::WhenAnyNumberOfCreaturesDealCombatDamageToAnyNumberOfPlayers(..)
                    | S::WhenAnyNumberOfPermanentsDealDamageToAnyNumberOfPlayers(..) => {
                        DamageGrouping::Batch
                    }
                    _ => DamageGrouping::SourceRecipient,
                };
            }
            S::WhenACreatureDealsCombatDamageToAPermanent(p, r)
            | S::WhenAPermanentDealsDamageToAPermanent(p, r)
            | S::WhenACreatureDealsCombatDamageToAnyNumberOfPermanents(p, r)
            | S::WhenAPermanentDealsDamageToAnyNumberOfPermanents(p, r)
            | S::WhenAnyNumberOfCreaturesDealCombatDamageToAPermanent(p, r) => {
                event.source_kind = ObjectKind::Permanent;
                event.sources = self.filter(p)?;
                event.recipients = DamageRecipients::Objects(self.filter(r)?);
                event.kind = if matches!(
                    value,
                    S::WhenACreatureDealsCombatDamageToAPermanent(..)
                        | S::WhenACreatureDealsCombatDamageToAnyNumberOfPermanents(..)
                        | S::WhenAnyNumberOfCreaturesDealCombatDamageToAPermanent(..)
                ) {
                    DamageKind::Combat
                } else {
                    DamageKind::Any
                };
                event.grouping = match value {
                    S::WhenACreatureDealsCombatDamageToAnyNumberOfPermanents(..)
                    | S::WhenAPermanentDealsDamageToAnyNumberOfPermanents(..) => {
                        DamageGrouping::Source
                    }
                    S::WhenAnyNumberOfCreaturesDealCombatDamageToAPermanent(..) => {
                        DamageGrouping::Recipient
                    }
                    _ => DamageGrouping::SourceRecipient,
                };
            }
            S::WhenAPermanentDealsDamage(p)
            | S::WhenACreatureDealsCombatDamage(p)
            | S::WhenAPermanentDealsAnAmountDamage(p, _) => {
                event.source_kind = ObjectKind::Permanent;
                event.sources = self.filter(p)?;
                event.grouping = DamageGrouping::Source;
                if matches!(value, S::WhenACreatureDealsCombatDamage(_)) {
                    event.kind = DamageKind::Combat;
                }
                if let S::WhenAPermanentDealsAnAmountDamage(_, c) = value {
                    amount = Some(c.as_ref());
                }
            }
            S::WhenAPermanentDealsAnAmountDamageToAPlayer(p, c, players) => {
                event.source_kind = ObjectKind::Permanent;
                event.sources = self.filter(p)?;
                event.recipients = DamageRecipients::Players(self.players(players)?);
                amount = Some(c.as_ref());
            }
            S::WhenAPermanentIsDealtDamage(p)
            | S::WhenAPermanentIsDealtCombatDamage(p)
            | S::WhenAPermanentIsDealtNoncombatDamage(p)
            | S::WhenAPermanentIsDealtExcessDamage(p)
            | S::WhenAPermanentIsDealtExcessNoncombatDamage(p)
            | S::WhenAPermanentIsDealtAnAmountOfDamage(p, _) => {
                event.recipients = DamageRecipients::Objects(self.filter(p)?);
                event.subject = DamageSubject::Recipient;
                event.grouping = DamageGrouping::Recipient;
                event.kind = match value {
                    S::WhenAPermanentIsDealtCombatDamage(_) => DamageKind::Combat,
                    S::WhenAPermanentIsDealtNoncombatDamage(_) => DamageKind::Noncombat,
                    S::WhenAPermanentIsDealtExcessDamage(_) => DamageKind::Excess,
                    S::WhenAPermanentIsDealtExcessNoncombatDamage(_) => DamageKind::ExcessNoncombat,
                    _ => DamageKind::Any,
                };
                if let S::WhenAPermanentIsDealtAnAmountOfDamage(_, c) = value {
                    amount = Some(c.as_ref());
                }
            }
            S::WhenAnyNumberOfPermanentsAreDealtExcessNoncombatDamage(p) => {
                event.recipients = DamageRecipients::Objects(self.filter(p)?);
                event.subject = DamageSubject::Recipient;
                event.grouping = DamageGrouping::Batch;
                event.kind = DamageKind::ExcessNoncombat;
            }
            S::WhenDamageThatWouldBeDealtToAPlayerIsPrevented(p) => {
                event.recipients = DamageRecipients::Players(self.players(p)?);
                event.subject = DamageSubject::Recipient;
                event.grouping = DamageGrouping::Recipient;
                event.kind = DamageKind::Prevented;
            }
            S::WhenAPlayerIsDealtDamage(p)
            | S::WhenAPlayerIsDealtCombatDamage(p)
            | S::WhenAPlayerIsDealtNoncombatDamage(p)
            | S::WhenAnyNumberOfPlayersAreDealtCombatDamage(p)
            | S::WhenAnyNumberOfPlayersAreDealtNoncombatDamage(p) => {
                event.recipients = DamageRecipients::Players(self.players(p)?);
                event.subject = DamageSubject::Recipient;
                event.grouping = if matches!(
                    value,
                    S::WhenAnyNumberOfPlayersAreDealtCombatDamage(_)
                        | S::WhenAnyNumberOfPlayersAreDealtNoncombatDamage(_)
                ) {
                    DamageGrouping::Batch
                } else {
                    DamageGrouping::Recipient
                };
                event.kind = match value {
                    S::WhenAPlayerIsDealtCombatDamage(_)
                    | S::WhenAnyNumberOfPlayersAreDealtCombatDamage(_) => DamageKind::Combat,
                    S::WhenAPlayerIsDealtNoncombatDamage(_)
                    | S::WhenAnyNumberOfPlayersAreDealtNoncombatDamage(_) => DamageKind::Noncombat,
                    _ => DamageKind::Any,
                };
            }
            S::WhenASourceDealsDamage(s) => {
                event.sources = self.damage_sources(s)?;
                event.grouping = DamageGrouping::Source;
            }
            S::WhenASourceDealsDamageToAPermanent(s, p)
            | S::WhenASourceDealsNoncombatDamageToAPermanent(s, p)
            | S::WhenASourceDealsAnAmountOfDamageToAPermanent(s, _, p) => {
                event.sources = self.damage_sources(s)?;
                event.recipients = DamageRecipients::Objects(self.filter(p)?);
                if matches!(value, S::WhenASourceDealsNoncombatDamageToAPermanent(..)) {
                    event.kind = DamageKind::Noncombat;
                }
                if let S::WhenASourceDealsAnAmountOfDamageToAPermanent(_, c, _) = value {
                    amount = Some(c.as_ref());
                }
            }
            S::WhenASourceDealsDamageToAPlayer(s, p)
            | S::WhenASourceDealsNoncombatDamageToAPlayer(s, p)
            | S::WhenASourceDealsAnAmountOfDamageToAPlayer(s, _, p) => {
                event.sources = self.damage_sources(s)?;
                event.recipients = DamageRecipients::Players(self.players(p)?);
                if matches!(value, S::WhenASourceDealsNoncombatDamageToAPlayer(..)) {
                    event.kind = DamageKind::Noncombat;
                }
                if let S::WhenASourceDealsAnAmountOfDamageToAPlayer(_, c, _) = value {
                    amount = Some(c.as_ref());
                }
            }
            S::WhenASourceDealsDamageToAnyNumberOfPlayersAndOrPermanents(s, players, p) => {
                event.sources = self.damage_sources(s)?;
                event.recipients = DamageRecipients::ObjectsAndPlayers {
                    objects: self.filter(p)?,
                    players: self.players(players)?,
                };
                event.grouping = DamageGrouping::Source;
            }
            S::WhenASpellDealsDamage(s) => {
                event.source_kind = ObjectKind::Spell;
                event.sources = self.spell_filter(s)?;
                event.grouping = DamageGrouping::Source;
            }
            S::WhenASpellDealsDamageToAPermanent(s, p) => {
                event.source_kind = ObjectKind::Spell;
                event.sources = self.spell_filter(s)?;
                event.recipients = DamageRecipients::Objects(self.filter(p)?);
            }
            S::WhenASpellDealsDamageToAPlayer(s, p) => {
                event.source_kind = ObjectKind::Spell;
                event.sources = self.spell_filter(s)?;
                event.recipients = DamageRecipients::Players(self.players(p)?);
            }
            _ => return Err(unsupported(value)),
        }
        let event = EventSpec::Damage(event);
        Ok(if let Some(c) = amount {
            let (comparison, right) = self.comparison(c)?;
            EventSpec::Filtered {
                event: Box::new(event),
                condition: Condition::Compare {
                    left: Expr::EventAmount,
                    comparison,
                    right,
                },
            }
        } else {
            event
        })
    }
}

// The old noncombat two-object trigger reuses "that permanent" for either
// role in different templates. Never guess when both participants are queries.
fn legacy_damage_ambiguity(event: &EventSpec) -> bool {
    match event {
        EventSpec::Damage(d) => matches!((&d.subject, &d.source_kind, &d.grouping, &d.recipients),
            (DamageSubject::Source, ObjectKind::Permanent, DamageGrouping::SourceRecipient, DamageRecipients::Objects(f))
            if !matches!(f, ObjectFilter::SameObject(_)) && !matches!(d.kind, DamageKind::Combat)),
        EventSpec::Filtered { event, .. } => legacy_damage_ambiguity(event),
        EventSpec::Either(events) => {
            // A shared singular permanent source and a player-recipient
            // alternative leave no shared permanent-recipient interpretation.
            let b = crate::semantics::event_bindings(event);
            if b.damage_source_permanent
                && !b.damage_recipient
                && legacy_damage_permanent_role(event) == Some(LegacyPermanentRef::Subject)
            {
                return false;
            }
            legacy_damage_permanent_role(event).is_none()
                || events.iter().any(legacy_damage_ambiguity)
        }
        _ => false,
    }
}

// Verified production at english_grammar.json5's "Whenever a [creature type]
// you control deals damage to a creature, that [type] deals that much damage
// to that creature's controller". The two legacy aliases name different roles.
// Match its typed output, with arbitrary source/recipient predicates; no names.
fn legacy_damage_recipient_alias(trigger: &source::Trigger, actions: &source::Actions) -> bool {
    if !matches!(
        trigger,
        source::Trigger::WhenAPermanentDealsDamageToAPermanent(..)
    ) {
        return false;
    }
    let source::Actions::ActionList(actions) = actions else {
        return false;
    };
    let [source::Action::PermanentDealsDamage(object, amount, recipient)] = actions.as_slice()
    else {
        return false;
    };
    matches!((object.as_ref(), amount.as_ref(), recipient.as_ref()),
        (source::Permanent::Trigger_ThatCreature, source::GameNumber::Trigger_ThatMuch,
         source::DamageRecipient::Player(player)) if matches!(player.as_ref(), source::Player::ControllerOfPermanent(p) if matches!(p.as_ref(), source::Permanent::Trigger_ThatPermanent)))
}

// This legacy name is the recipient in the combat-to-permanent templates,
// while "that creature" refers to the source. Other event subjects keep their
// ordinary meaning. Mixed alternatives cannot invent a common role.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LegacyPermanentRef {
    Subject,
    Recipient,
}
fn legacy_damage_permanent_role(event: &EventSpec) -> Option<LegacyPermanentRef> {
    match event {
        EventSpec::Damage(d)
            if matches!(
                (&d.kind, &d.source_kind, &d.grouping, &d.recipients),
                (
                    DamageKind::Combat,
                    ObjectKind::Permanent,
                    DamageGrouping::SourceRecipient,
                    DamageRecipients::Objects(_)
                )
            ) =>
        {
            Some(LegacyPermanentRef::Recipient)
        }
        EventSpec::Filtered { event, .. } => legacy_damage_permanent_role(event),
        EventSpec::Either(events) => {
            let first = events.first().and_then(legacy_damage_permanent_role)?;
            events
                .iter()
                .all(|e| legacy_damage_permanent_role(e) == Some(first))
                .then_some(first)
        }
        _ => Some(LegacyPermanentRef::Subject),
    }
}

impl Context {
    /// Who deals the damage of a spell's own effect.
    fn spell_source(&self, spell: &source::Spell) -> Result<ObjectRef> {
        match spell {
            source::Spell::ThisSpell => Ok(ObjectRef::Source),
            _ => Err(unsupported(spell)),
        }
    }

    fn per_each(&self, amount: &source::GameNumber, each: &source::GameNumber) -> Result<Expr> {
        Ok(Expr::Multiply(
            Box::new(self.number(amount)?),
            Box::new(self.number(each)?),
        ))
    }

    fn deal(
        &self,
        source: Result<ObjectRef>,
        amount: Result<Expr>,
        recipients: &source::DamageRecipient,
    ) -> Result<Operation> {
        Ok(Operation::DealDamage {
            source: source?,
            recipients: self.recipients(recipients)?,
            amount: amount?,
            preventable: true,
        })
    }

    fn deal_with(
        &self,
        source: Result<ObjectRef>,
        amount: &source::GameNumber,
        recipients: &source::DamageRecipient,
        rider: Result<DamageRider>,
    ) -> Result<Operation> {
        Ok(Operation::DealDamageWith {
            source: source?,
            recipients: self.recipients(recipients)?,
            amount: self.number(amount)?,
            rider: rider?,
        })
    }

    fn excess_rider(&self, instead: &[source::Action]) -> Result<DamageRider> {
        match instead {
            [source::Action::WouldDealExcessDamage_DealExcessToPlayerInstead(player)] => {
                Ok(DamageRider::ExcessTo(self.player(player)?))
            }
            _ => Err(invalid("excess damage goes only to one player")),
        }
    }

    /// Damage forms that differ only in who deals it and how the amount is read.
    fn damage_action(&mut self, value: &source::Action) -> Option<Result<Instruction>> {
        use source::Action as S;
        let source = Ok(ObjectRef::Source);
        let operation = match value {
            S::PermanentDealsDamageForEach(permanent, amount, recipients, each) => {
                self.deal(self.object(permanent), self.per_each(amount, each), recipients)
            }
            S::SpellDealsDamageForEach(spell, amount, recipients, each) => self.deal(
                self.spell_source(spell),
                self.per_each(amount, each),
                recipients,
            ),
            S::GraveyardCardDealsDamage(card, amount, recipients) => {
                self.deal(self.grave_card(card), self.number(amount), recipients)
            }
            S::ExiledCardDealsDamage(card, amount, recipients) => {
                self.deal(self.exiled_card(card), self.number(amount), recipients)
            }
            S::SacrificedPermanentDealsDamage(amount, recipients) => self.deal(
                self.object(&source::Permanent::TheSacrificedPermanent),
                self.number(amount),
                recipients,
            ),
            S::SchemeDealsDamage(_, amount, recipients)
            | S::PlaneDealsDamage(_, amount, recipients)
            | S::VanguardDealsDamage(_, amount, recipients)
            | S::BoonDealsDamage(_, amount, recipients)
            | S::EmblemDealsDamage(source::Emblem::ThisEmblem, amount, recipients)
            | S::DeadGuestPermanentDealsDamage(amount, recipients)
            | S::PermanentOrDeadPermanentDealsDamage(amount, recipients) => {
                self.deal(source, self.number(amount), recipients)
            }
            S::SpellDealsDamageDividedAmongRecipientsRoundedDown(spell, amount, recipients) => self
                .deal_with(
                    self.spell_source(spell),
                    amount,
                    recipients,
                    Ok(DamageRider::DividedEvenly(Rounding::Down)),
                ),
            S::PermanentDealsDamageAndPreventSomeOfIt(permanent, amount, recipients, prevented) => {
                self.deal_with(
                    self.object(permanent),
                    amount,
                    recipients,
                    self.number(prevented).map(DamageRider::Prevented),
                )
            }
            S::SpellDealsDamageExcessReplacable(spell, amount, recipients, instead) => self
                .deal_with(
                    self.spell_source(spell),
                    amount,
                    recipients,
                    self.excess_rider(instead),
                ),
            S::PermanentDealsDamageExcessReplacable(permanent, amount, recipients, instead) => self
                .deal_with(
                    self.object(permanent),
                    amount,
                    recipients,
                    self.excess_rider(instead),
                ),
            S::SpellDealsDamageEachPlayer(players, spell, amount, recipients) => {
                return Some(self.spell_damage_each_player(players, spell, amount, None, recipients))
            }
            S::SpellDealsDamageEachPlayerForEach(players, spell, amount, recipients, each) => {
                return Some(self.spell_damage_each_player(
                    players,
                    spell,
                    amount,
                    Some(each),
                    recipients,
                ))
            }
            _ => return None,
        };
        Some(operation.map(|operation| self.effect(operation)))
    }

    fn spell_damage_each_player(
        &mut self,
        players: &source::Players,
        spell: &source::Spell,
        amount: &source::GameNumber,
        each: Option<&source::GameNumber>,
        recipients: &source::DamageRecipient,
    ) -> Result<Instruction> {
        let source = self.spell_source(spell);
        let players = self.players(players)?;
        self.for_each_player(players, |context, _| {
            let amount = match each {
                Some(each) => context.per_each(amount, each),
                None => context.number(amount),
            };
            let operation = context.deal(source, amount, recipients)?;
            Ok(context.effect(operation))
        })
    }
}
