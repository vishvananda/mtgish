//! Reviewed rewrite descriptions shared by the build-time generator and tools.
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
    pub family: Family,
    pub subjects: Option<Subjects>,
    pub duration: Option<usize>,
    pub destination: Option<Destination>,
    pub entry: Option<usize>,
    pub changes: Option<usize>,
    pub amount: Option<Number>,
    pub selection: Option<Selection>,
    pub player_operation: Option<PlayerOperation>,
    pub object_operation: Option<ObjectOperation>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Family {
    Control,
    Move,
    PlayerAmount,
    ObjectEffect,
    LayerEffect,
    RuleEffect,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SubjectKind {
    Permanent,
    Permanents,
    GraveCard,
    GraveCards,
    ExiledCard,
    ExiledCards,
    Spell,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Subjects {
    pub kind: SubjectKind,
    pub argument: usize,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Destination {
    OwnersHand,
    OwnersGraveyard,
    OwnersLibraryTop,
    OwnersLibraryBottom,
    OwnersLibraryShuffled,
    BattlefieldYou,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub enum PlayerOperation {
    Draw,
    Discard { random: bool },
    GainLife,
    LoseLife,
    Mill,
    Scry,
    Surveil,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub enum ObjectOperation {
    Tap { tapped: bool },
    Destroy { regeneration: bool },
    Sacrifice,
    Regenerate,
}
/// Source field positions are only present in the reviewed registry. Generated
/// code replaces them with actual typed Rust values before any binding occurs.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Number {
    Constant {
        value: i64,
    },
    Source {
        argument: usize,
    },
    Multiply {
        left: Box<Number>,
        right: Box<Number>,
    },
    Add {
        left: Box<Number>,
        right: Box<Number>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Selection {
    AsManyAsCan { amount: Number },
    UpTo { amount: Number },
    AnyNumber,
}
impl Number {
    fn arguments(&self, out: &mut Vec<usize>) {
        match self {
            Self::Source { argument } => out.push(*argument),
            Self::Multiply { left, right } | Self::Add { left, right } => {
                left.arguments(out);
                right.arguments(out);
            }
            Self::Constant { .. } => {}
        }
    }
    fn generate(&self) -> String {
        match self {
            Self::Constant { value } => format!("Quantity::Constant({value})"),
            Self::Source { argument } => {
                format!("number_tree(a{argument})")
            }
            Self::Multiply { left, right } => format!(
                "Quantity::Multiply(Box::new({}),Box::new({}))",
                left.generate(),
                right.generate()
            ),
            Self::Add { left, right } => format!(
                "Quantity::Add(Box::new({}),Box::new({}))",
                left.generate(),
                right.generate()
            ),
        }
    }
}
impl Selection {
    fn arguments(&self, out: &mut Vec<usize>) {
        match self {
            Self::AsManyAsCan { amount } | Self::UpTo { amount } => amount.arguments(out),
            Self::AnyNumber => {}
        }
    }
    fn generate(&self) -> String {
        match self {
            Self::AsManyAsCan { amount } => {
                format!("SelectionInput::AsManyAsCan({})", amount.generate())
            }
            Self::UpTo { amount } => format!("SelectionInput::UpTo({})", amount.generate()),
            Self::AnyNumber => "SelectionInput::AnyNumber".into(),
        }
    }
}
impl Registry {
    pub fn parse(text: &str) -> Result<Self, String> {
        let value: Self = serde_json::from_str(text).map_err(|e| e.to_string())?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 2 {
            return Err("unsupported rewrite registry version".into());
        }
        let mut ids = BTreeSet::new();
        let mut sources = BTreeSet::new();
        for r in &self.rules {
            if r.id.is_empty() || !ids.insert(&r.id) || !sources.insert(&r.source) {
                return Err(format!("empty/duplicate rewrite ID or source: {}", r.id));
            }
            if r.source.is_empty()
                || !r.source.chars().enumerate().all(|(i, c)| {
                    c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit())
                })
            {
                return Err(format!("invalid Rust variant name: {}", r.source));
            }
            let mut arguments: Vec<usize> = [
                r.subjects.as_ref().map(|s| s.argument),
                r.duration,
                r.entry,
                r.changes,
            ]
            .into_iter()
            .flatten()
            .collect();
            if let Some(n) = &r.amount {
                n.arguments(&mut arguments);
            }
            if let Some(s) = &r.selection {
                s.arguments(&mut arguments);
            }
            let mut consumed = BTreeSet::new();
            for arg in arguments {
                if arg >= r.arity || !consumed.insert(arg) {
                    return Err(format!("invalid/reused argument in {}", r.id));
                }
            }
            if consumed.len() != r.arity {
                return Err(format!("unconsumed source field in {}", r.id));
            }
            let shape = match r.family {
                Family::Control => {
                    r.subjects.is_some()
                        && r.changes.is_none()
                        && r.destination.is_none()
                        && r.entry.is_none()
                }
                Family::Move => {
                    r.subjects.is_some()
                        && r.changes.is_none()
                        && r.destination.is_some()
                        && r.duration.is_none()
                }
                Family::LayerEffect | Family::RuleEffect => {
                    r.subjects.is_some()
                        && r.changes.is_some()
                        && r.destination.is_none()
                        && r.entry.is_none()
                        && r.selection.is_none()
                }
                Family::PlayerAmount => {
                    r.subjects.is_none()
                        && r.amount.is_some()
                        && r.player_operation.is_some()
                        && r.duration.is_none()
                        && r.destination.is_none()
                        && r.entry.is_none()
                        && r.changes.is_none()
                        && r.selection.is_none()
                }
                Family::ObjectEffect => {
                    r.subjects.is_some()
                        && r.object_operation.is_some()
                        && r.duration.is_none()
                        && r.destination.is_none()
                        && r.entry.is_none()
                        && r.changes.is_none()
                }
            };
            if !shape
                || (r.family != Family::PlayerAmount
                    && (r.amount.is_some() || r.player_operation.is_some()))
                || (r.family != Family::ObjectEffect && r.object_operation.is_some())
            {
                return Err(format!("incompatible recipe fields: {}", r.id));
            }
            if r.entry.is_some() && r.destination != Some(Destination::BattlefieldYou) {
                return Err(format!("entry flags on a non-battlefield move: {}", r.id));
            }
            if r.selection.is_some()
                && !matches!(
                    r.subjects.as_ref().map(|s| s.kind),
                    Some(
                        SubjectKind::Permanents
                            | SubjectKind::GraveCards
                            | SubjectKind::ExiledCards
                    )
                )
            {
                return Err(format!("selection requires a collection in {}", r.id));
            }
            if matches!(
                r.family,
                Family::ObjectEffect | Family::LayerEffect | Family::RuleEffect
            ) && !matches!(
                r.subjects.as_ref().map(|s| s.kind),
                Some(SubjectKind::Permanent | SubjectKind::Permanents)
            ) {
                return Err(format!(
                    "permanent effect requires battlefield subjects: {}",
                    r.id
                ));
            }
        }
        Ok(())
    }
    /// Rust checks arities/types; validation separately checks field accounting.
    pub fn generate(&self) -> String {
        let mut out=String::from("// Generated from rewrites/actions.json5; edit the registry.\nfn generated_rewrite(value: &source::Action) -> Option<SurfaceAction> {\n Some(match value {\n");
        for r in &self.rules {
            let args = (0..r.arity)
                .map(|i| format!("a{i}"))
                .collect::<Vec<_>>()
                .join(",");
            let pattern = if r.arity == 0 {
                format!("source::Action::{}", r.source)
            } else {
                format!("source::Action::{}({args})", r.source)
            };
            let subjects = r
                .subjects
                .as_ref()
                .map(|s| {
                    let (variant, ty) = match s.kind {
                        SubjectKind::Permanent => ("Permanent", "Permanent"),
                        SubjectKind::Permanents => ("Permanents", "Permanents"),
                        SubjectKind::GraveCard => ("GraveCard", "CardInGraveyards"),
                        SubjectKind::GraveCards => ("GraveCards", "CardsInGraveyards"),
                        SubjectKind::ExiledCard => ("ExiledCard", "CardInExile"),
                        SubjectKind::ExiledCards => ("ExiledCards", "CardsInExile"),
                        SubjectKind::Spell => ("Spell", "Spell"),
                    };
                    format!("Subjects::{variant}(source::{ty}::clone(a{}))", s.argument)
                })
                .unwrap_or_default();
            let duration = r.duration.map_or("DurationInput::Indefinite".into(), |i| {
                format!("DurationInput::Source(source::Expiration::clone(a{i}))")
            });
            let selection = r
                .selection
                .as_ref()
                .map_or("None".into(), |s| format!("Some({})", s.generate()));
            let payload=match r.family {
                Family::Control=>format!("SurfaceAction::Control {{ subjects: {subjects}, selection: {selection}, duration: {duration} }}"),
                Family::Move=>format!("SurfaceAction::Move {{ subjects: {subjects}, selection: {selection}, destination: DestinationInput::{:?}, entry: {} }}",r.destination.unwrap(),r.entry.map_or("vec![]".into(),|i|format!("a{i}.clone()"))),
                Family::PlayerAmount=>format!("SurfaceAction::PlayerAmount {{ operation: PlayerOperation::{:?}, amount: {} }}",r.player_operation.unwrap(),r.amount.as_ref().unwrap().generate()),
                Family::ObjectEffect=>format!("SurfaceAction::ObjectEffect {{ subjects: {subjects}, selection: {selection}, operation: ObjectOperation::{:?} }}",r.object_operation.unwrap()),
                Family::LayerEffect | Family::RuleEffect=>format!("SurfaceAction::ContinuousEffect {{ subjects: {subjects}, change: ChangeTemplate::{}(a{}.clone()), duration: {duration} }}",if r.family==Family::LayerEffect {"LayerEffects"} else {"Rules"},r.changes.unwrap()),
            };
            out.push_str(&format!("{pattern} => {payload},\n"));
        }
        out.push_str("_ => return None,\n })\n}\n");
        out
    }
}
