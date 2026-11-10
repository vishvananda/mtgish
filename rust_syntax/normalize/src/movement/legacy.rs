//! Compatibility-only Source and historical arrival payloads. These are not
//! current portable IR types; current operations carry destination-specific
//! arrivals. The backend projects to this vocabulary for its existing ABI.
use crate::ir::*;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EntrySpec {
    /// Overrides the action's ordinary entering controller. This is separate
    /// from the owner of a created card and from the player making a choice.
    #[serde(default)]
    pub controller: Option<EntryController>,
    pub tapped: bool,
    pub counters: Vec<(crate::counter::CounterKind, Expr)>,
    pub states: Vec<EntryState>,
}

impl EntrySpec {
    /// The common arrival supported by a plain tapped/untapped engine call.
    pub fn simple_tapped(&self) -> Option<bool> {
        (self.controller.is_none() && self.counters.is_empty() && self.states.is_empty())
            .then_some(self.tapped)
    }
}

/// What an object is as it enters, beyond tapped and counters.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EntryState {
    Transformed,
    /// Enters prepared, as `KeywordAction::Prepare` does to a permanent.
    Prepared,
    /// Enters as a copy of one of these, with copy-time exceptions. Additional
    /// arrival instructions remain separate `EntryState` values (707.9e).
    CopyOf {
        objects: ObjectSet,
        exceptions: Vec<CopyException>,
        until: Option<Duration>,
    },
    /// Attacking but never declared (508.4). The entering side chooses the defender
    /// unless one is given.
    Attacking {
        player: Option<PlayerRef>,
        or_planeswalker: bool,
    },
    Blocking(ObjectRef),
    AttachedTo(ObjectRef),
    AttachedToChosen(ObjectFilter),
    AttachedToPlayer(PlayerRef),
    Changes(Vec<Change>),
    ChangesUntil {
        changes: Vec<Change>,
        duration: Duration,
    },
    /// Applies only to an object matching the filter as it enters.
    When {
        filter: ObjectFilter,
        entry: Box<EntrySpec>,
    },
    FaceDown,
    Flipped,
    /// One exile pile with whatever else this action exiles; shuffled, nobody
    /// can tell which card came from where.
    ExilePile {
        shuffled: bool,
    },
    /// One set of changes, chosen as it enters.
    ChooseChanges(Vec<Vec<Change>>),
    /// Unresolved source material for diagnostics. Never valid semantic IR.
    Draft {
        name: String,
        operands: Vec<Operand>,
    },
    /// A counter, or this many, of the entering side's choice.
    CountersOfChoice {
        kinds: Vec<crate::counter::CounterKind>,
        amount: Expr,
        distinct: bool,
    },
    /// The counters this object has, or one of each kind.
    CountersOf {
        object: ObjectRef,
        one_of_each: bool,
    },
    FaceDownAs(FaceDownAs),
    FaceDownWith {
        face_down: FaceDownAs,
        abilities: Vec<Ability>,
        /// The card whose name is noted as this enters.
        noted: Option<ObjectRef>,
    },
    FaceDownLand(crate::mtg_types::LandType),
    NotAnAura,
    /// The counters noted when the object was exiled.
    NotedCounters,
    Perpetual(Vec<Change>),
    /// Apply these arrival changes if the stated player chooses. The creating
    /// ability's controller is distinct from an entering-controller override.
    Optional {
        decider: PlayerRef,
        entry: Box<EntrySpec>,
    },
    /// Stays exiled only for this long.
    ExiledFor(Duration),
    ExiledForWith {
        until: Duration,
        /// Tested as each permanent returns, under that permanent's own
        /// battlefield-entry event rather than the creating ability's event.
        objects: ObjectSet,
        /// A program capturing the creator's bindings. EventSubject is the
        /// returning permanent; the creator's damage/other-event roles are
        /// unavailable. Local results remain private to this return trigger.
        body: Program,
    },
    /// Capture exact exiled incarnations when these objects are created. Each
    /// original token, including multiplicity replacements, gets the same link.
    /// This arrival history is not copiable TokenRecipe/CopyException data.
    LinkExiledCards(ObjectSet),
}

/// Upgrade an already decoded historical movement fragment after its nested
/// payloads have been upgraded. Saved cards normally use `migrate::read_json`;
/// this boundary is also useful for old typed producer snapshots without a card.
pub fn upgrade_fields(owner: &str, value: &mut serde_json::Value) -> Result<(), String> {
    crate::migrate::upgrade_authoring_movement(owner, value).map(|_| ())
}
