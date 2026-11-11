//! Conditions in the normalized semantic model.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(tag = "kind", content = "args")]
pub enum Condition {
    /// One reviewed rules-defined form; see `rewrites/primitives.json5`.
    Primitive {
        name: String,
        operands: Vec<Operand>,
    },
    Searched(ResultId),
    TargetsMatch {
        object: ObjectRef,
        kind: ObjectKind,
        filter: ObjectFilter,
    },
    /// Original casting zone, not current membership of the stack object.
    WasCastFrom {
        object: ObjectRef,
        zone: EventZone,
    },
    ExistsPlayer {
        players: PlayerSet,
        binding: LoopBinding,
        condition: Box<Condition>,
    },
    /// CR 118.12: the optional cost was chosen, or the mandatory cost was
    /// started. Replacement of the payment's events does not turn this false.
    PaymentChosen(ResultId),
    PlayerIsActive(PlayerRef),
    AllOf(Vec<Condition>),
    AnyOf(Vec<Condition>),
    Not(Box<Condition>),
    ObjectMatches {
        object: ObjectRef,
        filter: ObjectFilter,
    },
    Controls {
        player: PlayerRef,
        filter: ObjectFilter,
        minimum: u32,
    },
    Compare {
        left: Expr,
        test: NumberTest,
    },
    /// How the flip that made `result` came out.
    CoinFlip {
        result: ResultId,
        outcome: CoinOutcome,
    },
    /// This player's run of a `ForEachPlayer` body reached this instruction.
    PlayerTook {
        player: PlayerRef,
        result: ResultId,
    },
    /// True during this part of the turn, whoever's turn it is.
    Window(TurnWindow),
    PlayerState {
        player: PlayerRef,
        state: PlayerState,
    },
    /// The chosen word of an anchor word ability (CR 614.12c).
    ChosenWord(String),
    GiftPromised(bool),
    /// This option has the most votes, or is tied for it when `tied` counts.
    MostVotes {
        option: VoteOption,
        tied: bool,
    },
    /// This player voted for this option.
    VotedFor {
        player: PlayerRef,
        option: VoteOption,
    },
    /// Anyone voted for this option.
    Voted(VoteOption),
    ItsNeitherDayNorNight,
    /// CR 810: this player is on `other`'s team.
    SameTeam {
        player: PlayerRef,
        other: PlayerRef,
    },
    ItsNight,
    /// This player is attacking that one.
    PlayerAttacks {
        attacker: PlayerRef,
        defender: PlayerRef,
    },
    /// Some subset of these meets the constraints and has this many objects.
    SubsetMeets {
        objects: ObjectSet,
        comparison: Comparison,
        amount: Expr,
        constraints: Vec<SelectionConstraint>,
    },
    /// Whether this happened for the player in the window.
    PlayerDid {
        player: PlayerRef,
        event: PlayerHistory,
        window: ObjectWindow,
    },
    /// A choice of this kind has been made.
    ValueChosen(ValueKind),
    Outcome(PriorOutcome),
    AllVotedFor(VoteOption),
    MostCommonColor {
        color: Box<Operand>,
        objects: ObjectSet,
        tied: bool,
    },
    /// In this graveyard, with this many cards above it.
    GraveyardPosition {
        card: ObjectRef,
        player: PlayerRef,
        above: ObjectFilter,
        comparison: Comparison,
        amount: Expr,
    },
    /// What a player showed as this spell was cast.
    AsCast {
        player: PlayerRef,
        action: AsCastAction,
    },
    PlayerIn {
        player: PlayerRef,
        players: PlayerSet,
    },
    /// This player is the one chosen.
    ValueChosenIs(PlayerRef),
    ChoseExtreme {
        player: PlayerRef,
        highest: bool,
    },
    /// Among what each player did, this player's card had the extreme mana value.
    HadExtreme {
        player: PlayerRef,
        what: EarlierObjects,
        filter: Option<ObjectFilter>,
        highest: bool,
    },
    VotedAs {
        player: PlayerRef,
        other: PlayerRef,
        same: bool,
    },
    /// The coin for this player, or this object, came up so.
    CoinCameUp {
        player: Option<PlayerRef>,
        outcome: CoinOutcome,
    },
    /// It has this many targets among these.
    TargetCount {
        object: ObjectRef,
        filter: ObjectFilter,
        comparison: Comparison,
        value: Expr,
    },
    /// The Nth object the player chose this way.
    ChoseNth {
        player: PlayerRef,
        nth: u32,
    },
    IsPrime(Expr),
    WonSubgame {
        player: PlayerRef,
        result: ResultId,
    },
    CompletedDungeon {
        player: PlayerRef,
        name: String,
    },
    NotedForDraft {
        player: PlayerRef,
        note: DraftNote,
    },
    /// This draw is the first this player makes in their own draw step, whatever
    /// they drew earlier in the turn.
    FirstDrawInOwnDrawStep(PlayerRef),
    /// A creature was actually chosen during this matched Ring temptation and
    /// satisfied this filter at the time of that choice. Re-checked against the
    /// recorded choice fact, never the player's current Ring-bearer or its live
    /// characteristics. False when no creature was chosen.
    RingBearerChosen(ObjectFilter),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum CoinOutcome {
    Won,
    Lost,
    Heads,
    Tails,
}
