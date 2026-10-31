//! Compile portable semantic data to named Rust callbacks. Capability failures
//! are separate from AST lowering failures; no source node is interpreted here.
use crate::{corpus, ir::*, validate};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
    io::{BufRead, Write as IoWrite},
    path::Path,
    rc::Rc,
};

mod casting;
mod costs;
mod delayed;
mod effects;
mod players;
mod read;
mod replacements;
mod statics;
mod survey;
mod triggers;
pub use crate::report::BackendDiagnostic as Diagnostic;
use effects::rule_name;
use read::{counter_kind, zone_kind, Candidate, Carried, EventAt, Frame, History, Pending, Reader};
pub use survey::survey;
type Result<T> = std::result::Result<T, Diagnostic>;
/// A reference to the result of an instruction that already reported a gap.
/// It says nothing new, so it is dropped rather than counted against the card.
const FOLLOWS_GAP: &str = "depends on an instruction with a gap";
fn gap(feature: impl Into<String>) -> Diagnostic {
    Diagnostic {
        path: String::new(),
        feature: feature.into(),
        message: "Cantrip code generation has not implemented this capability".into(),
    }
}
impl Diagnostic {
    fn at(mut self, path: impl AsRef<str>) -> Self {
        self.path = if self.path.is_empty() {
            path.as_ref().into()
        } else {
            format!("{}.{}", path.as_ref(), self.path)
        };
        self
    }
}
fn name(value: &impl std::fmt::Debug) -> String {
    format!("{value:?}")
}
/// The variant of an IR value, read from its derived `Debug`. With no list of
/// variants here, a form added to the end of an enum reports itself as a gap.
fn form(value: &impl std::fmt::Debug) -> String {
    let text = format!("{value:?}");
    let end = text
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(text.len());
    text[..end].into()
}
fn quoted(value: &str) -> String {
    format!("{value:?}")
}
/// `ZoneKind` for a `Scoped` zone list, the form both builders take.
fn zone_list(zones: &[EventZone]) -> Result<String> {
    // "Every zone" means the ones the engine looks in for an ability; on its
    // own, the command zone or the ante is somewhere it never looks.
    let every = |zone: &EventZone| {
        let has = |wanted: &EventZone| zones.iter().any(|z| z == wanted);
        matches!(zone, EventZone::Command | EventZone::Ante)
            && has(&EventZone::Battlefield)
            && has(&EventZone::Graveyard)
            && has(&EventZone::Hand)
            && has(&EventZone::Library)
            && has(&EventZone::Stack)
            && has(&EventZone::Exile)
    };
    if zones
        .iter()
        .any(|z| matches!(z, EventZone::Command) && !every(z))
    {
        return Err(gap("ability functioning in the command zone"));
    }
    let kinds = zones
        .iter()
        .filter(|zone| !every(zone))
        .map(read::zone_kind)
        .collect::<Result<Vec<_>>>()?;
    Ok(format!("ZoneKinds::of(&[{}])", kinds.join(", ")))
}
fn generator_fingerprint() -> &'static str {
    static HASH: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    HASH.get_or_init(|| {
        let inputs: BTreeMap<_, _> = [
            ("cantrip.rs", include_bytes!("cantrip.rs").as_slice()),
            (
                "cantrip/casting.rs",
                include_bytes!("cantrip/casting.rs").as_slice(),
            ),
            (
                "cantrip/costs.rs",
                include_bytes!("cantrip/costs.rs").as_slice(),
            ),
            (
                "cantrip/delayed.rs",
                include_bytes!("cantrip/delayed.rs").as_slice(),
            ),
            (
                "cantrip/effects.rs",
                include_bytes!("cantrip/effects.rs").as_slice(),
            ),
            (
                "cantrip/players.rs",
                include_bytes!("cantrip/players.rs").as_slice(),
            ),
            (
                "cantrip/read.rs",
                include_bytes!("cantrip/read.rs").as_slice(),
            ),
            (
                "cantrip/replacements.rs",
                include_bytes!("cantrip/replacements.rs").as_slice(),
            ),
            (
                "cantrip/statics.rs",
                include_bytes!("cantrip/statics.rs").as_slice(),
            ),
            (
                "cantrip/survey.rs",
                include_bytes!("cantrip/survey.rs").as_slice(),
            ),
            (
                "cantrip/triggers.rs",
                include_bytes!("cantrip/triggers.rs").as_slice(),
            ),
            ("semantics.rs", include_bytes!("semantics.rs").as_slice()),
            ("validate.rs", include_bytes!("validate.rs").as_slice()),
            ("ir.rs", include_bytes!("ir.rs").as_slice()),
            (
                "cantrip_vocab.rs",
                include_bytes!("cantrip_vocab.rs").as_slice(),
            ),
        ]
        .into_iter()
        .map(|(name, bytes)| (name, corpus::digest(bytes)))
        .collect();
        corpus::digest(&serde_json::to_vec(&inputs).expect("generator input hashes"))
    })
}
fn keyword(value: &Keyword) -> Result<String> {
    if let Keyword::Landwalk(land) = value {
        use crate::mtg_types::SubType;
        return Ok(format!(
            "Keyword::{}",
            match land.as_ref() {
                ObjectFilter::Subtype(SubType::Plains) => "Plainswalk",
                ObjectFilter::Subtype(SubType::Island) => "Islandwalk",
                ObjectFilter::Subtype(SubType::Swamp) => "Swampwalk",
                ObjectFilter::Subtype(SubType::Mountain) => "Mountainwalk",
                ObjectFilter::Subtype(SubType::Forest) => "Forestwalk",
                other => return Err(gap(format!("landwalk of {}", form(other)))),
            }
        ));
    }
    let n = name(value);
    if crate::cantrip_vocab::KEYWORDS.contains(&n.as_str()) {
        Ok(format!("Keyword::{n}"))
    } else {
        Err(gap(format!("keyword {}", form(value))))
    }
}
/// One protection quality an effect gives: the kind a resolving effect adds,
/// and the statement that gives the same in a layer.
fn granted_protection(quality: &Protection, grants: &mut Vec<(String, String)>) -> Result<()> {
    match quality {
        Protection::Objects {
            kind: ObjectKind::Any,
            filter: ObjectFilter::Color(c),
        } => {
            let c = color(c)?;
            grants.push((
                format!("EffectKind::Protection(ColorSet::NONE.with(Color::{c}))"),
                format!("chars.protection = chars.protection.with(q.words(asker.source).color(Color::{c}));"),
            ));
        }
        Protection::Objects {
            kind: ObjectKind::Any,
            filter: ObjectFilter::CardType(t),
        } => {
            let t = card_type(t)?;
            grants.push((
                format!("EffectKind::ProtectionFromTypes(TypeSet::of(&[CardType::{t}]))"),
                format!("chars.protection_types = chars.protection_types.with(CardType::{t});"),
            ));
        }
        Protection::Objects { filter, .. } => {
            return Err(gap(format!("protection from {}", form(filter))))
        }
        Protection::Each(qualities) => {
            for quality in qualities {
                granted_protection(quality, grants)?;
            }
        }
        other => return Err(gap(format!("protection {}", form(other)))),
    }
    Ok(())
}
fn color(value: &ManaColor) -> Result<&'static str> {
    Ok(match value {
        ManaColor::White => "White",
        ManaColor::Blue => "Blue",
        ManaColor::Black => "Black",
        ManaColor::Red => "Red",
        ManaColor::Green => "Green",
        ManaColor::Colorless => return Err(gap("colorless is not a color")),
        other => return Err(gap(format!("color {}", form(other)))),
    })
}
fn color_index(value: &ManaColor) -> Result<usize> {
    Ok(match value {
        ManaColor::White => 0,
        ManaColor::Blue => 1,
        ManaColor::Black => 2,
        ManaColor::Red => 3,
        ManaColor::Green => 4,
        ManaColor::Colorless => 5,
        other => return Err(gap(format!("color {}", form(other)))),
    })
}
fn color_mask(colors: &[ManaColor]) -> Result<u8> {
    colors
        .iter()
        .try_fold(0u8, |mask, color| Ok(mask | 1 << color_index(color)?))
}
fn mana(symbols: &[ManaSymbol]) -> Result<String> {
    let (mut generic, mut x) = (0u32, 0u32);
    let mut colors = [0u32; 6];
    let mut hybrid = Vec::new();
    let mut twobrid = [0u32; 5];
    let mut phyrexian = [0u32; 5];
    for symbol in symbols {
        match symbol {
            ManaSymbol::Generic(n) => {
                generic = generic
                    .checked_add(*n)
                    .ok_or_else(|| gap("generic mana overflow"))?
            }
            ManaSymbol::Color(c) => colors[color_index(c)?] += 1,
            ManaSymbol::X => x += 1,
            ManaSymbol::Hybrid(a, b) => hybrid.push(format!(
                "Some((Mana::{}, Mana::{}))",
                ["W", "U", "B", "R", "G", "C"][color_index(a)?],
                ["W", "U", "B", "R", "G", "C"][color_index(b)?]
            )),
            ManaSymbol::MonocoloredHybrid(c) => {
                color(c)?;
                twobrid[color_index(c)?] += 1;
            }
            ManaSymbol::Phyrexian(v) if v.len() == 1 => {
                color(&v[0])?;
                phyrexian[color_index(&v[0])?] += 1;
            }
            _ => return Err(gap("mana symbol")),
        }
    }
    if generic > 255
        || x > 255
        || colors
            .iter()
            .chain(twobrid.iter())
            .chain(phyrexian.iter())
            .any(|n| *n > 255)
        || hybrid.len() > 6
    {
        return Err(gap("mana cost exceeds Cantrip field bounds"));
    }
    hybrid.resize(6, "None".into());
    Ok(format!(
        "ManaCost {{ generic: {generic}, x: {x}, symbols: {colors:?}, hybrid: [{}], twobrid: {twobrid:?}, phyrexian: {phyrexian:?} }}",
        hybrid.join(",")
    ))
}
struct Number {
    code: String,
    min: i128,
    max: i128,
    /// Read from the game. A value that can pass a field's range only in a game
    /// no card reaches saturates there; a printed constant that passes is a gap.
    dynamic: bool,
}
impl Number {
    fn integer(n: i64) -> Self {
        Self {
            code: format!("{n}i64"),
            min: n.into(),
            max: n.into(),
            dynamic: false,
        }
    }
    fn fit(&self, min: i128, max: i128, kind: &str) -> Result<String> {
        if self.min >= min && self.max <= max {
            Ok(format!("({}) as {kind}", self.code))
        } else if self.dynamic {
            Ok(format!("({}).clamp({min}, {max}) as {kind}", self.code))
        } else {
            Err(gap(format!("numeric range for {kind}")))
        }
    }
    fn amount(&self, max: i128, kind: &str) -> Result<String> {
        if self.max <= max {
            Ok(format!("({}).max(0) as {kind}", self.code))
        } else if self.dynamic {
            Ok(format!("({}).clamp(0, {max}) as {kind}", self.code))
        } else {
            Err(gap(format!("numeric range for {kind}")))
        }
    }
}
#[derive(Clone)]
struct Compiler {
    /// Compiling the event of `Expr::EventHistory`.
    history: Option<History>,
    /// A `StackItem` named `item` is in reach, with its targets and X.
    stack_item: bool,
    pending: Pending,
    reader: Reader,
    event_at: EventAt,
    frame: Frame,
    /// A batch trigger's members are in reach: `Query::event_objects`.
    grouped: bool,
    candidates: std::cell::RefCell<Vec<Candidate>>,
    affected_in_scope: bool,
    /// The condition of the enclosing `Ability::Conditional`, asked as layers run.
    static_guard: Option<Condition>,
    slots: BTreeMap<u16, (usize, TargetSpec)>,
    selections: BTreeMap<u32, String>,
    creations: BTreeMap<u32, String>,
    moved: BTreeMap<u32, String>,
    /// The same transitions as `moved`, by the identities they left behind.
    before: BTreeMap<u32, String>,
    /// The block to leave if the payment about to be compiled is declined: a
    /// "you may" that is its first payment asks one question, not two.
    asking: Option<usize>,
    /// The cost of the activated ability being compiled.
    activation_cost: Option<Cost>,
    /// Each additional cost this card declares, by its ability's index.
    casting_costs: BTreeMap<u32, Cost>,
    /// Moves that already said when they end, in one call with the move.
    returning: BTreeSet<u32>,
    /// The `Divide` of the announcement just compiled, until a definition that
    /// can say so takes it. What's left of it at the ability's end is a gap.
    dividing: Option<String>,
    /// Locals an effect declares up front to outlive its nested blocks. `None`
    /// in code that isn't an effect's own body.
    hoisted: Option<Vec<String>>,
    /// Choices and searches that yield one object at most.
    single: BTreeSet<u32>,
    /// Some ability of the card reads what was exiled with its source.
    links_exile: bool,
    /// The announcement slot whose filter is being compiled.
    choosing: Option<usize>,
    performed: BTreeMap<u32, String>,
    values: BTreeMap<u32, String>,
    flips: BTreeMap<u32, String>,
    bound_objects: BTreeSet<u32>,
    bound_players: BTreeSet<u32>,
    /// Small generated functions that this module uses.
    helpers: BTreeSet<&'static str>,
    extras: Vec<String>,
    /// Abilities that are no part of the card until a grant or an event names
    /// them: `CardDef::delayed`.
    delayed: Vec<String>,
    /// The filter of each slot the last announcement compiled.
    announced_filters: Vec<String>,
    /// Compiling a delayed ability's body: what it carries from its maker.
    outer: Option<Rc<delayed::Outer>>,
    /// `CardDef::granted_preventions` and `granted_replacements`.
    made_preventions: Vec<String>,
    made_replacements: Vec<String>,
    granting: bool,
    functions: String,
    next: usize,
    /// Every gap so far. Compilation continues past one so a survey sees them
    /// all, but `module` publishes nothing unless this ends up empty.
    gaps: Vec<Diagnostic>,
    /// Results of instructions that have a gap.
    failed: BTreeSet<u32>,
}

