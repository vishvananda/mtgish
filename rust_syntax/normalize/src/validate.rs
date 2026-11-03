//! Validate semantic data after lowering and after loading a normalized export.
//! A successful deserialize alone does not prove reference or instruction scope.

use crate::ir::*;
use std::collections::{BTreeMap, BTreeSet};

/// The slot in `casting_costs` that holds the enclosing cost branch's cost.
const BRANCH_COST: u32 = u32::MAX;

#[derive(Clone, Copy)]
enum ResultKind {
    Observation,
    Search,
    Payment,
    Selection,
    ObjectTransition,
    CreatedObjects,
    Other,
}

type Result<T = ()> = std::result::Result<T, String>;

#[derive(Clone, Default)]
struct Scope<'a> {
    slots: BTreeMap<u16, &'a TargetSpec>,
    results: BTreeMap<u32, ResultKind>,
    result_zones: BTreeMap<u32, Vec<EventZone>>,
    activation_cost: Option<Cost>,
    announcing: bool,
    casting_costs: BTreeMap<u32, Cost>,
    payment_costs: BTreeMap<u32, Cost>,
    ordered_results: BTreeSet<u32>,
    bound_objects: BTreeSet<u32>,
    singular: BTreeSet<u32>,
    /// Picks gathered over a player loop: a set, never one object.
    gathered: BTreeSet<u32>,
    repeatable: bool,
    open_searches: Vec<PlayerRef>,
    /// Inside `Operation::Clash`, where `PlayerRef::ClashOpponent` has a value.
    clashing: bool,
    search_libraries: BTreeMap<u32, PlayerRef>,
    event: crate::semantics::EventBindings,
    affected: bool,
    affected_player: bool,
    replacement: bool,
    players: BTreeSet<u32>,
    declaration: bool,
    candidate: bool,
    ward_payer: bool,
    keyword_host: bool,
    definition: bool,
}

impl<'a> Scope<'a> {
    fn paid_component(&self, payment: &PaymentRef, path: &[u32]) -> Result<&CostPart> {
        let cost = match payment {
            PaymentRef::Activation => self.activation_cost.as_ref(),
            PaymentRef::Casting { ability } => self.casting_costs.get(ability),
            PaymentRef::Resolution(result) => self.payment_costs.get(&result.0),
            PaymentRef::Branch => self.casting_costs.get(&BRANCH_COST),
        }
        .ok_or("payment history has no declaration in this scope")?;
        crate::cost_history::component(cost, path)
            .ok_or("payment history has an invalid component path".into())
    }
    fn singular_history(&self, objects: &ObjectSet, index: u32) -> bool {
        // "The second card exiled this way": what was performed keeps its order.
        if let ObjectSet::CostObjects {
            payment,
            component,
            view: CostObjectView::Performed,
            phase: ObjectPhase::After,
        } = objects
        {
            let count = self
                .paid_component(payment, component)
                .ok()
                .and_then(crate::cost_history::exact_count);
            if let Some(count) = count {
                return i64::from(index) < count;
            }
        }
        if index != 0 {
            return false;
        }
        if let ObjectSet::InZone { objects, .. } = objects {
            return matches!(**objects, ObjectSet::One(_));
        }
        // "The target card in that player's graveyard": one per owner.
        if let ObjectSet::Matching {
            objects,
            filter: ObjectFilter::OwnedBy(PlayerSet::One(_)),
        } = objects
        {
            if matches!(**objects, ObjectSet::Targets(_)) {
                return true;
            }
        }
        // "The exiled card": one card held by a linked ability (607.2a).
        if matches!(
            objects,
            ObjectSet::Zone {
                filter: ObjectFilter::ExiledWith(ObjectRef::Source),
                ..
            }
        ) {
            return true;
        }
        if let ObjectSet::Transition { result, .. } | ObjectSet::Observed(result) = objects {
            return self.singular.contains(&result.0);
        }
        // "The spell cast this way".
        if matches!(objects, ObjectSet::Affected(_)) {
            return true;
        }
        let ObjectSet::CostObjects {
            payment,
            component,
            view,
            phase,
        } = objects
        else {
            return false;
        };
        if !matches!(
            (view, phase),
            (CostObjectView::Inputs, ObjectPhase::Before)
                | (CostObjectView::Performed, ObjectPhase::After)
        ) {
            return false;
        }
        self.paid_component(payment, component)
            .is_ok_and(crate::cost_history::singular)
    }
    fn zone_scope(&self, scope: &ZoneScope) -> Result {
        match scope {
            ZoneScope::Shared(
                EventZone::Battlefield
                | EventZone::Exile
                | EventZone::Stack
                | EventZone::Command
                | EventZone::Ante,
            ) => Ok(()),
            ZoneScope::Players {
                zone:
                    EventZone::Graveyard | EventZone::Hand | EventZone::Library | EventZone::PlanarDeck,
                players,
            } => self.players(players),
            _ => Err("zone query uses a shared/private zone with the wrong scope".into()),
        }
    }
    fn domain(&self, domain: &ObjectDomain) -> Result {
        if domain.zones.is_empty() {
            return Err("object target domain has no zones".into());
        }
        for (i, zone) in domain.zones.iter().enumerate() {
            if domain.zones[..i].contains(zone) {
                return Err("object target domain repeats a zone".into());
            }
        }
        match domain.kind {
            ObjectKind::Permanent if *domain != ObjectDomain::permanent() => {
                Err("permanent domain must be the battlefield".into())
            }
            ObjectKind::Spell if *domain != ObjectDomain::spell() => {
                Err("spell domain must be the stack".into())
            }
            _ => Ok(()),
        }
    }
    fn ordered(&self, objects: &ObjectSet) -> bool {
        match objects {
            ObjectSet::Observed(result) | ObjectSet::Transition { result, .. } => {
                self.ordered_results.contains(&result.0)
            }
            ObjectSet::Difference { objects, .. }
            | ObjectSet::Matching { objects, .. }
            | ObjectSet::InZone { objects, .. } => self.ordered(objects),
            ObjectSet::Zone {
                scope:
                    ZoneScope::Players {
                        zone: EventZone::Library | EventZone::Graveyard | EventZone::PlanarDeck,
                        players: PlayerSet::One(_),
                    },
                ..
            } => true,
            ObjectSet::Take { objects, .. } => self.ordered(objects),
            _ => false,
        }
    }
    fn known_zones(&self, objects: &ObjectSet) -> Option<Vec<EventZone>> {
        match objects {
            ObjectSet::CostObjects {
                payment,
                component,
                phase,
                ..
            } => crate::cost_history::zone(self.paid_component(payment, component).ok()?, phase)
                .map(|z| vec![z]),
            ObjectSet::Transition { result, .. }
            | ObjectSet::MovedBy { result, .. }
            | ObjectSet::Observed(result)
            | ObjectSet::Unselected(result)
            | ObjectSet::Selected(result) => self.result_zones.get(&result.0).cloned(),
            ObjectSet::Difference { objects, .. } | ObjectSet::Matching { objects, .. } => {
                self.known_zones(objects)
            }
            ObjectSet::One(ObjectRef::Result { result, .. })
                if matches!(
                    self.results.get(&result.0),
                    Some(ResultKind::Selection | ResultKind::Search)
                ) =>
            {
                self.result_zones.get(&result.0).cloned()
            }
            ObjectSet::Battlefield(_) | ObjectSet::Created(_) => Some(vec![EventZone::Battlefield]),
            ObjectSet::Zone { scope, .. } => Some(vec![match scope {
                ZoneScope::Shared(z) | ZoneScope::Players { zone: z, .. } => z.clone(),
            }]),
            ObjectSet::InZone { zone, .. } => Some(vec![zone.clone()]),
            ObjectSet::Take { objects, .. } => self.known_zones(objects),
            ObjectSet::Targets(slot) | ObjectSet::One(ObjectRef::Target { slot, .. }) => {
                match &self.slots.get(&slot.0)?.kind {
                    TargetKind::Object(domain) => Some(domain.zones.clone()),
                    _ => None,
                }
            }
            ObjectSet::One(ObjectRef::Query { objects, .. }) => self.known_zones(objects),
            ObjectSet::Union(sets) => {
                let mut zones = Vec::new();
                for set in sets {
                    zones.extend(self.known_zones(set)?);
                }
                Some(zones)
            }
            _ => None,
        }
    }
    fn require_zone(&self, objects: &ObjectSet, zone: EventZone) -> Result {
        self.objects(objects)?;
        if self
            .known_zones(objects)
            .is_some_and(|zones| zones.iter().any(|z| *z != zone))
        {
            return Err(format!("operation requires objects in {zone:?}"));
        }
        Ok(())
    }
    fn replacement_event(&self, event: &ReplacementEvent) -> Result {
        match event {
            ReplacementEvent::Enter { objects } => self.filter(objects),
            ReplacementEvent::Discard {
                players,
                causes,
                card,
                ..
            } => {
                self.players(players)?;
                causes.iter().chain(card).try_for_each(|f| self.filter(f))
            }
            ReplacementEvent::Damage(damage) => self.event(&EventSpec::Damage(damage.clone())),
            ReplacementEvent::ZoneChange { objects, .. } => self.filter(objects),
            ReplacementEvent::CopySpell { players, spells } => {
                self.players(players)?;
                self.filter(spells)
            }
            ReplacementEvent::Countered { spells, by } => {
                self.filter(spells)?;
                self.filter(by)
            }
            ReplacementEvent::DrawCausedBy { players, abilities } => {
                self.players(players)?;
                self.filter(abilities)
            }
            ReplacementEvent::ProduceColor { sources, .. } => self.filter(sources),
            ReplacementEvent::Primitive { name, operands } => self.primitive(name, operands),
            ReplacementEvent::Player { players, .. } => self.players(players),
            ReplacementEvent::Object { objects, .. } => self.filter(objects),
            ReplacementEvent::Tokens {
                tokens,
                controllers,
                creators,
                ..
            } => {
                self.filter(tokens)?;
                controllers
                    .iter()
                    .chain(creators)
                    .try_for_each(|players| self.players(players))
            }
            ReplacementEvent::Counters {
                objects,
                players,
                by,
                ..
            } => {
                self.filter(objects)?;
                players
                    .iter()
                    .chain(by)
                    .try_for_each(|players| self.players(players))
            }
            ReplacementEvent::ProduceMana { sources, by } => {
                self.filter(sources)?;
                by.iter().try_for_each(|player| self.player(player))
            }
            ReplacementEvent::EachPoint(event) => self.replacement_event(event),
            ReplacementEvent::Measured { event, amount, .. } => {
                self.replacement_event(event)?;
                let mut scope = self.clone();
                scope.event = crate::semantics::with_replacement(&self.event, event);
                scope.expr(amount)
            }
            ReplacementEvent::Caused { cause, event } => {
                self.filter(cause)?;
                self.replacement_event(event)
            }
            ReplacementEvent::PutOntoBattlefield { players, objects } => {
                self.players(players)?;
                self.filter(objects)
            }
            ReplacementEvent::FirstEachTurn(event) => self.replacement_event(event),
            ReplacementEvent::Either(events) => {
                if events.is_empty() {
                    return Err("replacement event alternatives are empty".into());
                }
                events.iter().try_for_each(|e| self.replacement_event(e))
            }
        }
    }
    fn event(&self, event: &EventSpec) -> Result {
        match event {
            EventSpec::Primitive { name, operands } => self.primitive(name, operands),
            EventSpec::ClassLevel { objects, level } => {
                self.filter(objects)?;
                self.expr(level)
            }
            EventSpec::FirstEachGame(event)
            | EventSpec::FirstEachTurn(event)
            | EventSpec::FirstForObjectEachTurn(event) => self.event(event),
            EventSpec::Caused { cause, event } => {
                self.filter(cause)?;
                self.event(event)
            }
            EventSpec::WhileActivating {
                event,
                players,
                abilities,
            } => {
                self.event(event)?;
                self.players(players)?;
                self.filter(abilities)
            }
            EventSpec::ZoneChangeFromElsewhere { objects, .. } => self.filter(objects),
            EventSpec::AttackUnblocked {
                attackers, count, ..
            } => {
                self.filter(attackers)?;
                count.iter().try_for_each(|(_, n)| self.expr(n))
            }
            EventSpec::AllAttack(objects)
            | EventSpec::OntoStack(objects)
            | EventSpec::Resolves(objects) => self.filter(objects),
            EventSpec::CausesTrigger { cause, .. } => self.event(cause),
            EventSpec::AttacksWith {
                object,
                others,
                count,
                by,
                ..
            } => {
                self.object(object)?;
                self.filter(others)?;
                self.expr(&count.1)?;
                by.iter().try_for_each(|p| self.players(p))
            }
            EventSpec::AttacksAsGroup { by, attackers, .. } => {
                self.players(by)?;
                self.filter(attackers)
            }
            EventSpec::SpellTargeting {
                players,
                spells,
                targets,
                count,
            } => {
                self.players(players)?;
                self.filter(spells)?;
                targets.iter().try_for_each(|t| self.objects(t))?;
                count.iter().try_for_each(|(_, n)| self.expr(n))
            }
            EventSpec::SpellOrActivation {
                players,
                spells,
                abilities,
            } => {
                self.players(players)?;
                self.filter(spells)?;
                self.filter(abilities)
            }
            EventSpec::PaysToActivate {
                players, abilities, ..
            } => {
                self.players(players)?;
                self.filter(abilities)
            }
            EventSpec::ObjectBecomes(objects) => self.filter(objects),
            EventSpec::ExploresCard { objects, cards } => {
                self.filter(objects)?;
                self.filter(cards)
            }
            EventSpec::TappedForManaOf { objects, by, .. } => {
                self.filter(objects)?;
                by.iter().try_for_each(|p| self.players(p))
            }
            EventSpec::ManaAdded {
                abilities, players, ..
            } => {
                self.filter(abilities)?;
                self.players(players)
            }
            EventSpec::TargetsChosen { players, objects } => {
                self.players(players)?;
                self.filter(objects)
            }
            EventSpec::ControlChanges { objects, to, from } => {
                self.filter(objects)?;
                self.players(to)?;
                self.players(from)
            }
            EventSpec::CountersOnPlayer { by, players } => {
                self.players(by)?;
                self.players(players)
            }
            EventSpec::NthCounter {
                kind, objects, nth, ..
            } => {
                self.operand(kind)?;
                self.filter(objects)?;
                self.expr(nth)
            }
            EventSpec::Player {
                players, objects, ..
            } => {
                let mut scoped = self.clone();
                scoped.event = crate::semantics::with_event(&self.event, event);
                scoped.players(players)?;
                objects.iter().try_for_each(|filter| scoped.filter(filter))
            }
            EventSpec::NthEachTurn { event, value, .. } => {
                self.event(event)?;
                self.expr(value)
            }
            // Its filters can name what the event binds, as a row's operands can.
            EventSpec::Pair {
                objects, others, ..
            } => {
                let mut scoped = self.clone();
                scoped.event = crate::semantics::with_event(&self.event, event);
                scoped.filter(objects)?;
                scoped.filter(others)
            }
            EventSpec::BecomesTarget { objects, by, .. } => {
                let mut scoped = self.clone();
                scoped.event = crate::semantics::with_event(&self.event, event);
                scoped.filter(objects)?;
                scoped.operand(by)
            }
            EventSpec::Attack {
                attackers,
                by,
                defenders,
            } => {
                let mut scoped = self.clone();
                scoped.event = crate::semantics::with_event(&self.event, event);
                scoped.filter(attackers)?;
                if let Some(by) = by {
                    self.players(by)?;
                }
                self.defenders(defenders)
            }
            EventSpec::Batch {
                event: inner,
                count,
                including,
                ..
            } => {
                self.event(inner)?;
                if let Some((_, value)) = count {
                    self.expr(value)?;
                }
                match including {
                    Some(object) => self.object(object),
                    None => Ok(()),
                }
            }
            EventSpec::Counters { objects, by, .. } => {
                let mut scoped = self.clone();
                scoped.event = crate::semantics::with_event(&self.event, event);
                scoped.filter(objects)?;
                match by {
                    Some(by) => self.players(by),
                    None => Ok(()),
                }
            }
            EventSpec::Activated { players, abilities } => {
                self.players(players)?;
                let mut scoped = self.clone();
                scoped.event = crate::semantics::with_event(&self.event, event);
                scoped.filter(abilities)
            }
            EventSpec::SagaChapter(_) => Ok(()),
            EventSpec::Discard { players, cards, .. } => {
                self.players(players)?;
                self.filter(cards)
            }
            EventSpec::ZoneChange { objects, .. } => self.filter(objects),
            EventSpec::Object { objects, .. } => self.filter(objects),
            EventSpec::Spell {
                players, spells, ..
            } => {
                self.players(players)?;
                self.filter(spells)
            }
            EventSpec::Turn { players, .. } => self.players(players),
            EventSpec::Damage(event) => {
                self.filter(&event.sources)?;
                // A recipient can be named by the source that dealt the damage.
                let bound = crate::semantics::event_bindings(&EventSpec::Damage(event.clone()));
                let mut scoped = self.clone();
                scoped.event.damage_source |= bound.damage_source;
                match &event.recipients {
                    DamageRecipients::Any => Ok(()),
                    DamageRecipients::Target { slot, .. } | DamageRecipients::Targets(slot) => {
                        self.recipients(&Recipients::Targets(slot.clone()))
                    }
                    DamageRecipients::Objects(f) => scoped.filter(f),
                    DamageRecipients::Players(p) => scoped.players(p),
                    DamageRecipients::ObjectsAndPlayers { objects, players } => {
                        scoped.filter(objects)?;
                        scoped.players(players)
                    }
                }
            }
            EventSpec::State { condition } => self.condition(condition),
            EventSpec::Either(events) => {
                if events.is_empty() {
                    return Err("alternative event matcher is empty".into());
                }
                for event in events {
                    self.event(event)?;
                }
                Ok(())
            }
            EventSpec::Filtered { event, condition } => {
                self.event(event)?;
                let mut event_scope = self.clone();
                event_scope.event = crate::semantics::event_bindings(event);
                event_scope.condition(condition)
            }
        }
    }

    fn target(&self, slot: &Slot, index: Option<u16>, kind: TargetKind) -> Result {
        let spec = self
            .slots
            .get(&slot.0)
            .ok_or_else(|| format!("unbound target slot {}", slot.0))?;
        if std::mem::discriminant(&spec.kind) != std::mem::discriminant(&kind) {
            return Err(format!("target slot {} has the wrong entity kind", slot.0));
        }
        if let (
            Some(index),
            Count::Exactly(Expr::Constant(max)) | Count::UpTo(Expr::Constant(max)),
        ) = (index, &spec.count)
        {
            if i64::from(index) >= *max {
                return Err(format!("target index {index} exceeds slot {}", slot.0));
            }
        }
        Ok(())
    }

