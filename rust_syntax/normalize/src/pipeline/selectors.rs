//! Generic predicates with typed unresolved operands. Domain-specific queries
//! and reference binding remain outside predicate consolidation.
use super::{source_tree, NumberPlayers};
use crate::{
    ir, mtg_types as source,
    selector_spec::{Domain, Registry},
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Predicate {
    Source(SelectorSource),
    Literal(PredicateLiteral),
    All(Vec<Predicate>),
    Either(Vec<Predicate>),
    Not(Box<Predicate>),
    Vocabulary(PredicateVocabulary),
    Color(source::Color),
    Keyword(source::CheckHasable),
    Name(source::NameFilter),
    ControlledBy(NumberPlayers),
    OwnedBy(NumberPlayers),
    SameObject(PredicateObject),
    SharesName(PredicateObject),
    Shares(ir::DistinctProperty, PredicateObject),
    InZone {
        zone: ir::EventZone,
        players: NumberPlayers,
    },
    Property {
        property: ir::Property,
        comparison: PredicateComparison,
    },
    Counters {
        counter: Option<source::CounterType>,
        comparison: PredicateComparison,
    },
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum PredicateLiteral {
    Any,
    PermanentDomain,
    Colorless,
    Token,
    Tapped,
    Attacking,
    Blocking,
    Modified,
    IsCard,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PredicateVocabulary {
    CardType(source::CardType),
    SuperType(source::SuperType),
    CreatureType(source::CreatureType),
    LandType(source::LandType),
    ArtifactType(source::ArtifactType),
    EnchantmentType(source::EnchantmentType),
    PlaneswalkerType(source::PlaneswalkerType),
    SpellType(source::SpellType),
    SubType(source::SubType),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PredicateObject {
    Permanent(source::Permanent),
    Spell(source::Spell),
    SingleCard(source::SingleCard),
    CardInGraveyards(source::CardInGraveyards),
    CardInHand(source::CardInHand),
    CardInExile(source::CardInExile),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PredicateComparison {
    Source(source::Comparison),
    Fixed {
        operator: ir::Comparison,
        amount: i64,
    },
}
macro_rules! domains {
    ($(($ty:ident,$generated:ident,$tree:ident)),+ $(,)?)=> {
        #[derive(Clone,Debug,Serialize,Deserialize)]
        pub enum SelectorSource {$($ty(source::$ty)),+}
        #[derive(Clone,Copy,Serialize)]
        pub enum SelectorRef<'a> {$($ty(&'a source::$ty)),+}
        impl SelectorSource {
            pub fn as_ref(&self)->SelectorRef<'_> {match self {$(Self::$ty(value)=>SelectorRef::$ty(value)),+}}
            pub fn decode(domain:Domain,value:&serde_json::Value)->Result<Self,String> {
                match domain {$(Domain::$ty=>source_tree::decode(value).map(Self::$ty).map_err(|e|e.to_string())),+}
            }
        }
        pub fn rewrite(value:SelectorRef<'_>)->Option<Predicate> {match value {$(SelectorRef::$ty(value)=>$generated(value)),+}}
        $ (pub fn $tree(value:&source::$ty)->Predicate {$generated(value).unwrap_or_else(||Predicate::Source(SelectorSource::$ty(value.clone())))})+
    }
}
domains!(
    (Permanents, generated_permanents, permanents_tree),
    (Cards, generated_cards, cards_tree),
    (Spells, generated_spells, spells_tree),
    (CardsInGraveyards, generated_grave_cards, grave_cards_tree),
    (CardsInHand, generated_hand_cards, hand_cards_tree),
    (CardsInExile, generated_exile_cards, exile_cards_tree)
);
pub fn source_key(value: SelectorRef<'_>) -> String {
    serde_json::to_string(&value).expect("typed source predicate")
}
pub fn registry() -> &'static Registry {
    static REGISTRY: std::sync::OnceLock<Registry> = std::sync::OnceLock::new();
    REGISTRY.get_or_init(|| {
        Registry::parse(include_str!("../../rewrites/selectors.json5"))
            .expect("validated selector registry")
    })
}
include!(concat!(env!("OUT_DIR"), "/pipeline_selectors.rs"));
