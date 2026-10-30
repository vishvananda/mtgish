//! Structural instruction recipes. Binding, result allocation, and legality are
//! later operations. Unresolved source is permitted here, never in final Card.
use super::{
    numbers::number_tree,
    operands::{check_tree, cost_tree, CheckRecipe, CostRecipe},
    source_tree, ExpandedRecipe, Quantity,
};
use crate::{
    mtg_types as source,
    program_spec::{Domain, Registry},
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum InstructionRecipe {
    Source(ProgramSource),
    Effect(ExpandedRecipe),
    Sequence(Vec<InstructionRecipe>),
    Actor {
        actor: ActorInput,
        body: Box<InstructionRecipe>,
    },
    Optional {
        actor: ActorInput,
        body: Box<InstructionRecipe>,
    },
    Conditional {
        condition: CheckRecipe,
        negated: bool,
        then: Box<InstructionRecipe>,
        otherwise: Box<InstructionRecipe>,
    },
    Payment {
        player: Option<source::Player>,
        cost: CostRecipe,
        optional: bool,
    },
    Targeted {
        targets: Vec<source::Target>,
        body: Box<InstructionRecipe>,
    },
    Modal {
        bounds: ModeBounds,
        modes: Vec<InstructionRecipe>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ActorInput {
    Inherit,
    Player(source::Player),
    Batch(source::Players),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ModeBounds {
    Exact(i64),
    All,
    OneOrMore,
    AnyNumber,
    UpTo(i64),
    UpToSource(Quantity),
    RepeatSource(Quantity),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ProgramSource {
    Action(source::Action),
    Actions(source::Actions),
}
#[derive(Clone, Copy, Serialize)]
pub enum ProgramRef<'a> {
    Action(&'a source::Action),
    Actions(&'a source::Actions),
}
impl ProgramSource {
    pub fn as_ref(&self) -> ProgramRef<'_> {
        match self {
            Self::Action(a) => ProgramRef::Action(a),
            Self::Actions(a) => ProgramRef::Actions(a),
        }
    }
    pub fn decode(domain: Domain, value: &serde_json::Value) -> Result<Self, String> {
        match domain {
            Domain::Action => source_tree::decode(value)
                .map(Self::Action)
                .map_err(|e| e.to_string()),
            Domain::Actions => source_tree::decode(value)
                .map(Self::Actions)
                .map_err(|e| e.to_string()),
        }
    }
}
pub fn registry() -> &'static Registry {
    static REGISTRY: std::sync::OnceLock<Registry> = std::sync::OnceLock::new();
    REGISTRY.get_or_init(|| {
        Registry::parse(include_str!("../../rewrites/programs.json5"))
            .expect("validated program registry")
    })
}
pub fn source_key(value: ProgramRef<'_>) -> String {
    serde_json::to_string(&value).expect("typed program source")
}
pub fn rewrite(value: ProgramRef<'_>) -> Option<InstructionRecipe> {
    match value {
        ProgramRef::Action(a) => generated_action(a),
        ProgramRef::Actions(a) => generated_program(a),
    }
}
pub fn action_tree(value: &source::Action) -> InstructionRecipe {
    generated_action(value).unwrap_or_else(|| {
        super::rewrite(value)
            .map(|s| InstructionRecipe::Effect(s.expand()))
            .unwrap_or_else(|| InstructionRecipe::Source(ProgramSource::Action(value.clone())))
    })
}
pub fn program_tree(value: &source::Actions) -> InstructionRecipe {
    generated_program(value)
        .unwrap_or_else(|| InstructionRecipe::Source(ProgramSource::Actions(value.clone())))
}
include!(concat!(env!("OUT_DIR"), "/pipeline_programs.rs"));