    fn object(&self, object: &ObjectRef) -> Result {
        match object {
            ObjectRef::Scoped => Ok(()),
            ObjectRef::RingBearer(player) => self.player(player),
            ObjectRef::HauntedBy(object) => self.object(object),
            ObjectRef::TheOther { among, excluding } => {
                self.objects(among)?;
                self.objects(excluding)
            }
            ObjectRef::AtEnd { objects, .. } => self.objects(objects),
            ObjectRef::NthChosen { by, .. } => by.iter().try_for_each(|player| self.player(player)),
            ObjectRef::Named { result, by } => {
                if !self.results.contains_key(&result.0) {
                    return Err(format!("result {} has not run", result.0));
                }
                by.iter().try_for_each(|player| self.player(player))
            }
            ObjectRef::Earlier { by, .. } => by.iter().try_for_each(|player| self.player(player)),
            ObjectRef::SourceOf(ability) => self.object(ability),
            ObjectRef::Discarded if self.event.discarded => Ok(()),
            ObjectRef::Discarded => {
                Err("discard event does not bind a singular discarded card".into())
            }
            ObjectRef::Damage(role) => match role {
                DamageSubject::Source if self.event.damage_source => Ok(()),
                DamageSubject::Recipient if self.event.damage_recipient => Ok(()),
                _ => Err("damage event does not bind a singular object for this role".into()),
            },
            ObjectRef::Query { objects, index } => {
                self.objects(objects)?;
                if !self.ordered(objects) && !self.singular_history(objects, *index) {
                    return Err("indexed query requires one ordered zone".into());
                }
                Ok(())
            }
            ObjectRef::Candidate if self.candidate => Ok(()),
            ObjectRef::Candidate => Err("filter candidate used outside an object predicate".into()),
            ObjectRef::Source => Ok(()),
            ObjectRef::MadeHere | ObjectRef::Granter => Ok(()),
            ObjectRef::ChosenBy { result, chooser } => {
                self.player(chooser)?;
                match self.results.get(&result.0) {
                    Some(ResultKind::Selection | ResultKind::Search) => Ok(()),
                    _ => Err(format!(
                        "result {} is not an available object selection",
                        result.0
                    )),
                }
            }
            ObjectRef::KeywordHost if self.keyword_host => Ok(()),
            ObjectRef::KeywordHost => {
                Err("keyword host used outside its keyword definition".into())
            }
            ObjectRef::SoleTargetOf(spell) => self.object(spell),
            ObjectRef::AttachedTo(attachment) | ObjectRef::PairedWith(attachment) => {
                self.object(attachment)
            }
            ObjectRef::Affected if self.affected => Ok(()),
            ObjectRef::Affected => {
                Err("affected object used outside a characteristic change".into())
            }
            ObjectRef::Target { slot, index } => self.target(
                slot,
                Some(*index),
                TargetKind::Object(ObjectDomain::permanent()),
            ),
            ObjectRef::EventSubject if self.event.object => Ok(()),
            ObjectRef::EventOther if self.event.other => Ok(()),
            ObjectRef::EventOther => Err("this event names no second object".into()),
            ObjectRef::EventStackObject if self.event.stack_object => Ok(()),
            ObjectRef::EventStackObject => Err("this event names no spell or ability".into()),
            ObjectRef::RecipientObject { slot, index } => {
                self.target(slot, Some(*index), TargetKind::DamageRecipient)
            }
            ObjectRef::BlockPair(_) if self.event.block_pair => Ok(()),
            ObjectRef::BlockPair(_) => Err("this event pairs no attacker and blocker".into()),
            ObjectRef::EventDestination if self.event.moved => Ok(()),
            ObjectRef::EventDestination => Err("this event moves no object".into()),
            ObjectRef::Chosen => Ok(()),
            ObjectRef::Primitive { name, operands } => self.primitive(name, operands),
            ObjectRef::Result {
                result,
                phase: ObjectPhase::After,
                ..
            } if matches!(
                self.results.get(&result.0),
                Some(ResultKind::CreatedObjects)
            ) =>
            {
                Ok(())
            }
            ObjectRef::Result { result, .. } if self.gathered.contains(&result.0) => {
                Err("a player loop's choice is a set, not one object".into())
            }
            ObjectRef::Result { result, .. }
                if matches!(
                    self.results.get(&result.0),
                    Some(ResultKind::Selection | ResultKind::Search | ResultKind::ObjectTransition)
                ) =>
            {
                Ok(())
            }
            ObjectRef::Bound(binding) if self.bound_objects.contains(&binding.0) => Ok(()),
            ObjectRef::Bound(_) => Err("object binding has no declaration".into()),
            ObjectRef::EventSubject => Err("event subject used outside a triggered ability".into()),
            ObjectRef::Result { result, .. } => Err(format!(
                "result {} does not provide objects in this scope",
                result.0
            )),
        }
    }

    fn entering_changes(&self, changes: &[Change]) -> Result {
        let mut scope = self.clone();
        scope.affected = true;
        changes.iter().try_for_each(|c| scope.change(c))
    }

    fn entry(&self, entry: &EntrySpec) -> Result {
        for (_, amount) in &entry.counters {
            self.count(&Count::Exactly(amount.clone()))?;
        }
        entry.states.iter().try_for_each(|state| match state {
            EntryState::CopyOf {
                objects,
                effects,
                until,
            } => {
                self.objects(objects)?;
                until.iter().try_for_each(|d| self.duration(d))?;
                self.operand(effects)
            }
            EntryState::CountersOfChoice { amount, .. } => {
                self.count(&Count::Exactly(amount.clone()))
            }
            EntryState::CountersOf { object, .. } => self.object(object),
            EntryState::Transformed
            | EntryState::Prepared
            | EntryState::FaceDown
            | EntryState::Flipped
            | EntryState::NotAnAura
            | EntryState::NotedCounters
            | EntryState::FaceDownAs(_)
            | EntryState::FaceDownLand(_)
            | EntryState::May(_)
            | EntryState::ExilePile { .. } => Ok(()),
            EntryState::FaceDownWith { noted, .. } => {
                noted.iter().try_for_each(|card| self.object(card))
            }
            EntryState::Perpetual(changes) => {
                changes.iter().try_for_each(|change| self.change(change))
            }
            EntryState::ExiledFor(until) | EntryState::RulesUntil { until, .. } => {
                self.duration(until)
            }
            EntryState::ExiledForWith { until, objects, .. } => {
                self.duration(until)?;
                self.objects(objects)
            }
            EntryState::Attacking { player, .. } => player.iter().try_for_each(|p| self.player(p)),
            EntryState::Blocking(object) | EntryState::AttachedTo(object) => self.object(object),
            EntryState::AttachedToChosen(filter) => self.filter(filter),
            EntryState::AttachedToPlayer(player) => self.player(player),
            EntryState::Primitive { name, operands } => self.primitive(name, operands),
            EntryState::Changes(changes) => self.entering_changes(changes),
            EntryState::ChooseChanges(options) => options
                .iter()
                .try_for_each(|changes| self.entering_changes(changes)),
            EntryState::When { filter, entry } => {
                self.filter(filter)?;
                self.entry(entry)
            }
            EntryState::ChangesUntil { changes, duration } => {
                self.duration(duration)?;
                self.entering_changes(changes)
            }
        })
    }

    fn duration(&self, duration: &Duration) -> Result {
        match duration {
            Duration::UntilEvent(event) => self.event(event),
            Duration::While(condition) => self.condition(condition),
            Duration::WhileItMatches(filter) => self.filter(filter),
            Duration::ThroughNextTurn | Duration::UntilStateBasedActionsChecked => Ok(()),
            Duration::DuringControllersNextUntapSteps(steps) => self.expr(steps),
            Duration::EndOfCombatOnNextTurn(player)
            | Duration::UntilNextUntapStep(player)
            | Duration::UntilLibraryTopChanges(player)
            | Duration::WhilePlayerHasCounter { player, .. } => self.player(player),
            Duration::UntilAnotherExiled { by, with } => {
                self.players(by)?;
                self.object(with)
            }
            Duration::ExtraCombat(result) if self.results.contains_key(&result.0) => Ok(()),
            Duration::ExtraCombat(result) => Err(format!("result {} is unavailable", result.0)),
            Duration::Primitive { name, operands } => self.primitive(name, operands),
            Duration::ExtraTurn(result) if self.results.contains_key(&result.0) => Ok(()),
            Duration::ExtraTurn(result) => Err(format!("result {} is unavailable", result.0)),
            Duration::EndOfTurn
            | Duration::EndOfCombat
            | Duration::Indefinite
            | Duration::DuringControllersNextUntapStep => Ok(()),
            Duration::UntilNextTurn(player)
            | Duration::UntilEndOfNextTurn(player)
            | Duration::DuringNextUntapStep(player)
            | Duration::DuringNextTurn(player) => self.player(player),
            Duration::WhileOnBattlefield(object) | Duration::WhileExiled(object) => {
                self.object(object)
            }
            Duration::WhileControlled { player, object } => {
                self.player(player)?;
                self.object(object)
            }
            Duration::UntilPaid { player, cost } => {
                self.player(player)?;
                self.cost(cost)
            }
            Duration::WhileAnyExiled(objects) => self.objects(objects),
            Duration::UntilCast(object) => self.object(object),
            Duration::WhileMatches { object, filter } => {
                self.object(object)?;
                self.filter(filter)
            }
            Duration::UntilNext { players, .. } => self.players(players),
            Duration::Either(durations) => durations.iter().try_for_each(|d| self.duration(d)),
        }
    }

    fn player(&self, player: &PlayerRef) -> Result {
        match player {
            PlayerRef::NotedFor(object) => self.object(object),
            PlayerRef::OtherChosen(player) => self.player(player),
            PlayerRef::AsOf { moment, player } => {
                if let Moment::LastEvent { event, .. } = moment.as_ref() {
                    self.event(event)?;
                }
                self.player(player)
            }
            PlayerRef::InChosenDirection { from, among } => {
                from.iter().try_for_each(|player| self.player(player))?;
                among.iter().try_for_each(|players| self.players(players))
            }
            PlayerRef::Named { result, .. } if self.results.contains_key(&result.0) => Ok(()),
            PlayerRef::Named { result, .. } => Err(format!("result {} has not run", result.0)),
            PlayerRef::Prior(_) => Ok(()),
            PlayerRef::Seat { of, .. } => self.player(of),
            PlayerRef::AttackingPlayer | PlayerRef::DefendingPlayer | PlayerRef::Holder(_) => {
                Ok(())
            }
            PlayerRef::Extreme { players, .. } => self.players(players),
            PlayerRef::SoleTargetOf(object) => self.object(object),
            PlayerRef::WardPayer if self.ward_payer => Ok(()),
            PlayerRef::WardPayer => Err("ward payer used outside a ward cost".into()),
            PlayerRef::Affected if self.affected_player => Ok(()),
            PlayerRef::Affected => {
                Err("affected player used outside a player continuous effect".into())
            }
            PlayerRef::EnchantedBy(object) => self.object(object),
            PlayerRef::PlayerOrControllerOf(recipient) => self.recipients(recipient),
            PlayerRef::Acting | PlayerRef::GiftedPlayer => Ok(()),
            PlayerRef::Performer(result) => match self.results.get(&result.0) {
                Some(ResultKind::Payment | ResultKind::Selection) => Ok(()),
                _ => Err(format!("result {} has no one who performed it", result.0)),
            },
            PlayerRef::ClashOpponent if self.clashing => Ok(()),
            PlayerRef::ClashOpponent => Err("a clash opponent outside a clash".into()),
            PlayerRef::DeclaringPlayer if self.declaration => Ok(()),
            PlayerRef::DeclaringPlayer => {
                Err("declaring player used outside a combat declaration cost".into())
            }
            PlayerRef::AbilityController | PlayerRef::Active => Ok(()),
            PlayerRef::Chosen | PlayerRef::ModeChooser => Ok(()),
            PlayerRef::Primitive { name, operands } => self.primitive(name, operands),
            PlayerRef::FirstWilling(players) | PlayerRef::OneOf(players) => self.players(players),
            PlayerRef::CasterOf(object)
            | PlayerRef::ControllerOf(object)
            | PlayerRef::OwnerOf(object)
            | PlayerRef::DefendingPlayerOf(object) => self.object(object),
            PlayerRef::Target { slot, index } => {
                self.target(slot, Some(*index), TargetKind::Player)
            }
            PlayerRef::EventOtherPlayer if self.event.other_player => Ok(()),
            PlayerRef::EventOtherPlayer => Err("this event names no second player".into()),
            PlayerRef::EventSubject if self.event.player => Ok(()),
            PlayerRef::EventSubject => Err("this event does not bind a player subject".into()),
            PlayerRef::Bound(binding) if self.players.contains(&binding.0) => Ok(()),
            PlayerRef::Bound(_) => Err("player binding has no declaration".into()),
        }
    }

    fn players(&self, players: &PlayerSet) -> Result {
        match players {
            PlayerSet::Scoped => Ok(()),
            PlayerSet::Intersection(sets) => sets.iter().try_for_each(|s| self.players(s)),
            PlayerSet::Matching { binding, condition } => {
                let mut child = self.clone();
                if !child.players.insert(binding.0) {
                    return Err("player predicate redeclares a binding".into());
                }
                child.condition(condition)
            }
            PlayerSet::Union(sets) => {
                for players in sets {
                    self.players(players)?;
                }
                Ok(())
            }
            PlayerSet::DamageRecipients if self.event.damage_players => Ok(()),
            PlayerSet::DamageRecipients => {
                Err("damage event does not bind player recipients".into())
            }
            PlayerSet::One(p) | PlayerSet::OpponentsOf(p) | PlayerSet::OthersThan(p) => {
                self.player(p)
            }
            PlayerSet::DamagedBy(result) if self.results.contains_key(&result.0) => Ok(()),
            PlayerSet::DamagedBy(result) => Err(format!("result {} is unavailable", result.0)),
            PlayerSet::Targets(slot) => self.target(slot, None, TargetKind::Player),
            PlayerSet::All => Ok(()),
        }
    }

    fn objects(&self, objects: &ObjectSet) -> Result {
        match objects {
            ObjectSet::Scoped => Ok(()),
            ObjectSet::Earlier { by, .. } => {
                by.iter().try_for_each(|players| self.players(players))
            }
            ObjectSet::Named(result) if self.results.contains_key(&result.0) => Ok(()),
            ObjectSet::Named(result) => Err(format!("result {} is unavailable", result.0)),
            ObjectSet::DamageSources(filter)
            | ObjectSet::AnyZone(filter)
            | ObjectSet::OutsideTheGame(filter) => self.filter(filter),
            ObjectSet::LibraryTops(players) => self.players(players),
            ObjectSet::SplicedOnto(object) | ObjectSet::TargetsOf(object) => self.object(object),
            ObjectSet::FromHistory { event, .. } => self.event(event),
            ObjectSet::Random { objects, count } => {
                self.objects(objects)?;
                self.expr(count)
            }
            ObjectSet::Chosen {
                chooser,
                objects,
                count,
            } => {
                self.players(chooser)?;
                self.objects(objects)?;
                self.count(count)
            }
            ObjectSet::CostObjects {
                payment,
                component,
                view,
                phase,
            } => {
                self.paid_component(payment, component)?;
                if self.announcing {
                    let known = matches!(
                        (payment, view, phase),
                        (
                            PaymentRef::Activation
                                | PaymentRef::Casting { .. }
                                | PaymentRef::Branch,
                            CostObjectView::Inputs,
                            ObjectPhase::Before
                        )
                    );
                    if !known {
                        return Err("payment history is unavailable during announcement".into());
                    }
                }
                if matches!(view, CostObjectView::Inputs) && !matches!(phase, ObjectPhase::Before) {
                    return Err("payment input history requires original identities".into());
                }
                Ok(())
            }
            ObjectSet::Observed(result) => match self.results.get(&result.0) {
                Some(ResultKind::Observation) => Ok(()),
                _ => Err("observation requires an available LookAt result".into()),
            },
            ObjectSet::Matching { objects, filter } => {
                self.objects(objects)?;
                self.filter(filter)
            }
            ObjectSet::Difference { objects, excluding } => {
                self.objects(objects)?;
                self.objects(excluding)
            }
            ObjectSet::Damage(DamageSubject::Source) if self.event.damage => Ok(()),
            ObjectSet::Damage(DamageSubject::Recipient) if self.event.damage_objects => Ok(()),
            ObjectSet::EventGroup if self.event.group => Ok(()),
            ObjectSet::EventGroup => Err("this event groups no objects".into()),
            ObjectSet::Damage(_) => Err("damage event does not bind this object collection".into()),
            ObjectSet::Zone {
                scope,
                kind,
                filter,
            } => {
                self.zone_scope(scope)?;
                let zone = match scope {
                    ZoneScope::Shared(z) | ZoneScope::Players { zone: z, .. } => z.clone(),
                };
                self.domain(&ObjectDomain {
                    kind: kind.clone(),
                    zones: vec![zone],
                })?;
                self.filter(filter)
            }
            ObjectSet::Union(sets) => {
                for set in sets {
                    self.objects(set)?;
                }
                Ok(())
            }
            ObjectSet::Take { objects, count, .. } => {
                self.objects(objects)?;
                if !self.ordered(objects) {
                    return Err("top/bottom selection requires one ordered zone".into());
                }
                self.count(&Count::Exactly(count.clone()))
            }
            ObjectSet::Transition { result, .. } => match self.results.get(&result.0) {
                Some(ResultKind::ObjectTransition) => Ok(()),
                _ => Err("transition set requires an object-transition result".into()),
            },
            ObjectSet::MovedBy { result, player, .. } => {
                self.player(player)?;
                match self.results.get(&result.0) {
                    Some(ResultKind::ObjectTransition) => Ok(()),
                    _ => Err("a player's share requires an object-transition result".into()),
                }
            }
            ObjectSet::Primitive { name, operands } => self.primitive(name, operands),
            ObjectSet::One(object) => self.object(object),
            // A loop over "any target" binds each one, player or permanent.
            ObjectSet::Targets(slot) => self
                .target(slot, None, TargetKind::Object(ObjectDomain::permanent()))
                .or_else(|_| self.target(slot, None, TargetKind::DamageRecipient)),
            ObjectSet::Battlefield(filter) => self.filter(filter),
            ObjectSet::InZone { objects, .. } => self.objects(objects),
            ObjectSet::Selected(result) | ObjectSet::Unselected(result) => {
                match self.results.get(&result.0) {
                    Some(ResultKind::Selection | ResultKind::Search) => Ok(()),
                    _ => Err(format!(
                        "result {} is not an available object selection",
                        result.0
                    )),
                }
            }
            ObjectSet::Affected(result) if self.results.contains_key(&result.0) => Ok(()),
            ObjectSet::Affected(result) => Err(format!("result {} is unavailable", result.0)),
            ObjectSet::Copies(result) | ObjectSet::Unpaid(result)
                if self.results.contains_key(&result.0) =>
            {
                Ok(())
            }
            ObjectSet::Copies(result) | ObjectSet::Unpaid(result) => {
                Err(format!("result {} is unavailable", result.0))
            }
            ObjectSet::Created(result) => match self.results.get(&result.0) {
                Some(ResultKind::CreatedObjects) => Ok(()),
                _ => Err(format!(
                    "result {} is not an available token-creation result",
                    result.0
                )),
            },
        }
    }

    fn recipients(&self, recipients: &Recipients) -> Result {
        match recipients {
            Recipients::DamagedByEvent if self.event.damage_any_recipient => Ok(()),
            Recipients::DamagedByEvent => Err("this event names no one damaged thing".into()),
            Recipients::CountersTheEventPut => self.object(&ObjectRef::EventSubject),
            Recipients::Objects(objects) => self.objects(objects),
            Recipients::Players(players) => self.players(players),
            Recipients::Targets(slot) | Recipients::Target { slot, .. } => {
                self.target(slot, None, TargetKind::DamageRecipient)
            }
            Recipients::Chosen => Ok(()),
            Recipients::AttackedBy(attacker) => self.object(attacker),
            Recipients::Union(items) => {
                for item in items {
                    self.recipients(item)?;
                }
                Ok(())
            }
        }
    }

    fn vote_option(&self, option: &VoteOption) -> Result {
        match option {
            VoteOption::Word(_) => Ok(()),
            VoteOption::Player(player) => self.player(player),
            VoteOption::Object(object) => self.object(object),
        }
    }

    fn player_number(&self, number: &PlayerNumber) -> Result {
        match number {
            PlayerNumber::Permanents(filter) => self.filter(filter),
            _ => Ok(()),
        }
    }

