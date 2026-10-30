//! Validate semantic data after lowering and after loading a normalized export.
//! A successful deserialize alone does not prove reference or instruction scope.

use crate::ir::*;
use std::collections::{BTreeMap, BTreeSet};

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
    open_searches: Vec<PlayerRef>,
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
        }
        .ok_or("payment history has no declaration in this scope")?;
        crate::cost_history::component(cost, path)
            .ok_or("payment history has an invalid component path".into())
    }
    fn singular_history(&self, objects: &ObjectSet, index: u32) -> bool {
        if index != 0 {
            return false;
        }
        let ObjectSet::CostObjects {
            payment,
            component,
            view: CostObjectView::Inputs,
            phase: ObjectPhase::Before,
        } = objects
        else {
            return false;
        };
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
                zone: EventZone::Graveyard | EventZone::Hand | EventZone::Library,
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
            ObjectSet::Observed(result) => self.ordered_results.contains(&result.0),
            ObjectSet::Difference { objects, .. }
            | ObjectSet::Matching { objects, .. }
            | ObjectSet::InZone { objects, .. } => self.ordered(objects),
            ObjectSet::Zone {
                scope:
                    ZoneScope::Players {
                        zone: EventZone::Library | EventZone::Graveyard,
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
            | ObjectSet::Observed(result)
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
    fn event(&self, event: &EventSpec) -> Result {
        match event {
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
                match &event.recipients {
                    DamageRecipients::Any => Ok(()),
                    DamageRecipients::Objects(f) => self.filter(f),
                    DamageRecipients::Players(p) => self.players(p),
                    DamageRecipients::ObjectsAndPlayers { objects, players } => {
                        self.filter(objects)?;
                        self.players(players)
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
            ObjectRef::KeywordHost if self.keyword_host => Ok(()),
            ObjectRef::KeywordHost => {
                Err("keyword host used outside its keyword definition".into())
            }
            ObjectRef::AttachedTo(attachment) => self.object(attachment),
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
            ObjectRef::Result { result, .. }
                if matches!(
                    self.results.get(&result.0),
                    Some(ResultKind::Selection | ResultKind::Search | ResultKind::ObjectTransition)
                ) =>
            {
                Ok(())
            }
            ObjectRef::Bound(_) => Err("object binding has no declaration".into()),
            ObjectRef::EventSubject => Err("event subject used outside a triggered ability".into()),
            ObjectRef::Result { result, .. } => Err(format!(
                "result {} does not provide objects in this scope",
                result.0
            )),
        }
    }

    fn player(&self, player: &PlayerRef) -> Result {
        match player {
            PlayerRef::WardPayer if self.ward_payer => Ok(()),
            PlayerRef::WardPayer => Err("ward payer used outside a ward cost".into()),
            PlayerRef::Affected if self.affected_player => Ok(()),
            PlayerRef::Affected => {
                Err("affected player used outside a player continuous effect".into())
            }
            PlayerRef::EnchantedBy(object) => self.object(object),
            PlayerRef::DeclaringPlayer if self.declaration => Ok(()),
            PlayerRef::DeclaringPlayer => {
                Err("declaring player used outside a combat declaration cost".into())
            }
            PlayerRef::AbilityController | PlayerRef::Active => Ok(()),
            PlayerRef::CasterOf(object)
            | PlayerRef::ControllerOf(object)
            | PlayerRef::OwnerOf(object) => self.object(object),
            PlayerRef::Target { slot, index } => {
                self.target(slot, Some(*index), TargetKind::Player)
            }
            PlayerRef::EventSubject if self.event.player => Ok(()),
            PlayerRef::EventSubject => Err("this event does not bind a player subject".into()),
            PlayerRef::Bound(binding) if self.players.contains(&binding.0) => Ok(()),
            PlayerRef::Bound(_) => Err("player binding has no declaration".into()),
        }
    }

    fn players(&self, players: &PlayerSet) -> Result {
        match players {
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
            PlayerSet::Targets(slot) => self.target(slot, None, TargetKind::Player),
            PlayerSet::All => Ok(()),
        }
    }

    fn objects(&self, objects: &ObjectSet) -> Result {
        match objects {
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
                            PaymentRef::Activation | PaymentRef::Casting { .. },
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
            ObjectSet::One(object) => self.object(object),
            ObjectSet::Targets(slot) => {
                self.target(slot, None, TargetKind::Object(ObjectDomain::permanent()))
            }
            ObjectSet::Battlefield(filter) => self.filter(filter),
            ObjectSet::InZone { objects, .. } => self.objects(objects),
            ObjectSet::Selected(result) => match self.results.get(&result.0) {
                Some(ResultKind::Selection | ResultKind::Search) => Ok(()),
                _ => Err(format!(
                    "result {} is not an available object selection",
                    result.0
                )),
            },
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
            Recipients::Objects(objects) => self.objects(objects),
            Recipients::Players(players) => self.players(players),
            Recipients::Targets(slot) => self.target(slot, None, TargetKind::DamageRecipient),
            Recipients::Union(items) => {
                for item in items {
                    self.recipients(item)?;
                }
                Ok(())
            }
        }
    }

    fn expr(&self, expression: &Expr) -> Result {
        match expression {
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
            Expr::AnnouncedX if self.definition => {
                Err("characteristic definition cannot use an unrelated announced X".into())
            }
            Expr::Aggregate { objects, .. } | Expr::CountDistinct { objects, .. } => {
                self.objects(objects)
            }
            Expr::EventActivationX if self.event.activation_x => Ok(()),
            Expr::EventActivationX => Err("event activation X used outside a cycling event".into()),
            Expr::Constant(_) | Expr::AnnouncedX => Ok(()),
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
            Expr::Add(a, b) | Expr::Subtract(a, b) | Expr::Multiply(a, b) | Expr::Minimum(a, b) => {
                self.expr(a)?;
                self.expr(b)
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
            ObjectFilter::Condition(condition) => self.condition(condition),
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
            ObjectFilter::HasActivatedAbility(filter) => self.activation_filter(filter),
            ObjectFilter::InZone(scope) => self.zone_scope(scope),
            ObjectFilter::InSet(objects) => self.objects(objects),
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
            ObjectFilter::SameObject(object) => self.object(object),
            ObjectFilter::Property { value, .. } => self.expr(value),
            ObjectFilter::Counters { value, .. } => self.expr(value),
        }
    }

    fn condition(&self, condition: &Condition) -> Result {
        match condition {
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
            ActivationFilter::Any | ActivationFilter::Mana | ActivationFilter::Cycling => Ok(()),
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
            AttackDefenders::Players(p) | AttackDefenders::PlayersAndPlaneswalkers(p) => {
                self.players(p)
            }
        }
    }
    fn object_action(&self, action: &ObjectAction) -> Result {
        match action {
            ObjectAction::Attack(d) => self.defenders(d),
            ObjectAction::Block(f) | ObjectAction::BeBlocked(f) => self.filter(f),
            ObjectAction::Activate(f) => self.activation_filter(f),
            ObjectAction::Untap
            | ObjectAction::Regenerate
            | ObjectAction::Sacrifice
            | ObjectAction::Counter
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
            RuleChange::Prohibit(action) => self.object_action(action),
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
            Change::GrantAbility(ability) => {
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
            | Change::CreatureTypes { .. }
            | Change::AddCardType(_)
            | Change::RemoveAllAbilities => Ok(()),
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
            PlayerAction::Activate(filter) => self.activation_filter(filter),
            _ => Ok(()),
        }
    }
    fn player_rule(&self, rule: &PlayerRule) -> Result {
        match rule {
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
                match modifier {
                    SpellCostModifier::Increase(cost) => self.cost(cost),
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
                                return Err(
                                    "non-spilling colored reduction requires a color".into()
                                );
                            }
                            self.count(&Count::Exactly(r.amount.clone()))?;
                        }
                        Ok(())
                    }
                }
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
            if let CounterAmount::Count(count) = &removal.amount {
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
    fn cost(&self, cost: &Cost) -> Result {
        for part in &cost.parts {
            match part {
                CostPart::Constrained { part, constraints } => {
                    let selectable = match part.as_ref() {
                        CostPart::Sacrifice { .. } | CostPart::Discard { .. } => true,
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
                CostPart::Loyalty(delta) => self.expr(delta)?,
                CostPart::Draw { count } | CostPart::Mill { count } => {
                    self.count(&Count::Exactly(count.clone()))?
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
            CastingRule::AdditionalCost { cost, .. } | CastingRule::AlternativeCost(cost) => {
                self.cost(cost)
            }
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
        if let Destination::Battlefield(player) = destination {
            self.player(player)?;
        }
        Ok(())
    }
    fn named(&self, named: &Named) -> Result {
        named.operands.iter().try_for_each(|o| self.operand(o))
    }
    fn operand(&self, operand: &Operand) -> Result {
        match operand {
            Operand::Absent | Operand::Bool(_) | Operand::Integer(_) | Operand::Text(_) => Ok(()),
            Operand::List(items) => items.iter().try_for_each(|o| self.operand(o)),
            Operand::Record(fields) => fields.iter().try_for_each(|f| self.operand(&f.value)),
            Operand::Form(named) => self.named(named),
            Operand::Object(object) => self.object(object),
            Operand::Objects(objects) => self.objects(objects),
            Operand::Player(player) => self.player(player),
            Operand::Players(players) => self.players(players),
            Operand::Number(expression) => self.expr(expression),
            Operand::Condition(condition) => self.condition(condition),
            Operand::Duration(_) => Ok(()),
            Operand::Cost(cost) => self.cost(cost),
            Operand::Ability(ability) => validate_ability(ability),
        }
    }
    fn operation(&self, operation: &Operation) -> Result {
        match operation {
            Operation::Named { actor, form } => {
                self.players(actor)?;
                self.named(form)
            }
            Operation::CreateDelayedTrigger {
                event,
                start,
                expires,
                intervening_if,
                body,
            } => {
                if !matches!(event, EventSpec::Turn { .. }) {
                    return Err(
                        "delayed event bindings need a separate captured/event scope".into(),
                    );
                }
                self.event(event)?;
                if let DelayedStart::PlayersNextTurn(player) = start {
                    self.player(player)?;
                }
                if let Some(duration) = expires {
                    if let Duration::UntilNextTurn(player) = duration {
                        self.player(player)?;
                    }
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
                if let Some(condition) = intervening_if {
                    child.condition(condition)?;
                }
                child.program(body)
            }
            Operation::LookAt { player, objects } => {
                self.player(player)?;
                self.objects(objects)
            }
            Operation::CreateRegenerationShields { objects } => {
                self.require_zone(objects, EventZone::Battlefield)
            }
            Operation::Search {
                player,
                library,
                top,
                choices,
            } => {
                if self
                    .open_searches
                    .iter()
                    .any(|p| crate::semantics::same_player(p, library))
                {
                    return Err("separated searches need a shared search episode".into());
                }
                self.player(player)?;
                self.player(library)?;
                if let Some(n) = top {
                    self.count(&Count::Exactly(n.clone()))?;
                }
                for choice in choices {
                    self.selection_constraints(&choice.constraints)?;
                    self.filter(&choice.filter)?;
                    self.count(&choice.count)?;
                    if !matches!(
                        choice.count,
                        Count::UpTo(_) | Count::AsManyAsCan(_) | Count::AnyNumber
                    ) {
                        return Err(
                            "search choice requires a fail-to-find or as-many-as-possible policy"
                                .into(),
                        );
                    }
                }
                Ok(())
            }
            Operation::Reveal { objects } => self.objects(objects),
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
                for (_, amount) in &entry.counters {
                    self.count(&Count::Exactly(amount.clone()))?;
                }
                Ok(())
            }
            Operation::ModifyEvent { change } => {
                if !self.replacement {
                    return Err("event change used outside a replacement program".into());
                }
                match change {
                    EventChange::SetTapped(_) => Ok(()),
                    EventChange::AddCounters { amount, .. } => {
                        self.count(&Count::Exactly(amount.clone()))
                    }
                    EventChange::SetController(player) => self.player(player),
                }
            }
            Operation::PayCost { player, cost, .. } => {
                self.player(player)?;
                self.cost(cost)
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
                    let definition = &token.definition;
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
                    if let Some((power, toughness)) = &definition.power_toughness {
                        self.expr(power)?;
                        self.expr(toughness)?;
                    }
                    for (j, ability) in definition.abilities.iter().enumerate() {
                        validate_ability(ability)
                            .map_err(|e| format!("tokens[{i}].definition.abilities[{j}]: {e}"))?;
                    }
                }
                for (_, amount) in &entry.counters {
                    self.count(&Count::Exactly(amount.clone()))?;
                }
                Ok(())
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
                if let Duration::UntilNextTurn(player) = duration {
                    self.player(player)?;
                }
                Ok(())
            }
        }
    }

    fn selection_constraints(&self, constraints: &[SelectionConstraint]) -> Result {
        for constraint in constraints {
            if let SelectionConstraint::TotalProperty { amount, .. } = constraint {
                self.expr(amount)?;
            }
            if let SelectionConstraint::CanMoveTo {
                destination, entry, ..
            } = constraint
            {
                self.destination(destination)?;
                for (_, n) in &entry.counters {
                    self.count(&Count::Exactly(n.clone()))?;
                }
            }
        }
        Ok(())
    }

    fn mana(&self, mana: &ManaProduction) -> Result {
        match mana {
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
        }
    }

    fn program(&mut self, program: &Program) -> Result {
        for (i, instruction) in program.instructions.iter().enumerate() {
            let checked: Result = (|| {
                match instruction {
                    Instruction::Effect { result, operation } => {
                        self.operation(operation)?;
                        let kind = match operation {
                            Operation::LookAt { .. } => ResultKind::Observation,
                            Operation::Search { .. } => ResultKind::Search,
                            Operation::PayCost { .. } => ResultKind::Payment,
                            Operation::CreateTokens { .. } => ResultKind::CreatedObjects,
                            Operation::ChooseObjects { .. } => ResultKind::Selection,
                            Operation::ReorderLibrary { .. }
                            | Operation::Destroy { .. }
                            | Operation::Move { .. }
                            | Operation::Counter { .. }
                            | Operation::Sacrifice { .. } => ResultKind::ObjectTransition,
                            _ => ResultKind::Other,
                        };
                        let zones = match operation {
                            Operation::LookAt { objects, .. } => self.known_zones(objects),
                            Operation::Search { .. } | Operation::ReorderLibrary { .. } => {
                                Some(vec![EventZone::Library])
                            }
                            Operation::ChooseObjects { objects, .. } => self.known_zones(objects),
                            _ => None,
                        };
                        if let Operation::PayCost { cost, .. } = operation {
                            self.payment_costs.insert(result.0, cost.clone());
                        }
                        if self.results.insert(result.0, kind).is_some() {
                            return Err(format!("duplicate result {}", result.0));
                        }
                        if let Operation::LookAt { objects, .. } = operation {
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
                    Instruction::Optional { decider, body } => {
                        self.player(decider)?;
                        let mut yes = self.clone();
                        yes.program(body)?;
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
            let ReplacementEvent::Enter { objects } = event;
            scope.filter(objects)?;
            if matches!(active, ReplacementActive::SourceEntering)
                && !matches!(objects, ObjectFilter::SameObject(ObjectRef::Source))
            {
                return Err("source-entry replacement must match only its source".into());
            }
            scope.replacement = true;
            scope.event = crate::semantics::EventBindings {
                object: true,
                permanent: true,
                ..Default::default()
            };
            return scope.program(body);
        }
        Ability::Enchant { hosts } => {
            return match hosts {
                EnchantHosts::Permanents(filter) => scope.filter(filter),
                EnchantHosts::Players(players) => scope.players(players),
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
                            | Change::CreatureTypes { .. }
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
