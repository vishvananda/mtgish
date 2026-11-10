//! Costs in the normalized semantic model.
use super::*;

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
    /// Costs this much less for each time an optional cost was paid.
    ReduceForEachPaid {
        reductions: Vec<ManaReduction>,
        cost: Cost,
    },
    /// May be cast as though it had flash, with a rider when that mattered.
    FlashWithRider(Box<Operand>),
    /// The commander tax is paid in life, this much for each previous cast.
    TaxPaidAsLife(Expr),
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

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum PaidResource {
    Mana,
    Energy,
    Life,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum CastScope {
    Turn,
    Game,
}

/// The keywords a cost change or a play permission names.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum CostedKeyword {
    Adventure,
    Bestow,
    Blitz,
    Boast,
    Buyback,
    Crew,
    Cycling,
    Dash,
    Echo,
    Equip,
    Flashback,
    Foretell,
    Morph,
    Mutate,
    Ninjutsu,
    Plot,
    PowerUp,
    Sneak,
    Suspend,
    TurnFaceUp,
    Unearth,
    Unlock,
    Warp,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum KeywordCostChange {
    Reduce(Box<Operand>),
    Increase(Box<Operand>),
    Alternate(Box<Operand>),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum GameDecision {
    HowCreaturesBlock(ObjectFilter),
    WhichCreaturesBlock,
    WhichCreaturesAttack,
    HowPlayersVote,
    CombatDamageOf(ObjectFilter),
    /// The player makes the choices of these players as they search.
    SearchesOf(PlayerSet),
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum TargetingKeyword {
    Hexproof,
    Shroud,
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
    /// Reduced by a symbol {X}, where X is `times`.
    ReduceX {
        amount: Box<Operand>,
        times: Expr,
    },
    /// An extra cost to pay.
    Additional(Cost),
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
    AttackWith(ObjectFilter),
    BlockWith(ObjectFilter),
    /// Attacking this player or permanent.
    AttackInto(Recipients),
    /// Paying life, for what it pays for.
    PayLife(ManaPayment),
    /// Sacrificing these, for what it pays for when that is named.
    Sacrifice {
        objects: ObjectFilter,
        paying: Option<ManaPayment>,
    },
    /// Playing a matching card by any means.
    PlayCard(ObjectFilter),
    VentureIntoDungeon,
    ExileObjects(ObjectFilter),
    DiscardCards,
    BeTargeted(ObjectFilter),
    /// Casting a spell from anywhere but this zone.
    CastOutside(EventZone),
    /// Attacking these defenders.
    AttackDefender(AttackDefenders),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum GameLossReason {
    NonpositiveLife,
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
    /// A cost the caster may pay to take this much off a spell's mana cost.
    PayToReduce {
        cost: Box<Operand>,
        mana: Box<Operand>,
    },
    /// Changes the cost once for each of `per`.
    Scaled {
        change: ActivationCostChange,
        per: CostScale,
    },
    /// Alternate cost under a limit, only during these players' turns.
    AlternateDuring {
        cost: PlayCost,
        limit: Option<PlayLimit>,
        during: PlayerSet,
    },
    /// An extra cost the caster may pay, for what the spell then gets.
    OptionalAdditional {
        cost: Box<Operand>,
        effects: Box<Operand>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum CostScale {
    Times(Expr),
    EachTarget,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum SpellTiming {
    Flash,
    SorceryOnly,
    OwnTurnOnly,
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
    /// A leading `Operand::Filter` narrows which cards get them.
    pub effects: Vec<Operand>,
    /// As though these cards had flash.
    pub flash: bool,
    /// The player may also look at the cards.
    pub may_look: bool,
    /// Where the card goes instead when it would leave the stack.
    pub then: Option<Destination>,
    /// Cast face down, as this creature spell.
    pub face_down: Option<FaceDownAs>,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum PlayLimit {
    /// This many cards while the permission lasts.
    Total(i64),
    EachTurn(i64),
    /// One card of each of these card types.
    OnePerCardType(TypeClass),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum TypeClass {
    Nonland,
    Permanent,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum PlayCost {
    Normal,
    Free,
    Alternate(Operand),
    Additional(Operand),
    /// By this keyword's cost and its other rules.
    Using(CostedKeyword),
    /// For its mana cost less this.
    Reduced(Operand),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum SpendAs {
    AnyColor,
    AnyType,
    ColorlessAsAnyColor,
    SnowAsAnyColor,
    ColorAsAnyColor(ManaColor),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum CardPlayAction {
    CastSpell,
    PlayLand,
    Either,
}

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum CostKeyword {
    CollectEvidence,
    Blight,
    Forage,
    Suspend,
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
    /// The cost of the enclosing `Ability::CostBranch`, read in its paid branch.
    Branch,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum CostObjectView {
    Inputs,
    Performed,
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
        destination: MovementDestination,
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
    /// Puts one of these exiled cards into its owner's graveyard.
    ExiledToGraveyard(ObjectSet),
    /// Beholds a card and exiles it, linked to this object.
    BeholdAndExile(Box<Operand>),
    /// Life equal to the mana value of what this cost is paid for.
    LifeEqualToOwnManaValue,
    ManaAnyNumberOfTimes(Vec<ManaSymbol>),
    /// A mana cost less a reduction: of this object, or of what is being cast.
    ManaCostReduced {
        object: Option<ObjectRef>,
        reduction: Box<Operand>,
    },
    /// Reveals the choice a previous instruction kept hidden.
    RevealChoice(ValueKind),
    /// Mana with {X}, with X bounded by a value.
    ManaXBounded {
        mana: Vec<ManaSymbol>,
        comparison: Comparison,
        value: Expr,
    },
    UnattachFrom {
        attachments: ObjectSet,
        hosts: ObjectSet,
    },
    EnergyOneOrMore,
    /// Each of these players performs this as part of the cost.
    EachPlayerAction {
        players: PlayerSet,
        action: Box<Operand>,
    },
    /// Less this reduction for each of `per`.
    ManaReducedForEach {
        mana: Vec<ManaSymbol>,
        reduction: Box<Operand>,
        per: Expr,
    },
    /// A keyword or game action done to these objects as a cost.
    ObjectAction {
        objects: ObjectSet,
        action: CostObjectAction,
    },
    ManaFromTreasure(Vec<ManaSymbol>),
    ShuffleFromHand,
    PlayFromTop(ObjectFilter),
    /// The selected cards of `part` all belong to one player.
    SinglePlayersCards(Box<CostPart>),
    /// Counters move as the cost is paid; it can't be paid if they can't.
    MoveCounters {
        from: ObjectSet,
        to: ObjectSet,
        kind: Option<crate::counter::CounterKind>,
        amount: MovedCounters,
    },
    /// `Waterbend` with its {X} set here.
    WaterbendX {
        mana: Vec<ManaSymbol>,
        x: Expr,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum CostObjectAction {
    Suspect,
    Ante,
    GainControl,
    AttachToPlayer(PlayerSet),
    PayManaCost,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct CostObjectSelection {
    pub objects: ObjectSet,
    pub choose: Option<Count>,
}
