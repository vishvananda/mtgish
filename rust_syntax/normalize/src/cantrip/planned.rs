//! What a card's file holds where Cantrip or the generator cannot yet do a
//! part of it. A gap stays a gap: `module` still refuses such a card. For a
//! tracked file the rest of the card is still written, and the part that is
//! missing becomes a hole naming its requirement, so the code around it is what
//! the card ends up with and the hole says exactly what it waits on.
use super::*;
use crate::corpus;
use std::path::Path;

/// The tracker's name for a gap feature: a stable slug of its words.
pub fn requirement(feature: &str) -> String {
    let mut id = String::new();
    for c in feature.chars() {
        if c.is_ascii_alphanumeric() {
            if c.is_ascii_uppercase() && id.ends_with(|p: char| p.is_ascii_lowercase()) {
                id.push('-');
            }
            id.push(c.to_ascii_lowercase());
        } else if !id.is_empty() && !id.ends_with('-') {
            id.push('-');
        }
    }
    id.trim_end_matches('-').to_string()
}

/// Requirements that have landed in Cantrip: their code is no longer held back.
/// Adding one here and regenerating is all a card waiting on it needs.
const LANDED: &[&str] = &[];

/// Whether the requirement `feature` has landed in Cantrip.
pub(super) fn landed(feature: &str) -> bool {
    LANDED.contains(&requirement(feature).as_str())
}

/// A value inside compiled code that Cantrip has no word for yet: `placeholder`
/// keeps the file compiling, `code` is what it will say, and `feature` is found
/// again by `value_gaps`, since a read has no way to record a gap of its own.
pub(super) fn planned_value(feature: &str, placeholder: &str, code: &str) -> String {
    if LANDED.contains(&requirement(feature).as_str()) {
        return code.to_string();
    }
    format!("planned_value!({feature:?}, {placeholder} => {code})")
}
/// Every `planned_value` in a file's code.
pub(super) fn value_gaps(rust: &str) -> Vec<Diagnostic> {
    const MARK: &str = "planned_value!(\"";
    let mut found = BTreeSet::new();
    for (at, _) in rust.match_indices(MARK) {
        let mut feature = String::new();
        let mut chars = rust[at + MARK.len()..].chars();
        while let Some(c) = chars.next() {
            match c {
                '"' => break,
                '\\' => feature.extend(chars.next()),
                c => feature.push(c),
            }
        }
        found.insert(feature);
    }
    found.remove(FOLLOWS_GAP);
    found.into_iter().map(gap).collect()
}
/// `placeholder`, read where the result of an instruction with a gap would be:
/// that instruction's own hole already says what the file waits on.
pub(super) fn unreported(placeholder: &str, result: &ResultId) -> String {
    planned_value(FOLLOWS_GAP, placeholder, &format!("result_{}", result.0))
}
/// What a structured instruction reads to know what to do: the code, or if it
/// has none a stand-in, so the body it holds is still compiled.
pub(super) fn planned_read(
    read: Result<String>,
    placeholder: &str,
    value: &impl serde::Serialize,
) -> String {
    match read {
        Ok(code) => code,
        Err(gap) => planned_value(&gap.feature, placeholder, &source(value)),
    }
}

/// The source of a value, for a hole to carry when no code can read it yet.
fn source(value: &impl serde::Serialize) -> String {
    quoted(&ron::to_string(value).unwrap_or_default())
}

