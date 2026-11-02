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
    /// alone (CR 107.3m).
    Trigger { enters_itself: bool },
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
        if faces::opponents_permanent_read_as_any(self.line_text(), modes) {
            return Err(gap("an opponent's permanent read as anyone's in the data"));
        }
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
            if needs_the_slot.iter().any(|read| bodies.iter().any(|b| b.contains(read))) {
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
        for (test, effect) in tests.iter().zip(&effects) {
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
        let enters_itself = matches!(event, EventSpec::ZoneChange { .. })
            && matches!(lane_j::event_object(event), Some(ObjectRef::Source));
        SlotCounts::Trigger { enters_itself }
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
        let SlotCounts::Trigger { enters_itself } = self.slot_counts else {
            return false;
        };
        let own = matches!(n, Expr::AnnouncedX)
            || self.number(n).is_ok_and(|n| n.code == EVENT_OBJECT_X);
        enters_itself && !self.granting && self.printed_x() && own
    }
    /// The slot's bounds, where the ability can read `count` as it is put on the
    /// stack: `None` leaves it to `target_count`.
    fn counted_read(&mut self, count: &Count, label: &str) -> Option<String> {
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
            SlotCounts::Trigger { .. } if matches!(n, Expr::AnnouncedX) => self.counts_own_x(n),
            SlotCounts::Trigger { enters_itself } => {
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
        let code = number.amount(u16::MAX.into(), "u16").ok()?;
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
                "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker) -> u16 {{ {code} }}"
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
    /// `target_count`, with what the ability reads as it goes on the stack.
    pub(super) fn slot_count(&mut self, count: &Count, label: &str) -> Result<String> {
        match self.counted_read(count, label) {
            Some(bounds) => Ok(bounds),
            None => self.target_count(count),
        }
    }
}

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

/// Whether a cost has {X} among its mana.
pub(super) fn has_mana_x(cost: &Cost) -> bool {
    cost.parts.iter().any(|part| {
        matches!(part, CostPart::Mana(symbols) if symbols.iter().any(|s| matches!(s, ManaSymbol::X)))
    })
}
