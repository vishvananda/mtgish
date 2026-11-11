//! Selection in the normalized semantic model.
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct Announcement {
    pub targets: Vec<TargetSpec>,
    /// Rare modal payload stays out of the common nonmodal announcement.
    pub modes: Option<Box<ModeSelection>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct ModeSelection {
    pub count: ModeCount,
    pub options: Vec<Mode>,
    pub rules: ModeRules,
    pub mechanism: ModeMechanism,
    /// Chosen modes' weights may total at most this, as pawprints.
    pub budget: Option<Expr>,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct ModeCount {
    pub minimum: Expr,
    pub maximum: Expr,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct ModeRules {
    pub repetition: ModeRepetition,
    pub availability: ModeAvailability,
    /// Paid once per mode beyond the first, as for escalate (702.120).
    pub surcharge: Option<Cost>,
    pub targets: ModeTargetPolicy,
    /// Either the stated number of modes or none: "you may choose two".
    pub acceptance: ModeAcceptance,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ModeMechanism {
    Announced {
        chooser: ModeChooser,
        method: ModePicking,
    },
    /// Targets or a mode's requirement determine which mode applies (601.2c).
    Implied,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ModeChooser {
    Controller,
    OneOf(PlayerSet),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ModePicking {
    PlayerChoice,
    Random,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ModeRepetition {
    OnceEach,
    MayRepeat,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ModeAvailability {
    Any,
    Unchosen(ModeHistory),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ModeTargetPolicy {
    MayShare,
    Distinct,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ModeAcceptance {
    Required,
    MayDecline,
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
    pub slot: TargetSlot,
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
pub struct TargetSlot(pub u16);

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct LoopBinding(pub u32);

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct ResultId(pub u32);

/// A preparation-local binding; never an instruction's observed/moved result.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct SelectionId(pub u32);

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum SelectionChooser {
    Players(PlayerSet),
    Chance,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct SelectionStep {
    pub binding: SelectionId,
    pub chooser: SelectionChooser,
    pub objects: ObjectSet,
    pub count: Count,
    pub constraints: Vec<SelectionConstraint>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct ChoiceTerms {
    /// What may be chosen: values, or a `Comparison` each number must meet.
    pub options: Vec<Operand>,
    pub except: Vec<Operand>,
    pub count: ChoiceCount,
    pub secret: bool,
    /// Not what was chosen before.
    pub unchosen: bool,
    pub random: bool,
    pub ordered: bool,
}

/// Cardinality for values, independent of object targeting and payment counts.
/// The default is exactly one; zero is a real bound, never an unbounded sentinel.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ChoiceCount {
    Exactly(Expr),
    UpTo(Expr),
    AnyNumber,
}

impl Default for ChoiceCount {
    fn default() -> Self {
        Self::Exactly(Expr::Constant(1))
    }
}

impl ChoiceTerms {
    /// Normalize the legacy grammar table's zero-as-any convention at its edge.
    pub fn ordinary(count: u32, options: Vec<Operand>) -> Self {
        Self {
            options,
            count: if count == 0 {
                ChoiceCount::AnyNumber
            } else {
                ChoiceCount::Exactly(Expr::Constant(i64::from(count)))
            },
            ..Self::default()
        }
    }
    /// An ordinary cardinality without rich flags or exclusions. Exact
    /// zero must not reach that backend's legacy zero-as-any calling convention.
    pub fn ordinary_count(&self) -> Option<u32> {
        if !self.except.is_empty() || self.secret || self.unchosen || self.random || self.ordered {
            return None;
        }
        match self.count {
            ChoiceCount::AnyNumber => Some(0),
            ChoiceCount::Exactly(Expr::Constant(n)) if n > 0 => u32::try_from(n).ok(),
            _ => None,
        }
    }
    pub fn exactly_one(&self) -> bool {
        matches!(self.count, ChoiceCount::Exactly(Expr::Constant(1)))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct SearchChoice {
    pub filter: ObjectFilter,
    /// Qualified hidden-zone searches use UpTo (may fail to find). Unqualified
    /// quantities use AsManyAsCan; the distinction is not inferred from whether
    /// matching cards happen to be present (CR 701.23b/d). Exactly, on a group
    /// with constraints, finds that whole group or fails to find.
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
        destination: MovementDestination,
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
    DifferentFromTarget(TargetSlot),
    /// Each pick has the name of a different one of these objects.
    NamedAfterOneEach(ObjectSet),
    /// No two of the chosen objects share a value of this property.
    NoCommonProperty(DistinctProperty),
    /// A number of distinct values of this property among the chosen objects.
    DistinctCount {
        property: DistinctProperty,
        comparison: Comparison,
        amount: Expr,
    },
    /// A number of mana symbols of this color among the chosen objects' mana costs.
    TotalManaSymbols {
        color: ManaColor,
        comparison: Comparison,
        amount: Expr,
    },
    /// Counts against that earlier slot's limit, as "up to three target creatures
    /// and/or creature cards in graveyards".
    SharesLimitWith(TargetSlot),
    /// Every object has the same set of values of this property.
    AllSameProperty(DistinctProperty),
    /// Not what an earlier run of the enclosing player loop picked.
    Unpicked,
    /// One choice for each type in this domain. An object can fill only one
    /// type's choice, even if it has several of those types.
    OneForEachCardType(CardTypeDomain),
    /// Choose a name by choosing one object, then take every offered object
    /// with that name. This is stronger than merely sharing a name.
    AllWithChosenName,
    /// A reviewed set qualification whose richer semantics remain explicit.
    Primitive {
        name: String,
        operands: Vec<Operand>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct SelectionChoice {
    pub player: PlayerRef,
    pub count: Count,
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

impl Announcement {
    pub fn empty() -> Self {
        Self {
            targets: Vec::new(),
            modes: None,
        }
    }
}
