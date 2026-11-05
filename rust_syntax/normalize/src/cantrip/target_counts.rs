//! How many targets a slot takes when no number is printed: "up to X target
//! creatures". A slot's size is a constant or `TargetSpec::COUNTED`, and what
//! `COUNTED` reads depends on what has the slot.
use super::*;

/// What a slot's `TargetSpec::COUNTED` reads.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum SlotCounts {
    /// A spell's or an activated ability's: the X announced.
    #[default]
    Announced,
    /// A triggered ability's: what `counted_by` says, else its source's X or
    /// kicks. A permanent keeps the X of its spell for its "enters" triggers
    /// alone (CR 107.3m), and the X of its morph or disguise cost for its
    /// "turned face up" triggers (CR 702.37f, 702.168e). Its "becomes
    /// monstrous" triggers read the X of the monstrosity that made it so (CR
    /// 701.37c), which Cantrip keeps nowhere.
    Trigger { enters_itself: bool, turned_up_itself: bool, monstrous_itself: bool },
    /// A delayed or a reflexive ability's: what it was told (`sized_slot`).
    Delayed,
}

/// The modes of `announcement`, if each is what a kind of target implies and the
/// ability is one target slot and what that target turns out to be.
fn implied_modes(announcement: &Announcement) -> Option<&[Mode]> {
    let selection = announcement.modes.as_ref()?;
    let one = |n: &Expr| matches!(n, Expr::Constant(1));
    let plain = selection.implied
        && announcement.targets.is_empty()
        && one(&selection.minimum)
        && one(&selection.maximum)
        && !selection.repeat
        && selection.unchosen.is_none()
        && selection.surcharge.is_none()
        && selection.chooser.is_none()
        && !selection.random;
    let modes = selection.modes.as_slice();
    let each = |mode: &Mode| mode.requires.is_none() && mode.announcement.modes.is_none();
    (plain && modes.len() > 1 && modes.iter().all(each)).then_some(modes)
}

/// "Deals X damage to target creature. If you controlled a modified creature as
/// you cast this spell, it deals X plus 2 damage instead": two modes of one slot
/// that a condition and its negation tell apart, read as the spell is cast. The
/// condition, the two modes in its order, if it is that.
pub(super) fn modes_told_as_cast(announcement: &Announcement) -> Option<(&Condition, &Mode, &Mode)> {
    let selection = announcement.modes.as_ref()?;
    let one = |n: &Expr| matches!(n, Expr::Constant(1));
    let plain = selection.implied
        && announcement.targets.is_empty()
        && one(&selection.minimum)
        && one(&selection.maximum)
        && !selection.repeat
        && selection.unchosen.is_none()
        && selection.surcharge.is_none()
        && selection.chooser.is_none()
        && selection.budget.is_none()
        && !selection.random;
    let [first, second] = selection.modes.as_slice() else {
        return None;
    };
    let (Some(condition), Some(otherwise)) = (&first.requires, &second.requires) else {
        return None;
    };
    let negated = format!("{:?}", Condition::Not(Box::new(condition.clone())));
    let same_slots = format!("{:?}", first.announcement) == format!("{:?}", second.announcement);
    (plain && same_slots && first.announcement.modes.is_none() && negated == format!("{otherwise:?}"))
        .then_some((condition, first, second))
}

/// The line of `functions` that starts with `start`, without its name.
fn line_of(functions: &str, start: &str, name: &str) -> Option<String> {
    let line = functions.lines().rev().find(|line| line.contains(start))?;
    Some(line.replace(name, ""))
}

