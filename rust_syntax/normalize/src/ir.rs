//! Portable semantic card definitions. No function pointers or runtime object
//! IDs belong here. Strict lowering rejects unsupported source constructs.
//! `crate::unified` represents them as `Named` forms instead: typed, reported
//! as unreduced, and never accepted by `validate::card` or counted as complete.

use crate::mtg_types::{CardType, CreatureType, SubType, SuperType};
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 101;

/// One record of any layout. Layout is data around ordinary faces, so Transforming,
/// Split, Planar and the rest share `Card`. `data` keeps layout fields that belong
/// to no face, such as `MeldsInto`.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct UnifiedCard {
    pub schema_version: u32,
    pub layout: String,
    pub faces: Vec<Card>,
    pub data: Vec<Field>,
}

/// One sticker on a sheet. `tickets` is what it costs to put on a permanent.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum Sticker {
    Name(String),
    PowerToughness {
        tickets: u32,
        power: i32,
        toughness: i32,
    },
    Abilities {
        tickets: u32,
        abilities: Vec<Ability>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct Field {
    pub name: String,
    pub value: Operand,
}

/// A source form not yet reduced to generic operations. Names come from the Rust
/// source enums, so no per-variant code exists for one. This is unified, not
/// complete: see "Two different finish lines" in AST_PLAN.md.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct Named {
    pub source_type: String,
    pub form: String,
    pub operands: Vec<Operand>,
}

