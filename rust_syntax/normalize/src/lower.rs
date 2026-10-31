//! Lower legacy Rust syntax into semantic operations. A record is complete only
//! when every part lowers; unsupported forms are reported with an AST path.

use crate::{ir::*, mtg_types as source};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Diagnostic {
    pub path: String,
    pub source_type: String,
    pub form: String,
    pub message: String,
}

impl Diagnostic {
    fn at(mut self, path: impl AsRef<str>) -> Self {
        self.path = if self.path.is_empty() {
            path.as_ref().into()
        } else {
            format!("{}.{}", path.as_ref(), self.path)
        };
        self
    }
}

type Result<T> = std::result::Result<T, Diagnostic>;

fn unsupported(value: &impl Serialize) -> Diagnostic {
    let inspected = crate::inspect::value(value).expect("source AST serialization");
    Diagnostic {
        path: String::new(),
        source_type: inspected["enum"].as_str().unwrap_or("value").into(),
        form: inspected["variant"].as_str().unwrap_or("value").into(),
        message: "semantic lowering has not been implemented for this form".into(),
    }
}

/// A reviewed primitive stands in for a form with no reduction. It never
/// overrides an invariant the reduced path rejected for a semantic reason.
fn rescue<T>(error: Diagnostic, primitive: impl FnOnce() -> Option<T>) -> Result<T> {
    if error.source_type == "invariant" {
        return Err(error);
    }
    primitive().ok_or(error)
}

fn invalid(message: &str) -> Diagnostic {
    Diagnostic {
        path: String::new(),
        source_type: "invariant".into(),
        form: "invalid".into(),
        message: message.into(),
    }
}

#[derive(Default, Clone)]
struct Context {
    prepared: crate::pipeline::PreparedRecipes,
    /// Set only by `unified_face`. A failed action becomes `Operation::Named`.
    lenient: bool,
    /// Why each named form failed. Shared, so restoring a snapshot keeps entries.
    blockers: std::rc::Rc<std::cell::RefCell<Vec<Diagnostic>>>,
    targets: Vec<TargetSpec>,
    modes: Option<ModeSelection>,
    result: u32,
    affected: bool,
    keyword_host: bool,
    candidate: bool,
    event: crate::semantics::EventBindings,
    created_tokens: Option<ResultId>,
    payment: Option<ResultId>,
    paid: std::collections::BTreeMap<crate::number_spec::PaymentAction, Option<PaidBinding>>,
    /// The latest ordinary action of each kind, for "sacrificed this way".
    acted: std::collections::BTreeMap<crate::number_spec::PaymentAction, ResultId>,
    singular: std::collections::BTreeSet<u32>,
    /// In a replacement whose actions happen instead of the event.
    body_replaces: bool,
    /// The player a "can't attack unless" rule is asking about.
    defending: Option<PlayerRef>,
    /// The latest die rolled in this scope.
    rolled: Option<ResultId>,
    /// Zones the next search looks through besides a library, and whether it's "and/or".
    search_also: Vec<EventZone>,
    search_either: bool,
    value_x: Option<ResultId>,
    revealed: Option<ObjectSet>,
    /// The player "they" means in an effect several players carry out.
    iterated: Option<PlayerRef>,
    /// The one player performing the current action.
    acting: Option<PlayerRef>,
    removed_counters: Option<ResultId>,
    announcing: bool,
    you: Option<PlayerRef>,
    payer: Option<PlayerRef>,
    cast_x: Option<ObjectRef>,
    binding_depth: u32,
    state_subject: Option<ObjectRef>,
    exiled: Option<ResultId>,
    /// The latest instruction of each kind a "this way" history can name.
    did: std::collections::BTreeMap<Done, ResultId>,
    exiled_permanents: bool,
    ambiguous_damage_permanent: bool,
    damage_permanent_recipient: bool,
    chosen_damage_source: Option<ResultId>,
    entered: Option<ResultId>,
    open_searches: Vec<PlayerRef>,
    inspection_hand: Option<Vec<ResultId>>,
    /// What stopped the last "exile until": only the final card matches it.
    exile_found: Option<ObjectFilter>,
    /// The recipient a "that player" in a damage amount refers to.
    damage_player: Option<PlayerRef>,
    /// "defending player" in an activation limit is no attacker's.
    activation_limit: bool,
    hand_choice: Option<ResultId>,
    permanent_choice: Option<ResultId>,
    exile_choice: Option<ResultId>,
    grave_choice: Option<ResultId>,
    narrowed_choice: Option<ResultId>,
    extra_turn: Option<ResultId>,
    iterated_object: Option<Binding>,
    /// The card a row just put into a hand: seeked, drafted or conjured.
    to_hand: Option<ResultId>,
    /// The latest snapshot of a set, for "those creatures".
    noted: Option<ResultId>,
}

/// Actions a later "this way" can name in the same resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
enum Done {
    Damage,
    Destroyed,
    Drew,
    Counters,
    Effect,
    LifeLost,
    Milled,
    Returned,
    Exiled,
    Library,
    Graveyard,
    Entered,
    Discarded,
    Sacrificed,
    Prevented,
}
impl Done {
    fn of(operation: &Operation) -> Option<Self> {
        Some(match operation {
            Operation::DealDamage { .. }
            | Operation::DealDamageParts { .. }
            | Operation::DealDamageFromEach { .. }
            | Operation::DealDamageWith { .. } => Self::Damage,
            Operation::ModifyEvent {
                change:
                    EventChange::PreventDamage
                    | EventChange::PreventSomeDamage(_)
                    | EventChange::PreventAllButDamage(_),
            } => Self::Prevented,
            Operation::Destroy { .. } => Self::Destroyed,
            Operation::Draw { .. } => Self::Drew,
            Operation::PutCounters { .. } => Self::Counters,
            Operation::CreateContinuousEffect { .. } => Self::Effect,
            Operation::LoseLife { .. } => Self::LifeLost,
            Operation::Mill { .. } => Self::Milled,
            Operation::Discard { .. } | Operation::DiscardObjects { .. } => Self::Discarded,
            Operation::Sacrifice { .. } => Self::Sacrificed,
            Operation::Move { destination, .. } => match destination {
                Destination::OwnersHand => Self::Returned,
                Destination::Exile => Self::Exiled,
                Destination::OwnersGraveyard => Self::Graveyard,
                Destination::Battlefield(_) | Destination::OwnersBattlefield => Self::Entered,
                Destination::OwnersLibraryTop
                | Destination::OwnersLibraryBottom
                | Destination::OwnersLibraryBottomRandom
                | Destination::OwnersLibraryFromTop(_)
                | Destination::OwnersLibraryShuffled => Self::Library,
            },
            _ => return None,
        })
    }
}

impl Context {
    fn fresh(&self) -> Self {
        Self {
            prepared: self.prepared.clone(),
            lenient: self.lenient,
            blockers: self.blockers.clone(),
            ..Self::default()
        }
    }
    fn target(&self, kind: TargetKind, ordinal: usize, element: u16) -> Result<Slot> {
        let found = self
            .targets
            .iter()
            .filter(|t| t.kind == kind)
            .nth(ordinal)
            .ok_or_else(|| invalid("reference has no target slot in this ability"))?;
        if let Count::Exactly(Expr::Constant(n)) | Count::UpTo(Expr::Constant(n)) = &found.count {
            if i64::from(element) >= *n {
                return Err(invalid("reference exceeds target slot cardinality"));
            }
        }
        Ok(found.slot.clone())
    }

    /// What a move leaves for a later "this way" to name.
    fn note_move(&mut self, operation: &Operation, result: ResultId) {
        let Operation::Move { destination, .. } = operation else {
            return;
        };
        match destination {
            Destination::Exile => {
                self.exiled = Some(result);
                self.exiled_permanents = false;
            }
            Destination::Battlefield(_) | Destination::OwnersBattlefield => {
                self.entered = Some(result)
            }
            _ => {}
        }
    }

    /// A move inside a cost or an optional body: a later reference reads nothing
    /// if the body never ran.
    fn note_moves(&mut self, body: &Program) {
        for instruction in &body.instructions {
            match instruction {
                Instruction::Effect { result, operation } => {
                    if crate::semantics::singular_operation(operation, |r| {
                        self.singular.contains(&r.0)
                    }) {
                        self.singular.insert(result.0);
                    }
                    self.note_move(operation, result.clone());
                    if let Some(done) = Done::of(operation) {
                        self.did.insert(done, result.clone());
                    }
                    if let Operation::ChooseObjects { objects, .. } = operation {
                        self.note_choice(objects, result.clone());
                    }
                }
                Instruction::Group(body) | Instruction::Optional { body, .. } => {
                    self.note_moves(body)
                }
                _ => {}
            }
        }
    }

    fn effect(&mut self, operation: Operation) -> Instruction {
        // A later ordinary action of the same kind shadows the shorthand.
        // Until its own event-history binder exists, fail rather than silently
        // referring back to an unrelated activation/payment component.
        use crate::number_spec::PaymentAction;
        let shadow = match &operation {
            Operation::Sacrifice { .. } => Some(PaymentAction::Sacrifice),
            Operation::Discard { .. } | Operation::DiscardObjects { .. } => {
                Some(PaymentAction::Discard)
            }
            Operation::Tap { tapped: true, .. } => Some(PaymentAction::Tap),
            Operation::Tap { tapped: false, .. } => Some(PaymentAction::Untap),
            _ => None,
        };
        if let Some(action) = shadow {
            self.paid.insert(action, None);
            self.acted.insert(action, ResultId(self.result));
        }
        if crate::semantics::singular_operation(&operation, |r| self.singular.contains(&r.0)) {
            self.singular.insert(self.result);
        }
        if let Operation::Search { library, .. } = &operation {
            self.open_searches.push(library.clone());
        }
        if let Operation::Reveal { objects } = &operation {
            self.revealed = Some(objects.clone());
        }
        if matches!(operation, Operation::RemoveCounters { .. }) {
            self.removed_counters = Some(ResultId(self.result));
        }
        if let Operation::Shuffle { players, .. } = &operation {
            self.open_searches
                .retain(|p| !crate::semantics::shuffle_closes(p, players));
        }
        if matches!(operation, Operation::CreateTokens { .. }) {
            self.created_tokens = Some(ResultId(self.result));
        }
        if let Some(done) = Done::of(&operation) {
            self.did.insert(done, ResultId(self.result));
        }
        self.note_move(&operation, ResultId(self.result));
        if let Operation::PayCost { cost, .. } = &operation {
            self.payment = Some(ResultId(self.result));
            self.install_paid_history(cost, PaymentRef::Resolution(ResultId(self.result)));
            for part in &cost.parts {
                if let CostPart::PlayerAction { body, .. } = part {
                    self.note_moves(body);
                }
            }
        }
        let result = ResultId(self.result);
        self.result += 1;
        Instruction::Effect { result, operation }
    }

    fn invalidate_child_bindings(&mut self, child: &Self) {
        for (action, binding) in &child.paid {
            if serde_json::to_value(self.paid.get(action)).unwrap()
                != serde_json::to_value(Some(binding)).unwrap()
            {
                self.paid.insert(*action, None);
            }
        }
        for (action, result) in &child.acted {
            if self.acted.get(action).map(|r| r.0) != Some(result.0) {
                self.acted.remove(action);
            }
        }
        if self
            .inspection_hand
            .as_ref()
            .map(|v| v.iter().map(|r| r.0).collect::<Vec<_>>())
            != child
                .inspection_hand
                .as_ref()
                .map(|v| v.iter().map(|r| r.0).collect::<Vec<_>>())
        {
            self.inspection_hand = None;
        }
        for library in &child.open_searches {
            if !self
                .open_searches
                .iter()
                .any(|p| crate::semantics::same_player(p, library))
            {
                self.open_searches.push(library.clone());
            }
        }
        if self.created_tokens.as_ref().map(|r| r.0) != child.created_tokens.as_ref().map(|r| r.0) {
            self.created_tokens = None;
        }
        let parent = self.did.clone();
        self.did
            .retain(|kind, result| child.did.get(kind).is_some_and(|r| r.0 == result.0));
        for kind in child.did.keys().filter(|k| !parent.contains_key(k)) {
            self.did.remove(kind);
        }
        if self.payment.as_ref().map(|r| r.0) != child.payment.as_ref().map(|r| r.0) {
            self.payment = None;
        }
        if self.exiled.as_ref().map(|r| r.0) != child.exiled.as_ref().map(|r| r.0) {
            self.exiled = None;
            self.exile_found = None;
            self.exiled_permanents = false;
        }
        if self.hand_choice.as_ref().map(|r| r.0) != child.hand_choice.as_ref().map(|r| r.0) {
            self.hand_choice = None;
        }
        if self.permanent_choice.as_ref().map(|r| r.0)
            != child.permanent_choice.as_ref().map(|r| r.0)
        {
            self.permanent_choice = None;
        }
        if self.exile_choice.as_ref().map(|r| r.0) != child.exile_choice.as_ref().map(|r| r.0) {
            self.exile_choice = None;
        }
        if self.grave_choice.as_ref().map(|r| r.0) != child.grave_choice.as_ref().map(|r| r.0) {
            self.grave_choice = None;
        }
        if self.narrowed_choice.as_ref().map(|r| r.0) != child.narrowed_choice.as_ref().map(|r| r.0)
        {
            self.narrowed_choice = None;
        }
        if self.extra_turn.as_ref().map(|r| r.0) != child.extra_turn.as_ref().map(|r| r.0) {
            self.extra_turn = None;
        }
        if self.entered.as_ref().map(|r| r.0) != child.entered.as_ref().map(|r| r.0) {
            self.entered = None;
        }
        if self.to_hand.as_ref().map(|r| r.0) != child.to_hand.as_ref().map(|r| r.0) {
            self.to_hand = None;
        }
        if self.noted.as_ref().map(|r| r.0) != child.noted.as_ref().map(|r| r.0) {
            self.noted = None;
        }
    }

    fn object(&self, value: &source::Permanent) -> Result<ObjectRef> {
        self.reduced_object(value)
            .or_else(|error| rescue(error, || self.primitive_object(value)))
    }

    fn reduced_object(&self, value: &source::Permanent) -> Result<ObjectRef> {
        use source::Permanent as S;
        if matches!(value, S::Trigger_ThatPermanent) && self.ambiguous_damage_permanent {
            return Err(invalid("damage trigger needs an explicit source/recipient role for this legacy permanent reference"));
        }
        if matches!(value, S::Trigger_ThatPermanent)
            && self.damage_permanent_recipient
            && self.event.damage_recipient
        {
            return Ok(ObjectRef::Damage(DamageSubject::Recipient));
        }
        if matches!(value, S::Trigger_ThatCreature) && self.event.damage_source_permanent {
            return Ok(ObjectRef::Damage(DamageSubject::Source));
        }
        if matches!(value, S::Trigger_ThatPermanent | S::Trigger_ThatCreature) {
            if let Some(subject) = &self.state_subject {
                return Ok(subject.clone());
            }
        }
        let (ordinal, index) = match value {
            S::ThePermanentSacrificedThisWay
            | S::TheSacrificedPermanent
            | S::ThisSacrificedPermanent => {
                return self.paid_object(crate::number_spec::PaymentAction::Sacrifice)
            }
            S::ThePermanentTappedThisWay => {
                return self.paid_object(crate::number_spec::PaymentAction::Tap)
            }

            S::EachablePermanent if self.iterated_object.is_some() => {
                return Ok(ObjectRef::Bound(self.iterated_object.clone().unwrap()));
            }
            S::EachablePermanent if self.candidate => return Ok(ObjectRef::Candidate),
            S::EachablePermanent => return Ok(ObjectRef::Affected),
            S::ThePermanentChosenThisWay | S::TheChosenPermanent
                if self.narrowed_choice.is_some() || self.permanent_choice.is_some() =>
            {
                return Ok(ObjectRef::Result {
                    result: self
                        .narrowed_choice
                        .clone()
                        .or_else(|| self.permanent_choice.clone())
                        .unwrap(),
                    phase: ObjectPhase::After,
                    index: 0,
                });
            }
            S::TheChosenPermanent => return Ok(ObjectRef::Chosen),
            S::ThePermanentPutOnTheBattlefieldThisWay => {
                let token = self
                    .created_tokens
                    .as_ref()
                    .filter(|r| self.singular.contains(&r.0));
                if let (None, None, Some((objects, true))) =
                    (&self.entered, token, self.paid_entered())
                {
                    return Ok(ObjectRef::Query {
                        objects: Box::new(objects),
                        index: 0,
                    });
                }
                return self.result_object(self.entered.as_ref().or(token), 0);
            }
            S::ThisPermanent | S::ThisPermanentOrThisCommandCard => {
                return Ok(if self.keyword_host {
                    ObjectRef::KeywordHost
                } else {
                    ObjectRef::Source
                })
            }
            S::HostPermanent => return Ok(ObjectRef::AttachedTo(Box::new(ObjectRef::Source))),
            S::HostPermanentOf(p) => return Ok(ObjectRef::AttachedTo(Box::new(self.object(p)?))),
            S::ActionPermanent if self.affected => return Ok(ObjectRef::Affected),
            S::Self_It if self.candidate => return Ok(ObjectRef::Candidate),
            S::Ref_TargetPermanent | S::Ref_TargetPermanent1 | S::Ref_TargetPermanents_0 => (0, 0),
            S::Ref_TargetPermanent2 => (1, 0),
            S::Ref_TargetPermanent3 => (2, 0),
            S::Ref_TargetPermanent4 => (3, 0),
            S::Ref_TargetPermanent5 => (4, 0),
            S::Ref_TargetPermanents_1 => (0, 1),
            S::AnyTargetAsAPermanent => {
                let target = self.target(TargetKind::DamageRecipient, 0, 0)?;
                return Ok(ObjectRef::RecipientObject {
                    slot: target,
                    index: 0,
                });
            }
            S::Trigger_ThatVehicle if self.event.other => return Ok(ObjectRef::EventOther),
            S::Trigger_ThatPermanent
            | S::Trigger_ThatCreature
            | S::Trigger_ThatLand
            | S::Trigger_ThatArtifact
            | S::Trigger_ThatVehicle
            | S::Trigger_ThatDeadPermanent
            | S::ThatEnteringPermanent
            | S::WouldDealDamage_ThatPermanent
            | S::WouldDie_ThatPermanent
            | S::WouldBeDestroyed_ThatPermanent
                if self.event.permanent =>
            {
                return Ok(ObjectRef::EventSubject)
            }
            S::Trigger_ThatOtherPermanent
            | S::Trigger_ThatOtherCreature
            | S::Trigger_ThatCreatureOrPlaneswalker
                if self.event.damage_recipient =>
            {
                return Ok(ObjectRef::Damage(DamageSubject::Recipient))
            }
            S::Trigger_ThatCreatureOrPlaneswalker | S::Trigger_ThatSacrificedPermanent
                if self.event.permanent =>
            {
                return Ok(ObjectRef::EventSubject)
            }
            S::Trigger_TheAttackingCreature if self.event.block_pair => {
                return Ok(ObjectRef::BlockPair(BlockRole::Attacker))
            }
            S::Trigger_TheBlockingCreature if self.event.block_pair => {
                return Ok(ObjectRef::BlockPair(BlockRole::Blocker))
            }
            S::Trigger_ThatOtherPermanent
            | S::Trigger_ThatOtherCreature
            | S::Trigger_ThatPermanent
                if self.event.other =>
            {
                return Ok(ObjectRef::EventOther)
            }
            S::Trigger_ThatOtherPermanent | S::Trigger_ThatOtherCreature
                if self.event.block_pair =>
            {
                return Ok(ObjectRef::BlockPair(BlockRole::Other))
            }
            _ => return Err(unsupported(value)),
        };
        Ok(ObjectRef::Target {
            slot: self.target(
                TargetKind::Object(ObjectDomain::permanent()),
                ordinal,
                index,
            )?,
            index,
        })
    }

    fn player(&self, value: &source::Player) -> Result<PlayerRef> {
        self.reduced_player(value)
            .or_else(|error| rescue(error, || self.primitive_player(value)))
    }

