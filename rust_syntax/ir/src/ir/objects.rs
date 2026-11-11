//! Objects in the normalized semantic model.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum AttackDefenders {
    Any,
    Objects(ObjectFilter),
    Players(PlayerSet),
    /// A battle is not included merely because a player controls/protects it.
    PlayersAndPlaneswalkers(PlayerSet),
    /// The defending player, including the protector of a battle (CR 508.5).
    DefendingPlayers(PlayerSet),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum EffectSubject {
    Objects(ObjectSet),
    /// Spells that match, including ones cast later.
    EachSpell(ObjectFilter),
    /// The next matching spell this player casts.
    NextSpell {
        spells: ObjectFilter,
        caster: PlayerRef,
        this_turn: bool,
        from: Option<EventZone>,
    },
    Players(PlayerSet),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum Information {
    CurrentOrLastKnown,
    Captured(LoopBinding),
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
    TotalPowerAndToughness,
    CreatureTypeCount,
    CardTypeCount,
    ColorSymbolCount,
    /// Cards exiled with this object.
    ExiledCount,
    /// Creatures that crewed this Vehicle this turn.
    CrewCount,
    SubtypeCount,
    SupertypeCount,
    ToxicValue,
    BushidoValue,
    RegenerationsThisTurn,
    GenericManaInCost,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum BlockRole {
    Attacker,
    Blocker,
    /// Whichever of the two is not the object the event was asked about.
    Other,
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
    AbilityHolder,
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
        slot: TargetSlot,
        index: u16,
    },
    /// The matched event's distinguished object: e.g. the entering/leaving,
    /// attacking, or damaged permanent, or the spell that was cast/copied.
    EventSubject,
    /// A creature of the attacker and blocker paired by a block event.
    BlockPair(BlockRole),
    /// The first public-zone incarnation produced by the matched zone-changing
    /// event (CR 400.7e), including a sacrifice whose destination was replaced.
    /// Selects nothing once that incarnation leaves its destination; never
    /// follows a second zone change or a move into a hidden zone.
    EventDestination,
    /// The object chosen by this ability, or by a linked ability of this source.
    Chosen,
    Bound(LoopBinding),
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
        slot: TargetSlot,
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
    GrantingObject,
    /// The creature `object` is paired with (CR 702.95b), if any.
    PairedWith(Box<ObjectRef>),
    /// The one object this spell targets.
    SoleTargetOf(Box<ObjectRef>),
    /// What a preceding instruction named, whatever kind of result it holds.
    Recorded {
        result: ResultId,
        by: Option<PlayerRef>,
    },
    /// What an earlier step acted on, read from the card's words.
    Earlier {
        what: EarlierObjects,
        by: Option<PlayerRef>,
    },
    /// The permanent an ability on the stack came from.
    SourceOf(Box<ObjectRef>),
    /// "That" object: whatever the row around this one names.
    Scoped,
    RingBearer(PlayerRef),
    HauntedBy(Box<ObjectRef>),
    /// The nth object this player chose.
    NthChosen {
        index: u32,
        by: Option<PlayerRef>,
    },
    /// The other one of `among`, given the ones `excluding` names. Either of
    /// them when `excluding` names them all, as Forge's Spellweaver Helix.
    TheOther {
        among: Box<ObjectSet>,
        excluding: Box<ObjectSet>,
    },
    /// The object at this end of a set that holds an order of its own: a pile
    /// shuffled as it was exiled, or a history, whose `Top` is its earliest.
    AtEnd {
        objects: Box<ObjectSet>,
        end: SequenceEnd,
    },
    /// The Aura's first owner's-graveyard incarnation after its watched enchanted
    /// permanent left the battlefield (CR 400.7f). Selects nothing after that
    /// incarnation moves again, or when a replacement sent the Aura elsewhere.
    /// The Aura and host name their original battlefield objects, independently
    /// of the source's current attachment or the host's destination incarnation.
    AuraAfterEnchantedLeft {
        aura: Box<ObjectRef>,
        host: Box<ObjectRef>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ObjectPhase {
    Before,
    After,
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
    Targets(TargetSlot),
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
    /// The objects of the enclosing `ForEachObject` this payment was not made for.
    Unpaid(ResultId),
    /// A reviewed set with no reduction.
    Primitive {
        name: String,
        operands: Vec<Operand>,
    },
    /// What a preceding instruction named, whatever kind of result it holds.
    Recorded(ResultId),
    /// Every matching card or object, whatever zone it is in.
    AnyZone(ObjectFilter),
    /// Cards a player owns outside the game (CR 400.11).
    OutsideTheGame(ObjectFilter),
    /// Some of these at random.
    /// Draft producer/old-decoder form; canonical resolution uses Prepared.
    #[doc(hidden)]
    #[serde(rename = "Random")]
    DraftRandom {
        objects: Box<ObjectSet>,
        count: Expr,
    },
    /// Some of these, which `chooser` picks as the instruction resolves.
    /// Draft producer/old-decoder form; canonical resolution uses Prepared.
    #[doc(hidden)]
    #[serde(rename = "Chosen")]
    DraftChosen {
        chooser: PlayerSet,
        objects: Box<ObjectSet>,
        count: Count,
    },
    Earlier {
        what: EarlierObjects,
        by: Option<PlayerSet>,
    },
    /// "Those" objects: whatever the row around this one names.
    Scoped,
    /// The cards spliced onto this spell as it was cast (CR 702.47).
    SplicedOnto(ObjectRef),
    /// What this spell or ability targets, as it is being cast or on the stack.
    TargetsOf(ObjectRef),
    /// The subjects of an event in a window as they last existed, in the order
    /// it happened, as Forge's `ThisTurnCast`.
    FromHistory {
        event: Box<EventSpec>,
        window: HistoryWindow,
    },
    /// A selection in the nearest enclosing WithSelections scope.
    Prepared(SelectionId),
    /// Exact exiled incarnations linked at this object's creation. The link is
    /// noncopiable object history and survives its source leaving/ceasing to
    /// exist through LKI. Copies of the object/ability acquire no link. A card
    /// that left exile and returned is a new incarnation and is not included.
    CreationLinkedCards(ObjectRef),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ZoneScope {
    Shared(EventZone),
    Players { zone: EventZone, players: PlayerSet },
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
    AllOf(Vec<ObjectFilter>),
    AnyOf(Vec<ObjectFilter>),
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
        test: NumberTest,
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
    /// Has something matching attached to it.
    HasAttached(Box<ObjectFilter>),
    AttachedToPlayer(PlayerRef),
    /// Has the highest or lowest value of `property` among `among`.
    Extreme {
        among: Box<ObjectSet>,
        property: Property,
        highest: bool,
    },
    /// Mana from a source that matches was spent to cast this spell.
    ManaSpentFrom(Box<ObjectFilter>),
    /// Shares at least one color with this object.
    SharesColorWith(ObjectRef),
    /// The filter chosen by a `ChooseValue` of `ValueKind::Filter`.
    ChosenFilter,
    /// Shares a color with any of a set, as protection from the colors of permanents.
    SharesColorWithAny(Box<ObjectSet>),
    /// What happened to this object in a window, as Forge's `attackedThisTurn`.
    History {
        event: ObjectHistory,
        window: ObjectWindow,
    },
    /// Mana spent to cast it, narrowed by color, source or who paid.
    ManaSpent {
        color: Option<ManaColor>,
        sources: Option<Box<ObjectFilter>>,
        payers: Option<PlayerSet>,
        comparison: Comparison,
        value: Expr,
    },
    /// Cast for this alternative cost.
    AlternateCostPaid(Vec<Operand>),
    /// A printed mana cost, symbol for symbol.
    ManaCostIs(Vec<Operand>),
    ManaSymbolInCost(ManaColor),
    BlockingAny(Box<ObjectFilter>),
    BlockedByAny(Box<ObjectFilter>),
    PairedWith(Box<ObjectFilter>),
    BandedWith(ObjectRef),
    AttachedCount {
        attachments: Box<ObjectFilter>,
        comparison: Comparison,
        value: Expr,
    },
    ProtectedBy(PlayerSet),
    NameFirstPrintedIn(FirstPrinting),
    InStartingDeck(PlayerSet),
    /// Every target it could choose matches.
    CanTargetOnly(Box<ObjectFilter>),
    /// Resolving it would destroy a permanent that matches.
    WouldDestroy(Box<ObjectFilter>),
    TargetableBy(ObjectRef),
    CouldEnchant(Box<ObjectSet>),
    CouldProduce(Box<Operand>),
    CouldProduceManaOf(ObjectRef),
    ExiledBy {
        player: PlayerRef,
        with: Option<ObjectRef>,
        this_turn: bool,
    },
    /// Of the type this ability noted.
    Noted(ValueKind),
    ChosenBy {
        kind: ValueKind,
        player: PlayerRef,
    },
    /// Chosen as this object entered or resolved, for the object itself.
    ChosenOn {
        kind: ValueKind,
        object: ObjectRef,
    },
    DraftNoted(DraftNote),
    /// In a chosen pile, or the pile chosen for this object.
    InPile(Option<ObjectRef>),
    /// In the chosen sector, or this object's sector.
    InSector(Option<ObjectRef>),
    NthChosen(u32),
    AttackingSame(ObjectRef),
    CastFromAmong(Box<ObjectSet>),
    /// Shares one of these card types with this object.
    SharesTypeFrom {
        types: Vec<CardType>,
        with: ObjectRef,
    },
    SharesMostCommonColor(Box<ObjectSet>),
    /// A spell or ability with this many targets.
    TargetCount(Comparison, Expr),
    /// Has the creature type most common in this player's library.
    MostCommonCreatureTypeIn(PlayerRef),
    /// Shares a color with this mana. Colorless mana shares none.
    SharesColorWithMana(ManaOccurrence),
    /// Its mana cost could be paid with some or all of this mana.
    ManaCostPayableWith(ManaOccurrence),
    /// A target of a spell or ability on the stack that matches.
    TargetedBy(Box<ObjectFilter>),
    /// Has a color outside the color identity of this player's commander.
    ColorOutsideCommanderIdentity(PlayerRef),
    /// Candidate is a host that this Aura could legally enchant. Unlike
    /// CouldEnchant, the candidate is not the Aura. This does not target.
    CouldBeEnchantedBy(ObjectRef),
}

/// Something that happened to an object. See `ObjectFilter::History`.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ObjectHistory {
    Attacked,
    AttackedDefender(Box<ObjectFilter>),
    Blocked,
    WasBlocked,
    Regenerated,
    AbilityActivated,
    Entered,
    Drawn(Option<PlayerSet>),
    Discarded,
    Milled,
    Exiled,
    ExiledFromGraveyard(PlayerSet),
    PutIntoGraveyard,
    DealtCombatDamage,
    DamagedBy(ObjectRef),
    Enlisted(Box<ObjectFilter>),
    Convoked(ObjectRef),
    Exploited(ObjectRef),
    TappedToPayFor(ObjectRef),
    DiedOrPutFromBattlefield,
    CycledOrDiscarded,
    CountersPutBy {
        kind: crate::counter::CounterKind,
        by: PlayerSet,
    },
    CountersPutWith {
        kind: crate::counter::CounterKind,
        with: ObjectRef,
    },
    CountersRemovedWith {
        kind: crate::counter::CounterKind,
        with: ObjectRef,
    },
    PhasedOutWith(ObjectRef),
    Cycled,
    Sacrificed,
    Surveilled,
    /// Put into a hand, as Forge's `ThisTurnEnteredYourHand`.
    PutIntoHand,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ObjectWindow {
    ThisTurn,
    ThisCombat,
    ThisGame,
    LastTurnOf(PlayerRef),
    SinceLastUpkeepOf(PlayerRef),
    SinceLastTurnOf(PlayerRef),
    LastCombatOf(PlayerRef),
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum FirstPrinting {
    ArabianNights,
    Antiquities,
    Homelands,
}

impl ObjectFilter {
    /// Membership of the objects named by one result, independent of spelling.
    pub fn named_result(&self) -> Option<&ResultId> {
        match self {
            Self::InSet(objects) => match objects.as_ref() {
                ObjectSet::Recorded(result) => Some(result),
                _ => None,
            },
            _ => None,
        }
    }
}

impl ObjectSet {
    /// The shared battlefield contains permanents, including tokens.
    pub fn battlefield(filter: ObjectFilter) -> Self {
        crate::battlefield_query!(filter)
    }

    pub fn battlefield_filter(&self) -> Option<&ObjectFilter> {
        match self {
            crate::battlefield_query!(filter) => Some(filter),
            _ => None,
        }
    }
}