impl Compiler {
    /// "Target spell or permanent": the source reads one mode for each kind of
    /// target, and no mode is announced (CR 601.2c): one slot with a filter of
    /// either, and an effect that goes by what was chosen. `None` if it isn't that.
    pub(super) fn implied_slots(
        &mut self,
        announcement: &Announcement,
        label: &str,
        frame: Option<Frame>,
    ) -> Result<Option<String>> {
        let Some(modes) = implied_modes(announcement) else {
            return Ok(None);
        };
        let before = self.clone();
        match self.union_of_modes(modes, label, frame)? {
            Some(targets) => Ok(Some(targets)),
            None => {
                *self = before;
                Ok(None)
            }
        }
    }
    fn union_of_modes(
        &mut self,
        modes: &[Mode],
        label: &str,
        frame: Option<Frame>,
    ) -> Result<Option<String>> {
        let mut filters: Vec<Vec<String>> = Vec::new();
        let mut arrays = Vec::new();
        let mut unused = Vec::new();
        for (i, mode) in modes.iter().enumerate() {
            let label = format!("{label}_mode_{i}");
            let announced = lane_p::without_modes(&mode.announcement);
            let name = match frame {
                Some(frame) => self.targets_for(&announced, &label, frame)?,
                None => self.announcement(&announced, &label)?,
            };
            if self.dividing.is_some() {
                return Ok(None);
            }
            let Some(array) = line_of(&self.functions, &format!("static {name}:"), &name) else {
                return Ok(None);
            };
            filters.push(self.announced_filters.clone());
            arrays.push(array);
            unused.push(format!("static {name}:"));
        }
        let first = filters[0].clone();
        let mut same = arrays[0].clone();
        for (names, array) in filters.iter().zip(&arrays) {
            if names.len() != first.len() {
                return Ok(None);
            }
            let array = names
                .iter()
                .zip(&first)
                .fold(array.clone(), |array, (name, first)| array.replace(name, first));
            if array != same {
                return Ok(None);
            }
        }
        let mut branch = None;
        for (slot, name) in first.iter().enumerate() {
            let tests: Vec<String> = filters.iter().map(|names| names[slot].clone()).collect();
            // A slot every mode fills alike ("any target"), by the same filter.
            if tests.iter().all(|test| *test == tests[0]) {
                continue;
            }
            let bodies: Option<Vec<String>> = tests
                .iter()
                .map(|test| line_of(&self.functions, &format!("fn {test}("), test))
                .collect();
            let Some(bodies) = bodies else {
                return Ok(None);
            };
            if bodies.iter().all(|body| *body == bodies[0]) {
                unused.extend(tests[1..].iter().map(|test| format!("fn {test}(")));
                continue;
            }
            let union = format!("{label}_target_{slot}");
            let either: Vec<String> = tests.iter().map(|t| format!("{t}(q, asker, t)")).collect();
            writeln!(
                self.functions,
                "#[allow(unused_variables)] fn {union}(q: &dyn Query, asker: &Asker, t: Target) -> bool {{ {} }}",
                either.join(" || ")
            )
            .unwrap();
            same = same.replace(&format!("target({name})"), &format!("target({union})"));
            let needs_the_slot = ["chosen_targets", "announced_x", "event_subject", "i64::from(x)"];
            // "Other than it", read of the trigger's subject, only narrows a mode:
            // told apart at resolution without it, a target still goes to the one
            // mode whose kind it is.
            let other = "!(q.event_subject().object() == Some(id))";
            if needs_the_slot
                .iter()
                .any(|read| bodies.iter().any(|b| b.replace(other, "").contains(read)))
            {
                return Ok(None);
            }
            branch = branch.or(Some((slot, tests)));
        }
        let Some(branch) = branch else {
            return Ok(None);
        };
        self.implied_branch = Some(branch);
        let name = format!("{}_TARGETS", label.to_ascii_uppercase());
        let kept: Vec<&str> = self
            .functions
            .lines()
            .filter(|line| !unused.iter().any(|gone| line.contains(gone.as_str())))
            .collect();
        self.functions = kept.join("\n") + "\n";
        writeln!(self.functions, "{}", same.replacen("static ", &format!("static {name}"), 1)).unwrap();
        Ok(Some(name))
    }
    /// A spell of `modes_told_as_cast`: one slot, and the mode its condition picks,
    /// kept as the spell became cast (`CardDef::cast_fact`, CR 601.2i) as Faerie
    /// Fencing keeps it, so a copy, which was not cast (CR 707.10), has the other.
    /// The spell's `spell(..)`, if it is that.
    pub(super) fn spell_told_as_cast(
        &mut self,
        announcement: &Announcement,
        label: &str,
    ) -> Result<Option<String>> {
        let Some((condition, first, second)) = modes_told_as_cast(announcement) else {
            return Ok(None);
        };
        if self.cast_fact.borrow().is_some() {
            return Err(gap("two facts kept as a spell is cast"));
        }
        let targets = self.announcement(&first.announcement, label)?;
        let then = self.effect(&first.body, &format!("{label}_mode_0"))?;
        let otherwise = self.effect(&second.body, &format!("{label}_mode_1"))?;
        let test = self.activation_condition(condition, &format!("{label}_as_cast"))?;
        *self.cast_fact.borrow_mut() = Some(format!("i64::from({test}(q, asker))"));
        let function = format!("{label}_told_as_cast");
        writeln!(
            self.functions,
            "fn {function}(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {{ if g.history().fact_as_cast(item.object) != 0 {{ {then}(g, item) }} else {{ {otherwise}(g, item) }} }}"
        )
        .unwrap();
        Ok(Some(format!("spell({targets}, {function})")))
    }
    /// The effect of an ability `implied_slots` wrote: each mode's, by the target.
    pub(super) fn implied_effect(
        &mut self,
        announcement: &Announcement,
        label: &str,
    ) -> Result<String> {
        let modes = implied_modes(announcement).expect("implied modes");
        let (slot, tests) = self.implied_branch.take().expect("a slot that tells the modes");
        let mut effects = Vec::new();
        for (i, mode) in modes.iter().enumerate() {
            effects.push(self.effect(&mode.body, &format!("{label}_mode_{i}"))?);
        }
        let function = format!("{label}_implied");
        let mut arms = String::new();
        // The last mode is what is left when no earlier one is chosen: its own test could only say yes.
        for (test, effect) in tests.iter().zip(&effects).take(effects.len() - 1) {
            writeln!(arms, "if {test}(g, asker, chosen) {{ return {effect}(g, item); }}").unwrap();
        }
        let last = effects.last().expect("modes");
        writeln!(
            self.functions,
            "fn {function}(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {{ let asker = &Asker::new(item.object, item.controller); let chosen = (0..g.state().targets(item).len()).filter(|&i| g.state().spec(item, i) == {slot}).map(|i| g.state().target(item, i)).next().unwrap_or(Target::None);\n{arms}{last}(g, item) }}"
        )
        .unwrap();
        Ok(function)
    }
    /// Whether every slot of a triggered ability is chosen by `chooser`, a player
    /// its subject can be: the engine asks the subject for all of them (CR 603.3d).
    pub(super) fn chosen_by_subject(&mut self, chooser: &PlayerRef) -> bool {
        let SlotCounts::Trigger { .. } = self.slot_counts else {
            return false;
        };
        let subject = match chooser {
            PlayerRef::EventSubject => true,
            PlayerRef::ControllerOf(object) => {
                self.controller_subject && matches!(**object, ObjectRef::EventSubject)
            }
            _ => false,
        };
        let chosen = subject && self.frame.player == Some(Carried::Subject);
        self.subject_chooses |= chosen;
        chosen
    }
    /// `event` is this object entering the battlefield.
    pub(super) fn counts_of_trigger(event: &EventSpec) -> SlotCounts {
        let itself = matches!(lane_j::event_object(event), Some(ObjectRef::Source));
        let enters_itself = matches!(event, EventSpec::ZoneChange { .. }) && itself;
        let turned_up_itself = itself
            && matches!(event, EventSpec::Object { event: ObjectEvent::TurnsFaceUp, .. });
        let monstrous_itself = itself
            && matches!(event, EventSpec::Object { event: ObjectEvent::BecomesMonstrous, .. });
        SlotCounts::Trigger { enters_itself, turned_up_itself, monstrous_itself }
    }
    /// Whether the mana cost printed on this card has an {X}.
    fn printed_x(&self) -> bool {
        self.printed_mana
            .iter()
            .flatten()
            .any(|symbol| matches!(symbol, ManaSymbol::X))
    }
    /// Whether `n` is the X this object's spell was cast with, and `COUNTED`
    /// reads it, as a total to divide does.
    pub(super) fn counts_own_x(&self, n: &Expr) -> bool {
        let SlotCounts::Trigger { enters_itself, .. } = self.slot_counts else {
            return false;
        };
        let own = matches!(n, Expr::AnnouncedX)
            || self.number(n).is_ok_and(|n| n.code == EVENT_OBJECT_X);
        enters_itself && !self.granting && self.printed_x() && own
    }
    /// The slot's bounds, where the ability can read `count` as it is put on the
    /// stack: `None` leaves it to `target_count`.
    pub(super) fn counted_read(&mut self, count: &Count, label: &str) -> Option<String> {
        let (n, bounds) = match count {
            Count::UpTo(n) => (n, ".between(0, TargetSpec::COUNTED)"),
            Count::Exactly(n) => (n, ".counted()"),
            _ => return None,
        };
        if matches!(n, Expr::Constant(_)) || lane_l::sizes_slot(&self.sized_slot, n) {
            return None;
        }
        let read = match self.slot_counts {
            // "When you do, distribute X": the X paid, told to the ability.
            SlotCounts::Delayed => matches!(n, Expr::AnnouncedX) && self.counted.is_some(),
            SlotCounts::Announced => matches!(n, Expr::AnnouncedX),
            // "When this is turned face up, ... up to X": the X its morph or
            // disguise cost was paid with, which the special action keeps on the
            // permanent (CR 702.168e; `Object::x`), read as the ability triggers.
            // Turned up any other way, or never cast with an X, it is 0.
            SlotCounts::Trigger { turned_up_itself: true, .. }
                if matches!(n, Expr::AnnouncedX) && !self.granting =>
            {
                self.count_of_trigger(&Expr::CastX(ObjectRef::Source), false, label).is_some()
            }
            SlotCounts::Trigger { .. } if matches!(n, Expr::AnnouncedX) => self.counts_own_x(n),
            SlotCounts::Trigger { enters_itself, .. } => {
                self.counts_own_x(n) || self.count_of_trigger(n, enters_itself, label).is_some()
            }
        };
        read.then(|| bounds.to_string())
    }
    /// `.counted_by` or `.counted_by_amount` for a triggered ability's slot. An
    /// ability has one number, so a second that differs is refused.
    pub(super) fn count_of_trigger(&mut self, n: &Expr, enters_itself: bool, label: &str) -> Option<()> {
        let number = self.number(n).ok()?;
        let amount = number.code == format!("i64::from({AMOUNT_READ}[0])");
        let kind = quantity::count_kind();
        let code = number.amount(quantity::x_max(), kind).ok()?;
        let code = match enters_itself {
            true => code.replace("q.event_subject().object()", "Some(asker.source)"),
            false => code,
        };
        let read_elsewhere = ["planned_value!", "q.event_subject()", "q.chosen_targets()", "item."];
        if !amount && read_elsewhere.iter().any(|read| code.contains(read)) {
            return None;
        }
        if let Some((read, _)) = &self.count_read {
            return (*read == code).then_some(());
        }
        let call = if amount {
            ".counted_by_amount()".to_string()
        } else {
            let function = format!("{label}_count");
            writeln!(
                self.functions,
                "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker) -> {kind} {{ {code} }}"
            )
            .unwrap();
            format!(".counted_by({function})")
        };
        self.count_read = Some((code, call));
        Some(())
    }
    /// "Up to X target permanents, where X is ...", or "up to twice X": no slot's
    /// size says either, so the slot takes any number and the rule of its targets
    /// together holds them to it. That rule is judged again on resolution (CR
    /// 608.2b), where a number that has changed must not make every target
    /// illegal: it reads the X announced, or else what the targets were chosen with
    /// (`SLOT_HELD`), which tells one spell or ability's only where the object has
    /// no other: a spell, or an ability that sacrifices its source to be activated.
    /// A player is no card to such a rule, so it is for objects alone.
    pub(super) fn slot_up_to_read(
        &mut self,
        spec: &TargetSpec,
        i: usize,
        label: &str,
    ) -> Option<String> {
        let Count::UpTo(n) = &spec.count else {
            return None;
        };
        let only_once = self.activation_cost.as_ref().is_none_or(sacrifices_its_source);
        if matches!(n, Expr::Constant(_))
            || lane_l::sizes_slot(&self.sized_slot, n)
            || self.slot_counts != SlotCounts::Announced
            || !spec.constraints.is_empty()
            || spec.distribute.is_some()
            || spec.players.is_some()
            || !matches!(spec.kind, TargetKind::Object(_))
        {
            return None;
        }
        let code = self.number(n).ok()?.amount(u32::MAX.into(), "u32").ok()?;
        let announced = code.contains(ANNOUNCED_X);
        let code = code.replace(ANNOUNCED_X, "i64::from(x)");
        let read_elsewhere = ["planned_value!", "q.event_subject()", "q.chosen_targets()", "item."];
        if read_elsewhere.iter().any(|read| code.contains(read))
            || (announced && code.contains("q."))
            || (!announced && !only_once)
        {
            return None;
        }
        let name = format!("{label}_{i}_up_to");
        let most = match announced {
            true => format!("{code} as usize"),
            false => {
                self.helpers.insert("slot_held");
                format!("({code} as usize).max(slot_held(q, source, {i}))")
            }
        };
        writeln!(self.functions, "#[allow(unused_variables)] fn {name}_most(q: &dyn Query, source: ObjectId, chooser: PlayerId, x: u32) -> usize {{ let asker = &Asker::new(source, chooser); {most} }}\nfn {name}(q: &dyn Query, choice: &Choice, x: u32) -> Verdict {{ let most = {name}_most(q, choice.source, choice.chooser, x); Verdict {{ complete: choice.selected.len() <= most, addable: Bits::new(choice.candidates.len(), choice.selected.len() < most) }} }}\nfn largest_{name}(q: &dyn Query, choice: &Choice, x: u32) -> Vec<ObjectId> {{ choice.candidates.iter().copied().take({name}_most(q, choice.source, choice.chooser, x)).collect() }}").unwrap();
        let number = match announced {
            true => "TargetSpec::COUNTED",
            false => "0",
        };
        Some(format!(".between(0, cantrip_api::TargetSpec::ANY_NUMBER).together(cantrip_api::select::set_part(&({name} as SelectFn), &(largest_{name} as cantrip_api::select::LargestFn), {number}, \"up to X\"))"))
    }
    /// A total to divide that is a function of the X announced and of nothing
    /// else ("X plus 1"): the name of that function, for `Divide::of_x`.
    pub(super) fn total_of_x(&mut self, total: &Expr, label: &str) -> Option<String> {
        let code = self.number(total).ok()?.code;
        if !code.contains(ANNOUNCED_X) {
            return None;
        }
        let mut code = code.replace(ANNOUNCED_X, "x");
        // A spell's own X, "if X is 6 or more" (Shatterskull Smashing), is the one
        // announced for it (CR 107.3a).
        if self.activation_cost.is_none() {
            code = code.replace(SOURCE_X, "x");
        }
        let reads = ["q.", "planned_value!", "item.", "g.", "asker"];
        if reads.iter().any(|read| code.contains(read)) {
            return None;
        }
        let name = format!("{label}_divided_total");
        let kind = if planned::switched_on("divide-reads-context") { "u32" } else { quantity::count_kind() };
        let context = if planned::switched_on("divide-reads-context") { "_: &dyn Query, _: &Asker, " } else { "" };
        writeln!(self.functions, "fn {name}({context}x: {kind}) -> {kind} {{ let x = i64::from(x); ({code}).clamp(0, i64::from({kind}::MAX)) as {kind} }}").unwrap();
        Some(name)
    }
    /// "When you do, put target creature card with mana value less than or equal
    /// to the number of study counters removed this way" (Vhal, Scholar of
    /// Mortality): a filter has no read of the number a reflexive ability was
    /// given, but the rule of a slot's targets together does (`COUNTED`), and one
    /// target's mana value is the total of a set of one. The slot with that test
    /// moved from its filter to its rule.
    pub(super) fn told_mana_value_slot(&self, spec: &TargetSpec) -> Option<TargetSpec> {
        let one = |n: &Expr| matches!(n, Expr::Constant(1));
        let single = matches!(&spec.count, Count::Exactly(n) | Count::UpTo(n) if one(n));
        if self.slot_counts != SlotCounts::Delayed || !single || !spec.constraints.is_empty() {
            return None;
        }
        let counted = self.counted?;
        let (filter, amount) = take_told(&spec.filter, counted)?;
        Some(TargetSpec {
            filter,
            constraints: vec![SelectionConstraint::TotalProperty {
                property: Property::ManaValue,
                comparison: Comparison::LessOrEqual,
                amount,
            }],
            ..spec.clone()
        })
    }
    /// `target_count`, with what the ability reads as it goes on the stack.
    pub(super) fn slot_count(&mut self, count: &Count, label: &str) -> Result<String> {
        if let (
            SlotCounts::Trigger { monstrous_itself: true, .. },
            Count::UpTo(Expr::AnnouncedX) | Count::Exactly(Expr::AnnouncedX),
        ) = (self.slot_counts, count)
        {
            return Err(gap(MONSTROSITY_X));
        }
        match self.counted_read(count, label) {
            Some(bounds) => Ok(bounds),
            None => self.target_count(count),
        }
    }
}

