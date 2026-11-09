//! Abilities in the normalized semantic model.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(tag = "kind", content = "args")]
pub enum Ability {
    /// Overrides the source's ordinary functioning zones. Check the condition
    /// while the ability functions / is activated / triggers, not again when
    /// an already-created stack ability resolves. Does not grant control or
    /// permission to activate another player's object.
    Scoped {
        zones: Vec<EventZone>,
        condition: Option<Condition>,
        ability: Box<Ability>,
    },
    Keyword(Keyword),
    /// Intrinsic rules for casting this object (CR 113.6d-f), independent of
    /// its resolution program and of ordinary battlefield static effects.
    Casting(CastingRule),
    /// One replacement application. The body modifies the pending event and
    /// completes before that event occurs; it is not an ETB trigger.
    Replacement {
        active: ReplacementActive,
        event: ReplacementEvent,
        body: Program,
    },
    /// A static enchant keyword. Its predicate governs both the Aura spell's
    /// target and whether the Aura may remain attached; entering otherwise does
    /// not turn the host choice into a target.
    Enchant {
        hosts: EnchantHosts,
    },
    Spell {
        announcement: Announcement,
        body: Program,
    },
    Activated {
        origin: ActivationOrigin,
        cost: Cost,
        limits: ActivationLimits,
        announcement: Announcement,
        body: Program,
    },
    Triggered {
        event: EventSpec,
        intervening_if: Option<Condition>,
        limits: TriggerLimits,
        announcement: Announcement,
        body: Program,
    },
    /// A live definition: selectors and expressions are re-evaluated in layers.
    Static {
        kind: StaticKind,
        affected: EffectSubjects,
        changes: Vec<Change>,
    },
    Conditional {
        condition: Condition,
        then: Vec<Ability>,
        otherwise: Vec<Ability>,
    },
    /// A spell with two resolutions, chosen by an optional cost at casting.
    /// Each branch is announced on its own, so kicked targets are only chosen
    /// when the cost is (CR 601.2c, 702.33).
    CostBranch {
        keyword: String,
        cost: Cost,
        paid: Box<Ability>,
        unpaid: Box<Ability>,
    },
    /// A spell with one resolution per cost, and one of the costs must be paid
    /// as it is cast (CR 601.2b). Each option is announced on its own.
    CostOptions {
        keyword: String,
        options: Vec<(Cost, Ability)>,
    },
    /// One triggered ability per chapter number (CR 714.2b), each on
    /// `EventSpec::SagaChapter`. The Saga rules supply lore counters and sacrifice.
    Saga {
        chapters: Vec<Ability>,
    },
    /// Levels 2 and up of a Class (716): gaining one is a sorcery-speed activated
    /// ability, and each level's abilities apply from then on.
    Class {
        levels: Vec<ClassLevel>,
    },
    /// A rule with no reduced form. Never present on a complete card.
    Named(Named),
    /// One rules-defined ability, such as a keyword, from a reviewed row in
    /// `rewrites/primitives.json5`. Operands are bound, as for `Operation::Primitive`.
    Primitive {
        name: String,
        operands: Vec<Operand>,
    },
    /// Done before the game's first turn, by a player holding this card (103.6).
    OpeningHand {
        announcement: Announcement,
        body: Program,
    },
    /// A keyword ability the rules define, with its parameters.
    /// Abilities that apply while this many counters are on the permanent:
    /// level up, with its activated cost, and station.
    CounterStages {
        counter: crate::counter::CounterKind,
        level_up: Option<Cost>,
        stages: Vec<CounterStage>,
    },
    /// "Backup N": when this enters, put counters on a target creature; if it
    /// is another, it gains `abilities` until end of turn (702.165).
    Backup {
        amount: i64,
        abilities: Vec<Ability>,
    },
    /// A rule of deck construction or commander choice, never read in a game.
    DeckRule(DeckRule),
    Mechanic {
        mechanic: Mechanic,
        cost: Option<Cost>,
        amount: Option<Expr>,
        filter: Option<ObjectFilter>,
    },
    /// Gift (CR 702.174): as this is cast its controller may promise a gift to
    /// an opponent, `PlayerRef::GiftedPlayer`, who gets `gift` if they do. The
    /// outcomes are absent for a permanent, which has no resolution to choose.
    Gift {
        gift: Vec<Program>,
        promised: Option<Box<Ability>>,
        unpromised: Option<Box<Ability>>,
    },
    /// Spree (CR 702.172): choose one or more modes, adding each one's cost.
    Spree {
        modes: Vec<SpreeMode>,
    },
    /// Prototype (CR 702.160): an alternate mana cost and power and toughness.
    Prototype {
        mana: Vec<ManaSymbol>,
        power: i32,
        toughness: i32,
    },
    /// An Attraction's visit, and its prize when it has one (CR 717.5).
    Visit {
        body: Box<Ability>,
        prize: Option<Box<Ability>>,
    },
    /// A room of a dungeon (CR 309) and the rooms it leads to.
    DungeonRoom {
        name: String,
        body: Box<Ability>,
        leads_to: Vec<String>,
    },
    /// Changes how often `abilities` trigger when `cause` is what triggers
    /// them, as Panharmonicon and Torpor Orb do. No `abilities` means all, and
    /// no `cause` means any event. Forge's `Mode$ Panharmonicon | ValidMode$`.
    TriggerRule {
        cause: Option<EventSpec>,
        abilities: Option<ObjectFilter>,
        rule: TriggerRule,
    },
    /// What this card does in a draft or before the game (Conspiracy).
    DraftRule(Vec<Operand>),
    /// CR 702.139: the condition a starting deck has to meet.
    Companion(Box<Operand>),
    /// A deck rule that only applies with this card as commander.
    CommanderDeckRule(Box<Operand>),
    /// CR 702.48: cast by sacrificing a matching permanent.
    Offering(ObjectFilter),
    /// Has flash for players while this holds.
    FlashIf(Condition),
    /// CR 702.167: exile this and materials, return transformed.
    Craft {
        materials: Box<Operand>,
        count: Option<(Comparison, Expr)>,
        cost: Box<Operand>,
    },
    /// These abilities apply while the condition holds.
    Mystery {
        condition: Condition,
        abilities: Vec<Ability>,
    },
    /// Protection that does not remove these permanents already attached.
    ProtectionExcept {
        from: Box<Operand>,
        keeps: ObjectSet,
    },
    /// Damage can't be prevented: from these sources, or all, and combat only.
    DamageCantBePrevented {
        sources: Option<ObjectSet>,
        combat_only: bool,
    },
    /// These cards can't enter, from this zone when one is named.
    CantEnter {
        cards: ObjectFilter,
        from: Option<ZoneScope>,
    },
    /// No more than this many creatures can attack this defender each combat.
    AttackCap {
        defender: Recipients,
        amount: Expr,
    },
    /// Done as this event happens, rather than as a trigger.
    AsEvent {
        object: ObjectRef,
        event: AsEvent,
        actions: Box<Operand>,
    },
    /// Done before the first turn of the game or as a scheme is set in motion.
    BeforeGame {
        when: BeforeGameMoment,
        body: Box<Operand>,
    },
    /// A keyword or ability with a condition its plain form lacks.
    Modified {
        ability: Box<Ability>,
        modifier: Box<Operand>,
    },
    /// Tiered (CR 702.183): choose one mode, adding its cost.
    Tiered {
        modes: Vec<SpreeMode>,
    },
    /// A player may pay `cost` to ignore these rules until `until`.
    PayToIgnore {
        subjects: EffectSubjects,
        rules: Box<Operand>,
        payer: Option<PlayerRef>,
        cost: Cost,
        until: Duration,
    },
    /// Is a card with this name for spells that name another.
    CountsAsNamed {
        name: String,
        for_spells: String,
    },
    /// Functions only on top of its owner's library.
    WhileOnTopOfLibrary(Vec<Ability>),
    /// If kicked with this cost, the spell has this ability.
    KickedHas {
        cost: Cost,
        ability: Box<Ability>,
    },
    StartingHandSize(Expr),
    VillainousChoiceTwice(PlayerSet),
    /// While these players search, found cards are exiled for this player to play.
    ExilesSearchedCards {
        searching: PlayerSet,
        player: PlayerRef,
    },
    /// An ability one or two cards print, with nothing to parameterise.
    Unique(UniqueAbility),
    /// One printed ability with an event and an effect for each: each `Triggered`
    /// reads its own event, which `EventSpec::Either` can't let a shared body do.
    Triggers(Vec<Ability>),
    /// Textual presentation, independent of reference scope and game meaning.
    /// Labels are ability/flavor words before an em dash. Reminder wording is
    /// supplied by grammar templates, never stored as an opaque text fragment.
    /// Absence of this wrapper means no label and no reminder.
    WithPresentation {
        label: Option<String>,
        reminder: bool,
        ability: Box<Ability>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum AsEvent {
    Transforms,
    EntersOrTurnsFaceUp,
    TurnsFaceUp,
    Escapes,
    BecomesAttachedTo(ObjectFilter),
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum BeforeGameMoment {
    GameBegins,
    OpeningHand,
    SetInMotion,
}

/// The closed list of abilities specific to one or two cards.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum UniqueAbility {
    /// Backup Plan.
    AdditionalOpeningHand,
    /// Power Play.
    StartingPlayer,
    /// Angel of Eternal Dawn.
    NeverNight,
    /// Everlasting Torment.
    AllDamageWithers,
    /// Undercity.
    VentureOnlyByName,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum StaticKind {
    Ordinary,
    /// Intrinsic definition of this source (604.3), including token-creation
    /// text and copied definitions. Functions in every zone, outside the game,
    /// and before play; applies before other effects in its layer (P/T in 7a).
    /// A granted or conditionally functioning static ability is not a CDA.
    /// Undeterminable numeric components use zero (208.2a), without replacing
    /// a supported formula by its current value or clamping negative values.
    CharacteristicDefining {
        keyword: Option<Keyword>,
        contributes_to_color_identity: bool,
    },
    /// Ordinary while its source functions, and goes on for this long after the
    /// source leaves the battlefield, as Forge's static with an `Effect` on leaving.
    ContinuesAfterLeaving(Duration),
}

/// A keyword's identity remains observable to cost modifiers and triggers,
/// even when its ordinary activated cost/targets/program are shared.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ActivationOrigin {
    Ordinary,
    Equip,
    /// None is ordinary cycling; Some retains the typecycling predicate.
    Cycling {
        search: Option<ObjectFilter>,
    },
    /// The cost identifies creatures that crew as payment is made; the source
    /// becomes crewed when this ability resolves (702.122b/e), even if already
    /// a creature. Keep identity separate from the generic cost/effect recipe.
    Crew,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct CounterStage {
    pub minimum: i64,
    pub maximum: Option<i64>,
    pub power_toughness: Option<(i32, i32)>,
    /// A station stage makes the permanent an artifact creature.
    pub animates: bool,
    pub abilities: Vec<Ability>,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum PartnerKind {
    Any,
    CharacterSelect,
    FatherAndSon,
    FriendsForever,
    Survivors,
    With(String),
}

/// What a card says about building a deck, as Forge's `K:Partner` and
/// `DeckHints` do. None of these changes a game in progress.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum DeckRule {
    CanBeCommander,
    Partner(PartnerKind),
    DoctorsCompanion,
    ChooseBackground,
    AnyNumberOfCopies,
    UpToCopies(Expr),
    NoMaximumDeckSize,
    AnyBasicLands,
    AnyCards(ObjectFilter),
    AnyCardsWithOneAdditionalColor(ObjectFilter),
    Banned,
    RemoveUnlessPlayingForAnte,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum TriggerRule {
    TriggersAgain,
    DoesntTrigger,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ActivationGrant {
    AsThoughHaste,
    InstantSpeed,
    /// During these players' turns, and any time they could cast an instant.
    InstantSpeedDuring(PlayerSet),
    /// This many more activations each turn than the rules allow.
    AdditionalTimes(Expr),
    AsThoughNotActivated,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct TriggerLimits {
    pub per_turn: Option<u32>,
    pub total: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct ActivationLimits {
    pub once: bool,
    pub once_each_turn: bool,
    pub sorcery: bool,
    /// Checked when activating, before costs are paid; not a resolution guard.
    pub condition: Option<Condition>,
    /// Players other than the controller who may activate this.
    pub activators: Option<PlayerSet>,
    pub keyword: Option<ActivatedKeyword>,
    /// Modifiers of the activation itself: `ActivationRule`, or a reviewed row.
    pub modifiers: Vec<Change>,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct ClassLevel {
    pub cost: Cost,
    pub abilities: Vec<Ability>,
}

/// Rules-defined kinds of activated ability, which other cards can refer to. Both
/// activate only once; power-up also reduces its cost on the turn it was cast.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ActivatedKeyword {
    Exhaust,
    PowerUp,
    /// Only if the creature attacked this turn, and once each turn.
    Boast,
    /// From the hand during its owner's upkeep, and once each turn.
    Forecast,
}

/// What one activated ability says about its own activation.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ActivationRule {
    Cost {
        modifier: SpellCostModifier,
        condition: Option<Condition>,
    },
    MaxPerTurn(Expr),
    XNotZero,
    XAtMost(Expr),
    XIs(Expr),
    CantBeCopied,
    AnyTimeOfInstant,
    /// Only these players may activate this ability.
    OnlyBy(PlayerSet),
    /// Spend only mana of this color (none is the chosen color).
    RestrictMana {
        portion: ManaCostPortion,
        color: Option<ManaColor>,
    },
    /// This cost may be paid rather than each mana of the activation cost.
    ManaInstead(Cost),
}

impl ActivationLimits {
    pub fn unrestricted() -> Self {
        Self {
            once: false,
            once_each_turn: false,
            sorcery: false,
            condition: None,
            activators: None,
            keyword: None,
            modifiers: vec![],
        }
    }
}
