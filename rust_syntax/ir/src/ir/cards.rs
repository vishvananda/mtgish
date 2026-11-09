//! Cards in the normalized semantic model.
use super::*;

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