/// `filter` without its one "mana value N or less", N the number told to the
/// ability (`counted`), and N.
fn take_told(filter: &ObjectFilter, counted: u32) -> Option<(ObjectFilter, Expr)> {
    let told = |n: &Expr| match n {
        Expr::Performed(r) | Expr::CountersRemoved(r) | Expr::DieResult(r) => r.0 == counted,
        _ => false,
    };
    match filter {
        ObjectFilter::Property {
            property: Property::ManaValue,
            comparison: Comparison::LessOrEqual,
            value,
        } if told(value) => Some((ObjectFilter::Any, value.clone())),
        ObjectFilter::All(filters) => {
            let mut found = None;
            let mut rest = Vec::new();
            for f in filters {
                match take_told(f, counted) {
                    Some((left, amount)) if found.is_none() => {
                        found = Some(amount);
                        rest.push(left);
                    }
                    Some(_) => return None,
                    None => rest.push(f.clone()),
                }
            }
            let rest: Vec<ObjectFilter> = rest.into_iter().filter(|f| !matches!(f, ObjectFilter::Any)).collect();
            Some((ObjectFilter::All(rest), found?))
        }
        _ => None,
    }
}

/// The hole of an "up to N targets" slot that no `COUNTED` reads, `read` being
/// the number's code: what is missing to read it.
pub(super) fn up_to_feature(counts: SlotCounts, activated: bool, read: &str) -> &'static str {
    match counts {
        // The number is a hole of its own, which says what it waits on.
        _ if read.contains("planned_value!") => FOLLOWS_GAP,
        // "Up to X target cards from your graveyard, where X is the number of black
        // permanents target opponent controls" (Reap): a rule of a slot's targets
        // together is asked with no other slot's targets (`within_together`,
        // `together_legal`), and a count that is no X is read again on resolution.
        SlotCounts::Announced if read.contains("q.chosen_targets()") => {
            "a target count read from another target"
        }
        // "Up to X target creatures you control, where X is the number of
        // Bobbleheads you control as you activate this ability": an activated
        // ability's `COUNTED` is its X alone (`StackItem::counted`), and no
        // `ActivatedDef` field reads a number as it is activated, as
        // `TriggeredDef::counted_by` does for a triggered ability.
        SlotCounts::Announced if activated && !read.contains("planned_value!") => {
            "an activated ability's target count read as it is activated"
        }
        _ => "target cardinality UpTo",
    }
}

