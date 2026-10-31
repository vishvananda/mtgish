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

/// The order of the `Layer` a `LayerChange` names.
fn layer_rank(layer: &str) -> u8 {
    [
        "Control", "Text", "Type", "Color", "Ability", "PtDefine", "PtSet", "PtModify",
    ]
    .iter()
    .position(|l| *l == layer)
    .expect("a layer") as u8
}

const HAS_CREATURE_TYPES: &str = "(chars.is(CardType::Creature) || chars.is(CardType::Kindred))";

impl Compiler {
    fn layered<T>(&mut self, part: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        let saved = (self.reader, self.event_at);
        (self.reader, self.event_at) = (Reader::Layer, EventAt::Nowhere);
        let compiled = part(self);
        (self.reader, self.event_at) = saved;
        compiled
    }
    /// An ability another object is given, compiled as that object's own: it
    /// sits in `CardDef::delayed`, and `AbilityRef::delayed` names it through
    /// the card printing the grant, which a granted ability is not.
    pub(super) fn grant(&mut self, ability: &Ability) -> Result<usize> {
        if self.granting {
            return Err(gap("granted ability that grants another"));
        }
        let label = format!("granted_{}", self.fresh());
        // The enclosing callback's reader belongs to it, not to this ability.
        let saved = (
            self.reader,
            self.event_at,
            self.frame,
            self.static_guard.take(),
        );
        let outer = self.outer.take();
        (self.reader, self.event_at, self.frame) =
            (Reader::Resolution, EventAt::Nowhere, Frame::default());
        let slots = std::mem::take(&mut self.slots);
        let results = self.results();
        self.granting = true;
        let mut grown = Parts::default();
        let compiled = self.ability(ability, &label, &mut grown, None);
        self.granting = false;
        self.restore_results(results);
        self.slots = slots;
        (self.reader, self.event_at, self.frame, self.static_guard) = saved;
        self.outer = outer;
        compiled?;
        let [def] = grown.abilities.as_slice() else {
            return Err(gap(format!("granted {}", form(ability))));
        };
        self.delayed.push(def.clone());
        Ok(self.delayed.len() - 1)
    }
    /// Whether `id` is among the objects a rule is about, as a `Query` reads.
    fn rule_subject(&self, affected: &ObjectSet, id: &str) -> Result<String> {
        Ok(match affected {
            ObjectSet::One(ObjectRef::Source) => format!("{id} == asker.source"),
            ObjectSet::One(ObjectRef::AttachedTo(host)) if matches!(**host, ObjectRef::Source) => {
                format!("q.state().obj(asker.source).attached_to == Some({id})")
            }
            ObjectSet::Battlefield(filter) => self.filter(filter, &Candidate::of(id))?,
            other => return Err(gap(format!("static rule subjects {}", form(other)))),
        })
    }
    /// A rule of the game a static ability changes, as one of `CardDef`'s tables.
    fn static_rule(
        &mut self,
        label: &str,
        affected: &ObjectSet,
        rule: &RuleChange,
        parts: &mut Parts,
    ) -> Result<bool> {
        let saved = (self.reader, self.event_at);
        (self.reader, self.event_at) = (Reader::Query, EventAt::Nowhere);
        let compiled = self.rule_table(label, affected, rule, parts);
        (self.reader, self.event_at) = saved;
        compiled
    }
    fn rule_table(
        &mut self,
        label: &str,
        affected: &ObjectSet,
        rule: &RuleChange,
        parts: &mut Parts,
    ) -> Result<bool> {
        const PRELUDE: &str = "let asker = &Asker::new(source, q.controller(source));";
        let guard = match &self.static_guard {
            Some(condition) => format!("({}) && ", self.condition(condition)?),
            None => String::new(),
        };
        let block = |c: &Self, blocker: bool, filter: &ObjectFilter| -> Result<String> {
            let (subject, other) = if blocker {
                ("blocker", "attacker")
            } else {
                ("attacker", "blocker")
            };
            Ok(format!(
                "{} && {}",
                c.rule_subject(affected, subject)?,
                c.filter(filter, &Candidate::of(other))?
            ))
        };
        const BLOCK: &str =
            "(q: &dyn Query, source: ObjectId, blocker: ObjectId, attacker: ObjectId) -> bool";
        let function = format!("{label}_rule");
        match rule {
            RuleChange::Prohibit(ObjectAction::Block(filter))
            | RuleChange::Prohibit(ObjectAction::BeBlocked(filter)) => {
                let blocker = matches!(rule, RuleChange::Prohibit(ObjectAction::Block(_)));
                let test = block(self, blocker, filter)?;
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}{BLOCK} {{ {PRELUDE} !({guard}{test}) }}").unwrap();
                parts.block_rules.push(function);
            }
            RuleChange::RequireCombat {
                action: ObjectAction::BeBlocked(filter),
                amount: RequirementAmount::Each,
            } => {
                let test = block(self, false, filter)?;
                writeln!(
                    self.functions,
                    "#[allow(unused_variables)] fn {function}{BLOCK} {{ {PRELUDE} {guard}{test} }}"
                )
                .unwrap();
                parts.block_requirements.push(function);
            }
            RuleChange::Prohibit(ObjectAction::Attack(AttackDefenders::Any)) => {
                let test = self.rule_subject(affected, "attacker")?;
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, source: ObjectId, attacker: ObjectId) -> bool {{ {PRELUDE} !({guard}{test}) }}").unwrap();
                parts.attack_rules.push(function);
            }
            RuleChange::UntapStep {
                behavior: UntapBehavior::Skip,
                ..
            } => {
                let test = self.rule_subject(affected, "id")?;
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, id: ObjectId) -> bool {{ {guard}{test} }}").unwrap();
                parts.untap_bans.push(function);
            }
            _ => return Ok(false),
        }
        Ok(true)
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
            Change::GrantKeyword(Keyword::Protection(quality)) => {
                let mut grants = Vec::new();
                granted_protection(quality, &mut grants)?;
                let body = grants.into_iter().map(|(_, statement)| statement).collect();
                LayerChange::of("Ability", body)
            }
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
            Change::AddCardType(t) => LayerChange::of(
                "Type",
                format!("chars.types = chars.types.with(CardType::{});", card_type(t)?),
            ),
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
            // Only a creature or a Kindred has creature types (205.3d).
            Change::CreatureTypes {
                mode: ChangeMode::Set,
                types: CreatureTypeSet::All,
            } => LayerChange::of(
                "Type",
                format!("if {HAS_CREATURE_TYPES} {{ chars.subtypes = chars.subtypes.union(SubtypeSet::CREATURE_TYPES); }}"),
            ),
            Change::CreatureTypes {
                mode: ChangeMode::Set,
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
                LayerChange::of(
                    "Type",
                    format!("chars.subtypes = chars.subtypes.minus(SubtypeSet::CREATURE_TYPES); if {HAS_CREATURE_TYPES} {{ {body} }}"),
                )
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
            Change::Rule(RuleChange::RequireCombat {
                action: ObjectAction::Attack(AttackDefenders::Any),
                amount: RequirementAmount::AtLeast(Expr::Constant(1)),
            }) => LayerChange::of("Ability", "chars.must_attack += 1;".into()),
            Change::GrantAbility(ability) => {
                let index = self.grant(ability)?;
                LayerChange::of(
                    "Ability",
                    format!("let card = {}; if let Some(slot) = chars.abilities.iter().position(|a| a.is_none()) {{ chars.abilities[slot] = Some(AbilityRef::delayed(card, {index})); }}", read::SOURCE_CARD),
                )
            }
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
        parts: &mut Parts,
    ) -> Result<()> {
        let affected = match affected {
            EffectSubjects::Objects(affected) => affected,
            EffectSubjects::Players(players) => {
                return self.player_static(label, players, changes, parts)
            }
        };
        // The keyword's own layer rule, not a copy of it: a copy doesn't take
        // the types (707.9d), and neither does what grants the keyword later.
        if let StaticKind::CharacteristicDefining {
            keyword: Some(printed @ Keyword::Changeling),
            ..
        } = kind
        {
            parts.keywords.push(keyword(printed)?);
            return Ok(());
        }
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
                .map_or_else(
                    || Ok::<_, Diagnostic>(("source == affected".to_string(), false)),
                    |f| Ok((f, true)),
                )?,
                ObjectSet::One(ObjectRef::AttachedTo(host))
                    if matches!(**host, ObjectRef::Source) =>
                {
                    match &condition {
                        None => (
                            "cantrip_mechanics::attach::attached_object".to_string(),
                            true,
                        ),
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
        let mut layered = Vec::new();
        for (i, change) in changes.iter().enumerate() {
            if let Change::Rule(rule) = change {
                if self.static_rule(&format!("{label}_{i}"), affected, rule, parts)? {
                    continue;
                }
            }
            layered.push((i, change, self.layered(|c| c.layer_change(change))?));
        }
        // Printed order isn't layer order. A later part of one effect follows
        // the part before it in a strictly later layer (613.6); parts in one
        // layer read the same subjects anyway.
        layered.sort_by_key(|(_, _, part)| layer_rank(part.layer));
        let mut defs = Vec::new();
        let mut last = None;
        for (i, change, part) in layered {
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
            let rank = layer_rank(part.layer);
            let follows = !defining && last.is_some_and(|before| before < rank);
            last = Some(rank);
            defs.push(if follows {
                format!("{def}.continues()")
            } else {
                def
            });
        }
        parts.statics.extend(defs);
        Ok(())
    }
}
