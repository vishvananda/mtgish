//! Reading the game: objects, players, numbers, filters and conditions. Every
//! callback shares this compiler, so one predicate means one thing wherever it
//! is asked. Generated reads name `q` and `asker` only: `q` is a `Query` or, in
//! a layer callback, a `LayerQuery`. Both answer `state`, `chars`, `words` and
//! `controller`, which is all generated here beyond what `Reader` selects.
use super::*;

/// The card printing what is being compiled: `delayed` is that card's list, and
/// a copy of the object has the same text (CR 707.2).
pub(super) const SOURCE_CARD: &str =
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
}
impl Candidate {
    pub fn of(id: &str) -> Self {
        Self {
            id: id.into(),
            chars: format!("q.chars({id})"),
            controller: format!("q.controller({id})"),
        }
    }
    pub fn affected() -> Self {
        Self {
            id: "affected".into(),
            chars: "(*so_far)".into(),
            controller: "q.controller(affected)".into(),
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
}

/// Where one event value travels from the matcher to the resolving ability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Carried {
    Subject,
    Data(usize),
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
    })
}
pub(super) fn zone_kind(zone: &EventZone) -> Result<String> {
    Ok(format!("ZoneKind::{}", event_zone(zone)?))
}
pub(super) fn counter_kind(kind: &crate::counter::CounterKind) -> Result<String> {
    use crate::counter::CounterKind as K;
    let word = match kind {
        K::PowerToughness(1, 1) => "PlusOne".to_string(),
        K::PowerToughness(-1, -1) => "MinusOne".to_string(),
        K::PowerToughness(..) => return Err(gap("power/toughness counter other than ±1/±1")),
        K::Named(counter) => name(counter),
    };
    let word = word.strip_suffix("Counter").unwrap_or(&word).to_string();
    if crate::cantrip_vocab::COUNTERS.contains(&word.as_str()) {
        Ok(format!("CounterKind::{word}"))
    } else {
        Err(gap(format!("counter kind {word}")))
    }
}