/// "When this creature becomes monstrous, ... X": `Event::Monstrous` carries no
/// number and nothing keeps the monstrosity's X (CR 701.37c).
pub(super) const MONSTROSITY_X: &str = "the X of the monstrosity a becomes-monstrous trigger reads";

/// Whether a cost sacrifices the ability's source, which so can't be activated again.
fn sacrifices_its_source(cost: &Cost) -> bool {
    cost.parts.iter().any(|part| {
        matches!(
            part,
            CostPart::Sacrifice {
                objects: ObjectSet::One(ObjectRef::Source),
                ..
            }
        )
    })
}

/// The X announced, as a number read outside a resolution says it.
const ANNOUNCED_X: &str =
    "planned_value!(\"numeric expression AnnouncedX\", 0i64 => i64::from(q.announced_x().unwrap_or(0)))";

/// How many targets a spell or ability of `source` holds in `slot`, on the stack or
/// resolving: "up to X" was as many or more as they were chosen.
pub(super) const SLOT_HELD: &str = "fn slot_held(q: &dyn Query, source: ObjectId, slot: usize) -> usize { let s = q.state(); s.stack.iter().chain(s.left_stack.iter()).filter(|i| i.object == source).map(|i| s.targets(i)[s.spec_targets(i, slot)].iter().filter(|t| **t != Target::None).count()).max().unwrap_or(0) }\n";

