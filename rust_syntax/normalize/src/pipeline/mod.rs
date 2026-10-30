//! Compiler stages. Intermediate source/recipe data is deliberately separate
//! from the portable semantic IR; only validated final cards can be exported.
pub mod candidates;
pub mod mechanics;
pub mod numbers;
pub mod operands;
pub mod programs;
pub mod selectors;
pub use numbers::{
    number_tree, NumberObject, NumberPlayers, NumberQuery, NumberRecipients, Quantity,
};
pub mod report;
#[path = "source.rs"]
pub mod source_tree;
pub use crate::rewrite_spec::{
    Destination as DestinationInput, Family, ObjectOperation, PlayerOperation, Registry,
};
use crate::{ir, mtg_types as source_types};
use serde::{Deserialize, Serialize};
use source_tree::{NodeId, SourceIndex};
use source_types as source;

pub fn registry() -> &'static Registry {
    static REGISTRY: std::sync::OnceLock<Registry> = std::sync::OnceLock::new();
    REGISTRY.get_or_init(|| {
        Registry::parse(include_str!("../../rewrites/actions.json5"))
            .expect("validated rewrite registry")
    })
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Subjects {
    Permanent(source::Permanent),
    Permanents(source::Permanents),
    GraveCard(source::CardInGraveyards),
    GraveCards(source::CardsInGraveyards),
    ExiledCard(source::CardInExile),
    ExiledCards(source::CardsInExile),
    Spell(source::Spell),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DurationInput {
    Indefinite,
    Source(source::Expiration),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SelectionInput {
    AsManyAsCan(Quantity),
    UpTo(Quantity),
    AnyNumber,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SurfaceAction {
    Control {
        subjects: Subjects,
        selection: Option<SelectionInput>,
        duration: DurationInput,
    },
    Move {
        subjects: Subjects,
        selection: Option<SelectionInput>,
        destination: DestinationInput,
        entry: Vec<source::EnterFlag>,
    },
    PlayerAmount {
        operation: PlayerOperation,
        amount: Quantity,
    },
    ObjectEffect {
        subjects: Subjects,
        selection: Option<SelectionInput>,
        operation: ObjectOperation,
    },
    ContinuousEffect {
        subjects: Subjects,
        change: ChangeTemplate,
        duration: DurationInput,
    },
}
/// Generic operation templates with typed unresolved operands. Binding is a
/// distinct pass; intermediate source operands cannot escape into final Card.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ExpandedRecipe {
    ContinuousEffect {
        subjects: Subjects,
        selection: Option<SelectionInput>,
        change: ChangeTemplate,
        duration: DurationInput,
        evaluation: ir::EffectEvaluation,
    },
    Move {
        subjects: Subjects,
        selection: Option<SelectionInput>,
        destination: DestinationInput,
        entry: Vec<source::EnterFlag>,
    },
    PlayerAmount {
        operation: PlayerOperation,
        amount: Quantity,
    },
    ObjectEffect {
        subjects: Subjects,
        selection: Option<SelectionInput>,
        operation: ObjectOperation,
    },
}
impl ExpandedRecipe {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::ContinuousEffect { .. } => "ContinuousEffect",
            Self::Move { .. } => "Move",
            Self::PlayerAmount { .. } => "PlayerAmount",
            Self::ObjectEffect { .. } => "ObjectEffect",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ChangeTemplate {
    ControllerOfActor,
    LayerEffects(Vec<source::LayerEffect>),
    Rules(Vec<source::PermanentRule>),
}
impl SurfaceAction {
    pub fn expand(self) -> ExpandedRecipe {
        match self {
            Self::Control {
                subjects,
                selection,
                duration,
            } => ExpandedRecipe::ContinuousEffect {
                subjects,
                selection,
                change: ChangeTemplate::ControllerOfActor,
                duration,
                evaluation: ir::EffectEvaluation::Snapshot,
            },
            Self::Move {
                subjects,
                selection,
                destination,
                entry,
            } => ExpandedRecipe::Move {
                subjects,
                selection,
                destination,
                entry,
            },
            Self::PlayerAmount { operation, amount } => {
                ExpandedRecipe::PlayerAmount { operation, amount }
            }
            Self::ObjectEffect {
                subjects,
                selection,
                operation,
            } => ExpandedRecipe::ObjectEffect {
                subjects,
                selection,
                operation,
            },
            Self::ContinuousEffect {
                subjects,
                change,
                duration,
            } => {
                let evaluation = if matches!(change, ChangeTemplate::Rules(_)) {
                    ir::EffectEvaluation::LiveObjects
                } else {
                    ir::EffectEvaluation::Snapshot
                };
                ExpandedRecipe::ContinuousEffect {
                    subjects,
                    selection: None,
                    change,
                    duration,
                    evaluation,
                }
            }
        }
    }
}
include!(concat!(env!("OUT_DIR"), "/pipeline_rewrites.rs"));
pub fn rewrite(action: &source::Action) -> Option<SurfaceAction> {
    generated_rewrite(action)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rewrite {
    pub origin: NodeId,
    pub rule: String,
    pub surface: SurfaceAction,
    pub expanded: ExpandedRecipe,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Structural {
    pub registries: std::collections::BTreeMap<String, String>,
    pub selectors: Vec<SelectorRewrite>,
    pub programs: Vec<ProgramRewrite>,
    pub operands: Vec<OperandRewrite>,
    pub mechanics: Vec<MechanicRewrite>,
    pub numbers: Vec<NumberRewrite>,
    pub source: SourceIndex,
    pub rewrites: Vec<Rewrite>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NumberRewrite {
    pub origin: NodeId,
    pub rule: String,
    pub source_key: String,
    pub expression: Quantity,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelectorRewrite {
    pub origin: NodeId,
    pub rule: String,
    pub source_key: String,
    pub predicate: selectors::Predicate,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProgramRewrite {
    pub origin: NodeId,
    pub rule: String,
    pub source_key: String,
    pub recipe: programs::InstructionRecipe,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OperandRewrite {
    pub origin: NodeId,
    pub rule: String,
    pub source_key: String,
    pub recipe: operands::OperandRecipe,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MechanicRewrite {
    pub origin: NodeId,
    pub rule: String,
    pub source_key: String,
    pub recipe: mechanics::MechanicRecipe,
}
impl Structural {
    pub fn new(card: &source::OracleCard) -> Result<Self, String> {
        let source = SourceIndex::new(card);
        let mut rewrites = vec![];
        let mut numbers = vec![];
        let mut selectors = vec![];
        let mut programs = vec![];
        let mut operands = vec![];
        let mut mechanics = vec![];
        for node in &source.nodes {
            if node.source_type == "Action" {
                if let Some(rule) = registry().rules.iter().find(|r| r.source == node.form) {
                    let action: source::Action = source_tree::decode(source.at(&node.id))
                        .map_err(|e| format!("{}: {e}", node.pointer))?;
                    let surface = rewrite(&action).ok_or("registry/generated rewriter disagree")?;
                    rewrites.push(Rewrite {
                        origin: node.id.clone(),
                        rule: rule.id.clone(),
                        expanded: surface.clone().expand(),
                        surface,
                    });
                }
            } else if node.source_type == "GameNumber" {
                if let Some(rule) = numbers::registry()
                    .rules
                    .iter()
                    .find(|r| r.source == node.form)
                {
                    let number: source::GameNumber = source_tree::decode(source.at(&node.id))
                        .map_err(|e| format!("{}: {e}", node.pointer))?;
                    numbers.push(NumberRewrite {
                        origin: node.id.clone(),
                        rule: rule.id.clone(),
                        source_key: numbers::number_key(&number),
                        expression: numbers::rewrite_number(&number)
                            .ok_or("numeric registry/generated rewriter disagree")?,
                    });
                }
            } else if let Some(domain) =
                crate::selector_spec::Domain::from_rust_name(&node.source_type)
            {
                if let Some(rule) = selectors::registry()
                    .rules
                    .iter()
                    .find(|r| r.domain == domain && r.source == node.form)
                {
                    let value = selectors::SelectorSource::decode(domain, source.at(&node.id))
                        .map_err(|e| format!("{}: {e}", node.pointer))?;
                    selectors.push(SelectorRewrite {
                        origin: node.id.clone(),
                        rule: rule.id.clone(),
                        source_key: selectors::source_key(value.as_ref()),
                        predicate: selectors::rewrite(value.as_ref())
                            .ok_or("selector registry/generated rewriter disagree")?,
                    });
                }
            }
        }
        for node in &source.nodes {
            let domain = match node.source_type.as_str() {
                "Action" => crate::program_spec::Domain::Action,
                "Actions" => crate::program_spec::Domain::Actions,
                _ => continue,
            };
            if let Some(rule) = programs::registry()
                .rules
                .iter()
                .find(|r| r.domain == domain && r.source == node.form)
            {
                let value = programs::ProgramSource::decode(domain, source.at(&node.id))
                    .map_err(|e| format!("{}: {e}", node.pointer))?;
                programs.push(ProgramRewrite {
                    origin: node.id.clone(),
                    rule: rule.id.clone(),
                    source_key: programs::source_key(value.as_ref()),
                    recipe: programs::rewrite(value.as_ref())
                        .ok_or("program registry/generated rewriter disagree")?,
                });
            }
        }
        for node in &source.nodes {
            let domain = match node.source_type.as_str() {
                "Cost" => crate::operand_spec::Domain::Cost,
                "Condition" => crate::operand_spec::Domain::Condition,
                _ => continue,
            };
            if let Some(rule) = operands::registry()
                .rules
                .iter()
                .find(|r| r.domain == domain && r.source == node.form)
            {
                let value = operands::OperandSource::decode(domain, source.at(&node.id))
                    .map_err(|e| format!("{}: {e}", node.pointer))?;
                operands.push(OperandRewrite {
                    origin: node.id.clone(),
                    rule: rule.id.clone(),
                    source_key: operands::source_key(value.as_ref()),
                    recipe: operands::rewrite(value.as_ref())
                        .ok_or("operand registry/generated rewriter disagree")?,
                });
            }
        }
        for node in &source.nodes {
            if node.source_type != "Rule" {
                continue;
            }
            if let Some(rule) = mechanics::registry()
                .rules
                .iter()
                .find(|r| r.source == node.form)
            {
                let value: source::Rule = source_tree::decode(source.at(&node.id))
                    .map_err(|e| format!("{}: {e}", node.pointer))?;
                mechanics.push(MechanicRewrite {
                    origin: node.id.clone(),
                    rule: rule.id.clone(),
                    source_key: mechanics::source_key(&value),
                    recipe: mechanics::rewrite(&value)
                        .ok_or("mechanic registry/generated rewriter disagree")?,
                });
            }
        }
        Ok(Self {
            registries: registry_fingerprints().clone(),
            selectors,
            programs,
            operands,
            mechanics,
            source,
            rewrites,
            numbers,
        })
    }
}
#[derive(Default)]
pub(crate) struct Prepared {
    pub mechanics: std::collections::BTreeMap<String, mechanics::MechanicRecipe>,
    pub operands: std::collections::BTreeMap<String, operands::OperandRecipe>,
    pub programs: std::collections::BTreeMap<String, programs::InstructionRecipe>,
    pub actions: std::collections::BTreeMap<String, ExpandedRecipe>,
    pub numbers: std::collections::BTreeMap<String, Quantity>,
    pub selectors: std::collections::BTreeMap<String, selectors::Predicate>,
}
pub(crate) type PreparedRecipes = std::sync::Arc<Prepared>;
pub(crate) fn surface_key(surface: &SurfaceAction) -> String {
    serde_json::to_string(surface).expect("typed structural recipe")
}
pub(crate) fn bind_prepared(
    source: &source::OracleCard,
    structural: &Structural,
) -> Result<ir::Card, Vec<crate::lower::Diagnostic>> {
    let prepared = std::sync::Arc::new(Prepared {
        mechanics: structural
            .mechanics
            .iter()
            .map(|r| (r.source_key.clone(), r.recipe.clone()))
            .collect(),
        operands: structural
            .operands
            .iter()
            .map(|r| (r.source_key.clone(), r.recipe.clone()))
            .collect(),
        programs: structural
            .programs
            .iter()
            .map(|r| (r.source_key.clone(), r.recipe.clone()))
            .collect(),
        selectors: structural
            .selectors
            .iter()
            .map(|r| (r.source_key.clone(), r.predicate.clone()))
            .collect(),
        actions: structural
            .rewrites
            .iter()
            .map(|r| (surface_key(&r.surface), r.expanded.clone()))
            .collect(),
        numbers: structural
            .numbers
            .iter()
            .map(|r| (r.source_key.clone(), r.expression.clone()))
            .collect(),
    });
    crate::lower::bind_card(source, prepared)
}
pub(crate) fn lower(
    source: &source::OracleCard,
) -> Result<ir::Card, Vec<crate::lower::Diagnostic>> {
    // Ordinary native lowering fuses local rewrite/expansion/binding through
    // generated adapters. Whole-tree provenance and partial-stage retention
    // belong to Structural/report/Intermediate, not every targeted lowering.
    crate::lower::bind_card(source, Default::default())
}
fn stage_error(message: String) -> Vec<crate::lower::Diagnostic> {
    vec![crate::lower::Diagnostic {
        path: "pipeline".into(),
        source_type: "pipeline".into(),
        form: "invalid-stage".into(),
        message,
    }]
}
/// Bind a saved structural/expanded artifact. Verify the registry, provenance,
/// and generated recipes before accepting it; cached final_card data is never
/// trusted. Source data is used for the still-existing native handlers.
pub fn bind(structural: &Structural) -> Result<ir::Card, Vec<crate::lower::Diagnostic>> {
    let source = structural
        .source
        .restore()
        .map_err(|e| stage_error(e.to_string()))?;
    let expected = Structural::new(&source).map_err(stage_error)?;
    if serde_json::to_value(&expected).unwrap() != serde_json::to_value(structural).unwrap() {
        return Err(stage_error(
            "staged data/provenance or rewrite registry changed".into(),
        ));
    }
    let card = bind_prepared(&source, structural)?;
    validate_final(card)
}
fn validate_final(value: ir::Card) -> Result<ir::Card, Vec<crate::lower::Diagnostic>> {
    crate::validate::card(&value).map_err(|message| {
        vec![crate::lower::Diagnostic {
            path: "finalization".into(),
            source_type: "invariant".into(),
            form: "invalid".into(),
            message,
        }]
    })?;
    Ok(value)
}
/// The semantic lowerer binds both generated templates and existing native
/// handlers. Unresolved data stays in Structural and diagnostics, not Card.
pub fn finalize(card: &source::OracleCard) -> Result<ir::Card, Vec<crate::lower::Diagnostic>> {
    let value = crate::lower::card(card)?;
    validate_final(value)
}
pub(crate) fn registry_sources() -> [(&'static str, &'static [u8]); 7] {
    [
        (
            "rust_syntax/normalize/rewrites/primitives.json5",
            include_bytes!("../../rewrites/primitives.json5").as_slice(),
        ),
        (
            "rust_syntax/normalize/rewrites/mechanics.json5",
            include_bytes!("../../rewrites/mechanics.json5").as_slice(),
        ),
        (
            "rust_syntax/normalize/rewrites/operands.json5",
            include_bytes!("../../rewrites/operands.json5").as_slice(),
        ),
        (
            "rust_syntax/normalize/rewrites/programs.json5",
            include_bytes!("../../rewrites/programs.json5").as_slice(),
        ),
        (
            "rust_syntax/normalize/rewrites/actions.json5",
            include_bytes!("../../rewrites/actions.json5").as_slice(),
        ),
        (
            "rust_syntax/normalize/rewrites/numbers.json5",
            include_bytes!("../../rewrites/numbers.json5").as_slice(),
        ),
        (
            "rust_syntax/normalize/rewrites/selectors.json5",
            include_bytes!("../../rewrites/selectors.json5").as_slice(),
        ),
    ]
}
pub fn registry_fingerprints() -> &'static std::collections::BTreeMap<String, String> {
    static HASHES: std::sync::OnceLock<std::collections::BTreeMap<String, String>> =
        std::sync::OnceLock::new();
    HASHES.get_or_init(|| {
        registry_sources()
            .into_iter()
            .map(|(path, data)| (path.into(), crate::corpus::digest(data)))
            .collect()
    })
}
pub fn input_fingerprints(
    root: &std::path::Path,
) -> crate::corpus::Result<std::collections::BTreeMap<String, String>> {
    let mut inputs = crate::corpus::input_fingerprints(root)?;
    for (path, compiled) in registry_sources() {
        let data = std::fs::read(root.join(path))?;
        if data != compiled {
            return Err(
                "rewrite registry changed since this binary was built; rebuild the ast tool".into(),
            );
        }
    }
    inputs.extend(registry_fingerprints().clone());
    Ok(inputs)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Intermediate {
    pub format: String,
    pub structural: Structural,
    pub final_card: Option<ir::Card>,
    pub diagnostics: Vec<crate::lower::Diagnostic>,
}
impl Intermediate {
    pub fn new(source: &source::OracleCard) -> Result<Self, String> {
        let structural = Structural::new(source)?;
        let (final_card, diagnostics) =
            match bind_prepared(source, &structural).and_then(validate_final) {
                Ok(card) => (Some(card), vec![]),
                Err(errors) => (None, errors),
            };
        Ok(Self {
            format: "mtgish.pipeline.intermediate.v8".into(),
            structural,
            final_card,
            diagnostics,
        })
    }
}