    fn reduced_player(&self, value: &source::Player) -> Result<PlayerRef> {
        use source::Player as S;
        Ok(match value {
            S::You => self.you.clone().unwrap_or(PlayerRef::AbilityController),
            S::ActionPlayer | S::EachablePlayer | S::PlayerAction_ThatPlayer
                if self.iterated.is_some() =>
            {
                self.iterated.clone().unwrap()
            }
            S::ActionPlayer if self.payer.is_some() => self.payer.clone().unwrap(),
            S::ActionPlayer if self.acting.is_some() => self.acting.clone().unwrap(),
            S::HostController => PlayerRef::ControllerOf(Box::new(ObjectRef::AttachedTo(
                Box::new(ObjectRef::Source),
            ))),
            S::TheActivePlayer => PlayerRef::Active,
            S::HostPlayer => PlayerRef::EnchantedBy(Box::new(ObjectRef::Source)),
            S::ControllerOfPermanent(p) => PlayerRef::ControllerOf(Box::new(self.object(p)?)),
            S::OwnerOfPermanent(p) => PlayerRef::OwnerOf(Box::new(self.object(p)?)),
            S::ControllerOfSpell(s) => PlayerRef::ControllerOf(Box::new(self.spell(s)?)),
            S::OwnerOfSpell(s) => PlayerRef::OwnerOf(Box::new(self.spell(s)?)),
            S::ControllerOfTargetSpell => {
                PlayerRef::ControllerOf(Box::new(self.spell(&source::Spell::Ref_TargetSpell)?))
            }
            S::ControllerOfTargetPermanent => PlayerRef::ControllerOf(Box::new(
                self.object(&source::Permanent::Ref_TargetPermanent)?,
            )),
            S::Ref_TargetPlayer | S::Ref_TargetPlayer1 => PlayerRef::Target {
                slot: self.target(TargetKind::Player, 0, 0)?,
                index: 0,
            },
            S::Ref_TargetPlayer2 => PlayerRef::Target {
                slot: self.target(TargetKind::Player, 1, 0)?,
                index: 0,
            },
            S::Ref_TargetPlayer3 => PlayerRef::Target {
                slot: self.target(TargetKind::Player, 2, 0)?,
                index: 0,
            },
            S::Trigger_ThatPlayer
            | S::WouldDrawACard_ThatPlayer
            | S::WouldGainLife_ThatPlayer
            | S::WouldDealDamage_DamageRecipientPlayer
                if self.event.player =>
            {
                PlayerRef::EventSubject
            }
            S::Trigger_ThatOtherPlayer if self.event.other_player => PlayerRef::EventOtherPlayer,
            S::Trigger_ThatPlayer if self.event.moved_owner => {
                PlayerRef::OwnerOf(Box::new(ObjectRef::EventSubject))
            }
            S::Trigger_ControllerOfThatSpellOrAbility if self.event.stack_object => {
                PlayerRef::ControllerOf(Box::new(ObjectRef::EventStackObject))
            }
            S::ControllerOfDestroyedPermanent => {
                PlayerRef::ControllerOf(Box::new(ObjectRef::Result {
                    result: self.done(Done::Destroyed)?,
                    phase: ObjectPhase::Before,
                    index: 0,
                }))
            }
            S::ControllerOfDeadPermanent => PlayerRef::ControllerOf(Box::new(self.dead_object()?)),
            S::OwnerOfDeadPermanent => PlayerRef::OwnerOf(Box::new(self.dead_object()?)),
            S::TheChosenPlayer => PlayerRef::Chosen,
            S::DefendingPlayer if self.defending.is_some() => self.defending.clone().unwrap(),
            S::DefendingPlayer if !self.activation_limit => {
                PlayerRef::DefendingPlayerOf(Box::new(ObjectRef::Source))
            }
            S::DealsDamage_ThatPlayer if self.damage_player.is_some() => {
                self.damage_player.clone().unwrap()
            }
            S::OwnerOfExiledCard(card) => PlayerRef::OwnerOf(Box::new(self.exiled_card(card)?)),
            S::OwnerOfGraveyrdCard(card) => PlayerRef::OwnerOf(Box::new(self.grave_card(card)?)),
            S::ControllerOfSpellOrAbility(object) if self.stack_object(object).is_ok() => {
                PlayerRef::ControllerOf(Box::new(self.stack_object(object)?))
            }
            S::Trigger_DefendingPlayer if self.event.permanent => {
                PlayerRef::DefendingPlayerOf(Box::new(ObjectRef::EventSubject))
            }
            S::PlayerCreatureIsAttacking(p)
            | S::PlayerOrControllerOfPlaneswalkerCreatureIsAttacking(p) => {
                PlayerRef::DefendingPlayerOf(Box::new(self.object(p)?))
            }
            S::Trigger_ControllerOfThatSource | S::WouldDealDamage_ControllerOfDamageSource
                if self.event.damage_source =>
            {
                PlayerRef::ControllerOf(Box::new(ObjectRef::Damage(DamageSubject::Source)))
            }
            S::Trigger_ControllerOfThatPermanent if self.event.permanent => {
                PlayerRef::ControllerOf(Box::new(
                    self.object(&source::Permanent::Trigger_ThatPermanent)?,
                ))
            }
            S::Trigger_ControllerOfThatSpell if self.event.spell => {
                PlayerRef::ControllerOf(Box::new(ObjectRef::EventSubject))
            }
            _ => return Err(unsupported(value)),
        })
    }

    fn players(&self, value: &source::Players) -> Result<PlayerSet> {
        use source::Players as S;
        Ok(match value {
            S::WasDealtDamageThisWay | S::PlayerDealtDamageThisWay => {
                PlayerSet::DamagedBy(self.done(Done::Damage)?)
            }
            S::Trigger_ThosePlayers if self.event.damage_players => PlayerSet::DamageRecipients,
            S::Or(values) => PlayerSet::Union(
                values
                    .iter()
                    .map(|p| self.players(p))
                    .collect::<Result<_>>()?,
            ),
            S::AnyPlayer => PlayerSet::All,
            S::SinglePlayer(player) => PlayerSet::One(self.player(player)?),
            S::Opponent => PlayerSet::OpponentsOf(self.player(&source::Player::You)?),
            S::OpponentOf(player) => PlayerSet::OpponentsOf(self.player(player)?),
            S::Other(player) => PlayerSet::OthersThan(self.player(player)?),
            S::IsTheirTurn => PlayerSet::One(PlayerRef::Active),
            S::Ref_TargetPlayers => PlayerSet::Targets(self.target(TargetKind::Player, 0, 0)?),
            S::And(values) => PlayerSet::Intersection(
                values
                    .iter()
                    .map(|p| self.players(p))
                    .collect::<Result<_>>()?,
            ),
            _ => {
                let binding = Binding(self.binding_depth);
                let mut child = self.clone();
                child.binding_depth = self
                    .binding_depth
                    .checked_add(1)
                    .ok_or_else(|| invalid("player predicate nesting is too deep"))?;
                PlayerSet::Matching {
                    binding: binding.clone(),
                    condition: Box::new(
                        child
                            .player_matches(PlayerRef::Bound(binding), value)
                            .map_err(|_| unsupported(value))?,
                    ),
                }
            }
        })
    }

    fn objects(&self, value: &source::Permanents) -> Result<ObjectSet> {
        if matches!(
            value,
            source::Permanents::ThePermanentsSacrificedThisWay
                | source::Permanents::TheSacrificedPermanents
        ) {
            return self.paid_objects(
                crate::number_spec::PaymentAction::Sacrifice,
                CostObjectView::Performed,
            );
        }
        if matches!(value, source::Permanents::ThePermanentsTappedThisWay) {
            return self.paid_objects(
                crate::number_spec::PaymentAction::Tap,
                CostObjectView::Performed,
            );
        }
        if matches!(
            value,
            source::Permanents::Trigger_ThosePermanents
                | source::Permanents::Trigger_ThoseCreatures
        ) {
            if let Some(role) = self.event.damage_subject {
                if role == DamageSubject::Source || self.event.damage_objects {
                    return Ok(ObjectSet::Damage(role));
                }
            }
            if self.event.group {
                return Ok(ObjectSet::EventGroup);
            }
        }
        if matches!(value, source::Permanents::HadCountersPutOnItThisWay) {
            return Ok(ObjectSet::Affected(self.done(Done::Counters)?));
        }
        if matches!(
            value,
            source::Permanents::ThePermanentsPutOnTheBattlefieldThisWay
        ) {
            if let (None, Some((objects, _))) = (&self.entered, self.paid_entered()) {
                return Ok(objects);
            }
            return self
                .entered
                .clone()
                .map(|result| ObjectSet::Transition {
                    result,
                    phase: ObjectPhase::After,
                })
                .ok_or_else(|| invalid("movement reference has no transition in this scope"));
        }
        if let (
            source::Permanents::ThePermanentsChosenThisWay
            | source::Permanents::TheChosenPermanents,
            Some(result),
        ) = (value, &self.permanent_choice)
        {
            return Ok(ObjectSet::Selected(result.clone()));
        }
        if let (
            source::Permanents::ThePermanentsNotChosenThisWay
            | source::Permanents::NotChosenThisWay,
            Some(result),
        ) = (value, &self.permanent_choice)
        {
            return Ok(ObjectSet::Unselected(result.clone()));
        }
        if let (source::Permanents::ThePermanentsList, Some(result)) = (value, &self.noted) {
            return Ok(ObjectSet::Selected(result.clone()));
        }
        if matches!(
            value,
            source::Permanents::ThePermanentsAffectedThisWay
                | source::Permanents::ThePermanentsGainedControlOfThisWay
        ) {
            return Ok(ObjectSet::Affected(self.done(Done::Effect)?));
        }
        if matches!(value, source::Permanents::TheTokensCreatedThisWay) {
            return self
                .created_tokens
                .clone()
                .map(ObjectSet::Created)
                .ok_or_else(|| invalid("tokens-created reference has no creation in this scope"));
        }
        if let source::Permanents::SinglePermanent(object) = value {
            return Ok(ObjectSet::One(self.object(object)?));
        }
        if matches!(value, source::Permanents::Ref_TargetPermanents) {
            return Ok(ObjectSet::Targets(self.target(
                TargetKind::Object(ObjectDomain::permanent()),
                0,
                0,
            )?));
        }
        Ok(ObjectSet::Battlefield(self.filter(value)?))
    }

    fn battlefield_objects(&self, value: &source::Permanents) -> Result<ObjectSet> {
        let objects = self.objects(value)?;
        Ok(if matches!(objects, ObjectSet::Battlefield(_)) {
            objects
        } else {
            ObjectSet::InZone {
                objects: Box::new(objects),
                zone: EventZone::Battlefield,
            }
        })
    }

    fn die_result(&self) -> Result<Expr> {
        self.rolled
            .clone()
            .map(Expr::DieResult)
            .ok_or_else(|| invalid("no die was rolled in this scope"))
    }

    fn comparison(&self, value: &source::Comparison) -> Result<(Comparison, Expr)> {
        use source::Comparison as S;
        let (comparison, number) = match value {
            S::EqualTo(n) => (Comparison::Equal, n),
            S::NotEqualTo(n) => (Comparison::NotEqual, n),
            S::Even => return Ok((Comparison::SameParity, Expr::Constant(0))),
            S::Odd => return Ok((Comparison::SameParity, Expr::Constant(1))),
            S::TheChosenQuality => return Ok((Comparison::SameParity, Expr::ChosenParity)),
            S::NotTheChosenQuality => return Ok((Comparison::DifferentParity, Expr::ChosenParity)),
            S::LessThan(n) => (Comparison::Less, n),
            S::LessThanOrEqualTo(n) => (Comparison::LessOrEqual, n),
            S::GreaterThan(n) => (Comparison::Greater, n),
            S::GreaterThanOrEqualTo(n) => (Comparison::GreaterOrEqual, n),
            _ => return Err(unsupported(value)),
        };
        Ok((comparison, self.number(number)?))
    }

    fn filter(&self, value: &source::Permanents) -> Result<ObjectFilter> {
        let mut context = self.clone();
        context.candidate = true;
        context.filter_inner(value)
    }

    fn filter_inner(&self, value: &source::Permanents) -> Result<ObjectFilter> {
        match value {
            source::Permanents::SharesANameWithAPermanent(objects) => {
                return Ok(ObjectFilter::SharesNameWithAny(Box::new(
                    self.objects(objects)?,
                )));
            }
            source::Permanents::IsNonCreatureTypeVariable(variable) => {
                let matching = self.filter_inner(&source::Permanents::IsCreatureTypeVariable(
                    variable.clone(),
                ))?;
                return Ok(ObjectFilter::Not(Box::new(matching)));
            }
            source::Permanents::IsntAttacking => {
                let attacking = self.filter_inner(&source::Permanents::IsAttacking)?;
                return Ok(ObjectFilter::Not(Box::new(attacking)));
            }
            source::Permanents::PowerAndToughnessIsEqualTo(source::PT::PT(p, t)) => {
                let is = |property, amount: &i32| ObjectFilter::Property {
                    property,
                    comparison: Comparison::Equal,
                    value: Expr::Constant(i64::from(*amount)),
                };
                return Ok(ObjectFilter::All(vec![
                    is(Property::Power, p),
                    is(Property::Toughness, t),
                ]));
            }
            _ => {}
        }
        if let source::Permanents::ExceptFor(excluded) = value {
            return Ok(ObjectFilter::Not(Box::new(self.filter_inner(excluded)?)));
        }
        if matches!(value, source::Permanents::WasDealtDamageThisWay) {
            return Ok(ObjectFilter::DamagedBy(self.done(Done::Damage)?));
        }
        if matches!(value, source::Permanents::WasDealtExcessDamageThisWay) {
            return Ok(ObjectFilter::DealtExcessBy(self.done(Done::Damage)?));
        }
        if matches!(
            value,
            source::Permanents::Ref_TargetPermanents
                | source::Permanents::ThePermanentsChosenThisWay
                | source::Permanents::TheChosenPermanents
                | source::Permanents::ThePermanentsNotChosenThisWay
                | source::Permanents::NotChosenThisWay
                | source::Permanents::TheTokensCreatedThisWay
        ) && match value {
            source::Permanents::Ref_TargetPermanents => true,
            source::Permanents::TheTokensCreatedThisWay => self.created_tokens.is_some(),
            _ => self.permanent_choice.is_some(),
        } {
            return Ok(ObjectFilter::InSet(Box::new(self.objects(value)?)));
        }
        let reduced = match self
            .pipeline_selector(crate::pipeline::selectors::SelectorRef::Permanents(value))
        {
            Some(result) => result,
            None => self.cast_history_filter(value),
        };
        reduced.or_else(|error| match self.history_filter(value) {
            Some(filter) => Ok(filter),
            None => rescue(error, || self.primitive_filter(value)),
        })
    }

    fn number(&self, value: &source::GameNumber) -> Result<Expr> {
        self.reduced_number(value).or_else(|error| {
            if let Some(number) = self.this_way_number(value) {
                return number;
            }
            match self.history_number(value) {
                Some(number) => Ok(number),
                None => rescue(error, || self.primitive_number(value)),
            }
        })
    }

    fn reduced_number(&self, value: &source::GameNumber) -> Result<Expr> {
        match value {
            source::GameNumber::PowerOfDeadPermanent => return self.dead_property(Property::Power),
            source::GameNumber::ToughnessOfDeadPermanent => {
                return self.dead_property(Property::Toughness)
            }
            source::GameNumber::ManaValueOfDeadPermanent => {
                return self.dead_property(Property::ManaValue)
            }
            _ => {}
        }
        if !self.prepared.numbers.is_empty() {
            if let Some(quantity) = self
                .prepared
                .numbers
                .get(&crate::pipeline::numbers::number_key(value))
            {
                return self.pipeline_quantity(quantity);
            }
        }
        if let Some(quantity) = crate::pipeline::numbers::rewrite_number(value) {
            return self.pipeline_quantity(&quantity);
        }
        use source::GameNumber as S;
        Ok(match value {
            S::AmountOfManaSpentToCastSpell(spell)
            | S::TheNumberOfColorsOfManaSpentToCastSpell(spell) => Expr::ManaSpent {
                object: self.spell(spell)?,
                filter: SpentManaFilter {
                    colors: None,
                    snow: false,
                },
                aggregation: if matches!(value, S::AmountOfManaSpentToCastSpell(_)) {
                    SpentManaAggregation::Count
                } else {
                    SpentManaAggregation::DistinctColors
                },
            },
            S::TheNumberOfCountersRemovedThisWay if self.removed_counters.is_some() => {
                Expr::CountersRemoved(self.removed_counters.clone().unwrap())
            }
            S::LifeLostThisWay => Expr::Performed(self.done(Done::LifeLost)?),
            S::NumberOfCardsDrawnThisWay => Expr::Performed(self.done(Done::Drew)?),
            S::TheAmountOfDamageDealtThisWay => Expr::Performed(self.done(Done::Damage)?),
            S::NumPermanentsDestroyedThisWay(filter) => {
                Expr::CountObjects(Box::new(self.done_matching(Done::Destroyed, filter)?))
            }
            S::PowerOfTheExiledCreature => Expr::ObjectProperty {
                object: match &self.exiled {
                    Some(result) => ObjectRef::Result {
                        result: result.clone(),
                        phase: ObjectPhase::Before,
                        index: 0,
                    },
                    None => self.paid_object(crate::number_spec::PaymentAction::Exile)?,
                },
                property: Property::Power,
                information: Information::CurrentOrLastKnown,
            },
            S::TheNumberOfCardsOfTypeRevealedFromHandThisWay(cards) => {
                Expr::CountObjects(Box::new(ObjectSet::Matching {
                    objects: Box::new(self.revealed_objects()?),
                    filter: self.card_filter(cards)?,
                }))
            }
            S::NumCardsMilledThisWay(cards) => Expr::CountObjects(Box::new(ObjectSet::Matching {
                objects: Box::new(ObjectSet::Transition {
                    result: self.done(Done::Milled)?,
                    phase: ObjectPhase::After,
                }),
                filter: self.card_filter(cards)?,
            })),
            S::NumGraveyardCardsExiledThisWay => match &self.exiled {
                Some(result) => Expr::CountObjects(Box::new(ObjectSet::Transition {
                    result: result.clone(),
                    phase: ObjectPhase::Before,
                })),
                None => Expr::CountObjects(Box::new(self.paid_objects(
                    crate::number_spec::PaymentAction::Exile,
                    CostObjectView::Inputs,
                )?)),
            },
            S::NumHandCardsExiledThisWay => Expr::CountObjects(Box::new(ObjectSet::InZone {
                objects: Box::new(ObjectSet::Transition {
                    result: self
                        .exiled
                        .clone()
                        .ok_or_else(|| invalid("exile count has no movement in this scope"))?,
                    phase: ObjectPhase::Before,
                }),
                zone: EventZone::Hand,
            })),
            S::TheNumberOfCountersRemovedThisWay => {
                let paid = self.paid_binding(crate::number_spec::PaymentAction::RemoveCounters)?;
                Expr::PaidCountersRemoved {
                    payment: paid.payment.clone(),
                    component: paid.component.clone(),
                }
            }
            S::NumPlayers(players) => Expr::CountPlayers(self.players(players)?),
            S::NumCardsDiscardedThisWay => Expr::CountObjects(Box::new(self.paid_objects(
                crate::number_spec::PaymentAction::Discard,
                CostObjectView::Performed,
            )?)),
            S::ValueX if self.value_x.is_some() => Expr::BoundValue(self.value_x.clone().unwrap()),
            S::ValueX if self.event.activation_x => Expr::EventActivationX,
            S::ValueX => self
                .cast_x
                .clone()
                .map(Expr::CastX)
                .unwrap_or(Expr::AnnouncedX),
            S::Trigger_ThatMuch
            | S::Trigger_AmountOfDamageDealt
            | S::Trigger_AmountOfExcessDamage
            | S::Trigger_AmountOfDamagePrevented
            | S::Trigger_AmountOfCards
            | S::Trigger_AmountOfLifeGained
            | S::Trigger_AmountOfLifeLost
            | S::WouldDealDamage_ThatMuchDamage
            | S::WouldGainLife_LifeAmount
            | S::WouldLoseLife_ThatMuch
            | S::WouldPayLife_ThatMuch
            | S::WouldScry_ThatMuch
            | S::WouldDrawACard_ThatMany
            | S::ThatManyCounters
                if self.event.amount =>
            {
                Expr::EventAmount
            }
            S::TheDiceResult => self.die_result()?,
            S::TheAmountOfDamagePreventedThisWay if self.did.contains_key(&Done::Prevented) => {
                Expr::Performed(self.done(Done::Prevented)?)
            }
            S::TheAmountOfDamagePreventedThisWay if self.event.amount => Expr::EventAmount,
            S::TheNumberOfCardsInHandRevealedThisWay => {
                Expr::CountObjects(Box::new(self.revealed_objects()?))
            }
            S::NumLibraryCardsRevealedThisWay(cards) => {
                Expr::CountObjects(Box::new(ObjectSet::Matching {
                    objects: Box::new(self.revealed_objects()?),
                    filter: self.card_filter(cards)?,
                }))
            }
            S::ManaValueOfTheCardRevealedThisWay => Expr::ObjectProperty {
                object: ObjectRef::Query {
                    objects: Box::new(self.revealed_objects()?),
                    index: 0,
                },
                property: Property::ManaValue,
                information: Information::CurrentOrLastKnown,
            },
            S::Trigger_NumberOfCreatures if self.event.group => {
                Expr::CountObjects(Box::new(ObjectSet::EventGroup))
            }
            S::NumGroupPermanents(objects, source::GroupFilter::DifferentNames) => {
                Expr::CountDistinct {
                    objects: Box::new(self.objects(objects)?),
                    property: DistinctProperty::Names,
                }
            }
            S::WhenAPermanentEntersTheBattlefield_AmountOfManaSpentToCast
                if self.event.permanent =>
            {
                Expr::ManaSpent {
                    object: ObjectRef::EventSubject,
                    filter: SpentManaFilter {
                        colors: None,
                        snow: false,
                    },
                    aggregation: SpentManaAggregation::Count,
                }
            }
            S::NumColorsManaSpentToCastEnteringPermanent if self.event.permanent => {
                Expr::ManaSpent {
                    object: ObjectRef::EventSubject,
                    filter: SpentManaFilter {
                        colors: None,
                        snow: false,
                    },
                    aggregation: SpentManaAggregation::DistinctColors,
                }
            }
            S::Trigger_ValueXOfThatSpell if self.event.spell => Expr::ObjectProperty {
                object: ObjectRef::EventSubject,
                property: Property::SpellX,
                information: Information::CurrentOrLastKnown,
            },
            S::Trigger_AmountOfCreatures if self.event.damage => {
                Expr::CountObjects(Box::new(ObjectSet::Damage(DamageSubject::Source)))
            }
            S::NumPermanentsExiledThisWay => {
                if !self.exiled_permanents {
                    return Err(invalid(
                        "permanent exile count requires an exclusively permanent batch",
                    ));
                }
                Expr::CountObjects(Box::new(ObjectSet::Transition {
                    result: self
                        .exiled
                        .clone()
                        .ok_or_else(|| invalid("exile count has no movement in this scope"))?,
                    phase: ObjectPhase::Before,
                }))
            }
            _ => return self.characteristic_number(value),
        })
    }