    fn expr(&self, expression: &Expr) -> Result {
        match expression {
            Expr::DamageBy(object) | Expr::ModesChosen(object) => self.object(object),
            Expr::AdditionalCostPaid(symbols) => {
                symbols.iter().try_for_each(|symbol| self.operand(symbol))
            }
            Expr::DraftNoted { note, .. } => self.player(&note.player),
            Expr::TurnsSinceForetold | Expr::ActivationCostSymbols(_) => Ok(()),
            Expr::ChosenColorsOf(object) => self.object(object),
            Expr::AbilitiesAmong { objects, .. } => self.objects(objects),
            Expr::Earlier {
                result, filter, by, ..
            } => {
                if let Some(result) = result {
                    if !self.results.contains_key(&result.0) {
                        return Err(format!("result {} has not run", result.0));
                    }
                }
                filter.iter().try_for_each(|f| self.filter(f))?;
                by.iter().try_for_each(|p| self.player(p))
            }
            Expr::Dice(DiceMeasure::Matching(_, value)) => self.expr(value),
            Expr::Dice(_) | Expr::CoinFlipsWon(_) | Expr::Game(_) => Ok(()),
            Expr::Stickers { objects, .. } => self.objects(objects),
            Expr::Noted { object, .. } => object.iter().try_for_each(|o| self.object(o)),
            Expr::Difference(a, b) => {
                self.expr(a)?;
                self.expr(b)
            }
            Expr::LargestSharedGroup { objects, .. } | Expr::SharingWithAnother { objects, .. } => {
                self.objects(objects)
            }
            Expr::ManaSpentFrom { object, sources } => {
                self.object(object)?;
                self.filter(sources)
            }
            Expr::AsOf { value, .. } => self.expr(value),
            Expr::Power(base, exponent) => {
                self.expr(base)?;
                self.expr(exponent)
            }
            Expr::TimesActivatedThisTurn | Expr::TurnNumber => Ok(()),
            Expr::Primitive { name, operands } => self.primitive(name, operands),
            Expr::ChosenParity | Expr::ChosenNumber | Expr::TimesResolvedThisTurn => Ok(()),
            Expr::PartySize(player) | Expr::StartingLife(player) => self.player(player),
            Expr::Votes(option) => self.vote_option(option),
            Expr::PlayerNumber { player, number } => {
                self.player(player)?;
                self.player_number(number)
            }
            Expr::AmongPlayers {
                players, number, ..
            } => {
                self.players(players)?;
                self.player_number(number)
            }
            Expr::TimesKicked(object) => self.object(object),
            Expr::TimesMutated(object) => self.object(object),
            Expr::ManaOf { filter, .. } => {
                if filter.colors.as_ref().is_some_and(Vec::is_empty) {
                    return Err("spent-mana type filter cannot be empty".into());
                }
                Ok(())
            }
            Expr::CountersPaid { object, .. } => self.object(object),
            Expr::SharedValues { object, with, .. } => {
                self.object(object)?;
                self.objects(with)
            }
            Expr::RepeatedCostsUnpaid { result, player } => {
                if !self.results.contains_key(&result.0) {
                    return Err(format!("result {} is unavailable", result.0));
                }
                self.player(player)
            }
            Expr::EventHistory { event, .. } => self.event(event),
            Expr::CountPlayers(players) => self.players(players),
            Expr::ManaSpent { object, filter, .. } => {
                self.object(object)?;
                if let Some(colors) = &filter.colors {
                    if colors.is_empty() {
                        return Err("spent-mana type filter cannot be empty".into());
                    }
                    for (i, color) in colors.iter().enumerate() {
                        if colors[..i].contains(color) {
                            return Err("spent-mana type filter repeats a type".into());
                        }
                    }
                }
                Ok(())
            }
            Expr::CountManaSymbols { objects, colors } => {
                if colors.is_empty() || colors.iter().any(|c| matches!(c, ManaColor::Colorless)) {
                    return Err("mana symbol color query requires actual colors".into());
                }
                self.objects(objects)
            }
            Expr::BoundValue(result) | Expr::CountersRemoved(result) => {
                if self.results.contains_key(&result.0) {
                    Ok(())
                } else {
                    Err("a bound value is read before it is set".into())
                }
            }
            Expr::AnnouncedX if self.definition => {
                Err("characteristic definition cannot use an unrelated announced X".into())
            }
            Expr::Aggregate { objects, .. } | Expr::CountDistinct { objects, .. } => {
                self.objects(objects)
            }
            Expr::EventActivationX if self.event.activation_x => Ok(()),
            Expr::EventActivationX => Err("event activation X used outside a cycling event".into()),
            Expr::Constant(_) | Expr::AnnouncedX => Ok(()),
            Expr::HeadsFlipped(result) => {
                if self.results.contains_key(&result.0) {
                    Ok(())
                } else {
                    Err(format!(
                        "result {} flipped no coins in this scope",
                        result.0
                    ))
                }
            }
            Expr::DieResult(result) => {
                if self.results.contains_key(&result.0) {
                    Ok(())
                } else {
                    Err(format!("result {} rolled no die in this scope", result.0))
                }
            }
            Expr::DistributedShare(slot) => match self.slots.get(&slot.0) {
                Some(target) if target.distribute.is_some() => Ok(()),
                _ => Err(format!("target slot {} announces no distribution", slot.0)),
            },
            Expr::ExcessDamage { result, to } if self.results.contains_key(&result.0) => {
                self.filter(to)
            }
            Expr::ExcessDamage { result, .. } => Err(format!("result {} is unavailable", result.0)),
            Expr::Performed(result)
            | Expr::PerformedTotal(result)
            | Expr::SurveilKeptOnTop(result)
            | Expr::ClampedDamage(result)
            | Expr::ChosenPowerToughness { result, .. }
            | Expr::DieResultAt { result, .. }
                if self.results.contains_key(&result.0) =>
            {
                Ok(())
            }
            Expr::ChosenPowerToughness { result, .. } | Expr::DieResultAt { result, .. } => {
                Err(format!("result {} is unavailable", result.0))
            }
            Expr::Performed(result)
            | Expr::PerformedTotal(result)
            | Expr::SurveilKeptOnTop(result)
            | Expr::ClampedDamage(result) => Err(format!("result {} is unavailable", result.0)),
            Expr::PaidCountersRemoved { payment, component } => {
                self.paid_component(payment, component).map(|_| ())
            }
            Expr::EventAmount if self.event.amount => Ok(()),
            Expr::EventAmount => Err("event amount used outside a damage event".into()),
            Expr::ObjectProperty {
                object,
                information,
                ..
            } => {
                if matches!(information, Information::Captured(_)) {
                    return Err("captured information has no binding declaration".into());
                }
                self.object(object)
            }
            Expr::PlayerLife(player) => self.player(player),
            Expr::AnyOf(values) => values.iter().try_for_each(|value| self.expr(value)),
            Expr::Add(a, b)
            | Expr::Subtract(a, b)
            | Expr::Multiply(a, b)
            | Expr::Minimum(a, b)
            | Expr::Maximum(a, b) => {
                self.expr(a)?;
                self.expr(b)
            }
            Expr::Conditional {
                condition,
                then,
                otherwise,
            } => {
                self.condition(condition)?;
                self.expr(then)?;
                self.expr(otherwise)
            }
            Expr::Divide {
                numerator,
                denominator,
                ..
            } => {
                if matches!(denominator.as_ref(), Expr::Constant(0)) {
                    return Err("division by zero".into());
                }
                self.expr(numerator)?;
                self.expr(denominator)
            }
            Expr::CountObjects(objects) => self.objects(objects),
            Expr::CountCounters {
                recipients,
                information,
                ..
            } => {
                if matches!(information, Information::Captured(_)) {
                    return Err("captured counter information has no binding declaration".into());
                }
                self.recipients(recipients)
            }
            Expr::CastX(object) => self.object(object),
        }
    }

    fn count(&self, count: &Count) -> Result {
        let value = match count {
            Count::Exactly(n) | Count::UpTo(n) | Count::AtLeast(n) | Count::AsManyAsCan(n) => n,
            Count::AnyNumber => return Ok(()),
            Count::Between { minimum, maximum } => {
                self.expr(minimum)?;
                maximum
            }
            Count::PerPlayer { players, each } => {
                self.players(players)?;
                return self.count(each);
            }
        };
        if matches!(value, Expr::Constant(n) if *n < 0) {
            return Err("negative selection count".into());
        }
        self.expr(value)
    }

    fn filter(&self, filter: &ObjectFilter) -> Result {
        let mut scope = self.clone();
        scope.candidate = true;
        scope.filter_inner(filter)
    }
    fn filter_inner(&self, filter: &ObjectFilter) -> Result {
        match filter {
            ObjectFilter::Noted(_) | ObjectFilter::NthChosen(_) => Ok(()),
            ObjectFilter::MostCommonCreatureTypeIn(player) => self.player(player),
            ObjectFilter::TargetCount(_, value) => self.expr(value),
            ObjectFilter::ChosenBy { player, .. } => self.player(player),
            ObjectFilter::ChosenOn { object, .. }
            | ObjectFilter::AttackingSame(object)
            | ObjectFilter::SharesTypeFrom { with: object, .. } => self.object(object),
            ObjectFilter::DraftNoted(note) => self.player(&note.player),
            ObjectFilter::InPile(object) | ObjectFilter::InSector(object) => {
                object.iter().try_for_each(|object| self.object(object))
            }
            ObjectFilter::CastFromAmong(objects) | ObjectFilter::SharesMostCommonColor(objects) => {
                self.objects(objects)
            }
            ObjectFilter::ArrivedThisTurn { .. } | ObjectFilter::ChosenFilter => Ok(()),
            ObjectFilter::SharesColorWith(object)
            | ObjectFilter::ExiledWith(object)
            | ObjectFilter::MadeBy(object)
            | ObjectFilter::WasAttachedTo(object) => self.object(object),
            ObjectFilter::DealtExcessBy(result) if self.results.contains_key(&result.0) => Ok(()),
            ObjectFilter::DealtExcessBy(result) => {
                Err(format!("result {} is unavailable", result.0))
            }
            ObjectFilter::DamagedBy(result) if self.results.contains_key(&result.0) => Ok(()),
            ObjectFilter::DamagedBy(result) => Err(format!("result {} is unavailable", result.0)),
            ObjectFilter::Primitive { name, operands } => self.primitive(name, operands),
            ObjectFilter::History { event, window } => {
                match event {
                    ObjectHistory::AttackedDefender(filter) | ObjectHistory::Enlisted(filter) => {
                        self.filter(filter)?
                    }
                    ObjectHistory::Drawn(Some(players))
                    | ObjectHistory::ExiledFromGraveyard(players) => self.players(players)?,
                    ObjectHistory::DamagedBy(object)
                    | ObjectHistory::Convoked(object)
                    | ObjectHistory::Exploited(object)
                    | ObjectHistory::TappedToPayFor(object)
                    | ObjectHistory::PhasedOutWith(object)
                    | ObjectHistory::CountersPutWith { with: object, .. }
                    | ObjectHistory::CountersRemovedWith { with: object, .. } => {
                        self.object(object)?
                    }
                    ObjectHistory::CountersPutBy { by, .. } => self.players(by)?,
                    _ => {}
                }
                match window {
                    ObjectWindow::LastTurnOf(player)
                    | ObjectWindow::SinceLastUpkeepOf(player)
                    | ObjectWindow::SinceLastTurnOf(player)
                    | ObjectWindow::LastCombatOf(player) => self.player(player),
                    _ => Ok(()),
                }
            }
            ObjectFilter::ManaSpent {
                sources,
                payers,
                value,
                ..
            } => {
                if let Some(sources) = sources {
                    self.filter(sources)?;
                }
                if let Some(payers) = payers {
                    self.players(payers)?;
                }
                self.expr(value)
            }
            ObjectFilter::AlternateCostPaid(operands) | ObjectFilter::ManaCostIs(operands) => {
                operands
                    .iter()
                    .try_for_each(|operand| self.operand(operand))
            }
            ObjectFilter::ManaSymbolInCost(_) | ObjectFilter::NameFirstPrintedIn(_) => Ok(()),
            ObjectFilter::InResult(result) if self.results.contains_key(&result.0) => Ok(()),
            ObjectFilter::InResult(result) => Err(format!("result {} is unavailable", result.0)),
            ObjectFilter::BlockingAny(filter)
            | ObjectFilter::BlockedByAny(filter)
            | ObjectFilter::PairedWith(filter)
            | ObjectFilter::CanTargetOnly(filter)
            | ObjectFilter::WouldDestroy(filter) => self.filter(filter),
            ObjectFilter::BandedWith(object)
            | ObjectFilter::TargetableBy(object)
            | ObjectFilter::CouldProduceManaOf(object) => self.object(object),
            ObjectFilter::AttachedCount {
                attachments, value, ..
            } => {
                self.filter(attachments)?;
                self.expr(value)
            }
            ObjectFilter::ProtectedBy(players) | ObjectFilter::InStartingDeck(players) => {
                self.players(players)
            }
            ObjectFilter::CouldEnchant(objects) => self.objects(objects),
            ObjectFilter::CouldProduce(operand) => self.operand(operand),
            ObjectFilter::ExiledBy { player, with, .. } => {
                self.player(player)?;
                match with {
                    Some(object) => self.object(object),
                    None => Ok(()),
                }
            }
            ObjectFilter::Condition(condition) => self.condition(condition),
            ObjectFilter::HasAttached(filter) => self.filter(filter),
            ObjectFilter::AttachedToPlayer(player) => self.player(player),
            ObjectFilter::Extreme { among, .. } => self.objects(among),
            ObjectFilter::ManaSpentFrom(sources) => self.filter(sources),
            ObjectFilter::TargetedBy(filter) => self.filter(filter),
            ObjectFilter::ColorOutsideCommanderIdentity(player) => self.player(player),
            ObjectFilter::SharesColorWithMana(_) | ObjectFilter::ManaCostPayableWith(_) => Ok(()),
            ObjectFilter::KickedWith(kicker) => {
                kicker.iter().try_for_each(|operand| self.operand(operand))
            }
            ObjectFilter::NthCast {
                ordinal,
                spells,
                by,
                ..
            } => {
                self.expr(ordinal)?;
                self.filter(spells)?;
                by.iter().try_for_each(|players| self.players(players))
            }
            ObjectFilter::Cast { by, from } => {
                if let Some(players) = by {
                    self.players(players)?;
                }
                if let Some(scope) = from {
                    self.zone_scope(scope)?;
                }
                Ok(())
            }
            ObjectFilter::EnteredFrom(scope) => self.zone_scope(scope),
            ObjectFilter::HasKeywordFamily(_) => Ok(()),
            ObjectFilter::HasKeyword(keyword) => self.keyword(keyword),
            ObjectFilter::TargetsAny(targets) => self.recipients(targets),
            ObjectFilter::OnStack(_) => Ok(()),
            ObjectFilter::AbilityOf(sources) => self.filter(sources),
            ObjectFilter::HasActivatedAbility(filter) => self.activation_filter(filter),
            ObjectFilter::InZone(scope) => self.zone_scope(scope),
            ObjectFilter::InSet(objects) | ObjectFilter::SharesColorWithAny(objects) => {
                self.objects(objects)
            }
            ObjectFilter::Any
            | ObjectFilter::CardType(_)
            | ObjectFilter::Supertype(_)
            | ObjectFilter::Subtype(_)
            | ObjectFilter::Color(_)
            | ObjectFilter::Colorless
            | ObjectFilter::IsCard
            | ObjectFilter::Name(_)
            | ObjectFilter::Token
            | ObjectFilter::Modified
            | ObjectFilter::Tapped
            | ObjectFilter::Attacking
            | ObjectFilter::Blocking => Ok(()),
            ObjectFilter::State(_) | ObjectFilter::Chosen(_) => Ok(()),
            ObjectFilter::BlockedByObject(object) => self.object(object),
            ObjectFilter::AttackingDefender(defenders) => self.defenders(defenders),
            ObjectFilter::AttachedTo(filter) => self.filter(filter),
            ObjectFilter::BlockingObject(object) => self.object(object),
            ObjectFilter::All(items) | ObjectFilter::Either(items) => {
                for item in items {
                    self.filter(item)?;
                }
                Ok(())
            }
            ObjectFilter::Not(filter) => self.filter(filter),
            ObjectFilter::ControlledBy(players) | ObjectFilter::OwnedBy(players) => {
                self.players(players)
            }
            ObjectFilter::SameObject(object)
            | ObjectFilter::SharesNameWith(object)
            | ObjectFilter::Shares { with: object, .. } => self.object(object),
            ObjectFilter::SharesNameWithAny(objects)
            | ObjectFilter::SharesNameWithAnother(objects)
            | ObjectFilter::SharesWithAny { with: objects, .. } => self.objects(objects),
            ObjectFilter::Property { value, .. } => self.expr(value),
            ObjectFilter::Counters { value, .. } => self.expr(value),
        }
    }

    fn condition(&self, condition: &Condition) -> Result {
        match condition {
            Condition::ChoseNth { player, .. } => self.player(player),
            Condition::IsPrime(value) => self.expr(value),
            Condition::WonSubgame { player, .. } | Condition::CompletedDungeon { player, .. } => {
                self.player(player)
            }
            Condition::NotedForDraft { player, note } => {
                self.player(player)?;
                self.player(&note.player)
            }
            Condition::TargetCount {
                object,
                filter,
                value,
                ..
            } => {
                self.object(object)?;
                self.filter(filter)?;
                self.expr(value)
            }
            Condition::AsCast { player, action } => {
                self.player(player)?;
                match action {
                    AsCastAction::Revealed(filter) => self.filter(filter),
                    AsCastAction::Controlled(objects) | AsCastAction::Chose(objects) => {
                        self.objects(objects)
                    }
                }
            }
            Condition::PlayerIn { player, players } => {
                self.player(player)?;
                self.players(players)
            }
            Condition::ValueChosenIs(player) | Condition::ChoseExtreme { player, .. } => {
                self.player(player)
            }
            Condition::HadExtreme { player, filter, .. } => {
                self.player(player)?;
                filter.iter().try_for_each(|filter| self.filter(filter))
            }
            Condition::VotedAs { player, other, .. } => {
                self.player(player)?;
                self.player(other)
            }
            Condition::CoinCameUp { player, .. } => {
                player.iter().try_for_each(|player| self.player(player))
            }
            Condition::Outcome(_) | Condition::AllVotedFor(_) => Ok(()),
            Condition::MostCommonColor { color, objects, .. } => {
                self.operand(color)?;
                self.objects(objects)
            }
            Condition::GraveyardPosition {
                card,
                player,
                above,
                amount,
                ..
            } => {
                self.object(card)?;
                self.player(player)?;
                self.filter(above)?;
                self.expr(amount)
            }
            Condition::PlayerDid {
                player,
                event,
                window,
            } => {
                self.player(player)?;
                match event {
                    PlayerHistory::AttackedWithTotalPower(_, value)
                    | PlayerHistory::PaidOrLostEnergy(_, value)
                    | PlayerHistory::RolledDie(_, value) => self.expr(value)?,
                    PlayerHistory::AttackedBy(filter)
                    | PlayerHistory::AttackedWith(filter)
                    | PlayerHistory::AttackedDefender(filter)
                    | PlayerHistory::CastSpell(filter)
                    | PlayerHistory::PlayedFromExile(filter)
                    | PlayerHistory::PermanentEntered(filter) => self.filter(filter)?,
                    PlayerHistory::AttackedPlayer(other) => self.player(other)?,
                    PlayerHistory::Cycled(_, value, filter) => {
                        self.expr(value)?;
                        self.filter(filter)?
                    }
                    _ => {}
                }
                match window {
                    ObjectWindow::LastTurnOf(player)
                    | ObjectWindow::SinceLastUpkeepOf(player)
                    | ObjectWindow::SinceLastTurnOf(player)
                    | ObjectWindow::LastCombatOf(player) => self.player(player),
                    _ => Ok(()),
                }
            }
            Condition::ValueChosen(_) => Ok(()),
            Condition::Primitive { name, operands } => self.primitive(name, operands),
            Condition::SubsetMeets {
                objects,
                amount,
                constraints,
                ..
            } => {
                self.objects(objects)?;
                self.expr(amount)?;
                self.selection_constraints(constraints)
            }
            Condition::Window(_) => Ok(()),
            Condition::FirstDrawInOwnDrawStep(player) => self.player(player),
            Condition::PlayerState { player, .. } => self.player(player),
            Condition::ItsNeitherDayNorNight | Condition::ItsNight => Ok(()),
            Condition::MostVotes { option, .. } | Condition::Voted(option) => {
                self.vote_option(option)
            }
            Condition::PlayerAttacks { attacker, defender } => {
                self.player(attacker)?;
                self.player(defender)
            }
            Condition::SameTeam { player, other } => {
                self.player(player)?;
                self.player(other)
            }
            Condition::VotedFor { player, option } => {
                self.player(player)?;
                self.vote_option(option)
            }
            Condition::ChosenWord(_) | Condition::GiftPromised(_) => Ok(()),
            Condition::PlayerTook { player, result } => {
                self.player(player)?;
                if self.results.contains_key(&result.0) {
                    Ok(())
                } else {
                    Err(format!("result {} is unavailable", result.0))
                }
            }
            Condition::Searched(result) => match self.results.get(&result.0) {
                Some(ResultKind::Search) => Ok(()),
                _ => Err("searched condition requires an available search result".into()),
            },
            Condition::TargetsMatch { object, filter, .. } => {
                self.object(object)?;
                self.filter(filter)
            }
            Condition::WasCastFrom { object, .. } => self.object(object),
            Condition::ExistsPlayer {
                players,
                binding,
                condition,
            } => {
                self.players(players)?;
                let mut child = self.clone();
                if !child.players.insert(binding.0) {
                    return Err("player predicate redeclares a binding".into());
                }
                child.condition(condition)
            }
            Condition::PaymentChosen(result) => match self.results.get(&result.0) {
                Some(ResultKind::Payment) => Ok(()),
                _ => Err(format!(
                    "result {} is not an available payment decision",
                    result.0
                )),
            },
            Condition::PlayerIsActive(player) => self.player(player),
            Condition::Compare { left, right, .. } => {
                self.expr(left)?;
                self.expr(right)
            }
            Condition::CoinFlip { result, .. } if self.results.contains_key(&result.0) => Ok(()),
            Condition::CoinFlip { .. } => Err("no coin was flipped before this branch".into()),
            Condition::All(items) | Condition::Either(items) => {
                for item in items {
                    self.condition(item)?;
                }
                Ok(())
            }
            Condition::Not(condition) => self.condition(condition),
            Condition::ObjectMatches { object, filter } => {
                self.object(object)?;
                self.filter(filter)
            }
            Condition::Controls { player, filter, .. } => {
                self.player(player)?;
                self.filter(filter)
            }
        }
    }