/// Operands of a `Named` form. Typed variants hold reduced values lowered with no
/// surrounding bindings, so each is meaningful without its enclosing ability.
/// A reference that needs those bindings stays a `Form`, which keeps it visible
/// rather than attaching it to the wrong antecedent.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(tag = "kind", content = "args")]
pub enum Operand {
    Absent,
    Bool(bool),
    Integer(i64),
    Text(String),
    List(Vec<Operand>),
    Record(Vec<Field>),
    Form(Box<Named>),
    /// A fieldless form of a vocabulary enum, such as a subtype or counter type.
    Vocabulary {
        family: String,
        form: String,
    },
    /// A form of a descriptive enum listed under `terms` in the primitive
    /// registry. Unlike `Form`, every operand is bound.
    Term {
        family: String,
        form: String,
        operands: Vec<Operand>,
    },
    Object(ObjectRef),
    Objects(ObjectSet),
    Recipients(Recipients),
    Player(PlayerRef),
    Players(PlayerSet),
    Number(Expr),
    Condition(Condition),
    Duration(Duration),
    Cost(Cost),
    Ability(Box<Ability>),
    Program(Program),
    Filter(ObjectFilter),
    Comparison(Comparison, Expr),
    /// The event a row holds; its other operands can name what it binds.
    Event(Box<EventSpec>),
    /// The instruction a "this way" row reads: the nearest one before it.
    Result(ResultId),
    Stickers(Vec<Sticker>),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct Card {
    pub schema_version: u32,
    pub name: String,
    pub characteristics: Characteristics,
    pub abilities: Vec<Ability>,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct Characteristics {
    pub supertypes: Vec<SuperType>,
    pub card_types: Vec<CardType>,
    pub subtypes: Vec<SubType>,
    pub mana_cost: Option<Vec<ManaSymbol>>,
    pub color_indicator: Vec<ManaColor>,
    pub power_toughness: Option<(i32, i32)>,
    pub loyalty: Option<Expr>,
    pub defense: Option<i32>,
}

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
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum CastingRule {
    /// Intrinsic permission to cast this exact card from a zone by an
    /// alternative cost. Does not waive ordinary timing, additional costs,
    /// or restrictions. `require` tests the resulting spell, not just the
    /// card's characteristics before announcement (702.34a).
    ///
    /// If this permission's cost was chosen, `leave_stack` redirects any
    /// departure to its destination, including countering/bouncing, while
    /// this stack-functioning ability exists. It is not a resolution action.
    /// Choosing an alternative cost is copied under 707.10, independently
    /// of whether the copy was cast or had mana actually spent on it.
    /// When granted to a card, this casting ability continues to the resulting
    /// stack object under 400.7g; the grant still has its original expiration.
    Permission {
        origin: CastingOrigin,
        player: PlayerRef,
        from: ZoneScope,
        require: ObjectFilter,
        cost: Cost,
        leave_stack: Option<Destination>,
    },
    AdditionalCost {
        cost: Cost,
        optional: bool,
        repetition: CostRepetition,
    },
    /// Replaces only the printed mana cost; additional costs and modifiers
    /// still apply. Only one alternative is chosen (CR 118.9).
    AlternativeCost(Cost),
    /// One permission, so the cost applies only when casting with flash.
    AlternativeCostWithFlash(Cost),
    /// Apply to the total cost at CR 601.2f, then clamp its mana component at
    /// zero. Colored reductions spill into generic mana (CR 118.7b-c).
    ReduceCost(Vec<ManaReduction>),
    /// Check the completed casting proposal (CR 601.2e), before paying costs.
    /// A payment cannot make an initially illegal proposal legal (601.6).
    Require(Condition),
    /// Changes timing, without granting permission to cast from a new zone.
    Flash {
        additional_cost: Option<Cost>,
    },
    ProhibitTargets {
        domain: ObjectDomain,
        filter: ObjectFilter,
    },
    RestrictMana {
        portion: ManaCostPortion,
        filter: ManaPaymentFilter,
    },
    SpendAsThoughAnyType,
    If {
        condition: Condition,
        rule: Box<CastingRule>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum CastingOrigin {
    Ordinary,
    Flashback,
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum CostRepetition {
    Once,
    /// Once for each announced target beyond this many initial targets.
    TargetsBeyond(u32),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct ManaReduction {
    /// None reduces generic mana only, not a mana of any type.
    pub color: Option<ManaColor>,
    pub amount: Expr,
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ManaCostPortion {
    All,
    X,
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ManaPaymentFilter {
    None,
    Colors(Vec<ManaColor>),
    /// Test the producing permanent, retaining its production provenance.
    ProducedBy(ObjectFilter),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum EnchantHosts {
    Permanents(ObjectFilter),
    Players(PlayerSet),
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
}
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum ReplacedObjectEvent {
    Destroyed,
    LeavesBattlefield,
    UntapsInUntapStep,
    Connives,
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
    SetDestination(Destination),
    /// How the entering object enters.
    Entry(EntryState),
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
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(tag = "kind", content = "args")]
pub enum Keyword {
    Devoid,
    Changeling,
    Protection(Box<Protection>),
    HexproofFrom(Box<Protection>),
    /// Unblockable if a defending player controls a land matching this
    /// predicate. Does not grant a land type or inspect the attacker's lands.
    Landwalk(Box<ObjectFilter>),
    /// Native triggered keyword (702.21), not an additional targeting cost.
    /// Each instance triggers independently. The opponent controlling the
    /// targeting spell/ability may pay when the ward trigger resolves; an
    /// uncounterable spell/ability may be left unpaid. Source-relative values
    /// still refer to the permanent with ward, with current/LKI information.
    /// Its cost is a fresh ability definition, evaluated at ward resolution,
    /// not an expression to snapshot when a continuous effect grants ward.
    Ward(Box<Cost>),
    Flying,
    Trample,
    Lifelink,
    Deathtouch,
    Vigilance,
    Haste,
    Reach,
    FirstStrike,
    DoubleStrike,
    Indestructible,
    Hexproof,
    Shroud,
    Menace,
    Defender,
    Flash,
    Fear,
    Intimidate,
    Infect,
    Wither,
    Prowess,
    Flanking,
    Horsemanship,
    Shadow,
    Skulk,
}

/// Protection/qualified hexproof quality, rather than a permanent-only filter.
/// Protection applies the targeting, attachment, damage-prevention and blocking
/// rules of 702.16; hexproof uses only its opponent targeting restriction.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum Protection {
    Objects {
        kind: ObjectKind,
        filter: ObjectFilter,
    },
    /// One instance per selected player (702.16i), including owned sources
    /// not controlled by another player (702.16k).
    Players(PlayerSet),
    /// Behaves as separate keyword instances (702.16g-i / 702.11f-g), not
    /// one ability with a merged quality. Duplicate instances remain present.
    Each(Vec<Protection>),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct Announcement {
    pub targets: Vec<TargetSpec>,
    pub modes: Option<ModeSelection>,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct ModeSelection {
    pub minimum: Expr,
    pub maximum: Expr,
    pub repeat: bool,
    pub modes: Vec<Mode>,
    /// The target chosen, or a mode's `requires`, decides the mode, as in
    /// "target spell or permanent", so no mode is announced even though each
    /// has its own target (601.2c).
    pub implied: bool,
    /// Only modes not yet chosen for this ability are available.
    pub unchosen: Option<ModeHistory>,
    /// Paid once for each mode chosen beyond the first, as for escalate (702.120).
    pub surcharge: Option<Cost>,
    /// No two chosen modes may share a target.
    pub distinct_targets: bool,
    /// One of these players chooses instead of the controller.
    pub chooser: Option<PlayerSet>,
    /// The game picks uniformly, not a player.
    pub random: bool,
    /// Chosen modes' weights may total at most this, as pawprints.
    pub budget: Option<Expr>,
    /// Either the stated number of modes or none: "you may choose two".
    pub may_decline: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ModeHistory {
    ThisTurn,
    ThisGame,
    /// Not chosen during this player's last combat.
    LastCombat(PlayerRef),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct Mode {
    pub announcement: Announcement,
    pub body: Program,
    /// What this mode spends of `ModeSelection::budget`.
    pub weight: i64,
    /// This mode is available only while this holds as modes are chosen.
    pub requires: Option<Condition>,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct TargetSpec {
    pub slot: Slot,
    pub kind: TargetKind,
    pub count: Count,
    pub filter: ObjectFilter,
    pub players: Option<PlayerSet>,
    /// A total split among this slot's targets when they are chosen (601.2d).
    /// Each target's part is `Expr::DistributedShare`.
    pub distribute: Option<Expr>,
    /// What the targets chosen for this slot must satisfy together.
    pub constraints: Vec<SelectionConstraint>,
    /// Who chooses this slot's targets, when not the ability's controller.
    pub chooser: Option<PlayerRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum TargetKind {
    Object(ObjectDomain),
    Player,
    DamageRecipient,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ObjectKind {
    Any,
    Permanent,
    Spell,
    Card,
    /// An activated or triggered ability on the stack (113.1, 114.1b).
    Ability,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct ObjectDomain {
    pub kind: ObjectKind,
    pub zones: Vec<EventZone>,
}
impl ObjectDomain {
    pub fn permanent() -> Self {
        Self {
            kind: ObjectKind::Permanent,
            zones: vec![EventZone::Battlefield],
        }
    }
    pub fn spell() -> Self {
        Self {
            kind: ObjectKind::Spell,
            zones: vec![EventZone::Stack],
        }
    }
    pub fn ability() -> Self {
        Self {
            kind: ObjectKind::Ability,
            zones: vec![EventZone::Stack],
        }
    }
    pub fn spell_or_permanent() -> Self {
        Self {
            kind: ObjectKind::Any,
            zones: vec![EventZone::Stack, EventZone::Battlefield],
        }
    }
    pub fn card(zone: EventZone) -> Self {
        Self {
            kind: ObjectKind::Card,
            zones: vec![zone],
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct Slot(pub u16);
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct Binding(pub u32);
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct ResultId(pub u32);

/// A program is a sequence of semantic instructions. The operations making
/// up one continuous-effect instruction stay in one payload.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct Program {
    pub instructions: Vec<Instruction>,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(tag = "kind", content = "args")]
pub enum Instruction {
    Effect {
        result: ResultId,
        operation: Operation,
    },
    Group(Program),
    If {
        condition: Condition,
        then: Program,
        otherwise: Program,
    },
    Optional {
        decider: PlayerRef,
        body: Program,
    },
    /// Run `body`, then run it again each time it reached `RepeatProcess`.
    Repeatable(Program),
    /// Each player, as `binding`, does `body`: choices in APNAP order, and the
    /// actions then happen together (101.4). A choice made in `body` can be read
    /// after the loop as a set of every player's picks.
    ForEachPlayer {
        players: PlayerSet,
        binding: Binding,
        /// Turn order begins with this player instead of the active player.
        starting_with: Option<PlayerRef>,
        body: Program,
    },
    /// The decider picks one option as this resolves; no target is announced.
    ChooseOne {
        decider: PlayerRef,
        options: Vec<Program>,
    },
    /// Run `body`, and run it again each time it reached `RepeatProcess`, for
    /// at most this many runs in all: "repeat this process X more times".
    Times {
        amount: Expr,
        body: Program,
    },
    /// Run `body` once for each of these objects, read as `ObjectRef::Bound`.
    ForEachObject {
        objects: ObjectSet,
        binding: Binding,
        body: Program,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(tag = "kind", content = "args")]
pub enum Operation {
    /// Create a once-only delayed trigger (603.7), eligible only after this
    /// instruction has executed. The turn players and start gate are resolved
    /// at creation. A stated expiration cancels a still-waiting trigger; it
    /// does not cancel an ability already on the stack.
    ///
    /// `body` is a lexical closure over the creator's reference frame: targets,
    /// results, payment histories, announced X, event data, bound players, and exact object
    /// incarnations survive independently of the creator's resolution. Source
    /// and ability controller are retained under 603.7d-g. Queries/properties
    /// and arithmetic in the body are evaluated when it resolves, using LKI
    /// where required; capturing references does not snapshot all expressions.
    /// An object that leaves its expected zone and returns is a new object.
    ///
    /// A matcher's own bindings do NOT replace the captured creator's event
    /// bindings: this form has no new targets/modes and exposes no bindings
    /// from the future event.
    /// Body-local results never escape into the creating program.
    CreateDelayedTrigger {
        event: EventSpec,
        start: DelayedStart,
        expires: Option<Duration>,
        intervening_if: Option<Condition>,
        body: Program,
    },
    /// Privately inspect the queried current objects, retaining their original
    /// identities and order as an observation result. Does not move cards,
    /// reveal them publicly, search, scry, or inspect later top cards anew.
    LookAt {
        player: PlayerRef,
        objects: ObjectSet,
    },
    /// At resolution, create one next-destruction replacement shield per
    /// selected permanent, expiring at end of turn (701.19a). This instruction
    /// does not itself tap, clear damage, remove combat participation, or
    /// regenerate anything. Those happen only when a shield replaces an actual
    /// destruction, unless regeneration is prohibited. Sacrifice/toughness-zero
    /// deaths are not destruction. Each selection is a snapshot of its current
    /// incarnation, and repeated instructions create independent shields.
    CreateRegenerationShields {
        objects: ObjectSet,
    },
    /// One search episode (CR 701.23h), even with several find clauses. Choices
    /// have disjoint original-library identities. A denied/replaced search can
    /// return searched=false; it does not cancel subsequent unconditional work.
    Search {
        player: PlayerRef,
        library: PlayerRef,
        top: Option<Expr>,
        choices: Vec<SearchChoice>,
        /// Zones searched besides the library, all of them that player's.
        also: Vec<EventZone>,
        /// "and/or": the searcher chooses which zones to look through.
        either: bool,
    },
    /// Explicit public revelation; searching never implies revelation.
    Reveal {
        objects: ObjectSet,
    },
    /// Reveal from the top of `library` through the first card matching `filter`.
    /// The result is every revealed card in order, or the whole library if none
    /// matched; the match is the only card in it passing `filter`.
    RevealUntil {
        library: PlayerRef,
        filter: ObjectFilter,
        /// How many matching cards end the reveal.
        count: Expr,
    },
    /// Reordering is within each library, without zone changes. Exclusions hold
    /// selected cards out of the shuffle while they remain in that library.
    Shuffle {
        players: PlayerSet,
        excluding: Option<ObjectSet>,
    },
    /// Reposition only cards still in this library, without a zone change.
    /// Reordered revealed/looked-at cards stop being observed and become new
    /// objects (701.20d-e); old observation/selection identities do not follow.
    /// Remember these objects as they are now, for a later "those".
    NoteObjects {
        objects: ObjectSet,
    },
    ReorderLibrary {
        player: PlayerRef,
        library: PlayerRef,
        objects: ObjectSet,
        end: SequenceEnd,
        order: CardOrder,
    },
    /// CR 701.22: privately look at N top cards, partition/reorder top and bottom.
    /// Scry 0 emits no scry event. Multiple players decide in APNAP order and
    /// reposition simultaneously; completion triggers follow the whole process.
    Scry {
        players: PlayerSet,
        amount: Expr,
    },
    /// CR 701.25: privately look at N top cards, move any number to the graveyard,
    /// and reorder the rest on top. Surveil 0 emits no surveil event. Replacements
    /// apply to actual movements; this is not drawing or revealing those cards.
    Surveil {
        players: PlayerSet,
        amount: Expr,
    },
    /// Change one spell on the stack, or the next spell a player casts.
    ChangeSpell {
        subject: SpellSubject,
        changes: Vec<Change>,
    },
    /// Cast or play cards while resolving, as Forge's `Play`.
    PlayCards {
        actor: PlayerSet,
        action: CardPlayAction,
        cards: ObjectSet,
        count: Count,
        cost: PlayCost,
        into_exile: bool,
    },
    /// Copy cards where they are; `ObjectSet::Copies` reads the copies.
    CopyCards {
        actor: PlayerSet,
        objects: ObjectSet,
        times: Expr,
    },
    /// Countering is a rules action; the destination applies only if it succeeds.
    Counter {
        objects: ObjectSet,
        destination: Destination,
    },
    /// Create a triggered ability that lasts for `duration`. Unlike a delayed
    /// trigger it can fire repeatedly, and it announces its own targets.
    CreateTrigger {
        event: EventSpec,
        duration: Duration,
        announcement: Announcement,
        body: Program,
        /// Ends once it has triggered, as for "your next spell this turn".
        once: bool,
    },
    /// Repeat the nearest enclosing `Instruction::Repeatable` or `Times`.
    RepeatProcess,
    SetLifeTotal {
        players: PlayerSet,
        amount: Expr,
    },
    /// Discard these cards: chosen by whoever the effect says, not by their owner.
    DiscardObjects {
        objects: ObjectSet,
    },
    /// Name a number for later instructions; see `Expr::BoundValue`.
    BindValue {
        value: Expr,
    },
    /// A reflexive trigger (603.12): once, if `performed` happened during this
    /// resolution, it goes on the stack and announces its own targets. `condition`
    /// is checked as it triggers. `body` closes over the creator like a delayed
    /// trigger.
    ReflexiveTrigger {
        performed: ResultId,
        condition: Option<Condition>,
        announcement: Announcement,
        body: Program,
    },
    /// Return these moved objects to where they were when `duration` ends, in one
    /// effect with the move (610.3). If `duration` has already ended, the move
    /// does not happen.
    ReturnAfter {
        objects: ObjectSet,
        duration: Duration,
    },
    /// Create a replacement effect on future events (CR 614.1, 615). Unlike a
    /// delayed trigger, `body` changes the pending event as it would happen.
    CreateReplacement {
        event: ReplacementEvent,
        limit: ReplacementLimit,
        duration: Duration,
        body: Program,
    },
    /// Apply a change to the currently proposed event. Legal only inside a
    /// replacement program for an event supporting this change.
    ModifyEvent {
        change: EventChange,
    },
    /// A resolution-time cost, with a typed decision/result. This differs from
    /// announcement costs on an activated ability.
    PayCost {
        player: PlayerRef,
        cost: Cost,
        optional: bool,
    },
    RemoveCounters {
        removals: Vec<CounterRemoval>,
    },
    Attach {
        attachments: ObjectSet,
        host: ObjectRef,
    },
    /// Every definition in this instruction is created in one batch. Counts and
    /// initial copiable values are evaluated before any token enters. Ability
    /// definitions, including characteristic-defining recipes, remain live.
    CreateTokens {
        players: PlayerSet,
        tokens: Vec<TokenSpec>,
        entry: EntrySpec,
    },
    /// Resolution-time choice. Its result is the selected current objects;
    /// it does not announce targets or mutate the selected objects.
    ChooseObjects {
        player: PlayerRef,
        objects: ObjectSet,
        count: Count,
        constraints: Vec<SelectionConstraint>,
    },
    Draw {
        players: PlayerSet,
        amount: Expr,
    },
    /// Players choose their cards before the simultaneous discard batch.
    Discard {
        players: PlayerSet,
        amount: Expr,
        random: bool,
    },
    GainLife {
        players: PlayerSet,
        amount: Expr,
    },
    LoseLife {
        players: PlayerSet,
        amount: Expr,
    },
    Mill {
        players: PlayerSet,
        amount: Expr,
    },
    AddMana {
        players: PlayerSet,
        mana: ManaProduction,
        restriction: Option<ManaUse>,
    },
    DealDamage {
        source: ObjectRef,
        recipients: Recipients,
        amount: Expr,
        preventable: bool,
    },
    /// Different amounts from one source, all dealt at the same time.
    DealDamageParts {
        source: ObjectRef,
        parts: Vec<DamagePart>,
        preventable: bool,
    },
    /// Each object deals damage at the same time; `ObjectRef::Affected` reads
    /// that object in `recipients` and `amount`.
    DealDamageFromEach {
        sources: ObjectSet,
        recipients: Recipients,
        amount: Expr,
    },
    Destroy {
        objects: ObjectSet,
        regeneration: bool,
    },
    Move {
        objects: ObjectSet,
        destination: Destination,
        entry: EntrySpec,
        face_down: bool,
    },
    Tap {
        objects: ObjectSet,
        tapped: bool,
    },
    /// Each player chooses their own eligible permanents in APNAP order;
    /// selections are collected before one simultaneous sacrifice batch.
    Sacrifice {
        players: PlayerSet,
        objects: ObjectSet,
        choose: Option<Count>,
    },
    /// All choices precede one simultaneous placement event batch.
    PutCounters {
        placements: Vec<CounterPlacement>,
    },
    CreateContinuousEffect {
        source: ObjectRef,
        affected: EffectSubjects,
        changes: Vec<Change>,
        duration: Duration,
        evaluation: EffectEvaluation,
    },
    /// One rules-defined action with no decomposition, from a reviewed row in
    /// `rewrites/primitives.json5`. Unlike `Named`, its operands are bound in the
    /// enclosing scope and none is left unreduced. Counts as complete, and reports
    /// list the records that depend on one.
    Primitive {
        actor: PlayerSet,
        name: String,
        operands: Vec<Operand>,
    },
    /// An action with no reduced form, carried out by `actor`. Never present on
    /// a complete card. It binds no result, so later references cannot reach it.
    Named {
        actor: PlayerSet,
        form: Named,
    },
    /// `separator` splits `objects` into two piles and `chooser` picks one,
    /// which is the result; the other pile is what `objects` still holds.
    SeparatePiles {
        separator: PlayerRef,
        chooser: PlayerRef,
        objects: ObjectSet,
    },
    /// Damage that reaches its recipients the way the rider says.
    DealDamageWith {
        source: ObjectRef,
        recipients: Recipients,
        amount: Expr,
        rider: DamageRider,
    },
    /// One flip of one coin (705). Branches read it through `Condition::CoinFlip`.
    FlipCoin {
        player: PlayerRef,
    },
    /// One result standing for whichever of these branch results was made.
    JoinResults {
        results: Vec<ResultId>,
    },
    /// Changes that stay with the object through zones, for the rest of the
    /// game. Alchemy's "perpetually", as Forge's `Effect` with `Duration$ Permanent`.
    CreatePerpetualEffect {
        objects: ObjectSet,
        changes: Vec<Change>,
    },
    /// An emblem in the command zone with these abilities (114).
    CreateEmblem {
        actor: PlayerSet,
        abilities: Vec<Ability>,
    },
    /// As Forge's `MoveCounter` with `Source$`, `Defined$` and `CounterNum$`.
    MoveCounters {
        actor: PlayerSet,
        from: ObjectSet,
        to: ObjectRef,
        kind: Option<crate::counter::CounterKind>,
        amount: MovedCounters,
    },
    /// Each object gets as many of these counters again; no `kind` means each kind.
    DoubleCounters {
        objects: ObjectSet,
        kind: Option<crate::counter::CounterKind>,
    },
    /// Creates a named card from outside the game in this zone (Alchemy).
    Conjure {
        actor: PlayerSet,
        card: ConjuredCard,
        count: Expr,
        zone: EventZone,
        /// Whose zone, when it isn't `actor`'s.
        owner: Option<PlayerRef>,
        /// Where in a library. None is a random position.
        spot: Option<ConjureSpot>,
        entry: Vec<Operand>,
    },
    /// Choose `count` values of this kind. A read of one goes through
    /// `ObjectFilter::Chosen`, `Change::ChosenType` or `Expr::ChosenNumber`.
    ChooseValue {
        actor: PlayerSet,
        kind: ValueKind,
        /// 0 is any number.
        count: u32,
        options: Vec<Operand>,
    },
    /// Choose one of these power/toughness adjustments.
    ChoosePowerToughness {
        player: PlayerRef,
        options: Vec<(i64, i64)>,
    },
    /// Reveal from the top of `library` until the revealed cards satisfy these
    /// group constraints. The result is every revealed card in order.
    RevealUntilGroup {
        library: PlayerRef,
        constraints: Vec<SelectionConstraint>,
    },
    /// A rules-defined action with a meaning of its own, with what it acts on.
    /// Yields what the row it came from yields.
    Keyword {
        actor: PlayerSet,
        action: KeywordAction,
        object: Option<ObjectRef>,
        other: Option<ObjectRef>,
        amount: Option<Expr>,
        subtype: Option<String>,
    },
    /// The top card of `actor`'s library moves if it is one of `cards`, as
    /// "reveal the top card; if it's a land, put it into your hand".
    MoveTopCard {
        actor: PlayerSet,
        cards: ObjectFilter,
        reveal: bool,
        destination: Destination,
        entry: Vec<Operand>,
    },
    /// Pay as much of this as `actor` chooses, for later "that much".
    PayAnyAmount {
        actor: PlayerSet,
        resource: PaidResource,
    },
    /// `actor` reveals from hand. No `count` is every matching card.
    RevealFromHand {
        actor: PlayerSet,
        cards: ObjectFilter,
        count: Option<Expr>,
        at_random: bool,
    },
    /// One keyword action on each of a set. No `until` is the action's own
    /// duration: for goad, until `actor`'s next turn (CR 701.38a).
    KeywordEach {
        actor: PlayerSet,
        action: KeywordAction,
        objects: ObjectSet,
        until: Option<Duration>,
    },
    /// Copy a spell or ability on the stack `times` times, as Forge's
    /// `CopySpellAbility`. `effects` change the copies.
    CopyOnStack {
        actor: PlayerSet,
        object: ObjectRef,
        times: Expr,
        targets: NewTargets,
        effects: Vec<Operand>,
    },
    /// Change what a spell or ability targets, as Forge's `ChangeTargets`.
    Retarget {
        actor: PlayerSet,
        object: ObjectRef,
        targets: NewTargets,
    },
    /// Each clashing player reveals; `won` runs if `actor` won, else `lost`
    /// (CR 701.30). Inside these, `PlayerRef::ClashOpponent` is the opponent.
    Clash {
        actor: PlayerSet,
        opponents: PlayerSet,
        won: Vec<Program>,
        lost: Vec<Program>,
    },
    /// Draft a card from a named card's spellbook, `times` times, into hand.
    DraftFromSpellbook {
        actor: PlayerSet,
        spellbook: String,
        times: Expr,
    },
    /// CR 701.38: each voter votes for one option. Votes in the same
    /// resolution are counted together by `Expr::Votes` and `Condition::MostVotes`.
    Vote {
        actor: PlayerSet,
        ballot: Ballot,
        secret: bool,
    },
    /// CR 701.40, 701.58: a card becomes a face-down 2/2, with ward {2} if cloaked.
    Manifest {
        actor: PlayerSet,
        source: ManifestSource,
        cloak: bool,
    },
    AbandonScheme {
        actor: PlayerSet,
    },
    /// Puts a sticker on each object, chosen from the sheet unless named.
    PutSticker {
        actor: PlayerSet,
        objects: ObjectSet,
        name_only: bool,
    },
    /// The player may begin the game with this card on the battlefield.
    BeginOnBattlefield {
        actor: PlayerSet,
        flags: Box<Operand>,
    },
}
/// Defines the keyword abilities with one table, so the enum and the rows that
/// lower to it cannot disagree.
macro_rules! mechanics {
    ($($name:ident),* $(,)?) => {
        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
        )]
        pub enum Mechanic {
            $($name),*
        }
        pub const MECHANICS: &[(&str, Mechanic)] =
            &[$((concat!("Rule::", stringify!($name)), Mechanic::$name)),*];
    };
}
mechanics!(
    Convoke,
    StartYourEngines,
    Cascade,
    Exalted,
    Banding,
    Storm,
    Rebound,
    Daybound,
    Nightbound,
    Station,
    Myriad,
    Persist,
    Delve,
    Aftermath,
    Improvise,
    Undying,
    SplitSecond,
    Soulbond,
    Evolve,
    Exploit,
    DoubleTeam,
    Ascend,
    Bargain,
    LivingWeapon,
    Retrace,
    Mentor,
    JobSelect,
    Decayed,
    Extort,
    UmbraArmor,
    Fuse,
    BattleCry,
    Assist,
    BasicSuspend,
    ForMirrodin,
    Sunburst,
    Phasing,
    HiddenAgenda,
    Melee,
    Enlist,
    Riot,
    JumpStart,
    Unleash,
    Conspire,
    Training,
    Ravenous,
    LivingMetal,
    Haunt,
    Increment,
    Provoke,
    Dethrone,
    Storied,
    ReadAhead,
    Ingest,
    Demonstrate,
    Compleated,
    Undaunted,
    Gravestorm,
    Vanishing,
    BloodthirstX,
    CasualtyX,
    BasicMayhem,
    TrampleOverPlaneswalkers,
    Morph,
    Kicker,
    CumulativeUpkeep,
    Unearth,
    Madness,
    Foretell,
    Echo,
    Disguise,
    Bestow,
    Buyback,
    Warp,
    Evoke,
    Escape,
    Ninjutsu,
    Mutate,
    Plot,
    Megamorph,
    Encore,
    Sneak,
    Disturb,
    Offspring,
    Dash,
    Reconfigure,
    Replicate,
    Multikicker,
    Blitz,
    Miracle,
    Embalm,
    Scavenge,
    Mayhem,
    Specialize,
    Squad,
    Emerge,
    Transmute,
    MoreThanMeetsTheEye,
    Outlast,
    Eternalize,
    Freerunning,
    WebSlinging,
    Spectacle,
    Harmonize,
    Recover,
    Fortify,
    WarpX,
    EmergeFromArtifact,
    Transfigure,
    SpecializeFromGraveyard,
    FreerunningX,
    BestowX,
    ForetellX,
    CommanderNinjutsu,
    Bushido,
    Firebending,
    Saddle,
    Soulshift,
    Modular,
    Bloodthirst,
    Casualty,
    Mobilize,
    Dredge,
    Ripple,
    AnnihilatorX,
    Toxic,
    Renown,
    VanishingEnters,
    Annihilator,
    Fading,
    Fabricate,
    Hideaway,
    Rampage,
    Afflict,
    Graft,
    Afterlife,
    Tribute,
    Amplify,
    Poisonous,
    Frenzy,
    Absorb,
    Affinity,
    Champion,
    BandsWithOthers,
    Suspend,
    Reinforce,
    Impending,
    SpliceOnto,
    Devour,
    Prowl,
    Surge,
    SurgeX,
    StartingIntensity,
);

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum KeywordAction {
    Transform,
    Fight,
    CopySpell,
    Investigate,
    Proliferate,
    Amass,
    Explore,
    RingTempts,
    Connive,
    Goad,
    CopyAbility,
    Earthbend,
    Venture,
    ManifestDread,
    Monstrosity,
    Discover,
    GainEnergy,
    BecomeMonarch,
    LoseGame,
    WinGame,
    TakeExtraTurn,
    PhaseOut,
    Prepare,
    TapOrUntap,
    ExchangeControl,
    RollDie(u32),
    EmpowerJace,
    Incubate,
    Adapt,
    Populate,
    Bolster,
    Learn,
    TakeInitiative,
    OpenAttraction,
    GetTickets,
    RemoveFromCombat,
    Flip,
    Awaken,
    Planeswalk,
    Convert,
    Suspect,
    Endure,
    Blight,
    TurnFaceUp,
    TurnFaceDown,
    Detain,
    EndTheTurn,
    TimeTravel,
    Intensify,
    Forage,
    Ascend,
    BecomeDay,
    BecomeNight,
    ChaosEnsues,
    Airbend,
    Saddle,
    Harness,
    Unattach,
    Fateseal,
    Skip(SkippedPart),
    AdditionalPhase(AddedPhases),
    Cipher,
    Recruit,
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum MovedCounters {
    One,
    All,
    AnyNumber,
    Exactly(Expr),
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
}
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum SkippedPart {
    Turn,
    UntapStep,
    DrawStep,
    CombatPhase,
}
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum AddedPhases {
    Combat,
    CombatAndMain,
    Beginning,
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct SearchChoice {
    pub filter: ObjectFilter,
    /// Qualified hidden-zone searches use UpTo (may fail to find). Unqualified
    /// quantities use AsManyAsCan; the distinction is not inferred from whether
    /// matching cards happen to be present (CR 701.23b/d).
    pub count: Count,
    pub constraints: Vec<SelectionConstraint>,
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum SelectionConstraint {
    DifferentNames,
    TotalProperty {
        property: Property,
        comparison: Comparison,
        amount: Expr,
    },
    /// A resolution choice to put a card somewhere must respect prohibitions
    /// on that action (608.2d), e.g. Grafdigger's Cage. Replacing an otherwise
    /// possible movement remains allowed; the replacement still applies later.
    CanMoveTo {
        destination: Destination,
        entry: EntrySpec,
        face_down: bool,
    },
    /// Nonempty selections have a property value common to every object.
    /// For an empty selection the count policy controls whether zero is legal;
    /// this constraint does not impose an unstated minimum selection count.
    CommonProperty(DistinctProperty),
    /// Pairwise different scalar values, not different totals or type sets.
    DifferentProperty(Property),
    /// No two selected objects share a controller.
    DifferentControllers,
    /// No two of these share an owner, as "from each player's graveyard".
    DifferentOwners,
    /// The game picks uniformly, not the choosing player.
    AtRandom,
    /// One object for each filter; an object fills only one of them.
    OneForEach(Vec<ObjectFilter>),
    /// None of these may be what that earlier slot chose: "another target" is a
    /// second use of the word, so 601.2c alone does not keep them apart.
    DifferentFromTarget(Slot),
    /// Each pick has the name of a different one of these objects.
    NamedAfterOneEach(ObjectSet),
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum CardOrder {
    Chosen,
    Preserve,
    Random,
}

/// One production. Choices and numeric observations precede adding its mana;
/// `All` does not split it into independent productions.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ManaProduction {
    /// One reviewed rules-defined form; see `rewrites/primitives.json5`.
    Primitive {
        name: String,
        operands: Vec<Operand>,
    },
    Amount {
        color: ManaColor,
        amount: Expr,
    },
    All(Vec<ManaProduction>),
    ChooseOne(Vec<ManaProduction>),
    Repeat {
        amount: Expr,
        production: Box<ManaProduction>,
        choice: ManaChoiceTiming,
    },
    /// One mana of any type the triggering tap produced (CR 605.5).
    SameTypeAsProduced,
    /// One mana of a color or type that one of these could produce (CR 106.7).
    CouldProduce {
        objects: ObjectSet,
        colors_only: bool,
    },
    CommanderIdentity,
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ManaChoiceTiming {
    Once,
    EachUnit,
}

/// Predicates over the payment receiving these units, not a filter on the
/// permanent that produced them. Other properties of mana need their own data.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ManaUse {
    CastSpell(ObjectFilter),
    ActivateAbility {
        sources: ObjectFilter,
        permanents_only: bool,
    },
    All(Vec<ManaUse>),
    Either(Vec<ManaUse>),
    Not(Box<ManaUse>),
    /// One reviewed rules-defined form; see `rewrites/primitives.json5`.
    Primitive {
        name: String,
        operands: Vec<Operand>,
    },
    /// Mana that stays through step and phase ends until this expires (CR 106.4).
    Persists(Duration),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct TokenSpec {
    pub amount: Expr,
    /// Text-changing effects on the creating spell do not rewrite a predefined
    /// token's rules-defined recipe (CR 111.10, 612.2a).
    pub words: TokenWords,
    pub recipe: TokenRecipe,
}

impl TokenSpec {
    pub fn definition(&self) -> Option<&TokenDefinition> {
        match &self.recipe {
            TokenRecipe::Defined(definition) => Some(definition),
            _ => None,
        }
    }
    pub fn definition_mut(&mut self) -> Option<&mut TokenDefinition> {
        match &mut self.recipe {
            TokenRecipe::Defined(definition) => Some(definition),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum TokenRecipe {
    Defined(TokenDefinition),
    /// A copy of `object` (707.2), with `adjustments` to the copiable values.
    Copy {
        object: ObjectRef,
        adjustments: Vec<Operand>,
    },
    /// One copy of each of `objects`.
    CopyEach {
        objects: ObjectSet,
        adjustments: Vec<Operand>,
    },
    /// The token Oracle gives this name, defined outside the card that makes it.
    Named(String),
    /// A copy of one of `objects`, picked as the effect resolves.
    CopyOne {
        objects: ObjectSet,
        adjustments: Vec<Operand>,
    },
    /// A copy of the Oracle card with this name, which is not in the game.
    CopyNamed {
        name: String,
        adjustments: Vec<Operand>,
    },
    /// A copy of a random Oracle card matching `cards`.
    CopyAtRandom {
        cards: Operand,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum TokenWords {
    CreatingEffect,
    Rules,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct TokenDefinition {
    pub name: TokenName,
    pub colors: Vec<ManaColor>,
    pub supertypes: Vec<SuperType>,
    pub card_types: Vec<CardType>,
    pub subtypes: Vec<SubType>,
    pub power_toughness: Option<(Expr, Expr)>,
    /// Fresh scopes: "this" is the token, and "you" its ability's controller.
    pub abilities: Vec<Ability>,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum TokenName {
    /// CR 111.4: subtype names followed by "Token".
    FromSubtypes,
    Explicit(String),
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct EntrySpec {
    pub tapped: bool,
    pub counters: Vec<(crate::counter::CounterKind, Expr)>,
    pub states: Vec<EntryState>,
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
    /// The ability chosen by `Primitive` Action::ChooseAnAbility.
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
    /// Replaces every card type; supertypes stay and subtypes follow 205.1a.
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
    /// Copiable values come from `object`; `effects` are the exceptions.
    CopyOf {
        object: ObjectRef,
        effects: Box<Operand>,
    },
    AddEnchantmentType(Box<Operand>),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct DamagePart {
    pub recipients: Recipients,
    pub amount: Expr,
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
pub struct SpreeMode {
    pub cost: Cost,
    pub ability: Box<Ability>,
}
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum PaidResource {
    Mana,
    Energy,
    Life,
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
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum TriggerRule {
    TriggersAgain,
    DoesntTrigger,
}
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum CastScope {
    Turn,
    Game,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum PlayerRule {
    /// One reviewed rules-defined form; see `rewrites/primitives.json5`.
    Primitive {
        name: String,
        operands: Vec<Operand>,
    },
    /// Grants a zone permission only. Normal costs, timing, land allowances,
    /// ownership-independent caster choice, and all prohibitions still apply.
    /// Exact object references follow only that zone incarnation; live zone
    /// queries can admit later cards. A top query is re-evaluated each play.
    PlayPermission {
        cards: ObjectSet,
        from: EventZone,
        action: CardPlayAction,
        filter: ObjectFilter,
    },
    Prohibit(PlayerAction),
    /// Count actions actually performed this turn, including before the rule
    /// began applying. This is not an activation limit belonging to the source.
    LimitPerTurn {
        action: PlayerAction,
        amount: Expr,
    },
    HandSize(HandSizeRule),
    /// Adds to the ordinary per-turn allowance, without changing land timing
    /// or the zones from which a land can be played (CR 305.2-3).
    AdditionalLandPlays(Count),
    SpellCost {
        spells: ObjectFilter,
        modifier: SpellCostModifier,
    },
    SpellTiming {
        spells: ObjectFilter,
        timing: SpellTiming,
    },
    Skip(TurnSection),
    /// A rule about whose information is visible; observing is not moving.
    Visibility {
        objects: ObjectSet,
        audience: VisibilityAudience,
    },
    RetainMana {
        colors: Option<Vec<ManaColor>>,
    },
    LifeTotalLocked,
    DamageDoesNotCauseLifeLoss,
    UntapLimit {
        permanents: ObjectFilter,
        amount: Expr,
    },
    /// Player abilities, as distinct from prohibitions or abilities granted
    /// to objects. Their ordinary targeting definitions apply to the player.
    GrantKeyword(PlayerKeyword),
    /// A `PlayPermission` under terms the plain form lacks.
    PlayPermissionWith {
        permission: Box<PlayerRule>,
        terms: PlayTerms,
    },
    /// Spells get abilities as they are cast. `nth_each_turn` says which one.
    SpellsHaveAbilities {
        spells: ObjectFilter,
        from: Option<EventZone>,
        nth_each_turn: Option<Expr>,
        abilities: Box<Operand>,
    },
    /// May spend mana as though it were another color or type.
    SpendManaAs {
        spend_as: SpendAs,
        on: ManaPayment,
    },
    /// What activating an ability matching `abilities` costs. `floor` keeps
    /// at least that much mana in the cost.
    ActivationCost {
        abilities: ObjectFilter,
        change: ActivationCostChange,
        floor: Option<i64>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ManaPayment {
    Anything,
    Spells(ObjectFilter),
    Abilities(ObjectFilter),
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ActivationCostChange {
    Reduce(Box<Operand>),
    Increase(Box<Operand>),
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum PlayerAction {
    Draw,
    GainLife,
    LoseLife,
    WinGame,
    LoseGame {
        reason: Option<GameLossReason>,
    },
    SearchLibraries,
    BecomeMonarch,
    ReceiveCounters(Option<crate::counter::CounterKind>),
    PlayLand {
        from: Option<ZoneScope>,
        filter: ObjectFilter,
    },
    CastSpell {
        from: Option<ZoneScope>,
        filter: ObjectFilter,
    },
    Activate(ActivationFilter),
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum GameLossReason {
    NonpositiveLife,
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum HandSizeRule {
    Unlimited,
    Set(Expr),
    Adjust(Expr),
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum SpellCostModifier {
    Increase(Cost),
    /// Colored reductions normally spill into generic mana (118.7b-c).
    Reduce {
        reductions: Vec<ManaReduction>,
        spill_into_generic: bool,
    },
    /// A floor on the mana component of the final total cost (601.2f).
    Minimum(Expr),
    /// CR 118.9: may be paid rather than the mana cost.
    Alternate {
        cost: PlayCost,
        limit: Option<PlayLimit>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum SpellTiming {
    Flash,
    SorceryOnly,
    OwnTurnOnly,
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
pub enum PlayerKeyword {
    Protection(Box<Protection>),
    HexproofFrom(Box<Protection>),
    Hexproof,
    Shroud,
}
/// The terms of a permission, the parameters Forge's `MayPlay` takes.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct PlayTerms {
    pub limit: Option<PlayLimit>,
    /// Only during these players' turns.
    pub during: Option<PlayerSet>,
    pub cost: PlayCost,
    pub spend_as: Option<SpendAs>,
    /// A card cast this way is exiled instead of going anywhere else.
    pub into_exile: bool,
    pub condition: Option<Condition>,
    /// What a card played this way gets: spell effects, entry flags or a trigger.
    pub effects: Vec<Operand>,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum PlayLimit {
    /// This many cards while the permission lasts.
    Total(i64),
    EachTurn(i64),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum PlayCost {
    Normal,
    Free,
    Alternate(Operand),
    Additional(Operand),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum SpendAs {
    AnyColor,
    AnyType,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum CardPlayAction {
    CastSpell,
    PlayLand,
    Either,
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
}
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum TapPowerCost {
    Crew,
    Saddle,
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
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum AttackDefenders {
    Any,
    Objects(ObjectFilter),
    Players(PlayerSet),
    /// A battle is not included merely because a player controls/protects it.
    PlayersAndPlaneswalkers(PlayerSet),
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ActivationFilter {
    Cycling,
    Any,
    Mana,
    Not(Box<ActivationFilter>),
    All(Vec<ActivationFilter>),
    Either(Vec<ActivationFilter>),
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
pub struct SelectionChoice {
    pub player: PlayerRef,
    pub count: Count,
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
    DistributedShare(Slot),
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
    PlayerLife(PlayerRef),
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
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum HistoryMeasure {
    Occurrences,
    TotalAmount,
    /// How many different sources did it.
    DistinctSources,
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
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum CostKeyword {
    CollectEvidence,
    Blight,
    Forage,
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ManifestSource {
    TopOfLibrary { player: PlayerRef, count: Expr },
    Cards(ObjectSet),
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum PlayerNumber {
    Life,
    CardsInHand,
    Energy,
    /// CR 702.179: absent speed reads zero.
    Speed,
    /// Permanents the player controls that match.
    Permanents(Box<ObjectFilter>),
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
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum Information {
    CurrentOrLastKnown,
    Captured(Binding),
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum Property {
    /// X recorded on this spell, including copied choices. Distinct from an
    /// unrelated activation's announced X or an entering object's cast history.
    SpellX,
    ColorCount,
    Power,
    Toughness,
    ManaValue,
    BasePower,
    BaseToughness,
    Loyalty,
    /// CR 702.161: the starting intensity plus what intensify added.
    Intensity,
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum BlockRole {
    Attacker,
    Blocker,
    /// Whichever of the two is not the object the event was asked about.
    Other,
}
/// How one damage effect differs from dealing `amount` to each recipient.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum DamageRider {
    /// `amount` is a total shared evenly among the recipients.
    DividedEvenly(Rounding),
    /// Damage beyond what is lethal goes to this player instead.
    ExcessTo(PlayerRef),
    /// This much of the damage is prevented.
    Prevented(Expr),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum Rounding {
    Down,
    Up,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(tag = "kind", content = "args")]
pub enum ObjectRef {
    /// One reviewed rules-defined reference; see `rewrites/primitives.json5`.
    Primitive {
        name: String,
        operands: Vec<Operand>,
    },
    /// The object carrying a parameterized keyword. Inside a granted ward
    /// definition this remains the recipient, rather than the granting source.
    KeywordHost,
    /// The card after a matched discard, wherever replacements sent it.
    Discarded,
    /// A singular role in the matched damage occurrence, when its grouping
    /// guarantees one object. Distinct from the source of this ability.
    Damage(DamageSubject),
    /// Index an ordered query, or index zero of a statically singular cost
    /// history. Missing entries select nothing; plural unordered histories
    /// cannot silently choose an arbitrary first object.
    Query {
        objects: Box<ObjectSet>,
        index: u32,
    },
    /// The object currently tested by the innermost object filter.
    Candidate,
    Source,
    /// The current permanent host of this attachment, in the query's view.
    /// Selects nothing when unattached. Last-known attachment information needs
    /// an explicit captured view rather than treating a departed source as live.
    AttachedTo(Box<ObjectRef>),
    /// The object whose characteristic change is currently being evaluated.
    Affected,
    Target {
        slot: Slot,
        index: u16,
    },
    /// The matched event's distinguished object: e.g. the entering/leaving,
    /// attacking, or damaged permanent, or the spell that was cast/copied.
    EventSubject,
    /// A creature of the attacker and blocker paired by a block event.
    BlockPair(BlockRole),
    /// The new object a matched zone change put in its destination (CR 400.7).
    /// Selects nothing once that object leaves the zone.
    EventDestination,
    /// The object chosen by this ability, or by a linked ability of this source.
    Chosen,
    Bound(Binding),
    Result {
        result: ResultId,
        phase: ObjectPhase,
        index: u32,
    },
    /// The spell or ability that made this event's subject a target (603.2).
    EventStackObject,
    /// The permanent chosen for a damage-recipient slot. A player chosen
    /// there leaves this undefined, so tests of it are false.
    RecipientObject {
        slot: Slot,
        index: u16,
    },
    /// The second object of a two-object event: the host of an attachment,
    /// the crewed vehicle, the exploited creature.
    EventOther,
    /// The object a linked ability of the source made.
    MadeHere,
    /// What this player picked in a loop that gave every player a choice.
    ChosenBy {
        result: ResultId,
        chooser: PlayerRef,
    },
    /// The permanent whose effect gave the source this ability, or made it.
    Granter,
    /// The creature `object` is paired with (CR 702.95b), if any.
    PairedWith(Box<ObjectRef>),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ObjectPhase {
    Before,
    After,
}

/// Namespaces are relative to the current ability, never a global last payment.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum PaymentRef {
    Activation,
    /// Index of a cost declaration among this card's top-level abilities.
    /// Only the spell resolution frame can access this casting history.
    Casting {
        ability: u32,
    },
    /// An earlier PayCost instruction in this lexical result frame.
    Resolution(ResultId),
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum CostObjectView {
    Inputs,
    Performed,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(tag = "kind", content = "args")]
pub enum ObjectSet {
    /// Original identities in a LookAt result; never re-run its source query.
    Observed(ResultId),
    /// Preserve the input sequence; ordinary filtering does not reveal cards.
    Matching {
        objects: Box<ObjectSet>,
        filter: ObjectFilter,
    },
    /// Preserve the left input's sequence and original object identities.
    Difference {
        objects: Box<ObjectSet>,
        excluding: Box<ObjectSet>,
    },
    /// Distinct objects occupying this role in the matched damage occurrence.
    Damage(DamageSubject),
    /// Private library/graveyard queries retain top-to-bottom order; filtering
    /// preserves that order. Several players' zones have no shared ordering.
    Zone {
        scope: ZoneScope,
        kind: ObjectKind,
        filter: ObjectFilter,
    },
    /// Set union, with duplicates removed; this is not an iteration.
    Union(Vec<ObjectSet>),
    Take {
        objects: Box<ObjectSet>,
        count: Expr,
        from: SequenceEnd,
    },
    /// Actual transitions, not all objects originally requested by an effect.
    /// Empty when the action sat in an optional body or cost that never ran.
    Transition {
        result: ResultId,
        phase: ObjectPhase,
    },
    One(ObjectRef),
    Targets(Slot),
    Battlefield(ObjectFilter),
    /// Intersect a reference/result set with current zone membership. A saved
    /// object reference alone does not imply the object is still on the field.
    InZone {
        objects: Box<ObjectSet>,
        zone: EventZone,
    },
    /// Original object identities selected by a preceding choice or search.
    Selected(ResultId),
    /// Objects returned by a preceding token creation; distinct from a choice.
    Created(ResultId),
    /// Object history for one exact payment component. An unpaid component,
    /// unchosen alternative, or omitted optional cost has an empty history. `Inputs`
    /// retains the designated original identities and their expected-zone/LKI
    /// view; `Performed` retains participants of the corresponding rules action
    /// after replacement processing. Paying a cost does not imply the action
    /// occurred (118.11-12). Before/After distinguish original and resulting
    /// incarnations, never a fresh zone query. Input histories use Before.
    /// During announcement, Inputs denotes the casting/activation proposal's
    /// planned inputs (601.5); payment must use those same choices. Afterward
    /// it denotes the recorded identities, so changes to a live selector cannot
    /// redirect it. Performed action history is unavailable before payment.
    /// A copied spell/ability uses the original's cost objects (707.10), even
    /// though the copy pays no cost; a resolution-time payment is its own frame.
    CostObjects {
        payment: PaymentRef,
        component: Vec<u32>,
        view: CostObjectView,
        phase: ObjectPhase,
    },
    /// Every object able to be the source of damage (CR 609.7a) that matches.
    DamageSources(ObjectFilter),
    /// The objects an earlier continuous effect in this resolution applied to.
    Affected(ResultId),
    /// Every object of a one-or-more event, grouped under one trigger (603.2c).
    EventGroup,
    /// The top card of each of these players' libraries.
    LibraryTops(PlayerSet),
    /// Objects a preceding choice could have selected and did not, by the
    /// candidates it offered; each player's own in a per-player choice.
    Unselected(ResultId),
    /// What this player's share of an action moved: their run of a `ForEachPlayer`
    /// body, or the part of a batch they performed.
    MovedBy {
        result: ResultId,
        phase: ObjectPhase,
        player: PlayerRef,
    },
    /// The copies a `CopyCards` made, wherever they were made.
    Copies(ResultId),
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ZoneScope {
    Shared(EventZone),
    Players { zone: EventZone, players: PlayerSet },
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum SequenceEnd {
    Top,
    Bottom,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(tag = "kind", content = "args")]
pub enum PlayerRef {
    /// One reviewed rules-defined reference; see `rewrites/primitives.json5`.
    Primitive {
        name: String,
        operands: Vec<Operand>,
    },
    /// "Any player may": asked in turn order, the first who agrees (101.4).
    FirstWilling(Box<PlayerSet>),
    /// Controller of the targeting spell/ability that caused a ward trigger.
    /// Bound only inside that keyword's cost recipe, never to its controller.
    WardPayer,
    /// Bound only while applying a player continuous effect, never in its
    /// subject selector or an ordinary resolving instruction.
    Affected,
    EnchantedBy(Box<ObjectRef>),
    /// The player proposing the attack/block declaration, including the
    /// defending player paying a blocking tax on an attacking creature.
    DeclaringPlayer,
    /// The stack ability/spell's controller; for a static definition, its
    /// source's controller or owner when it has none (CR 109.5 / 113.8).
    AbilityController,
    Active,
    /// Read the referenced object's controller, using last-known information
    /// after that object leaves its zone; never substitute the new card's owner.
    ControllerOf(Box<ObjectRef>),
    OwnerOf(Box<ObjectRef>),
    /// The recorded caster, independent of the object's current controller.
    /// A copy or an incarnation with no casting has no caster. While checking
    /// a casting proposal, reads that proposal's announcing player instead.
    CasterOf(Box<ObjectRef>),
    Target {
        slot: Slot,
        index: u16,
    },
    /// The cast/copy event's actor or the turn event's turn player. Other
    /// matchers do not implicitly bind the source's controller as this player.
    EventSubject,
    /// The player an attacking object is attacking, or that controls the
    /// planeswalker it is attacking, using last-known information.
    DefendingPlayerOf(Box<ObjectRef>),
    /// The player chosen by this ability, or by a linked ability of this source
    /// (CR 607.2). The choosing form is a primitive, so no result binds it yet.
    Chosen,
    Bound(Binding),
    /// The second player of a two-player event: the attacked player.
    EventOtherPlayer,
    /// "An opponent": one of these players, picked by the controller as this
    /// resolves.
    OneOf(Box<PlayerSet>),
    /// Who chose this spell's or ability's modes, when it isn't the controller.
    ModeChooser,
    /// This recipient if it is a player, or else its controller.
    PlayerOrControllerOf(Box<Recipients>),
    /// The opponent `actor` clashed with, inside `Operation::Clash`.
    ClashOpponent,
    /// The opponent a gift was promised to.
    GiftedPlayer,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(tag = "kind", content = "args")]
pub enum PlayerSet {
    DamageRecipients,
    /// Players in every listed set.
    Intersection(Vec<PlayerSet>),
    /// Every player for whom `condition` holds, with that player bound.
    Matching {
        binding: Binding,
        condition: Box<Condition>,
    },
    /// Set union, with duplicate players removed before a batch.
    Union(Vec<PlayerSet>),
    One(PlayerRef),
    All,
    OpponentsOf(PlayerRef),
    OthersThan(PlayerRef),
    Targets(Slot),
    /// Dealt damage by this earlier instruction of the same resolution.
    DamagedBy(ResultId),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(tag = "kind", content = "args")]
pub enum Recipients {
    Objects(ObjectSet),
    Players(PlayerSet),
    Targets(Slot),
    Union(Vec<Recipients>),
    /// The player, planeswalker or battle this creature is attacking (506.2).
    AttackedBy(ObjectRef),
    /// One of the things a plural slot targeted.
    Target {
        slot: Slot,
        index: u16,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(tag = "kind", content = "args")]
pub enum ObjectFilter {
    /// Exiled by an ability of this object, as linked by 607.
    ExiledWith(ObjectRef),
    /// One reviewed rules-defined form; see `rewrites/primitives.json5`.
    Primitive {
        name: String,
        operands: Vec<Operand>,
    },
    /// Evaluate a reusable condition with Candidate bound to this object.
    Condition(Box<Condition>),
    /// An actual casting event, retained by the permanent the spell becomes.
    /// Both absent means "was cast". A copied spell was not cast. Origin is
    /// the original zone/owner, not current stack membership or current owner.
    /// This data is not inherited after a later zone change or blink. During
    /// casting legality/payment checks, test the current casting proposal's
    /// actor/origin, before it becomes a completed casting event.
    Cast {
        by: Option<PlayerSet>,
        from: Option<ZoneScope>,
    },
    /// The zone from which this incarnation most recently entered the
    /// battlefield, independently of whether it was cast. A reanimated card
    /// can match a graveyard entry without having been cast from there.
    EnteredFrom(ZoneScope),
    HasKeywordFamily(KeywordFamily),
    /// At least one actual target belongs to this set. For casting-cost rules
    /// inspect the completed announcement, before costs are paid.
    TargetsAny(Box<Recipients>),
    /// What a spell or ability on the stack is.
    OnStack(StackTrait),
    /// An ability whose source matches.
    AbilityOf(Box<ObjectFilter>),
    /// Includes defined abilities even while their functioning zone is inactive.
    HasActivatedAbility(ActivationFilter),
    InSet(Box<ObjectSet>),
    Name(String),
    InZone(ZoneScope),
    /// Blocked by this object.
    BlockedByObject(ObjectRef),
    /// Attacking one of these.
    AttackingDefender(Box<AttackDefenders>),
    /// Has the value of this kind chosen by this ability or as this object entered.
    Chosen(ValueKind),
    IsCard,
    Any,
    All(Vec<ObjectFilter>),
    Either(Vec<ObjectFilter>),
    Not(Box<ObjectFilter>),
    CardType(CardType),
    Supertype(SuperType),
    Subtype(SubType),
    Color(ManaColor),
    Colorless,
    HasKeyword(Keyword),
    ControlledBy(PlayerSet),
    OwnedBy(PlayerSet),
    SameObject(ObjectRef),
    /// Has a name in common with this object (CR 201.2a).
    SharesNameWith(ObjectRef),
    Property {
        property: Property,
        comparison: Comparison,
        value: Expr,
    },
    Counters {
        kind: Option<crate::counter::CounterKind>,
        comparison: Comparison,
        value: Expr,
    },
    Token,
    Tapped,
    Attacking,
    Blocking,
    /// A creature with counters, Equipment, or an Aura controlled by its
    /// controller (the rules definition of modified).
    Modified,
    /// Dealt damage by this earlier instruction of the same resolution.
    DamagedBy(ResultId),
    /// Has at least one value of this property in common with the object.
    Shares {
        property: DistinctProperty,
        with: ObjectRef,
    },
    /// Has the same name as any of these objects.
    SharesNameWithAny(Box<ObjectSet>),
    /// Has the same name as a different object among these, never itself.
    SharesNameWithAnother(Box<ObjectSet>),
    /// Dealt more than lethal damage by this earlier instruction.
    DealtExcessBy(ResultId),
    /// Reached its current zone this turn, from this zone if one is named.
    ArrivedThisTurn {
        from: Option<EventZone>,
    },
    /// Created or put onto the battlefield by an ability of this object.
    MadeBy(ObjectRef),
    /// Has a value of this property in common with any of these objects.
    SharesWithAny {
        property: DistinctProperty,
        with: Box<ObjectSet>,
    },
    /// Was attached to this object as it last existed.
    WasAttachedTo(ObjectRef),
    /// A state the rules define for an object, which no other filter reads.
    State(ObjectState),
    AttachedTo(Box<ObjectFilter>),
    /// Blocking this attacker, not merely blocking.
    BlockingObject(ObjectRef),
    /// The spell cast at `ordinal` among the `spells` cast in `scope`, by
    /// `by` when that is named. Counts earlier casts, not this ability's life.
    NthCast {
        ordinal: Expr,
        spells: Box<ObjectFilter>,
        by: Option<PlayerSet>,
        scope: CastScope,
    },
    /// Was kicked with this kicker (CR 702.33d).
    KickedWith(Vec<Operand>),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum KeywordFamily {
    Ward,
    Protection,
    Landwalk,
    Hexproof,
    Toxic,
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
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum Count {
    Exactly(Expr),
    UpTo(Expr),
    AnyNumber,
    AtLeast(Expr),
    AsManyAsCan(Expr),
    /// `each` applied separately to what each of these players controls.
    PerPlayer {
        players: PlayerSet,
        each: Box<Count>,
    },
    Between {
        minimum: Expr,
        maximum: Expr,
    },
}

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
        binding: Binding,
        condition: Box<Condition>,
    },
    /// CR 118.12: the optional cost was chosen, or the mandatory cost was
    /// started. Replacement of the payment's events does not turn this false.
    PaymentChosen(ResultId),
    PlayerIsActive(PlayerRef),
    All(Vec<Condition>),
    Either(Vec<Condition>),
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
        comparison: Comparison,
        right: Expr,
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
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum CoinOutcome {
    Won,
    Lost,
    Heads,
    Tails,
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
pub struct Cost {
    pub parts: Vec<CostPart>,
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum CostPart {
    /// Copy this object's actual mana-cost symbols at payment, including X
    /// and colored/hybrid/Phyrexian symbols, not its numeric mana value.
    /// An absent mana cost is unpayable, distinct from a cost of zero.
    ManaCostOf(ObjectRef),
    /// Choose distinct inputs before one simultaneous movement cost payment.
    /// Counts are strict: no as-many-as-possible effect choices.
    Move {
        selections: Vec<CostObjectSelection>,
        destination: Destination,
        entry: EntrySpec,
        face_down: bool,
    },
    /// One option is announced as part of choosing how to pay this cost.
    Choice(Vec<Cost>),
    RemoveCounters {
        removals: Vec<CounterRemoval>,
    },
    Mana(Vec<ManaSymbol>),
    TapSymbol,
    Life(Expr),
    Loyalty(Expr),
    Sacrifice {
        objects: ObjectSet,
        choose: Option<Count>,
    },
    Discard {
        count: Expr,
        filter: ObjectFilter,
        random: bool,
    },
    Draw {
        count: Expr,
    },
    Mill {
        count: Expr,
    },
    /// One payment component, not a resolving tap/untap effect. Selected
    /// permanents must have the opposite state before payment (118.3,
    /// 107.5-6); setting an already-matching state does not pay the cost.
    /// Inputs chosen within this component are distinct, and counts are
    /// strict. Changes are performed as one component of paying the cost.
    /// `symbol` denotes {T}/{Q} and retains the creature activation restriction
    /// in 302.6/602.5a. It permits only the implicit source/keyword host with no
    /// object choice. Ordinary object tapping does not acquire that restriction
    /// or an unstated control requirement. TapSymbol is the compact {T} form.
    ChangeTapState {
        selections: Vec<CostObjectSelection>,
        tapped: bool,
        symbol: bool,
    },
    /// Qualifications on the chosen inputs to one payment component, checked
    /// together before paying it. A property total is computed over those
    /// inputs, not over every eligible object and not after movement.
    Constrained {
        part: Box<CostPart>,
        constraints: Vec<SelectionConstraint>,
    },
    /// One reviewed rules-defined form; see `rewrites/primitives.json5`.
    Primitive {
        name: String,
        operands: Vec<Operand>,
    },
    /// Reveal cards from the payer's hand; they stay there.
    Reveal {
        count: Count,
        filter: ObjectFilter,
        random: bool,
    },
    /// Pay `part` once per `times`, counted when the cost is paid.
    Repeated {
        part: Box<CostPart>,
        times: Expr,
    },
    /// "You may have that player draw a card": the player performs the body.
    PlayerAction {
        player: PlayerRef,
        body: Program,
    },
    Energy(Expr),
    /// Mana with an {X} set here, or by the payer while paying when `x` is None.
    ManaWithX {
        mana: Vec<ManaSymbol>,
        x: Option<Expr>,
    },
    /// "Waterbend {N}": tapped artifacts and creatures pay for generic mana.
    Waterbend(Vec<ManaSymbol>),
    PutCounters(Vec<CounterPlacement>),
    /// Tap any number of creatures with at least this total power.
    Teamwork(i32),
    /// A keyword action done as a cost, `amount` times where it counts.
    Keyword {
        action: CostKeyword,
        amount: Option<Expr>,
    },
    DiscardHand,
    /// CR 701.43: the object does not untap during its controller's next untap step.
    Exert(ObjectRef),
    Unattach(ObjectRef),
    Unspecialize(ObjectRef),
    /// CR 701.61: choose such a permanent you control or reveal such a card.
    Behold {
        count: Expr,
        what: Box<Operand>,
    },
    /// The top card of the payer's library is revealed and has to match.
    RevealTop(ObjectFilter),
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct CostObjectSelection {
    pub objects: ObjectSet,
    pub choose: Option<Count>,
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
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ManaColor {
    White,
    Blue,
    Black,
    Red,
    Green,
    Colorless,
    /// The color chosen as this resolved.
    Chosen,
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ManaSymbol {
    Generic(u32),
    Color(ManaColor),
    Snow,
    X,
    Hybrid(ManaColor, ManaColor),
    Phyrexian(Vec<ManaColor>),
    MonocoloredHybrid(ManaColor),
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
/// Match actual positive damage, after prevention/replacement, using event
/// information (including a departed source's last-known characteristics).
/// Group only within one simultaneous damage event; never across instructions
/// or separate first-strike/regular combat damage events.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct DamageEvent {
    pub source_kind: ObjectKind,
    pub sources: ObjectFilter,
    pub recipients: DamageRecipients,
    pub kind: DamageKind,
    pub grouping: DamageGrouping,
    /// The role meant by the legacy distinguished object reference. A grouped
    /// role does not acquire a singular reference just because it has a subject.
    pub subject: DamageSubject,
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum DamageRecipients {
    Any,
    /// The one player or object a damage-recipient target refers to.
    Target {
        slot: Slot,
        index: u16,
    },
    Objects(ObjectFilter),
    Players(PlayerSet),
    ObjectsAndPlayers {
        objects: ObjectFilter,
        players: PlayerSet,
    },
    /// Every target in this slot.
    Targets(Slot),
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum DamageKind {
    Any,
    Combat,
    Noncombat,
    /// Only what went past lethal counts, and an amount reads that excess.
    Excess,
    ExcessNoncombat,
    /// Damage a prevention effect stopped, and an amount reads what it stopped.
    Prevented,
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum DamageGrouping {
    /// One occurrence for each distinct source/recipient pair.
    SourceRecipient,
    Source,
    Recipient,
    Batch,
}
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum DamageSubject {
    Source,
    Recipient,
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
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum DiscardKind {
    Any,
    Cycling,
}
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum DiscardGrouping {
    Card,
    Player,
}

impl Program {
    pub fn empty() -> Self {
        Self {
            instructions: Vec::new(),
        }
    }
}

impl Announcement {
    pub fn empty() -> Self {
        Self {
            targets: Vec::new(),
            modes: None,
        }
    }
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

impl Expr {
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