    fn done(&self, kind: Done) -> Result<ResultId> {
        self.did
            .get(&kind)
            .cloned()
            .ok_or_else(|| invalid("this-way history has no action in this scope"))
    }

    fn done_matching(&self, kind: Done, filter: &source::Permanents) -> Result<ObjectSet> {
        Ok(ObjectSet::Matching {
            objects: Box::new(ObjectSet::Transition {
                result: self.done(kind)?,
                phase: ObjectPhase::Before,
            }),
            filter: self.filter(filter)?,
        })
    }

    fn count_matching(
        &self,
        objects: &source::Permanents,
        filter: &source::Permanents,
    ) -> Result<Expr> {
        Ok(Expr::CountObjects(Box::new(ObjectSet::Matching {
            objects: Box::new(self.objects(objects)?),
            filter: self.filter(filter)?,
        })))
    }

    fn condition(&self, value: &source::Condition) -> Result<Condition> {
        self.reduced_condition(value).or_else(|error| {
            if let Some(condition) = self.this_way_condition(value) {
                return condition;
            }
            self.history_condition(value)
                .unwrap_or_else(|| Err(error.clone()))
                .or_else(|_| rescue(error, || self.primitive_condition(value)))
        })
    }

    fn reduced_condition(&self, value: &source::Condition) -> Result<Condition> {
        if let source::Condition::DeadPermanentPassesFilter(filter) = value {
            return Ok(Condition::ObjectMatches {
                object: self.dead_object()?,
                filter: self.filter(filter)?,
            });
        }
        if let Some(crate::pipeline::operands::OperandRecipe::Condition(recipe)) =
            self.operand_recipe(crate::pipeline::operands::OperandRef::Condition(value))
        {
            return self.bind_check_recipe(&recipe);
        }

        use source::Condition as S;
        Ok(match value {
            S::CastSpellPassesFilter(filter) => Condition::ObjectMatches {
                object: if self.event.spell {
                    ObjectRef::EventSubject
                } else {
                    ObjectRef::Source
                },
                filter: self.spell_filter(filter)?,
            },
            S::GraveyardCardPassesFilter(card, filter) => Condition::ObjectMatches {
                object: self.grave_card(card)?,
                filter: self.card_filter(filter)?,
            },
            S::ExiledCardPassesFilter(card, filter) => Condition::ObjectMatches {
                object: self.exiled_card(card)?,
                filter: self.exile_filter(filter)?,
            },
            S::EnteringPermanentPassesFilter(filter) => Condition::ObjectMatches {
                object: if self.event.permanent {
                    ObjectRef::EventSubject
                } else {
                    self.result_object(self.entered.as_ref(), 0)?
                },
                filter: self.filter(filter)?,
            },
            S::TheCardExiledThisWayIsStillExiled => Condition::Compare {
                left: Expr::CountObjects(Box::new(self.still_exiled(ObjectSet::One(
                    self.exiled_card(&source::CardInExile::TheCardExiledThisWay)?,
                )))),
                comparison: Comparison::Greater,
                right: Expr::Constant(0),
            },
            S::DiceResultIs(comparison) => {
                let (op, right) = self.comparison(comparison)?;
                Condition::Compare {
                    left: self.die_result()?,
                    comparison: op,
                    right,
                }
            }
            S::APermanentWasDestroyedThisWay(filter) => Condition::Compare {
                left: Expr::CountObjects(Box::new(self.done_matching(Done::Destroyed, filter)?)),
                comparison: Comparison::Greater,
                right: Expr::Constant(0),
            },
            S::DestroyedPermanentIsPutInAGraveyardThisWay => Condition::Compare {
                left: Expr::CountObjects(Box::new(ObjectSet::Matching {
                    objects: Box::new(ObjectSet::Transition {
                        result: self.done(Done::Destroyed)?,
                        phase: ObjectPhase::After,
                    }),
                    filter: ObjectFilter::InZone(ZoneScope::Players {
                        zone: EventZone::Graveyard,
                        players: PlayerSet::All,
                    }),
                })),
                comparison: Comparison::Greater,
                right: Expr::Constant(0),
            },
            S::APermanentWasSacrificedThisWay(filter) => Condition::Compare {
                left: Expr::CountObjects(Box::new(ObjectSet::Matching {
                    objects: Box::new(self.paid_objects(
                        crate::number_spec::PaymentAction::Sacrifice,
                        CostObjectView::Performed,
                    )?),
                    filter: self.filter(filter)?,
                })),
                comparison: Comparison::Greater,
                right: Expr::Constant(0),
            },
            S::ACardWasDiscardedThisWay(cards) => Condition::Compare {
                left: Expr::CountObjects(Box::new(ObjectSet::Matching {
                    objects: Box::new(self.paid_objects(
                        crate::number_spec::PaymentAction::Discard,
                        CostObjectView::Performed,
                    )?),
                    filter: self.card_filter(cards)?,
                })),
                comparison: Comparison::Greater,
                right: Expr::Constant(0),
            },
            S::APermanentWasPutIntoHandThisWay(filter) => Condition::Compare {
                left: Expr::CountObjects(Box::new(self.done_matching(Done::Returned, filter)?)),
                comparison: Comparison::Greater,
                right: Expr::Constant(0),
            },
            S::ACardWasMilledThisWay(cards) => Condition::Compare {
                left: Expr::CountObjects(Box::new(ObjectSet::Matching {
                    objects: Box::new(ObjectSet::Transition {
                        result: self.done(Done::Milled)?,
                        phase: ObjectPhase::After,
                    }),
                    filter: self.card_filter(cards)?,
                })),
                comparison: Comparison::Greater,
                right: Expr::Constant(0),
            },
            S::APermanentPassesFilter(objects, filter) => Condition::Compare {
                left: self.count_matching(objects, filter)?,
                comparison: Comparison::GreaterOrEqual,
                right: Expr::Constant(1),
            },
            S::NoPermanentsPassFilter(objects, filter) => Condition::Compare {
                left: self.count_matching(objects, filter)?,
                comparison: Comparison::Equal,
                right: Expr::Constant(0),
            },
            S::NumPermanentsPassFilter(count, objects, filter) => {
                let (comparison, right) = self.comparison(count)?;
                Condition::Compare {
                    left: self.count_matching(objects, filter)?,
                    comparison,
                    right,
                }
            }
            S::NoPlayersPassFilter(players, filter) => Condition::Compare {
                left: Expr::CountPlayers(self.players(&source::Players::And(vec![
                    (**players).clone(),
                    (**filter).clone(),
                ]))?),
                comparison: Comparison::Equal,
                right: Expr::Constant(0),
            },
            S::ACardWasRevealedThisWay(cards) => {
                let objects = self.revealed_objects()?;
                Condition::Compare {
                    left: Expr::CountObjects(Box::new(ObjectSet::Matching {
                        objects: Box::new(objects),
                        filter: self.card_filter(cards)?,
                    })),
                    comparison: Comparison::GreaterOrEqual,
                    right: Expr::Constant(1),
                }
            }
            S::AnExiledCardPassesFilter(cards, filter) => Condition::Compare {
                left: Expr::CountObjects(Box::new(ObjectSet::Matching {
                    objects: Box::new(self.still_exiled(self.exile_cards(cards)?)),
                    filter: self.exile_filter(filter)?,
                })),
                comparison: Comparison::Greater,
                right: Expr::Constant(0),
            },
            S::APermanentWasPutOntoTheBattlefieldThisWay(objects) => {
                let result = self.entered.clone().ok_or_else(|| unsupported(value))?;
                Condition::Compare {
                    left: Expr::CountObjects(Box::new(ObjectSet::Matching {
                        objects: Box::new(ObjectSet::Transition {
                            result,
                            phase: ObjectPhase::After,
                        }),
                        filter: self.filter(objects)?,
                    })),
                    comparison: Comparison::Greater,
                    right: Expr::Constant(0),
                }
            }
            S::ACardWasPutIntoHandThisWay(cards) => {
                let results = self
                    .inspection_hand
                    .as_ref()
                    .ok_or_else(|| unsupported(value))?;
                let objects = ObjectSet::Union(
                    results
                        .iter()
                        .map(|result| ObjectSet::Transition {
                            result: result.clone(),
                            phase: ObjectPhase::Before,
                        })
                        .collect(),
                );
                Condition::Compare {
                    left: Expr::CountObjects(Box::new(ObjectSet::Matching {
                        objects: Box::new(objects),
                        filter: self.card_filter(cards)?,
                    })),
                    comparison: Comparison::Greater,
                    right: Expr::Constant(0),
                }
            }
            S::TopCardOfPlayersLibraryPassesFilter(player, cards) => Condition::ObjectMatches {
                object: ObjectRef::Query {
                    objects: Box::new(self.library_slice(
                        self.player(player)?,
                        Expr::Constant(1),
                        SequenceEnd::Top,
                    )),
                    index: 0,
                },
                filter: self.card_filter(cards)?,
            },
            S::ACardWasExiledThisWay(cards) => {
                let result = self.exiled.clone().ok_or_else(|| unsupported(value))?;
                Condition::Compare {
                    left: Expr::CountObjects(Box::new(ObjectSet::Matching {
                        objects: Box::new(ObjectSet::Transition {
                            result,
                            phase: ObjectPhase::Before,
                        }),
                        filter: self.card_filter(cards)?,
                    })),
                    comparison: Comparison::Greater,
                    right: Expr::Constant(0),
                }
            }
            _ => return Err(unsupported(value)),
        })
    }

    fn duration(&self, value: &source::Expiration) -> Result<Duration> {
        Ok(match value {
            source::Expiration::UntilEndOfTurn => Duration::EndOfTurn,
            source::Expiration::UntilEndOfCombat => Duration::EndOfCombat,
            source::Expiration::UntilCardsAreNoLongerExiled(cards) => {
                Duration::WhileAnyExiled(self.exile_cards(cards)?)
            }
            source::Expiration::UntilCardIsCastFromExile(card) => {
                Duration::UntilCast(self.exiled_card(card)?)
            }
            source::Expiration::UntilPlayerPaysMana(player, mana) => Duration::UntilPaid {
                player: self.player(player)?,
                cost: self.cost(&source::Cost::PayMana(mana.clone()))?,
            },
            source::Expiration::UntilEndOfGame => Duration::Indefinite,
            source::Expiration::UntilPlayersNextTurn(p) => Duration::UntilNextTurn(self.player(p)?),
            source::Expiration::UntilTheEndOfPlayersNextTurn(p) => {
                Duration::UntilEndOfNextTurn(self.player(p)?)
            }
            source::Expiration::UntilPermanentLeavesBattlefield(p)
            | source::Expiration::ForAsLongAsPermanentRemainsOnTheBattlefield(p) => {
                Duration::WhileOnBattlefield(self.object(p)?)
            }
            source::Expiration::UntilPermanentChangesControl(player, p)
            | source::Expiration::ForAsLongAsPlayerControlsPermanent(player, p) => {
                Duration::WhileControlled {
                    player: self.player(player)?,
                    object: self.object(p)?,
                }
            }
            source::Expiration::ForAsLongAsPermanentRemainsTapped(p) => Duration::WhileMatches {
                object: self.object(p)?,
                filter: self.filter(&source::Permanents::IsTapped)?,
            },
            source::Expiration::ForAsLongAsPermanentRemainsFaceDown(p) => Duration::WhileMatches {
                object: self.object(p)?,
                filter: self.filter(&source::Permanents::IsFaceDown)?,
            },
            source::Expiration::ForAsLongAsPermanentHasACounterOfTypeOnIt(p, kind) => {
                Duration::WhileMatches {
                    object: self.object(p)?,
                    filter: self.filter(&source::Permanents::HasACounterOfType(kind.clone()))?,
                }
            }
            source::Expiration::UntilPermanentNoLongerPassesFilter(p, filter) => {
                Duration::WhileMatches {
                    object: self.object(p)?,
                    filter: self.filter(filter)?,
                }
            }
            source::Expiration::UntilCardIsNoLongerExiled(card) => {
                Duration::WhileExiled(self.exiled_card(card)?)
            }
            source::Expiration::UntilPlayersNextEndStep(p) => Duration::UntilNext {
                point: TurnPoint::EndStepBegins,
                players: PlayerSet::One(self.player(p)?),
            },
            source::Expiration::UntilTheBeginningOfTheNextEndStep => Duration::UntilNext {
                point: TurnPoint::EndStepBegins,
                players: PlayerSet::All,
            },
            source::Expiration::UntilNextUpkeep(p)
            | source::Expiration::UntilTheBeginningOfPlayersNextUpkeep(p) => Duration::UntilNext {
                point: TurnPoint::UpkeepBegins,
                players: PlayerSet::One(self.player(p)?),
            },
            source::Expiration::DuringPlayersNextUntapStep(p) => {
                Duration::DuringNextUntapStep(self.player(p)?)
            }
            source::Expiration::DuringPlayersNextTurn(p) => {
                Duration::DuringNextTurn(self.player(p)?)
            }
            source::Expiration::Or(values) => Duration::Either(
                values
                    .iter()
                    .map(|value| self.duration(value))
                    .collect::<Result<_>>()?,
            ),
            _ => return Err(unsupported(value)),
        })
    }

    fn modifier(&self, value: &source::ModX, x: &Expr) -> Expr {
        match value {
            source::ModX::Integer(n) => Expr::Constant((*n).into()),
            source::ModX::PlusX => x.clone(),
            source::ModX::MinusX => Expr::multiply(Expr::Constant(-1), x.clone()),
            source::ModX::Zero => Expr::Constant(0),
        }
    }

    fn changes(&self, values: &[source::LayerEffect]) -> Result<Vec<Change>> {
        let mut context = self.clone();
        context.affected = true;
        context.layer_changes(values)
    }

    fn layer_changes(&self, values: &[source::LayerEffect]) -> Result<Vec<Change>> {
        use source::LayerEffect as S;
        let mut result = Vec::new();
        for (index, value) in values.iter().enumerate() {
            let changes: Result<Vec<Change>> = (|| {
                Ok(match value {
                    S::AdjustPT(power, toughness) => vec![Change::Pump {
                        power: Expr::Constant((*power).into()),
                        toughness: Expr::Constant((*toughness).into()),
                    }],
                    S::AdjustPTX(power, toughness, x) => {
                        let x = self.number(x)?;
                        vec![Change::Pump {
                            power: self.modifier(power, &x),
                            toughness: self.modifier(toughness, &x),
                        }]
                    }
                    S::AdjustPTForEach(power, toughness, n) => {
                        let n = self.number(n)?;
                        vec![Change::Pump {
                            power: Expr::multiply(Expr::Constant((*power).into()), n.clone()),
                            toughness: Expr::multiply(Expr::Constant((*toughness).into()), n),
                        }]
                    }
                    S::SetPT(source::PT::PT(p, t)) => vec![Change::SetBasePowerToughness {
                        power: Expr::Constant((*p).into()),
                        toughness: Expr::Constant((*t).into()),
                    }],
                    S::SetPT(source::PT::PTX(power, toughness, x)) => {
                        let x = self.number(x)?;
                        let component = |v: &source::PTXValue| match v {
                            source::PTXValue::Integer(n) => Expr::Constant((*n).into()),
                            source::PTXValue::X => x.clone(),
                        };
                        vec![Change::SetBasePowerToughness {
                            power: component(power),
                            toughness: component(toughness),
                        }]
                    }
                    S::SetPT(source::PT::ManualPT(p, t)) => vec![Change::SetBasePowerToughness {
                        power: self.number(p)?,
                        toughness: self.number(t)?,
                    }],
                    S::SetPowerAndToughnessBoth(n) => {
                        let n = self.number(n)?;
                        vec![Change::SetBasePowerToughness {
                            power: n.clone(),
                            toughness: n,
                        }]
                    }
                    S::SetPower(n) => vec![Change::SetPower(self.number(n)?)],
                    S::SetToughness(n) => vec![Change::SetToughness(self.number(n)?)],
                    S::SetController(p) => vec![Change::SetController(self.player(p)?)],
                    S::SetCreatureType(ty) => vec![Change::CreatureTypes { mode:ChangeMode::Set,types:CreatureTypeSet::Listed(vec![ty.clone()]) }],
                    S::SetCreatureTypes(types) => vec![Change::CreatureTypes { mode:ChangeMode::Set,types:CreatureTypeSet::Listed(types.clone()) }],
                    S::AddCreatureType(ty) => vec![Change::CreatureTypes { mode:ChangeMode::Add,types:CreatureTypeSet::Listed(vec![ty.clone()]) }],
                    S::AddCardtype(ty) => vec![Change::AddCardType(ty.clone())],
                    S::RemoveCardtype(ty) => vec![Change::RemoveCardType(ty.clone())],
                    S::AddSupertype(ty) => vec![Change::AddSupertype(ty.clone())],
                    S::AddLandType(ty) => vec![Change::AddLandType(ty.clone())],
                    S::AddArtifactType(ty) => vec![Change::AddArtifactType(ty.clone())],
                    S::SetArtifactType(ty) => vec![Change::SetArtifactType(ty.clone())],
                    S::SetCardtype(ty) => vec![Change::SetCardTypes(vec![ty.clone()])],
                    S::SetCardtypes(types) => vec![Change::SetCardTypes(types.clone())],
                    S::AddAbility(abilities) => abilities
                        .iter()
                        .map(|r| {
                            if let Ok(k) = self.keyword(r) {
                                Ok(Change::GrantKeyword(k))
                            } else {
                                if matches!(r, source::Rule::CDA_Color(_) | source::Rule::CDA_ColorButNotColorIdentity(_) | source::Rule::CDA_Power(_) | source::Rule::CDA_Toughness(_) | source::Rule::CDA_PowerToughness(..) | source::Rule::CDA_Types(_)) {
                                    return Err(invalid("granted characteristic text needs ordinary static-ability lowering"));
                                }
                                self.fresh()
                                    .ability(r)
                                    .map(|a| Change::GrantAbility(Box::new(a)))
                            }
                        })
                        .collect::<Result<_>>()?,
                    S::AddAbilityVariable(source::AbilityVariable::TheChosenAbility) => {
                        vec![Change::ChosenAbility]
                    }
                    S::SetColor(
                        source::SettableColor::TheChosenColor | source::SettableColor::TheChosenColors,
                    ) => {
                        vec![Change::ChosenColor { mode: ChangeMode::Set }]
                    }
                    S::AddColor(
                        source::SettableColor::TheChosenColor | source::SettableColor::TheChosenColors,
                    ) => {
                        vec![Change::ChosenColor { mode: ChangeMode::Add }]
                    }
                    S::SetColor(c) | S::AddColor(c) => vec![Change::Colors { mode:if matches!(value,S::SetColor(_)) {ChangeMode::Set} else {ChangeMode::Add},colors:self.fixed_colors(c)? }],
                    S::HasAllCreatureTypes => vec![Change::CreatureTypes {mode:ChangeMode::Set,types:CreatureTypeSet::All}],
                    S::RemoveAllCreatureTypes => vec![Change::CreatureTypes {mode:ChangeMode::Set,types:CreatureTypeSet::Listed(vec![])}],
                    S::LosesAllAbilities => vec![Change::RemoveAllAbilities],
                    _ => return Err(unsupported(value)),
                })
            })();
            let changes = changes.or_else(|error| {
                rescue(error, || {
                    self.primitive_change(value).map(|change| vec![change])
                })
            });
            result.extend(changes.map_err(|e| e.at(format!("changes[{index}]")))?);
        }
        Ok(result)
    }