/// `carried`'s read of the amount of an event, past a target filter's reach.
pub(super) const AMOUNT_READ: &str =
    "planned_value!(\"event value in a target filter beyond its subject\", [u32::MAX; 2] => q.event_data())";

/// The X of the source's spell (`Expr::CastX` of the source).
const SOURCE_X: &str = "Some(asker.source).map_or(0i64, |o: ObjectId| i64::from(q.state().obj(o).x))";

/// The object that entered, read in a target filter: its spell's X.
const EVENT_OBJECT_X: &str =
    "q.event_subject().object().map_or(0i64, |o: ObjectId| i64::from(q.state().obj(o).x))";

/// Chaos Mutation's slot as Oracle text has it. The source rewrites "any number of
/// target creatures controlled by different players" (`data/cantrip_rewrites.json`)
/// as a slot for each player, which holds a target of no one else's, and a change of
/// control in response would leave a different set of targets legal (CR 608.2b).
pub(super) fn oracle_slot(card: &str, spec: &TargetSpec) -> Option<TargetSpec> {
    (card == "Chaos Mutation" && matches!(spec.count, Count::PerPlayer { .. })).then(|| TargetSpec {
        count: Count::AnyNumber,
        constraints: vec![SelectionConstraint::DifferentControllers],
        ..spec.clone()
    })
}

