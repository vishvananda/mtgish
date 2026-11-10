//! Events in the normalized semantic model.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ReplacementActive {
    Battlefield,
    /// CR 113.6h/614.12: only the source's own entry is affected, using its
    /// prospective battlefield characteristics even before it is on the field.
    SourceEntering,
    /// "From anywhere": the source's own replacement works in every zone (113.6).
    Everywhere,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ReplacementEvent {
    Enter {
        objects: ObjectFilter,
    },
    /// Damage about to be dealt (CR 615). `grouping` stays per source/recipient
    /// pair, because prevention applies to each such damage event.
    Damage(DamageEvent),
    Either(Vec<ReplacementEvent>),
    /// An object about to change zones, matched as `EventSpec::ZoneChange`.
    ZoneChange {
        objects: ObjectFilter,
        from: Option<EventZone>,
        to: EventZone,
    },
    /// One rules-defined event from a reviewed row, as for `EventSpec::Primitive`.
    Primitive {
        name: String,
        operands: Vec<Operand>,
    },
    /// Applies once for each 1 of the pending amount, as "for each 1 damage".
    EachPoint(Box<ReplacementEvent>),
    /// `event`, when its pending amount compares this way.
    Measured {
        event: Box<ReplacementEvent>,
        comparison: Comparison,
        amount: Expr,
    },
    /// A player about to do something, as Forge's `Event$ Draw | ValidPlayer$`.
    Player {
        event: ReplacedPlayerEvent,
        players: PlayerSet,
    },
    /// Something about to happen to an object.
    Object {
        event: ReplacedObjectEvent,
        objects: ObjectFilter,
    },
    /// Tokens about to be created. `effect_only` leaves out a rule or a cost,
    /// and `each` applies to one token at a time rather than all of them.
    Tokens {
        tokens: ObjectFilter,
        controllers: Option<PlayerSet>,
        creators: Option<PlayerSet>,
        effect_only: bool,
        each: bool,
    },
    /// Counters about to be put on `objects`, or on `players` when named.
    Counters {
        kind: Option<crate::counter::CounterKind>,
        objects: ObjectFilter,
        players: Option<PlayerSet>,
        by: Option<PlayerSet>,
        effect_only: bool,
    },
    /// Tapping one of `sources` for mana, by `by` when that is named.
    ProduceMana {
        sources: ObjectFilter,
        by: Option<PlayerRef>,
    },
    /// The first such event each turn, as for `EventSpec::FirstEachTurn`.
    FirstEachTurn(Box<ReplacementEvent>),
    /// A discard about to happen, as Forge's `Event$ Discard | ValidSource$`.
    Discard {
        players: PlayerSet,
        causes: Option<ObjectFilter>,
        card: Option<ObjectFilter>,
        effect_only: bool,
    },
    CopySpell {
        players: PlayerSet,
        spells: ObjectFilter,
    },
    Countered {
        spells: ObjectFilter,
        by: ObjectFilter,
    },
    DrawCausedBy {
        players: PlayerSet,
        abilities: ObjectFilter,
    },
    ProduceColor {
        sources: ObjectFilter,
        color: ManaColor,
    },
    /// `event`, when a matching spell or ability would cause it: Forge's `ValidCause$`.
    Caused {
        cause: ObjectFilter,
        event: Box<ReplacementEvent>,
    },
    /// One of `players` about to put a matching permanent onto the battlefield,
    /// which playing a land does. The player binding is that player.
    PutOntoBattlefield {
        players: PlayerSet,
        objects: ObjectFilter,
    },
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum ReplacedPlayerEvent {
    /// One card, so each card of a larger draw is a separate event (CR 121.2).
    Draw,
    /// All the cards of one instruction together.
    DrawCards,
    DrawExceptFirstInDrawStep,
    DrawInDrawStep,
    GainLife,
    LoseLife,
    LoseGame,
    GetEnergy,
    Mill,
    Scry,
    Learn,
    Proliferate,
    BeginExtraTurn,
    RollDice,
    RollD6,
    Planeswalk,
    BeginTurn,
    BeginDrawStep,
    FlipCoin,
    RollPlanarDie,
    SearchLibrary,
    PlaneswalkByDie,
    SetSchemeInMotion,
    PayLife,
    /// Gaining life from a spell or ability, for its controller.
    GainLifeByEffect,
    /// A life total about to go down by any means, as Forge's `LifeReduced`. Its
    /// amount for `Measured` is the total it would become, not the change.
    ReduceLifeTotal,
    /// The step's turn-based untap (CR 502.3), whatever it untaps.
    UntapPermanents,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum ReplacedObjectEvent {
    Destroyed,
    LeavesBattlefield,
    UntapsInUntapStep,
    Connives,
    Adapts,
    Explores,
    BecomesTapped,
    AssignsCombatDamage,
}

/// How long a created replacement effect keeps applying within its duration.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ReplacementLimit {
    UntilExpired,
    /// Used up by the first event it applies to.
    NextOccurrence,
    /// A shield: used up once this much damage has been replaced (CR 615.7).
    TotalDamage(Expr),
    /// A shield on each recipient: used up once it has replaced this much.
    DamageToEach(Expr),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum DiceIgnore {
    Lowest,
    ChosenBy(PlayerRef),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum EventChange {
    SetTapped(bool),
    AddCounters {
        kind: crate::counter::CounterKind,
        amount: Expr,
    },
    SetController(PlayerRef),
    PreventDamage,
    /// Prevents up to this much of the pending damage.
    PreventSomeDamage(Expr),
    /// Prevents all but this much of the pending damage.
    PreventAllButDamage(Expr),
    /// The event does not happen. Unlike `PreventDamage` this is not prevention,
    /// so it applies to damage that can't be prevented.
    Cancel,
    /// The event's amount: damage, counters, life or mana.
    SetAmount(Expr),
    RedirectDamage(Recipients),
    /// Only this much of the damage goes there, as "the next 1 damage".
    RedirectSomeDamage(Expr, Recipients),
    SetDestination(Destination),
    /// How the entering object enters.
    Entry(BattlefieldState),
    AddMana(ManaProduction),
    /// Each mana produced is replaced, so an amount here is per mana: a land
    /// that makes two still makes two.
    ReplaceMana(ManaProduction),
    /// Only colored mana is replaced; colorless mana is produced as it was.
    ReplaceColoredMana(ManaProduction),
    /// `copies` of the tokens the event would create, then `tokens` besides.
    ReplaceTokens {
        copies: u32,
        tokens: Vec<TokenSpec>,
    },
    /// Roll one more die than the event would, and ignore one of the results.
    RollExtraDie(DiceIgnore),
    SearchTop(Expr),
    /// The event happens this many times instead.
    Repeats(Expr),
    AddCopy {
        new_targets: bool,
    },
    FlipExtraCoin,
    DamageAsInfect,
    IgnoreCounters(crate::counter::CounterKind),
    /// The event happens, then this runs at once. A body otherwise completes first.
    Then(Program),
    /// The dice are rolled, then the decider may swap one result with the value
    /// of one of these properties of `object`, as Forge's `DicePTExchanges`.
    ExchangeDieResult {
        decider: PlayerRef,
        object: ObjectRef,
        properties: Vec<Property>,
    },
    /// Damage still happens, including lifelink and damage triggers. Only its
    /// resulting loyalty removal is capped to leave this many counters on the
    /// planeswalker recipient (or its existing count if smaller). This is not
    /// damage prevention, cancellation, or a separate counter-removal action.
    LeaveLoyalty(Expr),
}

/// Eligibility gates for a scheduled trigger, relative to its creation.
/// Once opened, a gate remains open even if a turn or step is skipped. It does
/// not mean "this one turn only"; cancellation is represented by `expires`.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum DelayedStart {
    Immediately,
    NextTurn,
    PlayersNextTurn(PlayerRef),
    /// The extra turn this result created.
    ExtraTurn(ResultId),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum EventSpec {
    /// A player activates an ability, as Forge's `AbilityCast` with `ValidSA$`.
    Activated {
        players: PlayerSet,
        abilities: ObjectFilter,
    },
    /// Card predicates use the pre-discard view. Source-only matches follow
    /// the source across this transition (CR 702.29c); they do not compare the
    /// hand object's identity with the new graveyard/exile object's identity.
    Discard {
        players: PlayerSet,
        cards: ObjectFilter,
        source_only: bool,
        kind: DiscardKind,
        grouping: DiscardGrouping,
    },
    /// Battlefield departures use the pre-departure view to match; ordinary
    /// entering events use the post-event view (CR 603.10). This also preserves
    /// the distinction from a card entering a graveyard "from anywhere."
    ZoneChange {
        objects: ObjectFilter,
        from: Option<EventZone>,
        to: EventZone,
    },
    /// One occurrence per matching object, not once per simultaneous batch.
    Object {
        event: ObjectEvent,
        objects: ObjectFilter,
    },
    /// Binds the spell and the player who cast/copied it, not its owner.
    Spell {
        event: SpellEvent,
        players: PlayerSet,
        spells: ObjectFilter,
    },
    /// The player binding is the player whose turn contains this point.
    Turn {
        point: TurnPoint,
        players: PlayerSet,
    },
    Damage(DamageEvent),
    /// A state trigger is checked after semantic instructions, not only events.
    State {
        condition: Condition,
    },
    /// One ability with alternative events, not multiple independent abilities.
    /// Only bindings available in every branch may be used by the ability body.
    Either(Vec<EventSpec>),
    /// Checked at the event only. This is distinct from an intervening-if
    /// condition, which is checked again when the ability resolves.
    Filtered {
        event: Box<EventSpec>,
        condition: Condition,
    },
    /// The source's lore counters reached this chapter number (CR 714.2).
    SagaChapter(u32),
    /// One rules-defined event from a reviewed row. The row's `binds` says which
    /// event references it supplies, so references resolve from data.
    Primitive {
        name: String,
        operands: Vec<Operand>,
    },
    /// The first such event each turn, counted whether or not this ability
    /// existed yet, unlike a once-each-turn limit.
    FirstEachTurn(Box<EventSpec>),
    /// Something one player does. `objects` is what it was done to, when the
    /// form names that. The player binding is the player who did it.
    Player {
        event: PlayerEvent,
        players: PlayerSet,
        objects: Option<ObjectFilter>,
    },
    /// The event that brings its player's count this turn to `comparison value`.
    NthEachTurn {
        event: Box<EventSpec>,
        comparison: Comparison,
        value: Expr,
    },
    /// One object acting on or with another; both are bound by the event.
    Pair {
        event: PairEvent,
        objects: ObjectFilter,
        others: ObjectFilter,
    },
    /// `by` says what targeted the object.
    BecomesTarget {
        objects: ObjectFilter,
        kind: TargetingKind,
        by: Box<Operand>,
    },
    /// One creature attacking, as Forge's `Attacks`. `by` is the attacking
    /// player, when the form names one.
    Attack {
        attackers: ObjectFilter,
        by: Option<PlayerSet>,
        defenders: AttackDefenders,
    },
    /// Everything `event` matches at the same time is one occurrence, as
    /// Forge's `AttackersDeclared` and `ChangesZoneAll`. `count` is how many it
    /// takes, and `including` is one object that has to be among them.
    Batch {
        event: Box<EventSpec>,
        per: BatchPer,
        count: Option<(Comparison, Expr)>,
        including: Option<ObjectRef>,
    },
    /// Counters changing on one object, as Forge's `CounterAdded`. `by` is the
    /// player doing it, when the form names one; no `kind` means any counter.
    Counters {
        change: CounterChange,
        kind: Option<crate::counter::CounterKind>,
        objects: ObjectFilter,
        by: Option<PlayerSet>,
    },
    /// A Class gained this level (CR 716).
    ClassLevel {
        objects: ObjectFilter,
        level: Expr,
    },
    /// The first such event for each object each turn, unlike `FirstEachTurn`,
    /// which is the first for any object.
    FirstForObjectEachTurn(Box<EventSpec>),
    /// The first such event of the game.
    FirstEachGame(Box<EventSpec>),
    /// This event, when this spell or ability caused it.
    Caused {
        cause: ObjectFilter,
        event: Box<EventSpec>,
    },
    ZoneChangeFromElsewhere {
        objects: ObjectFilter,
        except: Vec<EventZone>,
        to: EventZone,
    },
    /// The counter that brings a permanent to `nth` of them.
    NthCounter {
        kind: Box<Operand>,
        objects: ObjectFilter,
        nth: Expr,
    },
    /// Attackers no creature blocked, as blocks are declared.
    AttackUnblocked {
        attackers: ObjectFilter,
        defenders: AttackDefenders,
        count: Option<(Comparison, Expr)>,
    },
    /// Every one of these attacks.
    AllAttack(ObjectFilter),
    /// Forge's `SpellAbilityCast` for what goes on the stack without casting.
    OntoStack(ObjectFilter),
    /// This spell or ability resolves.
    Resolves(ObjectFilter),
    /// `cause` makes an ability trigger: Panharmonicon's "causes". `own_ability`
    /// means it is an ability of the object the event is about.
    CausesTrigger {
        cause: Box<EventSpec>,
        own_ability: bool,
    },
    /// An object explored and revealed one of these.
    ExploresCard {
        objects: ObjectFilter,
        cards: ObjectFilter,
    },
    TappedForManaOf {
        objects: ObjectFilter,
        by: Option<PlayerSet>,
        mana: Box<Operand>,
    },
    ManaAdded {
        abilities: ObjectFilter,
        players: PlayerSet,
        mana: Box<Operand>,
    },
    /// Forge's `BecomesTargetOnce` from the side of the one choosing.
    TargetsChosen {
        players: PlayerSet,
        objects: ObjectFilter,
    },
    ControlChanges {
        objects: ObjectFilter,
        to: PlayerSet,
        from: PlayerSet,
    },
    CountersOnPlayer {
        by: PlayerSet,
        players: PlayerSet,
    },
    /// Attacks along with others.
    AttacksWith {
        object: ObjectRef,
        others: ObjectFilter,
        count: (Comparison, Expr),
        by: Option<PlayerSet>,
        different_defenders: bool,
    },
    /// Attackers that together pass a group test.
    AttacksAsGroup {
        by: PlayerSet,
        attackers: ObjectFilter,
        group: Box<Operand>,
    },
    SpellTargeting {
        players: PlayerSet,
        spells: ObjectFilter,
        targets: Option<ObjectSet>,
        count: Option<(Comparison, Expr)>,
    },
    SpellOrActivation {
        players: PlayerSet,
        spells: ObjectFilter,
        abilities: ObjectFilter,
    },
    PaysToActivate {
        players: PlayerSet,
        abilities: ObjectFilter,
        payment: ActivationPayment,
    },
    /// State trigger on objects: one comes to match this (603.8).
    ObjectBecomes(ObjectFilter),
    /// `event`, during the activation of one of `abilities` by one of `players`,
    /// as Forge's `WhileKeyword$`. The player binding is the activating player.
    WhileActivating {
        event: Box<EventSpec>,
        players: PlayerSet,
        abilities: ObjectFilter,
    },
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum BatchPer {
    /// One occurrence for all of it.
    Whole,
    /// One occurrence for each player or planeswalker attacked.
    Defender,
    /// One occurrence for each object the event is about.
    Subject,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum CounterChange {
    Put,
    Removed,
    LastRemoved,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum ObjectEvent {
    /// Includes leaving the game with the object's owner (CR 603.6c), not
    /// just zone changes. The matcher uses the pre-departure view.
    LeavesBattlefield,
    Attacks,
    AttacksAlone,
    Blocks,
    BecomesBlocked,
    AttacksAndIsNotBlocked,
    BecomesTapped,
    BecomesUntapped,
    TurnsFaceUp,
    Transforms,
    BecomesMonstrous,
    Mutates,
    Specializes,
    TappedForMana,
    /// Matches on the view before the card left, as `LeavesBattlefield` does.
    LeavesGraveyard,
    Crewed,
    Explores,
    LeavesWithoutDying,
    PhasesIn,
    PhasesOut,
    Connives,
    Fights,
    BecomesRenowned,
    BecomesSaddled,
    Evolves,
    Devoured,
    Destroyed,
    Sacrificed,
    Trains,
    CumulativeUpkeepPaid,
    EchoPaid,
    TappedForTeamwork,
    FinalChapterResolves,
    FinalChapterTriggers,
    Countered,
    Plotted,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum ObjectState {
    Enchanted,
    Equipped,
    Saddled,
    Commander,
    Kicked,
    FaceDown,
    Monstrous,
    Renowned,
    Unblocked,
    Blocked,
    AttackingAlone,
    Bargained,
    Goaded,
    Paired,
    Suspected,
    Damaged,
    TributeNotPaid,
    CastDuringOwnMainPhase,
    HasX,
    Suspended,
    HasAdventure,
    BlockedThisTurn,
    HasSticker,
    PowerBelowToughness,
    Transformed,
    NoAbilities,
    /// Controlled continuously since the turn began (CR 302.6).
    ControlledSinceTurnBegan,
    HauntedBySource,
    Escaped,
    Warped,
    DoubleFaced,
    Prepared,
    PowerAboveBase,
    RingBearer,
    Harnessed,
    Worthy,
    PowerEqualsToughness,
    PowerDiffersFromBase,
    BlockingAlone,
    KickedTwice,
    Modal,
    HybridInCost,
    PhyrexianInCost,
    RepeatedManaSymbol,
    Nameless,
    NonBasicLandType,
    Foretold,
    Unearthed,
    Embalmed,
    DevouredACreature,
    StartedTurnUntapped,
    ControlledSinceBeforeCombat,
    ControlledSinceMostRecentTurn,
    FirstLandPlayedThisTurn,
    HadToAttack,
    CouldntAttack,
    MostVotes,
    ReceivedAVote,
    ManaSpentBelowManaValue,
    SnowManaOfOwnColorSpent,
    CostPaid(CastCost),
    Sticker(StickerKind),
    UnlockedDoor,
    Manifested,
    /// `CostPaid`, on a spell cast this turn: "if her sneak cost was paid this turn".
    CostPaidThisTurn(CastCost),
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum CastCost {
    Additional,
    Prowl,
    Sneak,
    Spectacle,
    Surge,
    Emerge,
    Madness,
    WebSlinging,
    Teamwork,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum StickerKind {
    Name,
    Art,
    PowerToughness,
    Ability,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum TurnWindow {
    BeforeAttackersDeclared,
    Combat,
    Upkeep,
    EndStep,
    BeforeCombatDamage,
    DeclareAttackers,
    DeclareBlockers,
    BeforeBlockersDeclared,
    FirstCombat,
    AfterBlockersDeclared,
    MainPhase,
    BeforeCombat,
    AfterCombat,
    EndOfCombatStep,
    BeforeEndOfCombatStep,
    BeforeEndStep,
    AfterUpkeep,
    BeforeFirstCombat,
    UntapStep,
    DrawStep,
    FirstEndStep,
    ExtraTurn,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum PlayerState {
    Monarch,
    CitysBlessing,
    FullParty,
    StartingPlayer,
    InTheirUpkeep,
    InTheirMainPhase,
    MaxSpeed,
    CompletedDungeon,
    Initiative,
    Descended,
    CommittedCrime,
    BeingAttacked,
    EnduringStory,
    Poisoned,
    ReceivedAVote,
    HasBoon,
    CouldMulligan,
    WholePoolInDeck,
    PossibleDefender,
    RolledHighest,
    ShuffledThisWay,
    /// No effect stops this player drawing cards.
    CanDraw,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum PairEvent {
    Blocks,
    BecomesBlockedBy,
    Crews,
    Saddles,
    BecomesAttachedTo,
    Exploits,
    Enlists,
    Mentors,
    BecomesUnattachedFrom,
    ChampionedWith,
    Stations,
    EntersAttachedTo,
    TransformsInto,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum TargetingKind {
    Spell,
    Ability,
    SpellOrAbility,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum PlayerEvent {
    GainsLife,
    LosesLife,
    Draws,
    Attacks,
    Scries,
    Surveils,
    CommitsCrime,
    SearchesLibrary,
    LosesGame,
    RollsDice,
    WinsCoinFlip,
    Sacrifices,
    PlaysLand,
    UnlocksDoor,
    TapsForMana,
    ChaosEnsues,
    SetsSchemeInMotion,
    Planeswalks,
    EncountersPhenomenon,
    FullyUnlocksRoom,
    Expends,
    /// A permanent entered under this player's control; `objects` is that
    /// permanent. The player binding is that controller, who did nothing.
    EntersUnderControl,
    /// As `EntersUnderControl`, for a land that wasn't played or a spell not cast.
    EntersUnplayed,
    DayNightChanges,
    FinishVoting,
    GameBegins,
    LosesControl,
    RingTempts,
    CreatesToken,
    Exerts,
    Proliferates,
    RevealsFirstCardDrawn,
    PlaysCardFromExile,
    PlaysLandFromExile,
    PutsOntoBattlefield,
    GetsEnergy,
    BecomesMonarch,
    CompletesDungeon,
    TapsPermanent,
    PlaysCard,
    DrawsRevealedCard,
    DrawsCard,
    Seeks,
    VisitsAttraction,
    RollsADie,
    RollsPlanarDie,
    Shuffles,
    Conjures,
    ConjuresOther,
    Discovers,
    Clashes,
    WinsClash,
    Investigates,
    CollectsEvidence,
    PutsSticker,
    PutsNameSticker,
    PutsAbilitySticker,
    PutsArtSticker,
    PlacesSticker,
    RollsToVisit,
    Bends,
    ChoosesRingBearer,
    SolvesCase,
    SelectsTargetsAtRandom,
    SelectsModeAtRandom,
    FlipsCoins,
    Forages,
    RollsNatural20,
    RollsHighestNatural,
    RollsPlanarBlank,
    Foretells,
    PaysLife,
    GivesGift,
    LosesCoinFlip,
    TurnsFaceUp,
    SacrificesForEmerge,
    PaysCumulativeUpkeep,
    DeclinesCumulativeUpkeep,
    MillsCard,
    Mills,
    ManifestsDread,
    FacesDilemma,
    OpensAttraction,
    ClaimsPrize,
    DiscardsAtRandom,
    PlaysLandNotFromHand,
    BottomsWhileScrying,
    PlaneswalksAway,
    /// `objects` is what targeted the player.
    BecomesTarget,
    UntapsDuringUntapStep,
    /// Guessed at something hidden or undecided because of a spell or ability.
    Guesses,
    /// Separated cards or permanents into piles.
    GroupsIntoPiles,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum TurnPoint {
    UpkeepBegins,
    DrawStepBegins,
    MainPhaseBegins,
    FirstMainPhaseBegins,
    SecondMainPhaseBegins,
    PostcombatMainPhaseBegins,
    CombatBegins,
    CombatEnds,
    EndStepBegins,
    CleanupStepBegins,
    DeclareAttackersStepBegins,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum StackTrait {
    Spell,
    Triggered,
    Activated,
    Mana,
    Loyalty,
    XInCost,
    TapSymbolInCost,
    SingleTarget,
    /// The ability a keyword gives: `Ninjutsu`, `Equip` and so on.
    Keyword(AbilityKeyword),
    DungeonRoom,
    OfEmblem,
    /// The ability reading this filter.
    ThisAbility,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum AbilityKeyword {
    Backup,
    Boast,
    Embalm,
    Equip,
    Eternalize,
    Exhaust,
    Modular,
    Ninjutsu,
    Outlast,
    PowerUp,
    Craft,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum SpellEvent {
    Cast,
    Copy,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum EventZone {
    Battlefield,
    Graveyard,
    Hand,
    Library,
    Exile,
    Stack,
    Command,
    Ante,
    /// Not a zone, but where a card outside the game is played from.
    Outside,
    /// Each player's own ordered deck of planes and phenomena (901.3).
    PlanarDeck,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum DiscardKind {
    Any,
    Cycling,
    /// Discarded because of a spell or ability that matches.
    CausedBy(ObjectFilter),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum DiscardGrouping {
    Card,
    Player,
}