    fn static_changes(&self, values: &[source::StaticLayerEffect]) -> Result<Vec<Change>> {
        // A typed bridge shares the semantic conversion for live definitions
        // and resolving effects. No untyped or legacy value reaches the IR.
        use source::{LayerEffect as T, StaticLayerEffect as S};
        let mut changes = Vec::new();
        for (i, value) in values.iter().enumerate() {
            let change = match value {
                S::AdjustPT(p, t) => T::AdjustPT(*p, *t),
                S::AdjustPTX(p, t, x) => T::AdjustPTX(p.clone(), t.clone(), x.clone()),
                S::AdjustPTForEach(p, t, n) => T::AdjustPTForEach(*p, *t, n.clone()),
                S::SetPT(pt) => T::SetPT(pt.clone()),
                S::SetPowerAndToughnessBoth(n) => T::SetPowerAndToughnessBoth(n.clone()),
                S::SetPower(n) => T::SetPower(n.clone()),
                S::SetToughness(n) => T::SetToughness(n.clone()),
                S::SetController(p) => T::SetController(p.clone()),
                S::AddCreatureType(t) => T::AddCreatureType(t.clone()),
                S::SetCreatureType(t) => T::SetCreatureType(t.clone()),
                S::SetCreatureTypes(t) => T::SetCreatureTypes(t.clone()),
                S::AddCardtype(t) => T::AddCardtype(t.clone()),
                S::AddAbility(a) => T::AddAbility(a.clone()),
                S::SetColor(c) => T::SetColor(c.clone()),
                S::AddColor(c) => T::AddColor(c.clone()),
                S::HasAllCreatureTypes => T::HasAllCreatureTypes,
                S::RemoveAllCreatureTypes => T::RemoveAllCreatureTypes,
                S::LosesAllAbilities => T::LosesAllAbilities,
                _ => match as_family::<T>(value, "LayerEffect") {
                    Some(change) => change,
                    None => {
                        let (name, operands) = self
                            .primitive("StaticLayerEffect", value)
                            .ok_or_else(|| unsupported(value).at(format!("changes[{i}]")))?;
                        changes.push(Change::Primitive { name, operands });
                        continue;
                    }
                },
            };
            changes.extend(
                self.changes(&[change])
                    .map_err(|e| e.at(format!("static[{i}]")))?,
            );
        }
        Ok(changes)
    }

    fn counter_placement(
        &self,
        value: &source::PutCountersAction,
        actor: &PlayerSet,
    ) -> Result<CounterPlacement> {
        use source::PutCountersAction as S;
        let (recipients, counter, amount, choose) = match value {
            S::DistributedCounters(kind) => {
                let (recipients, amount) = self.distribution()?;
                (recipients, kind, amount, None)
            }
            S::ACounterOfTypeOnPermanent(kind, object) => (
                Recipients::Objects(ObjectSet::One(self.object(object)?)),
                kind,
                Expr::Constant(1),
                None,
            ),
            S::ACounterOfTypeOnAPermanent(kind, objects) => (
                Recipients::Objects(self.objects(objects)?),
                kind,
                Expr::Constant(1),
                Some(Count::AsManyAsCan(Expr::Constant(1))),
            ),
            S::ACounterOfTypeOnEachPermanent(kind, objects) => (
                Recipients::Objects(self.objects(objects)?),
                kind,
                Expr::Constant(1),
                None,
            ),
            S::ACounterOfTypeOnPlayer(kind, player) => (
                Recipients::Players(PlayerSet::One(self.player(player)?)),
                kind,
                Expr::Constant(1),
                None,
            ),
            S::ACounterOfTypeOnEachPlayer(kind, players) => (
                Recipients::Players(self.players(players)?),
                kind,
                Expr::Constant(1),
                None,
            ),
            S::NumberCountersOfTypeOnPermanent(n, kind, object) => (
                Recipients::Objects(ObjectSet::One(self.object(object)?)),
                kind,
                self.number(n)?,
                None,
            ),
            S::NumberCountersOfTypeOnAPermanent(n, kind, objects) => (
                Recipients::Objects(self.objects(objects)?),
                kind,
                self.number(n)?,
                Some(Count::AsManyAsCan(Expr::Constant(1))),
            ),
            S::NumberCountersOfTypeOnEachPermanent(n, kind, objects) => (
                Recipients::Objects(self.objects(objects)?),
                kind,
                self.number(n)?,
                None,
            ),
            S::NumberCountersOfTypeOnPlayer(n, kind, player) => (
                Recipients::Players(PlayerSet::One(self.player(player)?)),
                kind,
                self.number(n)?,
                None,
            ),
            S::NumberCountersOfTypeOnEachPlayer(n, kind, players) => (
                Recipients::Players(self.players(players)?),
                kind,
                self.number(n)?,
                None,
            ),
            S::ACounterOfTypeOnUptoNumberPermanents(kind, n, objects) => (
                Recipients::Objects(self.objects(objects)?),
                kind,
                Expr::Constant(1),
                Some(Count::UpTo(self.number(n)?)),
            ),
            _ => return Err(unsupported(value)),
        };
        let selection = choose
            .map(|count| match actor {
                PlayerSet::One(player) => Ok(SelectionChoice {
                    player: player.clone(),
                    count,
                }),
                _ => Err(invalid("counter selection requires one explicit decider")),
            })
            .transpose()?;
        Ok(CounterPlacement {
            recipients,
            kind: crate::counter::lower(counter),
            amount,
            selection,
        })
    }

    fn damage_parts(&self, values: &[source::DamageToRecipients]) -> Result<Vec<DamagePart>> {
        values
            .iter()
            .map(
                |source::DamageToRecipients::DamageToRecipients(n, recipient)| {
                    Ok(DamagePart {
                        recipients: self.recipients(recipient)?,
                        amount: self.number(n)?,
                    })
                },
            )
            .collect()
    }

    fn recipients(&self, value: &source::DamageRecipient) -> Result<Recipients> {
        use source::DamageRecipient as S;
        Ok(match value {
            S::Permanent(p) => Recipients::Objects(ObjectSet::One(self.object(p)?)),
            S::Player(p) => Recipients::Players(PlayerSet::One(self.player(p)?)),
            S::EachPermanent(p) => Recipients::Objects(self.objects(p)?),
            S::EachPlayer(p) => Recipients::Players(self.players(p)?),
            S::PlayerOrPlaneswalkerPermanentIsAttacking(attacker) => {
                Recipients::AttackedBy(self.object(attacker)?)
            }
            S::Ref_AnyTarget
            | S::Ref_AnyTarget1
            | S::Ref_AnyTargets
            | S::Ref_TargetPlayerOrPermanent => {
                Recipients::Targets(self.target(TargetKind::DamageRecipient, 0, 0)?)
            }
            S::MultipleRecipients(items) => Recipients::Union(
                items
                    .iter()
                    .map(|r| self.recipients(r))
                    .collect::<Result<_>>()?,
            ),
            _ => return Err(unsupported(value)),
        })
    }

    /// One player acting is who `ActionPlayer` reads; a batch has no such player.
    fn acted_by<T>(
        &mut self,
        actor: &PlayerSet,
        lower: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        let acting = match actor {
            PlayerSet::One(player) => Some(player.clone()),
            _ => None,
        };
        let outer = std::mem::replace(&mut self.acting, acting);
        let lowered = lower(self);
        self.acting = outer;
        lowered
    }

    fn action(&mut self, value: &source::Action, actor: &PlayerSet) -> Result<Instruction> {
        self.acted_by(actor, |context| context.acted_action(value, actor))
    }

    fn acted_action(&mut self, value: &source::Action, actor: &PlayerSet) -> Result<Instruction> {
        let before = self.clone();
        let reduced = match self
            .dead_action(value, actor)
            .or_else(|| self.damage_action(value))
        {
            Some(result) => result,
            None => self.reduced_action(value, actor),
        };
        match reduced {
            Ok(instruction) => Ok(instruction),
            Err(error) => {
                *self = before;
                let error = match self.reflexive_action(value, actor) {
                    Some(Ok(instruction)) => return Ok(instruction),
                    Some(Err(reason)) => reason,
                    None => error,
                };
                let error = match self.replacement_creation(value) {
                    Some(Ok(operation)) => return Ok(self.effect(operation)),
                    Some(Err(reason)) => reason,
                    None => error,
                };
                if let Some(instruction) = self.primitive_action(value, actor) {
                    return Ok(instruction);
                }
                if !self.lenient {
                    return Err(error);
                }
                self.blockers.borrow_mut().push(error);
                Ok(self.named_action(value, actor))
            }
        }
    }

