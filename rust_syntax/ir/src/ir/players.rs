//! Players in the normalized semantic model.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum PriorPlayer {
    WinningBidder,
    Remembered,
    GrantedBy,
    /// "That" player: whichever the row around this one names.
    That,
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
    /// Damage that would bring the life total below this leaves it there.
    DamageLifeFloor(Expr),
    /// An action the player may take while this applies, outside the stack.
    /// `limit` counts uses; `during` names the turns it can be used in.
    SpecialAction {
        body: Box<Operand>,
        cost: Option<Box<Operand>>,
        limit: Option<PlayLimit>,
        during: Option<PlayerSet>,
        instant_speed: bool,
    },
    /// Loosens when or how often matching activated abilities can be used.
    ActivationGrant {
        abilities: ObjectFilter,
        grant: ActivationGrant,
    },
    ActivationTiming(SpellTiming),
    /// Changes what a keyword's cost is, as Forge's `ReduceCost` does with
    /// `Type$`. `per` multiplies the change; `first_each_turn` limits it to
    /// the first such cost paid each turn.
    KeywordCost {
        keyword: CostedKeyword,
        cards: ObjectFilter,
        change: KeywordCostChange,
        per: Option<Expr>,
        first_each_turn: bool,
    },
    /// A keyword's action becomes available to these cards from this zone.
    KeywordAccess {
        keyword: CostedKeyword,
        cards: ObjectFilter,
        from: EventZone,
        during: Option<PlayerSet>,
    },
    /// This player makes a decision the rules give to someone else.
    Decides(GameDecision),
    /// Attacks if able. `count` is how many attackers, when not each one.
    MustAttack {
        attackers: ObjectFilter,
        defenders: AttackDefenders,
        count: Option<(Comparison, Expr)>,
    },
    AttackOnly(AttackDefenders),
    AttackedOnlyBy(ObjectFilter),
    AttackWithoutTapping {
        attackers: ObjectFilter,
        condition: Condition,
    },
    BlockerLimit(Expr),
    AdditionalVote {
        optional: bool,
    },
    /// Mana empties into this color, or colorless when none is named.
    UnspentManaBecomes(Option<ManaColor>),
    /// What mana of one color can be spent as.
    SpendColorAs {
        color: ManaColor,
        as_color: Option<ManaColor>,
        others_colorless: bool,
        on: ManaPayment,
    },
    /// Cards they draw are revealed; `first_during` limits it to the first
    /// each such turn, and `optional` makes revealing a choice.
    RevealDraws {
        first_during: Option<PlayerSet>,
        optional: bool,
    },
    /// Spells and abilities matching `by` can't make the player do this.
    CantBeCaused {
        action: PlayerAction,
        by: ObjectFilter,
    },
    TargetsMustInclude(ObjectFilter),
    /// Targetable by these sources as though lacking this keyword.
    TargetableDespite {
        sources: ObjectFilter,
        keyword: TargetingKeyword,
    },
    DrawsDuringDrawStepOf(PlayerSet),
    UntapsDuringUntapStepOf {
        permanents: ObjectFilter,
        players: PlayerSet,
    },
    UntapOnlyOneOfTypes(Vec<Operand>),
    /// Pays this much life rather than one of this mana symbol.
    LifeForMana {
        life: Expr,
        symbol: Box<Operand>,
    },
    /// A mana ability: pay this much life for this mana.
    LifeForManaAbility {
        life: Expr,
        mana: Box<Operand>,
    },
    /// May tap these for mana although others control them.
    TapOthersForMana {
        permanents: ObjectFilter,
        modifier: Box<Operand>,
    },
    /// Pays to change dice they roll.
    DiceChange {
        cost: Box<Operand>,
        limit: Option<PlayLimit>,
        reroll: bool,
    },
    SurveilBonus(Expr),
    DevotionBonus(Expr),
    CascadePut {
        filter: ObjectFilter,
        flags: Box<Operand>,
    },
    AsManaEmpties {
        mana: Box<Operand>,
        then: Box<Operand>,
    },
    /// A rule one or two cards print, with nothing to parameterise.
    Unique(UniquePlayerRule),
}

/// The closed list of player rules specific to one or two cards.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode,
)]
pub enum UniquePlayerRule {
    /// Edgar, King of Figaro.
    FirstTwoCoinFlipsWin,
    /// River Song.
    DrawsFromBottomOfLibrary,
    /// Strong, the Brutish Thespian.
    RadiationGainsLife,
    /// Circling Vultures.
    DiscardThisAtInstantSpeed,
    /// Dakkon Blackblade Avatar.
    ColoredCardsAsRandomBasicLands,
    /// Bell Borca, Spectral Sergeant.
    NoteExiledManaValue,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum HandSizeRule {
    Unlimited,
    Set(Expr),
    Adjust(Expr),
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
    UnspentMana,
    /// What the player secretly chose; players who chose none are left out.
    ChosenNumber,
    TurnsBegun,
    AttractionsVisitedThisTurn,
    DescentsThisTurn,
    CardsCycledOrDiscardedThisTurn,
    DungeonsCompleted,
    CommanderColorIdentity,
    UnspentManaOf(ManaColor),
    RingTemptations,
    StartingDeckSize,
    EnergyPaidOrLostThisTurn,
    CardsDrawnThisTurn,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(tag = "kind", content = "args")]
pub enum PlayerRef {
    /// The player a choice or payment ended up with: "the player who did".
    Performer(ResultId),
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
    /// The player doing this action, for one of several: "half their life".
    Acting,
    /// A seat relative to a player.
    Seat {
        of: Box<PlayerRef>,
        side: SeatSide,
    },
    AttackingPlayer,
    DefendingPlayer,
    Holder(Designation),
    /// The player with the most of something. Ties are the row's to resolve.
    Extreme {
        players: Box<PlayerSet>,
        number: PlayerNumber,
        highest: bool,
    },
    SoleTargetOf(Box<ObjectRef>),
    Named {
        result: ResultId,
        index: u16,
    },
    Prior(PriorPlayer),
    /// The player noted for this object.
    NotedFor(Box<ObjectRef>),
    /// The other of two chosen players.
    OtherChosen(Box<PlayerRef>),
    InChosenDirection {
        from: Option<Box<PlayerRef>>,
        among: Option<Box<PlayerSet>>,
    },
    /// `player` read at an earlier moment, as `Expr::AsOf` reads a number.
    AsOf {
        moment: Box<Moment>,
        player: Box<PlayerRef>,
    },
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
    /// "Those" players: whichever the row around this one names.
    Scoped,
}

/// Something a player did or had done to them. See `Condition::PlayerDid`.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum PlayerHistory {
    AttackedWithTotalPower(Comparison, Expr),
    AttackedBy(Box<ObjectFilter>),
    AttackedWith(Box<ObjectFilter>),
    AttackedPlayer(PlayerRef),
    AttackedDefender(Box<ObjectFilter>),
    CastSpell(Box<ObjectFilter>),
    PlayedLandNotFromHand,
    PlayedFromExile(Box<ObjectFilter>),
    PermanentEntered(Box<ObjectFilter>),
    ActivatedGraveyardAbility,
    ActivatedExhaust,
    WasDealtCombatDamage,
    PaidOrLostEnergy(Comparison, Expr),
    Cycled(Comparison, Expr, Box<ObjectFilter>),
    RolledDie(Comparison, Expr),
    WasMonarch,
    BentAllFour,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum SeatSide {
    Left,
    Right,
    /// The next opponent in turn order.
    Next,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum Designation {
    Monarch,
    Initiative,
}
