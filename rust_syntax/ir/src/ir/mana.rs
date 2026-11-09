//! Mana in the normalized semantic model.
use super::*;

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
    /// One mana of a color among these objects, or of each such color.
    ColorsAmong {
        objects: ObjectSet,
        each: bool,
    },
    /// Mana equal to this object's mana cost.
    ManaCostOf(ObjectRef),
    /// One mana for each colored symbol in this object's mana cost.
    SymbolsOf(ObjectRef),
    /// This many mana, each of a different color.
    DifferentColors(u32),
    /// Mana of a color other than the one just produced.
    OtherColor,
    /// The mana this instruction took away.
    LostThisWay(ResultId),
    /// The mana this object noted; `amount` keeps how much as well as what type.
    LastNoted {
        amount: bool,
    },
    /// One mana of any color noted during the draft.
    DraftNotedColor(DraftNote),
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
    /// Spendable only on this part of a cost or action.
    Purpose(ManaPurpose),
    /// Spending this mana on a matching spell also changes that spell.
    SpentOn {
        spells: ObjectFilter,
        changes: Vec<Change>,
        until: Duration,
    },
    /// Forge's `TriggersWhenSpent$`.
    WhenSpent {
        spells: ObjectFilter,
        abilities: bool,
        this_turn: bool,
        body: Box<Operand>,
    },
    /// What a spell or the permanent it becomes gets when this mana pays for it.
    SpentGives {
        spells: ObjectFilter,
        effects: Vec<Operand>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum ManaPurpose {
    XCost,
    GenericCost,
    CumulativeUpkeep,
    UnlockDoor,
    TurnFaceUp(ObjectFilter),
    MorphCost,
    DisturbCost,
    Foretell,
    ClassLevel,
    AbilityKeyword(AbilityKeyword),
    CostContaining(Box<Operand>),
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
