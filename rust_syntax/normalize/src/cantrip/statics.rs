//! Static abilities as layer callbacks. Selectors and values stay live: every
//! read here happens as the layers ask (CR 611.3), through a `LayerQuery`, and a
//! change never reads the `Chars` it writes.
use super::*;

const LAYER_PRELUDE: &str = "let asker = &Asker::new(source, q.controller(source));";

/// What one change does in one layer: `body` writes `chars`.
pub(super) struct LayerChange {
    layer: &'static str,
    body: String,
    /// `loses_all_abilities` and `control_changing` have no apply callback.
    whole: Option<&'static str>,
    /// The value reads the object being changed, so it needs `apply_each`.
    each: bool,
    /// Sets land types, which takes the land's rules text with it (CR 305.7).
    land: bool,
}
impl LayerChange {
    pub(super) fn of(layer: &'static str, body: String) -> Self {
        Self {
            layer,
            body,
            whole: None,
            each: false,
            land: false,
        }
    }
    /// A change that sets land types.
    pub(super) fn of_land(body: String) -> Self {
        Self {
            land: true,
            ..Self::of("Type", body)
        }
    }
}

/// The `Supertypes` field of a supertype the engine has.
fn supertype_field(t: &crate::mtg_types::SuperType) -> Result<&'static str> {
    use crate::mtg_types::SuperType as S;
    match t {
        S::Basic => Ok("basic"),
        S::Legendary => Ok("legendary"),
        S::Snow => Ok("snow"),
        other => Err(gap(format!("engine: no supertype {}", form(other)))),
    }
}

/// The order of the `Layer` a `LayerChange` names.
fn layer_rank(layer: &str) -> u8 {
    [
        "Control", "Text", "Type", "Color", "Ability", "PtDefine", "PtSet", "PtModify", "PtSwitch",
    ]
    .iter()
    .position(|l| *l == layer)
    .expect("a layer") as u8
}

const HAS_CREATURE_TYPES: &str = "(chars.is(CardType::Creature) || chars.is(CardType::Kindred))";

