//! Context-independent numeric consolidation. Unknown leaves remain typed source
//! operands; the final binder must resolve every one before exporting a Card.
use crate::{ir, mtg_types as source, number_spec::Registry};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Quantity {
    Constant(i64),
    Source(source::GameNumber),
    Add(Box<Quantity>, Box<Quantity>),
    Subtract(Box<Quantity>, Box<Quantity>),
    Multiply(Box<Quantity>, Box<Quantity>),
    Minimum(Box<Quantity>, Box<Quantity>),
    Maximum(Box<Quantity>, Box<Quantity>),
    Divide {
        numerator: Box<Quantity>,
        denominator: Box<Quantity>,
        rounding: ir::Rounding,
    },
    Property {
        object: NumberObject,
        property: ir::Property,
    },
    PlayerLife(source::Player),
    Count(NumberQuery),
    Aggregate {
        query: NumberQuery,
        property: ir::Property,
        aggregation: ir::Aggregation,
    },
    Distinct {
        query: NumberQuery,
        property: ir::DistinctProperty,
    },
    Counters {
        recipients: NumberRecipients,
        counter: Option<source::CounterType>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NumberObject {
    Payment(crate::number_spec::PaymentAction),
    Permanent(source::Permanent),
    Spell(source::Spell),
    GraveCard(source::CardInGraveyards),
    ExiledCard(source::CardInExile),
    HandCard(source::CardInHand),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NumberPlayers {
    Player(source::Player),
    Players(source::Players),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NumberQuery {
    Payment(crate::number_spec::PaymentAction),
    Battlefield(source::Permanents),
    Objects(source::Permanents),
    GraveCards(source::CardsInGraveyards),
    ExiledCards(source::CardsInExile),
    PrivateCards {
        zone: ir::EventZone,
        players: NumberPlayers,
        filter: Option<source::Cards>,
        excluding: Option<NumberObject>,
    },
    Union(Vec<NumberQuery>),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NumberRecipients {
    Objects(NumberQuery),
    Players(NumberPlayers),
}
pub fn registry() -> &'static Registry {
    static REGISTRY: std::sync::OnceLock<Registry> = std::sync::OnceLock::new();
    REGISTRY.get_or_init(|| {
        Registry::parse(include_str!("../../rewrites/numbers.json5"))
            .expect("validated numeric registry")
    })
}
include!(concat!(env!("OUT_DIR"), "/pipeline_numbers.rs"));
pub fn rewrite_number(value: &source::GameNumber) -> Option<Quantity> {
    generated_number(value)
}
pub fn number_tree(value: &source::GameNumber) -> Quantity {
    rewrite_number(value).unwrap_or_else(|| Quantity::Source(value.clone()))
}
pub(crate) fn number_key(value: &source::GameNumber) -> String {
    serde_json::to_string(value).expect("serializable numeric source")
}
