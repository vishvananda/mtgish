//! Movement in the normalized semantic model.
use super::*;

/// What a face-down permanent is, when not a plain 2/2.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct FaceDownAs {
    pub power_toughness: Box<Operand>,
    pub creature_type: Option<Box<Operand>>,
    pub artifact: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum CardOrder {
    Chosen,
    Preserve,
    Random,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct EntrySpec {
    pub tapped: bool,
    pub counters: Vec<(crate::counter::CounterKind, Expr)>,
    pub states: Vec<EntryState>,
}

/// Placement terms are independent of selecting the objects that will move.
/// The default retains the destination's own ordering and chooser rules.
#[derive(Clone, Debug, Default, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct MovementTerms {
    pub order: MovementOrder,
    pub placement: MovementPlacement,
    /// Qualifications over the entire moved set when no selection is stated.
    /// These do not implicitly ask a player to choose a subset.
    pub qualifiers: Vec<SelectionConstraint>,
}

impl MovementTerms {
    pub fn is_default(&self) -> bool {
        matches!(self.order, MovementOrder::DestinationDefault)
            && matches!(self.placement, MovementPlacement::DestinationDefault)
            && self.qualifiers.is_empty()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum MovementOrder {
    #[default]
    DestinationDefault,
    Random,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum MovementPlacement {
    #[default]
    DestinationDefault,
    /// A stated player chooses a library placement, rather than each owner.
    By(PlayerSet),
}

/// What an object is as it enters, beyond tapped and counters.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum EntryState {
    Transformed,
    /// Enters prepared, as `KeywordAction::Prepare` does to a permanent.
    Prepared,
    /// Enters as a copy of one of these, as Forge's `Clone`. `effects` are its exceptions.
    CopyOf {
        objects: ObjectSet,
        effects: Box<Operand>,
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
    /// A reviewed entry instruction with no reduction.
    Primitive {
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
        abilities: Vec<Operand>,
        /// The card whose name is noted as this enters.
        noted: Option<ObjectRef>,
    },
    FaceDownLand(Box<Operand>),
    NotAnAura,
    /// The counters noted when the object was exiled.
    NotedCounters,
    Perpetual(Vec<Change>),
    /// These entry flags apply if its controller chooses.
    May(Vec<Operand>),
    /// Stays exiled only for this long.
    ExiledFor(Duration),
    ExiledForWith {
        until: Duration,
        objects: ObjectSet,
        body: Box<Operand>,
    },
    RulesUntil {
        rules: Vec<Operand>,
        until: Duration,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum Destination {
    OwnersHand,
    OwnersGraveyard,
    OwnersLibraryTop,
    OwnersLibraryBottom,
    Exile,
    Battlefield(PlayerRef),
    OwnersBattlefield,
    /// "Shuffle it into its owner's library": the shuffle is one action.
    OwnersLibraryShuffled,
    /// The position counted from the top: third from the top is 3.
    OwnersLibraryFromTop(Expr),
    /// The bottom, in a random order.
    OwnersLibraryBottomRandom,
    /// Its owner's choice of the top or the bottom.
    OwnersLibraryTopOrBottom,
    Hand(PlayerRef),
    CommandZone,
    Ante,
    /// This end of its owner's planar deck (901.3).
    OwnersPlanarDeck(SequenceEnd),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum SequenceEnd {
    Top,
    Bottom,
}
