//! Rules-derived properties of complete typed programs. Keep these separate
//! from Cantrip capabilities and avoid a serialized flag that can become stale.
use crate::ir::*;
use serde::Serialize;

/// Structural equality of typed references, without imposing Eq on the entire
/// recursive query/expression language. This compares data, never source text.
pub(crate) fn same_player(a: &PlayerRef, b: &PlayerRef) -> bool {
    serde_json::to_value(a).expect("typed player") == serde_json::to_value(b).expect("typed player")
}
pub(crate) fn shuffle_closes(library: &PlayerRef, players: &PlayerSet) -> bool {
    match players {
        PlayerSet::One(player) => same_player(library, player),
        PlayerSet::All => true,
        PlayerSet::Union(sets) => sets.iter().any(|s| shuffle_closes(library, s)),
        _ => false,
    }
}

/// References guaranteed by every possible occurrence of an event matcher.
/// Derived from the typed matcher rather than serialized as independent flags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EventBindings {
    pub object: bool,
    pub permanent: bool,
    pub spell: bool,
    pub player: bool,
    pub amount: bool,
    pub moved: bool,
    pub damage: bool,
    pub damage_source: bool,
    pub damage_source_permanent: bool,
    pub damage_recipient: bool,
    pub damage_objects: bool,
    pub damage_players: bool,
    pub damage_subject: Option<DamageSubject>,
    pub discard: bool,
    pub discarded: bool,
    pub activation_x: bool,
    pub block_pair: bool,
    pub group: bool,
    pub stack_object: bool,
    pub other: bool,
    pub other_player: bool,
    pub moved_owner: bool,
}
impl EventBindings {
    /// What every alternative of an either-event still binds.
    fn shared(a: Self, b: Self) -> Self {
        Self {
            object: a.object && b.object,
            permanent: a.permanent && b.permanent,
            spell: a.spell && b.spell,
            player: a.player && b.player,
            amount: a.amount && b.amount,
            moved: a.moved && b.moved,
            block_pair: a.block_pair && b.block_pair,
            group: a.group && b.group,
            stack_object: a.stack_object && b.stack_object,
            other: a.other && b.other,
            other_player: a.other_player && b.other_player,
            moved_owner: a.moved_owner && b.moved_owner,
            damage: a.damage && b.damage,
            damage_source: a.damage_source && b.damage_source,
            damage_source_permanent: a.damage_source_permanent && b.damage_source_permanent,
            damage_recipient: a.damage_recipient && b.damage_recipient,
            damage_objects: a.damage_objects && b.damage_objects,
            damage_players: a.damage_players && b.damage_players,
            damage_subject: if a.damage_subject == b.damage_subject {
                a.damage_subject
            } else {
                None
            },
            discard: a.discard && b.discard,
            discarded: a.discarded && b.discarded,
            activation_x: a.activation_x && b.activation_x,
        }
    }
}
/// References available in the body of a replacement on this event.
pub fn replacement_bindings(event: &ReplacementEvent) -> EventBindings {
    match event {
        ReplacementEvent::ZoneChange { objects, from, to } => {
            event_bindings(&EventSpec::ZoneChange {
                objects: objects.clone(),
                from: from.clone(),
                to: to.clone(),
            })
        }
        ReplacementEvent::Primitive { name, operands } => event_bindings(&EventSpec::Primitive {
            name: name.clone(),
            operands: operands.clone(),
        }),
        ReplacementEvent::Enter { .. } => EventBindings {
            object: true,
            permanent: true,
            ..Default::default()
        },
        ReplacementEvent::Damage(damage) => event_bindings(&EventSpec::Damage(damage.clone())),
        ReplacementEvent::Either(events) => events
            .iter()
            .map(replacement_bindings)
            .reduce(EventBindings::shared)
            .unwrap_or_default(),
    }
}
/// Whether an operation affects at most one object, given which earlier results do.
pub fn singular_operation(operation: &Operation, single: impl Fn(&ResultId) -> bool) -> bool {
    let one = |count: &Count| {
        matches!(
            count,
            Count::Exactly(Expr::Constant(1))
                | Count::AsManyAsCan(Expr::Constant(1))
                | Count::UpTo(Expr::Constant(1))
        )
    };
    let chosen = |objects: &ObjectSet| match objects {
        ObjectSet::One(_) => true,
        ObjectSet::Selected(result) => single(result),
        _ => false,
    };
    match operation {
        Operation::ChooseObjects { count, .. } => one(count),
        Operation::Sacrifice {
            objects, choose, ..
        } => choose.as_ref().map_or_else(|| chosen(objects), one),
        Operation::Tap { objects, .. } => chosen(objects),
        Operation::Discard {
            players: PlayerSet::One(_),
            amount: Expr::Constant(1),
            ..
        } => true,
        Operation::DiscardObjects { objects } => chosen(objects),
        Operation::CreateTokens {
            players: PlayerSet::One(_),
            tokens,
            ..
        } => matches!(
            tokens.as_slice(),
            [TokenSpec {
                amount: Expr::Constant(1),
                ..
            }]
        ),
        _ => yields(operation).is_some(),
    }
}