    fn activation_filter(&self, filter: &ActivationFilter) -> Result {
        match filter {
            ActivationFilter::Is(_)
            | ActivationFilter::Any
            | ActivationFilter::Mana
            | ActivationFilter::Cycling => Ok(()),
            ActivationFilter::Of(sources) => self.filter(sources),
            ActivationFilter::Not(f) => self.activation_filter(f),
            ActivationFilter::All(v) | ActivationFilter::Either(v) => {
                for f in v {
                    self.activation_filter(f)?;
                }
                Ok(())
            }
        }
    }
    fn defenders(&self, defenders: &AttackDefenders) -> Result {
        match defenders {
            AttackDefenders::Any => Ok(()),
            AttackDefenders::Objects(f) => self.filter(f),
            AttackDefenders::Players(p) | AttackDefenders::PlayersAndPlaneswalkers(p) | AttackDefenders::DefendingPlayers(p) => {
                self.players(p)
            }
        }
    }
    fn object_action(&self, action: &ObjectAction) -> Result {
        match action {
            ObjectAction::Attack(d) => self.defenders(d),
            ObjectAction::Block(f) | ObjectAction::BeBlocked(f) => self.filter(f),
            ObjectAction::Activate(f) => self.activation_filter(f),
            ObjectAction::BeTargeted(f) => self.filter(f),
            ObjectAction::BeAttachedBy { attachments, .. } => self.filter(attachments),
            ObjectAction::PhaseIn
            | ObjectAction::BePlayed
            | ObjectAction::TurnFaceUp
            | ObjectAction::BecomeSuspected
            | ObjectAction::BeBeamedUp
            | ObjectAction::ChangeControl
            | ObjectAction::LoseCounters(_)
            | ObjectAction::BeAttacked
            | ObjectAction::Untap
            | ObjectAction::Cast
            | ObjectAction::Regenerate
            | ObjectAction::Sacrifice
            | ObjectAction::Counter
            | ObjectAction::ReceiveCounters(_)
            | ObjectAction::Transform
            | ObjectAction::PhaseOut
            | ObjectAction::Crew
            | ObjectAction::Copy => Ok(()),
        }
    }
    fn combat_action(&self, action: &ObjectAction) -> Result {
        if !matches!(
            action,
            ObjectAction::Attack(_) | ObjectAction::Block(_) | ObjectAction::BeBlocked(_)
        ) {
            return Err("combat requirement/cost must describe attacking or blocking".into());
        }
        self.object_action(action)
    }
    fn rule(&self, rule: &RuleChange) -> Result {
        match rule {
            RuleChange::Primitive { name, operands } => self.primitive(name, operands),
            RuleChange::Prohibit(action) => self.object_action(action),
            RuleChange::GoadedBy(player) => self.player(player),
            RuleChange::CantAttackDefenderIf(condition) => self.condition(condition),
            RuleChange::TapPowerBonus { bonus, .. } => self.expr(bonus),
            RuleChange::MayExert(then) => then.iter().try_for_each(|o| self.operand(o)),
            RuleChange::BlockedAsThoughLacking(ability) | RuleChange::CantGainAbility(ability) => {
                self.operand(ability)
            }
            RuleChange::DamageStaysMarked | RuleChange::LegendRuleExempt => Ok(()),
            RuleChange::TargetableDespite { sources: f, .. }
            | RuleChange::AttachOnlyTo(f)
            | RuleChange::AttackDrags(f)
            | RuleChange::AttacksWhenAttacking(f)
            | RuleChange::BlocksDespite { attackers: f, .. } => self.filter(f),
            RuleChange::BlocksOnlyIf(condition) => self.condition(condition),
            RuleChange::BlockedByDefender { players, .. } => self.players(players),
            RuleChange::TapCostUsesToughness(_) | RuleChange::Unique(_) => Ok(()),
            RuleChange::CounterLimit { max, .. } => self.expr(max),
            RuleChange::ActivationsEachTurn { abilities, times } => {
                self.filter(abilities)?;
                self.expr(times)
            }
            RuleChange::OwnActivationCost { abilities, change } => {
                self.filter(abilities)?;
                self.activation_cost_change(change)
            }
            RuleChange::AssignDamageTo {
                players,
                permanents,
            } => {
                players.iter().try_for_each(|p| self.player(p))?;
                self.filter(permanents)
            }
            RuleChange::RequireCombat { action, amount } => {
                self.combat_action(action)?;
                if let RequirementAmount::AtLeast(n) = amount {
                    self.count(&Count::AtLeast(n.clone()))?;
                }
                Ok(())
            }
            RuleChange::CombatCost {
                action,
                payer,
                cost,
            } => {
                self.combat_action(action)?;
                let mut scope = self.clone();
                scope.declaration = true;
                scope.player(payer)?;
                scope.cost(cost)
            }
            RuleChange::CombatGroup {
                participants,
                amount,
                ..
            } => {
                self.filter(participants)?;
                self.expr(amount)
            }
            RuleChange::AttackException { defenders, .. } => self.defenders(defenders),
            RuleChange::BlockWhileTapped | RuleChange::CombatDamage(_) => Ok(()),
            RuleChange::BlockCapacity(BlockCapacity::Unlimited) => Ok(()),
            RuleChange::BlockCapacity(BlockCapacity::Additional(n)) => {
                self.count(&Count::Exactly(n.clone()))
            }
            RuleChange::UntapStep { players, .. } => self.players(players),
        }
    }
    fn change(&self, change: &Change) -> Result {
        match change {
            Change::Continuing { changes, until } => {
                self.duration(until)?;
                changes.iter().try_for_each(|c| self.change(c))
            }
            // Copiable text is the object's own, so it may define characteristics.
            Change::Copiable(changes) => changes.iter().try_for_each(|c| match c {
                Change::GrantAbility(ability) => validate_ability(ability),
                _ => self.change(c),
            }),
            Change::Primitive { name, operands } => self.primitive(name, operands),
            Change::GrantKeyword(keyword) => self.keyword(keyword),
            Change::PlayerRule(rule) => self.player_rule(rule),
            Change::Rule(rule) => self.rule(rule),
            Change::SetController(player) => self.player(player),
            Change::SetPower(n) | Change::SetToughness(n) => self.expr(n),
            Change::Pump { power, toughness }
            | Change::SetBasePowerToughness { power, toughness } => {
                self.expr(power)?;
                self.expr(toughness)
            }
            Change::ColorOfMana { mana, .. } if self.results.contains_key(&mana.0) => Ok(()),
            Change::ColorOfMana { mana, .. } => Err(format!("result {} is unavailable", mana.0)),
            Change::GrantAbility(ability) | Change::OnlyAbility(ability) => {
                if has_definition(ability) {
                    return Err(
                        "a granted ability is not an intrinsic characteristic definition".into(),
                    );
                }
                validate_ability(ability)
            }
            Change::Colors { colors, .. }
                if colors.iter().any(|c| matches!(c, ManaColor::Colorless)) =>
            {
                Err("colorless is an empty color set, not a color".into())
            }
            Change::Colors { .. }
            | Change::ChosenColor { .. }
            | Change::ChosenAbility
            | Change::CreatureTypes { .. }
            | Change::AddCardType(_)
            | Change::RemoveCardType(_)
            | Change::AddSupertype(_)
            | Change::RemoveSupertype(_)
            | Change::AddLandType(_)
            | Change::AddArtifactType(_)
            | Change::SetArtifactType(_)
            | Change::SetCardTypes(_)
            | Change::RemoveAllAbilities
            | Change::ChosenType { .. }
            | Change::SwitchPowerToughness => Ok(()),
            Change::EntersWithCounters { amount, .. } => self.expr(amount),
            Change::SpellCost(modifier) => self.spell_cost_modifier(modifier),
            Change::CantBeCountered => Ok(()),
            Change::ActivationRule(_) => {
                Err("an activation rule belongs to an activated ability".into())
            }
            Change::SetName(_)
            | Change::LandTypes { .. }
            | Change::RemoveAllLandTypes
            | Change::RemoveSubtype(_)
            | Change::ColorlessDamageSource
            | Change::Incorporate(_) => Ok(()),
            Change::AddNames(filter) => self.filter(filter),
            Change::CreatureTypesOf { object, .. } => self.object(object),
            Change::TextOf { object, plus } => {
                self.object(object)?;
                self.operand(plus)
            }
            Change::OnlyAbilities(abilities) => abilities.iter().try_for_each(validate_ability),
            Change::GrantAbilityIfAbsent(ability) => validate_ability(ability),
            Change::GainAbilitiesOf { objects, kinds, .. } => {
                self.objects(objects)?;
                self.operand(kinds)
            }
            Change::ReplaceWord { from, to } => {
                self.operand(from)?;
                self.operand(to)
            }
            Change::AddEnchantmentType(operand)
            | Change::LosesAbility(operand)
            | Change::SetLandType(operand) => self.operand(operand),
            Change::CopyOf { object, effects } => {
                self.object(object)?;
                self.operand(effects)
            }
        }
    }

    fn subject_changes(&self, subjects: &EffectSubjects, changes: &[Change]) -> Result {
        if changes.is_empty() {
            return Err("empty continuous effect".into());
        }
        let mut scope = self.clone();
        scope.affected = false;
        scope.affected_player = false;
        match subjects {
            EffectSubjects::Objects(objects) => {
                scope.objects(objects)?;
                scope.affected = true;
            }
            EffectSubjects::Players(players) => {
                scope.players(players)?;
                scope.affected_player = true;
            }
        }
        for change in changes {
            if matches!(change, Change::PlayerRule(_)) != scope.affected_player {
                return Err(
                    "continuous effect change does not match its object/player subjects".into(),
                );
            }
            scope.change(change)?;
        }
        Ok(())
    }
    fn mana_payment(&self, payment: &ManaPayment) -> Result {
        match payment {
            ManaPayment::Anything => Ok(()),
            ManaPayment::Spells(filter) | ManaPayment::Abilities(filter) => self.filter(filter),
        }
    }
    fn player_action(&self, action: &PlayerAction) -> Result {
        match action {
            PlayerAction::CastSpell { filter, from } | PlayerAction::PlayLand { filter, from } => {
                if matches!(
                    from,
                    Some(ZoneScope::Shared(EventZone::Battlefield | EventZone::Stack))
                ) {
                    return Err("cast/play origin must be a card zone".into());
                }
                if let Some(zone) = from {
                    self.zone_scope(zone)?;
                }
                self.filter(filter)
            }
            PlayerAction::AttackWith(filter) | PlayerAction::BlockWith(filter) => {
                self.filter(filter)
            }
            PlayerAction::AttackInto(recipients) => self.recipients(recipients),
            PlayerAction::PayLife(payment) => self.mana_payment(payment),
            PlayerAction::Sacrifice { objects, paying } => {
                self.filter(objects)?;
                paying.iter().try_for_each(|p| self.mana_payment(p))
            }
            PlayerAction::Activate(filter) => self.activation_filter(filter),
            PlayerAction::PlayCard(filter)
            | PlayerAction::ExileObjects(filter)
            | PlayerAction::BeTargeted(filter) => self.filter(filter),
            PlayerAction::AttackDefender(defenders) => self.defenders(defenders),
            _ => Ok(()),
        }
    }
    fn player_rule(&self, rule: &PlayerRule) -> Result {
        match rule {
            PlayerRule::SpecialAction {
                body, cost, during, ..
            } => {
                self.operand(body)?;
                cost.iter().try_for_each(|c| self.operand(c))?;
                during.iter().try_for_each(|p| self.players(p))
            }
            PlayerRule::ActivationGrant { abilities, grant } => {
                self.filter(abilities)?;
                match grant {
                    ActivationGrant::InstantSpeedDuring(players) => self.players(players),
                    ActivationGrant::AdditionalTimes(times) => self.expr(times),
                    _ => Ok(()),
                }
            }
            PlayerRule::KeywordCost {
                cards, change, per, ..
            } => {
                self.filter(cards)?;
                per.iter().try_for_each(|n| self.expr(n))?;
                self.keyword_cost_change(change)
            }
            PlayerRule::KeywordAccess { cards, during, .. } => {
                self.filter(cards)?;
                during.iter().try_for_each(|p| self.players(p))
            }
            PlayerRule::Decides(GameDecision::HowCreaturesBlock(filter))
            | PlayerRule::Decides(GameDecision::CombatDamageOf(filter))
            | PlayerRule::AttackedOnlyBy(filter)
            | PlayerRule::TargetsMustInclude(filter)
            | PlayerRule::TargetableDespite {
                sources: filter, ..
            } => self.filter(filter),
            PlayerRule::Decides(GameDecision::SearchesOf(players))
            | PlayerRule::DrawsDuringDrawStepOf(players) => self.players(players),
            PlayerRule::MustAttack {
                attackers,
                defenders,
                count,
            } => {
                self.filter(attackers)?;
                self.defenders(defenders)?;
                count.iter().try_for_each(|(_, n)| self.expr(n))
            }
            PlayerRule::AttackOnly(defenders) => self.defenders(defenders),
            PlayerRule::AttackWithoutTapping {
                attackers,
                condition,
            } => {
                self.filter(attackers)?;
                self.condition(condition)
            }
            PlayerRule::BlockerLimit(n)
            | PlayerRule::SurveilBonus(n)
            | PlayerRule::DevotionBonus(n) => self.expr(n),
            PlayerRule::SpendColorAs { on, .. } => self.mana_payment(on),
            PlayerRule::RevealDraws {
                first_during: Some(players),
                ..
            } => self.players(players),
            PlayerRule::CantBeCaused { action, by } => {
                self.player_action(action)?;
                self.filter(by)
            }
            PlayerRule::UntapsDuringUntapStepOf {
                permanents,
                players,
            } => {
                self.filter(permanents)?;
                self.players(players)
            }
            PlayerRule::UntapOnlyOneOfTypes(types) => {
                types.iter().try_for_each(|o| self.operand(o))
            }
            PlayerRule::LifeForMana { life, symbol: mana }
            | PlayerRule::LifeForManaAbility { life, mana } => {
                self.expr(life)?;
                self.operand(mana)
            }
            PlayerRule::TapOthersForMana {
                permanents,
                modifier,
            } => {
                self.filter(permanents)?;
                self.operand(modifier)
            }
            PlayerRule::DiceChange { cost, .. } => self.operand(cost),
            PlayerRule::CascadePut { filter, flags } => {
                self.filter(filter)?;
                self.operand(flags)
            }
            PlayerRule::AsManaEmpties { mana, then } => {
                self.operand(mana)?;
                self.operand(then)
            }
            PlayerRule::GrantKeyword(
                PlayerKeyword::Protection(quality) | PlayerKeyword::HexproofFrom(quality),
            ) => self.protection(quality),
            PlayerRule::PlayPermission {
                cards,
                from,
                filter,
                ..
            } => {
                if matches!(from, EventZone::Stack | EventZone::Battlefield) {
                    return Err("play permission origin must be a card zone".into());
                }
                if self.known_zones(cards).is_some_and(|zones| {
                    zones
                        .iter()
                        .any(|z| matches!(z, EventZone::Stack | EventZone::Battlefield))
                }) {
                    return Err(
                        "play permission must select cards outside the battlefield and stack"
                            .into(),
                    );
                }
                self.require_zone(cards, from.clone())?;
                self.filter(filter)
            }
            PlayerRule::PlayPermissionWith { permission, terms } => {
                self.player_rule(permission)?;
                if let Some(players) = &terms.during {
                    self.players(players)?;
                }
                if let PlayCost::Alternate(cost) | PlayCost::Additional(cost) = &terms.cost {
                    self.operand(cost)?;
                }
                if let Some(condition) = &terms.condition {
                    self.condition(condition)?;
                }
                // A program here runs as the card is cast, and reads that spell.
                let mut cast = self.clone();
                cast.event.object = true;
                cast.event.spell = true;
                terms.effects.iter().try_for_each(|o| match o {
                    Operand::Program(_) => cast.operand(o),
                    _ => self.operand(o),
                })
            }
            PlayerRule::Primitive { name, operands } => self.primitive(name, operands),
            PlayerRule::SpellsHaveAbilities {
                spells,
                nth_each_turn,
                abilities,
                ..
            } => {
                self.filter(spells)?;
                if let Some(nth) = nth_each_turn {
                    self.expr(nth)?;
                }
                self.operand(abilities)
            }
            PlayerRule::SpendManaAs { on, .. } => match on {
                ManaPayment::Anything => Ok(()),
                ManaPayment::Spells(filter) | ManaPayment::Abilities(filter) => self.filter(filter),
            },
            PlayerRule::ActivationCost {
                abilities, change, ..
            } => {
                self.filter(abilities)?;
                self.activation_cost_change(change)
            }
            PlayerRule::Prohibit(action) => self.player_action(action),
            PlayerRule::LimitPerTurn { action, amount } => {
                self.player_action(action)?;
                self.count(&Count::Exactly(amount.clone()))
            }
            PlayerRule::HandSize(HandSizeRule::Set(n) | HandSizeRule::Adjust(n)) => self.expr(n),
            PlayerRule::AdditionalLandPlays(count) => {
                if !matches!(count, Count::Exactly(_) | Count::AnyNumber) {
                    return Err("additional land allowance must be exact or unlimited".into());
                }
                self.count(count)
            }
            PlayerRule::SpellCost { spells, modifier } => {
                self.filter(spells)?;
                let mut each = self.clone();
                each.affected = true;
                each.spell_cost_modifier(modifier)
            }
            PlayerRule::SpellTiming { spells, .. } => self.filter(spells),
            PlayerRule::Visibility { objects, .. } => self.objects(objects),
            PlayerRule::UntapLimit { permanents, amount } => {
                self.filter(permanents)?;
                self.count(&Count::Exactly(amount.clone()))
            }
            PlayerRule::RetainMana {
                colors: Some(colors),
            } if colors.is_empty() => Err("empty mana retention colors".into()),
            _ => Ok(()),
        }
    }

    fn protection(&self, protection: &Protection) -> Result {
        match protection {
            Protection::Objects { filter, .. } => self.filter(filter),
            Protection::Players(players) => self.players(players),
            Protection::Each(qualities) => {
                if qualities.len() < 2 {
                    return Err(
                        "multiple protection qualities require at least two instances".into(),
                    );
                }
                for quality in qualities {
                    self.protection(quality)?;
                }
                Ok(())
            }
        }
    }
    fn keyword(&self, keyword: &Keyword) -> Result {
        match keyword {
            Keyword::Protection(quality) | Keyword::HexproofFrom(quality) => {
                self.protection(quality)
            }
            Keyword::Landwalk(lands) => self.filter(lands),
            Keyword::Ward(cost) => {
                let mut scope = Scope::default();
                scope.ward_payer = true;
                scope.keyword_host = true;
                scope.cost(cost)
            }
            _ => Ok(()),
        }
    }

