//! Reading the game: objects, players, numbers, filters and conditions. Every
//! callback shares this compiler, so one predicate means one thing wherever it
//! is asked. Generated reads name `q` and `asker` only: `q` is a `Query` or, in
//! a layer callback, a `LayerQuery`. Both answer `state`, `chars`, `words` and
//! `controller`, which is all generated here beyond what `Reader` selects.
use super::*;

/// The card printing what is being compiled: `delayed` is that card's list, and
/// a copy of the object has the same text (CR 707.2).
const SOURCE_CARD: &str =
    "{ let o = q.state().obj(asker.source); o.copied_card().unwrap_or(o.card) }";

/// Which callback is being generated.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Reader {
    /// `g` and, unless in a mana callback, `item`; reads go through `fetch`.
    Resolution,
    /// A `Query` and an `Asker`: filters, matchers and conditions.
    Query,
    /// A `LayerQuery`: characteristics are mid-computation.
    Layer,
}

/// The object a filter asks about. In a layer callback the affected object's
/// characteristics are `so_far`, not what another read of it would answer.
#[derive(Clone)]
pub(super) struct Candidate {
    pub id: String,
    pub chars: String,
    pub controller: String,
    /// Where the targets of a spell not on the stack yet are, if it has any
    /// to speak of: `None` for one that is.
    pub announced: Option<String>,
}
impl Candidate {
    pub fn of(id: &str) -> Self {
        Self {
            id: id.into(),
            chars: format!("q.chars({id})"),
            controller: format!("q.controller({id})"),
            announced: None,
        }
    }
    pub fn affected() -> Self {
        Self {
            id: "affected".into(),
            chars: "(*so_far)".into(),
            controller: "q.controller(affected)".into(),
            announced: None,
        }
    }
    /// An object about to enter, which is controlled by the player it enters
    /// under (CR 110.2a) and not yet by whoever has it now.
    pub fn entering() -> Self {
        Self {
            controller: "z.controller".into(),
            ..Self::of("id")
        }
    }
}

/// The pending event a replacement program is changing, if one is.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Pending {
    Nothing,
    /// `entering`, the source's own arrival: an `Enters` callback.
    Entering,
    /// A static effect's `then`: `apply` already changed the event.
    Reshaped,
}

/// A question about past events: `Expr::EventHistory`.
#[derive(Clone)]
pub(super) struct History {
    pub outer: Option<String>,
    /// An event reference in the question is to the event of the ability that
    /// asks it, not the one in the record.
    pub event_at: EventAt,
    pub frame: Frame,
}

/// Where one event value travels from the matcher to the resolving ability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Carried {
    Subject,
    Data(usize),
    /// The stack position in the high bits of `data[i]`: what was put on the
    /// stack as the event happened.
    Position(usize),
}

/// What a matched event binds, for the matcher and the ability it triggers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Frame {
    pub object: Option<Carried>,
    pub player: Option<Carried>,
    pub destination: Option<Carried>,
    pub other: Option<Carried>,
    pub other_player: Option<Carried>,
    pub amount: Option<Carried>,
    pub damage_source: Option<Carried>,
    pub damage_recipient: Option<Carried>,
    /// Damage dealt to a permanent or a player: `data` holds which, as `Target`.
    pub recipient: Option<Carried>,
    pub stack_object: Option<Carried>,
    /// The serial of the activated ability an event is about, on the stack.
    pub activated: Option<Carried>,
    /// A spell's cast or an activation: a spell in `subject`, or else a serial in `data[1]`.
    pub spell_or_ability: bool,
    /// Who controls what `stack_object` names, spell or ability.
    pub stack_controller: Option<Carried>,
    pub attacker: Option<Carried>,
    pub blocker: Option<Carried>,
    /// "That many" of a "one or more" ability is how many objects its batch has.
    pub members: bool,
    /// What holds the controller of the object that is the subject, which a
    /// target filter finds from the subject alone.
    pub subject_controller: Option<Carried>,
}
impl Frame {
    /// What every branch of an alternative event binds the same way.
    pub fn meet(self, other: Self) -> Self {
        fn same(a: Option<Carried>, b: Option<Carried>) -> Option<Carried> {
            a.filter(|a| b == Some(*a))
        }
        Self {
            object: same(self.object, other.object),
            player: same(self.player, other.player),
            destination: same(self.destination, other.destination),
            other: same(self.other, other.other),
            other_player: same(self.other_player, other.other_player),
            amount: same(self.amount, other.amount),
            damage_source: same(self.damage_source, other.damage_source),
            damage_recipient: same(self.damage_recipient, other.damage_recipient),
            recipient: same(self.recipient, other.recipient),
            stack_object: same(self.stack_object, other.stack_object),
            activated: same(self.activated, other.activated),
            spell_or_ability: self.spell_or_ability && other.spell_or_ability,
            stack_controller: same(self.stack_controller, other.stack_controller),
            attacker: same(self.attacker, other.attacker),
            blocker: same(self.blocker, other.blocker),
            members: self.members && other.members,
            subject_controller: same(self.subject_controller, other.subject_controller),
        }
    }
}

/// Where the current callback finds the values of `Frame`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum EventAt {
    /// No event is in reach: `.condition` callbacks are not told one.
    Nowhere,
    /// The locals `subject` and `data` of a trigger matcher.
    Matcher,
    /// The resolving stack item.
    Item,
    /// `Query::event_subject`, the one value a target filter is given.
    TargetFilter,
    /// An `Enters` callback: the subject is what is entering, its source.
    Entering,
}

/// Whether `changes` read the object each is applied to, `ObjectRef::Affected`,
/// rather than what an earlier result affected, `ObjectSet::Affected`.
pub(super) fn reads_affected(changes: impl std::fmt::Debug) -> bool {
    format!("{changes:?}")
        .replace("Affected(", "")
        .contains("Affected")
}

pub(super) fn compare(left: &str, comparison: &Comparison, right: &str) -> String {
    let symbol = match comparison {
        Comparison::Equal => "==",
        Comparison::NotEqual => "!=",
        Comparison::Less => "<",
        Comparison::LessOrEqual => "<=",
        Comparison::Greater => ">",
        Comparison::GreaterOrEqual => ">=",
        Comparison::SameParity => return format!("(({left}) - ({right})) % 2 == 0"),
        Comparison::DifferentParity => return format!("(({left}) - ({right})) % 2 != 0"),
    };
    format!("({left}) {symbol} ({right})")
}

fn event_zone(zone: &EventZone) -> Result<&'static str> {
    Ok(match zone {
        EventZone::Battlefield => "Battlefield",
        EventZone::Graveyard => "Graveyard",
        EventZone::Hand => "Hand",
        EventZone::Library => "Library",
        EventZone::Exile => "Exile",
        EventZone::Stack => "Stack",
        EventZone::Command => "Command",
        EventZone::Ante => return Err(gap("ante zone")),
        EventZone::Outside => return Err(gap("outside the game")),
        EventZone::PlanarDeck => return Err(gap("planar deck")),
    })
}
pub(super) fn zone_kind(zone: &EventZone) -> Result<String> {
    if matches!(zone, EventZone::PlanarDeck) {
        return Ok(planned::planned_value(
            "planar deck",
            "ZoneKind::Command",
            "ZoneKind::PlanarDeck",
        ));
    }
    Ok(format!("ZoneKind::{}", event_zone(zone)?))
}
pub(super) fn counter_kind(kind: &crate::counter::CounterKind) -> Result<String> {
    use crate::counter::CounterKind as K;
    let word = match kind {
        K::PowerToughness(1, 1) => "PlusOne".to_string(),
        K::PowerToughness(-1, -1) => "MinusOne".to_string(),
        K::PowerToughness(power, toughness) => {
            let part = |n: &i32| {
                format!(
                    "{}{}",
                    if *n < 0 { "Minus" } else { "Plus" },
                    n.unsigned_abs()
                )
            };
            return Ok(planned::planned_value(
                "power/toughness counter other than ±1/±1",
                "CounterKind::PlusOne",
                &format!("CounterKind::{}{}", part(power), part(toughness)),
            ));
        }
        K::Named(counter) => name(counter),
    };
    let word = word.strip_suffix("Counter").unwrap_or(&word).to_string();
    // The source renames Sensei Golden-Tail's training counter so that it can't be
    // read as the keyword; Oracle text, which Cantrip prints, still says training.
    let word = if word == "Samurai" {
        "Training".to_string()
    } else {
        word
    };
    if crate::cantrip_vocab::COUNTERS.contains(&word.as_str()) {
        Ok(format!("CounterKind::{word}"))
    } else {
        Ok(planned::planned_value(
            &format!("counter kind {word}"),
            "CounterKind::Charge",
            &format!("CounterKind::{word}"),
        ))
    }
}

/// What the engine records about an object. A state it keeps no record of is a
/// gap: reading anything else would look finished and mean something different.
fn object_state(state: &ObjectState, id: &str) -> Option<String> {
    use ObjectState as S;
    Some(match state {
        S::FaceDown => format!("q.state().obj({id}).face_down"),
        S::Monstrous => format!("q.state().obj({id}).marks.monstrous"),
        S::Transformed => format!("q.state().obj({id}).transformed"),
        S::Prepared => format!("q.state().obj({id}).prepared"),
        S::Suspected => format!(
            "q.state().effects.iter().any(|e| e.affected == {id} && e.kind == EffectKind::Suspected)"
        ),
        S::Blocked => format!("q.state().attackers.iter().any(|a| a.id == {id} && a.blocked)"),
        S::Unblocked => format!("q.state().attackers.iter().any(|a| a.id == {id} && !a.blocked)"),
        S::AttackingAlone => format!(
            "(q.state().attackers.len() == 1 && q.state().attackers.iter().any(|a| a.id == {id}))"
        ),
        S::Damaged => format!("q.state().obj({id}).damage > 0"),
        _ => return None,
    })
}