impl Compiler {
    /// A read made while resolving, where `g` is the only reader.
    pub(super) fn fetch(&self, code: impl AsRef<str>) -> String {
        format!("{{ let q: &dyn Query = g; {} }}", code.as_ref())
    }
    fn carried(&self, carried: Carried) -> Result<(String, String)> {
        if self.history.is_some() {
            return Err(gap("event reference in a history question"));
        }
        let (subject, data) = match self.event_at {
            EventAt::Matcher => ("subject".to_string(), "data".to_string()),
            EventAt::Item => ("item.subject".into(), "item.data".into()),
            EventAt::TargetFilter => ("q.event_subject()".into(), String::new()),
            EventAt::Nowhere => return Err(gap("event value outside its ability")),
        };
        match carried {
            Carried::Subject => Ok((subject, data)),
            Carried::Data(_) if data.is_empty() => {
                Err(gap("event value in a target filter beyond its subject"))
            }
            Carried::Data(_) => Ok((subject, data)),
        }
    }
    fn carried_object(&self, carried: Option<Carried>) -> Result<String> {
        let carried = carried.ok_or_else(|| gap("event object this trigger does not bind"))?;
        let (subject, data) = self.carried(carried)?;
        Ok(match carried {
            Carried::Subject => format!("{subject}.object()"),
            Carried::Data(i) => {
                format!("Some(ObjectId({data}[{i}])).filter(|o| *o != ObjectId::NONE)")
            }
        })
    }
    fn carried_player(&self, carried: Option<Carried>) -> Result<String> {
        let carried = carried.ok_or_else(|| gap("event player this trigger does not bind"))?;
        let (subject, data) = self.carried(carried)?;
        Ok(match carried {
            Carried::Subject => format!("{subject}.player()"),
            Carried::Data(i) => format!("Some(PlayerId({data}[{i}] as u8))"),
        })
    }
    fn slot(&self, slot: &Slot) -> Result<usize> {
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
        if let Some(carried) = self.outside_object(object)? {
            return Ok(carried);
        }
        Ok(match object {
            ObjectRef::Primitive { name, .. } => {
                return Err(gap(format!("primitive object reference {name}")))
            }
            ObjectRef::Source => "Some(asker.source)".into(),
            // Inside a history question the event is what is being tested, and
            // the candidate is still the object the enclosing filter asks about.
            ObjectRef::Candidate if self.history.is_some() => match &self.history {
                Some(History { outer: Some(outer) }) => format!("Some({outer})"),
                _ => return Err(gap("candidate in a history question with no filter around it")),
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
            ObjectRef::Target { slot, index } => format!(
                "t_{}_raw.get({index}).copied().and_then(|t: Target| t.object())",
                self.slot(slot)?
            ),
            ObjectRef::RecipientObject { slot, index } => format!(
                "t_{}_raw.get({index}).copied().and_then(|t: Target| t.object())",
                self.slot(slot)?
            ),
            ObjectRef::EventSubject => self.carried_object(self.frame.object)?,
            ObjectRef::EventDestination => self.carried_object(self.frame.destination)?,
            ObjectRef::EventOther => self.carried_object(self.frame.other)?,
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
                format!("({}).get({index}).copied()", self.objects(objects)?)
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
                } else if matches!(phase, ObjectPhase::After) {
                    self.moved(result)?
                } else {
                    return Err(gap("object as it was before an instruction moved it"));
                };
                format!("({list}).get({index} as usize).copied()")
            }
            other => return Err(gap(format!("object reference {}", form(other)))),
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
            PlayerRef::ControllerOf(object) => format!(
                "{}.map(|o: ObjectId| q.controller(o))",
                self.object(object)?
            ),
            PlayerRef::OwnerOf(object) => format!(
                "{}.map(|o: ObjectId| q.state().obj(o).owner)",
                self.object(object)?
            ),
            PlayerRef::Target { slot, index } => format!(
                "t_{}_raw.get({index}).copied().and_then(|t: Target| t.player())",
                self.slot(slot)?
            ),
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
            other => return Err(gap(format!("player reference {}", form(other)))),
        })
    }
    /// One player, where a card's text leaves no way for there to be none.
    pub(super) fn certain_player(&self, player: &PlayerRef) -> Result<String> {
        fn certain(c: &Compiler, object: &ObjectRef) -> bool {
            match object {
                ObjectRef::Source | ObjectRef::Candidate | ObjectRef::Bound(_) => true,
                ObjectRef::EventSubject => c.frame.object == Some(Carried::Subject),
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
            _ => false,
        };
        if is_certain {
            Ok(format!(
                "{}.expect(\"a reference that always names a player\")",
                self.player(player)?
            ))
        } else {
            Err(gap(format!("possibly absent player {}", form(player))))
        }
    }
    /// `Vec<ObjectId>`.
    pub(super) fn objects(&self, objects: &ObjectSet) -> Result<String> {
        if let Some(carried) = self.outside_objects(objects)? {
            return Ok(carried);
        }
        Ok(match objects {
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
            ObjectSet::Selected(result) => self.selected(result)?,
            ObjectSet::Created(result) => self.created(result)?,
            ObjectSet::Transition { result, .. } => self.moved(result)?,
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
            ObjectSet::InZone { objects, zone } => format!(
                "({}).into_iter().filter(|&id| q.state().obj(id).zone.kind() == {}).collect::<Vec<ObjectId>>()",
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
            other => return Err(gap(format!("object collection {}", form(other)))),
        })
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
            ZoneScope::Shared(zone) => (zone, "q.state().seats().collect::<Vec<PlayerId>>()".into()),
            ZoneScope::Players { zone, players } => (zone, self.players(players)?),
        };
        let per_player = match zone {
            EventZone::Graveyard => "graveyard",
            EventZone::Hand => "hand",
            EventZone::Library => "library",
            EventZone::Exile => "exile",
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
            other => return Err(gap(format!("player collection {}", form(other)))),
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
    fn property(&self, id: &str, property: &Property) -> Result<Number> {
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
            other => return Err(gap(format!("numeric object property {}", form(other)))),
        })
    }
    pub(super) fn number(&self, n: &Expr) -> Result<Number> {
        let count = |code: String| Number {
            code,
            min: 0,
            max: u32::MAX.into(),
            dynamic: true,
        };
        Ok(match n {
            Expr::Primitive { name, .. } => {
                return Err(gap(format!("primitive numeric expression {name}")))
            }
            Expr::Constant(n) => Number::integer(*n),
            Expr::AnnouncedX
                if self.reader == Reader::Resolution && self.stack_item && self.outer.is_none() =>
            Number {
                code: "i64::from(item.x)".into(),
                min: 0,
                max: u16::MAX.into(),
                dynamic: true,
            },
            Expr::AnnouncedX if self.pending == Pending::Entering => Number {
                code: "i64::from(entering.x)".into(),
                min: 0,
                max: u16::MAX.into(),
                dynamic: true,
            },
            Expr::EventAmount => {
                let (_, data) = self.carried(
                    self.frame
                        .amount
                        .ok_or_else(|| gap("event amount this trigger does not bind"))?,
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
                if self.reader == Reader::Layer {
                    return Err(gap("event history in a layer"));
                }
                let scope = match window {
                    HistoryWindow::ThisTurn => "q.history().this_turn().iter()",
                    HistoryWindow::ThisGame => "q.history().all().iter()",
                    HistoryWindow::LastTurn => {
                        "q.history().all().iter().filter(|r| r.turn + 1 == q.state().turn.number)"
                    }
                    other => return Err(gap(format!("history window {}", form(other)))),
                };
                let mut asked = self.clone();
                asked.history = Some(History {
                    outer: self.candidates.borrow().last().map(|c| c.id.clone()),
                });
                let matched = asked.event(event)?;
                let hits = format!("{scope}.filter_map({})", matched.history_test());
                count(match measure {
                    HistoryMeasure::Occurrences => format!("{hits}.count() as i64"),
                    HistoryMeasure::TotalAmount if matched.frame.amount == Some(Carried::Data(0)) => {
                        format!("{hits}.map(|f: Fired| i64::from(f.data[0])).sum::<i64>()")
                    }
                    other => return Err(gap(format!("history measure {}", form(other)))),
                })
            }
            Expr::CountObjects(objects) => {
                count(format!("({}).len() as i64", self.objects(objects)?))
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
                let Recipients::Objects(objects) = recipients.as_ref() else {
                    return Err(gap("counters on a player"));
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
            Expr::Performed(result) | Expr::CountersRemoved(result) => {
                match self.performed.get(&result.0) {
                    Some(local) => count(format!("({local} as i64)")),
                    None if self.failed.contains(&result.0) => return Err(gap(FOLLOWS_GAP)),
                    None => return Err(gap("result this instruction does not report")),
                }
            }
            Expr::BoundValue(result) => match self.values.get(&result.0) {
                Some(local) => Number {
                    code: local.clone(),
                    min: i64::MIN.into(),
                    max: i64::MAX.into(),
                    dynamic: true,
                },
                None if self.failed.contains(&result.0) => return Err(gap(FOLLOWS_GAP)),
                None => return Err(gap("value outside generated scope")),
            },
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
                    min: if minimum { a.min.min(b.min) } else { a.min.max(b.min) },
                    max: if minimum { a.max.min(b.max) } else { a.max.max(b.max) },
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
            other => return Err(gap(format!("numeric expression {}", form(other)))),
        })
    }
    /// A boolean over `candidate`, the object `ObjectRef::Candidate` names.
    pub(super) fn filter(&self, f: &ObjectFilter, candidate: &Candidate) -> Result<String> {
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
            ObjectFilter::CardType(t) => format!("{chars}.is(CardType::{})", card_type(t)?),
            ObjectFilter::Supertype(t) => format!(
                "{chars}.supertypes.{}",
                match name(t).as_str() {
                    "Basic" => "basic",
                    "Legendary" => "legendary",
                    "Snow" => "snow",
                    other => return Err(gap(format!("supertype {other}"))),
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
            ObjectFilter::ControlledBy(players) => {
                self.player_in(players, &c.controller)?
            }
            ObjectFilter::OwnedBy(players) => {
                self.player_in(players, &format!("q.state().obj({id}).owner"))?
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
                format!("q.state().attackers.iter().any(|a| a.id == {id})")
            }
            ObjectFilter::Blocking => {
                format!("q.state().blocks.iter().any(|b| b.blocker == {id})")
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
                let value = self.number(value)?.code;
                let side = |field: &str| {
                    format!(
                        "{chars}.{field}.is_some_and(|v| {})",
                        compare("i64::from(v)", comparison, &value)
                    )
                };
                match property {
                    // A card that has no power has no value to compare (208.3).
                    Property::Power => side("power"),
                    Property::Toughness => side("toughness"),
                    Property::ManaValue if self.reader == Reader::Layer => compare(
                        &format!("i64::from(q.mana_value({id}, &{chars}))"),
                        comparison,
                        &value,
                    ),
                    other => compare(&self.property(id, other)?.code, comparison, &value),
                }
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
                format!("q.state().obj({id}).zone.kind() == {}", zone_kind(zone)?)
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
            other => return Err(gap(format!("object filter {}", form(other)))),
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
            Condition::Primitive { name, .. } => {
                return Err(gap(format!("primitive condition {name}")))
            }
            Condition::All(items) => combine(items, " && ", "true")?,
            Condition::Either(items) => combine(items, " || ", "false")?,
            Condition::Not(c) => format!("!({})", self.condition(c)?),
            Condition::PlayerIsActive(player) => {
                format!("{} == Some(q.state().turn.active)", self.player(player)?)
            }
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
                None if self.failed.contains(&result.0) => return Err(gap(FOLLOWS_GAP)),
                None => return Err(gap("payment outside generated scope")),
            },
            Condition::CoinFlip { result, outcome } => {
                let Some(local) = self.flips.get(&result.0) else {
                    return Err(gap(if self.failed.contains(&result.0) {
                        FOLLOWS_GAP
                    } else {
                        "coin flip outside generated scope"
                    }));
                };
                match outcome {
                    CoinOutcome::Won | CoinOutcome::Heads => local.clone(),
                    CoinOutcome::Lost | CoinOutcome::Tails => format!("!{local}"),
                }
            }
            other => return Err(gap(format!("condition {}", form(other)))),
        })
    }
}

fn c_slot_is_fixed(c: &Compiler, slot: &Slot, index: u16) -> bool {
    c.slots.get(&slot.0).is_some_and(|(_, spec)| {
        matches!(&spec.count, Count::Exactly(Expr::Constant(n)) if *n > i64::from(index))
    })
}
fn counters_on(id: &str, kind: Option<&crate::counter::CounterKind>) -> Result<String> {
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
