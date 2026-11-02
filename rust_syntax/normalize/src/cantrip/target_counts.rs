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

impl Compiler {
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
            SlotCounts::Delayed => false,
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
    fn count_of_trigger(&mut self, n: &Expr, enters_itself: bool, label: &str) -> Option<()> {
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
    /// `target_count`, with what the ability reads as it goes on the stack.
    pub(super) fn slot_count(&mut self, count: &Count, label: &str) -> Result<String> {
        match self.counted_read(count, label) {
            Some(bounds) => Ok(bounds),
            None => self.target_count(count),
        }
    }
}

/// `carried`'s read of the amount of an event, past a target filter's reach.
pub(super) const AMOUNT_READ: &str =
    "planned_value!(\"event value in a target filter beyond its subject\", [u32::MAX; 2] => q.event_data())";

/// The object that entered, read in a target filter: its spell's X.
const EVENT_OBJECT_X: &str =
    "q.event_subject().object().map_or(0i64, |o: ObjectId| i64::from(q.state().obj(o).x))";

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
