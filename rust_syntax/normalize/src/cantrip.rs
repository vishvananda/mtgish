//! Compile portable semantic data to named Rust callbacks. Capability failures
//! are separate from AST lowering failures; no source node is interpreted here.
use crate::{corpus, ir::*, validate};
use std::{
    collections::BTreeMap,
    fmt::Write,
    io::{BufRead, Write as IoWrite},
    path::Path,
};

pub use crate::report::BackendDiagnostic as Diagnostic;
type Result<T> = std::result::Result<T, Diagnostic>;
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
fn generator_fingerprint() -> &'static str {
    static HASH: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    HASH.get_or_init(|| {
        let inputs: BTreeMap<_, _> = [
            ("cantrip.rs", include_bytes!("cantrip.rs").as_slice()),
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
    let n = name(value);
    if crate::cantrip_vocab::KEYWORDS.contains(&n.as_str()) {
        Ok(format!("Keyword::{n}"))
    } else {
        Err(gap(format!("keyword {n}")))
    }
}
fn color(value: &ManaColor) -> Result<&'static str> {
    Ok(match value {
        ManaColor::White => "White",
        ManaColor::Blue => "Blue",
        ManaColor::Black => "Black",
        ManaColor::Red => "Red",
        ManaColor::Green => "Green",
        ManaColor::Colorless => return Err(gap("colorless is not a color")),
    })
}
fn color_index(value: &ManaColor) -> usize {
    match value {
        ManaColor::White => 0,
        ManaColor::Blue => 1,
        ManaColor::Black => 2,
        ManaColor::Red => 3,
        ManaColor::Green => 4,
        ManaColor::Colorless => 5,
    }
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
            ManaSymbol::Color(c) => colors[color_index(c)] += 1,
            ManaSymbol::X => x += 1,
            ManaSymbol::Hybrid(a, b) => hybrid.push(format!(
                "Some((Mana::{}, Mana::{}))",
                ["W", "U", "B", "R", "G", "C"][color_index(a)],
                ["W", "U", "B", "R", "G", "C"][color_index(b)]
            )),
            ManaSymbol::MonocoloredHybrid(c) => {
                color(c)?;
                twobrid[color_index(c)] += 1;
            }
            ManaSymbol::Phyrexian(v) if v.len() == 1 => {
                color(&v[0])?;
                phyrexian[color_index(&v[0])] += 1;
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
}
impl Number {
    fn integer(n: i64) -> Self {
        Self {
            code: format!("{n}i64"),
            min: n.into(),
            max: n.into(),
        }
    }
    fn fit(&self, min: i128, max: i128, kind: &str) -> Result<String> {
        if self.min < min || self.max > max {
            Err(gap(format!("numeric range for {kind}")))
        } else {
            Ok(format!("({}) as {kind}", self.code))
        }
    }
    fn amount(&self, max: i128, kind: &str) -> Result<String> {
        if self.max > max {
            Err(gap(format!("numeric range for {kind}")))
        } else {
            Ok(format!("({}).max(0) as {kind}", self.code))
        }
    }
}
struct Compiler<'a> {
    mana_context: bool,
    slots: BTreeMap<u16, (usize, &'a TargetSpec)>,
    selections: BTreeMap<u32, String>,
    creations: BTreeMap<u32, String>,
    extras: Vec<String>,
    functions: String,
    next: usize,
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
                    let index = color_index(&color);
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
                if !colors.iter().any(|c| color_index(c) == color_index(color)) {
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
fn mana_restriction(value: &ManaUse) -> Result<String> {
    use crate::mtg_types::CardType as T;
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
impl<'a> Compiler<'a> {
    fn new() -> Self {
        Self {
            mana_context: false,
            slots: BTreeMap::new(),
            selections: BTreeMap::new(),
            creations: BTreeMap::new(),
            extras: Vec::new(),
            functions: String::new(),
            next: 0,
        }
    }
    fn source(&self) -> &'static str {
        if self.mana_context {
            "asker.source"
        } else {
            "item.object"
        }
    }
    fn controller(&self) -> &'static str {
        if self.mana_context {
            "asker.controller"
        } else {
            "item.controller"
        }
    }
    fn raw_object(&self, object: &ObjectRef) -> Result<String> {
        match object {
            ObjectRef::Source => Ok(self.source().into()),
            ObjectRef::Target { slot, index: 0 } if self.slots.contains_key(&slot.0) => {
                Ok(format!("t_{}_raw", self.slots[&slot.0].0))
            }
            _ => Err(gap("object observation reference")),
        }
    }
    fn objects(&self, objects: &ObjectSet) -> Result<String> {
        match objects {
            ObjectSet::One(ObjectRef::Source) => Ok(format!("vec![{}]",self.source())),
            ObjectSet::One(ObjectRef::Target { slot, index: 0 }) => {
                let i = self
                    .slots
                    .get(&slot.0)
                    .ok_or_else(|| gap("target reference"))?
                    .0;
                Ok(format!("t_{i}.into_iter().collect::<Vec<_>>()"))
            }
            ObjectSet::Selected(result) => self
                .selections
                .get(&result.0)
                .map(|local| format!("{local}.clone()"))
                .ok_or_else(|| gap("selection result outside generated scope")),
            ObjectSet::Created(result) => self.creations.get(&result.0)
                .map(|local| format!("{local}.clone()"))
                .ok_or_else(|| gap("token result outside generated scope")),
            ObjectSet::Battlefield(filter) => Ok(format!(
                "{{ let q: &dyn Query = g; let asker = &Asker::new({}, {}); q.state().battlefield().iter().copied().filter(|&id| {}).collect::<Vec<_>>() }}",
                self.source(),self.controller(),
                self.filter(filter)?
            )),
            _ => Err(gap("object collection")),
        }
    }
    fn player(&self, player: &PlayerRef) -> Result<String> {
        match player {
            PlayerRef::AbilityController => Ok(self.controller().into()),
            PlayerRef::Active => Ok("g.state().turn.active".into()),
            PlayerRef::ControllerOf(object) => {
                Ok(format!("g.controller({})", self.raw_object(object)?))
            }
            PlayerRef::OwnerOf(object) => Ok(format!("g.obj({}).owner", self.raw_object(object)?)),
            _ => Err(gap("player observation reference")),
        }
    }
    fn players(&self, players: &PlayerSet) -> Result<String> {
        match players {
            PlayerSet::One(PlayerRef::Target { slot, index: 0 }) => {
                let i = self
                    .slots
                    .get(&slot.0)
                    .ok_or_else(|| gap("player target"))?
                    .0;
                Ok(format!("t_{i}.into_iter().collect::<Vec<_>>()"))
            }
            PlayerSet::One(p) => Ok(format!("vec![{}]", self.player(p)?)),
            PlayerSet::All => Ok("g.state().apnap().collect::<Vec<_>>()".into()),
            PlayerSet::OpponentsOf(p) => Ok(format!(
                "g.state().opponents({}).collect::<Vec<_>>()",
                self.player(p)?
            )),
            _ => Err(gap("player collection")),
        }
    }
    fn number(&self, n: &Expr) -> Result<Number> {
        Ok(match n {
            Expr::Constant(n) => Number::integer(*n),
            Expr::AnnouncedX if !self.mana_context => Number {
                code: "i64::from(item.x)".into(),
                min: 0,
                max: u16::MAX.into(),
            },
            Expr::CountObjects(objects) => Number {
                code: format!(
                    "i64::try_from(({}).len()).expect(\"bounded object count\")",
                    self.objects(objects)?
                ),
                min: 0,
                max: u32::MAX.into(),
            },
            Expr::ObjectProperty {
                object,
                property,
                information: Information::CurrentOrLastKnown,
            } => {
                let object = self.raw_object(object)?;
                match property {
                    // API costs hold u8 counts and obj.x is u16. This bound
                    // includes all mana fields, hybrid and X, even after leaving
                    // the stack; it is strictly below i32::MAX.
                    Property::ManaValue => Number {
                        code: format!("i64::from(g.mana_value({object}))"),
                        min: 0,
                        max: 255 * (1 + 6 + 10 + 5) + 6 + 255 * i128::from(u16::MAX),
                    },
                    Property::Power | Property::Toughness => Number {
                        code: format!(
                            "i64::from(g.chars({object}).{}.unwrap_or(0))",
                            if matches!(property, Property::Power) {
                                "power"
                            } else {
                                "toughness"
                            }
                        ),
                        min: i16::MIN.into(),
                        max: i16::MAX.into(),
                    },
                    _ => return Err(gap("numeric object property")),
                }
            }
            Expr::PlayerLife(p) => Number {
                code: format!("i64::from(g.state().player({}).life)", self.player(p)?),
                min: i32::MIN.into(),
                max: i32::MAX.into(),
            },
            Expr::Add(a, b) | Expr::Subtract(a, b) | Expr::Multiply(a, b) => {
                let a = self.number(a)?;
                let b = self.number(b)?;
                let (symbol, min, max) = match n {
                    Expr::Add(..) => ("+", a.min + b.min, a.max + b.max),
                    Expr::Subtract(..) => ("-", a.min - b.max, a.max - b.min),
                    _ => {
                        let values = [a.min * b.min, a.min * b.max, a.max * b.min, a.max * b.max];
                        (
                            "*",
                            *values.iter().min().unwrap(),
                            *values.iter().max().unwrap(),
                        )
                    }
                };
                if min < i64::MIN.into() || max > i64::MAX.into() {
                    return Err(gap("arithmetic exceeds i64"));
                }
                Number {
                    code: format!("({} {symbol} {})", a.code, b.code),
                    min,
                    max,
                }
            }
            _ => return Err(gap("numeric expression")),
        })
    }
    fn filter(&self, f: &ObjectFilter) -> Result<String> {
        let combine = |items: &[ObjectFilter], join: &str, identity: &str| -> Result<String> {
            if items.is_empty() {
                return Ok(identity.into());
            }
            Ok(format!(
                "({})",
                items
                    .iter()
                    .map(|x| self.filter(x))
                    .collect::<Result<Vec<_>>>()?
                    .join(join)
            ))
        };
        Ok(match f {
            ObjectFilter::Any => "true".into(),
            ObjectFilter::All(items) => combine(items, " && ", "true")?,
            ObjectFilter::Either(items) => combine(items, " || ", "false")?,
            ObjectFilter::Not(p) => format!("!({})", self.filter(p)?),
            ObjectFilter::CardType(t) => format!("q.chars(id).is(CardType::{})", card_type(t)?),
            ObjectFilter::Supertype(t) => format!(
                "q.chars(id).supertypes.{}",
                match name(t).as_str() {
                    "Basic" => "basic",
                    "Legendary" => "legendary",
                    "Snow" => "snow",
                    _ => return Err(gap("supertype predicate")),
                }
            ),
            ObjectFilter::Subtype(t) => {
                let n = name(t);
                if !crate::cantrip_vocab::SUBTYPES.contains(&n.as_str()) {
                    return Err(gap(format!("subtype {n}")));
                }
                format!("q.chars(id).has_subtype(q.words(asker.source).subtype(Subtype::{n}))")
            }
            ObjectFilter::Color(c) => format!(
                "q.chars(id).colors.has(q.words(asker.source).color(Color::{}))",
                color(c)?
            ),
            ObjectFilter::Colorless => "q.chars(id).colors.is_empty()".into(),
            ObjectFilter::ControlledBy(PlayerSet::One(PlayerRef::AbilityController)) => {
                "q.controller(id) == asker.controller".into()
            }
            ObjectFilter::OwnedBy(PlayerSet::One(PlayerRef::AbilityController)) => {
                "q.obj(id).owner == asker.controller".into()
            }
            ObjectFilter::SameObject(ObjectRef::Source) => "id == asker.source".into(),
            ObjectFilter::Tapped => "q.obj(id).tapped".into(),
            _ => return Err(gap("target predicate")),
        })
    }
    fn announcement(&mut self, a: &'a Announcement, label: &str) -> Result<String> {
        if a.modes.is_some() {
            return Err(gap("modal announcement"));
        }
        self.slots.clear();
        let mut targets = Vec::new();
        for (i, spec) in a.targets.iter().enumerate() {
            if matches!(&spec.kind,TargetKind::Object(domain) if *domain != ObjectDomain::permanent())
            {
                return Err(gap("target object domain"));
            }
            if !matches!(spec.count, Count::Exactly(Expr::Constant(1))) {
                return Err(
                    gap("target cardinality other than exactly one").at(format!("targets[{i}]"))
                );
            }
            self.slots.insert(spec.slot.0, (i, spec));
            if matches!(spec.kind, TargetKind::DamageRecipient) {
                if spec.players.is_some() || !matches!(spec.filter, ObjectFilter::Any) {
                    return Err(gap("restricted damage-recipient target"));
                }
                targets.push("cantrip_mechanics::targets::ANY".into());
                continue;
            }
            let filter = match spec.kind {
                TargetKind::Object(_) => format!(
                    "matches!(t, Target::Object(id) if q.is_on_battlefield(id) && ({}))",
                    self.filter(&spec.filter)?
                ),
                TargetKind::Player => match &spec.players {
                    Some(PlayerSet::All) => "matches!(t, Target::Player(_))".into(),
                    Some(PlayerSet::OpponentsOf(PlayerRef::AbilityController)) => {
                        "matches!(t, Target::Player(p) if p != asker.controller)".into()
                    }
                    _ => return Err(gap("player target predicate")),
                },
                _ => unreachable!(),
            };
            let function = format!("{label}_target_{i}");
            writeln!(
                self.functions,
                "fn {function}(q: &dyn Query, asker: &Asker, t: Target) -> bool {{ {filter} }}"
            )
            .unwrap();
            targets.push(format!("target({function})"));
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
    fn effect(&mut self, body: &Program, label: &str) -> Result<String> {
        self.selections.clear();
        self.creations.clear();
        let mut out = String::new();
        for (i, spec) in self.slots.values() {
            match spec.kind {
                TargetKind::Object(_) => {
                    writeln!(out,"let t_{i}_raw = g.state().target(item, {i}).object().expect(\"announced object target\"); let t_{i} = cantrip_mechanics::targets::object(g, item, {i});").unwrap();
                }
                TargetKind::Player => {
                    writeln!(
                        out,
                        "let t_{i} = cantrip_mechanics::targets::player(g, item, {i});"
                    )
                    .unwrap();
                }
                TargetKind::DamageRecipient => return Err(gap("any-target damage emission")),
            }
        }
        self.program(body, &mut out, true)?;
        writeln!(self.functions,"fn {label}(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {{ {out} Ok(()) }}").unwrap();
        Ok(label.into())
    }
    fn program(&mut self, body: &Program, out: &mut String, boundaries: bool) -> Result<()> {
        for (i, instruction) in body.instructions.iter().enumerate() {
            match instruction {
                Instruction::Effect {
                    operation:
                        Operation::ChooseObjects {
                            player,
                            objects,
                            count,
                            constraints,
                        },
                    result,
                } => {
                    if let Some(constraint) = constraints.first() {
                        return Err(gap(format!("selection constraint {}", form(constraint)))
                            .at(format!("instructions[{i}]")));
                    }
                    // The consumer supplies prompt wording, not targeting or
                    // rules semantics. Other consumers need their own wording.
                    let purpose = match body.instructions.get(i + 1) {
                        Some(Instruction::Effect {
                            operation:
                                Operation::Tap {
                                    objects: ObjectSet::Selected(selected),
                                    tapped,
                                },
                            ..
                        }) if selected.0 == result.0 => {
                            if *tapped {
                                "Tap"
                            } else {
                                "Untap"
                            }
                        }
                        _ => {
                            return Err(gap("resolution choice prompt purpose")
                                .at(format!("instructions[{i}]")));
                        }
                    };
                    self.choose(player, objects, count, result, purpose, out)
                        .map_err(|e| e.at(format!("instructions[{i}]")))?;
                }
                Instruction::Effect { operation, result } => self
                    .operation(operation, result, out)
                    .map_err(|e| e.at(format!("instructions[{i}]")))?,
                Instruction::Group(group) => {
                    // Today groups lower selection plus one mutation. A group
                    // with multiple mutations needs an outer instruction scope,
                    // including a policy for exporting result bindings.
                    if group
                        .instructions
                        .iter()
                        .filter(|instruction| {
                            !matches!(
                                instruction,
                                Instruction::Effect {
                                    operation: Operation::ChooseObjects { .. },
                                    ..
                                }
                            )
                        })
                        .count()
                        > 1
                        || group
                            .instructions
                            .iter()
                            .any(|instruction| !matches!(instruction, Instruction::Effect { .. }))
                    {
                        return Err(gap("grouped instruction with multiple mutations")
                            .at(format!("instructions[{i}].group")));
                    }
                    self.program(group, out, false)
                        .map_err(|e| e.at(format!("instructions[{i}].group")))?;
                }
                _ => {
                    return Err(
                        gap("conditional or optional instruction").at(format!("instructions[{i}]"))
                    );
                }
            }
            if boundaries {
                writeln!(out, "g.end_sentence();").unwrap();
            }
        }
        Ok(())
    }
    fn choose(
        &mut self,
        player: &PlayerRef,
        objects: &ObjectSet,
        count: &Count,
        result: &ResultId,
        purpose: &str,
        out: &mut String,
    ) -> Result<()> {
        let count = match count {
            Count::AnyNumber => "cantrip_mechanics::select::Count::AnyNumber".into(),
            Count::Exactly(n) | Count::UpTo(n) | Count::AsManyAsCan(n) => {
                let kind = match count {
                    Count::Exactly(_) => "Exactly",
                    Count::UpTo(_) => "UpTo",
                    _ => "AsManyAsCan",
                };
                format!(
                    "cantrip_mechanics::select::Count::{kind}({})",
                    self.number(n)?.amount(u32::MAX.into(), "u32")?
                )
            }
            Count::AtLeast(_) => return Err(gap("at-least resolution choice")),
            Count::Between { .. } => return Err(gap("bounded resolution choice")),
            Count::PerPlayer { .. } => return Err(gap("per-player resolution choice")),
        };
        let local = format!("selected_{}", self.next);
        self.next += 1;
        writeln!(out, "let {local}_player = {}; let {local}_candidates = {}; let {local}_rule = cantrip_mechanics::select::Selection::of({count}, &[]); let {local} = cantrip_mechanics::choose::select(g, {local}_player, item.object, Purpose::{purpose}, {local}_candidates, {local}_rule)?;", self.player(player)?, self.objects(objects)?).unwrap();
        self.selections.insert(result.0, local);
        Ok(())
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
        writeln!(self.functions, "pub mod token_{id} {{\n{}\n}}", child.rust).unwrap();
        let path = format!("token_{id}::CARD");
        self.extras.push(path.clone());
        self.extras
            .extend(child.extras.into_iter().map(|p| format!("token_{id}::{p}")));
        Ok(path)
    }
    fn operation(&mut self, op: &Operation, result: &ResultId, out: &mut String) -> Result<()> {
        let id = self.next;
        self.next += 1;
        match op {
            Operation::AddMana {
                players,
                mana,
                restriction,
            } => {
                let plan = mana_plan(mana)?;
                self.emit_mana(&plan, players, restriction.as_ref(), None, out)?;
            }
            Operation::CreateTokens {
                players,
                tokens,
                entry,
            } => {
                if !entry.counters.is_empty() {
                    return Err(gap("tokens entering with counters"));
                }
                let first = &tokens[0].words;
                if tokens
                    .iter()
                    .any(|t| std::mem::discriminant(&t.words) != std::mem::discriminant(first))
                {
                    return Err(gap("mixed token text origins in one creation batch"));
                }
                writeln!(
                    out,
                    "let players_{id} = {}; let mut tokens_{id} = Vec::new();",
                    self.players(players)?
                )
                .unwrap();
                for (i, token) in tokens.iter().enumerate() {
                    let amount = self.number(&token.amount)?.amount(u32::MAX.into(), "u32")?;
                    let TokenRecipe::Defined(definition) = &token.recipe else {
                        return Err(gap("a token copy has no Cantrip form yet"));
                    };
                    let definition = self
                        .token_definition(definition)
                        .map_err(|e| e.at(format!("tokens[{i}].definition")))?;
                    writeln!(out,"let count_{id}_{i} = {amount}; let def_{id}_{i} = g.card_id(&{definition});").unwrap();
                }
                writeln!(out, "for player in players_{id} {{").unwrap();
                for i in 0..tokens.len() {
                    writeln!(out,"tokens_{id}.extend(std::iter::repeat((def_{id}_{i},player)).take(count_{id}_{i} as usize));").unwrap();
                }
                let words = match first {
                    TokenWords::Rules => "Words::NONE",
                    TokenWords::CreatingEffect => "g.words(item.object)",
                };
                writeln!(out,"}} let words_{id} = {words}; let created_{id} = g.create_tokens_worded(&tokens_{id}, {}, None, words_{id})?;",entry.tapped).unwrap();
                self.creations.insert(result.0, format!("created_{id}"));
            }
            Operation::Draw { players, amount }
            | Operation::GainLife { players, amount }
            | Operation::LoseLife { players, amount }
            | Operation::Mill { players, amount } => {
                let number = self.number(amount)?;
                let (method, max, ty) = match op {
                    Operation::Draw { .. } => ("draw", u32::MAX.into(), "u32"),
                    Operation::Mill { .. } => ("mill", u32::MAX.into(), "u32"),
                    Operation::GainLife { .. } => ("gain_life", i32::MAX.into(), "i32"),
                    _ => ("lose_life", i32::MAX.into(), "i32"),
                };
                writeln!(out,"let amount_{id} = {}; let players_{id} = {}; for player in players_{id} {{ g.{method}(player, amount_{id}); }}",number.amount(max,ty)?,self.players(players)?).unwrap();
            }
            Operation::Destroy {
                objects,
                regeneration,
            } => {
                writeln!(
                    out,
                    "let objects_{id} = {}; g.{}(&objects_{id});",
                    self.objects(objects)?,
                    if *regeneration {
                        "destroy_all"
                    } else {
                        "destroy_all_no_regen"
                    }
                )
                .unwrap();
            }
            Operation::Move {
                objects,
                destination: Destination::OwnersHand,
                entry,
                face_down: false,
            } => {
                if entry.tapped || !entry.counters.is_empty() {
                    return Err(gap("move arrival flags"));
                }
                writeln!(
                    out,
                    "let objects_{id} = {}; g.return_to_hand(&objects_{id});",
                    self.objects(objects)?
                )
                .unwrap();
            }
            Operation::Tap { objects, tapped } => {
                writeln!(
                    out,
                    "let objects_{id} = {}; cantrip_api::instruction(g, |g| {{ for object in objects_{id} {{ g.{}(object); }} }});",
                    self.objects(objects)?,
                    if *tapped { "tap" } else { "untap" }
                )
                .unwrap();
            }
            Operation::CreateContinuousEffect {
                source,
                affected,
                changes,
                duration: crate::ir::Duration::EndOfTurn,
                evaluation: EffectEvaluation::Snapshot,
            } => {
                // Mixed continuous changes are intentionally not batched on the
                // unverified assumption that sentence or verb counts prove it.
                if changes.len() != 1 {
                    return Err(gap(
                        "continuous-effect instruction grouping requires verified boundaries",
                    ));
                }
                let Change::Pump { power, toughness } = &changes[0] else {
                    return Err(gap("continuous change"));
                };
                let EffectSubjects::Objects(affected) = affected else {
                    return Err(gap("continuous effect on players"));
                };
                let power = self
                    .number(power)?
                    .fit(i16::MIN.into(), i16::MAX.into(), "i16")?;
                let toughness =
                    self.number(toughness)?
                        .fit(i16::MIN.into(), i16::MAX.into(), "i16")?;
                writeln!(out,"let power_{id} = {power}; let toughness_{id} = {toughness}; let objects_{id} = {}; cantrip_api::instruction(g, |g| {{ for object in objects_{id} {{ cantrip_mechanics::pump(g, {}, object, power_{id}, toughness_{id}); }} }});",self.objects(affected)?,self.raw_object(source)?).unwrap();
            }
            Operation::Primitive { name, .. } => {
                return Err(gap(format!("primitive operation {name}")))
            }
            _ => return Err(gap(format!("operation {}", form(op)))),
        }
        Ok(())
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
        writeln!(out, "let mana_players_{id} = {};", self.players(players)?).unwrap();
        match plan {
            ManaPlan::Fixed(units) => {
                for (i, (_, amount)) in units.iter().enumerate() {
                    writeln!(
                        out,
                        "let mana_{id}_{i} = {};",
                        self.number(amount)?.amount(u32::MAX.into(), "u32")?
                    )
                    .unwrap();
                }
                writeln!(out, "for player in mana_players_{id} {{").unwrap();
                for (i, (color, _)) in units.iter().enumerate() {
                    out.push_str(&add(
                        "player",
                        &format!(
                            "Mana::{}",
                            ["W", "U", "B", "R", "G", "C"][color_index(color)]
                        ),
                        &format!("mana_{id}_{i}"),
                    ));
                }
                out.push_str("}\n");
            }
            ManaPlan::SameColor { colors, amount } | ManaPlan::Combination { colors, amount } => {
                let mask = colors.iter().fold(0u8, |m, c| m | 1 << color_index(c));
                let combination = matches!(plan, ManaPlan::Combination { .. });
                let number = self.number(amount)?;
                writeln!(
                    out,
                    "let mana_{id} = {};",
                    number.amount(
                        if combination {
                            u8::MAX.into()
                        } else {
                            u32::MAX.into()
                        },
                        if combination { "u8" } else { "u32" }
                    )?
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
                    writeln!(out,"if mana_{id} > 0 {{ let colors = cantrip_mechanics::choose_colors(g, player, {}, ColorSet({mask}), {})?;",self.source(),if combination{format!("mana_{id}")}else{"1".into()}).unwrap();
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
        }] = body.instructions.as_slice()
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
                        pool[color_index(color)] =
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
                let mask = colors.iter().fold(0u8, |m, c| m | 1 << color_index(c));
                metadata.push_str(&format!(".choose_color().colors(ColorSet({mask}))"));
                if let Expr::Constant(n) = amount {
                    if (0..=255).contains(n) {
                        metadata.push_str(&format!(".yields_any({n})"));
                    }
                }
            }
            ManaPlan::Combination { colors, amount } => {
                let mask = colors.iter().fold(0u8, |m, c| m | 1 << color_index(c));
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
        self.mana_context = true;
        self.emit_mana(
            &plan,
            &PlayerSet::One(PlayerRef::AbilityController),
            restriction.as_ref(),
            if matches!(plan, ManaPlan::SameColor { .. }) {
                Some("chosen")
            } else {
                None
            },
            &mut out,
        )?;
        self.mana_context = false;
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
fn cost(value: &Cost) -> Result<String> {
    let mut symbols = Vec::new();
    let (mut tap, mut life_total, mut loyalty_delta) = (false, 0u32, None);
    let mut sacrifice_this = false;
    for part in &value.parts {
        match part {
            CostPart::Mana(mana) => symbols.extend(mana.iter().cloned()),
            CostPart::TapSymbol if !tap => tap = true,
            CostPart::Sacrifice {
                objects: ObjectSet::One(ObjectRef::Source),
                choose: None,
            } if !sacrifice_this => sacrifice_this = true,
            CostPart::Life(Expr::Constant(n)) if (0..=255).contains(n) => {
                life_total = life_total
                    .checked_add(*n as u32)
                    .ok_or_else(|| gap("life cost overflow"))?;
            }
            CostPart::Loyalty(Expr::Constant(n))
                if (-128..=127).contains(n) && loyalty_delta.is_none() =>
            {
                loyalty_delta = Some(*n)
            }
            _ => return Err(gap("cost part")),
        }
    }
    if life_total > 255 {
        return Err(gap("aggregate life cost exceeds u8"));
    }
    let mut parts = vec![format!("Cost::mana({})", mana(&symbols)?)];
    if sacrifice_this {
        parts.push("SACRIFICE_THIS".into());
    }
    if tap {
        parts.push("Cost { tap: true, ..Cost::NONE }".into());
    }
    if life_total > 0 {
        parts.push(format!("life({life_total})"));
    }
    if let Some(delta) = loyalty_delta {
        parts.push(format!("loyalty({delta})"));
    }
    Ok(parts
        .into_iter()
        .fold("Cost::NONE".into(), |a, b| format!("{a}.and({b})")))
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
    emit_module(card, None).map(|module| module.rust)
}
struct EmittedModule {
    rust: String,
    extras: Vec<String>,
}
fn emit_module(card: &Card, token_name: Option<&TokenName>) -> Result<EmittedModule> {
    validate::card(card).map_err(|e| Diagnostic {
        path: "card".into(),
        feature: "invalid AST".into(),
        message: e,
    })?;
    let mut c = Compiler::new();
    let mut abilities = Vec::new();
    let mut keywords = Vec::new();
    let mut spell = "None".to_string();
    for (i, a) in card.abilities.iter().enumerate() {
        let compiled: Result<()> = (|| {
            match a {
                Ability::Keyword(k) => keywords.push(keyword(k)?),
                Ability::Casting(_) => return Err(gap("casting rules")),
                Ability::Scoped { .. } => return Err(gap("source-zone scope")),
                Ability::Spell { announcement, body } => {
                    if spell != "None" {
                        return Err(gap("multiple spell bodies"));
                    }
                    let targets = c.announcement(announcement, &format!("spell_{i}"))?;
                    let effect = c.effect(body, &format!("spell_{i}"))?;
                    spell = format!("spell({targets}, {effect})");
                }
                Ability::Activated {
                    cost: price,
                    limits,
                    announcement,
                    body,
                    ..
                } => {
                    if limits.once {
                        return Err(gap("once-per-game activation"));
                    }
                    if limits.condition.is_some() {
                        return Err(gap("activation condition"));
                    }
                    let targets = c.announcement(announcement, &format!("activated_{i}"))?;
                    let label = ability_text(body);
                    let mut def = if crate::semantics::activation_kind(price, announcement, body)
                        == crate::semantics::ActivationKind::Mana
                    {
                        let adds = c.mana_ability(body, &format!("mana_{i}"))?;
                        format!("mana_ability({}, {adds}, {})", cost(price)?, quoted(&label))
                    } else {
                        let effect = c.effect(body, &format!("activated_{i}"))?;
                        format!(
                            "ability({}, {effect}, {}).targets({targets})",
                            cost(price)?,
                            quoted(&label)
                        )
                    };
                    if limits.sorcery {
                        def.push_str(".sorcery_speed()");
                    }
                    if limits.once_each_turn {
                        def.push_str(".once_each_turn()");
                    }
                    abilities.push(def);
                }
                Ability::Triggered {
                    event,
                    intervening_if,
                    limits,
                    announcement,
                    body,
                } => {
                    if intervening_if.is_some()
                        || limits.per_turn.is_some()
                        || limits.total.is_some()
                    {
                        return Err(gap("trigger conditions or frequency limits"));
                    }
                    let EventSpec::ZoneChange {
                        objects: ObjectFilter::SameObject(ObjectRef::Source),
                        from: None,
                        to: EventZone::Battlefield,
                    } = event
                    else {
                        return Err(gap("trigger event matcher"));
                    };
                    let targets = c.announcement(announcement, &format!("triggered_{i}"))?;
                    let effect = c.effect(body, &format!("triggered_{i}"))?;
                    let text = format!("When this enters, {}", ability_text(body));
                    abilities.push(format!("triggered(when(EventMask::ZONE_CHANGE, cantrip_mechanics::triggers::this_enters), {effect}, {}).targets({targets})", quoted(&text)));
                }
                _ => return Err(gap("triggered, static, or conditional ability")),
            }
            Ok(())
        })();
        compiled.map_err(|e| e.at(format!("abilities[{i}]")))?;
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
        colors |= 1 << color_index(cl);
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
        "// Generated by mtgish-ast, schema {}, normalized SHA256 {fingerprint}.\n// Generator SHA256 {generator}; Cantrip API SHA256 {api}.\n#[allow(unused_imports)] use cantrip_api::*;\n{}\npub static CARD: CardDef = CardDef {{ name: {}, token_words: {token_words}, oracle: {oracle}, cost: {mana}, has_cost: {}, types: TypeSet::of(&[{}]), subtypes: SubtypeSet::of(&[{}]), supertypes: Supertypes {{ basic: {}, legendary: {}, snow: {} }}, colors: {colors}, power: {}, toughness: {}, loyalty: {}, defense: {}, keywords: Keywords::of(&[{}]), spell: {spell}, abilities: &[{}], ..CardDef::BLANK }};\npub static EXTRA: &[&CardDef] = &[{}];\n",
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
        c.extras.iter().map(|p|format!("&{p}")).collect::<Vec<_>>().join(",")
    );
    Ok(EmittedModule {
        rust,
        extras: c.extras,
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