impl Compiler {
    /// `SOURCE_CARD`, or for a face an object's `card` doesn't name, that face: a
    /// transformed permanent's `card` is its front and a Room's is the whole Room,
    /// and neither lists what a back face or a door prints.
    pub(super) fn source_card(&self) -> &'static str {
        match (self.own_face, self.reader) {
            (false, _) => SOURCE_CARD,
            (true, Reader::Layer) => "q.cards().current(q.state().obj(asker.source))",
            (true, _) => "q.card_id(&CARD)",
        }
    }
    /// A read made while resolving, where `g` is the only reader.
    pub(super) fn fetch(&self, code: impl AsRef<str>) -> String {
        format!("{{ let q: &dyn Query = g; {} }}", code.as_ref())
    }
    pub(super) fn carried(&self, carried: Carried) -> Result<(String, String)> {
        let (subject, data) = match self.event_at {
            // The asking matcher's locals, past the record's of the same names.
            EventAt::Matcher if self.history.is_some() => {
                ("outer_subject".to_string(), "outer_data".to_string())
            }
            EventAt::Matcher => ("subject".to_string(), "data".to_string()),
            EventAt::Item => ("item.subject".into(), "item.data".into()),
            EventAt::TargetFilter => ("q.event_subject()".into(), String::new()),
            EventAt::Entering => ("Target::Object(asker.source)".into(), String::new()),
            EventAt::Nowhere => return Err(gap("event value outside its ability")),
        };
        match carried {
            Carried::Subject => Ok((subject, data)),
            Carried::Data(_) | Carried::Position(_) if data.is_empty() => Ok((
                subject,
                planned::planned_value(
                    "event value in a target filter beyond its subject",
                    "[u32::MAX; 2]",
                    "q.event_data()",
                ),
            )),
            Carried::Data(_) | Carried::Position(_) => Ok((subject, data)),
        }
    }
    pub(super) fn carried_object(&self, carried: Option<Carried>) -> Result<String> {
        // What is entering is what a replacement of its entering is about.
        if let (EventAt::Nowhere, None, Some(Carried::Subject), Some(object)) =
            (self.event_at, &self.outer, carried, &self.event_object)
        {
            return self.object(object);
        }
        let entering = (self.event_at == EventAt::Entering).then_some(Carried::Subject);
        let carried = carried
            .or(entering)
            .ok_or_else(|| gap("event object this trigger does not bind"))?;
        let (subject, data) = self.carried(carried)?;
        Ok(match carried {
            Carried::Subject => format!("{subject}.object()"),
            Carried::Data(i) => {
                format!("Some(ObjectId({data}[{i}])).filter(|o| *o != ObjectId::NONE)")
            }
            Carried::Position(_) => return Err(gap("event object this trigger does not bind")),
        })
    }
    /// `Target`: the ability on the stack an activation event is about.
    pub(super) fn carried_ability(&self) -> Result<String> {
        let carried = self.frame.activated;
        let carried = carried.ok_or_else(|| gap("event object this trigger does not bind"))?;
        let (subject, data) = self.carried(carried)?;
        match carried {
            Carried::Data(i) => Ok(format!("Target::Ability({data}[{i}])")),
            Carried::Position(i) => Ok(self.fetch(format!(
                "Target::Ability(q.state().stack.get(({data}[{i}] >> 8) as usize).map_or(u32::MAX, |e| e.serial))"
            ))),
            Carried::Subject => Ok(subject.to_string()),
        }
    }
    pub(super) fn carried_player(&self, carried: Option<Carried>) -> Result<String> {
        let carried = carried.ok_or_else(|| gap("event player this trigger does not bind"))?;
        if self.event_at == EventAt::TargetFilter
            && self.frame.subject_controller == Some(carried)
        {
            return Ok("q.event_subject().object().map(|o: ObjectId| q.controller(o))".into());
        }
        let (subject, data) = self.carried(carried)?;
        Ok(match carried {
            Carried::Subject => format!("{subject}.player()"),
            Carried::Data(i) => format!("Some(PlayerId({data}[{i}] as u8))"),
            Carried::Position(_) => return Err(gap("event player this trigger does not bind")),
        })
    }
    /// `Option<Target>` of a target announced ahead of the slot whose filter is
    /// being read, which the engine lists without saying whose each is.
    pub(super) fn earlier_target(&self, slot: &Slot, index: u16) -> Result<Option<String>> {
        let Some(choosing) = self.choosing else {
            return Ok(None);
        };
        let referred = match self.slots.get(&slot.0) {
            Some((i, _)) if *i < choosing => *i,
            _ => return Err(gap("target reference to no earlier slot")),
        };
        let mut at = usize::from(index);
        for (i, spec) in self.slots.values() {
            if *i < referred {
                let Count::Exactly(Expr::Constant(n)) = &spec.count else {
                    return Ok(Some(planned::planned_value(
                        "target reference past a slot of no fixed size",
                        "None::<Target>",
                        &format!("q.chosen_targets_of({referred}).get({index}).copied()"),
                    )));
                };
                at += *n as usize;
            }
        }
        Ok(Some(format!("q.chosen_targets().get({at}).copied()")))
    }
    fn is_first_slot(&self, slot: &Slot) -> bool {
        self.slots.get(&slot.0).is_some_and(|(i, _)| *i == 0)
    }
    pub(super) fn slot(&self, slot: &Slot) -> Result<usize> {
        if self.reader != Reader::Resolution || !self.stack_item {
            return Err(gap("target reference outside a resolving ability"));
        }
        self.slots
            .get(&slot.0)
            .map(|(i, _)| *i)
            .ok_or_else(|| gap("target reference"))
    }
    /// `Option<ObjectId>`: a reference that finds nothing selects nothing.
    pub(super) fn object(&self, object: &ObjectRef) -> Result<String> {
        let carried = match self.outside_object(object) {
            Err(_) if self.target_gone(object).is_some() => None,
            carried => carried?,
        };
        if let Some(carried) = carried {
            return Ok(carried);
        }
        if let Some(stated) = self.target_gone(object) {
            return Ok(stated);
        }
        Ok(match object {
            ObjectRef::Primitive { name, .. } => {
                return Err(gap(format!("primitive object reference {name}")))
            }
            ObjectRef::Source => "Some(asker.source)".into(),
            // Inside a history question the event is what is being tested, and
            // the candidate is still the object the enclosing filter asks about.
            ObjectRef::Affected if self.affected_object.is_some() => {
                format!("Some({})", self.affected_object.unwrap_or_default())
            }
            ObjectRef::Candidate if self.history.is_some() => match &self.history {
                Some(History {
                    outer: Some(outer), ..
                }) => format!("Some({outer})"),
                _ => {
                    return Err(gap(
                        "candidate in a history question with no filter around it",
                    ))
                }
            },
            ObjectRef::Candidate => {
                let candidate = self.candidates.borrow();
                let candidate = candidate
                    .last()
                    .ok_or_else(|| gap("candidate outside a filter"))?;
                format!("Some({})", candidate.id)
            }
            ObjectRef::Affected if self.affected_in_scope => "Some(affected)".into(),
            ObjectRef::AttachedTo(host) => format!(
                "{}.and_then(|o: ObjectId| q.state().obj(o).attached_to)",
                self.object(host)?
            ),
            ObjectRef::Target { slot, index } if self.stack_slot(slot) => {
                return self.stack_object(slot, *index)
            }
            ObjectRef::Target { slot, index } if self.choosing.is_some() => format!(
                "{}.and_then(|t: Target| t.object())",
                self.earlier_target(slot, *index)?.expect("choosing")
            ),
            // What the cost of an activation is told: its targets, in order.
            ObjectRef::Target { slot, index } if self.costing_targets && slot.0 == 0 => {
                format!("targets.get({index}).copied().and_then(|t: Target| t.object())")
            }
            ObjectRef::Target { slot, index } => format!(
                "t_{}_raw.get({index}).copied().and_then(|t: Target| t.object())",
                self.slot(slot)?
            ),
            ObjectRef::RecipientObject { slot, index } => format!(
                "t_{}_raw.get({index}).copied().and_then(|t: Target| t.object())",
                self.slot(slot)?
            ),
            ObjectRef::EventSubject => self.carried_object(self.frame.object)?,
            ObjectRef::EventDestination if self.reads_replaced_move() => {
                let (subject, _) = self.carried(Carried::Subject)?;
                format!("{subject}.object().and_then(|o: ObjectId| q.state().obj(o).became)")
            }
            ObjectRef::EventDestination => self.carried_object(self.frame.destination)?,
            ObjectRef::EventOther => self.carried_object(self.frame.other)?,
            ObjectRef::EventStackObject
                if self.frame.stack_object.is_none() && self.frame.activated.is_some() =>
            {
                self.activated_object()?
            }
            ObjectRef::EventStackObject => self.carried_object(self.frame.stack_object)?,
            ObjectRef::BlockPair(BlockRole::Attacker) => {
                self.carried_object(self.frame.attacker)?
            }
            ObjectRef::BlockPair(BlockRole::Blocker) => self.carried_object(self.frame.blocker)?,
            ObjectRef::BlockPair(BlockRole::Other) => self.carried_object(self.frame.other)?,
            ObjectRef::Damage(DamageSubject::Source) => {
                self.carried_object(self.frame.damage_source)?
            }
            ObjectRef::Damage(DamageSubject::Recipient) => {
                self.carried_object(self.frame.damage_recipient)?
            }
            ObjectRef::Bound(binding) if self.bound_objects.contains(&binding.0) => {
                format!("Some(bound_{})", binding.0)
            }
            ObjectRef::Query { objects, index } => {
                // "That card" after a reveal is the match, not the top card, and
                // there is none when the library ran out, so read the matches alone.
                let matches = match objects.as_ref() {
                    ObjectSet::Observed(result) => self.reveal_matches.get(&result.0),
                    _ => None,
                };
                match matches {
                    Some(found) => format!("{found}.get({index}).copied()"),
                    None if lane_k::from_a_graveyard(objects) => format!(
                        "({}).into_iter().rev().nth({index} as usize)",
                        self.objects(objects)?
                    ),
                    None => format!("({}).get({index}).copied()", self.objects(objects)?),
                }
            }
            ObjectRef::Result {
                result,
                phase,
                index,
            } => {
                let list = if self.selections.contains_key(&result.0) {
                    self.selected(result)?
                } else if self.creations.contains_key(&result.0) {
                    self.created(result)?
                } else {
                    self.transition(result, phase)?
                };
                format!("({list}).get({index} as usize).copied()")
            }
            other => return self.object_rest(other),
        })
    }
    /// `Option<PlayerId>`, for the same reason as `object`.
    pub(super) fn player(&self, player: &PlayerRef) -> Result<String> {
        if let Some(carried) = self.outside_player(player)? {
            return Ok(carried);
        }
        Ok(match player {
            PlayerRef::Primitive { name, .. } => {
                return Err(gap(format!("primitive player reference {name}")))
            }
            PlayerRef::AbilityController => "Some(asker.controller)".into(),
            PlayerRef::Active => "Some(q.state().turn.active)".into(),
            PlayerRef::ControllerOf(object)
                if matches!(**object, ObjectRef::EventStackObject)
                    && self.frame.stack_controller.is_some() =>
            {
                self.carried_player(self.frame.stack_controller)?
            }
            PlayerRef::ControllerOf(object)
                if matches!(&**object, ObjectRef::Target { slot, .. } if self.stack_slot(slot)) =>
            {
                let ObjectRef::Target { slot, index } = &**object else {
                    unreachable!("just matched");
                };
                self.stack_controller(slot, *index)?
            }
            PlayerRef::ControllerOf(object) => format!(
                "{}.map(|o: ObjectId| q.controller(o))",
                lane_k::gone_or_not(self.object(object)?, object)
            ),
            PlayerRef::OwnerOf(object) => format!(
                "{}.map(|o: ObjectId| q.state().obj(o).owner)",
                lane_k::gone_or_not(self.object(object)?, object)
            ),
            PlayerRef::Target { slot, index } if self.choosing.is_some() => format!(
                "{}.and_then(|t: Target| t.player())",
                self.earlier_target(slot, *index)?.expect("choosing")
            ),
            // A player no longer a legal target is no one to act on (608.2b).
            PlayerRef::Target { slot, index } => {
                let i = self.slot(slot)?;
                format!("t_{i}_raw.get({index}).copied().filter(|t| t_{i}.contains(t)).and_then(|t: Target| t.player())")
            }
            PlayerRef::EventSubject => self.carried_player(self.frame.player)?,
            PlayerRef::EventOtherPlayer => self.carried_player(self.frame.other_player)?,
            PlayerRef::Bound(binding) if self.bound_players.contains(&binding.0) => {
                format!("Some(bound_{})", binding.0)
            }
            // The rules stop calling it the defending player once it has left
            // combat, but the attack it made still names one (CR 508.5).
            PlayerRef::DefendingPlayerOf(object) if self.reader != Reader::Layer => format!(
                "{}.and_then(|o: ObjectId| q.defending_player(o))",
                self.object(object)?
            ),
            PlayerRef::EnchantedBy(object) => format!(
                "{}.and_then(|o: ObjectId| q.state().obj(o).attached_player)",
                self.object(object)?
            ),
            PlayerRef::Chosen => self.chosen_value(ValueKind::Player)?,
            other => return self.player_rest(other),
        })
    }
    /// One player, where a card's text leaves no way for there to be none.
    pub(super) fn certain_player(&self, player: &PlayerRef) -> Result<String> {
        fn certain(c: &Compiler, object: &ObjectRef) -> bool {
            match object {
                ObjectRef::Source | ObjectRef::Candidate | ObjectRef::Bound(_) => true,
                // The object an effect is applied to, there as it reads.
                ObjectRef::Affected => c.affected_object.is_some() || c.affected_in_scope,
                ObjectRef::EventSubject => c.frame.object == Some(Carried::Subject),
                // A spell or ability that made a target of something, and
                // the source of damage, have a controller as they last existed.
                ObjectRef::EventStackObject => c.frame.stack_controller.is_some(),
                ObjectRef::Damage(DamageSubject::Source) => c.frame.damage_source.is_some(),
                ObjectRef::Target { slot, index } => c.slots.get(&slot.0).is_some_and(|(_, spec)| {
                    matches!(&spec.count, Count::Exactly(Expr::Constant(n)) if *n > i64::from(*index))
                }),
                _ => false,
            }
        }
        let is_certain = match player {
            PlayerRef::AbilityController | PlayerRef::Active | PlayerRef::Bound(_) => true,
            PlayerRef::Target { slot, index } => c_slot_is_fixed(self, slot, *index),
            PlayerRef::EventSubject => self.frame.player.is_some(),
            PlayerRef::EventOtherPlayer => self.frame.other_player.is_some(),
            PlayerRef::ControllerOf(object) | PlayerRef::OwnerOf(object) => certain(self, object),
            PlayerRef::Chosen => self.chosen_local(ValueKind::Player).is_ok(),
            PlayerRef::PlayerOrControllerOf(recipients) => match &**recipients {
                Recipients::Target { slot, index } => c_slot_is_fixed(self, slot, *index),
                Recipients::Targets(slot) => c_slot_is_fixed(self, slot, 0),
                _ => false,
            },
            PlayerRef::OneOf(players) => self.present.borrow().is_some() || self.is_picked(players),
            _ => false,
        };
        if is_certain {
            // Certain even of one that's no longer a legal target.
            let read = match player {
                PlayerRef::Target { slot, index } if self.choosing.is_none() => format!(
                    "t_{}_raw.get({index}).copied().and_then(|t: Target| t.player())",
                    self.slot(slot)?
                ),
                _ => self.player(player)?,
            };
            Ok(format!(
                "{read}.expect(\"a reference that always names a player\")"
            ))
        } else if self.present.borrow().is_some() {
            let read = self.player(player)?;
            let mut present = self.present.borrow_mut();
            let list = present.as_mut().expect("just seen");
            let local = format!("present_{}_{}", self.next, list.len());
            list.push((local.clone(), read));
            Ok(local)
        } else {
            Err(gap(format!("possibly absent player {}", form(player))))
        }
    }
    /// `Vec<ObjectId>`.
    pub(super) fn objects(&self, objects: &ObjectSet) -> Result<String> {
        let carried = match self.outside_objects(objects) {
            Err(_) if self.targets_gone(objects).is_some() => None,
            carried => carried?,
        };
        if let Some(carried) = carried {
            return Ok(carried);
        }
        if let Some(stated) = self.targets_gone(objects) {
            return Ok(stated);
        }
        Ok(match objects {
            ObjectSet::One(ObjectRef::Target { slot, .. }) | ObjectSet::Targets(slot)
                if self.stack_slot(slot) =>
            {
                return self.stack_objects(slot)
            }
            ObjectSet::One(object @ ObjectRef::Target { .. })
                if self.choosing.is_some() || self.costing_targets =>
            {
                format!("{}.into_iter().collect::<Vec<ObjectId>>()", self.object(object)?)
            }
            ObjectSet::Targets(slot) if self.choosing == Some(1) && self.is_first_slot(slot) => {
                "q.chosen_targets().iter().filter_map(|t| t.object()).collect::<Vec<ObjectId>>()"
                    .to_string()
            }
            ObjectSet::Targets(slot) if self.choosing.is_some() => {
                let choosing = self.choosing.expect("just asked");
                format!(
                    "{}.into_iter().filter_map(|t: Target| t.object()).collect::<Vec<ObjectId>>()",
                    self.chosen_in_slot(slot, choosing)?
                )
            }
            ObjectSet::One(ObjectRef::Target { slot, index }) => format!(
                "t_{i}_raw.get({index}).copied().filter(|t| t_{i}.contains(t)).and_then(|t: Target| t.object()).into_iter().collect::<Vec<_>>()",
                i = self.slot(slot)?
            ),
            ObjectSet::One(object) => {
                format!("{}.into_iter().collect::<Vec<ObjectId>>()", self.object(object)?)
            }
            ObjectSet::Targets(slot) => format!(
                "t_{}.iter().filter_map(|t| t.object()).collect::<Vec<ObjectId>>()",
                self.slot(slot)?
            ),
            ObjectSet::Battlefield(filter) => format!(
                "q.state().battlefield().iter().copied().filter(|&id| {}).collect::<Vec<ObjectId>>()",
                self.filter(filter, &Candidate::of("id"))?
            ),
            ObjectSet::Selected(result)
            | ObjectSet::Observed(result)
            | ObjectSet::Copies(result)
            | ObjectSet::Affected(result) => self.selected(result)?,
            ObjectSet::Unselected(result) if self.unchosen.contains_key(&result.0) => {
                format!("{}.clone()", self.unchosen[&result.0])
            }
            ObjectSet::CostObjects {
                payment: PaymentRef::Resolution(result),
                phase,
                ..
            } => self.transition(result, phase)?,
            ObjectSet::CostObjects {
                payment,
                component,
                phase: ObjectPhase::Before,
                ..
            } => self.cost_objects(payment, component)?,
            // What paid, as what it became.
            ObjectSet::CostObjects {
                payment,
                component,
                phase: ObjectPhase::After,
                ..
            } => {
                let paid = self.cost_objects(payment, component)?;
                // Read for what it was worth (608.2h): as it is there, or, a token
                // that stayed nowhere, as it last was.
                let then = match paid.starts_with("q.events()") {
                    true => "map(|o: ObjectId| q.state().obj(o).became.unwrap_or(o))",
                    false => "filter_map(|o: ObjectId| q.state().obj(o).became)",
                };
                format!("({paid}).into_iter().{then}.collect::<Vec<ObjectId>>()")
            }
            ObjectSet::Created(result) => self.created(result)?,
            ObjectSet::Transition { result, phase } => self.transition(result, phase)?,
            ObjectSet::Matching { objects, filter } => format!(
                "({}).into_iter().filter(|&id| {}).collect::<Vec<ObjectId>>()",
                self.objects(objects)?,
                self.filter(filter, &Candidate::of("id"))?
            ),
            ObjectSet::Difference { objects, excluding } => format!(
                "{{ let without: Vec<ObjectId> = {}; ({}).into_iter().filter(|id| !without.contains(id)).collect::<Vec<ObjectId>>() }}",
                self.objects(excluding)?,
                self.objects(objects)?
            ),
            ObjectSet::Union(sets) => {
                let mut code = String::from("{ let mut all: Vec<ObjectId> = Vec::new(); ");
                for set in sets {
                    write!(
                        code,
                        "for id in {} {{ if !all.contains(&id) {{ all.push(id); }} }} ",
                        self.objects(set)?
                    )
                    .unwrap();
                }
                code.push_str("all }");
                code
            }
            // What a move took from a zone: those objects are gone, and keep where they were.
            ObjectSet::InZone { objects, zone }
                if matches!(&**objects, ObjectSet::Transition { phase: ObjectPhase::Before, .. }) =>
            {
                format!(
                    "({}).into_iter().filter(|&id| q.state().obj(id).zone.kind() == {}).collect::<Vec<ObjectId>>()",
                    self.objects(objects)?,
                    zone_kind(zone)?
                )
            }
            ObjectSet::InZone { objects, zone } => format!(
                "({}).into_iter().filter(|&id| q.state().obj(id).became.is_none() && q.state().obj(id).zone.kind() == {}).collect::<Vec<ObjectId>>()",
                self.objects(objects)?,
                zone_kind(zone)?
            ),
            ObjectSet::Zone {
                scope,
                kind,
                filter,
            } => format!(
                "({}).into_iter().filter(|&id| {} && {}).collect::<Vec<ObjectId>>()",
                self.zone(scope)?,
                object_kind(kind)?,
                self.filter(filter, &Candidate::of("id"))?
            ),
            ObjectSet::Take {
                objects,
                count,
                from,
            } => {
                let count = self.number(count)?.amount(usize::MAX as i128, "usize")?;
                match from {
                    SequenceEnd::Top => format!(
                        "({}).into_iter().take({count}).collect::<Vec<ObjectId>>()",
                        self.objects(objects)?
                    ),
                    SequenceEnd::Bottom => format!(
                        "{{ let mut end = ({}).into_iter().rev().take({count}).collect::<Vec<ObjectId>>(); end.reverse(); end }}",
                        self.objects(objects)?
                    ),
                }
            }
            ObjectSet::LibraryTops(players) => format!(
                "({}).into_iter().filter_map(|p: PlayerId| q.state().zone(Zone::library(p)).first().copied()).collect::<Vec<ObjectId>>()",
                self.players(players)?
            ),
            ObjectSet::EventGroup if self.event_at == EventAt::Item && self.grouped => {
                "q.event_objects(item).to_vec()".into()
            }
            other => return self.objects_rest(other),
        })
    }
    /// `Vec<ObjectId>` of what paid part of a cost, as it was when it did. The
    /// engine keeps what an activation's cost chose of the battlefield, and
    /// what a spell sacrificed is in the history ahead of its cast.
    fn cost_objects(&self, payment: &PaymentRef, component: &[u32]) -> Result<String> {
        if self.reader != Reader::Resolution || !self.stack_item {
            if matches!(payment, PaymentRef::Activation) && self.activation_cost.is_some() {
                // A mana ability's effect runs as its cost is paid (605.3a), with
                // nothing between: what its one leaving part took left last, and
                // `old` reads as it last existed.
                let leaving = self.activation_cost.as_ref().map_or(0, |cost| {
                    let leaves = |part: &&CostPart| {
                        matches!(part, CostPart::Sacrifice { .. } | CostPart::Move { .. })
                    };
                    cost.parts.iter().filter(leaves).count()
                });
                if leaving == 1 && !self.stack_item && self.reader == Reader::Resolution {
                    return Ok("q.events().iter().rev().find_map(|r| match r.event { Event::ZoneChange { old, from, part: false, .. } if from.is(ZoneKind::Battlefield) => Some(old), _ => None }).into_iter().collect::<Vec<ObjectId>>()".into());
                }
                return Ok(planned::planned_value(
                    "cost objects outside a resolving ability",
                    "Vec::<ObjectId>::new()",
                    "q.paid_for(asker)",
                ));
            }
            return Err(gap("cost objects outside a resolving ability"));
        }
        let part = |cost: &'_ Cost| -> Result<CostPart> {
            let [index] = component else {
                return Err(gap("cost objects of a nested cost part"));
            };
            cost.parts
                .get(*index as usize)
                .cloned()
                .ok_or_else(|| gap("cost objects of no cost part"))
        };
        let chooses = |part: &CostPart| match part {
            CostPart::Sacrifice { choose, .. } => choose.is_some(),
            CostPart::ChangeTapState { selections, .. } => {
                selections.iter().any(|s| s.choose.is_some())
            }
            _ => false,
        };
        match payment {
            PaymentRef::Activation => {
                let cost = self
                    .activation_cost
                    .as_ref()
                    .ok_or_else(|| gap("cost objects outside an activation"))?;
                let paid = part(cost)?;
                if let CostPart::Sacrifice {
                    objects: ObjectSet::One(ObjectRef::Source),
                    choose: None,
                } = paid
                {
                    return Ok("vec![item.object]".into());
                }
                if !chooses(&paid) || cost.parts.iter().filter(|p| chooses(p)).count() != 1 {
                    return Ok(lane_g::paid_objects(component));
                }
                Ok("q.cost_objects(item).to_vec()".into())
            }
            PaymentRef::Casting { ability } => {
                let cost = self
                    .casting_costs
                    .get(ability)
                    .ok_or_else(|| gap("cost objects of no additional cost"))?;
                let paid = part(cost)?;
                let CostPart::Sacrifice {
                    choose: Some(Count::Exactly(Expr::Constant(n))),
                    ..
                } = paid
                else {
                    return Ok(lane_g::paid_objects(component));
                };
                Ok(format!("{{ let records = q.history().all(); let spell = q.history().original_spell(item.object); records.iter().rposition(|r| matches!(r.event, Event::Cast(c) if c.spell == spell)).map_or_else(Vec::new, |cast| records[..cast].iter().rev().filter_map(|r| match r.event {{ Event::ZoneChange {{ old, from, reason: ZoneReason::Sacrifice, .. }} if from.is(ZoneKind::Battlefield) => Some(old), _ => None }}).take({n}).collect::<Vec<ObjectId>>()) }}"))
            }
            PaymentRef::Resolution(result) => self.paid_at_resolution(result),
            other => Err(gap(format!("cost objects of {}", form(other)))),
        }
    }
    /// `Vec<ObjectId>` of one zone, each library top first.
    fn zone(&self, scope: &ZoneScope) -> Result<String> {
        let (zone, players) = match scope {
            ZoneScope::Shared(EventZone::Battlefield) => {
                return Ok("q.state().battlefield().to_vec()".into())
            }
            ZoneScope::Shared(EventZone::Stack) => {
                return Ok("q.state().zone(Zone::STACK).to_vec()".into())
            }
            ZoneScope::Shared(EventZone::Command) => {
                return Ok("q.state().zone(Zone::COMMAND).to_vec()".into())
            }
            ZoneScope::Shared(zone) => {
                (zone, "q.state().seats().collect::<Vec<PlayerId>>()".into())
            }
            ZoneScope::Players { zone, players } => (zone, self.players(players)?),
        };
        let per_player = match zone {
            EventZone::Graveyard => "graveyard",
            EventZone::Hand => "hand",
            EventZone::Library => "library",
            EventZone::Exile => "exile",
            EventZone::PlanarDeck => {
                return Ok(planned::planned_value(
                    "planar deck",
                    NO_OBJECTS,
                    &format!(
                        "({players}).into_iter().flat_map(|p: PlayerId| q.planar_deck(p)).collect::<Vec<ObjectId>>()"
                    ),
                ))
            }
            _ => return Err(gap("zone that belongs to no player")),
        };
        Ok(format!(
            "({players}).into_iter().flat_map(|p: PlayerId| q.state().zone(Zone::{per_player}(p)).to_vec()).collect::<Vec<ObjectId>>()"
        ))
    }
    /// `Vec<PlayerId>`.
    pub(super) fn players(&self, players: &PlayerSet) -> Result<String> {
        Ok(match players {
            PlayerSet::One(player) => {
                format!("{}.into_iter().collect::<Vec<PlayerId>>()", self.player(player)?)
            }
            PlayerSet::All => "q.state().apnap().collect::<Vec<PlayerId>>()".into(),
            PlayerSet::OpponentsOf(player) => format!(
                "{}.into_iter().flat_map(|p: PlayerId| q.state().opponents(p)).collect::<Vec<PlayerId>>()",
                self.player(player)?
            ),
            PlayerSet::OthersThan(player) => format!(
                "{{ let other = {}; q.state().apnap().filter(|p| Some(*p) != other).collect::<Vec<PlayerId>>() }}",
                self.player(player)?
            ),
            PlayerSet::Targets(slot) => format!(
                "t_{}.iter().filter_map(|t| t.player()).collect::<Vec<PlayerId>>()",
                self.slot(slot)?
            ),
            PlayerSet::Union(sets) => {
                let mut code = String::from("{ let mut all: Vec<PlayerId> = Vec::new(); ");
                for set in sets {
                    write!(
                        code,
                        "for p in {} {{ if !all.contains(&p) {{ all.push(p); }} }} ",
                        self.players(set)?
                    )
                    .unwrap();
                }
                code.push_str("all }");
                code
            }
            PlayerSet::Intersection(sets) => {
                let Some((first, rest)) = sets.split_first() else {
                    return Err(gap("intersection of no player sets"));
                };
                let mut code = format!("{{ let mut all = {}; ", self.players(first)?);
                for set in rest {
                    write!(
                        code,
                        "let other = {}; all.retain(|p: &PlayerId| other.contains(p)); ",
                        self.players(set)?
                    )
                    .unwrap();
                }
                code.push_str("all }");
                code
            }
            PlayerSet::Matching { binding, condition } => {
                let mut inner = self.clone();
                inner.bound_players.insert(binding.0);
                format!(
                    "q.state().apnap().filter(|&bound_{}: &PlayerId| {}).collect::<Vec<PlayerId>>()",
                    binding.0,
                    inner.condition(condition)?
                )
            }
            other => return self.players_stated(other),
        })
    }
    /// Whether the player `who` names is in the set.
    pub(super) fn player_in(&self, players: &PlayerSet, who: &str) -> Result<String> {
        Ok(match players {
            PlayerSet::All => "true".into(),
            PlayerSet::One(player) => format!("{} == Some({who})", self.player(player)?),
            other => format!("({}).contains(&{who})", self.players(other)?),
        })
    }
    pub(super) fn property(&self, id: &str, property: &Property) -> Result<Number> {
        let pt = |field: &str| Number {
            code: format!("i64::from(q.chars({id}).{field}.unwrap_or(0))"),
            min: i16::MIN.into(),
            max: i16::MAX.into(),
            dynamic: true,
        };
        Ok(match property {
            Property::Power => pt("power"),
            Property::Toughness => pt("toughness"),
            // API costs hold u8 counts and obj.x is u16. This bound includes all
            // mana fields, hybrid and X, even after leaving the stack; it is
            // strictly below i32::MAX.
            Property::ManaValue => Number {
                code: match self.reader {
                    Reader::Layer => format!("i64::from(q.mana_value({id}, &q.chars({id})))"),
                    _ => format!("i64::from(q.mana_value({id}))"),
                },
                min: 0,
                max: 255 * (1 + 6 + 10 + 5) + 6 + 255 * i128::from(u16::MAX),
                dynamic: true,
            },
            Property::Loyalty => Number {
                code: format!("i64::from(q.state().obj({id}).counters(CounterKind::Loyalty))"),
                min: i16::MIN.into(),
                max: i16::MAX.into(),
                dynamic: true,
            },
            Property::SpellX => Number {
                code: format!("i64::from(q.state().obj({id}).x)"),
                min: 0,
                max: u16::MAX.into(),
                dynamic: true,
            },
            Property::ColorCount => Number {
                code: format!("q.chars({id}).colors.iter().count() as i64"),
                min: 0,
                max: 5,
                dynamic: true,
            },
            other => return lane_j::property_rest(other, id),
        })
    }
    pub(super) fn number(&self, n: &Expr) -> Result<Number> {
        let count = |code: String| Number {
            code,
            min: 0,
            max: u32::MAX.into(),
            dynamic: true,
        };
        if let Expr::CountObjects(objects)
        | Expr::Aggregate { objects, .. }
        | Expr::CountDistinct { objects, .. } = n
        {
            if let Some(code) = self.outside_count(n, objects)? {
                return Ok(Number {
                    code,
                    min: 0,
                    max: u16::MAX.into(),
                    dynamic: true,
                });
            }
        }
        Ok(match n {
            Expr::Primitive { name, .. } => {
                return Err(gap(format!("primitive numeric expression {name}")))
            }
            Expr::Constant(n) => Number::integer(*n),
            Expr::AnnouncedX
                if self.reader == Reader::Resolution
                    && self.stack_item
                    && (self.outer.is_none() || self.counted.is_some()) =>
            {
                Number {
                    code: "i64::from(item.x)".into(),
                    min: 0,
                    max: u16::MAX.into(),
                    dynamic: true,
                }
            }
            Expr::AnnouncedX if self.pending == Pending::Entering => Number {
                code: "i64::from(entering.x)".into(),
                min: 0,
                max: u16::MAX.into(),
                dynamic: true,
            },
            Expr::CountManaSymbols { objects, colors }
                if self.reader != Reader::Layer
                    && matches!(colors.as_slice(), [ManaColor::Chosen]) =>
            {
                self.devotion_to_chosen(objects)?
            }
            Expr::CountManaSymbols { objects, colors } if self.reader != Reader::Layer => {
                let (
                    ObjectSet::Battlefield(ObjectFilter::ControlledBy(PlayerSet::One(player))),
                    [c],
                ) = (&**objects, colors.as_slice())
                else {
                    return Ok(count(self.mana_symbols(objects, colors)?));
                };
                count(format!(
                    "{}.map_or(0i64, |p: PlayerId| i64::from(cantrip_mechanics::devotion::devotion(q, p, Color::{})))",
                    self.player(player)?,
                    color(c)?
                ))
            }
            Expr::CastX(ObjectRef::Source | ObjectRef::EventSubject)
                if self.pending == Pending::Entering =>
            {
                Number {
                    code: "i64::from(entering.x)".into(),
                    min: 0,
                    max: u16::MAX.into(),
                    dynamic: true,
                }
            }
            Expr::CastX(object) => Number {
                code: format!(
                    "{}.map_or(0i64, |o: ObjectId| i64::from(q.state().obj(o).x))",
                    self.object(object)?
                ),
                min: 0,
                max: u16::MAX.into(),
                dynamic: true,
            },
            Expr::EventAmount
                if self.frame.members && self.grouped && self.event_at == EventAt::Item =>
            {
                count("q.event_objects(item).len() as i64".into())
            }
            // "That many" of an event the engine reports without the number, or of
            // several events the ability is one answer to.
            Expr::EventAmount if self.frame.amount.is_none() && self.event_at == EventAt::Item => {
                count(planned::planned_value(
                    "amount of an event its trigger does not carry",
                    "0i64",
                    "q.event_total(item)",
                ))
            }
            // Read as the ability triggers: its announcement, or "if you roll a 6".
            Expr::EventAmount
                if self.frame.amount.is_none()
                    && self.frame != Frame::default()
                    && matches!(self.event_at, EventAt::Matcher | EventAt::TargetFilter) =>
            {
                count(planned::planned_value(
                    "amount of an event its trigger does not carry",
                    "0i64",
                    "q.event_total_at(asker)",
                ))
            }
            Expr::EventAmount => {
                let (_, data) = self.carried(
                    self.frame.amount.ok_or_else(|| {
                        gap("event amount this trigger does not bind")
                    })?,
                )?;
                let Some(Carried::Data(i)) = self.frame.amount else {
                    return Err(gap("event amount outside the event data"));
                };
                count(format!("i64::from({data}[{i}])"))
            }
            Expr::EventHistory {
                event,
                window,
                measure,
            } => {
                let scope = match window {
                    HistoryWindow::ThisTurn => "q.history().this_turn().iter()",
                    HistoryWindow::ThisGame => "q.history().all().iter()",
                    HistoryWindow::ThisCombat => "cantrip_mechanics::history::this_combat(q).iter()",
                    HistoryWindow::LastTurn => {
                        "q.history().all().iter().filter(|r| r.turn + 1 == q.state().turn.number)"
                    }
                    other => &self.history_window_to_come(other)?,
                };
                if let (HistoryWindow::ThisTurn, HistoryMeasure::Occurrences, Some(entered)) =
                    (window, measure, self.entered_this_turn(event)?)
                {
                    return Ok(count(entered));
                }
                let mut asked = self.clone();
                asked.history = Some(History {
                    outer: self.candidates.borrow().last().map(|c| c.id.clone()),
                    event_at: self.event_at,
                    frame: self.frame,
                });
                let matched = asked.event(event)?;
                let hits = format!("{scope}.filter_map({})", matched.history_test());
                let answer = match measure {
                    HistoryMeasure::Occurrences => format!("{hits}.count() as i64"),
                    HistoryMeasure::TotalAmount
                        if matched.frame.amount == Some(Carried::Data(0)) =>
                    {
                        format!("{hits}.map(|f: Fired| i64::from(f.data[0])).sum::<i64>()")
                    }
                    other => lane_p::history_measure(other, &hits),
                };
                // The event is one the engine will have: the question is stated.
                let answer = match matched.planned.first() {
                    Some(feature) => planned::planned_value(feature, "0i64", &answer),
                    None => answer,
                };
                count(if self.event_at == EventAt::Matcher {
                    format!("{{ let (outer_subject, outer_data) = (subject, data); {answer} }}")
                } else {
                    answer
                })
            }
            Expr::ManaSpent {
                object,
                filter,
                aggregation,
            } => {
                if filter.snow {
                    // Who pays chooses which mana (CR 601.2h), but only a permanent's
                    // rule tells mana apart by its source: not the spell being paid for.
                    return Ok(count(planned::planned_value(
                        "engine: snow mana told apart as a spell is paid for",
                        "0i64",
                        &format!(
                            "{}.map_or(0, |o: ObjectId| q.history().mana_spent_to_cast(o).filter(|(source, _)| *source != ObjectId::NONE && q.chars(*source).supertypes.snow).map(|(_, mana)| i64::from(mana.total())).sum::<i64>())",
                            self.object(object)?
                        ),
                    )));
                }
                let kinds = match &filter.colors {
                    None => "[Mana::W, Mana::U, Mana::B, Mana::R, Mana::G, Mana::C]".to_string(),
                    Some(colors) => format!(
                        "[{}]",
                        colors
                            .iter()
                            .map(|c| color_index(c)
                                .map(|i| format!("Mana::{}", ["W", "U", "B", "R", "G", "C"][i])))
                            .collect::<Result<Vec<_>>>()?
                            .join(", ")
                    ),
                };
                let colored = "filter(|mana| *mana != Mana::C)";
                let read = match aggregation {
                    SpentManaAggregation::Count => format!("{kinds}.into_iter().map(|mana| i64::from(spent.get(mana))).sum::<i64>()"),
                    SpentManaAggregation::DistinctColors => format!("{kinds}.into_iter().{colored}.filter(|mana| spent.get(*mana) > 0).count() as i64"),
                    SpentManaAggregation::MaximumPerColor => format!("{kinds}.into_iter().{colored}.map(|mana| i64::from(spent.get(mana))).max().unwrap_or(0)"),
                };
                count(format!(
                    "{}.map_or(0, |o: ObjectId| {{ let spent = q.obj(o).spent; {read} }})",
                    self.object(object)?
                ))
            }
            // "Discard up to two cards. If you do, draw that many": as many as went.
            Expr::CountObjects(objects)
                if lane_p::paid_by_moving(objects).is_some_and(|result| {
                    !self.before.contains_key(&result.0) && self.moved.contains_key(&result.0)
                }) =>
            {
                let result = lane_p::paid_by_moving(objects).expect("just seen");
                count(format!("({}.len() as i64)", self.moved[&result.0]))
            }
            Expr::CountObjects(objects) if lane_k::convoked(objects).is_some() => count(format!(
                "{}.map_or(0, |o: ObjectId| q.history().convoked(o).count() as i64)",
                self.object(lane_k::convoked(objects).expect("just asked"))?
            )),
            Expr::CountObjects(objects) => {
                count(format!("({}).len() as i64", self.objects(objects)?))
            }
            Expr::CountDistinct { objects, property } => {
                let objects = self.objects(objects)?;
                let gather = |empty: &str, each: &str| {
                    format!("{{ let mut seen = {empty}; for id in {objects} {{ {each} }} seen.iter().count() as i64 }}")
                };
                let list = |value: &str| {
                    format!("{{ let mut seen = Vec::new(); for id in {objects} {{ let value = {value}; if !seen.contains(&value) {{ seen.push(value); }} }} seen.len() as i64 }}")
                };
                let subtypes = |test: &str| {
                    gather(
                        "SubtypeSet::NONE",
                        &format!("for kind in q.chars(id).subtypes.iter().filter(|kind| {test}) {{ seen = seen.with(kind); }}"),
                    )
                };
                count(match property {
                    DistinctProperty::CardTypes => gather(
                        "TypeSet::NONE",
                        "for kind in q.chars(id).types.iter() { seen = seen.with(kind); }",
                    ),
                    DistinctProperty::Colors => gather(
                        "ColorSet::NONE",
                        "for color in q.chars(id).colors.iter() { seen = seen.with(color); }",
                    ),
                    DistinctProperty::BasicLandTypes => subtypes("kind.basic_mana().is_some()"),
                    DistinctProperty::CreatureTypes => {
                        subtypes("SubtypeSet::CREATURE_TYPES.has(*kind)")
                    }
                    DistinctProperty::LandTypes => subtypes("SubtypeSet::LAND_TYPES.has(*kind)"),
                    DistinctProperty::Names => list("q.chars(id).name"),
                    DistinctProperty::Controllers => list("q.controller(id)"),
                    DistinctProperty::Owners => list("q.state().obj(id).owner"),
                    DistinctProperty::Powers => list("q.chars(id).power"),
                    DistinctProperty::Toughness => list("q.chars(id).toughness"),
                    DistinctProperty::ManaValues if self.reader != Reader::Layer => {
                        list("q.mana_value(id)")
                    }
                    DistinctProperty::ManaValues => {
                        list(&format!("{}.mana_value()", self.cost_of("id", "q.chars(id)")))
                    }
                    // A card with no mana cost has no cost to differ (CR 202.1b).
                    DistinctProperty::ManaCosts if self.reader != Reader::Layer => format!(
                        "{{ let mut seen = Vec::new(); for id in {objects} {{ if q.chars(id).no_cost || !q.card(q.chars(id).card).has_cost {{ continue; }} let value = q.mana_cost(id); if !seen.contains(&value) {{ seen.push(value); }} }} seen.len() as i64 }}"
                    ),
                    DistinctProperty::PermanentTypes => gather(
                        "TypeSet::NONE",
                        "for kind in q.chars(id).types.iter().filter(|kind| matches!(kind, CardType::Artifact | CardType::Battle | CardType::Creature | CardType::Enchantment | CardType::Land | CardType::Planeswalker)) { seen = seen.with(kind); }",
                    ),
                    DistinctProperty::CounterTypes => format!(
                        "{{ let mut seen = Vec::new(); for id in {objects} {{ for kind in CounterKind::ALL {{ if q.state().obj(id).counters(kind) > 0 && !seen.contains(&kind) {{ seen.push(kind); }} }} }} seen.len() as i64 }}"
                    ),
                    DistinctProperty::PlaneswalkerTypes => planned::planned_value(
                        "distinct PlaneswalkerTypes",
                        "0i64",
                        &subtypes("kind.is_planeswalker_type()"),
                    ),
                    DistinctProperty::ColorPairs => planned::planned_value(
                        "distinct ColorPairs",
                        "0i64",
                        &format!("cantrip_mechanics::domain::color_pairs(q, &{objects})"),
                    ),
                    DistinctProperty::UnlockedDoorNames => planned::planned_value(
                        "distinct UnlockedDoorNames",
                        "0i64",
                        &format!("cantrip_mechanics::rooms::unlocked_door_names(q, &{objects})"),
                    ),
                    other => return Err(gap(format!("distinct {}", form(other)))),
                })
            }
            Expr::CountPlayers(players) => {
                count(format!("({}).len() as i64", self.players(players)?))
            }
            Expr::ObjectProperty {
                object,
                property,
                information: Information::CurrentOrLastKnown,
            } => {
                let read = self.property("o", property)?;
                Number {
                    code: format!(
                        "{}.map_or(0i64, |o: ObjectId| {})",
                        self.object(object)?,
                        read.code
                    ),
                    min: read.min.min(0),
                    max: read.max.max(0),
                    dynamic: true,
                }
            }
            Expr::Aggregate {
                objects,
                property,
                aggregation,
            } => {
                let read = self.property("o", property)?;
                let reduce = match aggregation {
                    Aggregation::Sum => "sum::<i64>()",
                    Aggregation::Maximum => "max().unwrap_or(0)",
                    Aggregation::Minimum => "min().unwrap_or(0)",
                    Aggregation::AnyOf => {
                        let code = format!(
                            "cantrip_mechanics::numbers::AnyOf(&({}).into_iter().map(|o: ObjectId| {}).collect::<Vec<i64>>())",
                            self.objects(objects)?,
                            read.code
                        );
                        return Ok(lane_j::held_number("comparison with any one value".to_string(), &code));
                    }
                };
                Number {
                    code: format!(
                        "({}).into_iter().map(|o: ObjectId| {}).{reduce}",
                        self.objects(objects)?,
                        read.code
                    ),
                    min: i64::MIN.into(),
                    max: i64::MAX.into(),
                    dynamic: true,
                }
            }
            Expr::CountCounters {
                recipients,
                kind,
                information: Information::CurrentOrLastKnown,
            } => {
                if let Recipients::Players(players) = recipients.as_ref() {
                    use crate::counter::{CounterKind, CounterName};
                    let read = match kind {
                        Some(CounterKind::Named(CounterName::PoisonCounter)) => "poison",
                        Some(CounterKind::Named(CounterName::ExperienceCounter)) => "experience",
                        _ => return Ok(count(self.counters_among(recipients, kind.as_ref())?)),
                    };
                    return Ok(count(format!(
                        "({}).into_iter().map(|p: PlayerId| i64::from(q.state().player(p).{read}())).sum::<i64>()",
                        self.players(players)?
                    )));
                }
                let Recipients::Objects(objects) = recipients.as_ref() else {
                    return Ok(count(self.counters_among(recipients, kind.as_ref())?));
                };
                count(format!(
                    "({}).into_iter().map(|o: ObjectId| {}).sum::<i64>()",
                    self.objects(objects)?,
                    counters_on("o", kind.as_ref())?
                ))
            }
            Expr::PlayerLife(p) => Number {
                code: format!(
                    "{}.map_or(0i64, |p: PlayerId| i64::from(q.state().player(p).life))",
                    self.player(p)?
                ),
                min: i32::MIN.into(),
                max: i32::MAX.into(),
                dynamic: true,
            },
            Expr::Performed(result) | Expr::CountersRemoved(result) | Expr::DieResult(result)
                if self.counted == Some(result.0) =>
            {
                Number {
                    code: match self.event_at {
                        EventAt::TargetFilter => planned::planned_value(
                            "a target filter reading the amount its ability was told",
                            "0i64",
                            "q.told_amount(asker)",
                        ),
                        _ => "i64::from(item.x)".into(),
                    },
                    min: 0,
                    max: u16::MAX.into(),
                    dynamic: true,
                }
            }
            Expr::Performed(result)
            | Expr::PerformedTotal(result)
            | Expr::SurveilKeptOnTop(result)
            | Expr::CountersRemoved(result) => {
                match (self.performed.get(&result.0), self.moved.get(&result.0)) {
                    (Some(local), _) => count(format!("({local} as i64)")),
                    (None, Some(moved)) => count(format!("({moved}.len() as i64)")),
                    _ if self.failed.contains(&result.0) => {
                        count(planned::unreported("0i64", result))
                    }
                    _ if self.prevented.contains(&result.0) => count(planned::planned_value(
                        "damage prevented by an effect of this ability",
                        "0i64",
                        "q.told_amount(asker)",
                    )),
                    _ if self.notes.read_as_own.contains(&result.0) => {
                        return Err(gap("an amount read by each player as their own"))
                    }
                    _ => return Err(gap("result this instruction does not report")),
                }
            }
            Expr::BoundValue(result) | Expr::DieResult(result) => {
                match self.values.get(&result.0) {
                    Some(local) => Number {
                        code: local.clone(),
                        min: i64::MIN.into(),
                        max: i64::MAX.into(),
                        dynamic: true,
                    },
                    None if self.failed.contains(&result.0) => Number {
                        code: planned::unreported("0i64", result),
                        min: i64::MIN.into(),
                        max: i64::MAX.into(),
                        dynamic: true,
                    },
                    None => return Err(gap("value outside generated scope")),
                }
            }
            Expr::Add(a, b) | Expr::Subtract(a, b) | Expr::Multiply(a, b) => {
                let a = self.number(a)?;
                let b = self.number(b)?;
                let (symbol, min, max) = match n {
                    Expr::Add(..) => ("+", a.min.checked_add(b.min), a.max.checked_add(b.max)),
                    Expr::Subtract(..) => ("-", a.min.checked_sub(b.max), a.max.checked_sub(b.min)),
                    _ => {
                        let values = [
                            a.min.checked_mul(b.min),
                            a.min.checked_mul(b.max),
                            a.max.checked_mul(b.min),
                            a.max.checked_mul(b.max),
                        ];
                        if values.iter().any(Option::is_none) {
                            (" * ", None, None)
                        } else {
                            let values = values.map(Option::unwrap);
                            (
                                "*",
                                values.iter().min().copied(),
                                values.iter().max().copied(),
                            )
                        }
                    }
                };
                let dynamic = a.dynamic || b.dynamic;
                let (min, max) = match (min, max) {
                    (Some(min), Some(max)) if min >= i64::MIN.into() && max <= i64::MAX.into() => {
                        (min, max)
                    }
                    _ if dynamic => {
                        // Saturating arithmetic: only a bound was out of range,
                        // and no reachable game state has these values.
                        let method = match n {
                            Expr::Add(..) => "saturating_add",
                            Expr::Subtract(..) => "saturating_sub",
                            _ => "saturating_mul",
                        };
                        return Ok(Number {
                            code: format!("({}).{method}({})", a.code, b.code),
                            min: i64::MIN.into(),
                            max: i64::MAX.into(),
                            dynamic,
                        });
                    }
                    _ => return Err(gap("arithmetic exceeds i64")),
                };
                Number {
                    code: format!("({} {symbol} {})", a.code, b.code),
                    min,
                    max,
                    dynamic,
                }
            }
            Expr::Minimum(a, b) | Expr::Maximum(a, b) => {
                let (a, b) = (self.number(a)?, self.number(b)?);
                let minimum = matches!(n, Expr::Minimum(..));
                Number {
                    code: format!(
                        "({}).{}({})",
                        a.code,
                        if minimum { "min" } else { "max" },
                        b.code
                    ),
                    min: if minimum {
                        a.min.min(b.min)
                    } else {
                        a.min.max(b.min)
                    },
                    max: if minimum {
                        a.max.min(b.max)
                    } else {
                        a.max.max(b.max)
                    },
                    dynamic: a.dynamic || b.dynamic,
                }
            }
            Expr::Conditional {
                condition,
                then,
                otherwise,
            } => {
                let (a, b) = (self.number(then)?, self.number(otherwise)?);
                Number {
                    code: format!(
                        "(if {} {{ {} }} else {{ {} }})",
                        self.condition(condition)?,
                        a.code,
                        b.code
                    ),
                    min: a.min.min(b.min),
                    max: a.max.max(b.max),
                    dynamic: true,
                }
            }
            Expr::Divide {
                numerator,
                denominator,
                rounding,
            } => {
                let Expr::Constant(d) = denominator.as_ref() else {
                    return Err(gap("division by a variable"));
                };
                if *d <= 0 {
                    return Err(gap("division by a nonpositive constant"));
                }
                let a = self.number(numerator)?;
                let code = match rounding {
                    Rounding::Down => format!("({}).div_euclid({d})", a.code),
                    Rounding::Up => {
                        format!("({}).saturating_add({}).div_euclid({d})", a.code, d - 1)
                    }
                };
                Number {
                    code,
                    min: a.min.min(0),
                    max: a.max.max(0),
                    dynamic: true,
                }
            }
            Expr::PartySize(player) => {
                let party = self.party("player")?;
                Number {
                    code: format!(
                        "{}.map_or(0, |player: PlayerId| {party})",
                        self.player(player)?
                    ),
                    min: 0,
                    max: 4,
                    dynamic: true,
                }
            }
            Expr::StartingLife(player) => Number {
                code: format!(
                    "{}.map_or(0, |player: PlayerId| i64::from(q.state().player(player).starting_life))",
                    self.player(player)?
                ),
                min: i32::MIN.into(),
                max: i32::MAX.into(),
                dynamic: true,
            },
            other => return self.number_rest(other),
        })
    }
    /// A boolean over `t: &Target`: whether a spell targeting it targets these.
    pub(super) fn targeted(&self, recipients: &Recipients) -> Result<String> {
        Ok(match recipients {
            Recipients::Objects(objects) => format!(
                "t.object().is_some_and(|o| ({}).contains(&o))",
                self.objects(objects)?
            ),
            Recipients::Players(players) => format!(
                "matches!(t, Target::Player(p) if ({}).contains(p))",
                self.players(players)?
            ),
            Recipients::Union(items) => items
                .iter()
                .map(|item| self.targeted(item))
                .collect::<Result<Vec<_>>>()?
                .join(" || "),
            other => return Err(gap(format!("spell that targets {}", form(other)))),
        })
    }
    /// A boolean over `candidate`, the object `ObjectRef::Candidate` names.
    pub(super) fn filter(&self, f: &ObjectFilter, candidate: &Candidate) -> Result<String> {
        if let Some(either) = lane_p::any_of_filter(f) {
            return self.filter(&either, candidate);
        }
        self.candidates.borrow_mut().push(candidate.clone());
        let compiled = self.predicate(f, candidate);
        self.candidates.borrow_mut().pop();
        compiled
    }
    fn predicate(&self, f: &ObjectFilter, c: &Candidate) -> Result<String> {
        let (id, chars) = (c.id.as_str(), c.chars.as_str());
        let combine = |items: &[ObjectFilter], join: &str, identity: &str| -> Result<String> {
            if items.is_empty() {
                return Ok(identity.into());
            }
            Ok(format!(
                "({})",
                items
                    .iter()
                    .map(|x| self.predicate(x, c))
                    .collect::<Result<Vec<_>>>()?
                    .join(join)
            ))
        };
        Ok(match f {
            ObjectFilter::Primitive { name, .. } => {
                return Err(gap(format!("primitive object filter {name}")))
            }
            ObjectFilter::Any => "true".into(),
            ObjectFilter::All(items) => combine(items, " && ", "true")?,
            ObjectFilter::Either(items) => combine(items, " || ", "false")?,
            ObjectFilter::Not(p) => format!("!({})", self.predicate(p, c)?),
            ObjectFilter::TargetsAny(recipients) => {
                let wanted = self.targeted(recipients)?;
                match c.announced.as_deref() {
                    None => format!("q.state().stack.iter().find(|i| i.kind == StackKind::Spell && i.object == {id}).is_some_and(|i| q.state().targets(i).iter().any(|t| {wanted}))"),
                    Some("") => return Err(gap("targets of a spell as it is cast")),
                    Some(list) => format!("{list}.iter().any(|t| {wanted})"),
                }
            }
            ObjectFilter::CardType(t) => format!("{chars}.is(CardType::{})", card_type(t)?),
            ObjectFilter::Supertype(t) => format!(
                "{chars}.supertypes.{}",
                match name(t).as_str() {
                    "Basic" => "basic",
                    "Legendary" => "legendary",
                    "Snow" => "snow",
                    other => {
                        return Ok(planned::planned_value(
                            &format!("supertype {other}"),
                            "false",
                            &format!("{chars}.supertypes.{}", other.to_lowercase()),
                        ))
                    }
                }
            ),
            ObjectFilter::Subtype(t) => {
                let n = name(t);
                if !crate::cantrip_vocab::SUBTYPES.contains(&n.as_str()) {
                    return Err(gap(format!("subtype {n}")));
                }
                format!("{chars}.has_subtype(q.words(asker.source).subtype(Subtype::{n}))")
            }
            ObjectFilter::Color(c) => format!(
                "{chars}.colors.has(q.words(asker.source).color(Color::{}))",
                color(c)?
            ),
            ObjectFilter::Colorless => format!("{chars}.colors.is_empty()"),
            ObjectFilter::Chosen(kind) => self.chosen_filter(*kind, chars, id)?,
            ObjectFilter::ControlledBy(players) => self.player_in(players, &c.controller)?,
            ObjectFilter::OwnedBy(players) => {
                self.player_in(players, &format!("q.state().obj({id}).owner"))?
            }
            // Always true of what it tests: it can only mean something outside the
            // filter, which `Candidate` doesn't name.
            ObjectFilter::SameObject(ObjectRef::Candidate)
            | ObjectFilter::Shares {
                with: ObjectRef::Candidate,
                ..
            } if self.history.is_none() => {
                return Err(gap("filter comparing its candidate with itself"));
            }
            // "Another target card": what the event moved, each card of a merged
            // permanent (CR 730.3c), found from the subject a target filter reads.
            ObjectFilter::SameObject(object)
                if self.event_at == EventAt::TargetFilter
                    && self.frame.destination.is_some()
                    && lane_k::what_the_event_moved(object) =>
            {
                format!(
                    "q.event_subject().object().is_some_and(|o: ObjectId| o == {id} || q.found_all_after_move(o).contains(&{id}))"
                )
            }
            ObjectFilter::SameObject(object) => {
                format!("{} == Some({id})", self.object(object)?)
            }
            ObjectFilter::Tapped => format!("q.state().obj({id}).tapped"),
            ObjectFilter::Token => format!("q.state().obj({id}).token"),
            ObjectFilter::IsCard => {
                format!("!(q.state().obj({id}).token || q.state().obj({id}).card_copy)")
            }
            ObjectFilter::Attacking => {
                // Combat forgets a creature the moment it leaves (506.4).
                match self.reader {
                    Reader::Layer => format!("q.state().attackers.iter().any(|a| a.id == {id})"),
                    _ => format!("(q.state().attackers.iter().any(|a| a.id == {id}) || q.was_attacking({id}))"),
                }
            }
            ObjectFilter::Blocking => {
                format!("q.state().blocks.iter().any(|b| b.blocker == {id})")
            }
            ObjectFilter::HasKeyword(Keyword::Flanking) if self.reader != Reader::Layer => {
                format!("{chars}.abilities().any(|a| q.triggered_ability(a).is_some_and(|t| t.flanking))")
            }
            ObjectFilter::HasKeyword(k) => {
                format!("{chars}.keywords.has({})", keyword(k)?)
            }
            ObjectFilter::Name(words) => format!("{chars}.name == {}", quoted(words)),
            ObjectFilter::Property {
                property,
                comparison,
                value,
            } => {
                // Before X is announced, the filter allows what some X would.
                let open = matches!(value, Expr::AnnouncedX) && self.choosing.is_some();
                let value = if open {
                    "i64::from(x)".to_string()
                } else {
                    self.number(value)?.code
                };
                let side = |field: &str| match id {
                    // What an effect keeps watching may stop being a creature:
                    // then it has none, which compares as 0.
                    "e.affected" => compare(
                        &format!("i64::from({chars}.{field}.unwrap_or(0))"),
                        comparison,
                        &value,
                    ),
                    _ => format!(
                        "{chars}.{field}.is_some_and(|v| {})",
                        compare("i64::from(v)", comparison, &value)
                    ),
                };
                let test = match property {
                    // A card that has no power has no value to compare (208.3).
                    Property::Power => side("power"),
                    Property::Toughness => side("toughness"),
                    Property::ManaValue if self.reader == Reader::Layer => compare(
                        &format!("i64::from(q.mana_value({id}, &{chars}))"),
                        comparison,
                        &value,
                    ),
                    other => compare(&self.property(id, other)?.code, comparison, &value),
                };
                if open {
                    format!("q.announced_x().map_or(true, |x| {test})")
                } else {
                    test
                }
            }
            // A mana value is the object's, not its characteristics': a spell's holds its X
            // (CR 202.3e).
            ObjectFilter::Shares {
                property: DistinctProperty::ManaValues,
                with,
            } => format!(
                "{}.is_some_and(|o: ObjectId| q.mana_value({id}) == q.mana_value(o))",
                self.object(with)?
            ),
            ObjectFilter::Shares { property, with } => {
                let test = lane_j::sharing(property)?;
                format!(
                    "{}.is_some_and(|o: ObjectId| {{ let (a, b) = (&{chars}, &q.chars(o)); {test} }})",
                    self.object(with)?
                )
            }
            ObjectFilter::State(ObjectState::Kicked) => self.was_kicked("", &id),
            ObjectFilter::State(ObjectState::Commander) => lane_g::commander(&id),
            // An optional additional cost, or the waterbend kicker that stands for one.
            ObjectFilter::State(ObjectState::CostPaid(CastCost::Additional)) => {
                format!("(q.state().obj({id}).paid > 0 || q.state().obj({id}).kicked > 0)")
            }
            ObjectFilter::State(ObjectState::Saddled) => {
                format!("q.state().obj({id}).marks.saddled_turn == q.state().turn.number")
            }
            // Attachments are read as a layer may: by the state, and by what
            // the layers before it have made the attachment.
            ObjectFilter::State(state @ (ObjectState::Enchanted | ObjectState::Equipped)) => format!(
                "q.state().battlefield().iter().any(|&a| q.state().obj(a).attached_to == Some({id}) && q.chars(a).has_subtype(Subtype::{}))",
                if *state == ObjectState::Equipped { "Equipment" } else { "Aura" }
            ),
            ObjectFilter::State(state) => match object_state(state, id) {
                Some(read) => read,
                None => self.state_rest(state, c)?,
            },
            // Layers read the host only by which object it is.
            ObjectFilter::AttachedTo(host)
                if self.reader != Reader::Layer
                    || matches!(**host, ObjectFilter::SameObject(_)) =>
            {
                let test = self.filter(host, &Candidate::of("host"))?;
                format!("q.state().obj({id}).attached_to.is_some_and(|host| {test})")
            }
            ObjectFilter::BlockingObject(attacker) => format!(
                "{{ let attacker = {}; q.state().blocks.iter().any(|b| b.blocker == {id} && Some(b.attacker) == attacker) }}",
                self.object(attacker)?
            ),
            ObjectFilter::Modified if self.reader != Reader::Layer => format!("modified(q, {id})"),
            ObjectFilter::SharesNameWith(object) if self.reader != Reader::Layer => format!(
                "{}.is_some_and(|o: ObjectId| cantrip_mechanics::same_name(q, {id}, o))",
                self.object(object)?
            ),
            ObjectFilter::Cast { by, from } if self.reader != Reader::Layer => {
                // The history has it only once it became cast.
                if c.announced.is_some() {
                    return self.being_cast_from(by.as_ref(), from.as_ref(), c);
                }
                let mut tests = Vec::new();
                if let Some(by) = by {
                    tests.push(format!("{}.contains(&cast.player)", self.players(by)?));
                }
                match from {
                    None => {}
                    Some(ZoneScope::Shared(zone)) => {
                        tests.push(format!("cast.from == {}", zone_kind(zone)?));
                    }
                    Some(ZoneScope::Players { zone, players }) => {
                        tests.push(format!("cast.from == {}", zone_kind(zone)?));
                        let owner = "q.state().obj(cast.spell).owner";
                        tests.push(match players {
                            PlayerSet::All => "true".to_string(),
                            PlayerSet::One(PlayerRef::CasterOf(of))
                                if matches!(**of, ObjectRef::Candidate) =>
                            {
                                format!("{owner} == cast.player")
                            }
                            players => format!("{}.contains(&{owner})", self.players(players)?),
                        });
                    }
                }
                if tests.is_empty() {
                    tests.push("true".to_string());
                }
                format!(
                    "cast_of(q, {id}).is_some_and(|cast| {})",
                    tests.join(" && ")
                )
            }
            ObjectFilter::ExiledWith(ObjectRef::Source) if self.reader != Reader::Layer => {
                format!("q.exiled_with(q.linked_to(asker)).contains(&{id})")
            }
            ObjectFilter::Counters {
                kind,
                comparison,
                value,
            } => compare(
                &counters_on(id, kind.as_ref())?,
                comparison,
                &self.number(value)?.code,
            ),
            ObjectFilter::InZone(ZoneScope::Shared(zone)) => {
                format!(
                    "(q.state().obj({id}).became.is_none() && q.state().obj({id}).zone.kind() == {})",
                    zone_kind(zone)?
                )
            }
            ObjectFilter::InZone(ZoneScope::Players { zone, players }) => format!(
                "(q.state().obj({id}).zone.kind() == {} && {})",
                zone_kind(zone)?,
                self.player_in(players, &format!("q.state().obj({id}).owner"))?
            ),
            ObjectFilter::InSet(objects) => {
                format!("({}).contains(&{id})", self.objects(objects)?)
            }
            ObjectFilter::Condition(condition) => self.condition(condition)?,
            other => return self.predicate_rest(other, c),
        })
    }
    pub(super) fn condition(&self, condition: &Condition) -> Result<String> {
        let combine = |items: &[Condition], join: &str, identity: &str| -> Result<String> {
            if items.is_empty() {
                return Ok(identity.into());
            }
            Ok(format!(
                "({})",
                items
                    .iter()
                    .map(|x| self.condition(x))
                    .collect::<Result<Vec<_>>>()?
                    .join(join)
            ))
        };
        Ok(match condition {
            Condition::Primitive { name, .. } if name.starts_with(lane_j::CLASS_LEVEL) => {
                self.class_level(name)
            }
            Condition::Primitive { name, .. } if name == lane_j::CASE_SOLVED => self.case_solved(),
            Condition::Primitive { name, .. } => {
                return Err(gap(format!("primitive condition {name}")))
            }
            Condition::Searched(result) => match self.searched.get(&result.0) {
                Some(local) => local.clone(),
                None if self.failed.contains(&result.0) => planned::unreported("false", result),
                None => return Err(gap("condition Searched")),
            },
            Condition::All(items) => combine(items, " && ", "true")?,
            Condition::Either(items) => combine(items, " || ", "false")?,
            Condition::Not(c) => format!("!({})", self.condition(c)?),
            // A cost change is told the targets just announced (601.2f).
            Condition::TargetsMatch {
                object: ObjectRef::Source,
                kind,
                filter,
            } if self.costing_targets => {
                let zone = match kind {
                    ObjectKind::Permanent => "q.state().obj(id).zone.is(ZoneKind::Battlefield) && ",
                    ObjectKind::Spell => "q.state().obj(id).zone.is(ZoneKind::Stack) && ",
                    _ => "",
                };
                format!(
                    "targets.iter().filter_map(|t| t.object()).any(|id| {zone}{})",
                    self.filter(filter, &Candidate::of("id"))?
                )
            }
            Condition::ExistsPlayer {
                players,
                binding,
                condition,
            } => {
                let mut inner = self.clone();
                inner.bound_players.insert(binding.0);
                format!(
                    "({}).into_iter().any(|bound_{}: PlayerId| {})",
                    self.players(players)?,
                    binding.0,
                    inner.condition(condition)?
                )
            }
            Condition::PlayerIsActive(player) => {
                format!("{} == Some(q.state().turn.active)", self.player(player)?)
            }
            Condition::ObjectMatches {
                object,
                filter: ObjectFilter::State(ObjectState::Kicked),
            } => {
                let object = self.object(object)?;
                let test = self.was_kicked(&object, "candidate");
                format!("{object}.is_some_and(|candidate: ObjectId| {test})")
            }
            // "If the {1}{B} cost was paid": the one way of the card's that it was cast by.
            Condition::ObjectMatches {
                object: ObjectRef::Source,
                filter: ObjectFilter::AlternateCostPaid(_),
            } if self.stack_item && self.reader == Reader::Resolution => "item.way == 1".into(),
            Condition::ObjectMatches { object, filter } => format!(
                "{}.is_some_and(|candidate: ObjectId| {})",
                self.object(object)?,
                self.filter(filter, &Candidate::of("candidate"))?
            ),
            Condition::Controls {
                player,
                filter,
                minimum,
            } => format!(
                "q.state().battlefield().iter().copied().filter(|&id| q.controller(id) == {} && {}).count() >= {minimum}",
                format_args!("{}.unwrap_or(PlayerId(u8::MAX))", self.player(player)?),
                self.filter(filter, &Candidate::of("id"))?
            ),
            // "A 1 or 2": one comparison for each value.
            Condition::Compare {
                left,
                comparison,
                right: Expr::AnyOf(values),
            } => {
                let tests = values
                    .iter()
                    .map(|value| {
                        let number = self.number(left)?.code;
                        Ok(compare(&number, comparison, &self.number(value)?.code))
                    })
                    .collect::<Result<Vec<_>>>()?;
                format!("({})", tests.join(" || "))
            }
            Condition::Compare {
                left,
                comparison,
                right,
            } => compare(
                &self.number(left)?.code,
                comparison,
                &self.number(right)?.code,
            ),
            Condition::PaymentChosen(result) => match self.performed.get(&result.0) {
                Some(local) => format!("(({local} as i64) != 0)"),
                None if self.failed.contains(&result.0) => planned::unreported("false", result),
                None => return Err(gap("payment outside generated scope")),
            },
            Condition::CoinFlip { result, outcome } => {
                let local = match self.flips.get(&result.0) {
                    Some(local) => local.clone(),
                    None if self.failed.contains(&result.0) => {
                        planned::unreported("false", result)
                    }
                    None => return Err(gap("coin flip outside generated scope")),
                };
                match outcome {
                    CoinOutcome::Won | CoinOutcome::Heads => local,
                    CoinOutcome::Lost | CoinOutcome::Tails => format!("!{local}"),
                }
            }
            Condition::Window(window) => {
                let step = "(q.state().turn.step() as u8)";
                match window {
                    TurnWindow::BeforeAttackersDeclared => {
                        format!("{step} < (Step::DeclareAttackers as u8)")
                    }
                    TurnWindow::Combat => format!(
                        "((Step::BeginCombat as u8)..=(Step::EndOfCombat as u8)).contains(&{step})"
                    ),
                    TurnWindow::Upkeep => "q.state().turn.step() == Step::Upkeep".into(),
                    TurnWindow::EndStep => "q.state().turn.step() == Step::End".into(),
                    TurnWindow::BeforeCombatDamage => {
                        format!("{step} < (Step::CombatDamage as u8)")
                    }
                    TurnWindow::DeclareAttackers => {
                        "q.state().turn.step() == Step::DeclareAttackers".into()
                    }
                    TurnWindow::DeclareBlockers => {
                        "q.state().turn.step() == Step::DeclareBlockers".into()
                    }
                    TurnWindow::BeforeBlockersDeclared => {
                        format!("{step} < (Step::DeclareBlockers as u8)")
                    }
                    TurnWindow::FirstCombat => format!(
                        "({step} >= (Step::BeginCombat as u8) && {step} <= (Step::EndOfCombat as u8) && q.history().this_turn().iter().filter(|r| matches!(r.event, Event::StepBegan {{ step: Step::BeginCombat }})).count() == 1)"
                    ),
                    TurnWindow::AfterBlockersDeclared => {
                        format!("{step} > (Step::DeclareBlockers as u8)")
                    }
                    other => return lane_m::turn_window(other, step),
                }
            }
            Condition::PlayerState { player, state } => format!(
                "{}.is_some_and(|player: PlayerId| {})",
                self.player(player)?,
                self.player_state(state)?
            ),
            other => return self.condition_rest(other),
        })
    }
    /// Whether `player` is in this state.
    fn player_state(&self, state: &PlayerState) -> Result<String> {
        let active = "q.state().turn.active == player";
        let step = "q.state().turn.step()";
        Ok(match state {
            PlayerState::Monarch => "q.state().player(player).monarch".into(),
            PlayerState::CitysBlessing => "q.state().player(player).citys_blessing".into(),
            PlayerState::EnduringStory => "q.state().player(player).enduring_story".into(),
            PlayerState::Initiative => "q.state().player(player).initiative".into(),
            PlayerState::StartingPlayer => "q.state().turn.starting == player".into(),
            PlayerState::InTheirUpkeep => format!("{active} && {step} == Step::Upkeep"),
            PlayerState::InTheirMainPhase => format!(
                "{active} && matches!({step}, Step::PrecombatMain | Step::PostcombatMain)"
            ),
            PlayerState::HasBoon => planned::planned_value(
                "player state HasBoon",
                "false",
                "q.state().player(player).has_boon()",
            ),
            PlayerState::WholePoolInDeck => planned::planned_value(
                "player state WholePoolInDeck",
                "false",
                "q.state().player(player).whole_pool_in_deck()",
            ),
            PlayerState::ShuffledThisWay => planned::planned_value(
                "player state ShuffledThisWay",
                "false",
                "q.shuffled_this_way(asker, player)",
            ),
            PlayerState::ReceivedAVote => planned::planned_value(
                "player state ReceivedAVote",
                "false",
                "g.votes().received(player)",
            ),
            PlayerState::CouldMulligan => planned::planned_value(
                "player state CouldMulligan",
                "false",
                "q.state().could_mulligan(player)",
            ),
            PlayerState::PossibleDefender => planned::planned_value(
                "player state PossibleDefender",
                "false",
                "q.could_be_attacked(asker.controller, player)",
            ),
            PlayerState::RolledHighest => planned::planned_value(
                "player state RolledHighest",
                "false",
                "q.rolled_highest(asker, player)",
            ),
            PlayerState::CanDraw => planned::planned_value(
                "player state CanDraw",
                "false",
                "q.can_draw(player)",
            ),
            PlayerState::CompletedDungeon => {
                "q.history().any(EventQuery::kind(EventMask::DUNGEON_COMPLETED).player(player))"
                    .into()
            }
            PlayerState::CommittedCrime => {
                "q.history().any(EventQuery::kind(EventMask::CRIME).this_turn().player(player))"
                    .into()
            }
            PlayerState::BeingAttacked => {
                "q.state().attackers.iter().any(|a| a.target == Target::Player(player))".into()
            }
            // CR 700.11: a permanent card went to their graveyard this turn.
            PlayerState::Descended => format!(
                "q.history().this_turn().iter().any(|r| matches!(r.event, Event::ZoneChange {{ new, to, .. }} if to == Zone::graveyard(player) && !q.state().obj(new).token && [{}].iter().any(|&t| q.chars(new).is(t))))",
                [
                    "Artifact",
                    "Battle",
                    "Creature",
                    "Enchantment",
                    "Land",
                    "Planeswalker"
                ]
                .map(|t| format!("CardType::{t}"))
                .join(", ")
            ),
            PlayerState::FullParty => format!("{} == 4", self.party("player")?),
            PlayerState::MaxSpeed => "q.state().player(player).speed == Some(4)".into(),
            // CR 122.1f's word: a player with a poison counter.
            PlayerState::Poisoned => "q.state().player(player).poison() > 0".into(),
        })
    }
    /// `i64` the creatures of `player`'s party (CR 700.8): the most of Cleric,
    /// Rogue, Warrior and Wizard they can fill one creature to one role, by
    /// the deficiency form of Hall's theorem over the 16 sets of roles.
    pub(super) fn party(&self, player: &str) -> Result<String> {
        if self.reader == Reader::Layer {
            return Ok(planned::planned_value(
                "a party read in a layer",
                "0i64",
                &format!("cantrip_mechanics::party::size_in_layer(q, {player})"),
            ));
        }
        Ok(format!(
            "{{ let creatures: Vec<ObjectId> = q.state().battlefield().iter().copied().filter(|&id| q.controller(id) == {player} && q.chars(id).is(CardType::Creature)).collect(); (0u8..16).map(|roles: u8| creatures.iter().filter(|&&id| [Subtype::Cleric, Subtype::Rogue, Subtype::Warrior, Subtype::Wizard].iter().enumerate().any(|(i, &role)| (roles >> i) & 1 == 1 && q.chars(id).has_subtype(role))).count() as i64 + 4 - i64::from(roles.count_ones())).min().unwrap_or(0) }}"
        ))
    }
}

fn c_slot_is_fixed(c: &Compiler, slot: &Slot, index: u16) -> bool {
    c.slots.get(&slot.0).is_some_and(|(_, spec)| {
        matches!(&spec.count, Count::Exactly(Expr::Constant(n)) if *n > i64::from(index))
    })
}
pub(super) fn counters_on(id: &str, kind: Option<&crate::counter::CounterKind>) -> Result<String> {
    Ok(match kind {
        Some(kind) => format!(
            "i64::from(q.state().obj({id}).counters({}))",
            counter_kind(kind)?
        ),
        None => format!(
            "CounterKind::ALL.iter().map(|k| i64::from(q.state().obj({id}).counters(*k))).sum::<i64>()"
        ),
    })
}
fn object_kind(kind: &ObjectKind) -> Result<&'static str> {
    Ok(match kind {
        ObjectKind::Any | ObjectKind::Permanent | ObjectKind::Spell | ObjectKind::Card => "true",
        ObjectKind::Ability => return Err(gap("abilities on the stack in a zone query")),
    })
}