    fn removals(&self, removals: &[CounterRemoval]) -> Result {
        if removals.is_empty() {
            return Err("empty counter removal batch".into());
        }
        for removal in removals {
            self.player(&removal.player)?;
            self.recipients(&removal.recipients)?;
            if let CounterAmount::Count(count) | CounterAmount::AmongThem(count) = &removal.amount {
                self.count(count)?;
            }
            if let Some(selection) = &removal.selection {
                if !matches!(removal.recipients, Recipients::Objects(_)) {
                    return Err("counter removal selection requires objects".into());
                }
                self.player(&selection.player)?;
                self.count(&selection.count)?;
            }
        }
        Ok(())
    }
    fn cost_object_selection(&self, selection: &CostObjectSelection) -> Result {
        self.objects(&selection.objects)?;
        if let Some(count) = &selection.choose {
            if matches!(count, Count::AsManyAsCan(_)) {
                return Err("cost selection must be payable in full".into());
            }
            self.count(count)?;
        }
        Ok(())
    }
    fn play_cost(&self, cost: &PlayCost) -> Result {
        match cost {
            PlayCost::Alternate(paid) | PlayCost::Additional(paid) | PlayCost::Reduced(paid) => {
                self.operand(paid)
            }
            PlayCost::Normal | PlayCost::Free | PlayCost::Using(_) => Ok(()),
        }
    }
    fn activation_cost_change(&self, change: &ActivationCostChange) -> Result {
        match change {
            ActivationCostChange::Additional(cost) => self.cost(cost),
            ActivationCostChange::Reduce(amount) | ActivationCostChange::Increase(amount) => {
                self.operand(amount)
            }
            ActivationCostChange::ReduceX { amount, times } => {
                self.operand(amount)?;
                self.expr(times)
            }
        }
    }
    fn keyword_cost_change(&self, change: &KeywordCostChange) -> Result {
        match change {
            KeywordCostChange::Reduce(amount)
            | KeywordCostChange::Increase(amount)
            | KeywordCostChange::Alternate(amount) => self.operand(amount),
        }
    }
    fn spell_cost_modifier(&self, modifier: &SpellCostModifier) -> Result {
        match modifier {
            SpellCostModifier::Increase(cost) => self.cost(cost),
            SpellCostModifier::Alternate { cost, .. } => self.play_cost(cost),
            SpellCostModifier::AlternateDuring { cost, during, .. } => {
                self.players(during)?;
                self.play_cost(cost)
            }
            SpellCostModifier::PayToReduce { cost, mana } => {
                self.operand(cost)?;
                self.operand(mana)
            }
            SpellCostModifier::OptionalAdditional { cost, effects } => {
                self.operand(cost)?;
                self.operand(effects)
            }
            SpellCostModifier::Scaled { change, per } => {
                if let CostScale::Times(times) = per {
                    self.expr(times)?;
                }
                self.activation_cost_change(change)
            }
            SpellCostModifier::Minimum(n) => self.count(&Count::Exactly(n.clone())),
            SpellCostModifier::Reduce {
                reductions,
                spill_into_generic,
            } => {
                if reductions.is_empty() {
                    return Err("empty spell cost reduction".into());
                }
                for r in reductions {
                    if !spill_into_generic && r.color.is_none() {
                        return Err("non-spilling colored reduction requires a color".into());
                    }
                    self.count(&Count::Exactly(r.amount.clone()))?;
                }
                Ok(())
            }
        }
    }
    fn activation_rule(&self, rule: &ActivationRule) -> Result {
        match rule {
            ActivationRule::RestrictMana { .. } => Ok(()),
            ActivationRule::ManaInstead(cost) => self.cost(cost),
            ActivationRule::Cost {
                modifier,
                condition,
            } => {
                self.spell_cost_modifier(modifier)?;
                condition.iter().try_for_each(|c| self.condition(c))
            }
            ActivationRule::OnlyBy(players) => self.players(players),
            ActivationRule::MaxPerTurn(n) | ActivationRule::XAtMost(n) | ActivationRule::XIs(n) => {
                self.expr(n)
            }
            ActivationRule::XNotZero
            | ActivationRule::CantBeCopied
            | ActivationRule::AnyTimeOfInstant => Ok(()),
        }
    }
    fn cost(&self, cost: &Cost) -> Result {
        for part in &cost.parts {
            match part {
                CostPart::Primitive { name, operands } => self.primitive(name, operands)?,
                CostPart::LifeEqualToOwnManaValue
                | CostPart::ManaAnyNumberOfTimes(_)
                | CostPart::RevealChoice(_)
                | CostPart::EnergyOneOrMore
                | CostPart::ManaFromTreasure(_)
                | CostPart::ShuffleFromHand => {}
                CostPart::ManaCostReduced { object, reduction } => {
                    object.iter().try_for_each(|o| self.object(o))?;
                    self.operand(reduction)?;
                }
                CostPart::ManaXBounded { value, .. } => self.expr(value)?,
                CostPart::UnattachFrom { attachments, hosts } => {
                    self.objects(attachments)?;
                    self.objects(hosts)?;
                }
                CostPart::EachPlayerAction { players, action } => {
                    self.players(players)?;
                    self.operand(action)?;
                }
                CostPart::ManaReducedForEach { reduction, per, .. } => {
                    self.operand(reduction)?;
                    self.expr(per)?;
                }
                CostPart::PlayFromTop(filter) => self.filter(filter)?,
                CostPart::ObjectAction { objects, action } => {
                    self.objects(objects)?;
                    if let CostObjectAction::AttachToPlayer(players) = action {
                        self.players(players)?;
                    }
                }
                CostPart::SinglePlayersCards(part) => self.cost(&Cost {
                    parts: vec![part.as_ref().clone()],
                })?,
                CostPart::Constrained { part, constraints } => {
                    let selectable = match part.as_ref() {
                        CostPart::Sacrifice { .. }
                        | CostPart::Discard { .. }
                        | CostPart::Reveal { .. } => true,
                        CostPart::ChangeTapState {
                            selections,
                            symbol: false,
                            ..
                        }
                        | CostPart::Move { selections, .. } => selections.len() == 1,
                        _ => false,
                    };
                    if !selectable
                        || constraints.is_empty()
                        || constraints
                            .iter()
                            .any(|c| matches!(c, SelectionConstraint::CanMoveTo { .. }))
                    {
                        return Err("group payment requires a single object selection and nonempty payment constraints".into());
                    }
                    self.cost(&Cost {
                        parts: vec![part.as_ref().clone()],
                    })?;
                    self.selection_constraints(constraints)?;
                }
                CostPart::ManaCostOf(object) => self.object(object)?,
                CostPart::Move {
                    selections,
                    destination,
                    entry,
                    face_down,
                } => {
                    self.operation(&Operation::Move {
                        objects: ObjectSet::Union(
                            selections.iter().map(|s| s.objects.clone()).collect(),
                        ),
                        destination: destination.clone(),
                        entry: entry.clone(),
                        face_down: *face_down,
                    })?;
                    for selection in selections {
                        self.cost_object_selection(selection)?;
                    }
                }
                CostPart::ChangeTapState {
                    selections, symbol, ..
                } => {
                    if selections.is_empty() {
                        return Err("tap-state payment has no object selections".into());
                    }
                    if *symbol
                        && !matches!(
                            selections.as_slice(),
                            [CostObjectSelection {
                                objects: ObjectSet::One(ObjectRef::Source | ObjectRef::KeywordHost),
                                choose: None
                            }]
                        )
                    {
                        return Err("tap/untap symbol requires the implicit source or keyword host without a choice".into());
                    }
                    for selection in selections {
                        self.cost_object_selection(selection)?;
                        self.require_zone(&selection.objects, EventZone::Battlefield)?;
                    }
                }
                CostPart::Choice(options) => {
                    if options.is_empty() {
                        return Err("cost choice has no alternatives".into());
                    }
                    for cost in options {
                        self.cost(cost)?;
                    }
                }
                CostPart::Discard { count, filter, .. } => {
                    self.count(&Count::Exactly(count.clone()))?;
                    self.filter(filter)?;
                }
                CostPart::RemoveCounters { removals } => self.removals(removals)?,
                CostPart::Mana(_) | CostPart::TapSymbol => {}
                CostPart::Life(amount) => self.count(&Count::Exactly(amount.clone()))?,
                CostPart::Loyalty(delta) | CostPart::Energy(delta) => self.expr(delta)?,
                CostPart::Draw { count } | CostPart::Mill { count } => {
                    self.count(&Count::Exactly(count.clone()))?
                }
                CostPart::PlayerAction { player, body } => {
                    self.player(player)?;
                    self.clone().program(body)?;
                }
                CostPart::Reveal { count, filter, .. } => {
                    self.count(count)?;
                    self.filter(filter)?;
                }
                CostPart::ManaWithX { x, .. } => x.iter().try_for_each(|x| self.expr(x))?,
                CostPart::Exert(object)
                | CostPart::Unattach(object)
                | CostPart::Unspecialize(object) => self.object(object)?,
                CostPart::Keyword { amount, .. } => {
                    if let Some(amount) = amount {
                        self.expr(amount)?;
                    }
                }
                CostPart::DiscardHand => {}
                CostPart::BeholdAndExile(what) => self.operand(what)?,
                CostPart::Behold { count, what } => {
                    self.expr(count)?;
                    self.operand(what)?;
                }
                CostPart::ExiledToGraveyard(cards) => self.objects(cards)?,
                CostPart::RevealTop(filter) => self.filter(filter)?,
                CostPart::Waterbend(_) | CostPart::Teamwork(_) => {}
                CostPart::MoveCounters {
                    from, to, amount, ..
                } => {
                    self.objects(from)?;
                    self.objects(to)?;
                    if let MovedCounters::Exactly(amount) | MovedCounters::AtLeast(amount) = amount
                    {
                        self.expr(amount)?;
                    }
                }
                CostPart::WaterbendX { x, .. } => self.expr(x)?,
                CostPart::PutCounters(placements) => {
                    for placement in placements {
                        self.recipients(&placement.recipients)?;
                        self.count(&Count::Exactly(placement.amount.clone()))?;
                    }
                }
                CostPart::Repeated { part, times } => {
                    self.count(&Count::Exactly(times.clone()))?;
                    self.cost(&Cost {
                        parts: vec![part.as_ref().clone()],
                    })?;
                }
                CostPart::Sacrifice { objects, choose } => {
                    self.cost_object_selection(&CostObjectSelection {
                        objects: objects.clone(),
                        choose: choose.clone(),
                    })?;
                }
            }
        }
        Ok(())
    }

    fn casting_rule(&self, rule: &CastingRule) -> Result {
        match rule {
            CastingRule::FlashWithRider(rider) => self.operand(rider),
            CastingRule::TaxPaidAsLife(life) => self.expr(life),
            CastingRule::Permission {
                origin,
                player,
                from,
                require,
                cost,
                leave_stack,
            } => {
                self.player(player)?;
                self.zone_scope(from)?;
                self.filter(require)?;
                self.cost(cost)?;
                if let Some(destination) = leave_stack {
                    self.destination(destination)?;
                }
                if matches!(origin, CastingOrigin::Flashback) {
                    let owner = PlayerRef::OwnerOf(Box::new(ObjectRef::Source));
                    if !crate::semantics::same_player(player, &owner)
                        || !matches!(from,ZoneScope::Players {players:PlayerSet::One(p),..} if crate::semantics::same_player(p,&owner))
                    {
                        return Err(
                            "flashback permission belongs to the owner and their graveyard".into(),
                        );
                    }
                    let expected = ObjectFilter::Either(vec![
                        ObjectFilter::CardType(crate::mtg_types::CardType::Instant),
                        ObjectFilter::CardType(crate::mtg_types::CardType::Sorcery),
                    ]);
                    if !matches!(
                        from,
                        ZoneScope::Players {
                            zone: EventZone::Graveyard,
                            ..
                        }
                    ) || !matches!(leave_stack, Some(Destination::Exile))
                        || serde_json::to_value(require).unwrap()
                            != serde_json::to_value(expected).unwrap()
                    {
                        return Err("flashback requires graveyard permission, an instant/sorcery result, and its exile replacement".into());
                    }
                }
                Ok(())
            }
            CastingRule::AdditionalCost { cost, .. }
            | CastingRule::AlternativeCost(cost)
            | CastingRule::AlternativeCostWithFlash(cost) => self.cost(cost),
            CastingRule::ReduceForEachPaid { cost, .. } => self.cost(cost),
            CastingRule::ReduceCost(reductions) => {
                for reduction in reductions {
                    self.count(&Count::Exactly(reduction.amount.clone()))?;
                }
                Ok(())
            }
            CastingRule::Require(condition) => self.condition(condition),
            CastingRule::Flash { additional_cost } => {
                if let Some(cost) = additional_cost {
                    self.cost(cost)?;
                }
                Ok(())
            }
            CastingRule::ProhibitTargets { domain, filter } => {
                self.domain(domain)?;
                self.filter(filter)
            }
            CastingRule::RestrictMana { filter, .. } => match filter {
                ManaPaymentFilter::None => Ok(()),
                ManaPaymentFilter::Colors(colors) if colors.is_empty() => {
                    Err("mana payment colors cannot be empty".into())
                }
                ManaPaymentFilter::Colors(_) => Ok(()),
                ManaPaymentFilter::ProducedBy(filter) => self.filter(filter),
            },
            CastingRule::SpendAsThoughAnyType => Ok(()),
            CastingRule::If { condition, rule } => {
                self.condition(condition)?;
                self.casting_rule(rule)
            }
        }
    }

    fn destination(&self, destination: &Destination) -> Result {
        if let Destination::OwnersLibraryFromTop(depth) = destination {
            self.expr(depth)?;
        }
        if let Destination::Battlefield(player) = destination {
            self.player(player)?;
        }
        Ok(())
    }
    fn primitive(&self, name: &str, operands: &[Operand]) -> Result {
        if crate::primitive_spec::lookup(name).is_none() {
            return Err(format!("{name} is not a reviewed primitive"));
        }
        let mut scoped = self.clone();
        if crate::primitive_spec::lookup(name).is_some_and(|row| !row.binds.is_empty()) {
            scoped.event = crate::semantics::with_row(&self.event, name);
        }
        if let Some(event) = operands.iter().find_map(|o| match o {
            Operand::Event(event) => Some(event),
            _ => None,
        }) {
            scoped.event = crate::semantics::event_bindings(event);
        }
        operands.iter().try_for_each(|o| scoped.operand(o))
    }
    fn named(&self, named: &Named) -> Result {
        named.operands.iter().try_for_each(|o| self.operand(o))
    }
    /// A row's form says what its own "each" ranges over.
    fn operand(&self, operand: &Operand) -> Result {
        let checked = self.bare_operand(operand);
        if checked.is_err() && !self.affected {
            let mut each = self.clone();
            each.affected = true;
            return each.bare_operand(operand);
        }
        checked
    }