/// Manifesting puts one card onto the battlefield (701.40a), as a reviewed
/// primitive with no ordinary movement to read the permanent from.
/// What a reviewed action row leaves for a later "this way" to name.
pub fn yields(operation: &Operation) -> Option<&'static str> {
    let Operation::Primitive { name, .. } = operation else {
        return None;
    };
    crate::primitive_spec::lookup(name)?.yields.as_deref()
}

pub fn event_bindings(event: &EventSpec) -> EventBindings {
    match event {
        EventSpec::SagaChapter(_) => EventBindings::default(),
        EventSpec::Primitive { name, .. } => {
            let binds = crate::primitive_spec::lookup(name).map_or(&[][..], |r| &r.binds);
            let has = |field: &str| binds.iter().any(|b| b == field);
            EventBindings {
                object: has("object"),
                permanent: has("permanent"),
                spell: has("spell"),
                player: has("player"),
                amount: has("amount"),
                block_pair: has("block_pair"),
                group: has("group"),
                stack_object: has("stack_object"),
                other: has("other"),
                other_player: has("other_player"),
                moved: has("moved"),
                ..EventBindings::default()
            }
        }
        EventSpec::Discard { grouping, kind, .. } => EventBindings {
            object: matches!(grouping, DiscardGrouping::Card),
            player: true,
            amount: matches!(grouping, DiscardGrouping::Player),
            discard: true,
            discarded: matches!(grouping, DiscardGrouping::Card),
            activation_x: matches!(kind, DiscardKind::Cycling),
            ..Default::default()
        },
        EventSpec::ZoneChange { to, .. } => EventBindings {
            object: true,
            permanent: true,
            moved: true,
            moved_owner: matches!(to, EventZone::Graveyard),
            ..EventBindings::default()
        },
        EventSpec::Object { .. } => EventBindings {
            object: true,
            permanent: true,
            ..EventBindings::default()
        },
        EventSpec::Damage(event) => {
            let source = matches!(
                event.grouping,
                DamageGrouping::Source | DamageGrouping::SourceRecipient
            );
            let recipient = matches!(
                event.grouping,
                DamageGrouping::Recipient | DamageGrouping::SourceRecipient
            );
            let objects = matches!(event.recipients, DamageRecipients::Objects(_));
            let players = matches!(event.recipients, DamageRecipients::Players(_));
            let object = match event.subject {
                DamageSubject::Source => source,
                DamageSubject::Recipient => recipient && objects,
            };
            EventBindings {
                object,
                permanent: object
                    && match event.subject {
                        DamageSubject::Source => matches!(event.source_kind, ObjectKind::Permanent),
                        DamageSubject::Recipient => true,
                    },
                spell: object
                    && matches!(event.subject, DamageSubject::Source)
                    && matches!(event.source_kind, ObjectKind::Spell),
                player: recipient && players,
                amount: true,
                moved: false,
                damage: true,
                damage_source: source,
                damage_source_permanent: source
                    && matches!(event.source_kind, ObjectKind::Permanent),
                damage_recipient: recipient && objects,
                block_pair: false,
                group: false,
                stack_object: false,
                other: false,
                other_player: false,
                moved_owner: false,
                damage_objects: !players,
                damage_players: !objects,
                damage_subject: Some(event.subject),
                discard: false,
                discarded: false,
                activation_x: false,
            }
        }
        EventSpec::Spell { .. } => EventBindings {
            object: true,
            spell: true,
            player: true,
            ..EventBindings::default()
        },
        EventSpec::Turn { .. } => EventBindings {
            player: true,
            ..EventBindings::default()
        },
        EventSpec::State { .. } => EventBindings::default(),
        EventSpec::Filtered { event, .. } => event_bindings(event),
        EventSpec::Either(events) => events
            .iter()
            .map(event_bindings)
            .reduce(EventBindings::shared)
            .unwrap_or_default(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ActivationKind {
    Mana,
    Stack,
}
#[derive(Default)]
struct Effects {
    mana: bool,
    library: bool,
    objects: std::collections::BTreeMap<u32, (bool, bool)>,
    targets: std::collections::BTreeMap<u16, bool>,
}
impl Effects {
    fn program(&mut self, program: &Program) {
        for instruction in &program.instructions {
            match instruction {
                Instruction::Effect { result, operation } => match operation {
                    Operation::AddMana { .. } => self.mana = true,
                    Operation::Draw { .. } | Operation::Mill { .. } => self.library = true,
                    Operation::Move {
                        objects,
                        destination,
                        ..
                    }
                    | Operation::Counter {
                        objects,
                        destination,
                    } => {
                        let before = self.objects_library(objects);
                        let after = Self::destination_library(destination);
                        self.library |= before || after;
                        self.objects.insert(result.0, (before, after));
                    }
                    Operation::ChooseObjects { objects, .. }
                    | Operation::NoteObjects { objects } => {
                        let library = self.objects_library(objects);
                        self.objects.insert(result.0, (library, library));
                    }
                    Operation::Search { .. } => {
                        self.objects.insert(result.0, (true, true));
                    }
                    Operation::LookAt { objects, .. }
                    | Operation::ReorderLibrary { objects, .. } => {
                        let library = self.objects_library(objects);
                        self.objects.insert(result.0, (library, library));
                    }
                    Operation::Surveil { amount, .. } => {
                        self.library |= !matches!(amount, Expr::Constant(0))
                    }
                    Operation::Reveal { .. }
                    | Operation::RevealUntil { .. }
                    | Operation::Shuffle { .. }
                    | Operation::Scry { .. } => {}
                    Operation::PayCost { cost, .. } => {
                        self.cost(cost);
                    }
                    // Its body executes in a later triggered ability, not
                    // during this activation's resolution (CR 605.1a).
                    Operation::CreateDelayedTrigger { .. }
                    | Operation::CreateTrigger { .. }
                    | Operation::BindValue { .. }
                    | Operation::RepeatProcess
                    | Operation::DiscardObjects { .. }
                    | Operation::ReflexiveTrigger { .. }
                    | Operation::ReturnAfter { .. }
                    | Operation::CreateReplacement { .. }
                    | Operation::ModifyEvent { .. }
                    | Operation::CreateRegenerationShields { .. }
                    | Operation::Discard { .. }
                    | Operation::RemoveCounters { .. }
                    | Operation::CreateTokens { .. }
                    | Operation::Attach { .. }
                    | Operation::GainLife { .. }
                    | Operation::SetLifeTotal { .. }
                    | Operation::LoseLife { .. }
                    | Operation::DealDamage { .. }
                    | Operation::DealDamageParts { .. }
                    | Operation::DealDamageFromEach { .. }
                    | Operation::Destroy { .. }
                    | Operation::Tap { .. }
                    | Operation::Sacrifice { .. }
                    | Operation::PutCounters { .. }
                    | Operation::CreateContinuousEffect { .. } => {}
                    // Its effects are unknown, so it must not read as a mana ability.
                    Operation::Named { .. } | Operation::Primitive { .. } => self.library = true,
                },
                Instruction::Group(p) | Instruction::Optional { body: p, .. } => self.program(p),
                Instruction::If {
                    then, otherwise, ..
                } => {
                    self.program(then);
                    self.program(otherwise);
                }
                Instruction::Times { body, .. }
                | Instruction::ForEachObject { body, .. }
                | Instruction::ForEachPlayer { body, .. }
                | Instruction::Repeatable(body) => self.program(body),
                Instruction::ChooseOne { options, .. } => {
                    options.iter().for_each(|option| self.program(option));
                }
            }
        }
    }
    fn destination_library(destination: &Destination) -> bool {
        match destination {
            Destination::OwnersLibraryTop
            | Destination::OwnersLibraryBottom
            | Destination::OwnersLibraryFromTop(_)
            | Destination::OwnersLibraryShuffled => true,
            Destination::OwnersHand
            | Destination::OwnersGraveyard
            | Destination::Exile
            | Destination::Battlefield(_)
            | Destination::OwnersBattlefield => false,
        }
    }
    fn object_library(&self, object: &ObjectRef) -> bool {
        match object {
            ObjectRef::Target { slot, .. } => self.targets.get(&slot.0).copied().unwrap_or(false),
            ObjectRef::Query { objects, .. } => self.objects_library(objects),
            ObjectRef::Result { result, phase, .. } => self.result_library(result, phase),
            ObjectRef::Source
            | ObjectRef::KeywordHost
            | ObjectRef::Discarded
            | ObjectRef::Damage(_)
            | ObjectRef::Candidate
            | ObjectRef::AttachedTo(_)
            | ObjectRef::Affected
            | ObjectRef::EventSubject
            | ObjectRef::EventStackObject
            | ObjectRef::EventOther
            | ObjectRef::RecipientObject { .. }
            | ObjectRef::BlockPair(_)
            | ObjectRef::EventDestination
            | ObjectRef::Chosen
            | ObjectRef::Primitive { .. }
            | ObjectRef::Bound(_) => false,
        }
    }
    fn result_library(&self, result: &ResultId, phase: &ObjectPhase) -> bool {
        self.objects
            .get(&result.0)
            .is_some_and(|(before, after)| match phase {
                ObjectPhase::Before => *before,
                ObjectPhase::After => *after,
            })
    }
    fn objects_library(&self, objects: &ObjectSet) -> bool {
        match objects {
            ObjectSet::LibraryTops(_) => true,
            ObjectSet::Observed(result) => self.result_library(result, &ObjectPhase::Before),
            ObjectSet::Difference { objects, .. } | ObjectSet::Matching { objects, .. } => {
                self.objects_library(objects)
            }
            ObjectSet::Zone { scope, .. } => matches!(
                scope,
                ZoneScope::Players {
                    zone: EventZone::Library,
                    ..
                }
            ),
            ObjectSet::InZone { zone, .. } => matches!(zone, EventZone::Library),
            ObjectSet::One(o) => self.object_library(o),
            ObjectSet::Union(sets) => sets.iter().any(|s| self.objects_library(s)),
            ObjectSet::Take { objects, .. } => self.objects_library(objects),
            ObjectSet::Targets(slot) => self.targets.get(&slot.0).copied().unwrap_or(false),
            ObjectSet::Selected(result) => self.result_library(result, &ObjectPhase::After),
            ObjectSet::Transition { result, phase } => self.result_library(result, phase),
            ObjectSet::Battlefield(_) | ObjectSet::Created(_) | ObjectSet::DamageSources(_) => {
                false
            }
            ObjectSet::EventGroup
            | ObjectSet::Damage(_)
            | ObjectSet::CostObjects { .. }
            | ObjectSet::Affected(_) => false,
        }
    }
    fn cost(&mut self, cost: &Cost) -> bool {
        let mut loyalty = false;
        for part in &cost.parts {
            match part {
                CostPart::Primitive { .. } => {}
                CostPart::Move {
                    selections,
                    destination,
                    ..
                } => {
                    self.library |= Self::destination_library(destination)
                        || selections.iter().any(|s| self.objects_library(&s.objects));
                }
                CostPart::Choice(options) => {
                    for option in options {
                        loyalty |= self.cost(option);
                    }
                }
                CostPart::Constrained { part, .. } => {
                    loyalty |= self.cost(&Cost {
                        parts: vec![part.as_ref().clone()],
                    })
                }
                CostPart::Loyalty(_) => loyalty = true,
                CostPart::Mill { .. } | CostPart::Draw { .. } => self.library = true,
                CostPart::Repeated { part, .. } => {
                    loyalty |= self.cost(&Cost {
                        parts: vec![part.as_ref().clone()],
                    })
                }
                CostPart::Reveal { .. } | CostPart::PlayerAction { .. } => {}
                CostPart::RemoveCounters { .. }
                | CostPart::ManaCostOf(_)
                | CostPart::Mana(_)
                | CostPart::ChangeTapState { .. }
                | CostPart::TapSymbol
                | CostPart::Life(_)
                | CostPart::Sacrifice { .. }
                | CostPart::Discard { .. } => {}
            }
        }
        loyalty
    }
    fn announcement(&mut self, announcement: &Announcement) -> bool {
        for target in &announcement.targets {
            if let TargetKind::Object(domain) = &target.kind {
                self.targets
                    .insert(target.slot.0, domain.zones.contains(&EventZone::Library));
            }
        }
        let mut targets = !announcement.targets.is_empty();
        if let Some(modes) = &announcement.modes {
            for mode in &modes.modes {
                targets |= self.announcement(&mode.announcement);
                self.program(&mode.body);
            }
        }
        targets
    }
}
/// CR 605.1a and 115.6, effective September 25, 2026. Even optional targets
/// require targets. Costs/effects moving cards to or from libraries disqualify
/// an activated ability; granted token abilities do not produce mana for it.
pub fn activation_kind(cost: &Cost, announcement: &Announcement, body: &Program) -> ActivationKind {
    let mut effects = Effects::default();
    let targets = effects.announcement(announcement);
    effects.program(body);
    let loyalty = effects.cost(cost);
    if effects.mana && !targets && !loyalty && !effects.library {
        ActivationKind::Mana
    } else {
        ActivationKind::Stack
    }
}