enum ManaPlan {
    Fixed(Vec<(ManaColor, Expr)>),
    SameColor {
        colors: Vec<ManaColor>,
        amount: Expr,
    },
    Combination {
        colors: Vec<ManaColor>,
        amount: Expr,
    },
}
fn mana_plan(value: &ManaProduction) -> Result<ManaPlan> {
    Ok(match value {
        ManaProduction::Primitive { name, .. } => {
            return Err(gap(format!("primitive mana production {name}")))
        }
        ManaProduction::Amount { color, amount } => {
            ManaPlan::Fixed(vec![(color.clone(), amount.clone())])
        }
        ManaProduction::All(parts) => {
            let mut units: BTreeMap<usize, (ManaColor, Expr)> = BTreeMap::new();
            for part in parts {
                let ManaPlan::Fixed(part) = mana_plan(part)? else {
                    return Err(gap("combined mana choices in one production"));
                };
                for (color, amount) in part {
                    let index = color_index(&color)?;
                    if let Some((_, previous)) = units.get_mut(&index) {
                        *previous = Expr::add(previous.clone(), amount);
                    } else {
                        units.insert(index, (color, amount));
                    }
                }
            }
            ManaPlan::Fixed(units.into_values().collect())
        }
        ManaProduction::ChooseOne(options) => {
            let mut colors = Vec::new();
            let mut amount = None;
            for option in options {
                let ManaPlan::Fixed(units) = mana_plan(option)? else {
                    return Err(gap("nested mana bundle choice"));
                };
                let [(color, Expr::Constant(n))] = units.as_slice() else {
                    return Err(gap("mana choice among different bundles"));
                };
                if amount.is_some_and(|m| m != *n) {
                    return Err(gap("mana choice with differing amounts"));
                }
                if matches!(color, ManaColor::Colorless) {
                    return Err(gap("mana type choice including colorless"));
                }
                amount = Some(*n);
                if !colors.contains(color) {
                    colors.push(color.clone());
                }
            }
            ManaPlan::SameColor {
                colors,
                amount: Expr::Constant(amount.ok_or_else(|| gap("empty mana choice"))?),
            }
        }
        ManaProduction::Repeat {
            amount,
            production,
            choice,
        } => match mana_plan(production)? {
            ManaPlan::Fixed(units) => ManaPlan::Fixed(
                units
                    .into_iter()
                    .map(|(c, n)| (c, Expr::multiply(amount.clone(), n)))
                    .collect(),
            ),
            ManaPlan::SameColor {
                colors,
                amount: inner,
            } if matches!(choice, ManaChoiceTiming::Once) => ManaPlan::SameColor {
                colors,
                amount: Expr::multiply(amount.clone(), inner),
            },
            ManaPlan::SameColor {
                colors,
                amount: Expr::Constant(1),
            } => ManaPlan::Combination {
                colors,
                amount: amount.clone(),
            },
            _ => return Err(gap("repeated compound mana choice")),
        },
    })
}
/// "Subtype spells", "or abilities of subtype sources" and "... of subtypes".
fn subtype_restriction(value: &ManaUse) -> Option<String> {
    let of = |s: &crate::mtg_types::SubType| -> Option<String> {
        let word = name(s);
        crate::cantrip_vocab::SUBTYPES
            .contains(&word.as_str())
            .then(|| format!("SubtypeSpells::of(Subtype::{word})"))
    };
    let code = match value {
        ManaUse::CastSpell(ObjectFilter::Subtype(s)) => of(s)?,
        ManaUse::Either(uses) => {
            let [ManaUse::CastSpell(ObjectFilter::Subtype(spell)), ManaUse::ActivateAbility {
                sources: ObjectFilter::Subtype(source),
                permanents_only,
            }] = uses.as_slice()
            else {
                return None;
            };
            if name(spell) != name(source) {
                return None;
            }
            let reach = match permanents_only {
                true => "or_abilities_of_permanents",
                false => "or_abilities",
            };
            format!("{}.{reach}()", of(spell)?)
        }
        _ => return None,
    };
    Some(format!("ManaRestriction::Subtypes({code})"))
}
fn mana_restriction(value: &ManaUse) -> Result<String> {
    use crate::mtg_types::CardType as T;
    if let Some(restriction) = subtype_restriction(value) {
        return Ok(restriction);
    }
    if let ManaUse::Either(uses) = value {
        if let [ManaUse::CastSpell(ObjectFilter::CardType(spell)), ManaUse::ActivateAbility {
            sources,
            permanents_only,
        }] = uses.as_slice()
        {
            let constant = match (spell, sources, permanents_only) {
                (T::Creature, ObjectFilter::CardType(T::Creature), true) => {
                    Some("CREATURE_SPELLS_OR_ABILITIES_OF_CREATURES")
                }
                (T::Creature, ObjectFilter::CardType(T::Creature), false) => {
                    Some("CREATURE_SPELLS_OR_ABILITIES")
                }
                (T::Artifact, ObjectFilter::CardType(T::Artifact), true) => {
                    Some("ARTIFACT_SPELLS_OR_ABILITIES_OF_ARTIFACTS")
                }
                _ => None,
            };
            if let Some(constant) = constant {
                return Ok(format!("ManaRestriction::{constant}"));
            }
        }
    }
    let constant = match value {
        ManaUse::CastSpell(ObjectFilter::CardType(T::Artifact)) => "ARTIFACT_SPELLS",
        ManaUse::CastSpell(ObjectFilter::CardType(T::Creature)) => "CREATURE_SPELLS",
        ManaUse::CastSpell(ObjectFilter::CardType(T::Planeswalker)) => "PLANESWALKER_SPELLS",
        ManaUse::CastSpell(ObjectFilter::Not(v))
            if matches!(v.as_ref(), ObjectFilter::CardType(T::Creature)) =>
        {
            "NONCREATURE_SPELLS"
        }
        ManaUse::CastSpell(ObjectFilter::Either(v))
            if v.len() == 2
                && v.iter()
                    .any(|f| matches!(f, ObjectFilter::CardType(T::Instant)))
                && v.iter()
                    .any(|f| matches!(f, ObjectFilter::CardType(T::Sorcery))) =>
        {
            "INSTANT_OR_SORCERY_SPELLS"
        }
        ManaUse::Not(v) if matches!(v.as_ref(),ManaUse::CastSpell(ObjectFilter::Not(f)) if matches!(f.as_ref(),ObjectFilter::CardType(T::Artifact))) => {
            return Ok("ManaRestriction::NotNonartifactSpells".into())
        }
        ManaUse::ActivateAbility {
            sources: ObjectFilter::Any,
            permanents_only: false,
        } => "ABILITIES",
        ManaUse::ActivateAbility {
            sources: ObjectFilter::CardType(T::Land),
            permanents_only: false,
        } => "LAND_ABILITIES",
        ManaUse::ActivateAbility {
            sources: ObjectFilter::CardType(T::Artifact),
            permanents_only: false,
        } => "ARTIFACT_ABILITIES",
        ManaUse::ActivateAbility {
            sources: ObjectFilter::CardType(T::Artifact),
            permanents_only: true,
        } => "ABILITIES_OF_ARTIFACTS",
        ManaUse::ActivateAbility {
            sources: ObjectFilter::CardType(T::Creature),
            permanents_only: false,
        } => "CREATURE_ABILITIES",
        _ => return Err(gap("mana spending predicate")),
    };
    Ok(format!("ManaRestriction::{constant}"))
}
impl Compiler {
    fn new() -> Self {
        Self {
            history: None,
            stack_item: true,
            pending: Pending::Nothing,
            reader: Reader::Resolution,
            event_at: EventAt::Nowhere,
            frame: Frame::default(),
            grouped: false,
            candidates: Default::default(),
            affected_in_scope: false,
            static_guard: None,
            slots: BTreeMap::new(),
            selections: BTreeMap::new(),
            creations: BTreeMap::new(),
            moved: BTreeMap::new(),
            before: BTreeMap::new(),
            asking: None,
            activation_cost: None,
            casting_costs: BTreeMap::new(),
            returning: BTreeSet::new(),
            dividing: None,
            hoisted: None,
            single: BTreeSet::new(),
            links_exile: false,
            choosing: None,
            performed: BTreeMap::new(),
            values: BTreeMap::new(),
            flips: BTreeMap::new(),
            bound_objects: BTreeSet::new(),
            bound_players: BTreeSet::new(),
            helpers: BTreeSet::new(),
            extras: Vec::new(),
            delayed: Vec::new(),
            announced_filters: Vec::new(),
            outer: None,
            made_preventions: Vec::new(),
            made_replacements: Vec::new(),
            granting: false,
            functions: String::new(),
            next: 0,
            gaps: Vec::new(),
            failed: BTreeSet::new(),
        }
    }
    /// Compile one part, recording a gap rather than stopping at it, and prefix
    /// `path` to everything it reported.
    fn attempt(&mut self, path: String, part: impl FnOnce(&mut Self) -> Result<()>) {
        let mark = self.gaps.len();
        if let Err(gap) = part(self) {
            if gap.feature != FOLLOWS_GAP {
                self.gaps.push(gap);
            }
        }
        for gap in &mut self.gaps[mark..] {
            *gap = gap.clone().at(&path);
        }
    }
    fn selected(&self, result: &ResultId) -> Result<String> {
        match self.selections.get(&result.0) {
            Some(local) => Ok(format!("{local}.clone()")),
            None if self.failed.contains(&result.0) => Err(gap(FOLLOWS_GAP)),
            None => Err(gap("selection result outside generated scope")),
        }
    }
    fn created(&self, result: &ResultId) -> Result<String> {
        match self.creations.get(&result.0) {
            Some(local) => Ok(format!("{local}.clone()")),
            None if self.failed.contains(&result.0) => Err(gap(FOLLOWS_GAP)),
            None => Err(gap("token result outside generated scope")),
        }
    }
    fn moved(&self, result: &ResultId) -> Result<String> {
        match self.moved.get(&result.0) {
            Some(local) => Ok(format!("{local}.clone()")),
            None if self.failed.contains(&result.0) => Err(gap(FOLLOWS_GAP)),
            None => Err(gap("transition this instruction does not report")),
        }
    }
    /// What a transition moved, by the new objects or the ones they were.
    fn transition(&self, result: &ResultId, phase: &ObjectPhase) -> Result<String> {
        match (phase, self.before.get(&result.0)) {
            (ObjectPhase::Before, Some(list)) => Ok(list.clone()),
            (ObjectPhase::Before, None) if self.moved.contains_key(&result.0) => {
                Err(gap("object as it was before an instruction moved it"))
            }
            _ => self.moved(result),
        }
    }
    fn target_domain(domain: &ObjectDomain) -> Result<String> {
        let zones = |zones: &[EventZone]| -> Result<String> {
            let tests = zones
                .iter()
                .map(|z| {
                    read::zone_kind(z).map(|z| format!("q.state().obj(id).zone.kind() == {z}"))
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(format!("({})", tests.join(" || ")))
        };
        Ok(match domain.kind {
            ObjectKind::Permanent if domain.zones == [EventZone::Battlefield] => {
                "cantrip_mechanics::targets::permanent(q, id)".into()
            }
            ObjectKind::Card => format!("{} && !q.state().obj(id).token", zones(&domain.zones)?),
            ObjectKind::Spell if domain.zones == [EventZone::Stack] => zones(&domain.zones)?,
            _ => return Err(gap(format!("target object domain {:?}", domain.kind))),
        })
    }
    /// The body of a `TargetFilter`, or a shared target for an unrestricted one.
    fn target_test(&self, spec: &TargetSpec) -> Result<String> {
        let id = Candidate::of("id");
        Ok(match &spec.kind {
            TargetKind::Object(domain) => format!(
                "matches!(t, Target::Object(id) if {} && ({}))",
                Self::target_domain(domain)?,
                self.filter(&spec.filter, &id)?
            ),
            TargetKind::Player => match &spec.players {
                Some(players) => format!(
                    "matches!(t, Target::Player(p) if {})",
                    self.player_in(players, "p")?
                ),
                None => "matches!(t, Target::Player(_))".into(),
            },
            TargetKind::DamageRecipient => {
                let players = match &spec.players {
                    Some(players) => self.player_in(players, "p")?,
                    None => "false".into(),
                };
                format!(
                    "match t {{ Target::Player(p) => {players}, Target::Object(id) => q.is_on_battlefield(id) && {{ let c = q.chars(id); c.is(CardType::Creature) || c.is(CardType::Planeswalker) || c.is(CardType::Battle) }} && ({}), _ => false }}",
                    self.filter(&spec.filter, &id)?
                )
            }
        })
    }
    fn target_count(&self, count: &Count) -> Result<String> {
        let constant = |n: &Expr| match n {
            Expr::Constant(n) => u32::try_from(*n).ok(),
            _ => None,
        };
        Ok(match count {
            Count::Exactly(n) if constant(n) == Some(1) => String::new(),
            Count::Exactly(Expr::AnnouncedX) => ".counted()".into(),
            Count::Exactly(n) if constant(n).is_some() => {
                let n = constant(n).unwrap();
                format!(".between({n}, {n})")
            }
            Count::UpTo(n) if constant(n).is_some() => {
                format!(".between(0, {})", constant(n).unwrap())
            }
            Count::AnyNumber => ".between(0, cantrip_mechanics::ANY_NUMBER)".into(),
            Count::Between { minimum, maximum }
                if constant(minimum).is_some() && constant(maximum).is_some() =>
            {
                format!(
                    ".between({}, {})",
                    constant(minimum).unwrap(),
                    constant(maximum).unwrap()
                )
            }
            other => return Err(gap(format!("target cardinality {}", form(other)))),
        })
    }
    fn announcement(&mut self, a: &Announcement, label: &str) -> Result<String> {
        if a.modes.is_some() {
            return Err(gap("modal announcement"));
        }
        self.slots.clear();
        let reader = std::mem::replace(&mut self.reader, Reader::Query);
        let compiled = self.announced_targets(a, label);
        self.reader = reader;
        compiled
    }
    fn announced_targets(&mut self, a: &Announcement, label: &str) -> Result<String> {
        let mut targets = Vec::new();
        self.announced_filters.clear();
        for (i, spec) in a.targets.iter().enumerate() {
            let at = format!("targets[{i}]");
            if let Some(constraint) = spec.constraints.first() {
                return Err(gap(format!("target constraint {}", form(constraint))).at(at));
            }
            if let Some(total) = &spec.distribute {
                let total = match total {
                    Expr::Constant(n) if i == 0 && *n >= 0 => n.to_string(),
                    Expr::AnnouncedX if i == 0 => "TargetSpec::COUNTED".to_string(),
                    _ => return Err(gap("target with a divided amount").at(at)),
                };
                self.dividing = Some(format!("Divide {{ total: {total}, of_x: None }}"));
            }
            if spec.chooser.is_some() {
                return Err(gap("targets chosen by another player").at(at));
            }
            let count = self.target_count(&spec.count).map_err(|e| e.at(&at))?;
            self.slots.insert(spec.slot.0, (i, spec.clone()));
            if spec.kind == TargetKind::DamageRecipient
                && spec.players.is_none()
                && matches!(spec.filter, ObjectFilter::Any)
            {
                targets.push(format!("cantrip_mechanics::targets::ANY{count}"));
                self.announced_filters
                    .push("cantrip_mechanics::targets::ANY.filter".into());
                continue;
            }
            self.choosing = Some(i);
            let test = self.target_test(spec).map_err(|e| e.at(&at));
            self.choosing = None;
            let test = test?;
            let function = format!("{label}_target_{i}");
            writeln!(
                self.functions,
                "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, t: Target) -> bool {{ {test} }}"
            )
            .unwrap();
            targets.push(format!("target({function}){count}"));
            self.announced_filters.push(function);
        }
        if targets.is_empty() {
            return Ok("&[]".into());
        }
        // A named static also works when this definition is nested in a token
        // module and ability builders retain the slice for the whole game.
        let array = format!("{}_TARGETS", label.to_ascii_uppercase());
        writeln!(
            self.functions,
            "static {array}: &[cantrip_api::TargetSpec] = &[{}];",
            targets.join(",")
        )
        .unwrap();
        Ok(array)
    }
    fn token_definition(&mut self, token: &TokenDefinition) -> Result<String> {
        let pt = token
            .power_toughness
            .as_ref()
            .map(|(p, t)| {
                let integer = |n: &Expr| -> Result<i32> {
                    match n {
                        Expr::Constant(n) => {
                            i32::try_from(*n).map_err(|_| gap("token characteristic exceeds i32"))
                        }
                        _ => Err(gap("dynamic defining token characteristics")),
                    }
                };
                Ok((integer(p)?, integer(t)?))
            })
            .transpose()?;
        let card = Card {
            schema_version: SCHEMA_VERSION,
            name: match &token.name {
                TokenName::Explicit(name) => name.clone(),
                TokenName::FromSubtypes => "Token".into(),
            },
            characteristics: Characteristics {
                supertypes: token.supertypes.clone(),
                card_types: token.card_types.clone(),
                subtypes: token.subtypes.clone(),
                mana_cost: None,
                color_indicator: token.colors.clone(),
                power_toughness: pt,
                loyalty: None,
                defense: None,
            },
            abilities: token.abilities.clone(),
        };
        let id = self.next;
        self.next += 1;
        let child = emit_module(&card, Some(&token.name))?;
        self.gaps
            .extend(child.gaps.iter().map(|g| g.clone().at("token")));
        writeln!(self.functions, "pub mod token_{id} {{\n{}\n}}", child.rust).unwrap();
        let path = format!("token_{id}::CARD");
        self.extras.push(path.clone());
        self.extras
            .extend(child.extras.into_iter().map(|p| format!("token_{id}::{p}")));
        Ok(path)
    }
    fn emit_mana(
        &mut self,
        plan: &ManaPlan,
        players: &PlayerSet,
        restriction: Option<&ManaUse>,
        chosen: Option<&str>,
        out: &mut String,
    ) -> Result<()> {
        let id = self.next;
        self.next += 1;
        let restriction = restriction.map(mana_restriction).transpose()?;
        let add = |player: &str, color: &str, amount: &str| -> String {
            match &restriction {
                Some(r) => format!("g.add_restricted_mana({player}, {color}, {amount}, {r});"),
                None => format!("g.add_mana({player}, {color}, {amount});"),
            }
        };
        writeln!(
            out,
            "let mana_players_{id} = {};",
            self.fetch(self.players(players)?)
        )
        .unwrap();
        match plan {
            ManaPlan::Fixed(units) => {
                for (i, (_, amount)) in units.iter().enumerate() {
                    writeln!(
                        out,
                        "let mana_{id}_{i} = {};",
                        self.fetch(self.number(amount)?.amount(u32::MAX.into(), "u32")?)
                    )
                    .unwrap();
                }
                writeln!(out, "for player in mana_players_{id} {{").unwrap();
                for (i, (color, _)) in units.iter().enumerate() {
                    out.push_str(&add(
                        "player",
                        &format!(
                            "Mana::{}",
                            ["W", "U", "B", "R", "G", "C"][color_index(color)?]
                        ),
                        &format!("mana_{id}_{i}"),
                    ));
                }
                out.push_str("}\n");
            }
            ManaPlan::SameColor { colors, amount } | ManaPlan::Combination { colors, amount } => {
                let mask = color_mask(colors)?;
                let combination = matches!(plan, ManaPlan::Combination { .. });
                let number = self.number(amount)?;
                writeln!(
                    out,
                    "let mana_{id} = {};",
                    self.fetch(number.amount(
                        if combination {
                            u8::MAX.into()
                        } else {
                            u32::MAX.into()
                        },
                        if combination { "u8" } else { "u32" }
                    )?)
                )
                .unwrap();
                writeln!(out, "for player in mana_players_{id} {{").unwrap();
                if let Some(color) = chosen {
                    if combination {
                        return Err(gap("preselected color for a combination"));
                    }
                    writeln!(
                        out,
                        "let color = {color}.expect(\"mana activation selected a color\");"
                    )
                    .unwrap();
                    out.push_str(&add("player", "color.mana()", &format!("mana_{id}")));
                } else {
                    writeln!(out,"if mana_{id} > 0 {{ let colors = cantrip_mechanics::choose_colors(g, player, asker.source, ColorSet({mask}), {})?;",if combination{format!("mana_{id}")}else{"1".into()}).unwrap();
                    if combination {
                        writeln!(
                            out,
                            "for color in colors {{ {} }}",
                            add("player", "color.mana()", "1")
                        )
                        .unwrap();
                    } else {
                        out.push_str(&add("player", "colors[0].mana()", &format!("mana_{id}")));
                    }
                    out.push_str("}\n");
                }
                out.push_str("}\n");
            }
        }
        Ok(())
    }
    fn mana_ability(&mut self, body: &Program, label: &str) -> Result<String> {
        let [Instruction::Effect {
            operation:
                Operation::AddMana {
                    players: PlayerSet::One(PlayerRef::AbilityController),
                    mana,
                    restriction,
                },
            ..
        }, rest @ ..] = body.instructions.as_slice()
        else {
            return Err(gap("compound mana ability callback"));
        };
        let plan = mana_plan(mana)?;
        let mut metadata = format!("add_by({label})");
        // Listed yields help the engine advise payments; callbacks do the actual
        // production and also preserve mana-ability status when the amount is 0.
        match &plan {
            ManaPlan::Fixed(units) => {
                let mut pool = [0u32; 6];
                let mut known = true;
                for (color, n) in units {
                    if let Expr::Constant(n) = n {
                        pool[color_index(color)?] =
                            u32::try_from(*n).map_err(|_| gap("fixed mana amount exceeds u32"))?;
                    } else {
                        known = false;
                    }
                }
                if known {
                    metadata = format!("Adds {{ pool: ManaPool({pool:?}), ..{metadata} }}");
                }
            }
            ManaPlan::SameColor { colors, amount } => {
                let mask = color_mask(colors)?;
                metadata.push_str(&format!(".choose_color().colors(ColorSet({mask}))"));
                if let Expr::Constant(n) = amount {
                    if (0..=255).contains(n) {
                        metadata.push_str(&format!(".yields_any({n})"));
                    }
                }
            }
            ManaPlan::Combination { colors, amount } => {
                let mask = color_mask(colors)?;
                metadata.push_str(&format!(".colors(ColorSet({mask}))"));
                if let Expr::Constant(n) = amount {
                    if (0..=255).contains(n) {
                        metadata.push_str(&format!(".yields_any({n})"));
                    }
                }
            }
        }
        if let Some(r) = restriction {
            metadata.push_str(&format!(".only_for({})", mana_restriction(r)?));
        }
        let mut out = String::new();
        self.stack_item = false;
        let reader = std::mem::replace(&mut self.reader, Reader::Resolution);
        let emitted = self.emit_mana(
            &plan,
            &PlayerSet::One(PlayerRef::AbilityController),
            restriction.as_ref(),
            if matches!(plan, ManaPlan::SameColor { .. }) {
                Some("chosen")
            } else {
                None
            },
            &mut out,
        );
        // What a mana ability does besides adds is part of it: painlands' damage.
        self.reset_results();
        let rest = emitted.and_then(|()| {
            self.program(
                &Program {
                    instructions: rest.to_vec(),
                },
                &mut out,
                false,
            )
        });
        self.reader = reader;
        self.stack_item = true;
        rest?;
        writeln!(self.functions,"fn {label}(g: &mut dyn Game, asker: &Asker, chosen: Option<Color>) -> Result<(),Suspend> {{ {out} Ok(()) }}").unwrap();
        Ok(metadata)
    }
}
fn card_type(t: &crate::mtg_types::CardType) -> Result<String> {
    let n = name(t);
    if [
        "Artifact",
        "Battle",
        "Creature",
        "Enchantment",
        "Instant",
        "Kindred",
        "Land",
        "Planeswalker",
        "Sorcery",
    ]
    .contains(&n.as_str())
    {
        Ok(n)
    } else {
        Err(gap(format!("card type {n}")))
    }
}
fn ability_text(body: &Program) -> String {
    fn text(instruction: &Instruction) -> String {
        match instruction {
            Instruction::Effect { operation, .. } => match operation {
                Operation::AddMana { .. } => "Add mana.".into(),
                Operation::CreateTokens { .. } => "Create tokens.".into(),
                Operation::Draw {
                    amount: Expr::Constant(n),
                    ..
                } => format!("Draw {n} cards."),
                Operation::GainLife {
                    amount: Expr::Constant(n),
                    ..
                } => format!("Gain {n} life."),
                Operation::LoseLife {
                    amount: Expr::Constant(n),
                    ..
                } => format!("Lose {n} life."),
                Operation::Mill {
                    amount: Expr::Constant(n),
                    ..
                } => format!("Mill {n} cards."),
                Operation::CreateContinuousEffect { changes, .. } => match changes.as_slice() {
                    [Change::Pump {
                        power: Expr::Constant(p),
                        toughness: Expr::Constant(t),
                    }] => format!("Gets {p:+}/{t:+} until end of turn."),
                    _ => "Change characteristics until end of turn.".into(),
                },
                Operation::Tap { tapped: true, .. } => "Tap the chosen permanents.".into(),
                Operation::Tap { tapped: false, .. } => "Untap the chosen permanents.".into(),
                Operation::Destroy { .. } => "Destroy the chosen permanents.".into(),
                Operation::Move { .. } => {
                    "Return the chosen permanents to their owners' hands.".into()
                }
                Operation::GainLife { .. } => "Gain life.".into(),
                Operation::LoseLife { .. } => "Lose life.".into(),
                Operation::Draw { .. } => "Draw cards.".into(),
                Operation::Mill { .. } => "Mill cards.".into(),
                _ => "".into(),
            },
            Instruction::Group(group) => ability_text(group),
            _ => "".into(),
        }
    }
    body.instructions
        .iter()
        .map(text)
        .collect::<Vec<_>>()
        .join(" ")
}

/// One complete card module. Never returns partially executable code.
pub fn module(card: &Card) -> Result<String> {
    let mut module = emit_module(card, None)?;
    if module.gaps.is_empty() {
        Ok(module.rust)
    } else {
        Err(module.gaps.swap_remove(0))
    }
}
/// Everything the generator lacks for this card, not only the first gap.
pub fn gaps(card: &Card) -> Vec<Diagnostic> {
    match emit_module(card, None) {
        Ok(module) => module.gaps,
        Err(gap) => vec![gap],
    }
}
/// Every target of one slot, in the order chosen: all of them, for reading what
/// was announced, or the ones still legal, for acting on them (CR 608.2b).
const ANNOUNCED: &str = "fn announced(g: &dyn Game, item: &StackItem, spec: u8, legal: bool) -> Vec<Target> { (0..g.state().targets(item).len()).filter(|&i| g.state().spec(item, i) == spec && (!legal || g.target_legal(item, i))).map(|i| g.state().target(item, i)).collect() }\n";

/// "You may pay": the engine shows the cost, and bots read the verb.
const PAY_QUESTION: &str = "static PAY_QUESTION: Question = Question::new(\"Pay\", \"pay {cost}?\", \"\", \"Pay {cost}\", \"Decline\");\n";

/// What a card's abilities contribute to its `CardDef`.
struct Parts {
    keywords: Vec<String>,
    abilities: Vec<String>,
    statics: Vec<String>,
    replacements: Vec<String>,
    preventions: Vec<String>,
    enters: Vec<String>,
    block_rules: Vec<String>,
    block_requirements: Vec<String>,
    attack_rules: Vec<String>,
    untap_bans: Vec<String>,
    /// `CardDef`'s other rule tables, by field.
    tables: BTreeMap<&'static str, Vec<String>>,
    /// `CardDef`'s other fields.
    fields: BTreeMap<&'static str, String>,
    spell: String,
    modal: String,
    enchant: String,
    protection: u8,
}
impl Parts {
    fn table(&mut self, table: &'static str, entry: String) {
        self.tables.entry(table).or_default().push(entry);
    }
    fn field(&mut self, field: &'static str, value: String) {
        self.fields.insert(field, value);
    }
    /// The struct-literal fields for `tables` and `fields`.
    fn rest(&self) -> String {
        let mut rest = String::new();
        for (table, entries) in &self.tables {
            write!(rest, "{table}: &[{}], ", entries.join(",")).unwrap();
        }
        for (field, value) in &self.fields {
            write!(rest, "{field}: {value}, ").unwrap();
        }
        rest
    }
}
impl Default for Parts {
    fn default() -> Self {
        Self {
            keywords: Vec::new(),
            abilities: Vec::new(),
            statics: Vec::new(),
            replacements: Vec::new(),
            preventions: Vec::new(),
            enters: Vec::new(),
            block_rules: Vec::new(),
            block_requirements: Vec::new(),
            attack_rules: Vec::new(),
            untap_bans: Vec::new(),
            tables: BTreeMap::new(),
            fields: BTreeMap::new(),
            spell: "None".into(),
            modal: "None".into(),
            enchant: "None".into(),
            protection: 0,
        }
    }
}

struct EmittedModule {
    rust: String,
    extras: Vec<String>,
    gaps: Vec<Diagnostic>,
}
/// How an object was cast: it as a spell, or the spell it resolved from. A
/// spell's own `cast_from` isn't carried onto the permanent. A matcher sees the
/// history before its own event, so `cast_at` reads that entry from the event.
const CAST_OF: &str = "fn cast_of<'a>(q: &'a dyn Query, id: ObjectId) -> Option<&'a Cast> { let records = q.history().all(); let spell = records.iter().rev().find_map(|r| match r.event { Event::ZoneChange { old, new, reason: ZoneReason::Resolve, .. } if new == id => Some(old), _ => None }).unwrap_or(id); records.iter().rev().find_map(|r| match &r.event { Event::Cast(cast) if cast.spell == spell => Some(cast), _ => None }) }\n#[allow(dead_code)] fn cast_at<'a>(q: &'a dyn Query, event: &Event, id: ObjectId) -> Option<&'a Cast> { match event { Event::ZoneChange { old, new, reason: ZoneReason::Resolve, .. } if *new == id => cast_of(q, *old), _ => cast_of(q, id) } }\n";

fn emit_module(card: &Card, token_name: Option<&TokenName>) -> Result<EmittedModule> {
    validate::card(card).map_err(|e| Diagnostic {
        path: "card".into(),
        feature: "invalid AST".into(),
        message: e,
    })?;
    let mut c = Compiler::new();
    let mut parts = Parts::default();
    c.links_exile = format!("{:?}", card.abilities).contains("ExiledWith(Source)");
    for (i, a) in card.abilities.iter().enumerate() {
        if let Ability::Casting(CastingRule::AdditionalCost { cost, .. }) = a {
            c.casting_costs.insert(i as u32, cost.clone());
        }
    }
    for (i, a) in card.abilities.iter().enumerate() {
        c.attempt(format!("abilities[{i}]"), |c| {
            let compiled = c.ability(a, &format!("ability_{i}"), &mut parts, None);
            if c.dividing.take().is_some() {
                return Err(gap("target with a divided amount"));
            }
            compiled
        });
    }
    let ch = &card.characteristics;
    let types = ch
        .card_types
        .iter()
        .map(|t| card_type(t).map(|n| format!("CardType::{n}")))
        .collect::<Result<Vec<_>>>()?;
    let mut subtypes = Vec::new();
    for t in &ch.subtypes {
        let n = name(t);
        if !crate::cantrip_vocab::SUBTYPES.contains(&n.as_str()) {
            return Err(gap(format!("subtype {n}")).at("characteristics.subtypes"));
        }
        subtypes.push(format!("Subtype::{n}"));
    }
    let mut super_flags = [false; 3];
    for t in &ch.supertypes {
        match name(t).as_str() {
            "Basic" => super_flags[0] = true,
            "Legendary" => super_flags[1] = true,
            "Snow" => super_flags[2] = true,
            _ => return Err(gap("supertype")),
        }
    }
    let mana = ch
        .mana_cost
        .as_ref()
        .map(|m| mana(m))
        .transpose()?
        .unwrap_or_else(|| "ManaCost::ZERO".into());
    let i16_opt = |n: Option<i64>| -> Result<String> {
        match n {
            None => Ok("None".into()),
            Some(n) if i16::try_from(n).is_ok() => Ok(format!("Some({n})")),
            _ => Err(gap("printed characteristic exceeds i16")),
        }
    };
    // Filters are compiled through `&self`, so this one is found in their text.
    if c.functions.contains("cast_of(q, ") || c.functions.contains("cast_at(q, ") {
        c.helpers.insert("cast_of");
    }
    for helper in &c.helpers {
        c.functions.push_str(match *helper {
            "cast_of" => CAST_OF,
            "announced" => ANNOUNCED,
            "pay_question" => PAY_QUESTION,
            "about" => delayed::ABOUT,
            other => unreachable!("no helper {other}"),
        });
    }
    for made in std::mem::take(&mut c.made_preventions) {
        parts.table("granted_preventions", made);
    }
    for made in std::mem::take(&mut c.made_replacements) {
        parts.table("granted_replacements", made);
    }
    let rest = parts.rest();
    let Parts {
        keywords,
        abilities,
        statics,
        replacements,
        preventions,
        enters,
        block_rules,
        block_requirements,
        attack_rules,
        untap_bans,
        spell,
        modal,
        enchant,
        protection,
        ..
    } = parts;
    let enters = c.enters(&enters);
    let (pt, loyalty) = (
        ch.power_toughness,
        ch.loyalty
            .as_ref()
            .map(|n| match n {
                Expr::Constant(n) => Ok(*n),
                _ => Err(gap("printed loyalty expression")),
            })
            .transpose()?,
    );
    let mut colors = 0u8;
    for cl in &ch.color_indicator {
        color(cl)?;
        colors |= 1 << color_index(cl)?;
    }
    let colors = if ch.color_indicator.is_empty() {
        "None".into()
    } else {
        format!("Some(ColorSet({colors}))")
    };
    let fingerprint =
        corpus::digest(&serde_json::to_vec(&(card, token_name)).expect("serializable AST"));
    let oracle = token_name.is_none();
    let (name, token_words) = match token_name {
        Some(TokenName::FromSubtypes) => {
            let words = ch.subtypes.iter().map(name).collect::<Vec<_>>().join(" ");
            (
                format!("token!({words}).name"),
                format!("token!({words}).token_words"),
            )
        }
        _ => (quoted(&card.name), "&[]".into()),
    };
    let generator = generator_fingerprint();
    let api = crate::cantrip_vocab::API_FINGERPRINT;
    let rust = format!(
        "// Generated by mtgish-ast, schema {}, normalized SHA256 {fingerprint}.\n// Generator SHA256 {generator}; Cantrip API SHA256 {api}.\n#[allow(unused_imports)] use cantrip_api::*;\n{}\npub static CARD: CardDef = CardDef {{ name: {}, token_words: {token_words}, oracle: {oracle}, cost: {mana}, has_cost: {}, types: TypeSet::of(&[{}]), subtypes: SubtypeSet::of(&[{}]), supertypes: Supertypes {{ basic: {}, legendary: {}, snow: {} }}, colors: {colors}, power: {}, toughness: {}, loyalty: {}, defense: {}, keywords: Keywords::of(&[{}]), protection: ColorSet({protection}), spell: {spell}, abilities: &[{}], statics: &[{}], replacements: &[{}], preventions: &[{}], block_rules: &[{}], block_requirements: &[{}], attack_rules: &[{}], untap_bans: &[{}], delayed: &[{}], enters: {enters}, modal: {modal}, enchant: {enchant}, {rest}..CardDef::BLANK }};\npub static EXTRA: &[&CardDef] = &[{}];\n",
        SCHEMA_VERSION,
        c.functions,
        name,
        ch.mana_cost.is_some(),
        types.join(","),
        subtypes.join(","),
        super_flags[0],
        super_flags[1],
        super_flags[2],
        i16_opt(pt.map(|(p, _)| i64::from(p)))?,
        i16_opt(pt.map(|(_, t)| i64::from(t)))?,
        i16_opt(loyalty)?,
        i16_opt(ch.defense.map(i64::from))?,
        keywords.join(","),
        abilities.join(","),
        statics.join(","),
        replacements.join(","),
        preventions.join(","),
        block_rules.join(","),
        block_requirements.join(","),
        attack_rules.join(","),
        untap_bans.join(","),
        c.delayed.join(","),
        c.extras.iter().map(|p|format!("&{p}")).collect::<Vec<_>>().join(",")
    );
    Ok(EmittedModule {
        rust,
        extras: c.extras,
        gaps: c.gaps,
    })
}

/// An unsupported or malformed late record must preserve the previous module.
pub fn export(input: &Path, output: &Path) -> corpus::Result<usize> {
    let mut artifact = corpus::Artifact::create(output)?;
    let mut count = 0;
    for (i, line) in std::io::BufReader::new(std::fs::File::open(input)?)
        .lines()
        .enumerate()
    {
        let card: Card =
            ron::from_str(&line?).map_err(|e| format!("{}:{}: {e}", input.display(), i + 1))?;
        let module = module(&card).map_err(|d| {
            format!(
                "{}:{}: {}: {} ({})",
                input.display(),
                i + 1,
                d.path,
                d.feature,
                d.message
            )
        })?;
        writeln!(artifact, "pub mod card_{i} {{\n{module}\n}}")?;
        count += 1;
    }
    if count == 0 {
        return Err("card corpus is empty".into());
    }
    writeln!(artifact,"/// All card and token definitions required by these modules.\npub fn definitions() -> Vec<&'static cantrip_api::CardDef> {{ let mut definitions = Vec::new();")?;
    for i in 0..count {
        writeln!(
            artifact,
            "definitions.push(&card_{i}::CARD); definitions.extend_from_slice(card_{i}::EXTRA);"
        )?;
    }
    writeln!(artifact, "definitions }}")?;
    artifact.commit()?;
    Ok(count)
}
