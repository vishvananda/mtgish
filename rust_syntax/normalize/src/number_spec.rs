//! Declarative numeric rewrites shared by the build script and stage tools.
//! Every field becomes a typed operand; source variants are never guessed.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    pub version: u32,
    pub rules: Vec<Rule>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub id: String,
    pub source: String,
    pub arity: usize,
    pub expression: Formula,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Formula {
    Constant {
        value: i64,
    },
    Integer {
        argument: usize,
    },
    Source {
        argument: usize,
    },
    Binary {
        operation: Binary,
        left: Box<Formula>,
        right: Box<Formula>,
    },
    Divide {
        numerator: Box<Formula>,
        denominator: Box<Formula>,
        rounding: Rounding,
    },
    Property {
        object: Object,
        property: Property,
    },
    PlayerLife {
        argument: usize,
    },
    Count {
        query: Query,
    },
    Aggregate {
        query: Query,
        property: Property,
        aggregation: Aggregation,
    },
    Distinct {
        query: Query,
        property: DistinctProperty,
    },
    Counters {
        recipients: Recipients,
        counter: Option<usize>,
    },
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Binary {
    Add,
    Subtract,
    Multiply,
    Minimum,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Rounding {
    Down,
    Up,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Property {
    Power,
    Toughness,
    BasePower,
    BaseToughness,
    ManaValue,
    ColorCount,
    Loyalty,
    SpellX,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Aggregation {
    Sum,
    Maximum,
    Minimum,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum DistinctProperty {
    CardTypes,
    CreatureTypes,
    Colors,
    Names,
    BasicLandTypes,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ObjectKind {
    Payment(PaymentAction),
    Permanent,
    Spell,
    GraveCard,
    ExiledCard,
    HandCard,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Object {
    pub kind: ObjectKind,
    pub argument: Option<usize>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PaymentAction {
    Sacrifice,
    Discard,
    Tap,
    Untap,
    Exile,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum PrivateZone {
    Hand,
    Library,
    Graveyard,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Players {
    Player { argument: usize },
    Players { argument: usize },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Query {
    Payment {
        action: PaymentAction,
    },
    /// Restrict targets/results to objects still on the battlefield.
    Battlefield {
        argument: usize,
    },
    /// Preserve captured object identities for aggregates/last-known information.
    Objects {
        argument: usize,
    },
    GraveCards {
        argument: usize,
    },
    ExiledCards {
        argument: usize,
    },
    PrivateCards {
        zone: PrivateZone,
        players: Players,
        filter: Option<usize>,
        excluding: Option<Object>,
    },
    Union {
        queries: Vec<Query>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Recipients {
    Objects { query: Query },
    Players { players: Players },
}
impl Object {
    pub(crate) fn arguments(&self, out: &mut Vec<usize>) -> Result<(), String> {
        match (self.kind, self.argument) {
            (ObjectKind::Payment(_), None) => Ok(()),
            (ObjectKind::Payment(_), Some(_)) | (_, None) => {
                Err("numeric object has missing or extraneous argument".into())
            }
            (_, Some(i)) => {
                out.push(i);
                Ok(())
            }
        }
    }
    pub(crate) fn generate(&self) -> String {
        if let ObjectKind::Payment(action) = self.kind {
            return format!("NumberObject::Payment(crate::number_spec::PaymentAction::{action:?})");
        }
        let (kind, ty) = match self.kind {
            ObjectKind::Payment(_) => unreachable!(),
            ObjectKind::Permanent => ("Permanent", "Permanent"),
            ObjectKind::Spell => ("Spell", "Spell"),
            ObjectKind::GraveCard => ("GraveCard", "CardInGraveyards"),
            ObjectKind::ExiledCard => ("ExiledCard", "CardInExile"),
            ObjectKind::HandCard => ("HandCard", "CardInHand"),
        };
        format!(
            "NumberObject::{kind}(source::{ty}::clone(a{}))",
            self.argument.unwrap()
        )
    }
}
impl Players {
    pub(crate) fn argument(&self) -> usize {
        match self {
            Self::Player { argument } | Self::Players { argument } => *argument,
        }
    }
    pub(crate) fn generate(&self) -> String {
        let (kind, ty) = match self {
            Self::Player { .. } => ("Player", "Player"),
            Self::Players { .. } => ("Players", "Players"),
        };
        format!(
            "NumberPlayers::{kind}(source::{ty}::clone(a{}))",
            self.argument()
        )
    }
}
impl Query {
    pub(crate) fn arguments(&self, out: &mut Vec<usize>) -> Result<(), String> {
        match self {
            Self::Payment { .. } => {}
            Self::Battlefield { argument }
            | Self::Objects { argument }
            | Self::GraveCards { argument }
            | Self::ExiledCards { argument } => out.push(*argument),
            Self::PrivateCards {
                players,
                filter,
                excluding,
                ..
            } => {
                out.push(players.argument());
                out.extend(filter);
                if let Some(object) = excluding {
                    object.arguments(out)?;
                }
            }
            Self::Union { queries } => {
                if queries.is_empty() {
                    return Err("empty numeric query union".into());
                }
                for q in queries {
                    q.arguments(out)?;
                }
            }
        }
        Ok(())
    }
    pub(crate) fn generate(&self) -> String {
        match self {
            Self::Payment {action}=>format!("NumberQuery::Payment(crate::number_spec::PaymentAction::{action:?})"),
            Self::Battlefield {argument}=>format!("NumberQuery::Battlefield(source::Permanents::clone(a{argument}))"),
            Self::Objects {argument}=>format!("NumberQuery::Objects(source::Permanents::clone(a{argument}))"),
            Self::GraveCards {argument}=>format!("NumberQuery::GraveCards(source::CardsInGraveyards::clone(a{argument}))"),
            Self::ExiledCards {argument}=>format!("NumberQuery::ExiledCards(source::CardsInExile::clone(a{argument}))"),
            Self::PrivateCards {zone,players,filter,excluding}=>format!("NumberQuery::PrivateCards {{ zone: ir::EventZone::{zone:?}, players: {}, filter: {}, excluding: {} }}",players.generate(),filter.map_or("None".into(),|i|format!("Some(source::Cards::clone(a{i}))")),excluding.as_ref().map_or("None".into(),|o|format!("Some({})",o.generate()))),
            Self::Union {queries}=>format!("NumberQuery::Union(vec![{}])",queries.iter().map(Query::generate).collect::<Vec<_>>().join(",")),
        }
    }
}
impl Recipients {
    pub(crate) fn arguments(&self, out: &mut Vec<usize>) -> Result<(), String> {
        match self {
            Self::Objects { query } => query.arguments(out)?,
            Self::Players { players } => out.push(players.argument()),
        };
        Ok(())
    }
    pub(crate) fn generate(&self) -> String {
        match self {
            Self::Objects { query } => format!("NumberRecipients::Objects({})", query.generate()),
            Self::Players { players } => {
                format!("NumberRecipients::Players({})", players.generate())
            }
        }
    }
}
impl Formula {
    pub(crate) fn arguments(&self, out: &mut Vec<usize>) -> Result<(), String> {
        match self {
            Self::Constant { .. } => {}
            Self::Integer { argument }
            | Self::Source { argument }
            | Self::PlayerLife { argument } => out.push(*argument),
            Self::Binary { left, right, .. } => {
                left.arguments(out)?;
                right.arguments(out)?;
            }
            Self::Divide {
                numerator,
                denominator,
                ..
            } => {
                numerator.arguments(out)?;
                denominator.arguments(out)?;
            }
            Self::Property { object, .. } => object.arguments(out)?,
            Self::Count { query }
            | Self::Aggregate { query, .. }
            | Self::Distinct { query, .. } => query.arguments(out)?,
            Self::Counters {
                recipients,
                counter,
            } => {
                recipients.arguments(out)?;
                out.extend(counter);
            }
        }
        Ok(())
    }
    pub(crate) fn generate(&self) -> String {
        match self {
            Self::Constant {value}=>format!("Quantity::Constant({value})"),
            Self::Integer {argument}=>format!("Quantity::Constant(i64::from(*a{argument}))"),
            Self::Source {argument}=>format!("number_tree(a{argument})"),
            Self::Binary {operation,left,right}=>format!("Quantity::{operation:?}(Box::new({}),Box::new({}))",left.generate(),right.generate()),
            Self::Divide {numerator,denominator,rounding}=>format!("Quantity::Divide {{ numerator: Box::new({}), denominator: Box::new({}), rounding: ir::Rounding::{rounding:?} }}",numerator.generate(),denominator.generate()),
            Self::Property {object,property}=>format!("Quantity::Property {{ object: {}, property: ir::Property::{property:?} }}",object.generate()),
            Self::PlayerLife {argument}=>format!("Quantity::PlayerLife(source::Player::clone(a{argument}))"),
            Self::Count {query}=>format!("Quantity::Count({})",query.generate()),
            Self::Aggregate {query,property,aggregation}=>format!("Quantity::Aggregate {{ query: {}, property: ir::Property::{property:?}, aggregation: ir::Aggregation::{aggregation:?} }}",query.generate()),
            Self::Distinct {query,property}=>format!("Quantity::Distinct {{ query: {}, property: ir::DistinctProperty::{property:?} }}",query.generate()),
            Self::Counters {recipients,counter}=>format!("Quantity::Counters {{ recipients: {}, counter: {} }}",recipients.generate(),counter.map_or("None".into(),|i|format!("Some(source::CounterType::clone(a{i}))"))),
        }
    }
}
impl Registry {
    pub fn parse(text: &str) -> Result<Self, String> {
        let r: Self = serde_json::from_str(text).map_err(|e| e.to_string())?;
        r.validate()?;
        Ok(r)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 2 {
            return Err("unsupported numeric registry version".into());
        }
        let mut ids = BTreeSet::new();
        let mut sources = BTreeSet::new();
        for r in &self.rules {
            if r.id.is_empty() || !ids.insert(&r.id) || !sources.insert(&r.source) {
                return Err(format!("empty/duplicate numeric rule: {}", r.id));
            }
            if r.source.is_empty()
                || !r.source.chars().enumerate().all(|(i, c)| {
                    c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit())
                })
            {
                return Err("invalid Rust numeric variant name".into());
            }
            let mut arguments = vec![];
            r.expression.arguments(&mut arguments)?;
            let mut consumed = BTreeSet::new();
            for i in arguments {
                if i >= r.arity || !consumed.insert(i) {
                    return Err(format!("invalid/reused numeric field: {}", r.id));
                }
            }
            if consumed.len() != r.arity {
                return Err(format!("unconsumed numeric field: {}", r.id));
            }
        }
        Ok(())
    }
    pub fn generate(&self) -> String {
        let mut out="// Generated from rewrites/numbers.json5.\nfn generated_number(value: &source::GameNumber) -> Option<Quantity> { Some(match value {\n".to_string();
        for r in &self.rules {
            let args = if r.arity == 0 {
                String::new()
            } else {
                format!(
                    "({})",
                    (0..r.arity)
                        .map(|i| format!("a{i}"))
                        .collect::<Vec<_>>()
                        .join(",")
                )
            };
            out.push_str(&format!(
                "source::GameNumber::{}{args} => {},\n",
                r.source,
                r.expression.generate()
            ));
        }
        out.push_str("_=>return None,\n})}\n");
        out
    }
}