/// "For each player, target permanent that player controls" as the source says
/// it: up to that many targets, each under a different controller. That is a
/// slot for each player, which can't be declined (CR 601.2c), and not a count.
pub(super) fn each_player_slot(spec: &TargetSpec) -> Option<TargetSpec> {
    let (Count::UpTo(Expr::CountPlayers(players)), [SelectionConstraint::DifferentControllers]) =
        (&spec.count, spec.constraints.as_slice())
    else {
        return None;
    };
    let ObjectFilter::All(filters) = &spec.filter else {
        return None;
    };
    let [filter, ObjectFilter::ControlledBy(controllers)] = filters.as_slice() else {
        return None;
    };
    (format!("{controllers:?}") == format!("{players:?}")).then(|| TargetSpec {
        filter: filter.clone(),
        count: Count::PerPlayer {
            players: players.clone(),
            each: Box::new(Count::Exactly(Expr::Constant(1))),
        },
        constraints: Vec::new(),
        ..spec.clone()
    })
}

/// Whether a cost picks what it pays by X, with no {X} to announce it: "sacrifice
/// an artifact with mana value X" (Scrap Welder, Sidisi, Kozilek).
pub(super) fn x_in_cost_filter(cost: &Cost) -> bool {
    !has_mana_x(cost)
        && cost.parts.iter().any(|part| {
            let said = format!("{part:?}");
            said.contains("AnnouncedX") && said.contains("Property")
        })
}

/// Whether a cost removes X counters, X the number removed.
pub(super) fn removes_x_counters(cost: &Cost) -> bool {
    cost.parts.iter().any(|part| {
        matches!(part, CostPart::RemoveCounters { removals } if removals.iter().any(|r| matches!(
            r.amount,
            CounterAmount::Count(Count::Exactly(Expr::AnnouncedX))
        )))
    })
}

/// Whether a cost has {X} among its mana.
pub(super) fn has_mana_x(cost: &Cost) -> bool {
    cost.parts.iter().any(|part| {
        matches!(part, CostPart::Mana(symbols) if symbols.iter().any(|s| matches!(s, ManaSymbol::X)))
    })
}
