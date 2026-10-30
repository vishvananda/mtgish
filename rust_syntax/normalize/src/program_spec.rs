//! Reviewed mappings for instruction/announcement structure. Policy choices are
//! explicit; matching field types alone never chooses an execution policy.
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
    pub recipe: Recipe,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Domain {
    Action,
    Actions,
}
impl Domain {
    pub fn name(self) -> &'static str {
        match self {
            Self::Action => "Action",
            Self::Actions => "Actions",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Body {
    Action { argument: usize },
    List { argument: usize },
    Program { argument: usize },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Actor {
    Inherit,
    Player { argument: usize },
    Batch { argument: usize },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Bounds {
    Exact { count: i64 },
    All,
    OneOrMore,
    AnyNumber,
    UpTo { count: i64 },
    UpToSource { argument: usize },
    RepeatSource { argument: usize },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Recipe {
    Actor {
        actor: Actor,
        body: Body,
    },
    Optional {
        actor: Actor,
        body: Body,
    },
    Conditional {
        condition: usize,
        negated: bool,
        then: Body,
        otherwise: Option<Body>,
    },
    Payment {
        player: Option<usize>,
        cost: usize,
        optional: bool,
    },
    Sequence {
        body: Body,
    },
    Targeted {
        targets: usize,
        body: Body,
    },
    Modal {
        bounds: Bounds,
        modes: usize,
    },
}
impl Body {
    fn argument(&self) -> usize {
        match self {
            Self::Action { argument } | Self::List { argument } | Self::Program { argument } => {
                *argument
            }
        }
    }
    fn generate(&self) -> String {
        match self {
            Self::Action { argument } => format!("action_tree(a{argument})"),
            Self::List { argument } => format!(
                "InstructionRecipe::Sequence(a{argument}.iter().map(action_tree).collect())"
            ),
            Self::Program { argument } => format!("program_tree(a{argument})"),
        }
    }
}
impl Actor {
    fn arguments(&self, out: &mut Vec<usize>) {
        match self {
            Self::Inherit => {}
            Self::Player { argument } | Self::Batch { argument } => out.push(*argument),
        }
    }
    fn generate(&self) -> String {
        match self {
            Self::Inherit => "ActorInput::Inherit".into(),
            Self::Player { argument } => {
                format!("ActorInput::Player(source::Player::clone(a{argument}))")
            }
            Self::Batch { argument } => {
                format!("ActorInput::Batch(source::Players::clone(a{argument}))")
            }
        }
    }
}
impl Bounds {
    fn arguments(&self, out: &mut Vec<usize>) {
        match self {
            Self::UpToSource { argument } | Self::RepeatSource { argument } => out.push(*argument),
            _ => {}
        }
    }
    fn generate(&self) -> String {
        match self {
            Self::Exact { count } => format!("ModeBounds::Exact({count})"),
            Self::All => "ModeBounds::All".into(),
            Self::OneOrMore => "ModeBounds::OneOrMore".into(),
            Self::AnyNumber => "ModeBounds::AnyNumber".into(),
            Self::UpTo { count } => format!("ModeBounds::UpTo({count})"),
            Self::UpToSource { argument } => {
                format!("ModeBounds::UpToSource(number_tree(a{argument}))")
            }
            Self::RepeatSource { argument } => {
                format!("ModeBounds::RepeatSource(number_tree(a{argument}))")
            }
        }
    }
}
impl Recipe {
    fn arguments(&self, out: &mut Vec<usize>) -> Result<(), String> {
        match self {
            Self::Actor { actor, body } | Self::Optional { actor, body } => {
                actor.arguments(out);
                out.push(body.argument());
                if matches!(
                    self,
                    Self::Optional {
                        actor: Actor::Batch { .. },
                        ..
                    }
                ) {
                    return Err("batch optional decisions need an explicit iteration policy".into());
                }
            }
            Self::Conditional {
                condition,
                then,
                otherwise,
                ..
            } => {
                out.push(*condition);
                out.push(then.argument());
                out.extend(otherwise.as_ref().map(Body::argument));
            }
            Self::Payment { player, cost, .. } => {
                out.extend(player);
                out.push(*cost);
            }
            Self::Sequence { body } => out.push(body.argument()),
            Self::Targeted { targets, body } => {
                out.push(*targets);
                out.push(body.argument());
            }
            Self::Modal { bounds, modes } => {
                bounds.arguments(out);
                out.push(*modes);
            }
        }
        Ok(())
    }
    fn generate(&self) -> String {
        match self {
            Self::Actor {actor,body}=>format!("InstructionRecipe::Actor {{ actor: {},body:Box::new({}) }}",actor.generate(),body.generate()),
            Self::Optional {actor,body}=>format!("InstructionRecipe::Optional {{ actor: {},body:Box::new({}) }}",actor.generate(),body.generate()),
            Self::Conditional {condition,negated,then,otherwise}=>format!("InstructionRecipe::Conditional {{ condition:check_tree(a{condition}),negated:{negated},then:Box::new({}),otherwise:Box::new({}) }}",then.generate(),otherwise.as_ref().map_or("InstructionRecipe::Sequence(vec![])".into(),Body::generate)),
            Self::Payment {player,cost,optional}=>format!("InstructionRecipe::Payment {{ player:{},cost:cost_tree(a{cost}),optional:{optional} }}",player.map_or("None".into(),|i|format!("Some(source::Player::clone(a{i}))"))),
            Self::Sequence {body}=>body.generate(),
            Self::Targeted {targets,body}=>format!("InstructionRecipe::Targeted {{ targets:a{targets}.clone(),body:Box::new({}) }}",body.generate()),
            Self::Modal {bounds,modes}=>format!("InstructionRecipe::Modal {{ bounds:{},modes:a{modes}.iter().map(program_tree).collect() }}",bounds.generate()),
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
            return Err("unsupported program registry version".into());
        }
        let mut ids = BTreeSet::new();
        let mut sources = BTreeSet::new();
        for r in &self.rules {
            if r.id.is_empty() || !ids.insert(&r.id) || !sources.insert((r.domain, &r.source)) {
                return Err("duplicate program ID/domain/source".into());
            }
            if r.source.is_empty()
                || !r.source.chars().enumerate().all(|(i, c)| {
                    c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit())
                })
            {
                return Err("invalid Rust program variant".into());
            }
            let mut args = vec![];
            r.recipe.arguments(&mut args)?;
            let mut used = BTreeSet::new();
            for i in args {
                if i >= r.arity || !used.insert(i) {
                    return Err("invalid/reused program field".into());
                }
            }
            if used.len() != r.arity {
                return Err("unconsumed program field".into());
            }
            let body_is_resolution = |b: &Body| !matches!(b, Body::Program { .. });
            let valid = match (&r.domain, &r.recipe) {
                (Domain::Action, Recipe::Actor { actor, body }) => {
                    !matches!(actor, Actor::Inherit) && matches!(body, Body::Action { .. })
                }
                (Domain::Action, Recipe::Optional { body, .. }) => body_is_resolution(body),
                (
                    Domain::Action,
                    Recipe::Conditional {
                        then, otherwise, ..
                    },
                ) => {
                    matches!(then, Body::List { .. })
                        && otherwise
                            .as_ref()
                            .is_none_or(|b| matches!(b, Body::List { .. }))
                }
                (Domain::Action, Recipe::Payment { .. }) => true,
                (
                    Domain::Actions,
                    Recipe::Sequence {
                        body: Body::List { .. },
                    },
                ) => true,
                (
                    Domain::Actions,
                    Recipe::Targeted {
                        body: Body::Program { .. },
                        ..
                    },
                ) => true,
                (Domain::Actions, Recipe::Modal { bounds, .. }) => match bounds {
                    Bounds::Exact { count } => *count > 0,
                    Bounds::UpTo { count } => *count >= 0,
                    _ => true,
                },
                _ => false,
            };
            if !valid {
                return Err("incompatible program recipe/domain/body policy".into());
            }
        }
        Ok(())
    }
    pub fn generate(&self) -> String {
        let mut out = "// Generated from rewrites/programs.json5.\n".to_string();
        for (domain, name) in [
            (Domain::Action, "generated_action"),
            (Domain::Actions, "generated_program"),
        ] {
            let ty = domain.name();
            out.push_str(&format!("fn {name}(value:&source::{ty})->Option<InstructionRecipe> {{ Some(match value {{\n"));
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
                    r.recipe.generate()
                ));
            }
            out.push_str("_=>return None,\n})}\n");
        }
        out
    }
}