    fn reduced_action(&mut self, value: &source::Action, actor: &PlayerSet) -> Result<Instruction> {
        if let Some(recipe) =
            self.prepared_program(crate::pipeline::programs::ProgramRef::Action(value))
        {
            return self.program_instruction(&recipe, actor);
        }
        if let Some(surface) = crate::pipeline::rewrite(value) {
            let expanded = if self.prepared.actions.is_empty() {
                surface.expand()
            } else {
                self.prepared
                    .actions
                    .get(&crate::pipeline::surface_key(&surface))
                    .cloned()
                    .unwrap_or_else(|| surface.expand())
            };
            return self.bind_pipeline_recipe(expanded, actor);
        }
        use source::Action as S;
        let operation = match value {
            S::CreateFutureTrigger(future, actions) if next_event(future).is_some() => {
                let event = match next_event(future).unwrap() {
                    NextEvent::Spell(player, spells) => EventSpec::Spell {
                        event: SpellEvent::Cast,
                        players: PlayerSet::One(self.player(player)?),
                        spells: self.spell_filter(spells)?,
                    },
                    NextEvent::Trigger(trigger) => self.trigger(&trigger)?,
                };
                let mut child = self.clone();
                child.open_searches.clear();
                child.event = crate::semantics::event_bindings(&event);
                let known = child.targets.len();
                let body = child.actions(actions)?;
                self.result = child.result;
                Operation::CreateTrigger {
                    event,
                    duration: Duration::EndOfTurn,
                    announcement: Announcement {
                        targets: child.targets.split_off(known),
                        modes: child.modes,
                    },
                    body,
                    once: true,
                }
            }
            S::CreateFutureTrigger(event, actions) | S::CreateFutureTriggerI(event, _, actions) => {
                let (event, start, expires) =
                    self.future_turn(event)
                        .or_else(|error| match self.future_event(event) {
                            Ok(event) => Ok((event, DelayedStart::Immediately, None)),
                            Err(_) => Err(error),
                        })?;
                let mut child = self.clone();
                // Deferred work has its own search episode and result scope.
                // Existing reference bindings remain captured and read-only.
                child.open_searches.clear();
                let source::Actions::ActionList(actions) = actions.as_ref() else {
                    return Err(invalid(
                        "delayed announcements need their own target/mode scope",
                    ));
                };
                let intervening_if = if let S::CreateFutureTriggerI(_, condition, _) = value {
                    Some(child.condition(condition)?)
                } else {
                    None
                };
                let body = child.action_list(actions, actor)?;
                Operation::CreateDelayedTrigger {
                    event,
                    start,
                    expires,
                    intervening_if,
                    body,
                }
            }
            S::LookAtPlayersHand(p) | S::LookAtCardsOfTypeInPlayersHand(_, p) => {
                let PlayerSet::One(viewer) = actor else {
                    return Err(invalid("inspection requires one viewer"));
                };
                Operation::LookAt {
                    player: viewer.clone(),
                    objects: self.private_cards(
                        EventZone::Hand,
                        PlayerSet::One(self.player(p)?),
                        if let S::LookAtCardsOfTypeInPlayersHand(f, _) = value {
                            self.card_filter(f)?
                        } else {
                            ObjectFilter::Any
                        },
                    ),
                }
            }
            S::LookAtFaceDownPermanent(p) => {
                let PlayerSet::One(viewer) = actor else {
                    return Err(invalid("inspection requires one viewer"));
                };
                Operation::LookAt {
                    player: viewer.clone(),
                    objects: ObjectSet::One(self.object(p)?),
                }
            }
            S::LookAtFaceDownExiledCards(cards) => {
                let PlayerSet::One(viewer) = actor else {
                    return Err(invalid("inspection requires one viewer"));
                };
                Operation::LookAt {
                    player: viewer.clone(),
                    objects: self.exile_cards(cards)?,
                }
            }
            S::LookAtTopOfLibrary => {
                let PlayerSet::One(viewer) = actor else {
                    return Err(invalid("inspection requires one viewer"));
                };
                Operation::LookAt {
                    player: viewer.clone(),
                    objects: self.library_slice(
                        viewer.clone(),
                        Expr::Constant(1),
                        SequenceEnd::Top,
                    ),
                }
            }
            S::LookAtTheTopNumberCardsOfLibrary(n, actions)
            | S::LookAtTheTopNumberCardsOfPlayersLibrary(_, n, actions) => {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("inspection requires one viewer"));
                };
                let library = if let S::LookAtTheTopNumberCardsOfPlayersLibrary(p, ..) = value {
                    self.player(p)?
                } else {
                    player.clone()
                };
                return self.inspect_library(player.clone(), library, self.number(n)?, actions);
            }
            S::LookAtTheTopNumberCardsOfLibraryNEW(_) | S::LookAtTheTopCardOfPlayersLibrary(_) => {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("inspection requires one viewer"));
                };
                let (library, amount) = match value {
                    S::LookAtTheTopCardOfPlayersLibrary(p) => (self.player(p)?, Expr::Constant(1)),
                    S::LookAtTheTopNumberCardsOfLibraryNEW(n) => (player.clone(), self.number(n)?),
                    _ => unreachable!(),
                };
                Operation::LookAt {
                    player: player.clone(),
                    objects: self.library_slice(library, amount, SequenceEnd::Top),
                }
            }
            S::SearchLibrary(actions) => {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("search requires one searcher"));
                };
                return self.search_library(player.clone(), player.clone(), None, actions);
            }
            S::SearchLibraryAndGraveyard(actions)
            | S::SearchLibraryAndOrGraveyard(actions)
            | S::SearchLibraryAndGraveyardAndHand(actions)
            | S::SearchLibraryAndOrGraveyardAndOrHand(actions)
            | S::SearchHandAndLibrary(actions)
            | S::SearchHandAndOrLibrary(actions) => {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("search requires one searcher"));
                };
                self.search_either = matches!(
                    value,
                    S::SearchLibraryAndOrGraveyard(_)
                        | S::SearchLibraryAndOrGraveyardAndOrHand(_)
                        | S::SearchHandAndOrLibrary(_)
                );
                self.search_also = match value {
                    S::SearchLibraryAndGraveyard(_) | S::SearchLibraryAndOrGraveyard(_) => {
                        vec![EventZone::Graveyard]
                    }
                    S::SearchHandAndLibrary(_) | S::SearchHandAndOrLibrary(_) => {
                        vec![EventZone::Hand]
                    }
                    _ => vec![EventZone::Graveyard, EventZone::Hand],
                };
                return self.search_library(player.clone(), player.clone(), None, actions);
            }
            S::SearchPlayersLibraryAndGraveyardAndHand(library, actions) => {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("search requires one searcher"));
                };
                self.search_also = vec![EventZone::Graveyard, EventZone::Hand];
                return self.search_library(player.clone(), self.player(library)?, None, actions);
            }
            S::SetLifeTotal(amount) => Operation::SetLifeTotal {
                players: actor.clone(),
                amount: self.number(amount)?,
            },
            S::SearchPlayersLibrary(library, actions) => {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("search requires one searcher"));
                };
                return self.search_library(player.clone(), self.player(library)?, None, actions);
            }
            S::SearchTheTopNumberCardsOfLibrary(n, actions) => {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("search requires one searcher"));
                };
                return self.search_library(
                    player.clone(),
                    player.clone(),
                    Some(self.number(n)?),
                    actions,
                );
            }
            S::Shuffle => Operation::Shuffle {
                players: actor.clone(),
                excluding: None,
            },
            S::Exile(items, flags) => return self.exile(items, flags, actor),
            S::CreateTriggerUntil(trigger, actions, duration) => {
                let event = self.trigger(trigger)?;
                let duration = self.duration(duration)?;
                let mut child = self.clone();
                child.open_searches.clear();
                child.event = crate::semantics::event_bindings(&event);
                let known = child.targets.len();
                let body = child.actions(actions)?;
                self.result = child.result;
                Operation::CreateTrigger {
                    event,
                    duration,
                    announcement: Announcement {
                        targets: child.targets.split_off(known),
                        modes: child.modes,
                    },
                    body,
                    once: false,
                }
            }
            S::DiscardCard(card) => Operation::DiscardObjects {
                objects: ObjectSet::One(self.hand_card(card)?),
            },
            S::CreateTokensForEachPlayer(players, tokens, flags) => {
                let players = self.players(players)?;
                let create = S::CreateTokens(tokens.clone(), flags.clone());
                return self.for_each_player(players, |context, _| context.action(&create, actor));
            }
            // Choosing among earlier choices needs a result of theirs to read.
            S::ChooseAPermanent(objects) | S::ChooseUptoOnePermanent(objects)
                if matches!(actor, PlayerSet::One(_))
                    && (self.permanent_choice.is_some()
                        || !matches!(**objects, source::Permanents::TheChosenPermanents)) =>
            {
                let PlayerSet::One(player) = actor else {
                    unreachable!()
                };
                let choice = self.effect(Operation::ChooseObjects {
                    constraints: vec![],
                    player: player.clone(),
                    objects: self.objects(objects)?,
                    count: if matches!(value, S::ChooseUptoOnePermanent(_)) {
                        Count::UpTo(Expr::Constant(1))
                    } else {
                        Count::AsManyAsCan(Expr::Constant(1))
                    },
                });
                let Instruction::Effect { result, .. } = &choice else {
                    unreachable!()
                };
                // One of those chosen narrows "that permanent", not "those permanents".
                if matches!(**objects, source::Permanents::TheChosenPermanents) {
                    self.narrowed_choice = Some(result.clone());
                } else {
                    self.permanent_choice = Some(result.clone());
                }
                return Ok(choice);
            }
            S::ChooseUptoNumberPermanents(amount, objects)
                if matches!(actor, PlayerSet::One(_)) =>
            {
                let PlayerSet::One(player) = actor else {
                    unreachable!()
                };
                let choice = self.effect(Operation::ChooseObjects {
                    constraints: vec![],
                    player: player.clone(),
                    objects: self.objects(objects)?,
                    count: Count::UpTo(self.number(amount)?),
                });
                let Instruction::Effect { result, .. } = &choice else {
                    unreachable!()
                };
                self.permanent_choice = Some(result.clone());
                return Ok(choice);
            }
            S::ChooseAnyNumberOfGroupPermanents(objects, group)
                if matches!(actor, PlayerSet::One(_)) =>
            {
                let objects = self.objects(objects)?;
                let constraints = self.group_constraints(group)?;
                let (choice, result) = self.pick(actor, objects, Count::AnyNumber, constraints)?;
                self.permanent_choice = Some(result);
                return Ok(choice);
            }
            S::ChooseOneOrTwoPermanents(objects) if matches!(actor, PlayerSet::One(_)) => {
                let objects = self.objects(objects)?;
                let count = Count::Between {
                    minimum: Expr::Constant(1),
                    maximum: Expr::Constant(2),
                };
                let (choice, result) = self.pick(actor, objects, count, vec![])?;
                self.permanent_choice = Some(result);
                return Ok(choice);
            }
            S::ChooseMultiplePermanentsAmoungPermanents(slots, objects)
                if matches!(actor, PlayerSet::One(_)) =>
            {
                return self.pick_slots(actor, slots, objects);
            }
            S::ChooseMultiplePermanentsAmoungPermanentsForEachPlayer(players, slots, objects)
                if matches!(actor, PlayerSet::One(_)) =>
            {
                let players = self.players(players)?;
                return self.for_each_player(players, |context, _| {
                    context.pick_slots(actor, slots, objects)
                });
            }
            S::ChooseAPermanentForEachPlayer(players, objects)
                if matches!(actor, PlayerSet::One(_)) =>
            {
                let players = self.players(players)?;
                return self.for_each_player(players, |context, _| {
                    let objects = context.objects(objects)?;
                    let count = Count::AsManyAsCan(Expr::Constant(1));
                    let (choice, result) = context.pick(actor, objects, count, vec![])?;
                    context.permanent_choice = Some(result);
                    Ok(choice)
                });
            }
            S::ChooseUptoOnePermanentForEachPlayer(objects, players)
                if matches!(actor, PlayerSet::One(_)) =>
            {
                let players = self.players(players)?;
                return self.for_each_player(players, |context, _| {
                    let objects = context.objects(objects)?;
                    let count = Count::UpTo(Expr::Constant(1));
                    let (choice, result) = context.pick(actor, objects, count, vec![])?;
                    context.permanent_choice = Some(result);
                    Ok(choice)
                });
            }
            S::ChooseACardInEachPlayersGraveyard(cards, players)
                if matches!(actor, PlayerSet::One(_)) =>
            {
                let players = self.players(players)?;
                return self.for_each_player(players, |context, one| {
                    context.pick_graveyard(actor, one, cards, Expr::Constant(1))
                });
            }
            S::ChooseNumberCardsInEachPlayersGraveyard(amount, cards, players)
                if matches!(actor, PlayerSet::One(_)) =>
            {
                let players = self.players(players)?;
                let amount = self.number(amount)?;
                return self.for_each_player(players, |context, one| {
                    context.pick_graveyard(actor, one, cards, amount)
                });
            }
            S::CreatePermanentsList(objects) => {
                let note = self.effect(Operation::NoteObjects {
                    objects: self.objects(objects)?,
                });
                let Instruction::Effect { result, .. } = &note else {
                    unreachable!()
                };
                self.noted = Some(result.clone());
                return Ok(note);
            }
            S::PutCardFromHandOnBattlefield(card, flags) => {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("putting a card from hand needs one player"));
                };
                let (destination, entry) = self.move_entry(flags, player.clone())?;
                Operation::Move {
                    objects: ObjectSet::One(self.hand_card(card)?),
                    destination,
                    entry,
                    face_down: false,
                }
            }
            S::ShuffleEachPermanentIntoLibrary(objects) => Operation::Move {
                objects: self.objects(objects)?,
                destination: Destination::OwnersLibraryShuffled,
                entry: EntrySpec::default(),
                face_down: false,
            },
            S::PutAGraveyardCardOntoBattlefield(cards, flags) => {
                let objects = self.grave_cards(cards)?;
                return self.choose_and_enter(objects, flags, actor);
            }
            S::PutACardFromAGraveyardOnBattlefield(cards, owners, flags) => {
                let objects = self.private_cards(
                    EventZone::Graveyard,
                    self.players(owners)?,
                    self.grave_filter(cards)?,
                );
                return self.choose_and_enter(objects, flags, actor);
            }
            S::PutAnExiledCardIntoOwnersHand(cards) => {
                let objects = self.exile_cards(cards)?;
                return self.choose_and_move(
                    objects,
                    Destination::OwnersHand,
                    EntrySpec::default(),
                    actor,
                );
            }
            S::PutACardOfTypeMilledThisWayOntoTheBattlefield(cards, flags) => {
                let objects = self.grave_cards(
                    &source::CardsInGraveyards::CardsOfTypeMilledThisWay(cards.clone()),
                )?;
                return self.choose_and_enter(objects, flags, actor);
            }
            S::ChooseAnExiledCard(cards) if matches!(actor, PlayerSet::One(_)) => {
                let PlayerSet::One(player) = actor else {
                    unreachable!()
                };
                let choice = self.effect(Operation::ChooseObjects {
                    constraints: vec![],
                    player: player.clone(),
                    objects: self.exile_cards(cards)?,
                    count: Count::AsManyAsCan(Expr::Constant(1)),
                });
                let Instruction::Effect { result, .. } = &choice else {
                    unreachable!()
                };
                self.exile_choice = Some(result.clone());
                return Ok(choice);
            }
            S::ActionForEachPermanent(objects, actions) => {
                let objects = self.objects(objects)?;
                return self.for_each_object(objects, |context| {
                    let instructions = actions
                        .iter()
                        .map(|action| context.action(action, actor))
                        .collect::<Result<_>>()?;
                    Ok(Program { instructions })
                });
            }
            S::CreateTokensForEachPermanent(objects, tokens, flags) => {
                let objects = self.objects(objects)?;
                let create = S::CreateTokens(tokens.clone(), flags.clone());
                return self.for_each_object(objects, |context| {
                    Ok(Program {
                        instructions: vec![context.action(&create, actor)?],
                    })
                });
            }
            S::ActionForEachPlayer(players, actions) => {
                let players = self.players(players)?;
                let actor = actor.clone();
                return self.for_each_player(players, |context, _| {
                    let instructions = actions
                        .iter()
                        .map(|action| context.action(action, &actor))
                        .collect::<Result<_>>()?;
                    Ok(Instruction::Group(Program { instructions }))
                });
            }
            S::PutGraveyardCardInOwnersLibraryNthFromTheTop(card, n) => Operation::Move {
                objects: ObjectSet::One(self.grave_card(card)?),
                destination: Destination::OwnersLibraryFromTop(self.number(n)?),
                entry: EntrySpec::default(),
                face_down: false,
            },
            S::PutPermanentInOwnersLibraryNthFromTheTop(object, n) => Operation::Move {
                objects: ObjectSet::One(self.object(object)?),
                destination: Destination::OwnersLibraryFromTop(self.number(n)?),
                entry: EntrySpec::default(),
                face_down: false,
            },
            S::PutEachCardFromEachPlayersGraveyardOntoTheBattlefield(cards, players, flags) => {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("moving graveyard cards needs one player"));
                };
                let (destination, entry) = self.move_entry(flags, player.clone())?;
                Operation::Move {
                    objects: self.private_cards(
                        EventZone::Graveyard,
                        self.players(players)?,
                        self.card_filter(cards)?,
                    ),
                    destination,
                    entry,
                    face_down: false,
                }
            }
            S::PutACardFromPlayersGraveyardOnBattlefield(cards, owner, flags) => {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("choosing a graveyard card needs one player"));
                };
                let choice = self.effect(Operation::ChooseObjects {
                    constraints: vec![],
                    player: player.clone(),
                    objects: self.private_cards(
                        EventZone::Graveyard,
                        PlayerSet::One(self.player(owner)?),
                        self.grave_filter(cards)?,
                    ),
                    count: Count::AsManyAsCan(Expr::Constant(1)),
                });
                let Instruction::Effect { result, .. } = &choice else {
                    unreachable!()
                };
                let (destination, entry) = self.move_entry(flags, player.clone())?;
                let movement = self.effect(Operation::Move {
                    objects: ObjectSet::Selected(result.clone()),
                    destination,
                    entry,
                    face_down: false,
                });
                return Ok(Instruction::Group(Program {
                    instructions: vec![choice, movement],
                }));
            }
            S::PutAnyNumberOfCardsFromHandOntoBattlefield(cards, flags) => {
                let cards: source::CardsInHand =
                    as_family(cards, "CardsInHand").ok_or_else(|| unsupported(value))?;
                return self.put_from_hand(&cards, flags, Some(Count::AnyNumber), actor);
            }
            S::ReturnEachCardFromGraveyardToHand(cards) => Operation::Move {
                objects: self.private_cards(
                    EventZone::Graveyard,
                    actor.clone(),
                    self.card_filter(cards)?,
                ),
                destination: Destination::OwnersHand,
                entry: EntrySpec::default(),
                face_down: false,
            },
            S::DiscardCards(cards) => Operation::DiscardObjects {
                objects: self.private_cards(
                    EventZone::Hand,
                    actor.clone(),
                    self.hand_filter(cards)?,
                ),
            },
            S::AttachEachPermanentToPermanent(attachments, host) => Operation::Attach {
                attachments: self.objects(attachments)?,
                host: self.object(host)?,
            },
            S::ReturnEnchantingGraveyardCardToHand => Operation::Move {
                objects: ObjectSet::One(ObjectRef::Source),
                destination: Destination::OwnersHand,
                entry: EntrySpec::default(),
                face_down: false,
            },
            S::ReturnGraveyardCardsToTopOfLibraryInAnyOrder(cards) => Operation::Move {
                objects: self.grave_cards(cards)?,
                destination: Destination::OwnersLibraryTop,
                entry: EntrySpec::default(),
                face_down: false,
            },
            S::ReturnACardFromGraveyardToBattlefield(cards, _)
            | S::PutACardFromHandOrGraveyardOnBattlefield(cards, _, _)
            | S::PutUptoNumberCardsFromGraveyardToHand(_, cards)
            | S::ReturnAGraveyardCardToHand(cards) => {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("choosing a graveyard card needs one player"));
                };
                let filter = self.card_filter(cards)?;
                let owner = match value {
                    S::PutACardFromHandOrGraveyardOnBattlefield(_, owner, _) => {
                        PlayerSet::One(self.player(owner)?)
                    }
                    _ => actor.clone(),
                };
                let grave = self.private_cards(EventZone::Graveyard, owner.clone(), filter.clone());
                let choice = self.effect(Operation::ChooseObjects {
                    constraints: vec![],
                    player: player.clone(),
                    objects: if matches!(value, S::PutACardFromHandOrGraveyardOnBattlefield(..)) {
                        ObjectSet::Union(vec![
                            self.private_cards(EventZone::Hand, owner, filter),
                            grave,
                        ])
                    } else {
                        grave
                    },
                    count: match value {
                        S::PutUptoNumberCardsFromGraveyardToHand(n, _) => {
                            Count::UpTo(self.number(n)?)
                        }
                        _ => Count::AsManyAsCan(Expr::Constant(1)),
                    },
                });
                let Instruction::Effect { result, .. } = &choice else {
                    unreachable!()
                };
                let (destination, entry) = match value {
                    S::ReturnACardFromGraveyardToBattlefield(_, flags)
                    | S::PutACardFromHandOrGraveyardOnBattlefield(_, _, flags) => {
                        self.move_entry(flags, player.clone())?
                    }
                    _ => (Destination::OwnersHand, EntrySpec::default()),
                };
                let movement = self.effect(Operation::Move {
                    objects: ObjectSet::Selected(result.clone()),
                    destination,
                    entry,
                    face_down: false,
                });
                return Ok(Instruction::Group(Program {
                    instructions: vec![choice, movement],
                }));
            }
            S::PutACardFromHandOnTopOfLibrary
            | S::PutACardFromHandOnBottomOfLibrary
            | S::PutNumCardsFromHandOnTopOfLibraryAnyOrder(_)
            | S::DiscardUptoNumberCards(_) => {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("choosing cards in hand needs one player"));
                };
                let count = match value {
                    S::PutNumCardsFromHandOnTopOfLibraryAnyOrder(n) => {
                        Count::AsManyAsCan(self.number(n)?)
                    }
                    S::DiscardUptoNumberCards(n) => Count::UpTo(self.number(n)?),
                    _ => Count::AsManyAsCan(Expr::Constant(1)),
                };
                let choice = self.effect(Operation::ChooseObjects {
                    constraints: vec![],
                    player: player.clone(),
                    objects: self.private_cards(EventZone::Hand, actor.clone(), ObjectFilter::Any),
                    count,
                });
                let Instruction::Effect { result, .. } = &choice else {
                    unreachable!()
                };
                let chosen = ObjectSet::Selected(result.clone());
                let after = self.effect(if matches!(value, S::DiscardUptoNumberCards(_)) {
                    Operation::DiscardObjects { objects: chosen }
                } else {
                    Operation::Move {
                        objects: chosen,
                        destination: if matches!(value, S::PutACardFromHandOnBottomOfLibrary) {
                            Destination::OwnersLibraryBottom
                        } else {
                            Destination::OwnersLibraryTop
                        },
                        entry: EntrySpec::default(),
                        face_down: false,
                    }
                });
                return Ok(Instruction::Group(Program {
                    instructions: vec![choice, after],
                }));
            }
            S::PutTopOfLibraryInHand
            | S::PutTopOfLibraryInGraveyard
            | S::PutTopOfLibraryOnBottomOfLibrary
            | S::PutTopOfLibraryOnBattlefield(_) => {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("moving a library card needs one player"));
                };
                let (destination, entry) = match value {
                    S::PutTopOfLibraryInHand => (Destination::OwnersHand, EntrySpec::default()),
                    S::PutTopOfLibraryInGraveyard => {
                        (Destination::OwnersGraveyard, EntrySpec::default())
                    }
                    S::PutTopOfLibraryOnBottomOfLibrary => {
                        (Destination::OwnersLibraryBottom, EntrySpec::default())
                    }
                    S::PutTopOfLibraryOnBattlefield(flags) => {
                        self.move_entry(flags, player.clone())?
                    }
                    _ => unreachable!(),
                };
                Operation::Move {
                    objects: self.library_slice(
                        player.clone(),
                        Expr::Constant(1),
                        SequenceEnd::Top,
                    ),
                    destination,
                    entry,
                    face_down: false,
                }
            }
            S::DiscardAnyNumberOfCards | S::DiscardAnyNumberOfCardsOfType(_) => {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("choosing discards needs one player"));
                };
                let filter = match value {
                    S::DiscardAnyNumberOfCardsOfType(cards) => self.card_filter(cards)?,
                    _ => ObjectFilter::Any,
                };
                let choice = self.effect(Operation::ChooseObjects {
                    constraints: vec![],
                    player: player.clone(),
                    objects: self.private_cards(EventZone::Hand, actor.clone(), filter),
                    count: Count::AnyNumber,
                });
                let Instruction::Effect { result, .. } = &choice else {
                    unreachable!()
                };
                let discard = self.effect(Operation::DiscardObjects {
                    objects: ObjectSet::Selected(result.clone()),
                });
                return Ok(Instruction::Group(Program {
                    instructions: vec![choice, discard],
                }));
            }
            S::RevealAnyNumberOfCardsOfTypeFromHand(cards) => {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("choosing reveals needs one player"));
                };
                let choice = self.effect(Operation::ChooseObjects {
                    constraints: vec![],
                    player: player.clone(),
                    objects: self.private_cards(
                        EventZone::Hand,
                        actor.clone(),
                        self.card_filter(cards)?,
                    ),
                    count: Count::AnyNumber,
                });
                let Instruction::Effect { result, .. } = &choice else {
                    unreachable!()
                };
                let reveal = self.effect(Operation::Reveal {
                    objects: ObjectSet::Selected(result.clone()),
                });
                return Ok(Instruction::Group(Program {
                    instructions: vec![choice, reveal],
                }));
            }
            S::ChooseACardFromPlayersRevealedHand(cards, owner) => {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("choosing a card needs one player"));
                };
                let choice = self.effect(Operation::ChooseObjects {
                    constraints: vec![],
                    player: player.clone(),
                    objects: self.private_cards(
                        EventZone::Hand,
                        PlayerSet::One(self.player(owner)?),
                        self.card_filter(cards)?,
                    ),
                    count: Count::AsManyAsCan(Expr::Constant(1)),
                });
                let Instruction::Effect { result, .. } = &choice else {
                    unreachable!()
                };
                self.hand_choice = Some(result.clone());
                return Ok(choice);
            }
            S::GetOneTimeBoon(trigger, actions) => {
                let event = self.trigger(trigger)?;
                let mut child = self.clone();
                child.open_searches.clear();
                child.event = crate::semantics::event_bindings(&event);
                let known = child.targets.len();
                let body = child.actions(actions)?;
                self.result = child.result;
                Operation::CreateTrigger {
                    event,
                    duration: Duration::Indefinite,
                    announcement: Announcement {
                        targets: child.targets.split_off(known),
                        modes: child.modes,
                    },
                    body,
                    once: true,
                }
            }
            S::RevealTopCardOfLibrary => {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("revealing a library needs one player"));
                };
                let objects =
                    self.library_slice(player.clone(), Expr::Constant(1), SequenceEnd::Top);
                let reveal = self.effect(Operation::Reveal { objects });
                let Instruction::Effect { result, .. } = &reveal else {
                    unreachable!()
                };
                self.revealed = Some(ObjectSet::Observed(result.clone()));
                return Ok(reveal);
            }
            S::RevealTheTopNumberCardsOfLibrary(amount, actions) => {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("revealing a library needs one player"));
                };
                let objects =
                    self.library_slice(player.clone(), self.number(amount)?, SequenceEnd::Top);
                return self.reveal_library(
                    player.clone(),
                    Operation::Reveal { objects },
                    None,
                    actions,
                );
            }
            S::RevealCardsFromTheTopOfLibraryUntilACardOfTypeIsRevealed(cards, actions) => {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("revealing a library needs one player"));
                };
                let filter = self.card_filter(cards)?;
                let reveal = Operation::RevealUntil {
                    library: player.clone(),
                    filter: filter.clone(),
                    count: Expr::Constant(1),
                };
                return self.reveal_library(player.clone(), reveal, Some(filter), actions);
            }
            S::RevealCardsFromTheTopOfLibraryUntilANumberOfCardsOfTypeAreRevealed(
                amount,
                cards,
                actions,
            ) => {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("revealing a library needs one player"));
                };
                let filter = self.card_filter(cards)?;
                let reveal = Operation::RevealUntil {
                    library: player.clone(),
                    filter: filter.clone(),
                    count: self.number(amount)?,
                };
                return self.reveal_library(player.clone(), reveal, Some(filter), actions);
            }
            S::ReturnAnExiledCardToBattlefield(cards, flags) => {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("choosing an exiled card needs one player"));
                };
                let choice = self.effect(Operation::ChooseObjects {
                    constraints: vec![],
                    player: player.clone(),
                    objects: self.exile_cards(cards)?,
                    count: Count::AsManyAsCan(Expr::Constant(1)),
                });
                let Instruction::Effect { result, .. } = &choice else {
                    unreachable!()
                };
                let (destination, entry) = self.move_entry(flags, player.clone())?;
                let movement = self.effect(Operation::Move {
                    objects: ObjectSet::Selected(result.clone()),
                    destination,
                    entry,
                    face_down: false,
                });
                return Ok(Instruction::Group(Program {
                    instructions: vec![choice, movement],
                }));
            }
            S::EachPlayerStartingWithAction(players, first, actions) => {
                let players = self.players(players)?;
                let first = self.player(first)?;
                let mut each = self.for_each_player(players, |context, one| {
                    let instructions = actions
                        .iter()
                        .map(|action| context.action(action, one))
                        .collect::<Result<_>>()?;
                    Ok(Instruction::Group(Program { instructions }))
                })?;
                if let Instruction::ForEachPlayer { starting_with, .. } = &mut each {
                    *starting_with = Some(first);
                }
                return Ok(each);
            }
            S::ChooseAnAction(options) => {
                let PlayerSet::One(decider) = actor else {
                    return Err(invalid("a choice of actions needs one player"));
                };
                let mut programs = Vec::new();
                for option in options {
                    let mut child = self.clone();
                    let instructions = option
                        .iter()
                        .map(|action| child.action(action, actor))
                        .collect::<Result<_>>()?;
                    self.result = child.result;
                    self.invalidate_child_bindings(&child);
                    programs.push(Program { instructions });
                }
                return Ok(Instruction::ChooseOne {
                    decider: decider.clone(),
                    options: programs,
                });
            }
            S::EachPlayerCreatesTokens(players, tokens, flags) => {
                let players = self.players(players)?;
                let create = |context: &mut Self, players: &PlayerSet| {
                    Ok(context.effect(Operation::CreateTokens {
                        players: players.clone(),
                        tokens: tokens
                            .iter()
                            .enumerate()
                            .map(|(i, token)| {
                                context
                                    .token(token, 0)
                                    .map_err(|e| e.at(format!("tokens[{i}]")))
                            })
                            .collect::<Result<_>>()?,
                        entry: context.token_entry(flags)?,
                    }))
                };
                // Tokens counted per player need that player bound.
                let per_player = format!("{tokens:?}");
                if per_player.contains("EachablePlayer") || per_player.contains("ActionPlayer") {
                    return self.for_each_player(players, create);
                }
                return create(self, &players);
            }
            S::APlayerAction(players, action) => {
                let chosen = PlayerRef::OneOf(Box::new(self.players(players)?));
                let outer = self.iterated.replace(chosen.clone());
                let instruction = self.action(action, &PlayerSet::One(chosen));
                self.iterated = outer;
                return instruction;
            }
            S::DoNothing => return Ok(Instruction::Group(Program::empty())),
            S::RepeatThisProcess => Operation::RepeatProcess,
            S::RepeatableActionsNumTimes(amount, actions) => {
                let amount = self.number(amount)?;
                let mut child = self.clone();
                let instructions = actions
                    .iter()
                    .map(|action| child.action(action, actor))
                    .collect::<Result<_>>()?;
                self.result = child.result;
                self.invalidate_child_bindings(&child);
                return Ok(Instruction::Times {
                    amount,
                    body: Program { instructions },
                });
            }
            S::RepeatableActions(actions) => {
                let mut child = self.clone();
                let instructions = actions
                    .iter()
                    .map(|action| child.action(action, actor))
                    .collect::<Result<_>>()?;
                self.result = child.result;
                self.invalidate_child_bindings(&child);
                return Ok(Instruction::Repeatable(Program { instructions }));
            }
            S::ValueActions(value, cases) => {
                let value = self.number(value)?;
                let mut instructions = Vec::new();
                for source::ValueAction::ValueAction(range, actions) in cases {
                    let condition = game_range(&value, range);
                    instructions.push(self.conditional(
                        condition,
                        |c| {
                            Ok(Program {
                                instructions: actions
                                    .iter()
                                    .map(|action| c.action(action, actor))
                                    .collect::<Result<_>>()?,
                            })
                        },
                        |_| Ok(Program::empty()),
                    )?);
                }
                return Ok(Instruction::Group(Program { instructions }));
            }
            S::ReturnEachCardFromPlayersGraveyardToBattlefield(cards, player, flags) => {
                let you = self.player(&source::Player::You)?;
                let (destination, entry) = self.move_entry(flags, you)?;
                let players = PlayerSet::One(self.player(player)?);
                Operation::Move {
                    objects: self.private_cards(
                        EventZone::Graveyard,
                        players,
                        self.grave_filter(cards)?,
                    ),
                    destination,
                    entry,
                    face_down: false,
                }
            }
            S::AttachPermanentToAPermanent(attachment, hosts) => {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("attaching to a chosen permanent needs one chooser"));
                };
                let choice = self.effect(Operation::ChooseObjects {
                    constraints: vec![],
                    player: player.clone(),
                    objects: self.objects(hosts)?,
                    count: Count::AsManyAsCan(Expr::Constant(1)),
                });
                let Instruction::Effect { result, .. } = &choice else {
                    unreachable!()
                };
                let attach = self.effect(Operation::Attach {
                    attachments: ObjectSet::One(self.object(attachment)?),
                    host: ObjectRef::Result {
                        result: result.clone(),
                        phase: ObjectPhase::After,
                        index: 0,
                    },
                });
                return Ok(Instruction::Group(Program {
                    instructions: vec![choice, attach],
                }));
            }
            S::RevealHandAndPlayerChoosesACardToDiscard(chooser, cards) => {
                let PlayerSet::One(_) = actor else {
                    return Err(invalid("revealing a hand needs one player"));
                };
                let reveal = self.effect(Operation::Reveal {
                    objects: self.private_cards(EventZone::Hand, actor.clone(), ObjectFilter::Any),
                });
                let choice = self.effect(Operation::ChooseObjects {
                    constraints: vec![],
                    player: self.player(chooser)?,
                    objects: self.private_cards(
                        EventZone::Hand,
                        actor.clone(),
                        self.card_filter(cards)?,
                    ),
                    count: Count::AsManyAsCan(Expr::Constant(1)),
                });
                let Instruction::Effect { result, .. } = &choice else {
                    unreachable!()
                };
                let discard = self.effect(Operation::DiscardObjects {
                    objects: ObjectSet::Selected(result.clone()),
                });
                return Ok(Instruction::Group(Program {
                    instructions: vec![reveal, choice, discard],
                }));
            }
            S::CreateValueX(value) => {
                let value = self.number(value)?;
                self.value_x = Some(ResultId(self.result));
                Operation::BindValue { value }
            }
            S::PutACardFromGraveyardIntoHand(cards) => {
                return self.put_from_graveyard_into_hand(cards, actor)
            }
            S::PutACardFromHandOnBattlefield(cards, flags) => {
                return self.put_from_hand(
                    cards,
                    flags,
                    Some(Count::AsManyAsCan(Expr::Constant(1))),
                    actor,
                )
            }
            S::PutCardsFromHandOnBattlefield(cards, flags) => {
                return self.put_from_hand(cards, flags, None, actor)
            }
            S::ShuffleGraveyardIntoLibrary => {
                return self.shuffle_into_library(&[EventZone::Graveyard], actor)
            }
            S::ShuffleHandAndGraveyardIntoLibrary => {
                return self.shuffle_into_library(&[EventZone::Hand, EventZone::Graveyard], actor)
            }
            S::DiscardHand => {
                let PlayerSet::One(_) = actor else {
                    return Err(invalid("discarding a hand needs one player"));
                };
                Operation::Discard {
                    players: actor.clone(),
                    amount: Expr::CountObjects(Box::new(self.private_cards(
                        EventZone::Hand,
                        actor.clone(),
                        ObjectFilter::Any,
                    ))),
                    random: false,
                }
            }
            S::PermanentDoesntUntapDuringControllersNextUntap(p) => {
                self.skip_next_untap(ObjectSet::One(self.object(p)?))?
            }
            S::EachPermanentDoesntUntapDuringControllersNextUntap(p) => {
                self.skip_next_untap(self.objects(p)?)?
            }
            S::RevealHand => {
                let PlayerSet::One(_) = actor else {
                    return Err(invalid("revealing a hand needs one player"));
                };
                Operation::Reveal {
                    objects: self.private_cards(EventZone::Hand, actor.clone(), ObjectFilter::Any),
                }
            }
            S::ChooseADamageSource(sources) => {
                let PlayerSet::One(player) = actor else {
                    return Err(invalid("a damage source needs one chooser"));
                };
                let instruction = self.effect(Operation::ChooseObjects {
                    player: player.clone(),
                    objects: ObjectSet::DamageSources(self.damage_sources(sources)?),
                    count: Count::Exactly(Expr::Constant(1)),
                    constraints: vec![],
                });
                let Instruction::Effect { result, .. } = &instruction else {
                    unreachable!()
                };
                self.chosen_damage_source = Some(result.clone());
                return Ok(instruction);
            }
            S::CreatePreventDamageUntil(event, actions, duration) => {
                let event = self.prevented_event(event)?;
                let body = self.prevention_body(&event, actions)?;
                Operation::CreateReplacement {
                    event,
                    limit: ReplacementLimit::UntilExpired,
                    duration: self.duration(duration)?,
                    body,
                }
            }
            S::CreateFuturePreventDamage(event, actions) => {
                let (event, limit) = self.future_prevention(event)?;
                let body = self.prevention_body(&event, actions)?;
                Operation::CreateReplacement {
                    event,
                    limit,
                    duration: Duration::EndOfTurn,
                    body,
                }
            }
            S::CreateGraveyardCardEffectUntil(card, effects, duration) => {
                Operation::CreateContinuousEffect {
                    source: ObjectRef::Source,
                    affected: EffectSubjects::Objects(ObjectSet::InZone {
                        objects: Box::new(ObjectSet::One(self.grave_card(card)?)),
                        zone: EventZone::Graveyard,
                    }),
                    changes: self.graveyard_changes(effects)?,
                    duration: self.duration(duration)?,
                    evaluation: EffectEvaluation::Snapshot,
                }
            }
            S::CreateEachCardInPlayersGraveyardEffectUntil(cards, player, effects, duration) => {
                Operation::CreateContinuousEffect {
                    source: ObjectRef::Source,
                    affected: EffectSubjects::Objects(self.private_cards(
                        EventZone::Graveyard,
                        PlayerSet::One(self.player(player)?),
                        self.card_filter(cards)?,
                    )),
                    changes: self.graveyard_changes(effects)?,
                    duration: self.duration(duration)?,
                    evaluation: EffectEvaluation::Snapshot,
                }
            }
            S::CounterSpellOrAbility(object) if self.stack_object(object).is_ok() => {
                Operation::Counter {
                    objects: ObjectSet::One(self.stack_object(object)?),
                    destination: Destination::OwnersGraveyard,
                }
            }
            S::CounterSpell(s)
            | S::CounterSpellIntoExile(s)
            | S::CounterSpellIntoHand(s)
            | S::CounterSpellIntoTopOfLibrary(s)
            | S::CounterSpellIntoBottomOfLibrary(s) => Operation::Counter {
                objects: ObjectSet::One(self.spell(s)?),
                destination: match value {
                    S::CounterSpellIntoExile(_) => Destination::Exile,
                    S::CounterSpellIntoHand(_) => Destination::OwnersHand,
                    S::CounterSpellIntoTopOfLibrary(_) => Destination::OwnersLibraryTop,
                    S::CounterSpellIntoBottomOfLibrary(_) => Destination::OwnersLibraryBottom,
                    _ => Destination::OwnersGraveyard,
                },
            },
            S::RemoveCounters(removals) => Operation::RemoveCounters {
                removals: removals
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        self.remove_counter_action(r, actor)
                            .map_err(|e| e.at(format!("removals[{i}]")))
                    })
                    .collect::<Result<_>>()?,
            },
            S::AttachPermanentToPermanent(attachment, host) => Operation::Attach {
                attachments: ObjectSet::One(self.object(attachment)?),
                host: self.object(host)?,
            },
            S::CreateTokens(tokens, flags) => Operation::CreateTokens {
                players: actor.clone(),
                tokens: tokens
                    .iter()
                    .enumerate()
                    .map(|(i, token)| {
                        self.token(token, 0)
                            .map_err(|e| e.at(format!("tokens[{i}]")))
                    })
                    .collect::<Result<_>>()?,
                entry: self.token_entry(flags)?,
            },
            S::PutCounters(placements)
                if matches!(placements.as_slice(), [only]
                    if self.counter_placement(only, actor).is_err()
                        && self.primitive("PutCountersAction", only).is_some()) =>
            {
                let (name, operands) = self.primitive("PutCountersAction", &placements[0]).unwrap();
                Operation::Primitive {
                    actor: actor.clone(),
                    name,
                    operands,
                }
            }
            S::EachPlayerMayCostOrFallback(players, costs, fallback) => {
                let players = self.players(players)?;
                return self.for_each_player(players, |context, each| {
                    let PlayerSet::One(payer) = each else {
                        return Err(invalid("a payment has one payer"));
                    };
                    let mut parts = vec![];
                    for cost in costs {
                        parts.extend(context.cost_for(cost, payer)?.parts);
                    }
                    let payment = context.effect(Operation::PayCost {
                        cost: Cost { parts },
                        player: payer.clone(),
                        optional: true,
                    });
                    let paid = context
                        .payment
                        .clone()
                        .ok_or_else(|| invalid("a payment records its result"))?;
                    let otherwise = context.conditional(
                        Condition::Not(Box::new(Condition::PaymentChosen(paid))),
                        |c| {
                            Ok(Program {
                                instructions: vec![c.action(fallback, each)?],
                            })
                        },
                        |_| {
                            Ok(Program {
                                instructions: vec![],
                            })
                        },
                    )?;
                    Ok(Instruction::Group(Program {
                        instructions: vec![payment, otherwise],
                    }))
                });
            }
            S::AnyPlayerMayCost(players, cost) => {
                let willing = PlayerRef::FirstWilling(Box::new(self.players(players)?));
                let outer = self.iterated.replace(willing.clone());
                let cost = self.cost(cost);
                self.iterated = outer;
                Operation::PayCost {
                    player: willing,
                    cost: cost?,
                    optional: true,
                }
            }
            S::PutACardOfTypeMilledThisWayIntoHand(cards) => {
                return self.put_from_graveyard_into_hand(
                    &source::CardsInGraveyards::CardsOfTypeMilledThisWay(cards.clone()),
                    actor,
                )
            }
            S::PutCounters(placements) => Operation::PutCounters {
                placements: placements
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        self.counter_placement(p, actor)
                            .map_err(|e| e.at(format!("placements[{i}]")))
                    })
                    .collect::<Result<_>>()?,
            },
            S::AddMana(mana) | S::AddManaWithModifiers(mana, _) => Operation::AddMana {
                players: actor.clone(),
                mana: self.mana_production(mana)?,
                restriction: match value {
                    S::AddManaWithModifiers(_, modifier) => Some(self.mana_use(modifier)?),
                    _ => None,
                },
            },
            S::AddManaRepeated(mana, n)
            | S::AddManaRepeatedWithModifiers(mana, n, _)
            | S::AddCombinationMana(mana, n)
            | S::AddCombinationManaWithModifers(mana, n, _) => Operation::AddMana {
                players: actor.clone(),
                mana: ManaProduction::Repeat {
                    amount: self.number(n)?,
                    production: Box::new(self.mana_production(mana)?),
                    choice: if matches!(
                        value,
                        S::AddCombinationMana(..) | S::AddCombinationManaWithModifers(..)
                    ) {
                        ManaChoiceTiming::EachUnit
                    } else {
                        ManaChoiceTiming::Once
                    },
                },
                restriction: match value {
                    S::AddManaRepeatedWithModifiers(_, _, modifier)
                    | S::AddCombinationManaWithModifers(_, _, modifier) => {
                        Some(self.mana_use(modifier)?)
                    }
                    _ => None,
                },
            },
            S::AddManaRepeatedTwiceWithModifiers(a, n, b, m, modifier) => Operation::AddMana {
                players: actor.clone(),
                mana: ManaProduction::All(vec![
                    ManaProduction::Repeat {
                        amount: self.number(n)?,
                        production: Box::new(self.mana_production(a)?),
                        choice: ManaChoiceTiming::Once,
                    },
                    ManaProduction::Repeat {
                        amount: self.number(m)?,
                        production: Box::new(self.mana_production(b)?),
                        choice: ManaChoiceTiming::Once,
                    },
                ]),
                restriction: Some(self.mana_use(modifier)?),
            },
            S::PermanentDealsDamage(p, n, r) | S::HavePermanentDealDamage(p, n, r) => {
                let source = self.object(p)?;
                return self.damage_each_recipient(source, n, r, true);
            }
            S::SpellDealsDamage(spell, n, r) | S::SpellDealsDamageCantBePrevented(spell, n, r) => {
                if !matches!(spell.as_ref(), source::Spell::ThisSpell) {
                    return Err(unsupported(spell));
                }
                let preventable = matches!(value, S::SpellDealsDamage(_, _, _));
                return self.damage_each_recipient(ObjectRef::Source, n, r, preventable);
            }
            S::PermanentDealsMultipleDamage(permanent, parts) => Operation::DealDamageParts {
                source: self.object(permanent)?,
                parts: self.damage_parts(parts)?,
                preventable: true,
            },
            S::SpellDealsMultipleDamage(spell, parts) => {
                if !matches!(spell.as_ref(), source::Spell::ThisSpell) {
                    return Err(unsupported(spell));
                }
                Operation::DealDamageParts {
                    source: ObjectRef::Source,
                    parts: self.damage_parts(parts)?,
                    preventable: true,
                }
            }
            S::EachPermanentDealsDamage(permanents, n, recipient) => {
                Operation::DealDamageFromEach {
                    sources: self.objects(permanents)?,
                    recipients: self.recipients(recipient)?,
                    amount: self.number(n)?,
                }
            }
            S::SpellDealsDistributedDamage(spell) => {
                if !matches!(spell.as_ref(), source::Spell::ThisSpell) {
                    return Err(unsupported(spell));
                }
                self.distributed_damage(ObjectRef::Source)?
            }
            S::PermanentDealsDistributedDamage(p) => self.distributed_damage(self.object(p)?)?,
            S::DiscardedCardDealsDamage(c, n, r) | S::HaveDiscardedCardDealDamage(c, n, r) => {
                Operation::DealDamage {
                    source: self.hand_card(c)?,
                    recipients: self.recipients(r)?,
                    amount: self.number(n)?,
                    preventable: true,
                }
            }
            S::DealDamage(dealer, n, r) => {
                let source = match dealer.as_ref() {
                    source::DamageDealer::Permanent(p) => self.object(p)?,
                    source::DamageDealer::Spell(source::Spell::ThisSpell) => ObjectRef::Source,
                    _ => return Err(unsupported(dealer)),
                };
                Operation::DealDamage {
                    source,
                    recipients: self.recipients(r)?,
                    amount: self.number(n)?,
                    preventable: true,
                }
            }
            S::CreatePlayerEffect(player, rules) | S::CreatePlayerEffectUntil(player, rules, _) => {
                Operation::CreateContinuousEffect {
                    source: ObjectRef::Source,
                    affected: EffectSubjects::Players(PlayerSet::One(self.player(player)?)),
                    changes: self.player_changes(rules)?,
                    duration: match value {
                        S::CreatePlayerEffectUntil(_, _, duration) => self.duration(duration)?,
                        _ => Duration::Indefinite,
                    },
                    evaluation: EffectEvaluation::LiveObjects,
                }
            }
            S::CreateEachPlayerEffectUntil(players, rules, duration) => {
                Operation::CreateContinuousEffect {
                    source: ObjectRef::Source,
                    affected: EffectSubjects::Players(self.players(players)?),
                    changes: self.player_changes(rules)?,
                    duration: self.duration(duration)?,
                    evaluation: EffectEvaluation::LiveObjects,
                }
            }
            _ => return Err(unsupported(value)),
        };
        Ok(self.effect(operation))
    }

    fn action_list(&mut self, values: &[source::Action], actor: &PlayerSet) -> Result<Program> {
        Ok(Program {
            instructions: values
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    self.action(a, actor)
                        .map_err(|e| e.at(format!("actions[{i}]")))
                })
                .collect::<Result<_>>()?,
        })
    }

    fn stack_object(&self, value: &source::SpellOrAbility) -> Result<ObjectRef> {
        match value {
            source::SpellOrAbility::Ref_TargetSpellOrAbility => self.target_on_stack(),
            source::SpellOrAbility::Trigger_ThatSpellOrAbility if self.event.stack_object => {
                Ok(ObjectRef::EventStackObject)
            }
            _ => Err(unsupported(value)),
        }
    }

    fn target_on_stack(&self) -> Result<ObjectRef> {
        self.object_target(
            ObjectDomain {
                kind: ObjectKind::Any,
                zones: vec![EventZone::Stack],
            },
            0,
        )
    }

    /// Damage whose amount reads "that player" is worked out per recipient.
    fn damage_each_recipient(
        &mut self,
        source: ObjectRef,
        amount: &source::GameNumber,
        recipients: &source::DamageRecipient,
        preventable: bool,
    ) -> Result<Instruction> {
        let per_player = format!("{amount:?}").contains("DealsDamage_ThatPlayer");
        if let (true, source::DamageRecipient::EachPlayer(players)) = (per_player, recipients) {
            let players = self.players(players)?;
            return self.for_each_player(players, |context, each| {
                let PlayerSet::One(player) = each else {
                    return Err(invalid("per-player damage needs one player"));
                };
                context.damage_player = Some(player.clone());
                let amount = context.number(amount);
                context.damage_player = None;
                Ok(context.effect(Operation::DealDamage {
                    source,
                    recipients: Recipients::Players(each.clone()),
                    amount: amount?,
                    preventable,
                }))
            });
        }
        let outer = self.damage_player.take();
        if let source::DamageRecipient::Player(player) = recipients {
            self.damage_player = Some(self.player(player)?);
        }
        let amount = self.number(amount);
        self.damage_player = outer;
        let operation = Operation::DealDamage {
            source,
            recipients: self.recipients(recipients)?,
            amount: amount?,
            preventable,
        };
        Ok(self.effect(operation))
    }

    fn targets(&mut self, values: &[source::Target]) -> Result<()> {
        let mut proposal = self.proposal();
        proposal.proposal_targets(values)?;
        self.targets = proposal.targets;
        Ok(())
    }
    fn proposal_targets(&mut self, values: &[source::Target]) -> Result<()> {
        use source::Target as S;
        for (i, value) in values.iter().enumerate() {
            let mut target = TargetSpec {
                slot: Slot(
                    self.targets
                        .len()
                        .try_into()
                        .map_err(|_| invalid("too many target slots"))?,
                ),
                kind: TargetKind::Object(ObjectDomain::permanent()),
                count: Count::Exactly(Expr::Constant(1)),
                filter: ObjectFilter::Any,
                players: None,
                distribute: None,
                chooser: None,
                constraints: vec![],
            };
            match value {
                S::TargetSpellOrAbility(_) => {
                    target.kind = TargetKind::Object(ObjectDomain {
                        kind: ObjectKind::Any,
                        zones: vec![EventZone::Stack],
                    });
                    target.filter = self
                        .primitive_predicate("Target", value)
                        .ok_or_else(|| unsupported(value))?;
                }
                S::OneOrTwoTargetPermanents(p) => {
                    target.filter = self.filter(p)?;
                    target.count = Count::Between {
                        minimum: Expr::Constant(1),
                        maximum: Expr::Constant(2),
                    };
                }
                S::AnyNumberOfTargetPlayers(players) => {
                    target.kind = TargetKind::Player;
                    target.players = Some(self.players(players)?);
                    target.count = Count::AnyNumber;
                }
                S::TargetAbility(_) | S::UptoOneTargetAbility(_) => {
                    target.kind = TargetKind::Object(ObjectDomain::ability());
                    target.filter = self
                        .primitive_predicate("Target", value)
                        .ok_or_else(|| unsupported(value))?;
                    if matches!(value, S::UptoOneTargetAbility(_)) {
                        target.count = Count::UpTo(Expr::Constant(1));
                    }
                }
                S::TargetSpell(s)
                | S::UptoOneTargetSpell(s)
                | S::UptoNumberTargetSpells(_, s)
                | S::AnyNumberOfTargetSpells(s) => {
                    target.kind = TargetKind::Object(ObjectDomain::spell());
                    target.filter = self.spell_filter(s)?;
                    target.count = match value {
                        S::UptoOneTargetSpell(_) => Count::UpTo(Expr::Constant(1)),
                        S::UptoNumberTargetSpells(n, _) => Count::UpTo(self.number(n)?),
                        S::AnyNumberOfTargetSpells(_) => Count::AnyNumber,
                        _ => Count::Exactly(Expr::Constant(1)),
                    };
                }
                S::UptoOneTargetGraveyardCardInEachPlayersGraveyard(c, players) => {
                    target.kind = TargetKind::Object(ObjectDomain::card(EventZone::Graveyard));
                    target.filter = ObjectFilter::All(vec![
                        self.grave_filter(c)?,
                        ObjectFilter::OwnedBy(self.players(players)?),
                    ]);
                    target.count = Count::AnyNumber;
                    target.constraints = vec![SelectionConstraint::DifferentOwners];
                }
                S::TargetGraveyardCard(c)
                | S::UptoOneTargetGraveyardCard(c)
                | S::NumberTargetGraveyardCards(_, c)
                | S::UptoNumberTargetGraveyardCards(_, c)
                | S::AnyNumberOfTargetGraveyardCards(c) => {
                    target.kind = TargetKind::Object(ObjectDomain::card(EventZone::Graveyard));
                    target.filter = self.grave_filter(c)?;
                    target.count = match value {
                        S::UptoOneTargetGraveyardCard(_) => Count::UpTo(Expr::Constant(1)),
                        S::NumberTargetGraveyardCards(n, _) => Count::Exactly(self.number(n)?),
                        S::UptoNumberTargetGraveyardCards(n, _) => Count::UpTo(self.number(n)?),
                        S::AnyNumberOfTargetGraveyardCards(_) => Count::AnyNumber,
                        _ => Count::Exactly(Expr::Constant(1)),
                    };
                }
                S::NumberTargetGroupGraveyardCards(_, c, group)
                | S::UptoNumberTargetGroupGraveyardCards(_, c, group)
                | S::AnyNumberOfTargetGroupGraveyardCards(c, group) => {
                    target.kind = TargetKind::Object(ObjectDomain::card(EventZone::Graveyard));
                    target.filter = self.grave_filter(c)?;
                    target.constraints = self.group_constraints(group)?;
                    target.count = match value {
                        S::NumberTargetGroupGraveyardCards(n, ..) => {
                            Count::Exactly(self.number(n)?)
                        }
                        S::UptoNumberTargetGroupGraveyardCards(n, ..) => {
                            Count::UpTo(self.number(n)?)
                        }
                        _ => Count::AnyNumber,
                    };
                }
                S::NumberTargetGroupPermanents(_, p, group)
                | S::UptoNumberTargetGroupPermanents(_, p, group)
                | S::AnyNumberOfTargetGroupPermanents(p, group) => {
                    target.filter = self.filter(p)?;
                    target.constraints = self.group_constraints(group)?;
                    target.count = match value {
                        S::NumberTargetGroupPermanents(n, ..) => Count::Exactly(self.number(n)?),
                        S::UptoNumberTargetGroupPermanents(n, ..) => Count::UpTo(self.number(n)?),
                        _ => Count::AnyNumber,
                    };
                }
                S::TargetExiledCard(c) | S::UptoOneTargetExiledCard(c) => {
                    target.kind = TargetKind::Object(ObjectDomain::card(EventZone::Exile));
                    target.filter = self.exile_filter(c)?;
                    if matches!(value, S::UptoOneTargetExiledCard(_)) {
                        target.count = Count::UpTo(Expr::Constant(1));
                    }
                }
                S::TargetPermanent(p) => target.filter = self.filter(p)?,
                S::UptoOneTargetPermanentEachPlayerControls(p, players) => {
                    target.filter = self.filter(p)?;
                    target.count = Count::PerPlayer {
                        players: self.players(players)?,
                        each: Box::new(Count::UpTo(Expr::Constant(1))),
                    };
                }
                S::UptoOneTargetPermanent(p) => {
                    target.filter = self.filter(p)?;
                    target.count = Count::UpTo(Expr::Constant(1));
                }
                S::NumberTargetPermanents(n, p) => {
                    target.filter = self.filter(p)?;
                    target.count = Count::Exactly(self.number(n)?);
                }
                S::UptoNumberTargetPermanents(n, p) => {
                    target.filter = self.filter(p)?;
                    target.count = Count::UpTo(self.number(n)?);
                }
                S::AnyNumberOfTargetPermanents(p) => {
                    target.filter = self.filter(p)?;
                    target.count = Count::AnyNumber;
                }
                S::TargetPlayer(p) | S::UptoOneTargetPlayer(p) => {
                    target.kind = TargetKind::Player;
                    target.players = Some(self.players(p)?);
                    if matches!(value, S::UptoOneTargetPlayer(_)) {
                        target.count = Count::UpTo(Expr::Constant(1));
                    }
                }
                S::AnyTarget => target.kind = TargetKind::DamageRecipient,
                S::AnyTargetExceptPermanent(p) => {
                    target.kind = TargetKind::DamageRecipient;
                    target.filter =
                        ObjectFilter::Not(Box::new(ObjectFilter::SameObject(self.object(p)?)));
                }
                S::AnyTargetExceptAPermanent(p) => {
                    target.kind = TargetKind::DamageRecipient;
                    target.filter = ObjectFilter::Not(Box::new(self.filter(p)?));
                }
                S::TargetPermanentOfPlayersChoice(p, chooser) => {
                    target.filter = self.filter(p)?;
                    target.chooser = Some(self.player(chooser)?);
                }
                S::TargetPlayerOrPermanent(players, objects) => {
                    target.kind = TargetKind::DamageRecipient;
                    target.filter = self.filter(objects)?;
                    target.players = Some(self.players(players)?);
                }
                S::NumberAnyTargets(n) => {
                    target.kind = TargetKind::DamageRecipient;
                    target.count = Count::Exactly(self.number(n)?);
                }
                S::NumberTargetPlayers(n, players) => {
                    target.kind = TargetKind::Player;
                    target.players = Some(self.players(players)?);
                    target.count = Count::Exactly(self.number(n)?);
                }
                S::TargetPermanentEachPlayerControls(p, players) => {
                    let players = self.players(players)?;
                    target.filter = ObjectFilter::All(vec![
                        self.filter(p)?,
                        ObjectFilter::ControlledBy(players.clone()),
                    ]);
                    target.count = Count::UpTo(Expr::CountPlayers(players));
                    target.constraints = vec![SelectionConstraint::DifferentControllers];
                }
                S::AnyNumberOfAnyTargets => {
                    target.kind = TargetKind::DamageRecipient;
                    target.count = Count::AnyNumber;
                }
                S::UptoNumberAnyTargets(n) => {
                    target.kind = TargetKind::DamageRecipient;
                    target.count = Count::UpTo(self.number(n)?);
                }
                S::BetweenOneAndNumberAnyTargets(n) => {
                    target.kind = TargetKind::DamageRecipient;
                    target.count = Count::Between {
                        minimum: Expr::Constant(1),
                        maximum: self.number(n)?,
                    };
                }
                _ => return Err(unsupported(value).at(format!("targets[{i}]"))),
            }
            self.targets.push(target);
        }
        Ok(())
    }

    /// Announces targets with a total split among them, which is the first
    /// slot's when a second slot exists only for another clause.
    fn distributed_targets(
        &mut self,
        values: &[source::DistributedTarget],
        distribution: &source::Distribution,
    ) -> Result<()> {
        use source::{DistributedTarget as S, Distribution as D, Target as T};
        let total = match distribution {
            D::DistributeNumberAmongTargets(_) if values.len() > 1 => {
                return Err(invalid("distribution among several target slots"));
            }
            D::DistributeNumberAmongTargets(n) | D::DistributeNumberAmongAnyTargets(n) => {
                self.number(n)?
            }
            D::IfElse(..) => return Err(unsupported(distribution)),
        };
        let first = self.targets.len();
        let plain: Vec<T> = values
            .iter()
            .map(|value| match value.clone() {
                S::TargetPermanent(p) => T::TargetPermanent(p),
                S::BetweenOneAndNumberTargetPermanents(n, p)
                | S::UptoNumberTargetPermanents(n, p) => T::UptoNumberTargetPermanents(n, p),
                S::AnyNumberOfTargetPermanents(p) => T::AnyNumberOfTargetPermanents(p),
                S::NumberTargetPermanents(n, p) => T::NumberTargetPermanents(n, p),
                S::TargetPlayer(p) => T::TargetPlayer(p),
                S::UptoNumberAnyTargets(n) => T::UptoNumberAnyTargets(n),
                S::AnyNumberOfAnyTargets => T::AnyNumberOfAnyTargets,
                S::BetweenOneAndNumberAnyTargets(n) => T::BetweenOneAndNumberAnyTargets(n),
            })
            .collect();
        self.targets(&plain)?;
        for (value, target) in values.iter().zip(&mut self.targets[first..]) {
            if let (S::BetweenOneAndNumberTargetPermanents(..), Count::UpTo(maximum)) =
                (value, &target.count)
            {
                target.count = Count::Between {
                    minimum: Expr::Constant(1),
                    maximum: maximum.clone(),
                };
            }
        }
        self.targets[first].distribute = Some(total);
        Ok(())
    }

    /// The announced split: who receives a part, and the slot it is read from.
    fn distribution(&self) -> Result<(Recipients, Expr)> {
        let target = self
            .targets
            .iter()
            .find(|target| target.distribute.is_some())
            .ok_or_else(|| invalid("no distribution was announced in this scope"))?;
        let slot = target.slot.clone();
        let recipients = match target.kind {
            TargetKind::Object(_) => Recipients::Objects(ObjectSet::Targets(slot.clone())),
            TargetKind::Player => Recipients::Players(PlayerSet::Targets(slot.clone())),
            TargetKind::DamageRecipient => Recipients::Targets(slot.clone()),
        };
        Ok((recipients, Expr::DistributedShare(slot)))
    }

    fn distributed_damage(&self, source: ObjectRef) -> Result<Operation> {
        let (recipients, amount) = self.distribution()?;
        Ok(Operation::DealDamage {
            source,
            recipients,
            amount,
            preventable: true,
        })
    }

    fn actions(&mut self, value: &source::Actions) -> Result<Program> {
        if let Some(recipe) =
            self.prepared_program(crate::pipeline::programs::ProgramRef::Actions(value))
        {
            return self
                .bind_program_recipe(&recipe, &PlayerSet::One(PlayerRef::AbilityController));
        }
        Err(unsupported(value))
    }

    fn cost_for(&self, value: &source::Cost, player: &PlayerRef) -> Result<Cost> {
        let mut context = self.clone();
        context.payer = Some(player.clone());
        context.cost(value)
    }

    fn cost(&self, value: &source::Cost) -> Result<Cost> {
        match value {
            source::Cost::HavePlayerTakeAction(player, action) => {
                let player = self.player(player)?;
                let mut child = self.clone();
                child.iterated = Some(player.clone());
                // Nothing outside a cost reads what happens inside it.
                child.result = self.result.saturating_add(1 << 20);
                let action: source::Action =
                    as_family(action, "Action").ok_or_else(|| unsupported(action))?;
                let instruction = child.action(&action, &PlayerSet::One(player.clone()))?;
                return Ok(Cost {
                    parts: vec![CostPart::PlayerAction {
                        player,
                        body: Program {
                            instructions: vec![instruction],
                        },
                    }],
                });
            }
            source::Cost::RevealACardOfTypeFromHand(cards) => {
                return Ok(Cost {
                    parts: vec![CostPart::Reveal {
                        count: Expr::Constant(1),
                        filter: self.card_filter(cards)?,
                    }],
                })
            }
            source::Cost::PutPermanentIntoItsOwnersHand(permanent) => {
                return Ok(Cost {
                    parts: vec![CostPart::Move {
                        selections: vec![CostObjectSelection {
                            objects: ObjectSet::One(self.object(permanent)?),
                            choose: None,
                        }],
                        destination: Destination::OwnersHand,
                        entry: EntrySpec::default(),
                        face_down: false,
                    }],
                })
            }
            source::Cost::PutACardFromGraveyardIntoHand(cards) => {
                return Ok(Cost {
                    parts: vec![CostPart::Move {
                        selections: vec![CostObjectSelection {
                            objects: self.grave_cards(cards)?,
                            choose: Some(Count::Exactly(Expr::Constant(1))),
                        }],
                        destination: Destination::OwnersHand,
                        entry: EntrySpec::default(),
                        face_down: false,
                    }],
                })
            }
            source::Cost::PutACardFromHandOnBattlefield(cards, flags) => {
                let payer = self
                    .payer
                    .clone()
                    .unwrap_or(self.player(&source::Player::You)?);
                let (destination, entry) = self.move_entry(flags, payer.clone())?;
                let filter = self.card_filter(cards)?;
                return Ok(Cost {
                    parts: vec![CostPart::Move {
                        selections: vec![CostObjectSelection {
                            objects: self.private_cards(
                                EventZone::Hand,
                                PlayerSet::One(payer),
                                filter,
                            ),
                            choose: Some(Count::Exactly(Expr::Constant(1))),
                        }],
                        destination,
                        entry,
                        face_down: false,
                    }],
                });
            }
            source::Cost::PayManaForEach(mana, times) => {
                return Ok(Cost {
                    parts: vec![CostPart::Repeated {
                        part: Box::new(CostPart::Mana(
                            mana.iter().map(mana_symbol).collect::<Result<_>>()?,
                        )),
                        times: self.number(times)?,
                    }],
                })
            }
            _ => {}
        }
        if let Some(crate::pipeline::operands::OperandRecipe::Cost(recipe)) =
            self.operand_recipe(crate::pipeline::operands::OperandRef::Cost(value))
        {
            return self.bind_cost_recipe(&recipe);
        }
        match self.primitive_cost(value) {
            Some(cost) => Ok(cost),
            None => self.cost_as_action(value),
        }
    }

    /// A cost with the same form as an action is that action, done by the payer.
    /// One card from `objects` goes onto the battlefield.
    fn choose_and_enter(
        &mut self,
        objects: ObjectSet,
        flags: &[source::EnterFlag],
        actor: &PlayerSet,
    ) -> Result<Instruction> {
        let PlayerSet::One(player) = actor else {
            return Err(invalid("choosing a card needs one player"));
        };
        let (destination, entry) = self.move_entry(flags, player.clone())?;
        self.choose_and_move(objects, destination, entry, actor)
    }

    fn choose_and_move(
        &mut self,
        objects: ObjectSet,
        destination: Destination,
        entry: EntrySpec,
        actor: &PlayerSet,
    ) -> Result<Instruction> {
        let PlayerSet::One(player) = actor else {
            return Err(invalid("choosing a card needs one player"));
        };
        let choice = self.effect(Operation::ChooseObjects {
            constraints: vec![],
            player: player.clone(),
            objects,
            count: Count::AsManyAsCan(Expr::Constant(1)),
        });
        let Instruction::Effect { result, .. } = &choice else {
            unreachable!()
        };
        let movement = self.effect(Operation::Move {
            objects: ObjectSet::Selected(result.clone()),
            destination,
            entry,
            face_down: false,
        });
        Ok(Instruction::Group(Program {
            instructions: vec![choice, movement],
        }))
    }

    fn cost_as_action(&self, value: &source::Cost) -> Result<Cost> {
        // Some actions name the zone themselves, where the cost types the operand by it.
        let dealt = match value {
            source::Cost::HaveSpellDealDamage(spell, amount, recipient) => Some(
                source::Action::SpellDealsDamage(spell.clone(), amount.clone(), recipient.clone()),
            ),
            _ => None,
        };
        let action: source::Action = dealt
            .or_else(|| as_family(value, "Action"))
            .or_else(|| {
                cast(
                    value,
                    &[
                        ("Cost", "Action"),
                        ("CardsInHand", "Cards"),
                        ("CardsInGraveyards", "Cards"),
                    ],
                )
            })
            .ok_or_else(|| unsupported(value))?;
        let player = self.payer.clone().unwrap_or(PlayerRef::AbilityController);
        let mut child = self.clone();
        child.iterated = Some(player.clone());
        // Results inside a cost never collide with this scope or another cost's.
        child.result = self.result.saturating_mul(1 << 8).saturating_add(1 << 20);
        let instruction = child.action(&action, &PlayerSet::One(player.clone()))?;
        Ok(Cost {
            parts: vec![CostPart::PlayerAction {
                player,
                body: Program {
                    instructions: vec![instruction],
                },
            }],
        })
    }

    fn limits(
        &self,
        value: &source::ActivateModifier,
        limits: &mut ActivationLimits,
    ) -> Result<()> {
        match value {
            source::ActivateModifier::And(items) => {
                for item in items {
                    self.limits(item, limits)?;
                }
            }
            source::ActivateModifier::ActivateOnlyOnce => limits.once = true,
            source::ActivateModifier::Exhaust => {
                limits.once = true;
                limits.keyword = Some(ActivatedKeyword::Exhaust);
            }
            source::ActivateModifier::PowerUp => {
                limits.once = true;
                limits.keyword = Some(ActivatedKeyword::PowerUp);
            }
            source::ActivateModifier::OtherPlayersMayActivate(players) => {
                limits.activators = Some(self.players(players)?);
            }
            source::ActivateModifier::ActivateOnlyOnceEachTurn => limits.once_each_turn = true,
            source::ActivateModifier::ActivateOnlyAsASorcery => limits.sorcery = true,
            source::ActivateModifier::ActivateOnlyIf(condition)
            | source::ActivateModifier::CantActivateIf(condition) => {
                let mut condition = {
                    let mut context = self.clone();
                    context.activation_limit = true;
                    context.condition(condition)?
                };
                if matches!(value, source::ActivateModifier::CantActivateIf(_)) {
                    condition = Condition::Not(Box::new(condition));
                }
                limits.condition = Some(match limits.condition.take() {
                    Some(previous) => Condition::All(vec![previous, condition]),
                    None => condition,
                });
            }
            source::ActivateModifier::ActivateOnlyDuringTheirTurn => {
                let condition = Condition::PlayerIsActive(self.player(&source::Player::You)?);
                limits.condition = Some(match limits.condition.take() {
                    Some(previous) => Condition::All(vec![previous, condition]),
                    None => condition,
                });
            }
            _ => match self.primitive("ActivateModifier", value) {
                Some((name, operands)) => {
                    limits.modifiers.push(Change::Primitive { name, operands })
                }
                None => return Err(unsupported(value)),
            },
        }
        Ok(())
    }

    fn ability(&mut self, value: &source::Rule) -> Result<Ability> {
        let before = self.clone();
        self.reduced_ability(value).or_else(|error| {
            *self = before;
            match self.replacement_rule(value) {
                Some(result) => result,
                None => rescue(error, || self.primitive_ability(value)),
            }
        })
    }

    fn reduced_ability(&mut self, value: &source::Rule) -> Result<Ability> {
        if !self.prepared.mechanics.is_empty() {
            if let Some(recipe) = self
                .prepared
                .mechanics
                .get(&crate::pipeline::mechanics::source_key(value))
            {
                return self.bind_mechanic(recipe);
            }
        }
        if let Some(recipe) = crate::pipeline::mechanics::rewrite(value) {
            return self.bind_mechanic(&recipe);
        }
        use source::Rule as S;
        match value {
            S::Flashback(cost) => Ok(self.flashback(self.cost(cost)?)),
            S::FlashbackX(mana) => Ok(self.flashback(Cost {
                parts: vec![CostPart::Mana(
                    mana.iter().map(mana_symbol_x).collect::<Result<_>>()?,
                )],
            })),
            S::CDA_Color(_)
            | S::CDA_ColorButNotColorIdentity(_)
            | S::CDA_Power(_)
            | S::CDA_Toughness(_)
            | S::CDA_PowerToughness(..)
            | S::CDA_Types(_) => self.characteristic_definition(value),
            S::FromHand(_)
            | S::FromGraveyard(_)
            | S::FromGraveyardIf(..)
            | S::FromGraveyardOrBattlefield(_)
            | S::FromStack(_)
            | S::FromStackIf(..)
            | S::FromExile(_)
            | S::FromExileIf(..)
            | S::FromExileOrBattlefield(_)
            | S::FromAnyZone(_)
            | S::FromCommandZone(_)
            | S::FromCommandZoneOrBattlefield(_) => self.scoped_ability(value),
            S::Cycling(cost) => self.cycling(self.cost(cost)?),
            S::CyclingX(mana) => self.cycling(Cost {
                parts: vec![CostPart::Mana(
                    mana.iter().map(mana_symbol_x).collect::<Result<_>>()?,
                )],
            }),
            S::TypeCycling(filter, cost) => {
                self.typecycling(self.card_filter(filter)?, self.cost(cost)?)
            }
            S::ThisSpellEffect(effects) => with_source_scope(
                vec![EventZone::Stack],
                None,
                self.stack_effect(ObjectFilter::SameObject(ObjectRef::Source), effects)?,
            ),
            S::StackSpellsEffect(spells, effects) => {
                self.stack_effect(self.spell_filter(spells)?, effects)
            }
            S::CastEffect(rule) => self
                .casting_rule(rule)
                .map(Ability::Casting)
                .or_else(|error| {
                    rescue(error, || {
                        self.primitive("CastEffect", rule)
                            .map(|(name, operands)| Ability::Primitive { name, operands })
                    })
                }),
            S::NoMoreThanNumberCreaturesCanAttack(n) | S::NoMoreThanNumberCreaturesCanBlock(n) => {
                Ok(Ability::Static {
                    kind: StaticKind::Ordinary,
                    affected: EffectSubjects::Objects(ObjectSet::Battlefield(
                        ObjectFilter::CardType(source::CardType::Creature),
                    )),
                    changes: vec![Change::Rule(RuleChange::CombatGroup {
                        role: if matches!(value, S::NoMoreThanNumberCreaturesCanAttack(_)) {
                            CombatRole::Attacker
                        } else {
                            CombatRole::Blocker
                        },
                        participants: ObjectFilter::Any,
                        others_only: false,
                        comparison: Comparison::LessOrEqual,
                        amount: self.number(n)?,
                    })],
                })
            }
            S::ActivatedAbilityEffect(filter, source::ActivatedAbilityEffect::CantBeActivated) => {
                let (sources, filter) = self.permanent_activations(filter)?;
                Ok(Ability::Static {
                    kind: StaticKind::Ordinary,
                    affected: EffectSubjects::Objects(ObjectSet::Battlefield(sources.ok_or_else(
                        || {
                            invalid(
                                "activation restriction includes sources outside the battlefield",
                            )
                        },
                    )?)),
                    changes: vec![Change::Rule(RuleChange::Prohibit(ObjectAction::Activate(
                        filter,
                    )))],
                })
            }
            S::AsPermanentEnters(p, actions)
                if matches!(p.as_ref(), source::Permanent::ThisPermanent) =>
            {
                self.entry_replacement(
                    ReplacementActive::SourceEntering,
                    ObjectFilter::SameObject(ObjectRef::Source),
                    actions,
                )
            }
            S::PreventDamage(event, actions) => {
                let event = self.prevented_event(event)?;
                let body = self.prevention_body(&event, actions)?;
                Ok(Ability::Replacement {
                    active: ReplacementActive::Battlefield,
                    event,
                    body,
                })
            }
            S::ReplaceWouldEnter(event, actions) => {
                let objects = match event {
                    source::ReplacableEventWouldEnter::APermanentWouldEnterTheBattlefield(p) => self.filter(p)?,
                    source::ReplacableEventWouldEnter::PermanentWouldEnterTheBattlefield(p) => ObjectFilter::SameObject(self.object(p)?),
                    source::ReplacableEventWouldEnter::APermanentWouldEnterTheBattlefieldUnderAPlayersControl(p,players) => ObjectFilter::All(vec![self.filter(p)?,ObjectFilter::ControlledBy(self.players(players)?)]),
                    _=>return Err(unsupported(event)),
                };
                let active = if matches!(objects, ObjectFilter::SameObject(ObjectRef::Source)) {
                    ReplacementActive::SourceEntering
                } else {
                    ReplacementActive::Battlefield
                };
                self.entry_replacement(active, objects, actions)
            }
            S::EnchantPermanent(hosts) => Ok(Ability::Enchant {
                hosts: EnchantHosts::Permanents(self.filter(hosts)?),
            }),
            S::EnchantPlayer(players) => Ok(Ability::Enchant {
                hosts: EnchantHosts::Players(self.players(players)?),
            }),
            S::Equip(quality, cost) | S::EquipWithModifiers(quality, cost, _) => {
                let filter = self.filter(quality)?;
                fn planeswalker(f: &ObjectFilter) -> bool {
                    match f {
                        ObjectFilter::CardType(source::CardType::Planeswalker) => true,
                        ObjectFilter::All(v) | ObjectFilter::Either(v) => {
                            v.iter().any(planeswalker)
                        }
                        ObjectFilter::Not(f) => planeswalker(f),
                        _ => false,
                    }
                }
                if planeswalker(&filter) {
                    return Err(invalid(
                        "equip planeswalker requires its separate as-though-creature semantics",
                    ));
                }
                let slot = Slot(0);
                let mut limits = ActivationLimits {
                    sorcery: true,
                    ..ActivationLimits::unrestricted()
                };
                if let S::EquipWithModifiers(_, _, modifier) = value {
                    self.limits(modifier, &mut limits)?;
                }
                Ok(Ability::Activated {
                    origin: ActivationOrigin::Equip,
                    cost: self.cost(cost)?,
                    limits,
                    announcement: Announcement {
                        targets: vec![TargetSpec {
                            slot: slot.clone(),
                            kind: TargetKind::Object(ObjectDomain::permanent()),
                            count: Count::Exactly(Expr::Constant(1)),
                            filter: ObjectFilter::All(vec![
                                filter,
                                ObjectFilter::ControlledBy(PlayerSet::One(
                                    PlayerRef::AbilityController,
                                )),
                            ]),
                            players: None,
                            distribute: None,
                            chooser: None,
                            constraints: vec![],
                        }],
                        modes: None,
                    },
                    body: Program {
                        instructions: vec![self.effect(Operation::Attach {
                            attachments: ObjectSet::One(ObjectRef::Source),
                            host: ObjectRef::Target { slot, index: 0 },
                        })],
                    },
                })
            }
            S::PlayerEffect(player, rules) => Ok(Ability::Static {
                kind: StaticKind::Ordinary,
                affected: EffectSubjects::Players(PlayerSet::One(self.player(player)?)),
                changes: self.player_changes(rules)?,
            }),
            S::EachPlayerEffect(players, rules) => Ok(Ability::Static {
                kind: StaticKind::Ordinary,
                affected: EffectSubjects::Players(self.players(players)?),
                changes: self.player_changes(rules)?,
            }),
            S::PermanentRuleEffect(object, rules) => Ok(Ability::Static {
                kind: StaticKind::Ordinary,
                affected: EffectSubjects::Objects(ObjectSet::One(self.object(object)?)),
                changes: self.rule_changes(rules)?,
            }),
            S::EachPermanentRuleEffect(objects, rules) => Ok(Ability::Static {
                kind: StaticKind::Ordinary,
                affected: EffectSubjects::Objects(self.objects(objects)?),
                changes: self.rule_changes(rules)?,
            }),
            S::PermanentLayerEffect(object, changes) => Ok(Ability::Static {
                kind: StaticKind::Ordinary,
                affected: EffectSubjects::Objects(ObjectSet::One(self.object(object)?)),
                changes: self.static_changes(changes)?,
            }),
            S::EachPermanentLayerEffect(objects, changes) => Ok(Ability::Static {
                kind: StaticKind::Ordinary,
                affected: EffectSubjects::Objects(self.objects(objects)?),
                changes: self.static_changes(changes)?,
            }),
            S::If(condition, then) | S::Unless(condition, then) | S::IfElse(condition, then, _) => {
                let mut condition = self.condition(condition)?;
                if matches!(value, S::Unless(_, _)) {
                    condition = Condition::Not(Box::new(condition));
                }
                let abilities = |rules: &[source::Rule]| {
                    rules
                        .iter()
                        .enumerate()
                        .map(|(i, rule)| {
                            self.fresh()
                                .ability(rule)
                                .map_err(|e| e.at(format!("abilities[{i}]")))
                        })
                        .collect::<Result<Vec<_>>>()
                };
                Ok(Ability::Conditional {
                    condition,
                    then: abilities(then)?,
                    otherwise: match value {
                        S::IfElse(_, _, otherwise) => abilities(otherwise)?,
                        _ => vec![],
                    },
                })
            }
            S::SpellActions(actions) => match actions.as_ref() {
                source::Actions::Modal_ChooseOne_Entwine(cost, modes) => self.cost_branch(
                    "Entwine",
                    cost,
                    &source::Actions::Modal_ChooseBoth(modes.clone()),
                    &source::Actions::Modal_ChooseOne(modes.clone()),
                ),
                source::Actions::Modal_ChooseTwo_Entwine(cost, modes) => self.cost_branch(
                    "Entwine",
                    cost,
                    &source::Actions::Modal_ChooseBoth(modes.clone()),
                    &source::Actions::Modal_ChooseTwo(modes.clone()),
                ),
                source::Actions::AdditionalCost_Modal(options) => {
                    use source::AdditionalCostOption as O;
                    let [O::AdditionalCost(cost, paid), O::NoAdditionalCost(unpaid)] =
                        options.as_slice()
                    else {
                        return Err(unsupported(actions));
                    };
                    self.cost_branch("AdditionalCost", cost, paid, unpaid)
                }
                actions => self.spell_ability(actions),
            },
            S::TriggerMayOnceEachTurn(trigger, actions) => {
                let source::Actions::ActionList(list) = actions.as_ref() else {
                    return Err(unsupported(value));
                };
                self.reduced_ability(&S::TriggerOnceEachTurn(
                    trigger.clone(),
                    Box::new(source::Actions::ActionList(vec![
                        source::Action::MayActions(list.clone()),
                    ])),
                ))
            }
            S::SpellActions_Kicker(cost, source::WasKicked(paid), source::WasntKicked(unpaid)) => {
                self.cost_branch("Kicker", cost, paid, unpaid)
            }
            S::SpellActions_Overload(
                cost,
                source::OverloadPaid(paid),
                source::OverloadNotPaid(unpaid),
            ) => self.cost_branch("Overload", cost, paid, unpaid),
            S::SpellActions_Cleave(
                cost,
                source::CleavePaid(paid),
                source::CleaveNotPaid(unpaid),
            ) => self.cost_branch("Cleave", cost, paid, unpaid),
            S::SpellActions_Awaken(
                cost,
                source::WasAwakened(paid),
                source::WasntAwakened(unpaid),
            ) => self.cost_branch("Awaken", cost, paid, unpaid),
            S::Activated(cost, actions) | S::ActivatedWithModifiers(cost, actions, _) => {
                let cost = self.cost(cost)?;
                self.install_paid_history(&cost, PaymentRef::Activation);
                let mut limits = ActivationLimits::unrestricted();
                if cost.parts.iter().any(|p| matches!(p, CostPart::Loyalty(_))) {
                    limits.sorcery = true;
                }
                if let S::ActivatedWithModifiers(_, _, modifier) = value {
                    self.proposal().limits(modifier, &mut limits)?;
                }
                let body = self.actions(actions)?;
                Ok(Ability::Activated {
                    origin: ActivationOrigin::Ordinary,
                    cost,
                    limits,
                    announcement: Announcement {
                        targets: self.targets.clone(),
                        modes: self.modes.clone(),
                    },
                    body,
                })
            }
            S::TriggerA(trigger, actions)
            | S::TriggerI(trigger, _, actions)
            | S::TriggerOnce(trigger, actions)
            | S::TriggerOnceEachTurn(trigger, actions)
            | S::TriggerTwiceEachTurn(trigger, actions)
            | S::TriggerOnceEachTurnI(trigger, _, actions)
            | S::TriggerIOnce(trigger, _, actions)
            | S::TriggerIOnceEachTurn(trigger, _, actions) => {
                let event = self.trigger(trigger)?;
                self.event = crate::semantics::event_bindings(&event);
                if matches!(trigger, source::Trigger::WhenAPermanentEntersTheBattlefield(p) | source::Trigger::WhenAPermanentEntersTheBattlefieldFromExile(p) if matches!(p.as_ref(), source::Permanents::SinglePermanent(p) if matches!(p.as_ref(), source::Permanent::ThisPermanent)))
                {
                    self.cast_x = Some(ObjectRef::EventSubject);
                }
                let recipient_alias = legacy_damage_recipient_alias(trigger, actions);
                self.ambiguous_damage_permanent =
                    legacy_damage_ambiguity(&event) && !recipient_alias;
                self.damage_permanent_recipient = recipient_alias
                    || legacy_damage_permanent_role(&event) == Some(LegacyPermanentRef::Recipient);
                self.state_subject = match trigger {
                    source::Trigger::WhenAPermanentHasNumberCountersOfType(objects, _, _) => {
                        match objects.as_ref() {
                            source::Permanents::SinglePermanent(object) => {
                                Some(self.object(object)?)
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                };
                let intervening_if = match value {
                    S::TriggerI(_, condition, _)
                    | S::TriggerOnceEachTurnI(_, condition, _)
                    | S::TriggerIOnce(_, condition, _)
                    | S::TriggerIOnceEachTurn(_, condition, _) => Some(self.condition(condition)?),
                    _ => None,
                };
                let limits = TriggerLimits {
                    per_turn: match value {
                        S::TriggerOnceEachTurn(_, _)
                        | S::TriggerOnceEachTurnI(_, _, _)
                        | S::TriggerIOnceEachTurn(_, _, _) => Some(1),
                        S::TriggerTwiceEachTurn(_, _) => Some(2),
                        _ => None,
                    },
                    total: match value {
                        S::TriggerOnce(_, _) | S::TriggerIOnce(_, _, _) => Some(1),
                        _ => None,
                    },
                };
                let body = self.actions(actions)?;
                Ok(Ability::Triggered {
                    event,
                    intervening_if,
                    limits,
                    announcement: Announcement {
                        targets: self.targets.clone(),
                        modes: self.modes.clone(),
                    },
                    body,
                })
            }
            S::ClassAbilities(levels) => Ok(Ability::Class {
                levels: levels
                    .iter()
                    .map(|source::ClassAbility::ClassAbility(cost, rules)| {
                        Ok(ClassLevel {
                            cost: self.cost(&source::Cost::PayMana(cost.clone()))?,
                            abilities: rules
                                .iter()
                                .map(|rule| self.fresh().ability(rule))
                                .collect::<Result<_>>()?,
                        })
                    })
                    .collect::<Result<_>>()?,
            }),
            S::SagaChapters(chapters) => {
                let mut abilities = vec![];
                for source::SagaChapter::SagaChapter(numbers, actions) in chapters {
                    for number in numbers {
                        let mut chapter = self.clone();
                        let chapter_number = u32::try_from(*number)
                            .map_err(|_| invalid("saga chapter number is negative"))?;
                        let body = chapter.actions(actions)?;
                        abilities.push(Ability::Triggered {
                            event: EventSpec::SagaChapter(chapter_number),
                            intervening_if: None,
                            limits: TriggerLimits {
                                per_turn: None,
                                total: None,
                            },
                            announcement: Announcement {
                                targets: chapter.targets,
                                modes: chapter.modes,
                            },
                            body,
                        });
                    }
                }
                Ok(Ability::Saga {
                    chapters: abilities,
                })
            }
            _ => self.keyword(value).map(Ability::Keyword),
        }
    }
}

fn game_range(value: &Expr, range: &source::GameRange) -> Condition {
    let compare = |comparison, n: i32| Condition::Compare {
        left: value.clone(),
        comparison,
        right: Expr::Constant(n.into()),
    };
    match range {
        source::GameRange::BetweenValues(low, high) => Condition::All(vec![
            compare(Comparison::GreaterOrEqual, *low),
            compare(Comparison::LessOrEqual, *high),
        ]),
        source::GameRange::ValueOrBigger(n) => compare(Comparison::GreaterOrEqual, *n),
        source::GameRange::ValueOrSmaller(n) => compare(Comparison::LessOrEqual, *n),
        source::GameRange::ExactValue(n) => compare(Comparison::Equal, *n),
    }
}

fn keyword(value: &source::Rule) -> Result<Keyword> {
    use source::Rule as S;
    Ok(match value {
        S::Flying => Keyword::Flying,
        S::Trample => Keyword::Trample,
        S::Lifelink => Keyword::Lifelink,
        S::Deathtouch => Keyword::Deathtouch,
        S::Vigilance => Keyword::Vigilance,
        S::Haste => Keyword::Haste,
        S::Reach => Keyword::Reach,
        S::FirstStrike => Keyword::FirstStrike,
        S::DoubleStrike => Keyword::DoubleStrike,
        S::Indestructible => Keyword::Indestructible,
        S::Hexproof => Keyword::Hexproof,
        S::Shroud => Keyword::Shroud,
        S::Menace => Keyword::Menace,
        S::Defender => Keyword::Defender,
        S::Flash => Keyword::Flash,
        S::Fear => Keyword::Fear,
        S::Intimidate => Keyword::Intimidate,
        S::Infect => Keyword::Infect,
        S::Wither => Keyword::Wither,
        S::Prowess => Keyword::Prowess,
        S::Flanking => Keyword::Flanking,
        S::Horsemanship => Keyword::Horsemanship,
        S::Shadow => Keyword::Shadow,
        S::Skulk => Keyword::Skulk,
        _ => return Err(unsupported(value)),
    })
}

pub fn card(value: &source::OracleCard) -> std::result::Result<Card, Vec<Diagnostic>> {
    crate::corpus::on_large_stack(|| crate::pipeline::lower(value))
}
pub(crate) fn bind_card(
    value: &source::OracleCard,
    prepared: crate::pipeline::PreparedRecipes,
) -> std::result::Result<Card, Vec<Diagnostic>> {
    bind_face(
        value,
        Context {
            prepared,
            ..Context::default()
        },
    )
}
/// Lower a face that strict lowering rejected, keeping every part it can reduce.
/// Each rule yields exactly one ability, so ability indices still match rule indices.
pub(crate) fn unified_face(value: &source::OracleCard) -> (Card, Vec<Diagnostic>) {
    let base = Context {
        lenient: true,
        ..Context::default()
    };
    let card = bind_face(value, base.clone()).expect("lenient lowering is total");
    let blockers = base.blockers.take();
    (card, blockers)
}
fn bind_face(
    value: &source::OracleCard,
    base: Context,
) -> std::result::Result<Card, Vec<Diagnostic>> {
    let lenient = base.lenient;
    let source::OracleCard::Card {
        name,
        typeline,
        mana_cost,
        color_indicator,
        rules,
        card_pt,
        loyalty,
        defense,
    } = value
    else {
        return Err(vec![unsupported(value)]);
    };
    let mut unreduced = Vec::new();
    let mana_failed = lenient
        && mana_cost
            .as_ref()
            .is_some_and(|m| m.iter().any(|s| mana_symbol_x(s).is_err()));
    let loyalty_failed =
        lenient && matches!(loyalty, Some(l) if !matches!(l, source::LoyaltyNumber::Integer(_)));
    if mana_failed {
        unreduced.push(Ability::Named(named_field("ManaCost", mana_cost)));
    }
    if loyalty_failed {
        unreduced.push(Ability::Named(named_field("Loyalty", loyalty)));
    }
    let mana_cost = mana_cost.as_ref().filter(|_| !mana_failed);
    let loyalty = loyalty.as_ref().filter(|_| !loyalty_failed);
    let metadata = (|| -> Result<Characteristics> {
        let color_indicator = color_indicator
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|c| match c {
                source::ColorIndicatorColor::White => ManaColor::White,
                source::ColorIndicatorColor::Blue => ManaColor::Blue,
                source::ColorIndicatorColor::Black => ManaColor::Black,
                source::ColorIndicatorColor::Red => ManaColor::Red,
                source::ColorIndicatorColor::Green => ManaColor::Green,
            })
            .collect();
        Ok(Characteristics {
            supertypes: typeline.supertypes.clone(),
            card_types: typeline.cardtypes.clone(),
            subtypes: typeline.subtypes.clone(),
            mana_cost: mana_cost
                .map(|m| m.iter().map(mana_symbol_x).collect::<Result<_>>())
                .transpose()?,
            color_indicator,
            power_toughness: card_pt.as_ref().map(|p| (p.power, p.toughness)),
            loyalty: loyalty
                .map(|l| match l {
                    source::LoyaltyNumber::Integer(n) => Ok(Expr::Constant((*n).into())),
                    _ => Err(unsupported(l)),
                })
                .transpose()?,
            defense: *defense,
        })
    })();
    let mut errors = Vec::new();
    let mut abilities = Vec::new();
    let casting_histories: Vec<_> = rules
        .as_deref()
        .unwrap_or_default()
        .iter()
        .enumerate()
        .filter_map(|(i, rule)| {
            let source::Rule::CastEffect(effect) = rule else {
                return None;
            };
            let mut strict = base.fresh();
            strict.lenient = false;
            let native = strict.casting_rule(effect).ok()?;
            let ability = Ability::Casting(native);
            crate::cost_history::casting_cost(&ability)
                .cloned()
                .map(|cost| (i as u32, cost))
        })
        .collect();
    for (i, rule) in rules.as_deref().unwrap_or_default().iter().enumerate() {
        let mut context = base.fresh();
        if matches!(rule, source::Rule::SpellActions(_)) {
            // Multiple declarations of the same role are ambiguous, even when
            // they reside in different additional-cost abilities.
            let mut combined = std::collections::BTreeMap::new();
            for (ability, cost) in &casting_histories {
                context.install_paid_history(cost, PaymentRef::Casting { ability: *ability });
                for (action, _) in crate::cost_history::components(cost) {
                    if combined.insert(action, ()).is_some() {
                        context.paid.insert(action, None);
                    }
                }
            }
        }
        match context.ability(rule) {
            Ok(ability) => abilities.push(ability),
            Err(error) if lenient => {
                base.blockers
                    .borrow_mut()
                    .push(error.at(format!("Rules[{i}]")));
                abilities.push(Ability::Named(named_rule(rule)));
            }
            Err(error) => errors.push(error.at(format!("Rules[{i}]"))),
        }
    }
    abilities.append(&mut unreduced);
    let characteristics = match metadata {
        Ok(m) => Some(m),
        Err(e) => {
            errors.push(e.at("characteristics"));
            None
        }
    };
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(Card {
        schema_version: SCHEMA_VERSION,
        name: name.clone(),
        characteristics: characteristics.unwrap(),
        abilities,
    })
}

// Mana symbols are generated as exhaustive matches over the legacy vocabulary.
// This is inert printed data, shared by costs and characteristics.
include!("lower_mana.rs");
include!("lower_tokens.rs");
include!("lower_mana_production.rs");

include!("lower_events.rs");

include!("lower_queries.rs");

include!("lower_counters.rs");
include!("lower_replacement.rs");
include!("lower_prevention.rs");
include!("lower_history.rs");

include!("lower_rules.rs");

include!("lower_card_filters.rs");
include!("lower_zones.rs");
include!("lower_casting.rs");
include!("lower_cast_history.rs");
include!("lower_damage.rs");
include!("lower_scopes.rs");
include!("lower_library.rs");

include!("lower_players.rs");

include!("lower_keywords.rs");

include!("lower_inspection.rs");

include!("lower_characteristics.rs");
include!("lower_pipeline.rs");
include!("lower_groups.rs");
include!("lower_mechanics.rs");
include!("lower_operand_pipeline.rs");
include!("lower_program_pipeline.rs");
include!("lower_selector_pipeline.rs");
include!("lower_number_pipeline.rs");

include!("lower_paid.rs");
include!("lower_this_way.rs");
include!("lower_named.rs");
include!("lower_reflexive.rs");
include!("lower_reveal.rs");
include!("lower_replace.rs");
