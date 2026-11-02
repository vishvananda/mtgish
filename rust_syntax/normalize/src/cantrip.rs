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

mod card_file;
mod casting;
mod chosen;
mod copies;
mod costs;
mod delayed;
mod effects;
mod faces;
mod first_printed;
mod keywords;
mod lane_f;
mod lane_g;
mod lane_h;
mod lane_i;
mod lane_j;
mod lane_k;
mod lane_l;
mod lane_m;
mod lane_n;
mod lane_o;
mod lane_p;
mod lane_q;
mod mechanics;
mod option_words;
mod target_counts;
pub mod planned;
mod players;
mod playing;
pub mod pr_form;
mod read;
mod replacements;
mod statics;
mod survey;
mod triggers;
pub use crate::report::BackendDiagnostic as Diagnostic;
use effects::rule_name;
pub use lane_f::{is_card, whole, whole_gaps, whole_module, whole_printed, Whole, WholeFace};
use lane_j::{PaidFor, Picked, Share};
use read::{counter_kind, zone_kind, Candidate, Carried, EventAt, Frame, History, Pending, Reader};
pub use survey::survey;
type Result<T> = std::result::Result<T, Diagnostic>;
/// The result of an instruction that already reported a gap, read where its
/// value stands in. It says nothing new, so it is not counted against the card.
const FOLLOWS_GAP: &str = "depends on an instruction with a gap";
/// What a list of objects from a failed instruction reads as until it has code.
const NO_OBJECTS: &str = "Vec::<ObjectId>::new()";
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
fn constant_one(count: &Count) -> bool {
    matches!(count, Count::UpTo(Expr::Constant(1)))
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
                "cantrip/lane_h.rs",
                include_bytes!("cantrip/lane_h.rs").as_slice(),
            ),
            (
                "cantrip/lane_k.rs",
                include_bytes!("cantrip/lane_k.rs").as_slice(),
            ),
            (
                "cantrip/card_file.rs",
                include_bytes!("cantrip/card_file.rs").as_slice(),
            ),
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
            (
                "cantrip/chosen.rs",
                include_bytes!("cantrip/chosen.rs").as_slice(),
            ),
            (
                "cantrip/copies.rs",
                include_bytes!("cantrip/copies.rs").as_slice(),
            ),
            (
                "cantrip/keywords.rs",
                include_bytes!("cantrip/keywords.rs").as_slice(),
            ),
            (
                "cantrip/lane_f.rs",
                include_bytes!("cantrip/lane_f.rs").as_slice(),
            ),
            (
                "cantrip/lane_g.rs",
                include_bytes!("cantrip/lane_g.rs").as_slice(),
            ),
            (
                "cantrip/lane_i.rs",
                include_bytes!("cantrip/lane_i.rs").as_slice(),
            ),
            (
                "cantrip/lane_j.rs",
                include_bytes!("cantrip/lane_j.rs").as_slice(),
            ),
            (
                "cantrip/lane_l.rs",
                include_bytes!("cantrip/lane_l.rs").as_slice(),
            ),
            (
                "cantrip/lane_m.rs",
                include_bytes!("cantrip/lane_m.rs").as_slice(),
            ),
            (
                "cantrip/lane_n.rs",
                include_bytes!("cantrip/lane_n.rs").as_slice(),
            ),
            (
                "cantrip/lane_o.rs",
                include_bytes!("cantrip/lane_o.rs").as_slice(),
            ),
            (
                "cantrip/lane_p.rs",
                include_bytes!("cantrip/lane_p.rs").as_slice(),
            ),
            (
                "cantrip/mechanics.rs",
                include_bytes!("cantrip/mechanics.rs").as_slice(),
            ),
            (
                "cantrip/planned.rs",
                include_bytes!("cantrip/planned.rs").as_slice(),
            ),
            (
                "cantrip/playing.rs",
                include_bytes!("cantrip/playing.rs").as_slice(),
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
                other => return lane_p::landwalk(other),
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
        Protection::Objects {
            kind: ObjectKind::Any,
            filter: ObjectFilter::Subtype(s),
        } => {
            let word = name(s);
            if !crate::cantrip_vocab::SUBTYPES.contains(&word.as_str()) {
                return Err(gap(format!("subtype {word}")));
            }
            grants.push((
                lane_g::planned_kind(
                    "engine: no effect gives protection from a subtype",
                    format!("EffectKind::ProtectionFromSubtype(Subtype::{word})"),
                ),
                format!("chars.protection_subtypes = chars.protection_subtypes.with(q.words(asker.source).subtype(Subtype::{word}));"),
            ));
        }
        Protection::Objects {
            kind: ObjectKind::Any,
            filter: ObjectFilter::Any,
        } => grants.push((
            "EffectKind::ProtectionFromEverything".into(),
            "chars.protection_everything = true;".into(),
        )),
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
    let mut beyond = String::new();
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
            ManaSymbol::Snow => beyond.push_str(".with_snow(1)"),
            ManaSymbol::Phyrexian(v) => {
                let colors = v.iter().map(color).collect::<Result<Vec<_>>>()?;
                let colors = colors.iter().map(|c| format!("Color::{c}"));
                let colors = colors.collect::<Vec<_>>().join(", ");
                beyond.push_str(&format!(".with_phyrexian_hybrid(&[{colors}])"));
            }
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
    let cost = format!(
        "ManaCost {{ generic: {generic}, x: {x}, symbols: {colors:?}, hybrid: [{}], twobrid: {twobrid:?}, phyrexian: {phyrexian:?} }}",
        hybrid.join(",")
    );
    Ok(match beyond.is_empty() {
        true => cost,
        false => planned::planned_value(
            "engine: no snow or hybrid Phyrexian mana symbol",
            &cost,
            &format!("{cost}{beyond}"),
        ),
    })
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
    /// The file's holes for what has no place in its code: see `planned`.
    holes: String,
    /// Each ability's printed line, where its Oracle text is known.
    printed: Vec<Option<lane_k::Line>>,
    /// A Saga's chapters as printed, by number.
    chapters: BTreeMap<i64, String>,
    /// The printed line of the ability being compiled.
    line: Option<lane_k::Line>,
    /// What of that line's quotes a granted ability has already read.
    quoted_used: BTreeSet<usize>,
    /// The next choice is a "you may": choosing nothing is declining.
    may_choose: bool,
    /// `.with(part)` the next resolution choice takes: a rule of the set it picks.
    choice_rule: String,
    /// What runs once that choice found something, the rest of the block with it.
    chosen_guard: Option<String>,
    /// That guard is open, and the block has to close it.
    guard_opened: usize,
    /// The target of the announcement being compiled that is the second of "two
    /// target creatures", which the AST keeps as a slot of its own.
    second_of_two: Option<usize>,
    /// Of the local holding a mill of a set number, that number.
    whole_mills: std::collections::BTreeMap<String, String>,
    /// Whether the face's spell has targets.
    spell_targets: bool,
    /// "Kicker—Sacrifice an artifact or creature", as the face prints it.
    kicker_line: Option<String>,
    /// The face's kicker costs in print: where there are two, "Kicker {1}{U} and/or
    /// {B}" (CR 702.33e), the card's `kicker` is the first and its `optional_cost`
    /// the second, whose payment is `paid`, not `kicked`.
    kickers: Vec<String>,
    /// The face is a sorcery.
    sorcery: bool,
    /// What this face's replacements already say it enters as, abilities and
    /// keywords: `Entering::becomes` is one value, and each kicker can add to it.
    entering_as: (Vec<String>, Vec<String>),
    /// The `labels` a question the code asks wants.
    wanted_labels: Option<String>,
    /// What each counter stage of the face prints.
    stages: BTreeMap<i64, Vec<lane_k::Line>>,
    /// What's left of the printed lines of the stage being compiled.
    stage_lines: Vec<lane_k::Line>,
    /// The next "may" is done once each turn, under this key.
    do_once: Option<u8>,
    /// The abilities of one sentence are being written ("whenever this or that"):
    /// the key of its "do this only once each turn", one for them all.
    once_shared: Option<Option<u8>>,
    /// An "if it's the first time" the printed line asks of the object, as the
    /// resolving ability reads it (603.4).
    first_again: Option<String>,
    /// Reading a test of whether this spell may be cast.
    cast_test: bool,
    /// Compiling a delayed triggered ability, whose words are the sentence that
    /// makes it (`delayed_words`).
    waiting_trigger: bool,
    /// All of the face's Oracle text: a printed line holds one ability's words.
    face_text: String,
    /// The next tap or untap is a "may", which offers doing nothing with them.
    may_tap_or_untap: bool,
    /// Payments of "unless you pay": those ask whether to, whatever paying is.
    unless_paid: BTreeSet<u32>,
    /// The replacement program being compiled.
    replacing: lane_n::Replacing,
    /// The closure being opened is of an ability with a matcher to read it.
    watching: bool,
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
    /// The trigger being written waits on an event Cantrip doesn't have yet, so
    /// what that event would say ("that many") comes with it.
    event_planned: bool,
    /// A trigger on damage to a player fires with that player as its subject.
    recipient_subject: bool,
    candidates: std::cell::RefCell<Vec<Candidate>>,
    /// What this spell keeps of the board as it is cast: `CardDef::cast_fact`.
    cast_fact: std::rc::Rc<std::cell::RefCell<Option<String>>>,
    /// How many instances of an ability a static ability gave with "only once
    /// each turn": `CardDef::granted_once_each_turn`, the fn's body.
    granted_once: std::rc::Rc<std::cell::RefCell<Option<String>>>,
    affected_in_scope: bool,
    /// The condition of the enclosing `Ability::Conditional`, asked as layers run.
    static_guard: Option<Condition>,
    /// Asked with a triggered ability's event and not again: a Class's level.
    trigger_gate: Option<Condition>,
    /// What this triggered ability's event is always about, as "this creature"
    /// is: read where no event reaches, as "another creature" in a cost.
    event_object: Option<ObjectRef>,
    /// While an instruction is compiled to happen only if its players exist:
    /// the local each stands for, and how to read it. See `lane_j`.
    present: std::rc::Rc<std::cell::RefCell<Option<Vec<(String, String)>>>>,
    /// "An opponent chooses": who has been picked so far, and how to pick the
    /// ones the instruction being compiled named first. See `lane_j`.
    picked: std::rc::Rc<std::cell::RefCell<Picked>>,
    /// What each player's part of a batch was, by the result it is part of.
    shares: BTreeMap<u32, (String, Share)>,
    /// What the cost being compiled pays for, and whether it has an {X}.
    paid_for: PaidFor,
    mana_x: bool,
    slots: BTreeMap<u16, (usize, TargetSpec)>,
    selections: BTreeMap<u32, String>,
    /// "A source of your choice": each choice's local and the quality it asks.
    damage_sources: BTreeMap<String, ObjectFilter>,
    /// The quality of the source of your choice the made effect at hand is about.
    choosing_source: Option<ObjectFilter>,
    /// What later reads ask of the instructions before them (`lane_m`).
    notes: lane_m::Notes,
    /// By slot: who controlled the items an earlier step countered.
    countered: BTreeMap<u16, String>,
    /// The instruction that separated a face-down pile and a face-up pile.
    face_piles: Option<usize>,
    /// An instruction exiled cards into piles ("in a face-down pile"), which
    /// a later choice of "those piles" reads.
    exiled_piles: bool,
    /// The piles players separated (CR 700.3): the local of their rows, each a
    /// separator, its piles, the pile chosen and who may choose it.
    piles: Option<String>,
    /// A separation inside a per-player walk pushes its row into this local.
    piles_walk: Option<String>,
    /// How many piles each row has, and whether they are face down.
    pile_shape: (i64, bool),
    /// The `PlayersPayRepeatedly` whose unpaid payments a walk keeps, one
    /// entry of its payer for each.
    unpaid_rows: BTreeSet<u32>,
    /// What each player of a walk turned face up, as said for the walked player:
    /// "among them" of a later walk of the same players (Pyxis of Pandemonium).
    turned_up: Option<(u32, ObjectSet)>,
    /// The cards that ended each reveal, by the result that names all it showed.
    reveal_matches: BTreeMap<u32, String>,
    /// This resolution has revealed something. The AST does not always tie a
    /// later move to the reveal ("reveal the top card and put it into your hand"
    /// names the top card twice), and an unmarked move of a card all players saw
    /// is refused, so every later move into a hand or library is marked.
    revealed: bool,
    /// What was left over of a choice between piles, by the result of the choice.
    unchosen: BTreeMap<u32, String>,
    /// The words of a choice of `ValueKind::Word`, the card's `labels`.
    word_options: Vec<String>,
    /// `Vec<(PlayerId, u8)>`: the word chosen for each player.
    player_words: Option<String>,
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
    /// A cost this card's spell pays beyond its additional costs is being written.
    spell_cost: bool,
    /// The cost of the option of a choice of costs whose body is being compiled
    /// ("as an additional cost, choose a creature you control or ...").
    option_cost: Option<Cost>,
    /// "As this enters, choose a creature": the permanent keeps the object chosen
    /// (`Entering::chosen_object`), which its other abilities read (CR 607.2d).
    chose_object_as_enters: bool,
    /// What a mechanic's cost of this card's own spell took, by earlier objects' word.
    cast_pays: Vec<&'static str>,
    /// "As this enters, sacrifice any number": how many is what it keeps.
    enters_sacrifices: bool,
    /// An instant or sorcery whose one instruction that deals damage is all it deals.
    damages_once: bool,
    /// Moves that already said when they end, in one call with the move.
    returning: BTreeSet<u32>,
    /// The `Divide` of the announcement just compiled, until a definition that
    /// can say so takes it. What's left of it at the ability's end is a gap.
    dividing: Option<String>,
    /// Locals an effect declares up front to outlive its nested blocks. `None`
    /// in code that isn't an effect's own body.
    hoisted: Option<Vec<String>>,
    /// Values kept past the block or turn that chose them, and the lines that keep
    /// them, which go if nothing read them.
    unread: Vec<(String, Vec<String>)>,
    /// Locals that hold a value each player chose, by kind.
    chosen_lists: Vec<(ValueKind, String)>,
    /// The bound player whose own choice in a walk a filter reads.
    own_choice: std::cell::RefCell<Option<String>>,
    /// The local of the spells the last cast of cards made.
    played: Option<String>,
    /// Choices and searches that yield one object at most.
    single: BTreeSet<u32>,
    /// Results that moved or found one object at most.
    known_one: BTreeSet<u32>,
    /// Some ability of the card reads what was exiled with its source.
    links_exile: bool,
    /// Some ability of the card takes cards from one player's graveyard.
    one_graveyard: bool,
    /// A face other than the definition an object's `card` names: `lane_f`.
    own_face: bool,
    /// Which door of its Room this face is.
    door: Option<lane_f::Door>,
    /// The announcement slot whose filter is being compiled.
    choosing: Option<usize>,
    performed: BTreeMap<u32, String>,
    /// The payments whose `unpaid_` list this resolution keeps.
    unpaid: BTreeSet<u32>,
    /// The players a damage instruction dealt damage to.
    hurt_players: BTreeMap<u32, String>,
    /// The flips a coin-flipping instruction won and lost.
    coins: Option<(String, String)>,
    /// The players who each flipped a coin, and how each came up.
    coin_turns: Option<(String, String)>,
    /// A coin flipped for each object: `(object, flip)`.
    coin_objects: Option<String>,
    /// A damage instruction's excess (CR 120.10): the list of what each recipient
    /// took past lethal, its recipients' local, and the one object it aimed at.
    excess: BTreeMap<u32, (String, String, Option<String>)>,
    values: BTreeMap<u32, String>,
    flips: BTreeMap<u32, String>,
    /// The local of the object a change is being applied to, while its value
    /// is read per object.
    affected_object: Option<&'static str>,
    /// The bestow cost, which makes the card two definitions.
    bestow: Option<String>,
    /// What a bestow cost asks besides mana, an additional cost of the Aura.
    bestow_extra: Option<String>,
    /// A choice this card makes is a hole: a planned as-event ability ("As this
    /// creature transforms into Shinryu, choose an opponent") or a choice of a
    /// card type the craft materials share. What reads that value follows it.
    as_event_chooses: bool,
    /// Values this body chose, each in a local.
    chosen: Vec<(ValueKind, String)>,
    /// Values the card chooses as it enters, which its permanent keeps.
    enters_choosing: Vec<ValueKind>,
    /// The body of a mana ability is being written.
    mana_body: bool,
    /// The place on its card of the mana ability being written.
    activated_mana: Option<usize>,
    /// The color a definition made once for each color is written for.
    each_color: Option<String>,
    /// It chooses two of this kind as it enters: the second in its chosen number.
    enters_two: Option<ValueKind>,
    /// A rule on the cost of `spell` is being written.
    costed_spell: bool,
    /// A cost paid for a card in place of its mana cost is being written ("{X}
    /// rather than its mana cost, where X is its mana value"): "it" is that card.
    card_costed: bool,
    /// The local each object being moved is, while what it arrives with is read:
    /// "exile a card with a number of time counters on it equal to its mana value".
    moving_one: Option<String>,
    /// The card types it offers, if it chooses one as it enters.
    entering_card_type: Option<Vec<String>>,
    bound_objects: BTreeSet<u32>,
    /// The next closure is a reflexive ability's.
    reflexing: bool,
    /// The next closure is the body of a delayed ability asked of another object.
    source_moved: bool,
    watching_object: Option<ObjectRef>,
    bound_players: BTreeSet<u32>,
    /// A mechanic asks for the first way, so no other way may come before it.
    only_way: bool,
    /// An instruction asks "tap or untap", which the card's `labels` word.
    tap_or_untap: bool,
    /// A spell's body said "paradigm", which sets a field of the card.
    paradigm: bool,
    /// Inside a spell's cost change, which has `targets` in reach.
    costing_targets: bool,
    /// A spell's additional cost read the game for its size, so it is worked out
    /// as the total cost is (`cost_of_targets`, CR 601.2f).
    cost_reads_board: bool,
    /// An intervening "if" that reads the event, for each mode to ask again.
    mode_guard: Option<Condition>,
    /// What the cost being compiled is paid for.
    paying: lane_g::Paying,
    /// The instruction a reflexive ability's body counts: it is the item's `x`.
    counted: Option<u32>,
    /// The count of a reflexive ability's slot, which the ability is told.
    sized_slot: Option<String>,
    /// What a mana ability's definition ends with, and the requirement if any.
    mana_colors: Option<(String, Option<&'static str>)>,
    /// Small generated functions that this module uses.
    helpers: BTreeSet<&'static str>,
    extras: Vec<String>,
    /// Abilities that are no part of the card until a grant or an event names
    /// them: `CardDef::delayed`.
    delayed: Vec<String>,
    /// The filter of each slot the last announcement compiled.
    announced_filters: Vec<String>,
    /// For a slot for each player, the slot of the controller's own that goes with
    /// it (`lane_i::own_slot`), by their places among the slots.
    own_slots: BTreeMap<usize, usize>,
    /// Compiling a delayed ability's body: what it carries from its maker.
    outer: Option<Rc<delayed::Outer>>,
    /// `CardDef::granted_preventions` and `granted_replacements`.
    made_preventions: Vec<String>,
    made_replacements: Vec<String>,
    /// The counter a replacement of counters is about, indexing `counts`:
    /// `None` when it is about every kind.
    replaced_counter: Option<String>,
    /// `CardDef::lasting`: the conditions of "for as long as" durations.
    lasting: Vec<String>,
    /// `CardDef::lasting_block_rules`.
    made_block_rules: Vec<String>,
    /// Instructions that made an effect preventing damage: its amount comes later.
    prevented: BTreeSet<u32>,
    /// A resolving effect is on what its event is about.
    rule_on_event_object: bool,
    /// `CardDef::labels`: the options of a choice as it enters.
    entering_labels: Option<String>,
    /// The mana cost printed on the card.
    printed_mana: Option<Vec<ManaSymbol>>,
    /// What `COUNTED` reads in the slots being written.
    slot_counts: target_counts::SlotCounts,
    /// The one number a triggered ability's `COUNTED` reads: its code and the
    /// builder call that says so.
    count_read: Option<(String, String)>,
    /// The slot whose target says which of its implied modes an ability has,
    /// and each mode's test of it.
    implied_branch: Option<(usize, Vec<String>)>,
    /// Every slot of the trigger being written names who chooses it.
    every_slot_chosen: bool,
    /// The trigger being written says that its subject chooses its targets.
    subject_chooses: bool,
    /// Its slots leave that to the subject, and that is who the event binds.
    controller_subject: bool,
    /// A batch of counters is read only as how many they were.
    counters_total: bool,
    /// `CardDef::lasting_cost_changes`.
    made_cost_changes: Vec<String>,
    /// `CardDef::labels` of abilities to choose among as a body resolves.
    choice_labels: Vec<String>,
    /// Options of a "this or that" are among `choice_labels`.
    option_labels: bool,
    /// The abilities the body's `ChooseValue` offered.
    ability_choice: Vec<Ability>,
    /// "Choose two abilities from among ...": the sets of `ability_choice`
    /// offered, each one of the card's words, by index.
    ability_sets: Vec<Vec<usize>>,
    granting: bool,
    /// What `plan_entry` holds for the granted ability being compiled.
    holding: Option<Vec<(&'static str, String)>>,
    /// How many `granted_rules` this card has.
    granted_rules: usize,
    /// Rules of granted abilities, once `plan_entry` lands them.
    late: Vec<String>,
    /// Entries of the card's own tables that ask whether an object has a
    /// granted ability (Tomik, Orzhov Lawmage's attack limit).
    late_tables: Vec<(&'static str, String)>,
    /// `CardDef::becomes_aura_enchant`.
    aura: Option<String>,
    /// `player` is the player a stated rule is asked of.
    rule_player: bool,
    functions: String,
    next: usize,
    /// Every gap so far. Compilation continues past one so a survey sees them
    /// all, but `module` publishes nothing unless this ends up empty.
    gaps: Vec<Diagnostic>,
    /// Results of instructions that have a gap.
    failed: BTreeSet<u32>,
    /// The process a `RepeatProcess` here repeats: its `again_` flag's number.
    repeating: Option<usize>,
    /// The name of the card being compiled, for what is named after it.
    card_name: String,
    /// What follows the instruction being compiled in this ability, nearest
    /// first: its own program, then the rest of those around it.
    after: Vec<Instruction>,
    /// Whether a search's library was searched, by the search's result.
    searched: BTreeMap<u32, String>,
    /// The spell ends in cipher, so the card has what an encoded card gives.
    cipher: bool,
    /// "As long as": asked as the next ability's event happens.
    ability_guard: Option<Condition>,
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
        other @ (ManaProduction::SameTypeAsProduced
        | ManaProduction::CouldProduce { .. }
        | ManaProduction::CommanderIdentity
        | ManaProduction::ColorsAmong { .. }
        | ManaProduction::ManaCostOf(_)
        | ManaProduction::SymbolsOf(_)
        | ManaProduction::DifferentColors(_)
        | ManaProduction::OtherColor
        | ManaProduction::LostThisWay(_)
        | ManaProduction::DraftNotedColor(_)
        | ManaProduction::LastNoted { .. }) => {
            return Err(gap(format!("mana production {}", form(other))))
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
            event_planned: false,
            recipient_subject: false,
            candidates: Default::default(),
            cast_fact: Default::default(),
            granted_once: Default::default(),
            affected_in_scope: false,
            static_guard: None,
            trigger_gate: None,
            event_object: None,
            present: Default::default(),
            picked: Default::default(),
            shares: BTreeMap::new(),
            paid_for: PaidFor::Ability,
            mana_x: false,
            slots: BTreeMap::new(),
            selections: BTreeMap::new(),
            damage_sources: BTreeMap::new(),
            choosing_source: None,
            notes: Default::default(),
            countered: BTreeMap::new(),
            face_piles: None,
            exiled_piles: false,
            piles: None,
            piles_walk: None,
            pile_shape: (0, false),
            unpaid_rows: BTreeSet::new(),
            turned_up: None,
            unchosen: BTreeMap::new(),
            word_options: Vec::new(),
            player_words: None,
            reveal_matches: BTreeMap::new(),
            revealed: false,
            creations: BTreeMap::new(),
            moved: BTreeMap::new(),
            before: BTreeMap::new(),
            asking: None,
            activation_cost: None,
            casting_costs: BTreeMap::new(),
            spell_cost: false,
            option_cost: None,
            chose_object_as_enters: false,
            cast_pays: Vec::new(),
            enters_sacrifices: false,
            damages_once: false,
            returning: BTreeSet::new(),
            dividing: None,
            hoisted: None,
            unread: Vec::new(),
            chosen_lists: Vec::new(),
            own_choice: std::cell::RefCell::new(None),
            played: None,
            single: BTreeSet::new(),
            known_one: BTreeSet::new(),
            links_exile: false,
            one_graveyard: false,
            own_face: false,
            door: None,
            choosing: None,
            performed: BTreeMap::new(),
            unpaid: BTreeSet::new(),
            hurt_players: BTreeMap::new(),
            coins: None,
            coin_turns: None,
            coin_objects: None,
            excess: BTreeMap::new(),
            values: BTreeMap::new(),
            flips: BTreeMap::new(),
            affected_object: None,
            bestow: None,
            bestow_extra: None,
            as_event_chooses: false,
            chosen: Vec::new(),
            enters_choosing: Vec::new(),
            mana_body: false,
            activated_mana: None,
            each_color: None,
            enters_two: None,
            costed_spell: false,
            card_costed: false,
            moving_one: None,
            entering_card_type: None,
            bound_objects: BTreeSet::new(),
            reflexing: false,
            source_moved: false,
            watching_object: None,
            bound_players: BTreeSet::new(),
            costing_targets: false,
            cost_reads_board: false,
            mode_guard: None,
            paying: lane_g::Paying::Ability,
            counted: None,
            sized_slot: None,
            mana_colors: None,
            only_way: false,
            tap_or_untap: false,
            paradigm: false,
            helpers: BTreeSet::new(),
            extras: Vec::new(),
            delayed: Vec::new(),
            announced_filters: Vec::new(),
            own_slots: BTreeMap::new(),
            outer: None,
            made_preventions: Vec::new(),
            made_replacements: Vec::new(),
            replaced_counter: None,
            lasting: Vec::new(),
            made_block_rules: Vec::new(),
            prevented: BTreeSet::new(),
            rule_on_event_object: false,
            entering_labels: None,
            printed_mana: None,
            slot_counts: Default::default(),
            count_read: None,
            implied_branch: None,
            every_slot_chosen: false,
            subject_chooses: false,
            controller_subject: false,
            counters_total: false,
            made_cost_changes: Vec::new(),
            choice_labels: Vec::new(),
            option_labels: false,
            ability_choice: Vec::new(),
            ability_sets: Vec::new(),
            granting: false,
            holding: None,
            granted_rules: 0,
            late: Vec::new(),
            late_tables: Vec::new(),
            aura: None,
            rule_player: false,
            functions: String::new(),
            next: 0,
            gaps: Vec::new(),
            failed: BTreeSet::new(),
            repeating: None,
            card_name: String::new(),
            after: Vec::new(),
            searched: BTreeMap::new(),
            cipher: false,
            ability_guard: None,
            holes: String::new(),
            printed: Vec::new(),
            chapters: BTreeMap::new(),
            line: None,
            quoted_used: BTreeSet::new(),
            may_choose: false,
            choice_rule: String::new(),
            chosen_guard: None,
            guard_opened: 0,
            second_of_two: None,
            whole_mills: Default::default(),
            spell_targets: false,
            kicker_line: None,
            kickers: Vec::new(),
            sorcery: false,
            entering_as: Default::default(),
            wanted_labels: None,
            stages: BTreeMap::new(),
            stage_lines: Vec::new(),
            do_once: None,
            once_shared: None,
            first_again: None,
            cast_test: false,
            waiting_trigger: false,
            face_text: String::new(),
            may_tap_or_untap: false,
            unless_paid: BTreeSet::new(),
            replacing: Default::default(),
            watching: false,
        }
    }
    /// Compile one part, recording a gap rather than stopping at it, and prefix
    /// `path` to everything it reported. The part's failure comes back so the
    /// caller can leave a tracked hole where its code would go.
    fn attempt(
        &mut self,
        path: String,
        part: impl FnOnce(&mut Self) -> Result<()>,
    ) -> Option<String> {
        let mark = self.gaps.len();
        let mut failure = None;
        if let Err(gap) = part(self) {
            failure = Some(gap.feature.clone());
            self.gaps.push(gap);
        }
        for gap in &mut self.gaps[mark..] {
            *gap = gap.clone().at(&path);
        }
        failure
    }
    fn selected(&self, result: &ResultId) -> Result<String> {
        match self.selections.get(&result.0) {
            Some(local) => Ok(format!("{local}.clone()")),
            None if self.failed_anywhere(result) => Ok(planned::unreported(NO_OBJECTS, result)),
            None => Err(gap("selection result outside generated scope")),
        }
    }
    fn created(&self, result: &ResultId) -> Result<String> {
        match self.creations.get(&result.0) {
            Some(local) => Ok(format!("{local}.clone()")),
            None if self.failed_anywhere(result) => Ok(planned::unreported(NO_OBJECTS, result)),
            None => Err(gap("token result outside generated scope")),
        }
    }
    fn moved(&self, result: &ResultId) -> Result<String> {
        match self.moved.get(&result.0) {
            Some(local) => Ok(format!("{local}.clone()")),
            None if self.failed_anywhere(result) => Ok(planned::unreported(NO_OBJECTS, result)),
            None => Err(gap("transition this instruction does not report")),
        }
    }
    /// What a transition moved, by the new objects or the ones they were.
    fn transition(&self, result: &ResultId, phase: &ObjectPhase) -> Result<String> {
        match (phase, self.before.get(&result.0)) {
            (ObjectPhase::Before, Some(list)) => Ok(list.clone()),
            (ObjectPhase::Before, None) if self.moved.contains_key(&result.0) => {
                Ok(planned::planned_value(
                    "object as it was before an instruction moved it",
                    "Vec::<ObjectId>::new()",
                    &format!("q.as_they_were(&{})", self.moved[&result.0]),
                ))
            }
            // Chosen or made and not moved: nothing happened to them since.
            _ if self.selections.contains_key(&result.0) && !self.moved.contains_key(&result.0) => {
                self.selected(result)
            }
            _ if self.creations.contains_key(&result.0) && !self.moved.contains_key(&result.0) => {
                self.created(result)
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
            // "Target spell or permanent": whatever is in those zones.
            ObjectKind::Any => zones(&domain.zones)?,
            _ => return Err(gap(format!("target object domain {:?}", domain.kind))),
        })
    }
    /// The body of a `TargetFilter`, or a shared target for an unrestricted one.
    fn target_test(&self, spec: &TargetSpec) -> Result<String> {
        let id = Candidate::of("id");
        Ok(match &spec.kind {
            TargetKind::Object(domain) if lane_i::on_the_stack(domain) => {
                self.stack_target_test(&spec.filter)?
            }
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
                    None => "true".into(),
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
            Count::Exactly(n) if lane_l::sizes_slot(&self.sized_slot, n) => ".counted()".into(),
            Count::UpTo(n) if lane_l::sizes_slot(&self.sized_slot, n) => {
                ".between(0, TargetSpec::COUNTED)".into()
            }
            Count::Exactly(n) if constant(n).is_some() => {
                let n = constant(n).unwrap();
                format!(".between({n}, {n})")
            }
            Count::UpTo(n) if constant(n).is_some() => {
                format!(".between(0, {})", constant(n).unwrap())
            }
            Count::AnyNumber => ".between(0, cantrip_api::TargetSpec::ANY_NUMBER)".into(),
            Count::Between { minimum, maximum }
                if constant(minimum).is_some() && constant(maximum).is_some() =>
            {
                format!(
                    ".between({}, {})",
                    constant(minimum).unwrap(),
                    constant(maximum).unwrap()
                )
            }
            // One slot for each opponent, "that player" being whose it is.
            Count::PerPlayer { players, each } if constant_one(each) => {
                if lane_i::seats_have_slots(players) {
                    ".between(0, TargetSpec::EACH_OPPONENT)".into()
                } else {
                    format!(
                        ".between(0, {})",
                        planned::planned_value(
                            "target cardinality PerPlayer",
                            "1u32",
                            "TargetSpec::EACH_PLAYER",
                        )
                    )
                }
            }
            // How many is read as the slot is chosen, and a slot's size is a constant.
            Count::UpTo(n) => {
                let read = self.number(n)?.amount(u32::MAX.into(), "u32")?;
                format!(
                    ".between(0, {})",
                    planned::planned_value(
                        target_counts::up_to_feature(self.slot_counts, self.activation_cost.is_some(), &read),
                        "1u32",
                        &format!("TargetSpec::read_as_chosen(|q: &dyn Query, asker: &Asker| {read})"),
                    )
                )
            }
            other => return self.target_count_rest(other),
        })
    }
    fn announcement(&mut self, a: &Announcement, label: &str) -> Result<String> {
        if a.modes.is_some() {
            return Err(gap("modal announcement"));
        }
        if let ([only], Some(line)) = (a.targets.as_slice(), self.line_text()) {
            let shown = format!("{only:?}");
            let target = line.find("target ").map(|at| &line[at..]);
            let names_you = target
                .and_then(|words| words.split(['.', ',']).next())
                .is_some_and(|words| {
                    words.ends_with(" you control") && words.split(' ').count() <= 5
                });
            if names_you && line.matches("target").count() == 1 && !shown.contains("Control") {
                return Err(gap("a target the source doesn't limit to yours"));
            }
        }
        self.slots.clear();
        let reader = std::mem::replace(&mut self.reader, Reader::Query);
        let compiled = self.announced_targets(a, label);
        self.reader = reader;
        compiled
    }
    /// The set rule of a slot whose targets have one value in common, with the
    /// module's own helpers where the engine names none.
    fn common_property(&mut self, property: &DistinctProperty) -> Result<&'static str> {
        let (helpers, rule): (&[&'static str], _) = match property {
            DistinctProperty::CardTypes => {
                return Ok(".together(cantrip_mechanics::select::SHARE_A_CARD_TYPE)")
            }
            DistinctProperty::Owners => (
                &["common_by", "common_owner"],
                ".together(cantrip_api::select::set_part(&(common_owner as SelectFn), &(largest_common_owner as cantrip_api::select::LargestFn), 0, \"with the same owner\"))",
            ),
            DistinctProperty::Controllers => (
                &["common_by", "common_controller"],
                ".together(cantrip_api::select::set_part(&(common_controller as SelectFn), &(largest_common_controller as cantrip_api::select::LargestFn), 0, \"with the same controller\"))",
            ),
            DistinctProperty::CreatureTypes => (
                &["common_creature_type"],
                ".together(cantrip_api::select::set_part(&(common_creature_type as SelectFn), &(largest_common_creature_type as cantrip_api::select::LargestFn), 0, \"that share a creature type\"))",
            ),
            DistinctProperty::Toughness => (
                &["common_toughness"],
                ".together(cantrip_api::select::set_part(&(common_toughness as SelectFn), &(largest_common_toughness as cantrip_api::select::LargestFn), 0, \"with equal toughness\"))",
            ),
            other => return Err(gap(format!("target constraint CommonProperty {other:?}"))),
        };
        self.helpers.extend(helpers);
        Ok(rule)
    }
    fn announced_targets(&mut self, a: &Announcement, label: &str) -> Result<String> {
        let mut targets = Vec::new();
        let mut own_tests = Vec::new();
        self.announced_filters.clear();
        self.own_slots.clear();
        self.second_of_two = self.two_targets(a);
        // Slots a target of the AST is more than one of, before this one.
        let mut shift = 0;
        for (i, spec) in a.targets.iter().enumerate() {
            let at = format!("targets[{i}]");
            if lane_k::only_your_graveyard(self.line_text(), &spec.filter) {
                return Err(gap("a target from a graveyard, where the card says any").at(at));
            }
            let mut rule = lane_l::SlotRule::default();
            if Self::limit_shared(a) == Some(i) {
                self.slots.insert(spec.slot.0, (0, spec.clone()));
                continue;
            }
            // The second of "two target creatures" is the same slot's.
            if self.second_of_two == Some(i) {
                self.slots.insert(spec.slot.0, (i, spec.clone()));
                continue;
            }
            let i = i + shift;
            if faces::two_from_one_graveyard(i, spec)
                && Self::limit_shared(a).is_none()
                && self.second_of_two.is_none()
            {
                self.choosing = Some(i);
                let test = self.slot_test(spec, &rule).map_err(|e| e.at(&at));
                self.choosing = None;
                let test = test?;
                let (first, second) = (format!("{label}_target_{i}"), format!("{label}_target_{i}_same_graveyard"));
                writeln!(self.functions, "#[allow(unused_variables)] fn {first}(q: &dyn Query, asker: &Asker, t: Target) -> bool {{ {test} }}").unwrap();
                writeln!(self.functions, "#[allow(unused_variables)] fn {second}(q: &dyn Query, asker: &Asker, t: Target) -> bool {{ {} }}", faces::same_graveyard_as_first(&test)).unwrap();
                targets.push(format!("target({first}).optional()"));
                targets.push(format!("target({second}).optional()"));
                self.announced_filters.push(first);
                self.announced_filters.push(second);
                self.slots.insert(spec.slot.0, (i, spec.clone()));
                self.own_slots.insert(i, i + 1);
                shift += 1;
                continue;
            }
            let oracle = target_counts::oracle_slot(&self.card_name, spec);
            let spec = oracle.as_ref().unwrap_or(spec);
            let each_player = target_counts::each_player_slot(spec);
            let spec = each_player.as_ref().unwrap_or(spec);
            let told = self.told_mana_value_slot(spec);
            let spec = told.as_ref().unwrap_or(spec);
            let doubled;
            let spec = if self.second_of_two.is_some() {
                doubled = TargetSpec {
                    count: Count::Exactly(Expr::Constant(2)),
                    ..spec.clone()
                };
                &doubled
            } else {
                spec
            };
            let together = match spec.constraints.as_slice() {
                [] => String::new(),
                [SelectionConstraint::CommonProperty(property)]
                    if self.common_property(property).is_ok() =>
                {
                    self.common_property(property)
                        .map(String::from)
                        .map_err(|e| e.at(&at))?
                }
                constraints => {
                    rule = self.slot_rule(constraints, i).map_err(|e| e.at(&at))?;
                    rule.together.clone()
                }
            };
            if let (Some(total), true) = (&spec.distribute, i > 0) {
                // Cantrip divides only among a first slot's targets.
                let total = self
                    .number(total)
                    .and_then(|n| n.amount(u32::MAX.into(), "u32"))
                    .map_err(|e| e.at(&at))?;
                self.dividing = Some(planned::planned_value(
                    "target with a divided amount",
                    "Divide { total: 1, of_x: None }",
                    &format!("Divide::of_slot({i}, {total})"),
                ));
            } else if let Some(total) = &spec.distribute {
                // "X plus 1 damage divided as you choose" (Meteor Shower): the X
                // announced, through a function of it (`Divide::of_x`, as
                // Shatterskull Smashing's "twice X").
                let of_x = match total {
                    Expr::Constant(_) | Expr::AnnouncedX => None,
                    other if i == 0 && self.slot_counts == Default::default() => {
                        self.total_of_x(other, label)
                    }
                    _ => None,
                };
                let total = match total {
                    _ if of_x.is_some() => "TargetSpec::COUNTED".to_string(),
                    Expr::Constant(n) if i == 0 && *n >= 0 => n.to_string(),
                    Expr::AnnouncedX
                        if i == 0 && self.slot_counts == Default::default() =>
                    {
                        "TargetSpec::COUNTED".to_string()
                    }
                    other if i == 0 && self.counts_own_x(other) => {
                        "TargetSpec::COUNTED".to_string()
                    }
                    Expr::AnnouncedX
                        if i == 0
                            && self.slot_counts == target_counts::SlotCounts::Delayed
                            && self.counted.is_some() =>
                    {
                        "TargetSpec::COUNTED".to_string()
                    }
                    // "When you do, distribute that many": the number the
                    // reflexive ability was given (CR 603.12).
                    Expr::Performed(r) | Expr::CountersRemoved(r) | Expr::DieResult(r)
                        if i == 0
                            && self.slot_counts == target_counts::SlotCounts::Delayed
                            && self.counted == Some(r.0) =>
                    {
                        "TargetSpec::COUNTED".to_string()
                    }
                    Expr::AnnouncedX => {
                        return Err(gap("target with a divided amount of an X it can't read")
                            .at(at))
                    }
                    other if i == 0 => {
                        let total = self
                            .number(other)
                            .and_then(|n| n.amount(u32::MAX.into(), "u32"))
                            .map_err(|e| e.at(&at))?;
                        planned::planned_value(
                            "target with a divided amount read as chosen",
                            "1u32",
                            &format!("Divide::read(|q: &dyn Query, asker: &Asker| {total})"),
                        )
                    }
                    _ => return Err(gap("target with a divided amount").at(at)),
                };
                let of_x = of_x.map_or_else(|| "None".to_string(), |f| format!("Some({f})"));
                self.dividing = Some(format!("Divide {{ total: {total}, of_x: {of_x} }}"));
            }
            self.choosing = Some(i);
            let chosen_by = match &spec.chooser {
                Some(chooser) => self.chosen_by_another(chooser, &mut rule),
                None => Ok(()),
            };
            // "X damage divided as you choose among any number of targets": each target
            // is given at least one of the total (CR 601.2d), so the total holds the
            // slot to it, and the slot itself is of any number (`ask_divided`).
            // A triggered ability's `counted_by` reads such a number itself, where it can.
            let divided_any = matches!(
                (&spec.distribute, &spec.count),
                (Some(total), Count::UpTo(n))
                    if !matches!(n, Expr::Constant(_) | Expr::AnnouncedX)
                        && format!("{total:?}") == format!("{n:?}")
            );
            const ANY: &str = ".between(0, cantrip_api::TargetSpec::ANY_NUMBER)";
            let read = match divided_any {
                true if self.slot_counts == Default::default() => Some(ANY.to_string()),
                true => Some(match self.counted_read(&spec.count, &format!("{label}_{i}")) {
                    Some(count) => count,
                    None => match self.target_count(&spec.count) {
                        Ok(count) if !count.contains("planned_value!") => count,
                        _ => ANY.to_string(),
                    },
                }),
                false => self.slot_up_to_read(spec, i, label).filter(|_| together.is_empty()),
            };
            let count = match read {
                Some(count) => Ok(count),
                None => self.slot_count(&spec.count, &format!("{label}_{i}")),
            };
            self.choosing = None;
            chosen_by.map_err(|e| e.at(&at))?;
            let count = count.map_err(|e| e.at(&at))?;
            let count = lane_k::divided_among_some(
                count,
                spec.distribute.is_some()
                    && lane_k::printed_otherwise(self.line_text(), "any number of target")
                    && !faces::distributed_among_any(self.line_text()),
            );
            self.slots.insert(spec.slot.0, (i, spec.clone()));
            if spec.kind == TargetKind::DamageRecipient
                && spec.players.is_none()
                && matches!(spec.filter, ObjectFilter::Any)
                && rule.test.is_none()
            {
                targets
                    .push(rule.finish(format!("cantrip_mechanics::targets::ANY{count}{together}")));
                self.announced_filters
                    .push("cantrip_mechanics::targets::ANY.filter".into());
                continue;
            }
            self.choosing = Some(i);
            let test = self.slot_test(spec, &rule).map_err(|e| e.at(&at));
            let test = match Self::limit_shared(a) {
                Some(other) => test.and_then(|test| {
                    let more = lane_l::SlotRule::default();
                    let other = self.slot_test(&a.targets[other], &more)?;
                    Ok(format!("({test}) || ({other})"))
                }),
                None => test,
            };
            self.choosing = None;
            let test = test?;
            if let Some(own) = lane_i::own_slot(&spec.count, &test) {
                own_tests.push((i, own));
            }
            let test = lane_i::per_player_target(&spec.count, test);
            let function = format!("{label}_target_{i}");
            writeln!(
                self.functions,
                "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, t: Target) -> bool {{ {test} }}"
            )
            .unwrap();
            targets.push(rule.finish(format!("target({function}){count}{together}")));
            self.announced_filters.push(function);
        }
        for (i, test) in own_tests {
            let function = format!("{label}_own_target_{i}");
            writeln!(
                self.functions,
                "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, t: Target) -> bool {{ {test} }}"
            )
            .unwrap();
            self.own_slots.insert(i, targets.len());
            targets.push(format!("target({function}).between(0, 1)"));
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
        if token.chosen_color {
            return Err(gap("token of the chosen color"));
        }
        if token.chosen_creature_type {
            return Err(gap("token of the chosen creature type"));
        }
        if token.abilities_of.is_some() {
            return Err(gap("token with the abilities of other objects"));
        }
        let sized = self.token_sized(token)?;
        let pt = token
            .power_toughness
            .as_ref()
            .map(|(p, t)| {
                let integer = |n: &Expr| -> Result<i32> {
                    match n {
                        Expr::Constant(n) => {
                            i32::try_from(*n).map_err(|_| gap("token characteristic exceeds i32"))
                        }
                        _ => Ok(0),
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
        let child = emit_face(&card, Some(&token.name), self.quoted_text())?;
        self.gaps
            .extend(child.gaps.iter().map(|g| g.clone().at("token")));
        writeln!(self.functions, "pub mod token_{id} {{\n{}\n}}", child.rust).unwrap();
        let path = format!("token_{id}::CARD");
        self.extras.push(path.clone());
        self.extras
            .extend(child.extras.into_iter().map(|p| format!("token_{id}::{p}")));
        Ok(match sized {
            Some(size) => planned::planned_value(
                "dynamic defining token characteristics",
                &path,
                &format!("{path}.sized_as_created({size})"),
            ),
            None => path,
        })
    }
    fn emit_mana(
        &mut self,
        plan: &ManaPlan,
        players: &PlayerSet,
        restriction: Option<&ManaUse>,
        chosen: Option<&str>,
        out: &mut String,
    ) -> Result<()> {
        if matches!(players, PlayerSet::One(PlayerRef::AbilityController))
            && self
                .line_text()
                .is_some_and(|line| line.contains("that player adds"))
        {
            return Err(gap("mana for that player the source gives to you"));
        }
        let id = self.next;
        self.next += 1;
        let (lasting, restriction) = lane_g::mana_lasting(restriction);
        let restriction = restriction
            .map(|r| self.mana_use(r))
            .transpose()?
            .map(|rule| lane_h::rule_through_words(&rule));
        let add = |player: &str, color: &str, amount: &str| -> String {
            match (&restriction, lasting) {
                (Some(r), _) => format!("g.add_restricted_mana({player}, {color}, {amount}, {r});"),
                (None, Some(until)) => format!("g.add_mana_{until}({player}, {color}, {amount});"),
                (None, None) => format!("g.add_mana({player}, {color}, {amount});"),
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
                    if matches!(color, ManaColor::Chosen) {
                        let chosen = self.fetch(self.chosen_value(ValueKind::Color)?);
                        let adds = add("player", "color.mana()", &format!("mana_{id}_{i}"));
                        writeln!(out, "if let Some(color) = {chosen} {{ {adds} }}").unwrap();
                        continue;
                    }
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
        if let Some(adds) = self.mana_of_a_color_among(body, label)? {
            return Ok(adds);
        }
        if let Some(adds) = self.counted_mana_of_one_color(body, label)? {
            return Ok(adds);
        }
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
            return self.mana_program(body, label);
        };
        if let Some(adds) = self.mana_beyond(mana, restriction.as_ref(), rest, label)? {
            return Ok(adds);
        }
        // What no plan names is said by the instruction, and that is the program.
        if mana_plan(mana).is_err() {
            return self.mana_program(body, label);
        }
        let plan = match mana_plan(mana)? {
            // "{B} or one mana of the chosen color": `colors_from` limits it.
            ManaPlan::SameColor { colors, amount } if colors.contains(&ManaColor::Chosen) => {
                if !self.enters_choosing.contains(&ValueKind::Color) {
                    return Err(gap("mana of a color not chosen as it entered"));
                }
                ManaPlan::SameColor {
                    colors: vec![
                        ManaColor::White,
                        ManaColor::Blue,
                        ManaColor::Black,
                        ManaColor::Red,
                        ManaColor::Green,
                    ],
                    amount,
                }
            }
            // One color the engine limits it to (`colors_from`) as it is offered.
            ManaPlan::Fixed(units) if matches!(units.as_slice(), [(ManaColor::Chosen, _)]) => {
                if !self.enters_choosing.contains(&ValueKind::Color) {
                    return Err(gap("mana of a color not chosen as it entered"));
                }
                let [(_, amount)] = <[_; 1]>::try_from(units).unwrap();
                ManaPlan::SameColor {
                    colors: vec![
                        ManaColor::White,
                        ManaColor::Blue,
                        ManaColor::Black,
                        ManaColor::Red,
                        ManaColor::Green,
                    ],
                    amount,
                }
            }
            plan => plan,
        };
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
        if let (_, Some(r)) = lane_g::mana_lasting(restriction.as_ref()) {
            let code = self.mana_use(r)?;
            // The engine reads a restriction's words as the mana is added only
            // for mana it adds itself, so such mana is listed.
            if let Some(listed) = lane_k::listed_mana(&plan, rest, restriction.as_ref()) {
                return Ok(format!("{listed}.only_for({code})"));
            }
            metadata.push_str(&format!(".only_for({code})"));
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
        self.chosen.clear();
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

/// What `card_files` found for one record.
#[derive(serde::Serialize)]
pub struct CardFileRow {
    pub line: usize,
    pub name: String,
    /// The file, when the card has one; `None` with no gap needs only content.
    pub rust: Option<String>,
    pub gap: Option<String>,
}

/// Each face's printed text, by name.
fn oracle_texts(oracle: &Path) -> corpus::Result<BTreeMap<String, String>> {
    let printed: serde_json::Value =
        serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(oracle)?))?;
    Ok(printed
        .as_array()
        .ok_or("oracle data is not a list")?
        .iter()
        .flat_map(|card| card["cards"].as_array().into_iter().flatten())
        .filter_map(|face| {
            let text = face["text"].as_str().unwrap_or_default();
            Some((face["name"].as_str()?.to_string(), text.to_string()))
        })
        .collect())
}

/// Every single-faced, complete, primitive-free record the recipes of
/// `card_file` cover, or the gap that stops it, as one JSON object a line.
pub fn card_files(input: &Path, oracle: &Path, output: &Path) -> corpus::Result<usize> {
    let oracle = oracle_texts(oracle)?;
    let mut artifact = corpus::Artifact::create(output)?;
    let mut count = 0;
    corpus::map_cards(
        input,
        |line, source| -> corpus::Result<Option<CardFileRow>> {
            let identity = corpus::Identity::of(&source)?;
            let lowered = crate::unified::lower(&source);
            let forms = crate::unified::forms(&lowered.card);
            if !(lowered.diagnostics.is_empty() && forms.named.is_empty())
                || !forms.primitives.is_empty()
                || lowered.card.layout != "Card"
                || lowered.card.faces.len() != 1
            {
                return Ok(None);
            }
            let name = identity.names.join(" // ");
            let face = &lowered.card.faces[0];
            if !gaps(face).is_empty() {
                return Ok(None);
            }
            let Some(text) = oracle.get(&name) else {
                return Ok(None);
            };
            let (rust, gap) = match card_file::card_file(face, text) {
                Ok(rust) => (rust, None),
                Err(recipe) => match lane_f::whole_printed(&source, &lowered.card, Some(&oracle)) {
                    Ok(whole) if whole.faces.iter().all(|f| f.gaps.is_empty()) => {
                        let module: String = whole.faces.iter().map(|f| f.rust.as_str()).collect();
                        let characteristics = pr_form::Characteristics::Oracle;
                        (pr_form::render(&module, &name, text, characteristics), None)
                    }
                    _ => (None, Some(recipe.feature)),
                },
            };
            Ok(Some(CardFileRow {
                line,
                name,
                rust,
                gap,
            }))
        },
        |_, row| {
            if let Some(row) = row {
                writeln!(artifact, "{}", serde_json::to_string(&row)?)?;
                count += 1;
            }
            Ok(())
        },
    )?;
    artifact.commit()?;
    Ok(count)
}

/// The module of each named card in a source corpus, for reading what the
/// generator emits; a card with a gap shows the gaps instead.
pub fn modules_named(input: &Path, names: &[String]) -> corpus::Result<Vec<(String, String)>> {
    let wanted: BTreeSet<&str> = names.iter().map(String::as_str).collect();
    let mut found = Vec::new();
    corpus::map_cards(
        input,
        |_, source| -> corpus::Result<Option<(String, String)>> {
            let identity = corpus::Identity::of(&source)?;
            let Some(name) = identity
                .names
                .iter()
                .find(|n| wanted.contains(n.as_str()))
                .cloned()
            else {
                return Ok(None);
            };
            let lowered = crate::unified::lower(&source);
            let mut rendered = String::new();
            if std::env::var_os("MTGISH_SHOW_AST").is_some() {
                for face in &lowered.card.faces {
                    writeln!(rendered, "{face:#?}").unwrap();
                }
            }
            match whole_module(&source, &lowered.card) {
                Ok(rust) => rendered.push_str(&rust),
                Err(_) => {
                    let gaps = whole_gaps(&source, &lowered.card);
                    write!(rendered, "// gaps: {gaps:?}").unwrap()
                }
            }
            Ok(Some((name, rendered)))
        },
        |_, module| {
            found.extend(module);
            Ok(())
        },
    )?;
    Ok(found)
}

/// `modules_named` with each card's printed words to read, holes and all, as the
/// tracked generation has them.
pub fn modules_printed(
    input: &Path,
    oracle: &Path,
    names: &[String],
) -> corpus::Result<Vec<(String, String)>> {
    let oracle = oracle_texts(oracle)?;
    let wanted: BTreeSet<&str> = names.iter().map(String::as_str).collect();
    let mut found = Vec::new();
    corpus::map_cards(
        input,
        |_, source| -> corpus::Result<Option<(String, String)>> {
            let identity = corpus::Identity::of(&source)?;
            let Some(name) = identity
                .names
                .iter()
                .find(|n| wanted.contains(n.as_str()))
                .cloned()
            else {
                return Ok(None);
            };
            let lowered = crate::unified::lower(&source);
            let rendered = match lane_f::whole_printed(&source, &lowered.card, Some(&oracle)) {
                Ok(whole) => whole.faces.iter().map(|f| f.rust.as_str()).collect(),
                Err(_) => String::new(),
            };
            Ok(Some((name, rendered)))
        },
        |_, module| {
            found.extend(module);
            Ok(())
        },
    )?;
    Ok(found)
}

/// One complete card module of one face, whatever else its record holds. Never
/// returns partially executable code. `whole_module` is a record's whole file.
pub fn module(card: &Card) -> Result<String> {
    let mut module = emit_module(card, None)?;
    if module.gaps.is_empty() {
        Ok(module.rust)
    } else {
        Err(module.gaps.swap_remove(0))
    }
}
/// Everything the generator lacks for this face, not only the first gap.
/// `whole_gaps` adds what its card lacks as a card.
pub fn gaps(card: &Card) -> Vec<Diagnostic> {
    match emit_module(card, None) {
        Ok(module) => module.gaps,
        Err(gap) => vec![gap],
    }
}
/// Every target of one slot, in the order chosen: all of them, for reading what
/// was announced, or the ones still legal, for acting on them (CR 608.2b).
const ANNOUNCED: &str = "fn announced(g: &dyn Game, item: &StackItem, spec: u8, legal: bool) -> Vec<Target> { (0..g.state().targets(item).len()).filter(|&i| g.state().spec(item, i) == spec && (!legal || g.target_legal(item, i))).map(|i| g.state().target(item, i)).collect() }\n";
/// Spliced on, its slots follow the spell's own and the cards spliced before it (702.47c).
const ANNOUNCED_SPLICED: &str = "fn announced(g: &dyn Game, item: &StackItem, spec: u8, legal: bool) -> Vec<Target> { let me = g.card_id(&CARD); let spell = g.obj(item.object).card; let mut before = 0; if spell != me { before += g.card(spell).spell.map_or(0, |s| s.targets.len()); let members = g.state().members(item.members); for &o in &members[..(item.data[0] as usize).min(members.len())] { let card = g.obj(o).card; if card == me { break; } before += g.card(card).spell.map_or(0, |s| s.targets.len()); } } let spec = spec + before as u8; (0..g.state().targets(item).len()).filter(|&i| g.state().spec(item, i) == spec && (!legal || g.target_legal(item, i))).map(|i| g.state().target(item, i)).collect() }\n";

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
    /// The `PayWith` fields a card's keywords set.
    pay_with: BTreeSet<&'static str>,
}
impl Parts {
    /// Add to any list of the card definition, by its field name.
    fn entry(&mut self, list: &'static str, entry: String) {
        match list {
            "keywords" => self.keywords.push(entry),
            "abilities" => self.abilities.push(entry),
            "statics" => self.statics.push(entry),
            "replacements" => self.replacements.push(entry),
            "preventions" => self.preventions.push(entry),
            "block_rules" => self.block_rules.push(entry),
            "block_requirements" => self.block_requirements.push(entry),
            "attack_rules" => self.attack_rules.push(entry),
            "untap_bans" => self.untap_bans.push(entry),
            "enters" => self.enters.push(entry),
            table => self.table(table, entry),
        }
    }
    fn table(&mut self, table: &'static str, entry: String) {
        self.tables.entry(table).or_default().push(entry);
    }
    fn field(&mut self, field: &'static str, value: String) {
        self.fields.insert(field, value);
    }
    /// Nothing but `abilities` was added.
    fn only_abilities(&self) -> bool {
        self.keywords.is_empty()
            && self.statics.is_empty()
            && self.replacements.is_empty()
            && self.preventions.is_empty()
            && self.enters.is_empty()
            && self.block_rules.is_empty()
            && self.block_requirements.is_empty()
            && self.attack_rules.is_empty()
            && self.untap_bans.is_empty()
            && self.tables.is_empty()
            && self.fields.is_empty()
    }
    /// A definition holding only what was added: `enters` is its compiled field.
    fn card(&self, enters: &str) -> String {
        format!(
            "CardDef {{ keywords: Keywords::of(&[{}]), abilities: &[{}], statics: &[{}], replacements: &[{}], preventions: &[{}], block_rules: &[{}], block_requirements: &[{}], attack_rules: &[{}], untap_bans: &[{}], enters: {enters}, {}..CardDef::BLANK }}",
            self.keywords.join(","),
            self.abilities.join(","),
            self.statics.join(","),
            self.replacements.join(","),
            self.preventions.join(","),
            self.block_rules.join(","),
            self.block_requirements.join(","),
            self.attack_rules.join(","),
            self.untap_bans.join(","),
            self.rest()
        )
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
            pay_with: BTreeSet::new(),
        }
    }
}

struct EmittedModule {
    rust: String,
    extras: Vec<String>,
    gaps: Vec<Diagnostic>,
    /// What this face has that a Room's door may not.
    door_refused: Option<&'static str>,
}
/// How an object was cast: it as a spell, or the spell it resolved from. A
/// spell's own `cast_from` isn't carried onto the permanent. A matcher sees the
/// history before its own event, so `cast_at` reads that entry from the event.
const CAST_OF: &str = "fn cast_of<'a>(q: &'a dyn Query, id: ObjectId) -> Option<&'a Cast> { let records = q.history().all(); let spell = records.iter().rev().find_map(|r| match r.event { Event::ZoneChange { old, new, reason: ZoneReason::Resolve, .. } if new == id => Some(old), _ => None }).unwrap_or(id); records.iter().rev().find_map(|r| match &r.event { Event::Cast(cast) if cast.spell == spell => Some(cast), _ => None }) }\n#[allow(dead_code)] fn cast_at<'a>(q: &'a dyn Query, event: &Event, id: ObjectId) -> Option<&'a Cast> { match event { Event::ZoneChange { old, new, reason: ZoneReason::Resolve, .. } if *new == id => cast_of(q, *old), _ => cast_of(q, id) } }\n";

/// CR 700.9: counters, an Equipment, or an Aura its controller controls.
const MODIFIED: &str = "fn modified(q: &dyn Query, id: ObjectId) -> bool { q.state().obj(id).has_counters() || q.state().battlefield().iter().any(|&a| q.state().obj(a).attached_to == Some(id) && { let chars = q.chars(a); chars.has_subtype(Subtype::Equipment) || chars.has_subtype(Subtype::Aura) && q.controller(a) == q.controller(id) }) }\n";

/// A set rule of one player every object has: the first chosen says which.
const COMMON_BY: &str = "fn common_by(q: &dyn Query, choice: &Choice, key: fn(&dyn Query, ObjectId) -> PlayerId) -> Verdict { let first = choice.selected.first().map(|&c| key(q, c)); Verdict { complete: choice.selected.iter().all(|&c| Some(key(q, c)) == first), addable: Bits::of(choice.candidates.len(), |i| first.map_or(true, |k| key(q, choice.candidates[i]) == k)) } }\nfn largest_by(q: &dyn Query, choice: &Choice, key: fn(&dyn Query, ObjectId) -> PlayerId) -> Vec<ObjectId> { choice.candidates.iter().map(|&a| choice.candidates.iter().copied().filter(|&b| key(q, b) == key(q, a)).collect::<Vec<_>>()).max_by_key(Vec::len).unwrap_or_default() }\n";
const COMMON_OWNER: &str = "fn owner_of(q: &dyn Query, id: ObjectId) -> PlayerId { q.state().obj(id).owner }\nfn common_owner(q: &dyn Query, choice: &Choice, _: u32) -> Verdict { common_by(q, choice, owner_of) }\nfn largest_common_owner(q: &dyn Query, choice: &Choice, _: u32) -> Vec<ObjectId> { largest_by(q, choice, owner_of) }\n";
const COMMON_CONTROLLER: &str = "fn controller_of(q: &dyn Query, id: ObjectId) -> PlayerId { q.controller(id) }\nfn common_controller(q: &dyn Query, choice: &Choice, _: u32) -> Verdict { common_by(q, choice, controller_of) }\nfn largest_common_controller(q: &dyn Query, choice: &Choice, _: u32) -> Vec<ObjectId> { largest_by(q, choice, controller_of) }\n";
const COMMON_TOUGHNESS: &str = "fn toughness_of(q: &dyn Query, id: ObjectId) -> i16 { q.chars(id).toughness.unwrap_or(0) }\nfn common_toughness(q: &dyn Query, choice: &Choice, _: u32) -> Verdict { let first = choice.selected.first().map(|&c| toughness_of(q, c)); Verdict { complete: choice.selected.iter().all(|&c| Some(toughness_of(q, c)) == first), addable: Bits::of(choice.candidates.len(), |i| first.map_or(true, |k| toughness_of(q, choice.candidates[i]) == k)) } }\nfn largest_common_toughness(q: &dyn Query, choice: &Choice, _: u32) -> Vec<ObjectId> { choice.candidates.iter().map(|&a| choice.candidates.iter().copied().filter(|&b| toughness_of(q, b) == toughness_of(q, a)).collect::<Vec<_>>()).max_by_key(Vec::len).unwrap_or_default() }\n";
const NO_COMMON_CREATURE_TYPE: &str = "fn creature_types_of(q: &dyn Query, id: ObjectId) -> SubtypeSet { SubtypeSet::CREATURE_TYPES.minus(SubtypeSet::CREATURE_TYPES.minus(q.chars(id).subtypes)) }\nfn apart_in_type(q: &dyn Query, a: ObjectId, b: ObjectId) -> bool { !creature_types_of(q, a).intersects(creature_types_of(q, b)) }\nfn no_common_creature_type(q: &dyn Query, choice: &Choice, _: u32) -> Verdict { let selected = choice.selected; Verdict { complete: selected.iter().enumerate().all(|(i, &a)| selected[..i].iter().all(|&b| apart_in_type(q, a, b))), addable: Bits::of(choice.candidates.len(), |i| selected.iter().all(|&b| apart_in_type(q, choice.candidates[i], b))) } }\nfn grow_apart_in_type(q: &dyn Query, left: &[ObjectId], have: &mut Vec<ObjectId>, best: &mut Vec<ObjectId>) { if have.len() > best.len() { *best = have.clone(); } if have.len() + left.len() <= best.len() { return; } for (i, &c) in left.iter().enumerate() { if have.iter().all(|&o| apart_in_type(q, o, c)) { have.push(c); grow_apart_in_type(q, &left[i + 1..], have, best); have.pop(); } } }\nfn largest_no_common_creature_type(q: &dyn Query, choice: &Choice, _: u32) -> Vec<ObjectId> { let mut best = Vec::new(); grow_apart_in_type(q, choice.candidates, &mut Vec::new(), &mut best); best }\n";
const COMMON_CREATURE_TYPE: &str = "fn common_creature_type(q: &dyn Query, choice: &Choice, _: u32) -> Verdict { let common = choice.selected.iter().fold(SubtypeSet::CREATURE_TYPES, |common, &c| common.minus(common.minus(q.chars(c).subtypes))); Verdict { complete: choice.selected.is_empty() || !common.is_empty(), addable: Bits::of(choice.candidates.len(), |i| common.intersects(q.chars(choice.candidates[i]).subtypes)) } }\nfn largest_common_creature_type(q: &dyn Query, choice: &Choice, _: u32) -> Vec<ObjectId> { SubtypeSet::CREATURE_TYPES.iter().map(|t| choice.candidates.iter().copied().filter(|&c| q.chars(c).subtypes.has(t)).collect::<Vec<_>>()).max_by_key(Vec::len).unwrap_or_default() }\n";

fn emit_module(card: &Card, token_name: Option<&TokenName>) -> Result<EmittedModule> {
    emit_face(card, token_name, lane_f::Place::default())
}
/// A card's module, as `place` puts it among the faces of its card.
fn emit_face(
    card: &Card,
    token_name: Option<&TokenName>,
    place: lane_f::Place,
) -> Result<EmittedModule> {
    validate::card(card).map_err(|e| Diagnostic {
        path: "card".into(),
        feature: "invalid AST".into(),
        message: e,
    })?;
    let joined = lane_m::spells_joined(card);
    let card = joined.as_ref().unwrap_or(card);
    let mut c = Compiler::new();
    let mut parts = Parts::default();
    c.card_name = card.name.clone();
    c.read_printed(&card.abilities, place.printed.as_deref());
    if place
        .printed
        .as_deref()
        .is_some_and(lane_k::reads_graveyard_order)
    {
        parts.field("reads_graveyard_order", "true".into());
    }
    let abilities = format!("{:?}", card.abilities);
    // "Exiled with this" (CR 607.2a), and "they exiled with this" (Uba Mask).
    c.links_exile = place.links_exile
        || abilities.contains("ExiledWith(Source)")
        || abilities.contains("with: Some(Source)");
    c.one_graveyard = abilities.contains("InZone(Players { zone: Graveyard, players: One(");
    c.own_face = place.own_face;
    c.door = place.door;
    c.printed_mana = card.characteristics.mana_cost.clone();
    c.sorcery = (card.characteristics.card_types.iter())
        .any(|t| matches!(t, crate::mtg_types::CardType::Sorcery));
    c.enters_choosing = chosen::enters_choosing(card);
    c.enters_choosing.extend(place.keeps.iter().copied());
    c.enters_two = chosen::enters_two(card);
    c.entering_card_type = lane_p::entering_card_type(card);
    c.cast_pays = lane_p::cast_pays(card);
    c.enters_sacrifices = lane_p::enters_sacrifices(card);
    c.damages_once = lane_p::damages_once(card);
    for (i, a) in card.abilities.iter().enumerate() {
        if let Ability::Casting(CastingRule::AdditionalCost { cost, .. }) = a {
            c.casting_costs.insert(i as u32, cost.clone());
        }
    }
    let mut firsts: Vec<String> = card
        .abilities
        .iter()
        .filter_map(|a| match a {
            Ability::Mechanic { mechanic, .. } => mechanics::first_delayed(*mechanic),
            _ => None,
        })
        .collect();
    firsts.extend(
        place
            .earthbend
            .then(|| keywords::EARTHBEND_RETURN.to_string()),
    );
    // "Myriad, myriad" (Scurry of Squirrels): each trigger makes its own delayed trigger with its own tokens, both from
    // the one first `delayed` ability, so a mechanic printed twice asks for that slot once.
    firsts.sort();
    firsts.dedup();
    c.attempt("abilities".into(), |c| match firsts.as_slice() {
        [] => Ok(()),
        [first] => {
            c.delayed.push(first.clone());
            Ok(())
        }
        _ => Err(gap("two mechanics that need the first delayed ability")),
    });
    for (i, a) in card.abilities.iter().enumerate() {
        c.on_ability(i);
        let failure = c.attempt(format!("abilities[{i}]"), |c| {
            let compiled = c.ability(a, &format!("ability_{i}"), &mut parts, None);
            if let Some(divide) = c.dividing.take() {
                // Only spells and triggered abilities divide yet.
                if compiled.is_err() || !matches!(a, Ability::Activated { .. }) {
                    return Err(gap("target with a divided amount"));
                }
                let Some(def) = parts.abilities.pop() else {
                    return Err(gap("target with a divided amount"));
                };
                c.plan_entry(
                    "target with a divided amount",
                    "abilities",
                    &format!("{def}.divide({divide})"),
                    &mut parts,
                );
            }
            compiled
        });
        if let Some(feature) = failure {
            c.reset_after_gap();
            c.plan_ability(&feature, a, &format!("ability_{i}"));
        }
    }
    if let Some(fact) = c.cast_fact.borrow_mut().take() {
        writeln!(c.functions, "#[allow(unused_variables)] fn kept_as_cast(q: &dyn Query, asker: &Asker) -> u32 {{ ({fact}).max(0) as u32 }}").unwrap();
        parts.field("cast_fact", "Some(kept_as_cast)".into());
    }
    if let Some(body) = c.granted_once.borrow_mut().take() {
        writeln!(c.functions, "#[allow(unused_variables)] fn granted_once(q: &dyn Query, source: ObjectId, ability: AbilityRef) -> usize {{ {body} }}").unwrap();
        parts.field("granted_once_each_turn", "Some(granted_once)".into());
    }
    // A cast announces one number, a kicker's when there is one: {X} stays unannounced.
    if let (Some(kicker), Some(cost)) = (parts.fields.get("kicker"), &c.printed_mana) {
        if cost.iter().any(|symbol| matches!(symbol, ManaSymbol::X)) {
            let feature = gap("engine: X in a mana cost beside a kicker");
            c.plan_trait("kicker", feature, kicker.clone());
        }
    }
    for (table, entry) in std::mem::take(&mut c.late_tables) {
        parts.table(table, entry);
    }
    parts
        .tables
        .entry("granted_rules")
        .or_default()
        .append(&mut c.late);
    if parts.tables.get("granted_rules").is_some_and(Vec::is_empty) {
        parts.tables.remove("granted_rules");
    }
    if let Some(aura) = c.aura.take() {
        parts.field("becomes_aura_enchant", aura);
    }
    c.attempt("abilities".into(), |c| {
        let ways = parts.tables.get("ways").map_or(0, Vec::len);
        match c.only_way && ways != 1 {
            true => Err(gap("another way beside a mechanic's only way")),
            false => Ok(()),
        }
    });
    if c.cipher {
        parts.entry("statics", "cantrip_mechanics::cipher::STATIC".into());
    }
    if let Some(labels) = c.wanted_labels.take() {
        let taken = parts.fields.contains_key("labels");
        c.attempt("abilities".into(), |_| match taken {
            true => Err(gap("a question beside other labels")),
            false => Ok(()),
        });
        parts.field("labels", labels);
    }
    if c.tap_or_untap {
        let taken = parts.fields.contains_key("labels");
        c.attempt("abilities".into(), |_| match taken {
            true => Err(gap("tap or untap beside other labels")),
            false => Ok(()),
        });
        parts.field("labels", "cantrip_mechanics::choose::TAP_OR_UNTAP".into());
    }
    if !parts.pay_with.is_empty() {
        let fields: Vec<String> = parts
            .pay_with
            .iter()
            .map(|f| format!("{f}: true"))
            .collect();
        parts.field(
            "pay_with",
            format!("PayWith {{ {}, ..PayWith::NONE }}", fields.join(", ")),
        );
    }
    let ch = &card.characteristics;
    let mut types = Vec::new();
    for t in &ch.card_types {
        match card_type(t) {
            Ok(n) => types.push(format!("CardType::{n}")),
            Err(gap) => types.extend(lane_p::printed_type(&gap.feature, &name(t))),
        }
    }
    let mut subtypes = Vec::new();
    for t in &ch.subtypes {
        let n = name(t);
        if n == "TimeLord" {
            // Cantrip's oracle build splits a type line at spaces, and fails on "Time".
            subtypes.push(planned::planned_value(
                "engine: a two-word subtype in a card's type line",
                "Subtype::TimeLord",
                "Subtype::TimeLord",
            ));
        } else if crate::cantrip_vocab::SUBTYPES.contains(&n.as_str()) {
            subtypes.push(format!("Subtype::{n}"));
        } else {
            subtypes.push(lane_p::printed_subtype(&n));
        }
    }
    let mut super_flags = [false; 3];
    let mut other_supers = Vec::new();
    for t in &ch.supertypes {
        match name(t).as_str() {
            "Basic" => super_flags[0] = true,
            "Legendary" => super_flags[1] = true,
            "Snow" => super_flags[2] = true,
            other => other_supers.push(other.to_string()),
        }
    }
    let mana = match ch.mana_cost.as_ref().map(|m| mana(m)).transpose() {
        Ok(cost) => cost.unwrap_or_else(|| "ManaCost::ZERO".into()),
        Err(gap) => {
            c.plan_trait(
                "characteristics.mana_cost",
                gap,
                format!("{:?}", ch.mana_cost),
            );
            "ManaCost::ZERO".into()
        }
    };
    let i16_opt = |c: &mut Compiler, n: Option<i64>| -> String {
        match n {
            None => "None".into(),
            Some(n) if i16::try_from(n).is_ok() => format!("Some({n})"),
            Some(n) => {
                let gap = gap("printed characteristic exceeds i16");
                c.plan_trait("characteristics", gap, n.to_string());
                "None".into()
            }
        }
    };
    if let Some((rule, function)) = faces::mana_told_apart(&c.functions) {
        c.functions.push_str(function);
        parts.table("mana_told_apart", rule.to_string());
    }
    // Filters are compiled through `&self`, so this one is found in their text.
    if c.functions.contains("cast_of(q, ") || c.functions.contains("cast_at(q, ") {
        c.helpers.insert("cast_of");
    }
    if c.functions.contains("modified(q, ") {
        c.helpers.insert("modified");
    }
    if c.functions.contains("about(q, ") {
        c.helpers.insert("about");
    }
    if c.functions.contains("granted_by(q, asker)") {
        c.helpers.insert("granted_by");
    }
    if c.functions.contains("granter(q, item)") {
        c.helpers.insert("granter");
    }
    if c.functions.contains("could_enchant(q, ") {
        c.helpers.insert("could_enchant");
    }
    if c.functions.contains("time_travel_for(g, player") {
        c.helpers.insert("time_travel");
    }
    for helper in &c.helpers {
        c.functions.push_str(match *helper {
            "cast_of" => CAST_OF,
            "modified" => MODIFIED,
            "common_by" => COMMON_BY,
            "common_owner" => COMMON_OWNER,
            "common_controller" => COMMON_CONTROLLER,
            "common_creature_type" => COMMON_CREATURE_TYPE,
            "common_toughness" => COMMON_TOUGHNESS,
            "no_common_creature_type" => NO_COMMON_CREATURE_TYPE,
            "announced" if parts.fields.contains_key("splice") => ANNOUNCED_SPLICED,
            "announced" => ANNOUNCED,
            "slot_held" => target_counts::SLOT_HELD,
            "pay_question" => PAY_QUESTION,
            "most_assigned" => lane_l::MOST_ASSIGNED,
            "about" => delayed::ABOUT,
            "clauses" => lane_l::CLAUSES,
            "granter" => lane_j::GRANTER,
            "granted_by" => lane_j::GRANTED_BY,
            "could_enchant" => lane_j::COULD_ENCHANT,
            "time_travel" => lane_j::TIME_TRAVEL,
            other => unreachable!("no helper {other}"),
        });
    }
    for made in std::mem::take(&mut c.made_preventions) {
        parts.table("granted_preventions", made);
    }
    for made in std::mem::take(&mut c.made_replacements) {
        parts.table("granted_replacements", made);
    }
    for made in std::mem::take(&mut c.lasting) {
        parts.table("lasting", made);
    }
    for made in std::mem::take(&mut c.made_block_rules) {
        parts.table("lasting_block_rules", made);
    }
    for made in std::mem::take(&mut c.made_cost_changes) {
        parts.table("lasting_cost_changes", made);
    }
    if let Some(labels) = c.entering_labels.take() {
        let taken = c.option_labels && parts.fields.contains_key("labels");
        c.attempt("abilities".into(), |_| match taken {
            true => Err(gap("a choice's words beside other labels")),
            false => Ok(()),
        });
        parts.field("labels", labels);
    }
    // "Loyalty X" (Nissa, Steward of Elements): it enters with the X its spell was cast
    // with in loyalty counters, and with none put onto the battlefield uncast (CR
    // 306.5b, 107.3m), as an artifact enters with X charge counters (Chalice of the Void).
    let loyalty_x = matches!(ch.loyalty, Some(Expr::AnnouncedX))
        && ch.mana_cost.iter().flatten().any(|s| matches!(s, ManaSymbol::X));
    if loyalty_x {
        writeln!(c.functions, "fn enters_with_x_loyalty(_: &mut dyn Game, entering: &mut Entering) -> Result<(), Suspend> {{ entering.counters[CounterKind::Loyalty as usize] += entering.x as i16; Ok(()) }}").unwrap();
        parts.enters.push("enters_with_x_loyalty".into());
    }
    let door_refused = parts.door_refused();
    let rest = parts.rest() + &place.fields;
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
    let pt = ch.power_toughness;
    let loyalty = match &ch.loyalty {
        Some(Expr::Constant(n)) => Some(*n),
        Some(_) if loyalty_x => None,
        Some(other) => {
            c.plan_loyalty(other);
            None
        }
        None => None,
    };
    let mut colors = 0u8;
    for cl in &ch.color_indicator {
        match color(cl).and_then(|_| color_index(cl)) {
            Ok(index) => colors |= 1 << index,
            Err(gap) => c.plan_trait("characteristics.color_indicator", gap, name(cl)),
        }
    }
    let colors = if ch.color_indicator.is_empty() {
        "None".into()
    } else {
        format!("Some(ColorSet({colors}))")
    };
    let fingerprint =
        corpus::digest(&serde_json::to_vec(&(card, token_name)).expect("serializable AST"));
    let oracle = token_name.is_none() && !place.no_card;
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
    let printed = [
        i16_opt(&mut c, pt.map(|(p, _)| i64::from(p))),
        i16_opt(&mut c, pt.map(|(_, t)| i64::from(t))),
        i16_opt(&mut c, loyalty),
        i16_opt(&mut c, ch.defense.map(i64::from)),
    ];
    let holes = std::mem::take(&mut c.holes);
    c.functions.push_str(&holes);
    c.functions.push_str(&place.holes);
    c.extras.extend(place.extras);
    let mut listed = BTreeSet::new();
    c.extras.retain(|path| listed.insert(path.clone()));
    let bestow = c.bestow.take();
    let bestowed = match c.bestow_extra.take() {
        Some(extra) => format!("CardDef {{ additional_cost: Some({extra}), ..FACE }}"),
        None => "FACE".to_string(),
    };
    if bestow.is_some() {
        c.extras.push("BESTOWED".into());
    }
    // What a card brings along is listed by the file's own `CARD`.
    let extra = match place.own_face {
        true => String::new(),
        false => format!(
            "pub static EXTRA: &[&CardDef] = &[{}];\n",
            c.extras
                .iter()
                .map(|p| format!("&{p}"))
                .collect::<Vec<_>>()
                .join(",")
        ),
    };
    let supertypes = lane_p::printed_supertypes(super_flags, &other_supers);
    let rust = format!(
        "// Generated by mtgish-ast, schema {}, normalized SHA256 {fingerprint}.\n// Generator SHA256 {generator}; Cantrip API SHA256 {api}.\n#[allow(unused_imports)] use cantrip_api::*;\n{}\npub static CARD: CardDef = CardDef {{ name: {}, token_words: {token_words}, oracle: {oracle}, cost: {mana}, has_cost: {}, types: TypeSet::of(&[{}]), subtypes: SubtypeSet::of(&[{}]), supertypes: {supertypes}, colors: {colors}, power: {}, toughness: {}, loyalty: {}, defense: {}, keywords: Keywords::of(&[{}]), protection: ColorSet({protection}), spell: {spell}, abilities: &[{}], statics: &[{}], replacements: &[{}], preventions: &[{}], block_rules: &[{}], block_requirements: &[{}], attack_rules: &[{}], untap_bans: &[{}], delayed: &[{}], enters: {enters}, modal: {modal}, enchant: {enchant}, {rest}..CardDef::BLANK }};\n{extra}",
        SCHEMA_VERSION,
        lane_k::without_unread_data(&c.functions),
        name,
        ch.mana_cost.is_some(),
        types.join(","),
        subtypes.join(","),
        printed[0],
        printed[1],
        printed[2],
        printed[3],
        keywords.join(","),
        abilities.join(","),
        statics.join(","),
        replacements.join(","),
        preventions.join(","),
        block_rules.join(","),
        block_requirements.join(","),
        attack_rules.join(","),
        untap_bans.join(","),
        c.delayed.join(",")
    );
    // The creature and the Aura it is while bestowed have one text (CR 702.103b).
    let rust = match bestow {
        Some(cost) => {
            // Token modules come first, so the card's own definition is the last one.
            let own = "pub static CARD: CardDef = CardDef {";
            let at = rust.rfind(own).expect("a module defines its card");
            format!("{}const FACE: CardDef = CardDef {{{}", &rust[..at], &rust[at + own.len()..])
                + &format!("\npub static BESTOWED: CardDef = cantrip_mechanics::bestow::bestowed({bestowed});\npub static CARD: CardDef = CardDef {{ bestow: Some(&BESTOWED), bestow_cost: {cost}, ..FACE }};\n")
        }
        None => rust,
    };
    let rust = lane_p::remembered(rust);
    let mut gaps = c.gaps;
    gaps.extend(planned::value_gaps(&rust));
    // A stand-in leans on some other hole's report: with none, the card isn't whole.
    if gaps.is_empty()
        && (rust.contains(FOLLOWS_GAP) || rust.contains(&planned::requirement(FOLLOWS_GAP)))
    {
        gaps.push(gap("result this instruction does not report"));
    }
    Ok(EmittedModule {
        rust,
        extras: c.extras,
        gaps,
        door_refused,
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
