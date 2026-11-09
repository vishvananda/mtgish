//! Actions in the normalized semantic model.
use super::*;

/// One thing to do for a kind of counter, in `Operation::EachCounterKind`.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum CounterStep {
    /// Put a counter of that kind on this object.
    PutThatKind(ObjectRef),
    /// Put a counter of this other kind on this object.
    Put(crate::counter::CounterKind, ObjectRef),
    /// Remove a counter of that kind from this object.
    RemoveThatKind(ObjectRef),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ExchangeKind {
    Ownership,
    Control,
    LifeTotals,
    LifeWithPower,
    LifeWithToughness,
    TextBoxes,
    Power,
    Ante,
    Zones(EventZone, EventZone),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum CombatChange {
    Reselect {
        objects: ObjectSet,
        players_only: bool,
        optional: bool,
    },
    AssignAttack {
        objects: ObjectSet,
        to: Box<Operand>,
    },
    Block {
        blocker: ObjectRef,
        attackers: ObjectSet,
    },
    SwapBlocks {
        first: ObjectRef,
        second: ObjectRef,
        attacking: bool,
    },
    /// During the phase `during` added, otherwise this turn.
    OnlyAttackers {
        objects: ObjectSet,
        during: Option<ResultId>,
    },
    OnlyBlockers(ObjectSet),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum MovedCounters {
    One,
    All,
    AnyNumber,
    Exactly(Expr),
    /// One of each kind the recipient doesn't have.
    OneOfEachKindMissing,
    AtLeast(Expr),
}

/// What a choice of a value picks, as Forge's `ChooseColor`, `ChooseType`,
/// `NameCard` and `ChooseNumber`.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum SpellSubject {
    One(ObjectRef),
    Next {
        spells: ObjectFilter,
        player: PlayerRef,
        this_turn: bool,
        from_hand: bool,
    },
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum ValueKind {
    Color,
    ColorOrColorless,
    CreatureType,
    LandType,
    BasicLandType,
    CardType,
    CardName,
    Number,
    Parity,
    Direction,
    Word,
    Player,
    Ability,
    ColorWord,
    PlaneswalkerType,
    Recipient,
    /// One of the filters listed as options.
    Filter,
    Object,
    PermanentType,
    NonbasicLandType,
    Letter,
    /// One of the programs listed as options.
    Action,
    /// One of a fixed list of pairs of a color and a creature type.
    ColorAndCreatureType,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum SkippedPart {
    Turn,
    UntapStep,
    DrawStep,
    CombatPhase,
    CombatPhaseThisTurn,
    AllCombatPhasesNextTurn,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum AddedPhases {
    Combat,
    CombatAndMain,
    Beginning,
    Upkeep,
    End,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum AsCastAction {
    Revealed(ObjectFilter),
    Controlled(ObjectSet),
    Chose(ObjectSet),
}

/// What a player noted during the draft for a card in their pool.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct DraftNote {
    pub kind: DraftNoteKind,
    pub player: PlayerRef,
    pub card: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum DraftNoteKind {
    Name,
    CreatureType,
    Color,
    Exiled,
    Number,
    Player,
    Removed,
    Guessed,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum CounterChoice {
    /// One of these, chosen for each object.
    OneOf(Vec<crate::counter::CounterKind>),
    /// One of these the object lacks, at random.
    RandomMissing(Vec<crate::counter::CounterKind>),
    OnePerColor(crate::counter::CounterKind),
    /// One keyword counter for each such ability.
    PerAbility {
        of: Option<ObjectRef>,
        kinds: Vec<Operand>,
        missing_only: bool,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ActivationPayment {
    Life,
    Sacrifice,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ReflexiveWhen {
    YouDo,
    ExcessDamage(Option<ObjectRef>),
    Revealed(ObjectFilter),
    Regenerates,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum NoteKind {
    Number(Expr),
    Player(PlayerRef),
    LifeTotal,
    CountersOn(ObjectRef),
    /// Spent on this activation: its type, and how much if `amount`.
    ManaSpent {
        amount: bool,
    },
    /// Numbers these in order starting from zero.
    Numbered(ObjectSet),
    /// The most common creature type in these libraries.
    MostCommonCreatureType(PlayerSet),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum HandPick {
    TheyPlayIt,
    TopOfLibrary,
    CastFree,
    Discard,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum LandTypeScope {
    All,
    Basic,
    Nonbasic,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct SpreeMode {
    pub cost: Cost,
    pub ability: Box<Ability>,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum NewTargets {
    Same,
    /// "You may choose new targets."
    May,
    /// "Change the target of": it changes if there is another legal one.
    Must,
    /// To the object or objects in this set.
    To(ObjectSet),
    /// One copy for each of these, targeting it.
    Each(ObjectSet),
    ToPlayers(PlayerSet),
    /// At random among legal targets, excepting these.
    Random(Option<Box<Operand>>),
    /// One copy for each legal target.
    EachLegal,
    /// One copy for each player, targeting them or something they control.
    EachPlayer(PlayerSet),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ConjuredCard {
    Named(String),
    /// A duplicate of each of these, with any perpetual changes.
    Duplicate(ObjectSet),
    Spellbook {
        book: String,
        pick: SpellbookPick,
    },
    /// A card the rules pick at random from all that match.
    Random(Box<Operand>),
    /// One of these, chosen.
    OneOf(Vec<String>),
    Each(Vec<String>),
    /// A card from the spellbook that matches.
    SpellbookMatching {
        book: String,
        cards: Box<Operand>,
    },
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum SpellbookPick {
    Choice,
    Random,
    Each,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ConjureSpot {
    Shuffled,
    Top,
    /// At random among the top this many cards.
    AmongTop(Expr),
    FromTop(Expr),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum TurnSection {
    Untap,
    Upkeep,
    Draw,
    Main,
    Combat,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum VisibilityAudience {
    AffectedPlayer,
    Everyone,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum Ballot {
    Words(Vec<String>),
    Players(PlayerSet),
    Objects(ObjectSet),
    /// Votes for up to one of these, so a voter may abstain.
    UpToOne(ObjectSet),
    GraveyardCards(ObjectFilter),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum VoteOption {
    Word(String),
    Player(PlayerRef),
    Object(ObjectRef),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ManifestSource {
    TopOfLibrary { player: PlayerRef, count: Expr },
    Cards(ObjectSet),
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum PlayerPairAction {
    Heist,
    ControlNextTurn,
    ExchangeLife,
    ControlNextCombat,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum CounterCopies {
    All,
    OneOfEachKind,
    One,
    /// One counter of a kind the recipient lacks.
    OneMissing,
    OneOfEachKindDistributed,
    /// One counter, if the controller wants it.
    UpToOne,
}