impl Compiler {
    /// A part that stopped midway can leave anything scoped to it still set, and
    /// `?` passes over the line that would have put it back: the next ability
    /// starts as the first one did.
    pub(super) fn reset_after_gap(&mut self) {
        self.pending = Pending::Nothing;
        self.may_choose = false;
        self.chosen_guard = None;
        self.guard_opened = false;
        self.dividing = None;
        self.asking = None;
        self.repeating = None;
        self.after.clear();
        self.stack_item = true;
        self.reader = Reader::Resolution;
        self.event_at = EventAt::Nowhere;
        self.frame = Frame::default();
        self.grouped = false;
        self.recipient_subject = false;
        self.affected_in_scope = false;
        self.affected_object = None;
        self.static_guard = None;
        self.ability_guard = None;
        self.history = None;
        self.choosing = None;
        self.costing_targets = false;
        self.granting = false;
        self.outer = None;
        self.hoisted = None;
        self.activation_cost = None;
        self.slots.clear();
        self.bound_objects.clear();
        self.bound_players.clear();
        self.candidates.borrow_mut().clear();
        self.announced_filters.clear();
        self.reset_results();
    }
    /// A structure the generator keeps, its body real, that has a part it can't
    /// do yet: the hole says so beside the code.
    pub(super) fn plan_structure(&mut self, feature: &str, form: &str, out: &mut String) {
        let id = requirement(feature);
        self.gaps.push(gap(feature));
        writeln!(out, "planned!({id:?}, {form});").unwrap();
    }
    /// A printed characteristic Cantrip has no word for.
    pub(super) fn plan_trait(&mut self, path: &str, gap: Diagnostic, value: String) {
        let id = requirement(&gap.feature);
        self.gaps.push(gap.at(path));
        writeln!(self.holes, "planned_item!({id:?}, {value:?});").unwrap();
    }
    /// An ability with no code: the file keeps its source.
    pub(super) fn plan_ability(&mut self, feature: &str, ability: &Ability) {
        let id = requirement(feature);
        writeln!(self.holes, "planned_item!({id:?}, {});", source(ability)).unwrap();
    }
    /// The code a card will have once `feature` exists in Cantrip. Until then it
    /// is written out but not compiled, and the card waits on the requirement:
    /// `planned!("id" => code)`. Everything after it carries on as if it ran.
    pub(super) fn plan_code(&mut self, feature: &str, code: &str, out: &mut String) {
        let id = requirement(feature);
        if LANDED.contains(&id.as_str()) {
            writeln!(out, "{code}").unwrap();
            return;
        }
        self.gaps.push(gap(feature));
        writeln!(out, "planned!({id:?} => {code});").unwrap();
    }
    /// The entry `list` of the card definition will hold once `feature` exists:
    /// `planned_item!("id" => list: code)` until then.
    pub(super) fn plan_entry(
        &mut self,
        feature: &str,
        list: &'static str,
        code: &str,
        parts: &mut Parts,
    ) {
        let id = requirement(feature);
        if LANDED.contains(&id.as_str()) {
            parts.entry(list, code.to_string());
            return;
        }
        self.gaps.push(gap(feature));
        writeln!(self.holes, "planned_item!({id:?} => {list}: {code});").unwrap();
    }
    /// One field of a failed instruction: its value read as the code it would
    /// have been, where the generator can read it, or its source.
    fn plan_field(&self, value: &serde_json::Value) -> String {
        fn read<T: serde::de::DeserializeOwned>(value: &serde_json::Value) -> Option<T> {
            serde_json::from_value(value.clone()).ok()
        }
        let code = read::<Expr>(value)
            .and_then(|n| self.number(&n).ok().map(|n| n.code))
            .or_else(|| read::<ObjectSet>(value).and_then(|o| self.objects(&o).ok()))
            .or_else(|| read::<ObjectRef>(value).and_then(|o| self.object(&o).ok()))
            .or_else(|| read::<PlayerRef>(value).and_then(|p| self.player(&p).ok()))
            .or_else(|| read::<PlayerSet>(value).and_then(|p| self.players(&p).ok()))
            .or_else(|| read::<Condition>(value).and_then(|c| self.condition(&c).ok()));
        match code {
            Some(code) => self.fetch(code),
            None => source(value),
        }
    }
    /// An instruction with no code: `planned!(requirement, form, field = value)`.
    pub(super) fn plan_instruction(
        &mut self,
        feature: &str,
        instruction: &Instruction,
        out: &mut String,
    ) {
        let id = requirement(feature);
        let value = serde_json::to_value(instruction).unwrap_or_default();
        let (form, fields) = match instruction {
            Instruction::Effect { .. } => (
                value["args"]["operation"]["kind"].clone(),
                value["args"]["operation"]["args"].clone(),
            ),
            _ => (value["kind"].clone(), value["args"].clone()),
        };
        let form = form.as_str().unwrap_or("Instruction").to_string();
        write!(out, "planned!({id:?}, {form}").unwrap();
        match fields.as_object() {
            Some(fields) => {
                for (field, value) in fields {
                    write!(out, ", {field:?} = {}", self.plan_field(value)).unwrap();
                }
            }
            None if fields.is_null() => {}
            None => write!(out, ", \"value\" = {}", self.plan_field(&fields)).unwrap(),
        }
        writeln!(out, ");").unwrap();
    }
}

/// One face of a tracked card.
#[derive(serde::Serialize)]
pub struct TrackedFace {
    pub name: String,
    pub oracle: Option<String>,
    pub rust: String,
    pub gaps: Vec<Diagnostic>,
}

/// A record's generated code, whole.
#[derive(serde::Serialize)]
pub struct TrackedCard {
    pub line: usize,
    pub name: String,
    pub layout: String,
    pub faces: Vec<TrackedFace>,
    /// The card's file where its faces are joined into one card.
    pub rust: Option<String>,
    /// A gapless single face's file in Cantrip's hand-written style.
    pub file: Option<String>,
}

/// Every card's code, a JSON object a line. A record that can't be emitted
/// stops the run: nothing is left for the tracker to miss.
pub fn tracked(input: &Path, oracle: &Path, output: &Path) -> corpus::Result<usize> {
    let oracle = oracle_texts(oracle)?;
    let mut artifact = corpus::Artifact::create(output)?;
    let mut count = 0;
    corpus::map_cards(
        input,
        |line, source| -> corpus::Result<TrackedCard> {
            let identity = corpus::Identity::of(&source)?;
            let lowered = crate::unified::lower(&source);
            let name = identity.names.join(" // ");
            let whole = lane_f::whole_printed(&source, &lowered.card, Some(&oracle))
                .map_err(|g| format!("{name}: {}: {} ({})", g.path, g.feature, g.message))?;
            let faces: Vec<TrackedFace> = whole
                .faces
                .into_iter()
                .map(|face| TrackedFace {
                    oracle: oracle.get(&face.name).cloned(),
                    name: face.name,
                    rust: face.rust,
                    gaps: face.gaps,
                })
                .collect();
            let file = match faces.as_slice() {
                [one] if one.gaps.is_empty() && lowered.card.layout == "Card" => one
                    .oracle
                    .as_deref()
                    .and_then(|text| card_file::card_file(&lowered.card.faces[0], text).ok())
                    .flatten(),
                _ => None,
            };
            Ok(TrackedCard {
                line,
                name,
                layout: lowered.card.layout.clone(),
                faces,
                rust: whole.rust,
                file,
            })
        },
        |_, card| {
            writeln!(artifact, "{}", serde_json::to_string(&card)?)?;
            count += 1;
            Ok(())
        },
    )?;
    artifact.commit()?;
    Ok(count)
}
