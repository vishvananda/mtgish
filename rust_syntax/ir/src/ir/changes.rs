//! Changes in the normalized semantic model.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum CardTypeDomain {
    All,
    Permanent,
    AmongObjects(ObjectSet),
    /// Types observed in matching casts, including objects that have since
    /// moved. This is not a filter over the currently live battlefield.
    AmongCasts {
        by: PlayerSet,
        filter: ObjectFilter,
        window: HistoryWindow,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(tag = "kind", content = "args")]
pub enum Change {
    /// One reviewed rules-defined form; see `rewrites/primitives.json5`.
    Primitive {
        name: String,
        operands: Vec<Operand>,
    },
    PlayerRule(PlayerRule),
    /// Changes a game rule, without granting the subject an ability.
    Rule(RuleChange),
    Pump {
        power: Expr,
        toughness: Expr,
    },
    SetBasePowerToughness {
        power: Expr,
        toughness: Expr,
    },
    CreatureTypes {
        mode: ChangeMode,
        types: CreatureTypeSet,
    },
    Colors {
        mode: ChangeMode,
        colors: Vec<ManaColor>,
    },
    /// The abilities chosen by a `ChooseValue` of `ValueKind::Ability`.
    ChosenAbility,
    /// The color chosen by this ability or as this object entered.
    ChosenColor {
        mode: ChangeMode,
    },
    /// The type chosen by this ability or as this object entered.
    ChosenType {
        kind: ValueKind,
        mode: ChangeMode,
    },
    AddCardType(CardType),
    RemoveCardType(CardType),
    AddSupertype(SuperType),
    AddLandType(crate::mtg_types::LandType),
    AddArtifactType(crate::mtg_types::ArtifactType),
    /// Sets card types; supertypes stay and subtypes follow 205.1a. The exact
    /// Artifact+Creature pair retains prior card types under 205.1b, as in
    /// "becomes an artifact creature". Creature-type replacement is a separate
    /// CreatureTypes change. Other type lists replace the prior card types.
    SetCardTypes(Vec<CardType>),
    GrantKeyword(Keyword),
    GrantAbility(Box<Ability>),
    RemoveAllAbilities,
    SetController(PlayerRef),
    SetPower(Expr),
    SetToughness(Expr),
    SetArtifactType(crate::mtg_types::ArtifactType),
    RemoveSupertype(SuperType),
    SwitchPowerToughness,
    /// The permanent this spell becomes enters with these counters.
    EntersWithCounters {
        kind: crate::counter::CounterKind,
        amount: Expr,
    },
    /// Only for a spell: the total cost it is cast for.
    SpellCost(SpellCostModifier),
    CantBeCountered,
    /// Only in `ActivationLimits::modifiers`.
    ActivationRule(ActivationRule),
    LosesAbility(Box<Operand>),
    SetLandType(Box<Operand>),
    /// Copiable values come from `object`, with these copy-time exceptions.
    CopyOf {
        object: ObjectRef,
        exceptions: Vec<CopyException>,
    },
    AddEnchantmentType(Box<Operand>),
    SetName(String),
    /// Has these kinds of abilities of each of `objects`.
    GainAbilitiesOf {
        objects: ObjectSet,
        kinds: Box<Operand>,
        /// Only one of the gained abilities may be activated each turn.
        once_each_turn: bool,
        /// How mana may be spent to activate them.
        spend_as: Option<SpendAs>,
    },
    /// CR 612: a text-changing effect replacing one word with another.
    ReplaceWord {
        from: Box<Operand>,
        to: Box<Operand>,
    },
    /// On a spell and on the permanent it becomes, until this expires.
    Continuing {
        changes: Vec<Change>,
        until: Duration,
    },
    /// Part of the object's copiable values (707.2).
    Copiable(Vec<Change>),
    /// Every land type of this scope (CR 305.6).
    LandTypes {
        scope: LandTypeScope,
        mode: ChangeMode,
    },
    RemoveAllLandTypes,
    RemoveSubtype(SubType),
    /// The creature types this object has.
    CreatureTypesOf {
        object: ObjectRef,
        mode: ChangeMode,
    },
    /// Has this ability unless it already does.
    GrantAbilityIfAbsent(Box<Ability>),
    /// Also has every name of these cards.
    AddNames(ObjectFilter),
    /// Counts as colorless wherever it is a source of damage.
    ColorlessDamageSource,
    /// Has this object's full text, and more.
    TextOf {
        object: ObjectRef,
        plus: Box<Operand>,
    },
    /// Adds these symbols to its mana cost and their colors (perpetually).
    Incorporate(Vec<ManaSymbol>),
    /// Takes the color of the mana an earlier instruction added.
    ColorOfMana {
        mode: ChangeMode,
        mana: ResultId,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ChangeMode {
    Set,
    Add,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum CreatureTypeSet {
    Listed(Vec<CreatureType>),
    /// The rules' creature-type universe, preserving other subtype categories.
    All,
}

/// Subjects are live for static definitions. Resolving rule effects retain
/// queries and capture resolution variables, as for object rule effects.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum EffectSubjects {
    Objects(ObjectSet),
    Players(PlayerSet),
}

impl From<ObjectSet> for EffectSubjects {
    fn from(objects: ObjectSet) -> Self {
        Self::Objects(objects)
    }
}

impl From<PlayerSet> for EffectSubjects {
    fn from(players: PlayerSet) -> Self {
        Self::Players(players)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum EffectEvaluation {
    /// Select objects and evaluate values when the instruction resolves.
    Snapshot,
    /// Retain queries and evaluate them when the continuous effect is applied.
    Live,
    /// Resolving rule effects (CR 611.2c-d): retain live subject queries and
    /// rule predicates, but capture resolution variables such as announced X.
    LiveObjects,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum RuleChange {
    /// One reviewed rules-defined form; see `rewrites/primitives.json5`.
    Primitive {
        name: String,
        operands: Vec<Operand>,
    },
    Prohibit(ObjectAction),
    /// Requirements are maximized subject to restrictions; they do not compel
    /// payment of a declaration cost (CR 508.1d / 509.1c).
    RequireCombat {
        action: ObjectAction,
        amount: RequirementAmount,
    },
    CombatCost {
        action: ObjectAction,
        payer: PlayerRef,
        cost: Cost,
    },
    /// A restriction checked only when the subject participates in this role.
    /// Counts use the proposed declaration, not the current battlefield state.
    CombatGroup {
        role: CombatRole,
        participants: ObjectFilter,
        others_only: bool,
        comparison: Comparison,
        amount: Expr,
    },
    AttackException {
        ignore: AttackCheck,
        defenders: AttackDefenders,
    },
    BlockWhileTapped,
    BlockCapacity(BlockCapacity),
    CombatDamage(CombatDamageRule),
    UntapStep {
        players: PlayerSet,
        behavior: UntapBehavior,
    },
    /// Attacks each combat if able, and not its goader if able (CR 701.38).
    GoadedBy(PlayerRef),
    LegendRuleExempt,
    /// Counts as `bonus` more power for this tap cost (CR 702.122, 702.171).
    TapPowerBonus {
        cost: TapPowerCost,
        bonus: Expr,
    },
    /// CR 701.43: may be exerted as it attacks, with what that does.
    MayExert(Option<Box<Operand>>),
    /// Blocked as though it lacked this ability.
    BlockedAsThoughLacking(Box<Operand>),
    CantGainAbility(Box<Operand>),
    /// Damage stays marked through the cleanup step (CR 514.2).
    DamageStaysMarked,
    /// Can't attack a player or their permanents while this holds of them.
    CantAttackDefenderIf(Condition),
    /// Targetable by these sources as though lacking this keyword.
    TargetableDespite {
        sources: ObjectFilter,
        keyword: TargetingKeyword,
    },
    /// This can be attached only to what matches.
    AttachOnlyTo(ObjectFilter),
    /// Can block only while this holds of the attacking player.
    BlocksOnlyIf(Condition),
    /// Can be blocked only by, or not by, a defending player among these.
    BlockedByDefender {
        players: PlayerSet,
        unless: bool,
    },
    /// When this attacks, those attack too.
    AttackDrags(ObjectFilter),
    /// Attacks if able when one of these attacks.
    AttacksWhenAttacking(ObjectFilter),
    /// Can block attackers matching `attackers` despite this evasion keyword.
    BlocksDespite {
        attackers: ObjectFilter,
        evasion: BlockEvasion,
    },
    /// Uses toughness rather than power for this tap cost.
    TapCostUsesToughness(TapPowerCost),
    CounterLimit {
        kind: crate::counter::CounterKind,
        max: Expr,
    },
    /// This many activations of matching abilities each turn.
    ActivationsEachTurn {
        abilities: ObjectFilter,
        times: Expr,
    },
    /// Its own abilities cost differently, as `PlayerRule::ActivationCost`.
    OwnActivationCost {
        abilities: ObjectFilter,
        change: ActivationCostChange,
    },
    /// May assign its combat damage among these rather than as the rules say.
    AssignDamageTo {
        players: Option<PlayerRef>,
        permanents: ObjectFilter,
    },
    /// A rule one or two cards print, with nothing to parameterise.
    Unique(UniqueObjectRule),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum BlockEvasion {
    Flying,
    Shadow,
    Landwalk,
}

/// The closed list of object rules specific to one or two cards.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum UniqueObjectRule {
    /// Ghostly Flame.
    ColorlessSourceOfDamage,
    /// Ood Sphere.
    TapsOnlyToAttack,
    /// Tromokratis.
    BlockedOnlyByAll,
    /// Nacatl War-Pride.
    BlockedByExactlyOne,
    /// Ogre Enforcer.
    DestroyedOnlyBySingleSource,
    /// Sanctum Lurker.
    SurvivesZeroLoyalty,
    /// Whippoorwill.
    DamageToItUnpreventable,
    /// Zilortha, Strength Incarnate.
    LethalDamageUsesPower,
    /// Loot, the Anomaly.
    AssignsAsThoughPowerPositive,
    /// Me, the Immortal.
    CountersFollowBetweenZones,
    /// Panglacial Wurm.
    CastWhileSearched,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum TapPowerCost {
    Crew,
    Saddle,
    Station,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ObjectAction {
    Counter,
    Copy,
    /// One proposed attack from the affected creature to a defender.
    Attack(AttackDefenders),
    /// One proposed block from the affected blocker to an attacker.
    Block(ObjectFilter),
    /// One proposed block of the affected attacker by a matching blocker.
    /// A blocking tax is paid per such pairing by the declaring blocker player.
    BeBlocked(ObjectFilter),
    Activate(ActivationFilter),
    Untap,
    Regenerate,
    Sacrifice,
    /// A spell or ability on the stack matching this cannot target the object.
    BeTargeted(ObjectFilter),
    /// No counter of this kind, or none at all, can be put on the object.
    ReceiveCounters(Option<crate::counter::CounterKind>),
    Transform,
    PhaseOut,
    Crew,
    Cast,
    /// Being attached by what matches. `keep_existing` spares what already is.
    BeAttachedBy {
        attachments: ObjectFilter,
        keep_existing: bool,
    },
    TurnFaceUp,
    BecomeSuspected,
    BeBeamedUp,
    ChangeControl,
    LoseCounters(crate::counter::CounterKind),
    BeAttacked,
    PhaseIn,
    BePlayed,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ActivationFilter {
    Cycling,
    Any,
    Mana,
    Not(Box<ActivationFilter>),
    All(Vec<ActivationFilter>),
    Either(Vec<ActivationFilter>),
    /// Abilities of a source that matches.
    Of(Box<ObjectFilter>),
    Is(StackTrait),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum RequirementAmount {
    /// One collective requirement with this minimum.
    AtLeast(Expr),
    /// A separate requirement for each matching participant.
    Each,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum CombatRole {
    Attacker,
    Blocker,
    BlockedAttacker,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum AttackCheck {
    Defender,
    SummoningSickness,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum BlockCapacity {
    Additional(Expr),
    Unlimited,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum CombatDamageRule {
    NoAssignment,
    UseToughness,
    AsUnblocked { optional: bool },
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum UntapBehavior {
    Skip,
    Optional,
    Additional,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct CounterPlacement {
    pub recipients: Recipients,
    pub kind: crate::counter::CounterKind,
    pub amount: Expr,
    /// A resolution-time selection, independent of announcement targets.
    pub selection: Option<SelectionChoice>,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct CounterRemoval {
    /// The player instructed to remove counters, who chooses kinds/counts
    /// when the instruction leaves them open. This is also the cost's payer.
    pub player: PlayerRef,
    pub recipients: Recipients,
    /// None permits choosing counters of any kinds.
    pub kind: Option<crate::counter::CounterKind>,
    pub amount: CounterAmount,
    pub selection: Option<SelectionChoice>,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum CounterAmount {
    Count(Count),
    /// All counters present, including zero; distinct from choosing any number.
    All,
    /// This many in total, spread however the player likes among the recipients.
    AmongThem(Count),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum Duration {
    EndOfTurn,
    EndOfCombat,
    Indefinite,
    UntilNextTurn(PlayerRef),
    UntilEndOfNextTurn(PlayerRef),
    /// Ends when the object leaves the battlefield, or at once if it already has.
    WhileOnBattlefield(ObjectRef),
    WhileControlled {
        player: PlayerRef,
        object: ObjectRef,
    },
    WhileMatches {
        object: ObjectRef,
        filter: ObjectFilter,
    },
    WhileExiled(ObjectRef),
    /// Ends when the next such point arrives for one of these players.
    UntilNext {
        point: TurnPoint,
        players: PlayerSet,
    },
    DuringNextUntapStep(PlayerRef),
    /// Each affected object's own controller.
    DuringControllersNextUntapStep,
    DuringNextTurn(PlayerRef),
    /// Ends when the first of these ends.
    Either(Vec<Duration>),
    /// Ends when this player pays the cost, a special action they may take
    /// any time they have priority.
    UntilPaid {
        player: PlayerRef,
        cost: Cost,
    },
    /// Ends when none of these cards remain in exile.
    WhileAnyExiled(ObjectSet),
    /// Ends when this card is cast from exile.
    UntilCast(ObjectRef),
    /// One reviewed rules-defined form; see `rewrites/primitives.json5`.
    Primitive {
        name: String,
        operands: Vec<Operand>,
    },
    /// Through the extra turn this earlier instruction created.
    ExtraTurn(ResultId),
    /// Ends when this event next happens.
    UntilEvent(Box<EventSpec>),
    /// Ends as soon as this stops being true.
    While(Box<Condition>),
    /// Ends when the object affected stops matching.
    WhileItMatches(ObjectFilter),
    /// Through the end of the next turn, whoever's it is.
    ThroughNextTurn,
    EndOfCombatOnNextTurn(PlayerRef),
    UntilNextUntapStep(PlayerRef),
    /// Ends when this card stops being on top of this player's library.
    UntilLibraryTopChanges(PlayerRef),
    /// Through the combat phase this instruction created.
    ExtraCombat(ResultId),
    WhilePlayerHasCounter {
        player: PlayerRef,
        kind: crate::counter::CounterKind,
    },
    /// Ends when these players next exile a card with this object.
    UntilAnotherExiled {
        by: PlayerSet,
        with: ObjectRef,
    },
    /// The next this many untap steps of the object's controller.
    DuringControllersNextUntapSteps(Expr),
    /// Ends the next time state-based actions are checked, as Forge's
    /// `UntilStateBasedActionChecked`.
    UntilStateBasedActionsChecked,
    /// Through the end of this player's next upkeep that begins after the
    /// effect is created. An upkeep already in progress is not that next
    /// upkeep; skipped upkeeps do not consume it. Additional upkeeps count.
    UntilEndOfNextUpkeep(PlayerRef),
}
