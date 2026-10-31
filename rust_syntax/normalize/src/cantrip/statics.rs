//! Static abilities as layer callbacks. Selectors and values stay live: every
//! read here happens as the layers ask (CR 611.3), through a `LayerQuery`, and a
//! change never reads the `Chars` it writes.
use super::*;

const LAYER_PRELUDE: &str = "let asker = &Asker::new(source, q.controller(source));";

/// What one change does in one layer: `body` writes `chars`.
struct LayerChange {
    layer: &'static str,
    body: String,
    /// `loses_all_abilities` and `control_changing` have no apply callback.
    whole: Option<&'static str>,
    /// The value reads the object being changed, so it needs `apply_each`.
    each: bool,
}
impl LayerChange {
    fn of(layer: &'static str, body: String) -> Self {
        Self {
            layer,
            body,
            whole: None,
            each: false,
        }
    }
}

impl Compiler {
    fn layered<T>(&mut self, part: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        let saved = (self.reader, self.event_at);
        (self.reader, self.event_at) = (Reader::Layer, EventAt::Nowhere);
        let compiled = part(self);
        (self.reader, self.event_at) = saved;
        compiled
    }
    fn layer_change(&mut self, change: &Change) -> Result<LayerChange> {
        let i16_of = |c: &Self, n: &Expr| -> Result<String> {
            c.number(n)?.fit(i16::MIN.into(), i16::MAX.into(), "i16")
        };
        Ok(match change {
            Change::Primitive { name, .. } => {
                return Err(gap(format!("primitive static change {name}")))
            }
            Change::Pump { power, toughness } => LayerChange::of(
                "PtModify",
                format!(
                    "let (p, t) = ({}, {}); chars.power = chars.power.map(|v| v.saturating_add(p)); chars.toughness = chars.toughness.map(|v| v.saturating_add(t));",
                    i16_of(self, power)?,
                    i16_of(self, toughness)?
                ),
            ),
            Change::SetBasePowerToughness { power, toughness } => LayerChange::of(
                "PtSet",
                format!(
                    "let (p, t) = ({}, {}); chars.power = Some(p); chars.toughness = Some(t);",
                    i16_of(self, power)?,
                    i16_of(self, toughness)?
                ),
            ),
            Change::SetPower(n) => LayerChange::of(
                "PtSet",
                format!("chars.power = Some({});", i16_of(self, n)?),
            ),
            Change::SetToughness(n) => LayerChange::of(
                "PtSet",
                format!("chars.toughness = Some({});", i16_of(self, n)?),
            ),
            Change::GrantKeyword(k) => LayerChange::of(
                "Ability",
                format!("chars.keywords = chars.keywords.with({});", keyword(k)?),
            ),
            Change::Colors { mode, colors } => {
                let mut set = String::from("ColorSet::NONE");
                for c in colors {
                    write!(
                        set,
                        ".with(q.words(asker.source).color(Color::{}))",
                        color(c)?
                    )
                    .unwrap();
                }
                LayerChange::of(
                    "Color",
                    match mode {
                        ChangeMode::Set => format!("chars.colors = {set};"),
                        ChangeMode::Add => format!(
                            "for color in ({set}).iter() {{ chars.colors = chars.colors.with(color); }}"
                        ),
                    },
                )
            }
            Change::CreatureTypes {
                mode: ChangeMode::Add,
                types: CreatureTypeSet::Listed(types),
            } => {
                let mut body = String::new();
                for t in types {
                    let word = name(t);
                    if !crate::cantrip_vocab::SUBTYPES.contains(&word.as_str()) {
                        return Err(gap(format!("subtype {word}")));
                    }
                    write!(
                        body,
                        "chars.subtypes = chars.subtypes.with(q.words(asker.source).subtype(Subtype::{word}));"
                    )
                    .unwrap();
                }
                LayerChange::of("Type", body)
            }
            Change::RemoveAllAbilities => LayerChange {
                layer: "Ability",
                body: String::new(),
                whole: Some("loses_all_abilities"),
                each: false,
            },
            Change::SetController(PlayerRef::AbilityController) => LayerChange {
                layer: "Control",
                body: String::new(),
                whole: Some("control_changing"),
                each: false,
            },
            Change::Rule(rule) => return Err(gap(format!("static rule {}", rule_name(rule)))),
            Change::PlayerRule(rule) => {
                return Err(gap(format!("static player rule {}", form(rule))))
            }
            other => return Err(gap(format!("static change {}", form(other)))),
        })
    }
    /// One `StaticDef` per layer, joined as one effect (CR 613.6).
    pub(super) fn static_ability(
        &mut self,
        label: &str,
        kind: &StaticKind,
        affected: &EffectSubjects,
        changes: &[Change],
    ) -> Result<Vec<String>> {
        let EffectSubjects::Objects(affected) = affected else {
            return Err(gap("static ability on players"));
        };
        let defining = matches!(kind, StaticKind::CharacteristicDefining { .. });
        if defining && !matches!(affected, ObjectSet::One(ObjectRef::Source)) {
            return Err(gap("characteristic-defining ability of another object"));
        }
        let guard = self.static_guard.clone();
        let condition = guard
            .as_ref()
            .map(|c| self.layered(|c2| c2.condition(c)))
            .transpose()?;
        let affects = self.layered(|c| {
            Ok(match affected {
                ObjectSet::One(ObjectRef::Source) => match &condition {
                    None => Some("cantrip_mechanics::statics::itself".to_string()),
                    Some(_) => None,
                }
                .map_or_else(|| Ok::<_, Diagnostic>(("source == affected".to_string(), false)), |f| Ok((f, true)))?,
                ObjectSet::One(ObjectRef::AttachedTo(host)) if matches!(**host, ObjectRef::Source) => {
                    match &condition {
                        None => ("cantrip_mechanics::attach::attached_object".to_string(), true),
                        Some(_) => (
                            "q.state().obj(source).attached_to == Some(affected)".to_string(),
                            false,
                        ),
                    }
                }
                ObjectSet::Battlefield(filter) => {
                    (c.filter(filter, &Candidate::affected())?, false)
                }
                other => return Err(gap(format!("static subjects {}", form(other)))),
            })
        })?;
        let (affects, named) = affects;
        let affects = if named {
            affects
        } else {
            let function = format!("{label}_affects");
            let test = match &condition {
                Some(condition) => format!("({condition}) && ({affects})"),
                None => affects,
            };
            writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn LayerQuery, source: ObjectId, affected: ObjectId, so_far: &Chars) -> bool {{ {LAYER_PRELUDE} {test} }}").unwrap();
            function
        };
        let mut defs = Vec::new();
        for (i, change) in changes.iter().enumerate() {
            let part = self.layered(|c| c.layer_change(change))?;
            let text = quoted(&format!("Static: {}.", form(change)));
            let def = match part.whole {
                Some(builder) => format!("{builder}({affects}, {text})"),
                None => {
                    let apply = format!("{label}_apply_{i}");
                    writeln!(self.functions, "#[allow(unused_variables)] fn {apply}(q: &dyn LayerQuery, source: ObjectId, chars: &mut Chars) {{ {LAYER_PRELUDE} {} }}", part.body).unwrap();
                    if defining {
                        let layer = if part.layer == "PtSet" {
                            "PtDefine"
                        } else {
                            part.layer
                        };
                        format!("characteristic_defining(Layer::{layer}, {apply}, {text})")
                    } else {
                        format!(
                            "static_ability(Layer::{}, {affects}, {apply}, {text})",
                            part.layer
                        )
                    }
                }
            };
            let _ = part.each;
            defs.push(if i == 0 {
                def
            } else {
                format!("{def}.continues()")
            });
        }
        Ok(defs)
    }
}