    fn bare_operand(&self, operand: &Operand) -> Result {
        match operand {
            Operand::Absent | Operand::Bool(_) | Operand::Integer(_) | Operand::Text(_) => Ok(()),
            Operand::List(items) => items.iter().try_for_each(|o| self.operand(o)),
            Operand::Record(fields) => fields.iter().try_for_each(|f| self.operand(&f.value)),
            Operand::Form(named) => self.named(named),
            Operand::Vocabulary { form, .. } | Operand::Term { form, .. }
                if crate::lower::reads_scope(form) =>
            {
                Err(format!("{form} names something in scope").into())
            }
            Operand::Vocabulary { .. } => Ok(()),
            Operand::Term { operands, .. } => operands.iter().try_for_each(|o| self.operand(o)),
            Operand::Object(object) => self.object(object),
            Operand::Objects(objects) => self.objects(objects),
            Operand::Recipients(recipients) => self.recipients(recipients),
            Operand::Player(player) => self.player(player),
            Operand::Players(players) => self.players(players),
            Operand::Number(expression) => self.expr(expression),
            Operand::Condition(condition) => self.condition(condition),
            Operand::Duration(_) => Ok(()),
            Operand::Filter(filter) => self.filter(filter),
            Operand::Comparison(_, expression) => self.expr(expression),
            Operand::Stickers(stickers) => {
                for sticker in stickers {
                    if let Sticker::Abilities { abilities, .. } = sticker {
                        for ability in abilities {
                            validate_ability_in(ability, None, &BTreeMap::new())?;
                        }
                    }
                }
                Ok(())
            }
            Operand::Cost(cost) => self.cost(cost),
            // A nested body closes over this scope, targets included.
            Operand::Ability(ability) => match &**ability {
                Ability::Spell { announcement, body } => {
                    let mut child = self.clone();
                    child.replacement = false;
                    child.open_searches.clear();
                    child.announcement(announcement)?;
                    child.program(body)
                }
                other => validate_ability(other),
            },
            Operand::Event(event) => self.event(event),
            Operand::Result(result) if self.results.contains_key(&result.0) => Ok(()),
            Operand::Result(result) => Err(format!("result {} has not run", result.0)),
            Operand::Program(program) => {
                let mut child = self.clone();
                child.open_searches.clear();
                child.program(program)
            }
        }
    }
    fn new_targets(&self, targets: &NewTargets) -> Result {
        match targets {
            NewTargets::To(objects) | NewTargets::Each(objects) => self.objects(objects),
            NewTargets::ToPlayers(players) | NewTargets::EachPlayer(players) => {
                self.players(players)
            }
            NewTargets::Random(except) => except.iter().try_for_each(|o| self.operand(o)),
            NewTargets::Same | NewTargets::May | NewTargets::Must | NewTargets::EachLegal => Ok(()),
        }
    }
    fn operation(&self, operation: &Operation) -> Result {
        match operation {
            Operation::Meld { objects, entry, .. } => {
                self.objects(objects)?;
                entry.iter().try_for_each(|flag| self.operand(flag))
            }
            Operation::PutStickers {
                actor,
                objects,
                amount,
                ticket_cost,
                ..
            } => {
                self.players(actor)?;
                self.objects(objects)?;
                ticket_cost.iter().try_for_each(|(_, n)| self.expr(n))?;
                amount.iter().try_for_each(|n| self.expr(n))
            }
            Operation::Piles {
                actor,
                objects,
                count,
                chooser,
                ..
            } => {
                self.players(actor)?;
                self.objects(objects)?;
                count.iter().try_for_each(|n| self.expr(n))?;
                chooser.iter().try_for_each(|p| self.players(p))
            }
            Operation::ChoosePile {
                actor,
                from,
                object,
                ..
            } => {
                self.players(actor)?;
                from.iter().try_for_each(|p| self.players(p))?;
                object.iter().try_for_each(|o| self.object(o))
            }
            Operation::Note(kind) => match kind {
                NoteKind::Number(value) => self.expr(value),
                NoteKind::Player(player) => self.player(player),
                NoteKind::CountersOn(object) => self.object(object),
                NoteKind::Numbered(objects) => self.objects(objects),
                NoteKind::MostCommonCreatureType(players) => self.players(players),
                NoteKind::LifeTotal | NoteKind::ManaSpent { .. } => Ok(()),
            },
            Operation::Bid { players, start } => {
                self.players(players)?;
                start.iter().try_for_each(|n| self.expr(n))
            }
            Operation::Guess {
                actor,
                object,
                filter,
                player,
            } => {
                self.players(actor)?;
                object.iter().try_for_each(|o| self.object(o))?;
                filter.iter().try_for_each(|f| self.filter(f))?;
                player.iter().try_for_each(|p| self.player(p))
            }
            Operation::Seek {
                actor,
                cards,
                count,
                top,
            } => {
                self.players(actor)?;
                cards.iter().try_for_each(|f| self.filter(f))?;
                self.expr(count)?;
                top.iter().try_for_each(|n| self.expr(n))
            }
            Operation::DrawThenDiscard {
                players,
                draw,
                discard,
            } => {
                self.players(players)?;
                self.expr(draw)?;
                self.expr(discard)
            }
            Operation::RedistributeLife(players) => self.players(players),
            Operation::WithReflexive { action, .. } => self.operation(action),
            Operation::GiveControl {
                players, objects, ..
            } => {
                self.players(players)?;
                self.objects(objects)
            }
            Operation::AnyPlayerMayPay { players, .. } => self.players(players),
            Operation::EachPlayer { players, action } => {
                self.players(players)?;
                self.operation(action)
            }
            Operation::EachObject { objects, action } => {
                self.objects(objects)?;
                let mut each = self.clone();
                each.affected = true;
                each.operation(action)
            }
            Operation::LimitX { value, .. } => self.expr(value),
            Operation::RestartGame { keep, then } => {
                self.objects(keep)?;
                self.operation(then)
            }
            Operation::EachNumber { from, to, action } => {
                self.expr(from)?;
                self.expr(to)?;
                self.operation(action)
            }
            Operation::CounterWith { object, when, .. } => {
                self.object(object)?;
                when.iter().try_for_each(|f| self.filter(f))
            }
            Operation::Populate { actor, .. } => self.players(actor),
            Operation::ReduceSpeed {
                player,
                amount,
                floor,
            } => {
                self.player(player)?;
                self.expr(amount)?;
                self.expr(floor)
            }
            Operation::PutCountersCapped {
                amount,
                object,
                at_most,
                ..
            } => {
                self.expr(amount)?;
                self.object(object)?;
                self.expr(at_most)
            }
            Operation::MillUntil {
                actor,
                cards,
                at_most,
            } => {
                self.players(actor)?;
                self.filter(cards)?;
                self.expr(at_most)
            }
            Operation::ChooseDefender { actor, attacker } => {
                self.players(actor)?;
                self.player(attacker)
            }
            Operation::PlayersPayRepeatedly {
                players,
                cost,
                at_most,
            } => {
                self.players(players)?;
                self.cost(cost)?;
                self.expr(at_most)
            }
            Operation::DelayedUnlessPaid { payer, .. } => self.player(payer),
            Operation::FightEach { fighters, others } => {
                self.objects(fighters)?;
                self.objects(others)
            }
            Operation::PickFromHand {
                actor,
                player,
                filter,
                ..
            } => {
                self.players(actor)?;
                self.player(player)?;
                self.filter(filter)
            }
            Operation::DistributeCounters {
                amount, objects, ..
            } => {
                self.expr(amount)?;
                self.objects(objects)
            }
            Operation::PutCountersEach {
                amount, objects, ..
            } => {
                self.objects(objects)?;
                let mut each = self.clone();
                each.affected = true;
                each.expr(amount)
            }
            Operation::PutChosenCounters { objects, choice } => {
                self.objects(objects)?;
                match choice {
                    CounterChoice::PerAbility { of, kinds, .. } => {
                        of.iter().try_for_each(|object| self.object(object))?;
                        kinds.iter().try_for_each(|kind| self.operand(kind))
                    }
                    _ => Ok(()),
                }
            }
            Operation::NoteObjects { objects } => self.objects(objects),
            Operation::Named { actor, form } => {
                self.players(actor)?;
                self.named(form)
            }
            Operation::Primitive {
                actor,
                name,
                operands,
            } => {
                self.players(actor)?;
                self.primitive(name, operands)
            }
            Operation::CreatePerpetualEffect { objects, changes } => {
                self.objects(objects)?;
                self.entering_changes(changes)
            }
            Operation::KeywordEach {
                actor,
                objects,
                until,
                ..
            } => {
                self.players(actor)?;
                self.objects(objects)?;
                until.iter().try_for_each(|until| self.duration(until))
            }
            Operation::MoveTopCard {
                actor,
                cards,
                entry,
                ..
            } => {
                self.players(actor)?;
                self.filter(cards)?;
                entry.iter().try_for_each(|o| self.operand(o))
            }
            Operation::PayAnyAmount { actor, .. } => self.players(actor),
            Operation::RevealFromHand {
                actor,
                cards,
                count,
                ..
            } => {
                self.players(actor)?;
                self.filter(cards)?;
                count.iter().try_for_each(|n| self.expr(n))
            }
            Operation::CopyOnStack {
                actor,
                object,
                times,
                targets,
                effects,
                card,
            } => {
                self.players(actor)?;
                self.object(object)?;
                self.expr(times)?;
                self.new_targets(targets)?;
                card.iter().try_for_each(|o| self.object(o))?;
                effects.iter().try_for_each(|o| self.operand(o))
            }
            Operation::Retarget {
                actor,
                object,
                targets,
            } => {
                self.players(actor)?;
                self.object(object)?;
                self.new_targets(targets)
            }
            Operation::AbandonScheme { actor } => self.players(actor),
            Operation::Together(operations) => {
                operations.iter().try_for_each(|o| self.operation(o))
            }
            Operation::Repeat { times, body } => {
                self.expr(times)?;
                self.operation(body)
            }
            Operation::TurnFaceDownAs {
                objects,
                as_creature,
            } => {
                self.objects(objects)?;
                self.operand(&as_creature.power_toughness)?;
                as_creature
                    .creature_type
                    .iter()
                    .try_for_each(|o| self.operand(o))
            }
            Operation::Exchange {
                first,
                second,
                until,
                ..
            } => {
                self.operand(first)?;
                self.operand(second)?;
                until.iter().try_for_each(|until| self.duration(until))
            }
            Operation::ChooseValueWith { actor, terms, .. } => {
                self.players(actor)?;
                terms.options.iter().try_for_each(|o| self.operand(o))?;
                terms.except.iter().try_for_each(|o| self.operand(o))?;
                terms.count.iter().try_for_each(|n| self.expr(n))
            }
            Operation::RevealChoices { .. } => Ok(()),
            Operation::CoinFlips {
                actor, times, each, ..
            } => {
                self.players(actor)?;
                times.iter().try_for_each(|n| self.expr(n))?;
                each.iter().try_for_each(|o| self.objects(o))
            }
            Operation::AttachEach {
                actor,
                attachments,
                count,
                hosts,
                ..
            } => {
                self.players(actor)?;
                self.objects(attachments)?;
                count.iter().try_for_each(|c| self.count(c))?;
                self.recipients(hosts)
            }
            Operation::CopyEach {
                actor,
                objects,
                targets,
            }
            | Operation::RetargetEach {
                actor,
                objects,
                targets,
            } => {
                self.players(actor)?;
                self.filter(objects)?;
                self.new_targets(targets)
            }
            Operation::TakeSpell {
                actor,
                object,
                targets,
            } => {
                self.players(actor)?;
                self.object(object)?;
                self.new_targets(targets)
            }
            Operation::SetX { object, value } => {
                self.object(object)?;
                self.expr(value)
            }
            Operation::ChangeCombat { change } => match change {
                CombatChange::Reselect { objects, .. } | CombatChange::OnlyBlockers(objects) => {
                    self.objects(objects)
                }
                CombatChange::OnlyAttackers { objects, .. } => self.objects(objects),
                CombatChange::AssignAttack { objects, to } => {
                    self.objects(objects)?;
                    self.operand(to)
                }
                CombatChange::Block { blocker, attackers } => {
                    self.object(blocker)?;
                    self.objects(attackers)
                }
                CombatChange::SwapBlocks { first, second, .. } => {
                    self.object(first)?;
                    self.object(second)
                }
            },
            Operation::PlayersDo {
                players,
                body,
                otherwise,
                ..
            } => {
                self.players(players)?;
                body.iter().try_for_each(|o| self.operand(o))?;
                otherwise.iter().try_for_each(|o| self.operand(o))
            }
            Operation::CreateBoon {
                actor,
                event,
                condition,
                times,
                body,
            } => {
                self.players(actor)?;
                self.event(event)?;
                self.expr(times)?;
                let mut child = self.clone();
                child.replacement = false;
                child.open_searches.clear();
                child.event = crate::semantics::event_bindings(event);
                condition.iter().try_for_each(|c| child.condition(c))?;
                child.operand(body)
            }
            Operation::AddPhases {
                actor,
                count,
                at_start,
                ..
            } => {
                self.players(actor)?;
                self.expr(count)?;
                at_start.iter().try_for_each(|o| self.operand(o))
            }
            Operation::ChooseAndPay {
                actor,
                objects,
                cost,
            } => {
                self.players(actor)?;
                self.objects(objects)?;
                self.cost(cost)
            }
            Operation::DuplicateCounters { from, to, .. } => {
                self.recipients(from)?;
                self.recipients(to)
            }
            Operation::EffectOn {
                subject,
                effects,
                until,
            } => {
                match subject {
                    EffectSubject::Objects(objects) => self.objects(objects)?,
                    EffectSubject::EachSpell(spells) => self.filter(spells)?,
                    EffectSubject::NextSpell { spells, caster, .. } => {
                        self.filter(spells)?;
                        self.player(caster)?;
                    }
                    EffectSubject::Players(players) => self.players(players)?,
                }
                if let Some(until) = until {
                    self.duration(until)?;
                }
                effects.iter().try_for_each(|effect| self.operand(effect))
            }
            Operation::GameEffect { until, effect } => {
                self.duration(until)?;
                self.operand(effect)
            }
            Operation::AttachChosen {
                actor,
                attachments,
                count,
                hosts,
            } => {
                self.players(actor)?;
                self.objects(attachments)?;
                self.count(count)?;
                self.objects(hosts)
            }
            Operation::DrawUpTo { players, amount } => {
                self.players(players)?;
                self.expr(amount)
            }
            Operation::DiscardAllBut { players, keep } => {
                self.players(players)?;
                self.expr(keep)
            }
            Operation::SacrificeAllBut {
                players,
                objects,
                keep,
            } => {
                self.players(players)?;
                self.objects(objects)?;
                self.expr(keep)
            }
            Operation::TowardPlayer { actor, player, .. } => {
                self.players(actor)?;
                self.player(player)
            }
            Operation::Intensify { cards, owner } => {
                self.filter(cards)?;
                self.player(owner)
            }
            Operation::FlipCoins { actor, count } => {
                self.players(actor)?;
                self.expr(count)
            }
            Operation::Wish { actor, filter, .. } => {
                self.players(actor)?;
                self.filter(filter)
            }
            Operation::PutSticker { actor, objects, .. } => {
                self.players(actor)?;
                self.objects(objects)
            }
            Operation::BeginOnBattlefield { actor, flags } => {
                self.players(actor)?;
                self.operand(flags)
            }
            Operation::Manifest { actor, source, .. } => {
                self.players(actor)?;
                match source {
                    ManifestSource::TopOfLibrary { player, count } => {
                        self.player(player)?;
                        self.expr(count)
                    }
                    ManifestSource::Cards(cards) => self.objects(cards),
                }
            }
            Operation::Vote { actor, ballot, .. } => {
                self.players(actor)?;
                match ballot {
                    Ballot::Words(_) => Ok(()),
                    Ballot::Players(players) => self.players(players),
                    Ballot::Objects(objects) | Ballot::UpToOne(objects) => self.objects(objects),
                    Ballot::GraveyardCards(cards) => self.filter(cards),
                }
            }
            Operation::Clash {
                actor,
                opponents,
                won,
                lost,
            } => {
                self.players(actor)?;
                self.players(opponents)?;
                let mut child = self.clone();
                child.open_searches.clear();
                child.clashing = true;
                won.iter()
                    .chain(lost)
                    .try_for_each(|p| child.clone().program(p))
            }
            Operation::DraftFromSpellbook { actor, times, .. } => {
                self.players(actor)?;
                self.expr(times)
            }
            Operation::CreateEmblem { actor, abilities } => {
                self.players(actor)?;
                abilities
                    .iter()
                    .try_for_each(|a| self.operand(&Operand::Ability(Box::new(a.clone()))))
            }
            Operation::MoveCounters {
                actor,
                from,
                to,
                amount,
                ..
            } => {
                self.players(actor)?;
                self.objects(from)?;
                self.objects(to)?;
                match amount {
                    MovedCounters::Exactly(amount) | MovedCounters::AtLeast(amount) => {
                        self.expr(amount)
                    }
                    _ => Ok(()),
                }
            }
            Operation::EachCounterKind {
                actor,
                among,
                options,
                ..
            } => {
                self.players(actor)?;
                self.objects(among)?;
                options.iter().try_for_each(|option| match option {
                    CounterStep::PutThatKind(object)
                    | CounterStep::Put(_, object)
                    | CounterStep::RemoveThatKind(object) => self.object(object),
                })
            }
            Operation::DoubleCounters { objects, .. } => self.objects(objects),
            Operation::Conjure {
                actor,
                card,
                count,
                owner,
                spot,
                entry,
                ..
            } => {
                self.players(actor)?;
                self.expr(count)?;
                if let ConjuredCard::Duplicate(objects) = card {
                    self.objects(objects)?;
                }
                if let Some(owner) = owner {
                    self.player(owner)?;
                }
                if let Some(ConjureSpot::AmongTop(n) | ConjureSpot::FromTop(n)) = spot {
                    self.expr(n)?;
                }
                entry.iter().try_for_each(|o| self.operand(o))
            }
            Operation::ChangeSpell { subject, changes } => {
                match subject {
                    SpellSubject::One(spell) => self.object(spell)?,
                    SpellSubject::Next { spells, player, .. } => {
                        self.filter(spells)?;
                        self.player(player)?;
                    }
                }
                changes.iter().try_for_each(|change| self.change(change))
            }
            Operation::PlayCards {
                actor,
                cards,
                count,
                ..
            } => {
                self.players(actor)?;
                self.objects(cards)?;
                self.count(count)
            }
            Operation::PlayWith {
                actor,
                cards,
                count,
                terms,
                group,
                ..
            } => {
                self.players(actor)?;
                self.objects(cards)?;
                self.count(count)?;
                self.play_cost(&terms.cost)?;
                if let Some(then) = &terms.then {
                    self.destination(then)?;
                }
                terms.effects.iter().try_for_each(|o| self.operand(o))?;
                group.iter().try_for_each(|o| self.operand(o))
            }
            Operation::ChangeZone {
                actor,
                cards,
                count,
                group,
                destination,
                entry,
                face_down,
                ..
            } => {
                self.players(actor)?;
                self.objects(cards)?;
                if let Some(count) = count {
                    self.count(count)?;
                }
                self.destination(destination)?;
                if let Some(face_down) = face_down {
                    self.operand(&face_down.power_toughness)?;
                    face_down
                        .creature_type
                        .iter()
                        .try_for_each(|o| self.operand(o))?;
                }
                entry.iter().try_for_each(|o| self.operand(o))?;
                group.iter().try_for_each(|o| self.operand(o))
            }
            Operation::CopyCards {
                actor,
                objects,
                times,
            } => {
                self.players(actor)?;
                self.objects(objects)?;
                self.expr(times)
            }
            Operation::ChooseAtRandom { options, .. } => {
                options.iter().try_for_each(|o| self.operand(o))
            }
            Operation::ChooseAction { actor, options, .. } => {
                self.players(actor)?;
                options.iter().try_for_each(|o| self.operand(o))
            }
            Operation::ChooseValue { actor, options, .. } => {
                self.players(actor)?;
                options.iter().try_for_each(|o| self.operand(o))
            }
            Operation::Keyword {
                actor,
                object,
                other,
                amount,
                ..
            } => {
                self.players(actor)?;
                object
                    .iter()
                    .chain(other)
                    .try_for_each(|o| self.object(o))?;
                amount.iter().try_for_each(|n| self.expr(n))
            }
            Operation::CreateDelayedTrigger {
                event,
                start,
                expires,
                intervening_if,
                body,
            } => {
                self.event(event)?;
                if let DelayedStart::PlayersNextTurn(player) = start {
                    self.player(player)?;
                }
                if let DelayedStart::ExtraTurn(result) = start {
                    if !self.results.contains_key(&result.0) {
                        return Err(format!("result {} created no extra turn", result.0));
                    }
                }
                if let Some(duration) = expires {
                    self.duration(duration)?;
                    if matches!(duration, Duration::Indefinite) {
                        return Err("indefinite delayed expiration should be absent".into());
                    }
                }
                // Lexical closure: preserve creator bindings, not the turn
                // event's bindings. New results/searches are local to this
                // future invocation; its mutations do not occur at creation.
                let mut child = self.clone();
                child.replacement = false;
                child.open_searches.clear();
                // A row's event says what it binds, as a typed one's creator did.
                if crate::semantics::delayed_reads_event(event) {
                    child.event = crate::semantics::with_event(&child.event, event);
                }
                if let Some(condition) = intervening_if {
                    child.condition(condition)?;
                }
                child.program(body)
            }
            Operation::LookAt { player, objects } => {
                self.player(player)?;
                self.objects(objects)
            }
            Operation::FlipCoin { player } => self.player(player),
            Operation::CreateRegenerationShields { objects } => {
                self.require_zone(objects, EventZone::Battlefield)
            }
            Operation::Search {
                player,
                library,
                top,
                choices,
                ..
            } => {
                self.player(player)?;
                self.player(library)?;
                if let Some(n) = top {
                    self.count(&Count::Exactly(n.clone()))?;
                }
                for choice in choices {
                    self.selection_constraints(&choice.constraints)?;
                    self.filter(&choice.filter)?;
                    self.count(&choice.count)?;
                    let whole_group =
                        matches!(choice.count, Count::Exactly(_)) && !choice.constraints.is_empty();
                    if !whole_group
                        && !matches!(
                            choice.count,
                            Count::UpTo(_) | Count::AsManyAsCan(_) | Count::AnyNumber
                        )
                    {
                        return Err(
                            "search choice requires a fail-to-find or as-many-as-possible policy"
                                .into(),
                        );
                    }
                }
                Ok(())
            }
            Operation::Reveal { objects } => self.objects(objects),
            Operation::RevealUntil {
                library,
                filter,
                count,
                at_most,
            } => {
                self.player(library)?;
                self.expr(count)?;
                if let Some(at_most) = at_most {
                    self.expr(at_most)?;
                }
                self.filter(filter)
            }
            Operation::RevealUntilGroup {
                library,
                constraints,
            } => {
                self.player(library)?;
                self.selection_constraints(constraints)
            }
            Operation::Shuffle { players, excluding } => {
                self.players(players)?;
                if let Some(objects) = excluding {
                    self.require_zone(objects, EventZone::Library)?;
                }
                Ok(())
            }
            Operation::ReorderLibrary {
                player,
                library,
                objects,
                ..
            } => {
                self.player(player)?;
                self.player(library)?;
                self.require_zone(objects, EventZone::Library)
            }
            Operation::Scry { players, amount } | Operation::Surveil { players, amount } => {
                self.players(players)?;
                self.count(&Count::Exactly(amount.clone()))
            }
            Operation::JoinResults { results } if results.len() < 2 => {
                Err("a join needs at least two branches".into())
            }
            Operation::JoinResults { .. } => Ok(()),
            Operation::ChoosePowerToughness { player, .. } => self.player(player),
            Operation::Counter {
                objects,
                destination,
            } => {
                self.require_zone(objects, EventZone::Stack)?;
                self.destination(destination)
            }
            Operation::Move {
                objects,
                destination,
                entry,
                face_down,
            } => {
                self.objects(objects)?;
                self.destination(destination)?;
                if entry.tapped
                    && !matches!(
                        destination,
                        Destination::Battlefield(_) | Destination::OwnersBattlefield
                    )
                {
                    return Err("tapped arrival requires a battlefield destination".into());
                }
                if *face_down && !matches!(destination, Destination::Exile) {
                    return Err("face-down movement currently requires exile".into());
                }
                self.entry(entry)?;
                Ok(())
            }
            Operation::ModifyEvent { change } => {
                if !self.replacement {
                    return Err("event change used outside a replacement program".into());
                }
                match change {
                    EventChange::SetTapped(_)
                    | EventChange::AddCopy { .. }
                    | EventChange::FlipExtraCoin
                    | EventChange::DamageAsInfect
                    | EventChange::IgnoreCounters(_) => Ok(()),
                    EventChange::SearchTop(amount) | EventChange::Repeats(amount) => {
                        self.expr(amount)
                    }
                    EventChange::AddCounters { amount, .. } => {
                        self.count(&Count::Exactly(amount.clone()))
                    }
                    EventChange::SetController(player) => self.player(player),
                    EventChange::ExchangeDieResult {
                        decider,
                        object,
                        properties,
                    } => {
                        self.player(decider)?;
                        self.object(object)?;
                        if properties.len() < 2 {
                            return Err("an exchange offers at least two properties".into());
                        }
                        Ok(())
                    }
                    EventChange::PreventDamage
                    | EventChange::Cancel
                    | EventChange::SetDestination(_) => Ok(()),
                    EventChange::PreventSomeDamage(amount)
                    | EventChange::PreventAllButDamage(amount)
                    | EventChange::SetAmount(amount) => self.count(&Count::Exactly(amount.clone())),
                    EventChange::RedirectDamage(recipients) => self.recipients(recipients),
                    EventChange::RedirectSomeDamage(amount, recipients) => {
                        self.expr(amount)?;
                        self.recipients(recipients)
                    }
                    EventChange::Entry(state) => self.entry(&EntrySpec {
                        states: vec![state.clone()],
                        ..EntrySpec::default()
                    }),
                    EventChange::AddMana(mana)
                    | EventChange::ReplaceMana(mana)
                    | EventChange::ReplaceColoredMana(mana) => self.mana(mana),
                    EventChange::RollExtraDie(DiceIgnore::Lowest) => Ok(()),
                    EventChange::RollExtraDie(DiceIgnore::ChosenBy(player)) => self.player(player),
                    EventChange::Then(body) => {
                        // The event is over, so nothing here can change it.
                        let mut child = self.clone();
                        child.replacement = false;
                        child.program(body)
                    }
                    EventChange::ReplaceTokens { tokens, .. } => tokens
                        .iter()
                        .try_for_each(|token| self.count(&Count::Exactly(token.amount.clone()))),
                }
            }
            Operation::CreateTrigger {
                event,
                duration,
                announcement,
                body,
                intervening_if,
                ..
            } => {
                self.duration(duration)?;
                self.event(event)?;
                let mut child = self.clone();
                child.replacement = false;
                child.open_searches.clear();
                child.event = crate::semantics::event_bindings(event);
                intervening_if
                    .iter()
                    .try_for_each(|condition| child.condition(condition))?;
                child.announcement(announcement)?;
                child.program(body)
            }
            Operation::BindValue { value } => self.expr(value),
            Operation::RepeatProcess => {
                if self.repeatable {
                    Ok(())
                } else {
                    Err("nothing to repeat in this scope".into())
                }
            }
            Operation::DiscardObjects { objects } => self.objects(objects),
            Operation::ReflexiveTrigger {
                performed,
                condition,
                announcement,
                body,
            } => {
                if performed
                    .as_ref()
                    .is_some_and(|performed| !self.results.contains_key(&performed.0))
                {
                    return Err("a reflexive trigger reads an action that has not run".into());
                }
                if let Some(condition) = condition {
                    self.condition(condition)?;
                }
                let mut child = self.clone();
                child.replacement = false;
                child.open_searches.clear();
                child.announcement(announcement)?;
                child.program(body)
            }
            Operation::ReturnAfter { objects, duration } => {
                self.duration(duration)?;
                self.objects(objects)
            }
            Operation::CreateReplacement {
                event,
                limit,
                duration,
                body,
            } => {
                self.duration(duration)?;
                if let ReplacementLimit::TotalDamage(amount)
                | ReplacementLimit::DamageToEach(amount) = limit
                {
                    self.count(&Count::Exactly(amount.clone()))?;
                }
                self.replacement_event(event)?;
                let mut child = self.clone();
                child.replacement = true;
                child.open_searches.clear();
                child.event = crate::semantics::replacement_bindings(event);
                child.program(body)
            }
            Operation::PayCost { player, cost, .. } => {
                self.player(player)?;
                self.cost(cost)
            }
            Operation::PayRepeatedly {
                player,
                cost,
                at_most,
            } => {
                self.player(player)?;
                self.cost(cost)?;
                self.expr(at_most)
            }
            Operation::RemoveCounters { removals } => self.removals(removals),
            Operation::Attach { attachments, host } => {
                self.require_zone(attachments, EventZone::Battlefield)?;
                self.require_zone(&ObjectSet::One(host.clone()), EventZone::Battlefield)
            }
            Operation::CreateTokens {
                players,
                tokens,
                entry,
            } => {
                self.players(players)?;
                if tokens.is_empty() {
                    return Err("empty token creation batch".into());
                }
                for (i, token) in tokens.iter().enumerate() {
                    self.count(&Count::Exactly(token.amount.clone()))?;
                    let definition = match &token.recipe {
                        TokenRecipe::Defined(definition) => definition,
                        TokenRecipe::Copy {
                            object,
                            adjustments,
                        } => {
                            self.object(object)?;
                            adjustments.iter().try_for_each(|o| self.operand(o))?;
                            continue;
                        }
                        TokenRecipe::CopyEach {
                            objects,
                            adjustments,
                        }
                        | TokenRecipe::CopyOne {
                            objects,
                            adjustments,
                        } => {
                            self.objects(objects)?;
                            adjustments.iter().try_for_each(|o| self.operand(o))?;
                            continue;
                        }
                        TokenRecipe::Named(name) if name.is_empty() => {
                            return Err("named token recipe is empty".into());
                        }
                        TokenRecipe::CopyNamed { name, .. } if name.is_empty() => {
                            return Err("named token recipe is empty".into());
                        }
                        TokenRecipe::CopyNamed { adjustments, .. } => {
                            adjustments.iter().try_for_each(|o| self.operand(o))?;
                            continue;
                        }
                        TokenRecipe::CopyAtRandom { cards } => {
                            self.operand(cards)?;
                            continue;
                        }
                        TokenRecipe::Named(_) => continue,
                    };
                    if matches!(&definition.name, TokenName::Explicit(name) if name.is_empty()) {
                        return Err("explicit token name is empty".into());
                    }
                    if definition
                        .colors
                        .iter()
                        .any(|c| matches!(c, ManaColor::Colorless))
                    {
                        return Err("colorless is not a token color".into());
                    }
                    if let Some((objects, kinds)) = &definition.abilities_of {
                        self.objects(objects)?;
                        self.operand(kinds)?;
                    }
                    if let Some((power, toughness)) = &definition.power_toughness {
                        self.expr(power)?;
                        self.expr(toughness)?;
                    }
                    for (j, ability) in definition.abilities.iter().enumerate() {
                        validate_ability(ability)
                            .map_err(|e| format!("tokens[{i}].definition.abilities[{j}]: {e}"))?;
                    }
                }
                self.entry(entry)?;
                Ok(())
            }
            Operation::SeparatePiles {
                separator,
                chooser,
                objects,
            } => {
                self.player(separator)?;
                self.player(chooser)?;
                self.objects(objects)
            }
            Operation::ChooseObjects {
                player,
                objects,
                count,
                constraints,
            } => {
                self.player(player)?;
                self.objects(objects)?;
                self.count(count)?;
                self.selection_constraints(constraints)
            }
            Operation::PutCounters { placements } => {
                if placements.is_empty() {
                    return Err("empty counter placement batch".into());
                }
                for placement in placements {
                    self.recipients(&placement.recipients)?;
                    self.count(&Count::Exactly(placement.amount.clone()))?;
                    if let Some(selection) = &placement.selection {
                        self.player(&selection.player)?;
                        self.count(&selection.count)?;
                    }
                }
                Ok(())
            }
            Operation::Draw { players, amount }
            | Operation::Discard {
                players, amount, ..
            }
            | Operation::SetLifeTotal { players, amount }
            | Operation::GainLife { players, amount }
            | Operation::LoseLife { players, amount }
            | Operation::Mill { players, amount } => {
                self.players(players)?;
                self.expr(amount)
            }
            Operation::AddMana {
                players,
                mana,
                restriction,
            } => {
                self.players(players)?;
                self.mana(mana)?;
                if let Some(r) = restriction {
                    self.mana_use(r)?;
                }
                Ok(())
            }
            Operation::DealDamage {
                source,
                recipients,
                amount,
                ..
            } => {
                self.object(source)?;
                self.recipients(recipients)?;
                self.expr(amount)
            }
            Operation::DealDamageWith {
                source,
                recipients,
                amount,
                rider,
            } => {
                self.object(source)?;
                self.recipients(recipients)?;
                self.expr(amount)?;
                match rider {
                    DamageRider::DividedEvenly(_) => Ok(()),
                    DamageRider::ExcessTo(_) if matches!(recipients, Recipients::Players(_)) => {
                        Err("a player has no lethal damage to exceed".into())
                    }
                    DamageRider::ExcessTo(player) | DamageRider::DividedBy(player) => {
                        self.player(player)
                    }
                    DamageRider::Prevented(prevented) => self.expr(prevented),
                }
            }
            Operation::DealDamageParts { source, parts, .. } => {
                self.object(source)?;
                for part in parts {
                    self.recipients(&part.recipients)?;
                    self.expr(&part.amount)?;
                }
                Ok(())
            }
            Operation::DealDamageFromEach {
                sources,
                recipients,
                amount,
            } => {
                self.require_zone(sources, EventZone::Battlefield)?;
                let mut per_object = self.clone();
                per_object.affected = true;
                per_object.recipients(recipients)?;
                per_object.expr(amount)
            }
            Operation::Destroy { objects, .. } | Operation::Tap { objects, .. } => {
                self.require_zone(objects, EventZone::Battlefield)
            }
            Operation::Sacrifice {
                players,
                objects,
                choose,
            } => {
                self.players(players)?;
                self.require_zone(objects, EventZone::Battlefield)?;
                if let Some(count) = choose {
                    self.count(count)?;
                }
                Ok(())
            }
            Operation::CreateContinuousEffect {
                source,
                affected,
                changes,
                duration,
                evaluation,
                ..
            } => {
                let mut per_object = self.clone();
                per_object.affected = matches!(affected, EffectSubjects::Objects(_));
                per_object.duration(duration)?;
                let rule = |c: &Change| matches!(c, Change::Rule(_) | Change::PlayerRule(_));
                let has_rules = changes.iter().any(rule);
                if has_rules && changes.iter().any(|c| !rule(c)) {
                    return Err("mixed rule and characteristic changes require independently evaluated parts".into());
                }
                if has_rules != matches!(evaluation, EffectEvaluation::LiveObjects) {
                    return Err("rule changes require live subject queries with captured resolution variables".into());
                }
                self.object(source)?;
                self.subject_changes(affected, changes)?;
                Ok(())
            }
        }
    }

