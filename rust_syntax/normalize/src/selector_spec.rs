//! Reviewed predicate recipes shared by generation and stage inspection.
use crate::number_spec::{Players, PrivateZone, Property};
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
    pub domain: Domain,
    pub source: String,
    pub arity: usize,
    pub predicate: Recipe,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Domain {
    Permanents,
    Cards,
    Spells,
    CardsInGraveyards,
    CardsInHand,
    CardsInExile,
}
impl Domain {
    pub const ALL: [Self; 6] = [
        Self::Permanents,
        Self::Cards,
        Self::Spells,
        Self::CardsInGraveyards,
        Self::CardsInHand,
        Self::CardsInExile,
    ];
    pub fn rust_name(self) -> &'static str {
        match self {
            Self::Permanents => "Permanents",
            Self::Cards => "Cards",
            Self::Spells => "Spells",
            Self::CardsInGraveyards => "CardsInGraveyards",
            Self::CardsInHand => "CardsInHand",
            Self::CardsInExile => "CardsInExile",
        }
    }
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Permanents => "permanents",
            Self::Cards => "cards",
            Self::Spells => "spells",
            Self::CardsInGraveyards => "grave_cards",
            Self::CardsInHand => "hand_cards",
            Self::CardsInExile => "exile_cards",
        }
    }
    #[allow(dead_code)] // Runtime helper; this module is also compiled by the build script.
    pub fn from_rust_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.rust_name() == name)
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Literal {
    Any,
    Colorless,
    Token,
    Tapped,
    Attacking,
    Blocking,
    Modified,
    IsCard,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Vocabulary {
    CardType,
    SuperType,
    CreatureType,
    LandType,
    ArtifactType,
    EnchantmentType,
    PlaneswalkerType,
    SpellType,
    SubType,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ObjectDomain {
    Permanent,
    Spell,
    SingleCard,
    CardInGraveyards,
    CardInHand,
    CardInExile,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum ComparisonOperator {
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Compare {
    Source {
        argument: usize,
    },
    Fixed {
        operator: ComparisonOperator,
        amount: i64,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Recipe {
    Literal {
        value: Literal,
    },
    All {
        predicates: Vec<Recipe>,
    },
    Either {
        predicates: Vec<Recipe>,
    },
    Not {
        predicate: Box<Recipe>,
    },
    Source {
        argument: usize,
    },
    AllFrom {
        argument: usize,
    },
    EitherFrom {
        argument: usize,
    },
    Vocabulary {
        vocabulary: Vocabulary,
        argument: Option<usize>,
        value: Option<String>,
    },
    Color {
        argument: usize,
    },
    Keyword {
        argument: usize,
    },
    Name {
        argument: usize,
    },
    ControlledBy {
        players: Players,
    },
    OwnedBy {
        players: Players,
    },
    SharesName {
        domain: ObjectDomain,
        argument: usize,
    },
    SameObject {
        domain: ObjectDomain,
        argument: usize,
    },
    InZone {
        zone: PrivateZone,
        players: Players,
    },
    Property {
        property: Property,
        comparison: Compare,
    },
    Counters {
        counter: Option<usize>,
        comparison: Compare,
    },
}
fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .enumerate()
            .all(|(i, c)| c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit()))
}
impl Compare {
    fn arguments(&self, out: &mut Vec<usize>) {
        if let Self::Source { argument } = self {
            out.push(*argument);
        }
    }
    fn generate(&self) -> String {
        match self {Self::Source {argument}=>format!("PredicateComparison::Source(source::Comparison::clone(a{argument}))"),Self::Fixed {operator,amount}=>format!("PredicateComparison::Fixed {{ operator: ir::Comparison::{operator:?}, amount: {amount} }}")}
    }
}
impl Recipe {
    fn arguments(&self, out: &mut Vec<usize>) -> Result<(), String> {
        match self {
            Self::Literal { .. } => {}
            Self::All { predicates } | Self::Either { predicates } => {
                if predicates.is_empty() {
                    return Err("empty fixed predicate composition".into());
                }
                for p in predicates {
                    p.arguments(out)?;
                }
            }
            Self::Not { predicate } => predicate.arguments(out)?,
            Self::Source { argument }
            | Self::AllFrom { argument }
            | Self::EitherFrom { argument }
            | Self::Color { argument }
            | Self::Keyword { argument }
            | Self::Name { argument }
            | Self::SharesName { argument, .. }
            | Self::SameObject { argument, .. } => out.push(*argument),
            Self::Vocabulary {
                argument, value, ..
            } => {
                if argument.is_some() == value.is_some() {
                    return Err("vocabulary requires one source field or constant".into());
                }
                if let Some(value) = value {
                    if !identifier(value) {
                        return Err("invalid vocabulary constant".into());
                    }
                }
                out.extend(argument);
            }
            Self::ControlledBy { players }
            | Self::OwnedBy { players }
            | Self::InZone { players, .. } => out.push(players.argument()),
            Self::Property { comparison, .. } => comparison.arguments(out),
            Self::Counters {
                counter,
                comparison,
            } => {
                out.extend(counter);
                comparison.arguments(out);
            }
        }
        Ok(())
    }
    fn generate(&self, domain: Domain) -> String {
        let suffix = domain.suffix();
        match self {
            Self::Literal {value}=>format!("Predicate::Literal(PredicateLiteral::{value:?})"),
            Self::All {predicates}|Self::Either {predicates}=>format!("Predicate::{}(vec![{}])",if matches!(self,Self::All {..}) {"All"} else {"Either"},predicates.iter().map(|p|p.generate(domain)).collect::<Vec<_>>().join(",")),
            Self::Not {predicate}=>format!("Predicate::Not(Box::new({}))",predicate.generate(domain)),
            Self::Source {argument}=>format!("{suffix}_tree(a{argument})"),
            Self::AllFrom {argument}|Self::EitherFrom {argument}=>format!("Predicate::{}(a{argument}.iter().map({suffix}_tree).collect())",if matches!(self,Self::AllFrom {..}) {"All"} else {"Either"}),
            Self::Vocabulary {vocabulary,argument,value}=> {
                let ty=format!("{vocabulary:?}");
                let value=argument.map_or_else(||format!("source::{ty}::{}",value.as_ref().unwrap()),|i|format!("source::{ty}::clone(a{i})"));
                format!("Predicate::Vocabulary(PredicateVocabulary::{ty}({value}))")
            }
            Self::Color {argument}=>format!("Predicate::Color(source::Color::clone(a{argument}))"),
            Self::Keyword {argument}=>format!("Predicate::Keyword(source::CheckHasable::clone(a{argument}))"),
            Self::Name {argument}=>format!("Predicate::Name(source::NameFilter::clone(a{argument}))"),
            Self::ControlledBy {players}=>format!("Predicate::ControlledBy({})",players.generate()),
            Self::OwnedBy {players}=>format!("Predicate::OwnedBy({})",players.generate()),
            Self::SameObject {domain,argument}=>format!("Predicate::SameObject(PredicateObject::{domain:?}(source::{domain:?}::clone(a{argument})))"),
            Self::SharesName {domain,argument}=>format!("Predicate::SharesName(PredicateObject::{domain:?}(source::{domain:?}::clone(a{argument})))"),
            Self::InZone {zone,players}=>format!("Predicate::InZone {{ zone: ir::EventZone::{zone:?},players: {} }}",players.generate()),
            Self::Property {property,comparison}=>format!("Predicate::Property {{ property: ir::Property::{property:?}, comparison: {} }}",comparison.generate()),
            Self::Counters {counter,comparison}=>format!("Predicate::Counters {{ counter: {}, comparison: {} }}",counter.map_or("None".into(),|i|format!("Some(source::CounterType::clone(a{i}))")),comparison.generate()),
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
        if self.version != 1 {
            return Err("unsupported selector registry version".into());
        }
        let mut ids = BTreeSet::new();
        let mut sources = BTreeSet::new();
        for r in &self.rules {
            if r.id.is_empty() || !ids.insert(&r.id) || !sources.insert((r.domain, &r.source)) {
                return Err("empty/duplicate selector ID or domain/source".into());
            }
            if !identifier(&r.source) {
                return Err("invalid Rust selector variant".into());
            }
            let mut arguments = vec![];
            r.predicate.arguments(&mut arguments)?;
            let mut consumed = BTreeSet::new();
            for i in arguments {
                if i >= r.arity || !consumed.insert(i) {
                    return Err(format!("invalid/reused selector field: {}", r.id));
                }
            }
            if consumed.len() != r.arity {
                return Err(format!("unconsumed selector field: {}", r.id));
            }
        }
        Ok(())
    }
    pub fn generate(&self) -> String {
        let mut out = "// Generated from rewrites/selectors.json5.\n".to_string();
        for domain in Domain::ALL {
            let ty = domain.rust_name();
            let suffix = domain.suffix();
            out.push_str(&format!("fn generated_{suffix}(value:&source::{ty})->Option<Predicate> {{ Some(match value {{\n"));
            for r in self.rules.iter().filter(|r| r.domain == domain) {
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
                    "source::{ty}::{}{args}=>{},\n",
                    r.source,
                    r.predicate.generate(domain)
                ));
            }
            out.push_str("_=>return None,\n})}\n");
        }
        out
    }
}