/// The `Keyword` an ability named to be lost is, when the engine models it as one.
pub(super) fn lost_keyword(lost: &Operand) -> Result<&'static str> {
    let Operand::Term { form: name, .. } = lost else {
        return Err(gap(format!("losing {}", form(lost))));
    };
    Ok(match name.as_str() {
        "Flying" => "Flying",
        "Defender" => "Defender",
        "Indestructible" => "Indestructible",
        "FirstStrike" => "FirstStrike",
        "DoubleStrike" => "DoubleStrike",
        "Trample" => "Trample",
        "Shroud" => "Shroud",
        "Deathtouch" => "Deathtouch",
        "Infect" => "Infect",
        "Shadow" => "Shadow",
        "Banding" => "Banding",
        other => return Err(gap(format!("losing ability {other}"))),
    })
}

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
        let label = format!("granted_{}", self.fresh());
        let grown = self.grown_parts(ability, &label)?;
        let [def] = grown.abilities.as_slice() else {
            return Err(lane_g::grant_gap(ability));
        };
        self.delayed.push(def.clone());
        Ok(self.delayed.len() - 1)
    }
    /// What an ability adds to a card, compiled on its own.
    pub(super) fn grown_parts(&mut self, ability: &Ability, label: &str) -> Result<Parts> {
        if self.granting && self.holding.is_none() {
            return Err(gap("granted ability that grants another"));
        }
        // The enclosing callback's reader belongs to it, not to this ability.
        let saved = (
            self.reader,
            self.event_at,
            self.frame,
            self.static_guard.take(),
        );
        let around_item = (self.stack_item, self.pending);
        (self.stack_item, self.pending) = (true, Pending::Nothing);
        let outer = self.outer.take();
        (self.reader, self.event_at, self.frame) =
            (Reader::Resolution, EventAt::Nowhere, Frame::default());
        let slots = std::mem::take(&mut self.slots);
        let results = self.results();
        self.granting = true;
        let mut grown = Parts::default();
        let compiled = self.ability(ability, label, &mut grown, None);
        self.granting = false;
        self.restore_results(results);
        self.slots = slots;
        (self.reader, self.event_at, self.frame, self.static_guard) = saved;
        (self.stack_item, self.pending) = around_item;
        self.outer = outer;
        compiled?;
        Ok(grown)
    }
    /// Whether `id` is among the objects a rule is about, as a `Query` reads.
    pub(super) fn rule_subject(&self, affected: &ObjectSet, id: &str) -> Result<String> {
        Ok(match affected {
            ObjectSet::One(ObjectRef::Source) => format!("{id} == asker.source"),
            ObjectSet::One(ObjectRef::AttachedTo(host)) if matches!(**host, ObjectRef::Source) => {
                format!("q.state().obj(asker.source).attached_to == Some({id})")
            }
            ObjectSet::Battlefield(filter) => self.filter(filter, &Candidate::of(id))?,
            other => match self.outside_subject(other, &Candidate::of(id))? {
                Some(test) => test,
                None => self
                    .stated_subject(other, &Candidate::of(id))
                    .map_err(|_| gap(format!("static rule subjects {}", form(other))))?,
            },
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
        if let Some(word) =
            lane_k::rule_keyword(affected, rule).filter(|_| self.static_guard.is_none())
        {
            parts.keywords.push(format!("Keyword::{word}"));
            return Ok(true);
        }
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
            // The rule is asked in any zone an ability functions from.
            RuleChange::Prohibit(ObjectAction::Activate(ActivationFilter::Any)) => {
                let test = self.rule_subject(affected, "object")?;
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, object: ObjectId, ability: &ActivatedDef) -> bool {{ !({guard}q.state().obj(object).zone.is(ZoneKind::Battlefield) && {test}) }}").unwrap();
                parts.table("activation_rules", function);
            }
            // "Can't attack unless defending player controls an Island": a rule on
            // the defending player. An attack rule is asked per attacked target, so
            // "can't attack you" leaves your planeswalkers open.
            RuleChange::Prohibit(ObjectAction::Attack(
                AttackDefenders::Players(players @ PlayerSet::Matching { .. })
                | AttackDefenders::PlayersAndPlaneswalkers(players @ PlayerSet::Matching { .. }),
            )) => {
                let test = self.rule_subject(affected, "attacker")?;
                let defenders = self.players(players)?;
                let walkers = matches!(
                    rule,
                    RuleChange::Prohibit(ObjectAction::Attack(
                        AttackDefenders::PlayersAndPlaneswalkers(_)
                    ))
                );
                let attacked = format!(
                    "(match defender {{ Target::Player(p) => ({defenders}).contains(&p), Target::Object(id) => {walkers} && q.chars(id).is(CardType::Planeswalker) && ({defenders}).contains(&q.controller(id)), _ => false }})"
                );
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, attacker: ObjectId, defender: Target) -> bool {{ let source = asker.source; {PRELUDE} !({guard}{test} && {attacked}) }}").unwrap();
                parts.attack_rules.push(function);
            }
            RuleChange::Prohibit(ObjectAction::Attack(AttackDefenders::Any)) => {
                let test = self.rule_subject(affected, "attacker")?;
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, attacker: ObjectId, defender: Target) -> bool {{ let source = asker.source; {PRELUDE} !({guard}{test}) }}").unwrap();
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
            other => return self.more_rules(&function, affected, other, parts),
        }
        Ok(true)
    }
    /// `change_in_layer`, where a change reading the object it applies to is asked per
    /// object, which `static_ability_for_each` gives it.
    fn layer_change(&mut self, change: &Change) -> Result<LayerChange> {
        if !read::reads_affected(change) {
            return self.change_in_layer(change);
        }
        self.affected_object = Some("affected");
        let part = self.change_in_layer(change);
        self.affected_object = None;
        Ok(LayerChange {
            each: true,
            ..part?
        })
    }
    /// Whether the layers carry this rule, as `change_in_layer` has it.
    pub(super) fn layered_rule(&mut self, rule: &RuleChange) -> bool {
        self.layered(|c| c.change_in_layer(&Change::Rule(rule.clone())))
            .is_ok()
    }
    fn change_in_layer(&mut self, change: &Change) -> Result<LayerChange> {
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
                    self.pump_amount(power, i16_of(self, power)?),
                    self.pump_amount(toughness, i16_of(self, toughness)?)
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
            Change::ChosenColor { .. } | Change::ChosenType { .. } => self.chosen_layer(change)?,
            Change::GrantKeyword(Keyword::Protection(quality)) => {
                let mut grants = Vec::new();
                self.grant_protection(quality, &mut grants)?;
                let body = grants.into_iter().map(|(_, statement)| statement).collect();
                LayerChange::of("Ability", body)
            }
            // Ward is a triggered ability, given as any other is.
            Change::GrantKeyword(
                ward @ (Keyword::Ward(_) | Keyword::Flanking | Keyword::Prowess),
            ) => {
                return self.layer_change(&Change::GrantAbility(Box::new(Ability::Keyword(ward.clone()))));
            }
            Change::GrantKeyword(Keyword::HexproofFrom(quality)) => LayerChange::of(
                "Ability",
                format!(
                    "chars.hexproof_from = chars.hexproof_from.and({});",
                    self.hexproof_of(quality)?
                ),
            ),
            Change::GrantKeyword(k) => LayerChange::of(
                "Ability",
                lane_k::keyword_granted(&keyword(k)?),
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
            Change::SetCardTypes(types) => {
                let types = types
                    .iter()
                    .map(|t| card_type(t).map(|t| format!("CardType::{t}")))
                    .collect::<Result<Vec<_>>>()?;
                LayerChange::of(
                    "Type",
                    format!(
                        "let kept = TypeSet::of(&[{}]); chars.types = kept; chars.subtypes = chars.subtypes.iter().filter(|s| s.card_types().0 & kept.0 != 0).fold(SubtypeSet::NONE, SubtypeSet::with);",
                        types.join(", ")
                    ),
                )
            }
            Change::AddSupertype(t) => LayerChange::of(
                "Type",
                format!("chars.supertypes.{} = true;", supertype_field(t)?),
            ),
            Change::RemoveSupertype(t) => LayerChange::of(
                "Type",
                format!("chars.supertypes.{} = false;", supertype_field(t)?),
            ),
            Change::AddLandType(t) => {
                let word = name(t);
                LayerChange::of(
                    "Type",
                    format!("chars.subtypes = chars.subtypes.with(q.words(asker.source).subtype(Subtype::{word}));"),
                )
            }
            Change::AddArtifactType(t) => {
                let word = name(t);
                LayerChange::of(
                    "Type",
                    format!("chars.subtypes = chars.subtypes.with(Subtype::{word});"),
                )
            }
            Change::SetArtifactType(t) => {
                let word = name(t);
                LayerChange::of(
                    "Type",
                    format!("chars.subtypes = chars.subtypes.iter().filter(|s| !s.card_types().has(CardType::Artifact)).fold(SubtypeSet::NONE, SubtypeSet::with).with(Subtype::{word});"),
                )
            }
            Change::SetLandType(land) => {
                let words = match &**land {
                    Operand::Vocabulary { form: word, .. } => vec![word.as_str()],
                    Operand::List(list) => copies::words(list)?,
                    other => return Err(gap(format!("land type {}", form(other)))),
                };
                let set = words.iter().fold("SubtypeSet::NONE".to_string(), |set, word| {
                    format!("{set}.with(q.words(asker.source).subtype(Subtype::{word}))")
                });
                LayerChange {
                    layer: "Type",
                    body: format!("chars.set_land_subtypes({set});"),
                    whole: None,
                    each: false,
                    land: true,
                }
            }
            Change::SwitchPowerToughness => LayerChange::of(
                "PtSwitch",
                "std::mem::swap(&mut chars.power, &mut chars.toughness);".into(),
            ),
            Change::LosesAbility(lost) => match lost_keyword(lost) {
                Ok(word) => LayerChange::of(
                    "Ability",
                    format!("chars.keywords = Keywords(chars.keywords.0 & !Keywords::of(&[Keyword::{word}]).0);"),
                ),
                Err(failed) => return self.lost_layer(lost, failed),
            },
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
                land: false,
            },
            Change::SetController(PlayerRef::AbilityController) => LayerChange {
                layer: "Control",
                body: String::new(),
                whole: Some("control_changing"),
                each: false,
                land: false,
            },
            Change::Rule(RuleChange::RequireCombat {
                action: ObjectAction::Attack(AttackDefenders::Any),
                amount: RequirementAmount::AtLeast(Expr::Constant(1)),
            }) => LayerChange::of("Ability", "chars.must_attack += 1;".into()),
            Change::GrantAbility(ability) if self.grants_keyword(ability).is_some() => {
                let keyword = self.grants_keyword(ability).expect("a keyword");
                LayerChange::of(
                    "Ability",
                    format!("chars.keywords = chars.keywords.with({keyword});"),
                )
            }
            Change::GrantAbility(ability) => {
                if let Some(keyword) = self.granted_keyword(ability) {
                    return Ok(keyword);
                }
                return self.grant_in_layer(ability);
            }
            Change::Rule(RuleChange::CombatGroup {
                role: CombatRole::BlockedAttacker,
                participants: ObjectFilter::Any,
                others_only: false,
                comparison: Comparison::LessOrEqual,
                amount: Expr::Constant(1),
            }) => LayerChange::of(
                "Ability",
                "chars.keywords = chars.keywords.with(Keyword::CantBeBlockedByMoreThanOne);".into(),
            ),
            Change::Rule(rule) => return self.layer_rule(rule),
            Change::GainAbilitiesOf { .. } => {
                return self
                    .gain_abilities_of(change)
                    .or_else(|failed| self.gained_layer(change, failed))
            }
            Change::SetName(word) => LayerChange::of(
                "Text",
                format!("chars.name = {}; chars.name_set = true;", quoted(word)),
            ),
            Change::RemoveCardType(t) => LayerChange::of(
                "Type",
                format!(
                    "chars.types = TypeSet(chars.types.0 & !(1 << CardType::{} as u8));",
                    card_type(t)?
                ),
            ),
            Change::PlayerRule(rule) => {
                return Err(gap(format!("static player rule {}", form(rule))))
            }
            other => return self.more_layers(other),
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
        let changes = &self.casting_tables(label, affected, changes, parts)?[..];
        let changes = &lane_o::in_one_effect(changes)[..];
        if changes.is_empty() || self.copy_static(label, affected, changes, parts)? {
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
                other => match c.outside_subject(other, &Candidate::affected())? {
                    Some(test) => (test, false),
                    None => (c.stated_subject(other, &Candidate::affected())?, false),
                },
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
            let text = format!("Static: {}.", form(change));
            let text = quoted(&self.said(lane_k::Said::Sentence, text));
            let def = match part.whole {
                Some(builder) => format!("{builder}({affects}, {text})"),
                None => {
                    let apply = format!("{label}_apply_{i}");
                    let each = if part.each {
                        "affected: ObjectId, "
                    } else {
                        ""
                    };
                    writeln!(self.functions, "#[allow(unused_variables)] fn {apply}(q: &dyn LayerQuery, source: ObjectId, {each}chars: &mut Chars) {{ {LAYER_PRELUDE} {} }}", part.body).unwrap();
                    if part.each && (part.land || defining) {
                        return Err(gap(
                            "a change by the object it applies to, defining or to a land",
                        ));
                    } else if part.each {
                        format!(
                            "static_ability_for_each(Layer::{}, {affects}, {apply}, {text})",
                            part.layer
                        )
                    } else if part.land {
                        format!("sets_land_subtypes({affects}, {apply}, {text})")
                    } else if defining {
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
            let rank = layer_rank(part.layer);
            let follows = !defining && last.is_some_and(|before| before < rank);
            last = Some(rank);
            defs.push(if follows {
                format!("{def}.continues()")
            } else {
                def
            });
        }
        if let ObjectSet::Zone { .. } | ObjectSet::AnyZone(_) = affected {
            let own = matches!(
                affected,
                ObjectSet::AnyZone(ObjectFilter::SameObject(ObjectRef::Source))
            );
            let own = if own { ".own()" } else { "" };
            for def in &mut defs {
                write!(def, "{own}.everywhere()").unwrap();
            }
        }
        parts.statics.extend(defs);
        Ok(())
    }
}