    fn selection_constraints(&self, constraints: &[SelectionConstraint]) -> Result {
        for constraint in constraints {
            if let SelectionConstraint::TotalProperty { amount, .. }
            | SelectionConstraint::DistinctCount { amount, .. }
            | SelectionConstraint::TotalManaSymbols { amount, .. } = constraint
            {
                self.expr(amount)?;
            }
            if let SelectionConstraint::NamedAfterOneEach(objects) = constraint {
                self.objects(objects)?;
            }
            if let SelectionConstraint::OneForEach(filters) = constraint {
                for filter in filters {
                    self.filter(filter)?;
                }
            }
            if let SelectionConstraint::CanMoveTo {
                destination, entry, ..
            } = constraint
            {
                self.destination(destination)?;
                self.entry(entry)?;
            }
        }
        Ok(())
    }

    fn mana(&self, mana: &ManaProduction) -> Result {
        match mana {
            ManaProduction::SameTypeAsProduced
            | ManaProduction::CommanderIdentity
            | ManaProduction::DifferentColors(_)
            | ManaProduction::OtherColor
            | ManaProduction::LastNoted { .. } => Ok(()),
            ManaProduction::DraftNotedColor(note) => self.player(&note.player),
            ManaProduction::ColorsAmong { objects, .. } => self.objects(objects),
            ManaProduction::ManaCostOf(object) | ManaProduction::SymbolsOf(object) => {
                self.object(object)
            }
            ManaProduction::LostThisWay(result) if self.results.contains_key(&result.0) => Ok(()),
            ManaProduction::LostThisWay(result) => {
                Err(format!("result {} is unavailable", result.0))
            }
            ManaProduction::CouldProduce { objects, .. } => self.objects(objects),
            ManaProduction::Primitive { name, operands } => self.primitive(name, operands),
            ManaProduction::Amount { amount, .. } => self.count(&Count::Exactly(amount.clone())),
            ManaProduction::All(v) | ManaProduction::ChooseOne(v) => {
                if v.is_empty() {
                    return Err("empty mana production expression".into());
                }
                for p in v {
                    self.mana(p)?;
                }
                Ok(())
            }
            ManaProduction::Repeat {
                amount, production, ..
            } => {
                self.count(&Count::Exactly(amount.clone()))?;
                self.mana(production)
            }
        }
    }
    fn mana_use(&self, r: &ManaUse) -> Result {
        match r {
            ManaUse::CastSpell(f) | ManaUse::ActivateAbility { sources: f, .. } => self.filter(f),
            ManaUse::All(v) | ManaUse::Either(v) => {
                for r in v {
                    self.mana_use(r)?;
                }
                Ok(())
            }
            ManaUse::Not(r) => self.mana_use(r),
            ManaUse::Persists(until) => self.duration(until),
            ManaUse::SpentOn {
                spells,
                changes,
                until,
            } => {
                self.filter(spells)?;
                changes.iter().try_for_each(|c| self.change(c))?;
                self.duration(until)
            }
            ManaUse::Purpose(ManaPurpose::CostContaining(symbol)) => self.operand(symbol),
            ManaUse::Purpose(_) => Ok(()),
            ManaUse::WhenSpent { spells, body, .. } => {
                self.filter(spells)?;
                let mut scoped = self.clone();
                scoped.event = crate::semantics::event_bindings(&EventSpec::Spell {
                    event: SpellEvent::Cast,
                    players: PlayerSet::All,
                    spells: spells.clone(),
                });
                scoped.event.stack_object = true;
                scoped.operand(body)
            }
            ManaUse::SpentGives { spells, effects } => {
                self.filter(spells)?;
                effects.iter().try_for_each(|effect| self.operand(effect))
            }
            ManaUse::Primitive { name, operands } => self.primitive(name, operands),
        }
    }

    /// A move in a cost or an optional body stays readable; it reads nothing
    /// if that body never ran.
    fn adopt_moves(&mut self, body: &Program) {
        for instruction in &body.instructions {
            match instruction {
                Instruction::Effect { result, operation } => {
                    if crate::semantics::singular_operation(operation, |r| {
                        self.singular.contains(&r.0)
                    }) {
                        self.singular.insert(result.0);
                    }
                    if matches!(
                        operation,
                        Operation::Move { .. }
                            | Operation::Destroy { .. }
                            | Operation::Sacrifice { .. }
                            | Operation::Discard { .. }
                            | Operation::DiscardObjects { .. }
                            | Operation::DiscardAllBut { .. }
                            | Operation::Mill { .. }
                    ) {
                        self.results.insert(result.0, ResultKind::ObjectTransition);
                    }
                    if matches!(operation, Operation::CreateTokens { .. }) {
                        self.results.insert(result.0, ResultKind::CreatedObjects);
                    }
                    if let Operation::ChooseObjects { objects, .. } = operation {
                        self.results.insert(result.0, ResultKind::Selection);
                        if let Some(zones) = self.known_zones(objects) {
                            self.result_zones.insert(result.0, zones);
                        }
                    }
                }
                Instruction::Group(body) | Instruction::Optional { body, .. } => {
                    self.adopt_moves(body)
                }
                _ => {}
            }
        }
    }

    /// A player loop's picks and moves stay readable as a set, never as one
    /// object; anything else it did can still be asked whether it happened.
    fn adopt_choices(&mut self, body: &Program, run: &Self) {
        for instruction in &body.instructions {
            match instruction {
                Instruction::Effect {
                    result,
                    operation: Operation::ChooseObjects { .. },
                } => {
                    self.results.insert(result.0, ResultKind::Selection);
                    self.gathered.insert(result.0);
                    if let Some(zones) = run.result_zones.get(&result.0) {
                        self.result_zones.insert(result.0, zones.clone());
                    }
                }
                Instruction::Effect { result, operation } => {
                    if run.ordered_results.contains(&result.0) {
                        self.ordered_results.insert(result.0);
                    }
                    let moved = matches!(
                        operation,
                        Operation::Move { .. }
                            | Operation::Destroy { .. }
                            | Operation::Sacrifice { .. }
                            | Operation::Discard { .. }
                            | Operation::DiscardObjects { .. }
                            | Operation::DiscardAllBut { .. }
                            | Operation::Mill { .. }
                            | Operation::Draw { .. }
                            | Operation::DrawUpTo { .. }
                    );
                    self.results.insert(
                        result.0,
                        if moved {
                            ResultKind::ObjectTransition
                        } else if matches!(
                            operation,
                            Operation::Reveal { .. } | Operation::RevealUntil { .. }
                        ) {
                            ResultKind::Observation
                        } else if matches!(operation, Operation::CreateTokens { .. }) {
                            ResultKind::CreatedObjects
                        } else if crate::semantics::makes_card(operation) {
                            ResultKind::ObjectTransition
                        } else {
                            ResultKind::Other
                        },
                    );
                    self.gathered.insert(result.0);
                }
                Instruction::Group(body)
                | Instruction::Optional { body, .. }
                | Instruction::ForEachPlayer { body, .. } => self.adopt_choices(body, run),
                _ => {}
            }
        }
    }

    fn program(&mut self, program: &Program) -> Result {
        for (i, instruction) in program.instructions.iter().enumerate() {
            let checked: Result = (|| {
                match instruction {
                    Instruction::Effect { result, operation } => {
                        self.operation(operation)?;
                        if let Operation::PayCost { cost, .. } = operation {
                            for part in &cost.parts {
                                if let CostPart::PlayerAction { body, .. } = part {
                                    self.adopt_moves(body);
                                }
                            }
                        }
                        let kind = match operation {
                            Operation::LookAt { .. }
                            | Operation::Reveal { .. }
                            | Operation::RevealUntil { .. }
                            | Operation::RevealUntilGroup { .. } => ResultKind::Observation,
                            Operation::Search { .. } => ResultKind::Search,
                            Operation::PayCost { .. } | Operation::PayRepeatedly { .. } => {
                                ResultKind::Payment
                            }
                            Operation::CreateTokens { .. } => ResultKind::CreatedObjects,
                            Operation::ChooseObjects { .. }
                            | Operation::SeparatePiles { .. }
                            | Operation::NoteObjects { .. }
                            // Bolster chooses the creature it counters (701.39a).
                            | Operation::Keyword {
                                action: KeywordAction::Bolster,
                                ..
                            } => ResultKind::Selection,
                            Operation::ReorderLibrary { .. }
                            | Operation::Destroy { .. }
                            | Operation::Move { .. }
                            | Operation::Counter { .. }
                            | Operation::Tap { .. }
                            | Operation::Discard { .. }
                            | Operation::DiscardObjects { .. }
                            | Operation::Sacrifice { .. }
                            | Operation::JoinResults { .. }
                            | Operation::Draw { .. }
                            | Operation::Mill { .. } => ResultKind::ObjectTransition,
                            other if crate::semantics::makes_card(other) => {
                                ResultKind::ObjectTransition
                            }
                            other => match crate::semantics::yields(other) {
                                Some("observed") => ResultKind::Observation,
                                Some("extra_turn") => ResultKind::Other,
                                Some("created") => ResultKind::CreatedObjects,
                                Some("paid" | "took") => ResultKind::Other,
                                Some("chosen") => ResultKind::Selection,
                                Some(_) => ResultKind::ObjectTransition,
                                None => ResultKind::Other,
                            },
                        };
                        let zones = match operation {
                            Operation::LookAt { objects, .. } | Operation::Reveal { objects } => {
                                self.known_zones(objects)
                            }
                            Operation::Search { .. }
                            | Operation::ReorderLibrary { .. }
                            | Operation::RevealUntil { .. }
                            | Operation::RevealUntilGroup { .. } => Some(vec![EventZone::Library]),
                            Operation::Draw { .. } => Some(vec![EventZone::Hand]),
                            Operation::ChooseObjects { objects, .. }
                            | Operation::SeparatePiles { objects, .. } => self.known_zones(objects),
                            _ => None,
                        };
                        if let Operation::PayCost { cost, .. } = operation {
                            self.payment_costs.insert(result.0, cost.clone());
                        }
                        if crate::semantics::singular_operation(operation, |r| {
                            self.singular.contains(&r.0)
                        }) {
                            self.singular.insert(result.0);
                        }
                        if self.results.insert(result.0, kind).is_some() {
                            return Err(format!("duplicate result {}", result.0));
                        }
                        if matches!(
                            operation,
                            Operation::RevealUntil { .. } | Operation::RevealUntilGroup { .. }
                        ) {
                            self.ordered_results.insert(result.0);
                        }
                        if let Operation::LookAt { objects, .. }
                        | Operation::Reveal { objects }
                        | Operation::Move { objects, .. } = operation
                        {
                            if self.ordered(objects) {
                                self.ordered_results.insert(result.0);
                            }
                        }
                        if let Some(zones) = zones {
                            self.result_zones.insert(result.0, zones);
                        }
                        match operation {
                            Operation::Search { library, .. } => {
                                self.open_searches.push(library.clone());
                                self.search_libraries.insert(result.0, library.clone());
                            }
                            Operation::Shuffle { players, .. } => self
                                .open_searches
                                .retain(|p| !crate::semantics::shuffle_closes(p, players)),
                            _ => {}
                        }
                    }
                    Instruction::Group(program) => self.program(program)?,
                    Instruction::If {
                        condition,
                        then,
                        otherwise,
                    } => {
                        self.condition(condition)?;
                        let mut yes = self.clone();
                        yes.program(then)?;
                        let mut no = self.clone();
                        no.program(otherwise)?;
                        self.open_searches = yes.open_searches;
                        for p in no.open_searches {
                            if !self
                                .open_searches
                                .iter()
                                .any(|q| crate::semantics::same_player(&p, q))
                            {
                                self.open_searches.push(p);
                            }
                        }
                        // If no search occurred there was no open search; if it
                        // did, this exact guard shuffles and closes that search.
                        if let Condition::Searched(result) = condition {
                            if let (
                                Some(library),
                                [Instruction::Effect {
                                    operation:
                                        Operation::Shuffle {
                                            players: PlayerSet::One(shuffled),
                                            ..
                                        },
                                    ..
                                }],
                            ) = (
                                self.search_libraries.get(&result.0),
                                then.instructions.as_slice(),
                            ) {
                                if otherwise.instructions.is_empty()
                                    && crate::semantics::same_player(library, shuffled)
                                {
                                    self.open_searches
                                        .retain(|p| !crate::semantics::same_player(p, library));
                                }
                            }
                        }
                    }
                    Instruction::ForEachObject {
                        objects,
                        binding,
                        body,
                    } => {
                        self.objects(objects)?;
                        let mut iterated = self.clone();
                        iterated.bound_objects.insert(binding.0);
                        iterated.program(body)?;
                        self.adopt_choices(body, &iterated);
                    }
                    Instruction::Times { amount, body } => {
                        self.expr(amount)?;
                        let mut repeated = self.clone();
                        repeated.repeatable = true;
                        repeated.program(body)?;
                    }
                    Instruction::Repeatable(body) => {
                        let mut repeated = self.clone();
                        repeated.repeatable = true;
                        repeated.program(body)?;
                        self.adopt_choices(body, &repeated);
                    }
                    Instruction::ForEachPlayer {
                        players,
                        binding,
                        starting_with,
                        body,
                        ..
                    } => {
                        self.players(players)?;
                        if let Some(player) = starting_with {
                            self.player(player)?;
                        }
                        let mut each = self.clone();
                        if !each.players.insert(binding.0) {
                            return Err(format!("duplicate player binding {}", binding.0));
                        }
                        each.program(body)?;
                        self.adopt_choices(body, &each);
                    }
                    Instruction::ChooseOne {
                        decider,
                        options,
                        offered,
                    } => {
                        self.player(decider)?;
                        if options.len() < 2 {
                            return Err("a choice needs at least two options".into());
                        }
                        if let Some(offered) = offered {
                            self.count(&Count::Exactly(offered.clone()))?;
                        }
                        for option in options {
                            self.clone().program(option)?;
                        }
                    }
                    Instruction::Optional { decider, body } => {
                        self.player(decider)?;
                        let mut yes = self.clone();
                        yes.program(body)?;
                        self.adopt_moves(body);
                        for p in yes.open_searches {
                            if !self
                                .open_searches
                                .iter()
                                .any(|q| crate::semantics::same_player(&p, q))
                            {
                                self.open_searches.push(p);
                            }
                        }
                    }
                }
                Ok(())
            })();
            checked.map_err(|e| format!("instructions[{i}]: {e}"))?;
        }
        Ok(())
    }

    fn announcement(&mut self, announcement: &'a Announcement) -> Result {
        let mut proposal = self.clone();
        proposal.announcing = true;
        proposal.payment_costs.clear();
        for target in &announcement.targets {
            if let TargetKind::Object(domain) = &target.kind {
                proposal.domain(domain)?;
            }
            proposal.count(&target.count)?;
            proposal.filter(&target.filter)?;
            proposal.selection_constraints(&target.constraints)?;
            for constraint in &target.constraints {
                if let SelectionConstraint::DifferentFromTarget(slot)
                | SelectionConstraint::SharesLimitWith(slot) = constraint
                {
                    if !self.slots.contains_key(&slot.0) {
                        return Err(format!("target excludes unannounced slot {}", slot.0));
                    }
                }
            }
            if let Some(chooser) = &target.chooser {
                proposal.player(chooser)?;
            }
            if let Some(total) = &target.distribute {
                proposal.expr(total)?;
            }
            if let Some(players) = &target.players {
                proposal.players(players)?;
            }
            if self.slots.insert(target.slot.0, target).is_some() {
                return Err(format!("duplicate target slot {}", target.slot.0));
            }
            proposal.slots.insert(target.slot.0, target);
        }
        if let Some(selection) = &announcement.modes {
            proposal.expr(&selection.minimum)?;
            proposal.expr(&selection.maximum)?;
            if let Some(cost) = &selection.surcharge {
                proposal.cost(cost)?;
            }
            if let Some(players) = &selection.chooser {
                proposal.players(players)?;
            }
            if let Some(budget) = &selection.budget {
                proposal.expr(budget)?;
            }
            if let (Expr::Constant(min), Expr::Constant(max)) =
                (&selection.minimum, &selection.maximum)
            {
                if *min < 0
                    || *max < *min
                    || (!selection.repeat && *max > selection.modes.len() as i64)
                {
                    return Err("invalid mode bounds".into());
                }
            }
            for (i, mode) in selection.modes.iter().enumerate() {
                if let Some(condition) = &mode.requires {
                    proposal.condition(condition)?;
                }
                let mut scope = self.clone();
                scope
                    .announcement(&mode.announcement)
                    .map_err(|e| format!("modes[{i}]: {e}"))?;
                scope
                    .program(&mode.body)
                    .map_err(|e| format!("modes[{i}]: {e}"))?;
            }
        }
        Ok(())
    }
}

