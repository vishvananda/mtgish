//! Cost/condition operands are independent of the enclosing ability or action.
//! Source leaves remain unresolved until lexical and payer binding succeeds.
use super::{
    numbers::{number_tree, NumberObject, NumberPlayers, NumberQuery},
    selectors::Predicate,
    source_tree, Quantity,
};
pub use crate::operand_spec::{AmountOperation, MatchDomain};
use crate::{
    ir, mtg_types as source,
    operand_spec::{Domain, Registry},
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CostRecipe {
    Constrained {
        cost: Box<CostRecipe>,
        group: source::GroupFilter,
    },
    Source(source::Cost),
    All(Vec<CostRecipe>),
    Choice(Vec<CostRecipe>),
    Mana(source::ManaCost),
    ManaX {
        mana: source::ManaCostX,
        variable: source::GameNumber,
    },
    TapPermanent {
        object: source::Permanent,
        tapped: bool,
    },
    TapObjects {
        subjects: super::Subjects,
        selection: Option<CostSelection>,
        tapped: bool,
    },
    Amount {
        operation: AmountOperation,
        amount: Quantity,
    },
    Loyalty(i64),
    OwnMana,
    Sacrifice {
        subjects: super::Subjects,
        selection: Option<CostSelection>,
    },
    Discard {
        amount: Quantity,
        filter: CostFilter,
        random: bool,
    },
    Return {
        subjects: source::Permanents,
        amount: Quantity,
    },
    Exile {
        items: Vec<source::ExilableCost>,
        flags: Vec<source::ExileFlag>,
    },
    RemoveCounters(Vec<source::RemoveCountersCost>),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CostSelection {
    Exactly(Quantity),
    AnyNumber,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CostFilter {
    Any,
    Cards(Predicate),
    HandCard(source::CardInHand),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CheckRecipe {
    Source(source::Condition),
    All(Vec<CheckRecipe>),
    Either(Vec<CheckRecipe>),
    Payment {
        negated: bool,
    },
    Compare {
        left: Quantity,
        comparison: source::Comparison,
    },
    SpellX(source::Comparison),
    Matches {
        object: NumberObject,
        domain: MatchDomain,
        filter: Predicate,
    },
    Active {
        players: NumberPlayers,
        negated: bool,
    },
    Player {
        players: NumberPlayers,
        filter: source::Players,
    },
    SourceZones(Vec<ir::EventZone>),
    Graveyard {
        object: Option<source::CardInGraveyards>,
        owner: source::Player,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum OperandRecipe {
    Cost(CostRecipe),
    Condition(CheckRecipe),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum OperandSource {
    Cost(source::Cost),
    Condition(source::Condition),
}
#[derive(Clone, Copy, Serialize)]
pub enum OperandRef<'a> {
    Cost(&'a source::Cost),
    Condition(&'a source::Condition),
}
impl OperandSource {
    pub fn as_ref(&self) -> OperandRef<'_> {
        match self {
            Self::Cost(c) => OperandRef::Cost(c),
            Self::Condition(c) => OperandRef::Condition(c),
        }
    }
    pub fn decode(domain: Domain, value: &serde_json::Value) -> Result<Self, String> {
        match domain {
            Domain::Cost => source_tree::decode(value)
                .map(Self::Cost)
                .map_err(|e| e.to_string()),
            Domain::Condition => source_tree::decode(value)
                .map(Self::Condition)
                .map_err(|e| e.to_string()),
        }
    }
}
pub fn registry() -> &'static Registry {
    static REGISTRY: std::sync::OnceLock<Registry> = std::sync::OnceLock::new();
    REGISTRY.get_or_init(|| {
        Registry::parse(include_str!("../../rewrites/operands.json5"))
            .expect("validated operand registry")
    })
}
pub fn source_key(value: OperandRef<'_>) -> String {
    serde_json::to_string(&value).expect("typed operand source")
}
pub fn rewrite(value: OperandRef<'_>) -> Option<OperandRecipe> {
    match value {
        OperandRef::Cost(c) => generated_cost(c).map(OperandRecipe::Cost),
        OperandRef::Condition(c) => generated_check(c).map(OperandRecipe::Condition),
    }
}
pub fn cost_tree(value: &source::Cost) -> CostRecipe {
    generated_cost(value).unwrap_or_else(|| CostRecipe::Source(value.clone()))
}
pub fn check_tree(value: &source::Condition) -> CheckRecipe {
    generated_check(value).unwrap_or_else(|| CheckRecipe::Source(value.clone()))
}
include!(concat!(env!("OUT_DIR"), "/pipeline_operands.rs"));
