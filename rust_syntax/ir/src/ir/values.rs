//! Values in the normalized semantic model.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum EarlierObjects {
    It,
    Revealed,
    Exiled,
    Milled,
    Found,
    Devoured,
    PutInGraveyard,
    Sacrificed,
    LeftGraveyard,
    Unattached,
    Selected,
    Countered,
    Discarded,
    Webslung,
    Manifested,
    Targeted,
    Returned,
    Tapped,
    Shuffled,
    Entered,
    Evidence,
    SpellBecame,
    LastExiled,
    CopiedSpell,
    Created,
    LastCast,
    GainedControl,
    Conjured,
    LastDrawn,
    Sought,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ObjectMeasure {
    Count,
    Of(Property),
    Total(Property),
    Greatest(Property),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum DiceMeasure {
    Result,
    Nth(u32),
    Total,
    Different,
    MostEqual,
    Matching(Comparison, Box<Expr>),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum StickerMeasure {
    Names,
    NamesOfLength(Comparison, Box<Expr>),
    NamesBeginningWith(Box<Operand>),
    TotalPower,
    TotalToughness,
    Letter(Box<Operand>),
    UniqueVowels,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum NotedMeasure {
    Number,
    Highest,
    LifeTotal,
    CreatureTypes,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum GameValue {
    Stake,
    WinningBid,
    LifePaid,
    LifeLostThisWay,
    ManaPaidThisWay,
    PlayersWhoLost,
    ResolutionsThisTurn,
    ExcessDamageThisWay,
    ManaLostThisWay,
    EachValue,
    MostDiscardedThisWay,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum PriorOutcome {
    CostPaid,
    FreerunningPaid,
    MayhemPaid,
    NoLifePaid,
    NoVotes,
    NoSpellCast,
    WonClash,
    GuessedWrong,
    RevealedPutInHand,
    Unearthed,
    ArtSticker,
    TappedForAttacking,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(tag = "kind", content = "args")]
pub enum Expr {
    /// The number of counters a `RemoveCounters` actually removed.
    CountersRemoved(ResultId),
    /// How much that action really did: life lost, cards drawn, damage dealt.
    Performed(ResultId),
    /// The same number for a cost component that removed counters.
    PaidCountersRemoved {
        payment: PaymentRef,
        component: Vec<u32>,
    },
    /// The number a `BindValue` computed, fixed at that point in the resolution.
    BoundValue(ResultId),
    /// The part of this slot's distributed total assigned to the recipient at
    /// hand, so one operation over the slot gives each target its own amount.
    DistributedShare(TargetSlot),
    /// What the die rolled by this result showed.
    DieResult(ResultId),
    /// How often an event happened in a window, or the total of its amounts.
    EventHistory {
        event: Box<EventSpec>,
        window: HistoryWindow,
        measure: HistoryMeasure,
    },
    CountPlayers(PlayerSet),
    /// One reviewed rules-defined form; see `rewrites/primitives.json5`.
    Primitive {
        name: String,
        operands: Vec<Operand>,
    },
    /// Actual mana spent to cast this spell, or the spell that became this
    /// permanent (400.7d). Includes mana paid for additional costs, after
    /// reductions. Cost symbols, life payments, and an "as though" spending
    /// permission do not change actual mana types (609.4b). A spell copy has
    /// no mana spent on it; copying cost choices does not copy this payment.
    /// A later zone change does not inherit the old incarnation's cast data.
    ManaSpent {
        object: ObjectRef,
        filter: SpentManaFilter,
        aggregation: SpentManaAggregation,
    },
    /// Count mana-cost symbols containing any listed color, once per symbol
    /// even for a hybrid containing two listed colors (700.5). Two-brid and
    /// Phyrexian symbols count; generic, X, and colorless symbols do not.
    /// Object color changes, including devoid, do not change mana symbols.
    CountManaSymbols {
        objects: Box<ObjectSet>,
        colors: Vec<ManaColor>,
    },
    /// Aggregate current numeric properties. Empty inputs yield zero;
    /// maximum/minimum do not clamp negative nonempty inputs at zero.
    Aggregate {
        objects: Box<ObjectSet>,
        property: Property,
        aggregation: Aggregation,
    },
    /// One number about one player.
    PlayerNumber {
        player: PlayerRef,
        number: PlayerNumber,
    },
    /// The largest or smallest `number` among `players`, as Forge's
    /// `HighestLifeTotal`. Sum is not read here.
    AmongPlayers {
        players: PlayerSet,
        number: PlayerNumber,
        aggregation: Aggregation,
    },
    /// The most objects that share one value of `property`.
    LargestSharedGroup {
        objects: Box<ObjectSet>,
        property: DistinctProperty,
    },
    /// Mana from sources that match, spent on this object.
    ManaSpentFrom {
        object: ObjectRef,
        sources: Box<ObjectFilter>,
    },
    /// `value` read at an earlier moment.
    AsOf {
        moment: Moment,
        value: Box<Expr>,
    },
    Exponentiate(Box<Expr>, Box<Expr>),
    /// Turns taken in the game, this one included.
    TurnNumber,
    /// Count distinct values, rather than objects or subtype occurrences.
    CountDistinct {
        objects: Box<ObjectSet>,
        property: DistinctProperty,
    },
    /// X chosen for the activation that caused the matched cycling discard.
    EventActivationX,
    Constant(i64),
    AnnouncedX,
    /// X recorded on the spell that became this object, including copied
    /// choices; zero if it did not enter from a spell (CR 107.3m / 707.10).
    /// Entry replacements must not read an unrelated activation's announced X.
    CastX(ObjectRef),
    EventAmount,
    ObjectProperty {
        object: ObjectRef,
        property: Property,
        information: Information,
    },
    Add(Box<Expr>, Box<Expr>),
    Subtract(Box<Expr>, Box<Expr>),
    Multiply(Box<Expr>, Box<Expr>),
    Minimum(Box<Expr>, Box<Expr>),
    Maximum(Box<Expr>, Box<Expr>),
    Conditional {
        condition: Box<Condition>,
        then: Box<Expr>,
        otherwise: Box<Expr>,
    },
    Divide {
        numerator: Box<Expr>,
        denominator: Box<Expr>,
        rounding: Rounding,
    },
    CountObjects(Box<ObjectSet>),
    CountCounters {
        recipients: Box<Recipients>,
        kind: Option<crate::counter::CounterKind>,
        information: Information,
    },
    /// What an odd-or-even choice named: 0 for even, 1 for odd.
    ChosenParity,
    /// The number chosen by this ability or as this object entered.
    ChosenNumber,
    /// How many creatures are in this player's party (700.8).
    PartySize(PlayerRef),
    StartingLife(PlayerRef),
    TimesKicked(ObjectRef),
    TimesResolvedThisTurn,
    /// Damage past lethal dealt by this earlier instruction to these objects.
    ExcessDamage {
        result: ResultId,
        to: Box<ObjectFilter>,
    },
    /// How many times this ability has been activated this turn.
    TimesActivatedThisTurn,
    /// One half of the adjustment `ChoosePowerToughness` picked.
    ChosenPowerToughness {
        result: ResultId,
        toughness: bool,
    },
    /// One of the dice a roll kept, in the order the roller chose.
    DieResultAt {
        result: ResultId,
        index: u32,
    },
    /// Votes cast for this option in this resolution.
    Votes(VoteOption),
    TimesMutated(ObjectRef),
    /// Any of these values, for a comparison against a listed set.
    AnyOf(Vec<Expr>),
    /// How many of the coins a `FlipCoins` flipped came up heads.
    HeadsFlipped(ResultId),
    /// A measure of what an earlier instruction or cost acted on, as Forge's
    /// `Remembered$` and `Sacrificed$` counts.
    Earlier {
        what: EarlierObjects,
        measure: ObjectMeasure,
        result: Option<ResultId>,
        filter: Option<Box<ObjectFilter>>,
        by: Option<PlayerRef>,
    },
    Dice(DiceMeasure),
    /// Coin flips won, or lost.
    CoinFlipsWon(bool),
    Stickers {
        objects: Box<ObjectSet>,
        measure: StickerMeasure,
    },
    /// A value noted earlier, on this object or not.
    Noted {
        object: Option<ObjectRef>,
        measure: NotedMeasure,
    },
    Game(GameValue),
    Difference(Box<Expr>, Box<Expr>),
    DamageBy(ObjectRef),
    AdditionalCostPaid(Vec<Operand>),
    DraftNoted {
        note: DraftNote,
        highest: bool,
    },
    ModesChosen(ObjectRef),
    TurnsSinceForetold,
    ChosenColorsOf(ObjectRef),
    AbilitiesAmong {
        abilities: Box<Operand>,
        objects: Box<ObjectSet>,
    },
    /// Mana other than what paid for a whole spell, in `filter`.
    ManaOf {
        mana: ManaOccurrence,
        filter: SpentManaFilter,
    },
    /// Counters removed to pay for this spell or ability.
    CountersPaid {
        object: ObjectRef,
        kind: crate::counter::CounterKind,
    },
    /// How many values of `property` this object has in common with any of `with`.
    SharedValues {
        object: ObjectRef,
        property: DistinctProperty,
        with: Box<ObjectSet>,
    },
    /// How many of the payments a `PlayersPayRepeatedly` offered this player
    /// they left unpaid.
    RepeatedCostsUnpaid {
        result: ResultId,
        player: PlayerRef,
    },
    /// Mana symbols of these colors in the activation cost being changed, counted
    /// as `CountManaSymbols` counts a mana cost.
    ActivationCostSymbols(Vec<ManaColor>),
    /// The damage of this instruction, counted only up to each recipient's life,
    /// loyalty or toughness before it was dealt.
    ClampedDamage(ResultId),
    /// What every run of a `ForEachPlayer` did, added up. `Performed` read by
    /// another instruction over the players is each player's own.
    PerformedTotal(ResultId),
    /// How many of `objects` have `property` in common with another of them.
    SharingWithAnother {
        objects: Box<ObjectSet>,
        property: DistinctProperty,
    },
    /// How many of the cards a surveil looked at it left on top, as it finished:
    /// drawing them afterwards doesn't change the number.
    SurveilKeptOnTop(ResultId),
}

/// Mana a number or filter reads that did not pay for a whole spell.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ManaOccurrence {
    /// Spent on the {X} of this spell or activation.
    SpentOnX,
    /// Spent on the whole cost of this activation.
    SpentOnActivation,
    /// Spent on the cost whose payment the matched event reports.
    SpentOnEventCost,
    /// What the matched event's mana ability produced.
    ProducedByEvent,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct SpentManaFilter {
    /// None matches all six types. Some restricts to the listed actual types.
    pub colors: Option<Vec<ManaColor>>,
    /// A unit produced by a snow source, not a seventh mana type.
    pub snow: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum HistoryWindow {
    ThisTurn,
    LastTurn,
    ThisCombat,
    ThisGame,
    SinceLastTurnOf(PlayerRef),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum Moment {
    ThisSpellCast,
    TurnBegan,
    Cast(ObjectRef),
    /// The last time this event happened in the window.
    LastEvent {
        event: Box<EventSpec>,
        window: HistoryWindow,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum HistoryMeasure {
    Occurrences,
    TotalAmount,
    /// How many different sources did it.
    DistinctSources,
    /// A property of the objects that matched, as Forge's `Count$ValidStack`.
    Property(Property, Aggregation),
    Distinct(DistinctProperty),
    MostFromOneSource,
    /// How many different spells and abilities caused it, read through
    /// `EventSpec::Caused`. Two abilities of one source are two causes.
    DistinctCauses,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum SpentManaAggregation {
    Count,
    /// Count distinct actual colors among matching units; colorless is not a color.
    DistinctColors,
    /// Largest count of one actual color; excludes colorless and yields zero
    /// for an empty set of colored units.
    MaximumPerColor,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum Aggregation {
    Sum,
    Maximum,
    Minimum,
    /// Any one of the values, for a comparison against each of them.
    AnyOf,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum DistinctProperty {
    Powers,
    CardTypes,
    CreatureTypes,
    Colors,
    Names,
    /// Plains, Island, Swamp, Mountain, Forest; excludes other land subtypes.
    BasicLandTypes,
    /// All land subtypes, including nonbasic types; distinct from domain.
    LandTypes,
    /// Selected from a single graveyard: a card there is its owner's.
    Owners,
    Controllers,
    Toughness,
    ManaValues,
    PlaneswalkerTypes,
    CounterTypes,
    PermanentTypes,
    ManaCosts,
    ColorPairs,
    /// The names of unlocked doors, each face of a Room having its own.
    UnlockedDoorNames,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum Rounding {
    Down,
    Up,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum Comparison {
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    SameParity,
    DifferentParity,
}

impl Expr {
    /// A player's life is a signed player quantity, not a nonnegative count.
    pub fn player_life(player: PlayerRef) -> Self {
        Self::PlayerNumber {
            player,
            number: PlayerNumber::Life,
        }
    }

    /// Fold only constants, using checked arithmetic. Dynamic reads stay at the
    /// same semantic point; overflow keeps the expression for a backend to judge.
    pub fn add(a: Self, b: Self) -> Self {
        if matches!(a, Self::Constant(0)) {
            return b;
        }
        if matches!(b, Self::Constant(0)) {
            return a;
        }
        if let (Self::Constant(a), Self::Constant(b)) = (&a, &b) {
            if let Some(value) = a.checked_add(*b) {
                return Self::Constant(value);
            }
        }
        Self::Add(Box::new(a), Box::new(b))
    }
    pub fn multiply(a: Self, b: Self) -> Self {
        if matches!(a, Self::Constant(1)) {
            return b;
        }
        if matches!(b, Self::Constant(1)) {
            return a;
        }
        if let (Self::Constant(a), Self::Constant(b)) = (&a, &b) {
            if let Some(value) = a.checked_mul(*b) {
                return Self::Constant(value);
            }
        }
        Self::Multiply(Box::new(a), Box::new(b))
    }
}