pub fn validate_ability(ability: &Ability) -> Result {
    validate_ability_in(ability, None, &BTreeMap::new())
}
// Crew identity carries rules about participants at payment and becoming crewed
// at resolution. Only its reviewed generic definition may carry that identity.
fn valid_crew_definition(
    cost: &Cost,
    limits: &ActivationLimits,
    announcement: &Announcement,
    body: &Program,
) -> bool {
    if limits.once
        || limits.sorcery
        || limits.condition.is_some()
        || limits.activators.is_some()
        || limits.keyword.is_some()
        || !announcement.targets.is_empty()
        || announcement.modes.is_some()
    {
        return false;
    }
    let [CostPart::Constrained { part, constraints }] = cost.parts.as_slice() else {
        return false;
    };
    let [SelectionConstraint::TotalProperty {
        property: Property::Power,
        comparison: Comparison::GreaterOrEqual,
        amount: Expr::Constant(threshold),
    }] = constraints.as_slice()
    else {
        return false;
    };
    if *threshold < 0 {
        return false;
    }
    let CostPart::ChangeTapState {
        selections,
        tapped: true,
        symbol: false,
    } = part.as_ref()
    else {
        return false;
    };
    let [CostObjectSelection {
        objects: ObjectSet::Battlefield(ObjectFilter::All(filters)),
        choose: Some(Count::AnyNumber),
    }] = selections.as_slice()
    else {
        return false;
    };
    if filters.len()!=4 || !filters.iter().any(|f|matches!(f,ObjectFilter::CardType(crate::mtg_types::CardType::Creature)))
        || !filters.iter().any(|f|matches!(f,ObjectFilter::Not(inner) if matches!(inner.as_ref(),ObjectFilter::Tapped)))
        || !filters.iter().any(|f|matches!(f,ObjectFilter::ControlledBy(PlayerSet::One(PlayerRef::AbilityController))))
        || !filters.iter().any(|f|matches!(f,ObjectFilter::Not(inner) if matches!(inner.as_ref(),ObjectFilter::SameObject(ObjectRef::Source)))) {return false;}
    matches!(body.instructions.as_slice(),[Instruction::Effect {operation:Operation::CreateContinuousEffect {
        source:ObjectRef::Source,affected:EffectSubjects::Objects(ObjectSet::One(ObjectRef::Source)),changes,
        duration:Duration::EndOfTurn,evaluation:EffectEvaluation::Snapshot},..}]
        if matches!(changes.as_slice(),[Change::AddCardType(crate::mtg_types::CardType::Artifact),Change::AddCardType(crate::mtg_types::CardType::Creature)]))
}

fn valid_cycling_body(origin: &ActivationOrigin, body: &Program) -> bool {
    let ActivationOrigin::Cycling { search } = origin else {
        return false;
    };
    let Some(filter) = search else {
        return matches!(
            body.instructions.as_slice(),
            [Instruction::Effect {
                operation: Operation::Draw {
                    players: PlayerSet::One(PlayerRef::AbilityController),
                    amount: Expr::Constant(1)
                },
                ..
            }]
        );
    };
    let [Instruction::Group(body)] = body.instructions.as_slice() else {
        return false;
    };
    let [Instruction::Effect {
        result,
        operation:
            Operation::Search {
                player: PlayerRef::AbilityController,
                library: PlayerRef::AbilityController,
                top: None,
                choices,
                ..
            },
    }, Instruction::Effect {
        operation: Operation::Reveal {
            objects: ObjectSet::Selected(reveal),
        },
        ..
    }, Instruction::Effect {
        operation:
            Operation::Move {
                objects: ObjectSet::Selected(moved),
                destination: Destination::OwnersHand,
                entry,
                face_down: false,
            },
        ..
    }, Instruction::Effect {
        operation:
            Operation::Shuffle {
                players: PlayerSet::One(PlayerRef::AbilityController),
                excluding: None,
            },
        ..
    }] = body.instructions.as_slice()
    else {
        return false;
    };
    choices.len() == 1
        && matches!(choices[0].count, Count::UpTo(Expr::Constant(1)))
        && choices[0].constraints.is_empty()
        && serde_json::to_value(&choices[0].filter).ok() == serde_json::to_value(filter).ok()
        && reveal.0 == result.0
        && moved.0 == result.0
        && !entry.tapped
        && entry.counters.is_empty()
        && entry.states.is_empty()
}
fn has_definition(ability: &Ability) -> bool {
    match ability {
        Ability::Static {
            kind: StaticKind::CharacteristicDefining { .. },
            ..
        } => true,
        Ability::Scoped { ability, .. } => has_definition(ability),
        Ability::Conditional {
            then, otherwise, ..
        } => then.iter().chain(otherwise).any(has_definition),
        _ => false,
    }
}

fn validate_ability_in(
    ability: &Ability,
    zones: Option<&[EventZone]>,
    casting_costs: &BTreeMap<u32, Cost>,
) -> Result {
    let mut scope = Scope::default();
    let (announcement, body) = match ability {
        Ability::Named(named) => return scope.named(named),
        Ability::Primitive { name, operands } => return scope.primitive(name, operands),
        Ability::CounterStages {
            level_up, stages, ..
        } => {
            level_up.iter().try_for_each(|cost| scope.cost(cost))?;
            for ability in stages.iter().flat_map(|stage| &stage.abilities) {
                validate_ability_in(ability, zones, casting_costs)?;
            }
            return Ok(());
        }
        Ability::Backup { abilities, .. } => {
            for ability in abilities {
                validate_ability_in(ability, zones, casting_costs)?;
            }
            return Ok(());
        }
        Ability::Mystery {
            condition,
            abilities,
        } => {
            scope.condition(condition)?;
            for ability in abilities {
                validate_ability_in(ability, zones, casting_costs)?;
            }
            return Ok(());
        }
        Ability::DraftRule(effects) => {
            return effects.iter().try_for_each(|effect| scope.operand(effect))
        }
        Ability::DamageCantBePrevented { sources, .. } => {
            return sources.iter().try_for_each(|set| scope.objects(set))
        }
        Ability::CantEnter { cards, from } => {
            from.iter().try_for_each(|zone| scope.zone_scope(zone))?;
            return scope.filter(cards);
        }
        Ability::AttackCap { defender, amount } => {
            scope.recipients(defender)?;
            return scope.expr(amount);
        }
        Ability::AsEvent {
            object,
            event,
            actions,
        } => {
            scope.object(object)?;
            if let AsEvent::BecomesAttachedTo(hosts) = event {
                scope.filter(hosts)?;
            }
            return scope.operand(actions);
        }
        Ability::BeforeGame { body, .. } => return scope.operand(body),
        Ability::Modified { ability, modifier } => {
            validate_ability_in(ability, zones, casting_costs)?;
            return scope.operand(modifier);
        }
        Ability::Tiered { modes } => {
            for mode in modes {
                scope.cost(&mode.cost)?;
                validate_ability_in(&mode.ability, zones, casting_costs)?;
            }
            return Ok(());
        }
        Ability::PayToIgnore {
            subjects,
            rules,
            payer,
            cost,
            ..
        } => {
            match subjects {
                EffectSubjects::Objects(objects) => scope.objects(objects)?,
                EffectSubjects::Players(players) => scope.players(players)?,
            }
            payer.iter().try_for_each(|p| scope.player(p))?;
            scope.cost(cost)?;
            return scope.operand(rules);
        }
        Ability::CountsAsNamed { .. } | Ability::Unique(_) => return Ok(()),
        Ability::WhileOnTopOfLibrary(abilities) | Ability::Triggers(abilities) => {
            for ability in abilities {
                validate_ability_in(ability, zones, casting_costs)?;
            }
            return Ok(());
        }
        Ability::KickedHas { cost, ability } => {
            scope.cost(cost)?;
            return validate_ability_in(ability, zones, casting_costs);
        }
        Ability::StartingHandSize(size) => return scope.expr(size),
        Ability::VillainousChoiceTwice(players) => return scope.players(players),
        Ability::ExilesSearchedCards { searching, player } => {
            scope.players(searching)?;
            return scope.player(player);
        }
        Ability::Companion(rule) | Ability::CommanderDeckRule(rule) => return scope.operand(rule),
        Ability::Offering(filter) => return scope.filter(filter),
        Ability::FlashIf(condition) => return scope.condition(condition),
        Ability::Craft {
            materials,
            count,
            cost,
        } => {
            scope.operand(materials)?;
            if let Some((_, value)) = count {
                scope.expr(value)?;
            }
            return scope.operand(cost);
        }
        Ability::ProtectionExcept { from, keeps } => {
            scope.operand(from)?;
            return scope.objects(keeps);
        }
        Ability::DeckRule(rule) => {
            return match rule {
                DeckRule::UpToCopies(count) => scope.expr(count),
                DeckRule::AnyCards(cards) | DeckRule::AnyCardsWithOneAdditionalColor(cards) => {
                    scope.filter(cards)
                }
                _ => Ok(()),
            };
        }
        Ability::Mechanic {
            cost,
            amount,
            filter,
            ..
        } => {
            cost.iter().try_for_each(|cost| scope.cost(cost))?;
            amount.iter().try_for_each(|amount| scope.expr(amount))?;
            return filter.iter().try_for_each(|filter| scope.filter(filter));
        }
        Ability::CostBranch {
            cost, paid, unpaid, ..
        } => {
            scope.cost(cost)?;
            let mut paid_costs = casting_costs.clone();
            paid_costs.insert(BRANCH_COST, cost.clone());
            validate_ability_in(paid, zones, &paid_costs)?;
            return validate_ability_in(unpaid, zones, casting_costs);
        }
        Ability::CostOptions { options, .. } => {
            for (cost, ability) in options {
                scope.cost(cost)?;
                let mut paid_costs = casting_costs.clone();
                paid_costs.insert(BRANCH_COST, cost.clone());
                validate_ability_in(ability, zones, &paid_costs)?;
            }
            return Ok(());
        }
        Ability::Gift {
            gift,
            promised,
            unpromised,
        } => {
            for program in gift {
                scope.clone().program(program)?;
            }
            for outcome in promised.iter().chain(unpromised) {
                validate_ability_in(outcome, zones, casting_costs)?;
            }
            return Ok(());
        }
        Ability::Spree { modes } => {
            if modes.is_empty() {
                return Err("spree has no modes".into());
            }
            for mode in modes {
                scope.cost(&mode.cost)?;
                validate_ability_in(&mode.ability, zones, casting_costs)?;
            }
            return Ok(());
        }
        Ability::Prototype { .. } => return Ok(()),
        Ability::Visit { body, prize } => {
            validate_ability_in(body, zones, casting_costs)?;
            if let Some(prize) = prize {
                validate_ability_in(prize, zones, casting_costs)?;
            }
            return Ok(());
        }
        Ability::DungeonRoom { body, .. } => {
            return validate_ability_in(body, zones, casting_costs)
        }
        Ability::TriggerRule {
            cause, abilities, ..
        } => {
            if let Some(event) = cause {
                scope.event(event)?;
            }
            if let Some(filter) = abilities {
                scope.filter(filter)?;
            }
            return Ok(());
        }
        Ability::Class { levels } => {
            if levels.is_empty() {
                return Err("class has no levels".into());
            }
            for level in levels {
                scope.cost(&level.cost)?;
                for ability in &level.abilities {
                    validate_ability_in(ability, zones, casting_costs)?;
                }
            }
            return Ok(());
        }
        Ability::Saga { chapters } => {
            if chapters.is_empty() {
                return Err("saga has no chapters".into());
            }
            for chapter in chapters {
                if !matches!(
                    chapter,
                    Ability::Triggered {
                        event: EventSpec::SagaChapter(1..),
                        ..
                    }
                ) {
                    return Err("saga chapter must trigger on a positive chapter number".into());
                }
                validate_ability_in(chapter, zones, casting_costs)?;
            }
            return Ok(());
        }
        Ability::Scoped {
            zones: active,
            condition,
            ability,
        } => {
            if active.is_empty()
                || active
                    .iter()
                    .enumerate()
                    .any(|(i, z)| active[..i].contains(z))
            {
                return Err("ability scope has empty or repeated zones".into());
            }
            let effective = active
                .iter()
                .filter(|z| zones.is_none_or(|outer| outer.contains(z)))
                .cloned()
                .collect::<Vec<_>>();
            if effective.is_empty() {
                return Err("nested source scopes have no common zone".into());
            }
            if let Some(condition) = condition {
                scope.condition(condition)?;
            }
            return validate_ability_in(ability, Some(&effective), casting_costs);
        }
        Ability::Replacement {
            active,
            event,
            body,
        } => {
            scope.replacement_event(event)?;
            fn only_source(filter: &ObjectFilter) -> bool {
                match filter {
                    ObjectFilter::SameObject(ObjectRef::Source) => true,
                    ObjectFilter::All(filters) => filters.iter().any(only_source),
                    _ => false,
                }
            }
            if matches!(active, ReplacementActive::SourceEntering)
                && !matches!(event, ReplacementEvent::Enter { objects } if only_source(objects))
            {
                return Err("source-entry replacement must match only its source".into());
            }
            scope.replacement = true;
            scope.event = crate::semantics::replacement_bindings(event);
            return scope.program(body);
        }
        Ability::Enchant { hosts } => {
            return match hosts {
                EnchantHosts::Permanents(filter) => scope.filter(filter),
                EnchantHosts::Players(players) => scope.players(players),
                EnchantHosts::GraveyardCards { filter, owners } => {
                    scope.filter(filter)?;
                    scope.players(owners)
                }
            }
        }
        Ability::Static {
            kind,
            affected,
            changes,
        } => {
            if let StaticKind::CharacteristicDefining { keyword, .. } = kind {
                scope.definition = true;
                if zones.is_some()
                    || !matches!(
                        affected,
                        EffectSubjects::Objects(ObjectSet::One(ObjectRef::Source))
                    )
                {
                    return Err("characteristic definition must intrinsically affect only its source everywhere".into());
                }
                if changes.iter().any(|c| {
                    !matches!(
                        c,
                        Change::SetPower(_)
                            | Change::SetToughness(_)
                            | Change::SetBasePowerToughness { .. }
                            | Change::Colors { .. }
                            | Change::ChosenColor { .. }
                            | Change::ChosenType { .. }
                            | Change::CreatureTypes { .. }
                            | Change::LandTypes { .. }
                            | Change::Primitive { .. }
                    )
                }) {
                    return Err("characteristic definition may define only color, subtypes, power, or toughness".into());
                }
                if let Some(keyword) = keyword {
                    let valid = match keyword {
                        Keyword::Devoid => {
                            matches!(changes.as_slice(),[Change::Colors {mode:ChangeMode::Set,colors}] if colors.is_empty())
                        }
                        Keyword::Changeling => matches!(
                            changes.as_slice(),
                            [Change::CreatureTypes {
                                mode: ChangeMode::Set,
                                types: CreatureTypeSet::All
                            }]
                        ),
                        _ => false,
                    };
                    if !valid {
                        return Err(
                            "defining keyword does not match its characteristic definition".into(),
                        );
                    }
                }
            }
            return scope.subject_changes(affected, changes);
        }
        Ability::Conditional {
            condition,
            then,
            otherwise,
        } => {
            scope.condition(condition)?;
            for ability in then.iter().chain(otherwise) {
                if has_definition(ability) {
                    return Err("characteristic definition cannot be a conditional ability".into());
                }
                validate_ability_in(ability, zones, casting_costs)?;
            }
            return Ok(());
        }
        Ability::Keyword(keyword) => return scope.keyword(keyword),
        Ability::Casting(rule) => return scope.casting_rule(rule),
        Ability::Spell { announcement, body } => {
            scope.casting_costs = casting_costs.clone();
            (announcement, body)
        }
        Ability::OpeningHand { announcement, body } => (announcement, body),
        Ability::Activated {
            origin,
            cost,
            limits,
            announcement,
            body,
        } => {
            if matches!(origin, ActivationOrigin::Crew)
                && !valid_crew_definition(cost, limits, announcement, body)
            {
                return Err("invalid crew payment, participants, or resolution program".into());
            }
            if matches!(origin, ActivationOrigin::Cycling { .. }) {
                if zones != Some(&[EventZone::Hand][..])
                    || announcement.modes.is_some()
                    || !announcement.targets.is_empty()
                    || limits.sorcery
                    || limits.once
                    || limits.once_each_turn
                    || limits.condition.is_some()
                    || !cost.parts.iter().any(|part| {
                        matches!(
                            part,
                            CostPart::Discard {
                                count: Expr::Constant(1),
                                filter: ObjectFilter::SameObject(ObjectRef::Source),
                                random: false
                            }
                        )
                    })
                    || !valid_cycling_body(origin, body)
                {
                    return Err("invalid cycling scope, source discard, or draw program".into());
                }
            }
            scope.cost(cost)?;
            scope.activation_cost = Some(cost.clone());
            if let Some(condition) = &limits.condition {
                let mut proposal = scope.clone();
                proposal.announcing = true;
                proposal.condition(condition)?;
            }
            // A cost change can read what the ability targets.
            let mut announced = scope.clone();
            announced.announcement(announcement)?;
            for modifier in &limits.modifiers {
                match modifier {
                    Change::Primitive { name, operands } => announced.primitive(name, operands)?,
                    Change::ActivationRule(rule) => {
                        let mut proposal = announced.clone();
                        proposal.announcing = true;
                        proposal.activation_rule(rule)?;
                    }
                    _ => return Err("an activation modifier must be a reviewed form".into()),
                }
            }
            if matches!(origin, ActivationOrigin::Equip) {
                let [target] = announcement.targets.as_slice() else {
                    return Err("equip must announce exactly one target slot".into());
                };
                if !limits.sorcery
                    || announcement.modes.is_some()
                    || !matches!(&target.kind, TargetKind::Object(domain) if *domain == ObjectDomain::permanent())
                    || !matches!(target.count, Count::Exactly(Expr::Constant(1)))
                {
                    return Err("invalid equip activation structure".into());
                }
                let [Instruction::Effect {
                    operation:
                        Operation::Attach {
                            attachments: ObjectSet::One(ObjectRef::Source),
                            host: ObjectRef::Target { slot, index: 0 },
                        },
                    ..
                }] = body.instructions.as_slice()
                else {
                    return Err("equip must attach its source to its announced target".into());
                };
                if slot.0 != target.slot.0 {
                    return Err("equip attachment uses a different target slot".into());
                }
            }
            (announcement, body)
        }
        Ability::Triggered {
            event,
            intervening_if,
            limits,
            announcement,
            body,
        } => {
            scope.event(event)?;
            scope.event = crate::semantics::event_bindings(event);
            if matches!(limits.per_turn, Some(0)) || matches!(limits.total, Some(0)) {
                return Err("trigger limit must be positive".into());
            }
            if let Some(condition) = intervening_if {
                scope.condition(condition)?;
            }
            (announcement, body)
        }
    };
    scope.announcement(announcement)?;
    if announcement.modes.is_some() && !body.instructions.is_empty() {
        return Err("modal ability has a separate resolution body".into());
    }
    scope.program(body)
}

/// Accepts `Named` forms. Complete cards go through `card`, which rejects them.
pub fn first_invalid(card: &Card) -> Option<(usize, String)> {
    let casting_costs = card
        .abilities
        .iter()
        .enumerate()
        .filter_map(|(i, ability)| {
            crate::cost_history::casting_cost(ability)
                .cloned()
                .map(|cost| (i as u32, cost))
        })
        .collect();
    card.abilities.iter().enumerate().find_map(|(i, ability)| {
        validate_ability_in(ability, None, &casting_costs)
            .err()
            .map(|e| (i, e))
    })
}

fn face(card: &Card) -> Result {
    if card.schema_version != SCHEMA_VERSION {
        return Err(format!(
            "unsupported AST schema version {}",
            card.schema_version
        ));
    }
    if card.name.is_empty() {
        return Err("card name is empty".into());
    }
    match first_invalid(card) {
        Some((i, e)) => Err(format!("{}: abilities[{i}]: {e}", card.name)),
        None => Ok(()),
    }
}

pub fn card(card: &Card) -> Result {
    face(card)?;
    match crate::unified::named_forms(card).keys().next() {
        Some(form) => Err(format!("{}: unreduced form {form}", card.name)),
        None => Ok(()),
    }
}

/// Validate a record that may hold `Named` forms.
pub fn unified(card: &UnifiedCard) -> Result {
    if card.schema_version != SCHEMA_VERSION {
        return Err(format!(
            "unsupported AST schema version {}",
            card.schema_version
        ));
    }
    if card.faces.is_empty() && card.data.is_empty() {
        return Err(format!("{} record is empty", card.layout));
    }
    card.faces.iter().try_for_each(face)?;
    card.data.iter().try_for_each(|f| operand(&f.value))
}

/// A `Named` operand must stand alone, so it is checked with no surrounding scope.
pub fn operand(value: &Operand) -> Result {
    Scope::default().operand(value)
}
