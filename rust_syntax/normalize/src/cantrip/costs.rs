//! Abilities and costs: what a card's `CardDef` is made of. An ability that
//! cannot be generated reports its gap and contributes nothing, and `module`
//! then publishes nothing, so no card loses an ability silently.
use super::*;

/// What the land gains of "{N}, Exile this card from your hand: Target land gains
/// "..." until this card is cast from exile. You may cast this card for as long as
/// it remains exiled": Cantrip's `land_until_cast`, which all six that print it take.
fn land_until_cast<'a>(cost: &Cost, body: &'a Program) -> Option<&'a Ability> {
    use crate::ir::Duration as D;
    let moves = |part: &CostPart| matches!(part, CostPart::Move { destination: Destination::Exile, .. });
    if cost.parts.iter().filter(|part| moves(part)).count() != 1 {
        return None;
    }
    let [Instruction::Effect {
        operation:
            Operation::CreateContinuousEffect {
                changes: gains,
                duration: D::UntilCast(ObjectRef::Source),
                ..
            },
        ..
    }, Instruction::Effect {
        operation:
            Operation::CreateContinuousEffect {
                changes: permission,
                ..
            },
        ..
    }] = body.instructions.as_slice()
    else {
        return None;
    };
    let ([Change::GrantAbility(gained)], [Change::PlayerRule(PlayerRule::PlayPermission {
        cards: ObjectSet::One(ObjectRef::Source),
        from: EventZone::Exile,
        action: CardPlayAction::CastSpell,
        filter: ObjectFilter::Any,
    })]) = (gains.as_slice(), permission.as_slice())
    else {
        return None;
    };
    Some(gained)
}

/// Where an `Ability::Scoped` moves an ability, and what gates it.
#[derive(Clone, Copy)]
pub(super) struct Scope<'s> {
    zones: &'s [EventZone],
    condition: Option<&'s Condition>,
}

pub(super) fn constant_u8(n: &Expr) -> Option<u8> {
    match n {
        Expr::Constant(n) => u8::try_from(*n).ok(),
        _ => None,
    }
}
fn exactly_u8(count: Option<&Count>) -> Option<u8> {
    match count {
        Some(Count::Exactly(n)) => constant_u8(n),
        _ => None,
    }
}

/// Crew N: the keyword's whole text is its cost's total power (CR 702.122a).
fn crew(price: &Cost, text: &str) -> Result<String> {
    if let [CostPart::Constrained { part, constraints }] = price.parts.as_slice() {
        if let (
            CostPart::ChangeTapState { tapped: true, .. },
            [SelectionConstraint::TotalProperty {
                property: Property::Power,
                comparison: Comparison::GreaterOrEqual,
                amount: Expr::Constant(power),
            }],
        ) = (part.as_ref(), constraints.as_slice())
        {
            if let Ok(power) = u8::try_from(*power) {
                let named = format!("Crew {power}");
                let text = if text.starts_with("Crew") {
                    text
                } else {
                    named.as_str()
                };
                return Ok(format!(
                    "cantrip_mechanics::crew::crew({power}, {})",
                    quoted(text)
                ));
            }
        }
    }
    Err(gap("crew with a different cost"))
}

/// The cards of a graveyard cost with every graveyard narrowed to the payer's
/// spelled as that scope, which is what the engine's cost can pay.
pub(super) fn own_graveyard(objects: &ObjectSet) -> Option<ObjectSet> {
    let ObjectSet::Zone {
        scope:
            ZoneScope::Players {
                zone: EventZone::Graveyard,
                players: PlayerSet::All,
            },
        kind,
        filter,
    } = objects
    else {
        return None;
    };
    let is_yours = |f: &ObjectFilter| {
        matches!(
            f,
            ObjectFilter::InZone(ZoneScope::Players {
                zone: EventZone::Graveyard,
                players: PlayerSet::One(PlayerRef::AbilityController),
            })
        )
    };
    let filter = match filter {
        filter if is_yours(filter) => ObjectFilter::Any,
        ObjectFilter::All(items) if items.iter().any(is_yours) => {
            let mut rest: Vec<ObjectFilter> =
                items.iter().filter(|f| !is_yours(f)).cloned().collect();
            match rest.len() {
                1 => rest.remove(0),
                _ => ObjectFilter::All(rest),
            }
        }
        _ => return None,
    };
    Some(ObjectSet::Zone {
        scope: ZoneScope::Players {
            zone: EventZone::Graveyard,
            players: PlayerSet::One(PlayerRef::AbilityController),
        },
        kind: kind.clone(),
        filter,
    })
}

/// A cost whose mana is a waterbend cost, spelled as that mana: `.waterbend()`
/// lets a tapped permanent pay each generic of an ability's whole mana cost, so
/// other mana beside it could be paid that way too and stays a gap.
fn waterbend_as_mana(price: &Cost) -> Option<Cost> {
    let bending = |part: &CostPart| matches!(part, CostPart::Waterbend(_));
    let mana = |part: &CostPart| matches!(part, CostPart::Mana(_));
    if price.parts.iter().filter(|p| bending(p)).count() != 1 || price.parts.iter().any(mana) {
        return None;
    }
    let parts = price
        .parts
        .iter()
        .map(|part| match part {
            CostPart::Waterbend(symbols) => CostPart::Mana(symbols.clone()),
            other => other.clone(),
        })
        .collect();
    Some(Cost { parts })
}

/// Whether a filter keeps to permanents the ability's controller controls.
pub(super) fn yours(filter: &ObjectFilter) -> bool {
    match filter {
        ObjectFilter::ControlledBy(PlayerSet::One(PlayerRef::AbilityController)) => true,
        ObjectFilter::All(filters) => filters.iter().any(yours),
        _ => false,
    }
}

/// "This spell can't be countered": the card's keyword, which the rules read.
fn cant_be_countered(zones: &[EventZone], affected: &EffectSubjects, changes: &[Change]) -> bool {
    let EffectSubjects::Objects(ObjectSet::Zone {
        scope: ZoneScope::Shared(EventZone::Stack),
        kind: ObjectKind::Spell,
        filter: ObjectFilter::SameObject(ObjectRef::Source),
    }) = affected
    else {
        return false;
    };
    matches!(zones, [EventZone::Stack])
        && matches!(
            changes,
            [Change::Rule(RuleChange::Prohibit(ObjectAction::Counter))]
        )
}

/// "{3}, {T} or {U}, {T}: Draw a card." as two abilities' words, and "Equip {2}
/// or {B}", whose keyword is said once.
pub(super) fn worded_apart(printed: &str) -> Option<(String, String)> {
    let (price, rest) = match printed.split_once(": ") {
        Some((price, rest)) => (price, format!(": {rest}")),
        None => (printed, String::new()),
    };
    let (one, two) = price.split_once(" or ")?;
    if two.contains(" or ") {
        return None;
    }
    let keyword = ["Equip\u{2014}", "Equip "].iter().find(|word| one.starts_with(**word));
    let two = match keyword {
        Some(word) => {
            let mut letters = two.chars();
            let first = letters.next()?.to_uppercase().collect::<String>();
            format!("{word}{first}{}", letters.as_str())
        }
        None => two.to_string(),
    };
    Some((format!("{one}{rest}"), format!("{two}{rest}")))
}

/// "{3}, {T} or {U}, {T}": an activated ability whose cost is one of several,
/// which is one ability at each price, if nothing tells the abilities apart: a
/// limit on activations counts one ability, and its body can't read the price.
/// The first price and the others.
fn alternative_prices(
    cost: &Cost,
    limits: &ActivationLimits,
    body: &Program,
) -> Option<(Cost, Vec<Cost>)> {
    let free = !limits.once
        && !limits.once_each_turn
        && limits.keyword.is_none()
        && limits.modifiers.is_empty();
    if !free || format!("{body:?}").contains("Paid") {
        return None;
    }
    let mut choices = cost.parts.iter().enumerate().filter_map(|(at, part)| match part {
        CostPart::Choice(options) => Some((at, options)),
        _ => None,
    });
    let (at, options) = choices.next()?;
    if choices.next().is_some() {
        return None;
    }
    let mut prices = options.iter().map(|option| {
        let mut parts = cost.parts[..at].to_vec();
        parts.extend(option.parts.iter().cloned());
        parts.extend(cost.parts[at + 1..].iter().cloned());
        Cost { parts }
    });
    let first = prices.next()?;
    Some((first, prices.collect()))
}

impl Compiler {
    /// A generated `ObjectFilter`: what a cost accepts of an object.
    pub(super) fn cost_filter(&mut self, filter: &ObjectFilter, label: &str) -> Result<String> {
        // A cost's filter is a plain function, with no event or stack item to read.
        // What is entering is its asker's source.
        let saved = (self.reader, self.event_at);
        (self.reader, self.event_at) = match saved.1 {
            EventAt::Entering => (Reader::Query, EventAt::Entering),
            _ => (Reader::Query, EventAt::Nowhere),
        };
        let test = self.filter(filter, &Candidate::of("id"));
        (self.reader, self.event_at) = saved;
        let mut test = test?;
        // A payer only ever pays with their own: "they control" says no more than that.
        const THEIRS: &str = ") == Some(q.controller(id))";
        if let Some(at) = test.find("Some(bound_") {
            if let Some(end) = test[at..].find(THEIRS) {
                let says = test[at..at + end + THEIRS.len()].to_string();
                test = test.replace(&says, "true");
            }
        }
        if self.reads_a_local(&test) {
            return Err(gap(
                "a cost at resolution filtered by who is asked to pay it",
            ));
        }
        let function = format!("{label}_{}", self.fresh());
        writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, id: ObjectId) -> bool {{ {test} }}").unwrap();
        Ok(function)
    }
    fn cost_part(&mut self, part: &CostPart, label: &str) -> Result<String> {
        Ok(match part {
            CostPart::Primitive { name, .. } => {
                return Err(gap(format!("primitive cost part {name}")))
            }
            CostPart::Sacrifice {
                objects: ObjectSet::Battlefield(filter),
                choose,
            } => {
                let Some(n) = exactly_u8(choose.as_ref()) else {
                    return self.sacrifice_count(filter, choose.as_ref(), label);
                };
                format!("sacrifice({n}, {})", self.cost_filter(filter, label)?)
            }
            CostPart::Discard {
                count: Expr::Constant(1),
                filter: ObjectFilter::SameObject(ObjectRef::Source),
                random: false,
            } => "DISCARD_THIS".into(),
            CostPart::Discard {
                count,
                filter,
                random: false,
            } => {
                let Some(n) = constant_u8(count) else {
                    return self.discard_count(count, filter, label);
                };
                match filter {
                    ObjectFilter::Any => format!("discard_cards({n})"),
                    filter => format!("discard({n}, {})", self.cost_filter(filter, label)?),
                }
            }
            CostPart::ChangeTapState {
                selections,
                tapped: true,
                symbol: false,
            } => {
                // "Tap enchanted land": a permanent the Aura's controller may no
                // longer control, and `tap_untapped` is of what you control.
                if let [CostObjectSelection {
                    objects: ObjectSet::One(ObjectRef::AttachedTo(_)),
                    choose: None,
                }] = selections.as_slice()
                {
                    return Ok(planned::planned_value(
                        "engine: a cost that taps what its payer may not control",
                        "Cost::NONE",
                        "Cost { variable: Some(VariableCost::TapAttached), ..Cost::NONE }",
                    ));
                }
                let [CostObjectSelection {
                    objects: ObjectSet::Battlefield(filter),
                    choose,
                }] = selections.as_slice()
                else {
                    return Err(gap("tap cost shape"));
                };
                let Some(n) = exactly_u8(choose.as_ref()) else {
                    return self.tap_count(filter, choose.as_ref(), label);
                };
                format!("tap_untapped({n}, {})", self.cost_filter(filter, label)?)
            }
            // Put on the source by the activating player (Mazemind Tome's form).
            CostPart::PutCounters(placements) => {
                if let Some(n) = lane_j::blights(placements) {
                    return Ok(format!("cantrip_mechanics::blight::cost({n})"));
                }
                let [CounterPlacement {
                    recipients: Recipients::Objects(ObjectSet::One(ObjectRef::Source)),
                    kind,
                    amount: Expr::Constant(n),
                    selection: None,
                }] = placements.as_slice()
                else {
                    return Err(gap("counters put as a cost on another object"));
                };
                let n = u32::try_from(*n).map_err(|_| gap("counter cost without a fixed count"))?;
                format!("put_counters({}, {n})", read::counter_kind(kind)?)
            }
            CostPart::RemoveCounters { removals } if !lane_g::plain_removal(removals) => {
                return self.counter_removal_beyond(removals, label)
            }
            CostPart::RemoveCounters { removals } => {
                let [CounterRemoval {
                    recipients: Recipients::Objects(objects),
                    kind,
                    amount: CounterAmount::Count(Count::Exactly(n)),
                    selection,
                    ..
                }] = removals.as_slice()
                else {
                    return Err(gap("counter removal cost shape"));
                };
                let Some(n) = constant_u8(n) else {
                    return self.counters_count(objects, kind.as_ref(), n, selection.is_some());
                };
                // One counter from one of them is one counter from among them.
                if let Some(selection) = selection {
                    let one = matches!(&selection.count, Count::Exactly(Expr::Constant(1)));
                    let yours = matches!(selection.player, PlayerRef::AbilityController);
                    if n != 1 || !one || !yours {
                        return Err(gap("counter removal cost shape"));
                    }
                }
                match (objects, kind) {
                    (ObjectSet::One(ObjectRef::Source), Some(kind)) => {
                        format!("remove_counters({}, {n})", read::counter_kind(kind)?)
                    }
                    (ObjectSet::One(ObjectRef::Source), None) if n == 1 => {
                        "REMOVE_A_COUNTER".to_string()
                    }
                    (ObjectSet::Battlefield(filter), kind) if yours(filter) => {
                        let filter = self.cost_filter(filter, label)?;
                        match kind {
                            Some(kind) => format!(
                                "remove_kind_counters_among({}, {n}, {filter})",
                                read::counter_kind(kind)?
                            ),
                            None => format!("remove_counters_among({n}, {filter})"),
                        }
                    }
                    _ => return Err(gap("counter removal cost shape")),
                }
            }
            CostPart::Move {
                selections,
                destination,
                entry,
                face_down: false,
            } if entry.counters.is_empty() && entry.states.is_empty() && !entry.tapped => {
                if self.links_exile && matches!(destination, Destination::Exile) {
                    return Err(gap("exile cost an ability reads as exiled with its source"));
                }
                let [CostObjectSelection { objects, choose }] = selections.as_slice() else {
                    return self.move_cost_beyond(selections, destination, label);
                };
                let Some(n) = exactly_u8(choose.as_ref()) else {
                    return self.move_cost_beyond(selections, destination, label);
                };
                let yours = own_graveyard(objects);
                let objects = yours.as_ref().unwrap_or(objects);
                match (destination, objects) {
                    (Destination::OwnersHand, ObjectSet::Battlefield(filter)) => {
                        match lane_i::without_tapped(filter) {
                            Some(rest) => format!(
                                "return_tapped_to_hand({n}, {})",
                                self.cost_filter(&rest, label)?
                            ),
                            None => format!(
                                "return_to_hand({n}, {})",
                                self.cost_filter(filter, label)?
                            ),
                        }
                    }
                    (Destination::Exile, ObjectSet::Battlefield(filter)) => {
                        format!(
                            "exile_permanents({n}, {})",
                            self.cost_filter(filter, label)?
                        )
                    }
                    (
                        Destination::Exile,
                        ObjectSet::Zone {
                            scope:
                                ZoneScope::Players {
                                    zone: EventZone::Graveyard,
                                    players: PlayerSet::One(PlayerRef::AbilityController),
                                },
                            filter,
                            ..
                        },
                    ) => match filter {
                        ObjectFilter::Any => format!("exile_from_graveyard({n})"),
                        filter => format!(
                            "exile_from_graveyard_of({n}, {})",
                            self.cost_filter(filter, label)?
                        ),
                    },
                    (
                        Destination::Exile,
                        ObjectSet::Zone {
                            scope:
                                ZoneScope::Players {
                                    zone: EventZone::Hand,
                                    players: PlayerSet::One(PlayerRef::AbilityController),
                                },
                            filter,
                            ..
                        },
                    ) => format!("exile_from_hand({n}, {})", self.cost_filter(filter, label)?),
                    _ => return self.move_cost_beyond(selections, destination, label),
                }
            }
            other => return self.cost_part_beyond(other, label),
        })
    }
    /// A `Cost`. Mana symbols combine into one part before bounds are checked.
    pub(super) fn cost(&mut self, value: &Cost, label: &str) -> Result<String> {
        self.mana_x = value.parts.iter().any(|part| {
            matches!(part, CostPart::Mana(mana) if mana.iter().any(|s| matches!(s, ManaSymbol::X)))
        });
        let mut symbols = Vec::new();
        let (mut tap, mut life_total, mut loyalty_delta) = (false, 0u32, None);
        let mut sacrifice_this = false;
        let mut others = Vec::new();
        for part in &value.parts {
            if let CostPart::Move {
                selections,
                destination,
                entry,
                face_down: false,
            } = part
            {
                // The source itself is no choice of how many.
                if let (
                    [CostObjectSelection {
                        objects: ObjectSet::One(ObjectRef::Source),
                        choose: None,
                    }],
                    true,
                ) = (
                    selections.as_slice(),
                    entry.counters.is_empty() && entry.states.is_empty() && !entry.tapped,
                ) {
                    match destination {
                        Destination::Exile => {
                            others.push("EXILE_THIS".to_string());
                            continue;
                        }
                        Destination::OwnersHand => {
                            others.push("RETURN_THIS".to_string());
                            continue;
                        }
                        _ => {}
                    }
                }
            }
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
                CostPart::Energy(Expr::Constant(n)) if (0..=255).contains(n) => {
                    others.push(format!("Cost {{ energy: {n}, ..Cost::NONE }}"))
                }
                other => {
                    let part = self.cost_part(other, label);
                    others.push(part.or_else(|failed| self.stated_cost_part(other, label, failed))?)
                }
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
        parts.extend(others);
        Ok(parts
            .into_iter()
            .fold("Cost::NONE".into(), |a, b| format!("{a}.and({b})")))
    }
    /// The modes of an announcement, as a `ModalDef` static. Each mode is a
    /// program with slots of its own: the engine renumbers them as it runs it.
    pub(super) fn modal(
        &mut self,
        announcement: &Announcement,
        label: &str,
        frame: Option<Frame>,
    ) -> Result<String> {
        let selection = announcement.modes.as_ref().expect("a modal announcement");
        if !announcement.targets.is_empty() {
            return Err(gap("targets announced for every mode"));
        }
        let ModeSelection {
            minimum,
            maximum,
            repeat,
            modes,
            unchosen,
            surcharge,
            may_decline,
            ..
        } = selection;
        let bound = |n: &Expr| match n {
            Expr::Constant(n) => u8::try_from(*n).ok(),
            _ => None,
        };
        let flat = lane_p::flat_modes(selection);
        let modes = flat.as_ref().unwrap_or(modes);
        // "Choose one. If you control a commander, you may choose both instead."
        let (most, more) = match maximum {
            Expr::Conditional {
                condition,
                then,
                otherwise,
            } if bound(then).is_some() => (&**otherwise, Some((condition, bound(then)))),
            most => (most, None),
        };
        let mut defs = Vec::new();
        for (i, mode) in modes.iter().enumerate() {
            // A slot in a mode reads a `max` above 1 as a range.
            if format!("{:?}", mode.announcement.targets).contains("PerPlayer {") {
                return Err(gap("target cardinality PerPlayer"));
            }
            let label = format!("{label}_mode_{i}");
            // A mode that is a choice of modes itself, chosen its own way.
            let inner = match &mode.announcement.modes {
                Some(_) => Some(self.modal(&mode.announcement, &format!("{label}_inner"), None)?),
                None => None,
            };
            let announced = lane_p::without_modes(&mode.announcement);
            let targets = match frame {
                Some(frame) => self.targets_for(&announced, &label, frame)?,
                None => self.announcement(&announced, &label)?,
            };
            let divide = self.dividing.take();
            let effect = match self.mode_guard.clone() {
                Some(guard) => self.guarded_effect(&guard, &mode.body, &label)?,
                None => self.effect(&mode.body, &label)?,
            };
            let def = format!(
                "mode({effect}, {}).targets({targets})",
                quoted(&self.said_mode(i, modes.len(), ability_text(&mode.body)))
            );
            let def = match divide {
                Some(divide) => planned::planned_value(
                    "target with a divided amount",
                    &def,
                    &format!("{def}.divide({divide})"),
                ),
                None => def,
            };
            let def = match inner {
                Some(inner) => planned::planned_value(
                    "modal announcement",
                    &def,
                    &format!("{def}.then_modes(&{inner})"),
                ),
                None => def,
            };
            defs.push(match &mode.requires {
                Some(condition) => {
                    let test = self.activation_condition(condition, &format!("{label}_open"))?;
                    planned::planned_value(
                        "mode with a requirement",
                        &def,
                        &format!("{def}.open_if({test})"),
                    )
                }
                None => def,
            });
        }
        let listed = format!("modes(&[{}])", defs.join(","));
        // "When this enters, choose up to one \u{2014}": the source reads it as one.
        let up_to = ["one", "two", "three"].iter().any(|n| {
            self.line_text().is_some_and(|line| {
                line.contains(&format!("choose up to {n} \u{2014}"))
            })
        });
        let fewest = match up_to || lane_k::printed_otherwise(self.line_text(), "Choose up to") {
            true => Some(0),
            false => bound(minimum),
        };
        // "Choose up to X, where X is ...": a triggered ability's own count,
        // read from the game as it is put on the stack (CR 603.3c, 700.2).
        let counted = match self.slot_counts {
            target_counts::SlotCounts::Trigger { enters_itself }
                if fewest == Some(0) && bound(most).is_none() && !label.contains("_mode_") =>
            {
                self.count_of_trigger(most, enters_itself, label).is_some()
            }
            _ => false,
        };
        let mut modal = match (fewest, bound(most)) {
            (Some(min), Some(max)) => format!("{listed}.choose({min}, {max})"),
            _ if counted => format!("{listed}.up_to_that_many()"),
            _ => self.mode_count_to_come(&listed, minimum, most)?,
        };
        if *may_decline {
            modal = planned::planned_value(
                "modes that may all be declined",
                &modal,
                &format!("{modal}.may_decline()"),
            );
        }
        if let Some((condition, Some(most))) = more {
            let around = self.frame;
            self.frame = frame.unwrap_or(around);
            let test = self.activation_condition(condition, &format!("{label}_more_modes"));
            self.frame = around;
            let test = test?;
            modal.push_str(&format!(".up_to_if({test}, {most})"));
        }
        if *repeat {
            modal.push_str(".repeat()");
        }
        modal.push_str(&lane_l::mode_history(unchosen));
        if let Some(surcharge) = surcharge {
            let cost = self.cost(surcharge, label)?;
            modal.push_str(&format!(".escalate({cost})"));
        }
        let modal = self.modes_to_come(selection, modal)?;
        let name = format!("{}_MODES", label.to_ascii_uppercase());
        writeln!(self.functions, "static {name}: ModalDef = {modal};").unwrap();
        Ok(name)
    }
    /// "This ability costs {1} less to activate ...": a function asked for this
    /// ability only, reading the game as a cast's cost change does.
    fn activation_cost_change(
        &mut self,
        modifier: &SpellCostModifier,
        condition: Option<&Condition>,
        label: &str,
    ) -> Result<String> {
        let saved = (self.reader, self.event_at);
        (self.reader, self.event_at) = (Reader::Query, EventAt::Nowhere);
        self.costing_targets = true;
        let reads = self
            .cost_delta(modifier)
            .and_then(|delta| Ok((delta, condition.map(|c| self.condition(c)).transpose()?)));
        self.costing_targets = false;
        (self.reader, self.event_at) = saved;
        let (delta, test) = reads?;
        let test = test.unwrap_or_else(|| "true".into());
        let function = format!("{label}_cost");
        writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, costed: Costed, targets: &[Target]) -> CostDelta {{ if {test} {{ {delta} }} else {{ CostDelta::NONE }} }}").unwrap();
        Ok(function)
    }
    /// "Activate no more than twice each turn" (CR 602.5b), asked of the event
    /// history, since the engine counts only once each turn. `index` is the
    /// ability's place on its card, the same on a copy (CR 707.2).
    fn activations_each_turn(&mut self, n: i64, index: usize, label: &str) -> String {
        let function = format!("{label}_each_turn");
        let card = self.source_card();
        writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker) -> bool {{ let this = AbilityRef {{ card: {card}, index: {index} }}; (q.history().this_turn().iter().filter(|r| matches!(r.event, Event::AbilityActivated {{ source, ability, .. }} if source == asker.source && ability == this)).count() as i64) < {n} }}").unwrap();
        function
    }
    fn activated(
        &mut self,
        label: &str,
        index: usize,
        origin: &ActivationOrigin,
        price: &Cost,
        limits: &ActivationLimits,
        announcement: &Announcement,
        body: &Program,
        scope: Option<Scope>,
    ) -> Result<String> {
        let waterbent = waterbend_as_mana(price);
        let price = waterbent.as_ref().unwrap_or(price);
        let text = self.said(lane_k::Said::Activated, ability_text(body));
        let suffix = match origin {
            ActivationOrigin::Ordinary => "",
            ActivationOrigin::Equip => ".equip()",
            // The zone the keyword functions from, unless a scope states it.
            ActivationOrigin::Cycling { .. } if scope.is_none() => {
                ".from(ZoneKind::Hand).cycling()"
            }
            ActivationOrigin::Cycling { .. } => ".cycling()",
            ActivationOrigin::Crew => return crew(price, &text),
            #[allow(unreachable_patterns)]
            other => return Err(gap(format!("activation origin {}", form(other)))),
        };
        let implied = match body.instructions.is_empty() {
            true => self.implied_slots(announcement, label, None)?,
            false => None,
        };
        let mut def = if let Some(targets) = implied {
            let effect = self.implied_effect(announcement, label)?;
            format!(
                "ability({}, {effect}, {}).targets({targets})",
                self.cost(price, label)?,
                quoted(&text)
            )
        } else if announcement.modes.is_some() {
            let modal = self.modal(announcement, label, None)?;
            format!(
                "modal_ability({}, &{modal}, {})",
                self.cost(price, label)?,
                quoted(&text)
            )
        } else if crate::semantics::activation_kind(price, announcement, body)
            == crate::semantics::ActivationKind::Mana
        {
            if let Some(colors) = lane_g::filter_land(body) {
                format!(
                    "cantrip_mechanics::lands::filter_two::<{colors}>({}, {})",
                    self.cost(price, label)?,
                    quoted(&text)
                )
            } else {
                self.mana_body = true;
                self.activated_mana = Some(index);
                let adds = self.mana_ability(body, &format!("{label}_mana"));
                self.mana_body = false;
                self.activated_mana = None;
                let adds = adds?;
                let colors = self.mana_colors.take();
                self.paid_for = PaidFor::Mana;
                let cost = self.cost(price, label);
                self.paid_for = PaidFor::Ability;
                let made = format!(
                    "mana_ability({}, {adds}, {}){}",
                    cost?,
                    quoted(&text),
                    self.colors_from(body, label)?
                );
                match colors {
                    Some((suffix, None)) => format!("{made}{suffix}"),
                    Some((suffix, Some(feature))) => {
                        planned::planned_value(feature, &made, &format!("{made}{suffix}"))
                    }
                    None => made,
                }
            }
        } else {
            let targets = self.announcement(announcement, label)?;
            let effect = self.effect(body, label)?;
            let after_x = match self.targets_read_x(label) {
                true => ".targets_after_x()",
                false => "",
            };
            format!(
                "ability({}, {effect}, {}).targets({targets}){after_x}",
                self.cost(price, label)?,
                quoted(&text)
            )
        };
        def.push_str(suffix);
        if waterbent.is_some() {
            if def.starts_with("mana_ability") {
                return Err(gap("waterbend on a mana ability"));
            }
            def.push_str(".waterbend()");
        }
        let mut allowed = None;
        for modifier in &limits.modifiers {
            let Change::ActivationRule(rule) = modifier else {
                return Err(gap("activation modifier"));
            };
            match rule {
                ActivationRule::XNotZero => def.push_str(".x_cant_be_zero()"),
                ActivationRule::Cost {
                    modifier,
                    condition,
                } => {
                    let change =
                        self.activation_cost_change(modifier, condition.as_ref(), label)?;
                    def.push_str(&format!(".cost_change({change})"));
                }
                ActivationRule::MaxPerTurn(Expr::Constant(n)) if *n >= 1 => {
                    allowed = Some(self.activations_each_turn(*n, index, label));
                }
                other => {
                    def = self.activation_rule_beyond(def, other, label)?;
                }
            }
        }
        match limits.keyword {
            Some(ActivatedKeyword::Exhaust) => def.push_str(".exhaust()"),
            Some(ActivatedKeyword::PowerUp) => {
                def.push_str(".cost_change(cantrip_mechanics::power_up::reduction).power_up()")
            }
            Some(ActivatedKeyword::Boast) => {
                let boast = self.boast(label);
                def.push_str(&format!(".condition({boast}).once_each_turn()"));
            }
            Some(ActivatedKeyword::Forecast) => {}
            None => {}
        }
        if limits.sorcery {
            def.push_str(".sorcery_speed()");
        }
        if limits.once_each_turn {
            def.push_str(".once_each_turn()");
        }
        if limits.once {
            def.push_str(".once()");
        }
        let conditions: Vec<&Condition> = limits
            .condition
            .iter()
            .chain(scope.and_then(|s| s.condition))
            .collect();
        match (conditions.as_slice(), allowed) {
            ([], None) => {}
            ([], Some(function)) => def.push_str(&format!(".condition({function})")),
            ([condition], None) => {
                let function = self.activation_condition(condition, &format!("{label}_if"))?;
                def.push_str(&format!(".condition({function})"));
            }
            _ => return Err(gap("activation with two conditions")),
        }
        if let Some(scope) = scope {
            let [zone] = scope.zones else {
                return Err(gap("activation from several zones"));
            };
            if *zone != EventZone::Battlefield {
                def.push_str(&format!(".from({})", read::zone_kind(zone)?));
            }
        }
        if matches!(limits.keyword, Some(ActivatedKeyword::Forecast)) {
            def = lane_g::forecast(def);
        }
        self.activators(def, limits.activators.as_ref())
    }
    /// Backup N (702.165a). The listed abilities also stand on the card itself,
    /// so here they are only what the target gains: keywords, and the one other
    /// ability `backup::resolve` can name, kept in `delayed`.
    fn backup(
        &mut self,
        amount: i64,
        abilities: &[Ability],
        label: &str,
        parts: &mut Parts,
    ) -> Result<()> {
        let n = i16::try_from(amount).map_err(|_| gap("backup beyond i16"))?;
        let mut keywords = Vec::new();
        let mut granted = None;
        let mut stated = Vec::new();
        let mut feature = String::new();
        for (i, listed) in abilities.iter().enumerate() {
            match listed {
                Ability::Keyword(k) => keywords.push(keyword(k)?),
                Ability::Triggered { .. } | Ability::Activated { .. } if granted.is_none() => {
                    let mut own = Parts::default();
                    self.ability(listed, &format!("{label}_gains_{i}"), &mut own, None)?;
                    let [def] = own.abilities.as_slice() else {
                        return Err(gap("backup grants an ability of several parts"));
                    };
                    let index = u8::try_from(self.delayed.len())
                        .map_err(|_| gap("backup beyond the delayed abilities"))?;
                    self.delayed.push(def.clone());
                    granted = Some(index);
                }
                other => {
                    feature = format!("backup grants {}", form(other));
                    stated.extend(self.granted_refs(other)?);
                }
            }
        }
        let ability = granted.map_or("None".to_string(), |i| format!("Some({i})"));
        let mut call = format!(
            "cantrip_mechanics::backup::resolve(g, item, {n}, &[{}], {ability})",
            keywords.join(",")
        );
        if !stated.is_empty() {
            call = planned::planned_value(
                &feature,
                &call,
                &format!(
                    "cantrip_mechanics::backup::resolve_granting(g, item, {n}, &[{}], {ability}, &[{}])",
                    keywords.join(","),
                    stated.join(", ")
                ),
            );
        }
        let function = format!("{label}_backup");
        writeln!(
            self.functions,
            "fn {function}(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {{ {call} }}"
        )
        .unwrap();
        parts.abilities.push(format!(
            "cantrip_mechanics::backup::trigger({}, {function})",
            quoted(&format!("Backup {n}"))
        ));
        Ok(())
    }
    /// Level up and its stages (702.87, 711): each is what its counters make
    /// true, so a stage is a conditional ability over them.
    fn counter_stages(
        &mut self,
        counter: &crate::counter::CounterKind,
        level_up: Option<&Cost>,
        stages: &[CounterStage],
        label: &str,
        parts: &mut Parts,
    ) -> Result<()> {
        if let Some(price) = level_up {
            let [CostPart::Mana(symbols)] = price.parts.as_slice() else {
                return Err(gap("level up cost other than mana"));
            };
            parts.abilities.push(format!(
                "cantrip_mechanics::level_up::ability({}, \"Level up\")",
                mana(symbols)?
            ));
        }
        let held = Expr::CountCounters {
            recipients: Box::new(Recipients::Objects(ObjectSet::One(ObjectRef::Source))),
            kind: Some(counter.clone()),
            information: Information::CurrentOrLastKnown,
        };
        let compare = |comparison, n| Condition::Compare {
            left: held.clone(),
            comparison,
            right: Expr::Constant(n),
        };
        for (i, stage) in stages.iter().enumerate() {
            let least = compare(Comparison::GreaterOrEqual, stage.minimum);
            let condition = match stage.maximum {
                Some(most) => Condition::All(vec![least, compare(Comparison::LessOrEqual, most)]),
                None => least,
            };
            // A station stage makes it a creature in addition to its other types (CR 721.2b).
            let mut changes = Vec::new();
            if stage.animates {
                changes.push(Change::AddCardType(crate::mtg_types::CardType::Creature));
            }
            if let Some((power, toughness)) = stage.power_toughness {
                changes.push(Change::SetBasePowerToughness {
                    power: Expr::Constant(power.into()),
                    toughness: Expr::Constant(toughness.into()),
                });
            }
            let mut then: Vec<Ability> = (!changes.is_empty())
                .then(|| Ability::Static {
                    kind: StaticKind::Ordinary,
                    affected: EffectSubjects::Objects(ObjectSet::One(ObjectRef::Source)),
                    changes,
                })
                .into_iter()
                .collect();
            if stage.animates {
                then.push(Ability::Static {
                    kind: StaticKind::Ordinary,
                    affected: EffectSubjects::Objects(ObjectSet::One(ObjectRef::Source)),
                    changes: vec![Change::AddCardType(crate::mtg_types::CardType::Creature)],
                });
            }
            // "It has [abilities]" for as long as the stage lasts (CR 711.2a, 721.2a).
            then.extend(stage.abilities.iter().map(|ability| match ability {
                Ability::Activated { .. } | Ability::Triggered { .. } => Ability::Static {
                    kind: StaticKind::Ordinary,
                    affected: EffectSubjects::Objects(ObjectSet::One(ObjectRef::Source)),
                    changes: vec![Change::GrantAbility(Box::new(ability.clone()))],
                },
                other => other.clone(),
            }));
            let staged = Ability::Conditional {
                condition,
                then,
                otherwise: Vec::new(),
            };
            self.on_stage(stage.minimum.into());
            let staged = self.ability(&staged, &format!("{label}_stage_{i}"), parts, None);
            self.stage_lines.clear();
            staged?;
        }
        Ok(())
    }
    /// Add one ability to the card.
    pub(super) fn ability(
        &mut self,
        ability: &Ability,
        label: &str,
        parts: &mut Parts,
        scope: Option<Scope>,
    ) -> Result<()> {
        match ability {
            Ability::Keyword(k) => match k {
                // "Snow swampwalk", "desertwalk": no word of Cantrip's says it, and it
                // is a rule of the block all the same (CR 702.14c).
                Keyword::Landwalk(land) if lane_p::landwalk_rule(land) => {
                    let test = self.filter(land, &Candidate::of("land"))?;
                    // The filter names the land type by the text of `asker.source`, and
                    // a rule of the block is told its source outright (CR 612.2).
                    let test = test.replace("asker.source", "source");
                    let rule = format!("{label}_walk");
                    writeln!(self.functions, "#[allow(unused_variables)] fn {rule}(q: &dyn Query, source: ObjectId, blocker: ObjectId, attacker: ObjectId) -> bool {{ attacker != source || !q.defending_player(attacker).is_some_and(|defending| q.state().battlefield().iter().any(|&land| q.controller(land) == defending && q.chars(land).is(CardType::Land) && {test})) }}").unwrap();
                    parts.block_rules.push(rule);
                }
                Keyword::Prowess => parts
                    .abilities
                    .push("cantrip_mechanics::triggers::PROWESS".into()),
                Keyword::Flanking => parts
                    .abilities
                    .push("cantrip_mechanics::flanking::TRIGGER".into()),
                Keyword::Ward(cost) if lane_k::ward_by_sacrifice(cost).is_some() => {
                    parts.abilities.extend(lane_k::ward_by_sacrifice(cost));
                }
                Keyword::Ward(cost) => {
                    let ward = match cost.parts.as_slice() {
                        [CostPart::Mana(symbols)] => match symbols.as_slice() {
                            [ManaSymbol::Generic(n)] if u8::try_from(*n).is_ok() => {
                                format!("cantrip_mechanics::triggers::ward::<{n}>(\"Ward {{{n}}}\")")
                            }
                            _ => self.ward_beyond(cost, label)?,
                        },
                        [CostPart::Life(Expr::Constant(n))] if u8::try_from(*n).is_ok() => {
                            format!("cantrip_mechanics::triggers::ward_life::<{n}>(\"Ward\u{2014}Pay {n} life.\")")
                        }
                        [CostPart::Discard {
                            count: Expr::Constant(1),
                            filter: ObjectFilter::Any,
                            random: false,
                        }] => "cantrip_mechanics::triggers::ward_discard(\"Ward\u{2014}Discard a card.\")".into(),
                        [CostPart::Life(Expr::ObjectProperty {
                            object: ObjectRef::KeywordHost,
                            property: Property::Power,
                            ..
                        })] => "cantrip_mechanics::triggers::ward_life_equal_to_power(\"Ward\u{2014}Pay life equal to this creature's power.\")".into(),
                        _ => self.ward_beyond(cost, label)?,
                    };
                    parts.abilities.push(ward);
                }
                Keyword::Protection(protection) => match protection.as_ref() {
                    Protection::Objects {
                        kind: ObjectKind::Any,
                        filter: ObjectFilter::Color(c),
                    } => {
                        color(c)?;
                        parts.protection |= 1 << color_index(c)?;
                    }
                    other => self.protection_beyond(other, parts)?,
                },
                Keyword::HexproofFrom(quality) => {
                    parts.field("hexproof_from", self.hexproof_of(quality)?)
                }
                k => parts.keywords.push(keyword(k)?),
            },
            Ability::Casting(rule) => self.casting(rule, label, parts, None)?,
            Ability::CostBranch {
                keyword,
                cost,
                paid,
                unpaid,
            } => match self.cost_branch(keyword, cost, paid, unpaid, label, parts) {
                Err(failed) if lane_o::branch_stated(&failed.feature) => {
                    self.stated_branch(keyword, cost, paid, unpaid, label, failed, parts)?
                }
                other => other?,
            },
            Ability::Scoped {
                zones,
                condition,
                ability,
            } => {
                // "Max speed — ..." on a card in a graveyard: its scope, conditioned.
                if let (None, Ability::Conditional { condition: held, then, otherwise }) =
                    (condition, &**ability)
                {
                    if let ([only], true) = (then.as_slice(), otherwise.is_empty()) {
                        let scoped = Ability::Scoped {
                            zones: zones.clone(),
                            condition: Some(held.clone()),
                            ability: Box::new(only.clone()),
                        };
                        return self.ability(&scoped, label, parts, None);
                    }
                }
                if self.from_command_zone(zones, condition.as_ref(), ability, label, parts)? {
                    return Ok(());
                }
                let scope = Scope {
                    zones,
                    condition: condition.as_ref(),
                };
                match ability.as_ref() {
                    Ability::Activated { .. } | Ability::Triggered { .. } => {
                        self.ability(ability, label, parts, Some(scope))?
                    }
                    Ability::Static {
                        affected, changes, ..
                    } if condition.is_none() && cant_be_countered(zones, affected, changes) => {
                        parts.keywords.push("Keyword::CantBeCountered".into());
                    }
                    other => {
                        return self.scoped_static(zones, condition.as_ref(), other, label, parts)
                    }
                }
            }
            Ability::Spell { announcement, body } => {
                if parts.spell != "None" {
                    return self.later_spell(announcement, body, label, parts);
                }
                let implied = match body.instructions.is_empty() {
                    true => self.implied_slots(announcement, label, None)?,
                    false => None,
                };
                if let Some(targets) = implied {
                    let effect = self.implied_effect(announcement, label)?;
                    parts.spell = format!("spell({targets}, {effect})");
                } else if announcement.modes.is_some() {
                    if !body.instructions.is_empty() {
                        return Err(gap("modal spell with instructions besides its modes"));
                    }
                    let modal = self.modal(announcement, label, None)?;
                    parts.spell = "Some(SpellDef::MODAL)".into();
                    parts.modal = format!("Some(&{modal})");
                } else {
                    let targets = self.announcement(announcement, label)?;
                    let effect = self.effect(body, label)?;
                    parts.spell = format!("spell({targets}, {effect})");
                    if self.targets_read_x(label) {
                        parts.field("late_targets", "true".into());
                    }
                    if let Some(divide) = self.dividing.take() {
                        parts.fields.insert("divide", format!("Some({divide})"));
                    }
                    self.spell_keywords(parts);
                }
            }
            Ability::Activated {
                origin,
                cost,
                limits,
                announcement,
                body,
            } => {
                if let Some(gained) = land_until_cast(cost, body) {
                    let rest = Cost {
                        parts: cost.parts.iter().filter(|part| !matches!(part, CostPart::Move { .. })).cloned().collect(),
                    };
                    let price = self.cost(&rest, label)?;
                    let text = self.said(lane_k::Said::Activated, ability_text(body));
                    let index = self.delayed.len();
                    self.granted_refs(gained)?;
                    parts.abilities.push(format!(
                        "cantrip_mechanics::land_until_cast::ability::<{index}>({price}, {})",
                        quoted(&text)
                    ));
                    return Ok(());
                }
                let either = alternative_prices(cost, limits, body);
                let (cost, others) = match &either {
                    Some((first, others)) => (first, others.as_slice()),
                    None => (cost, &[][..]),
                };
                let before = self.clone();
                self.activation_cost = Some(cost.clone());
                let index = parts.abilities.len();
                let def = self.activated(
                    label,
                    index,
                    origin,
                    cost,
                    limits,
                    announcement,
                    body,
                    scope,
                );
                self.activation_cost = None;
                if matches!(limits.keyword, Some(ActivatedKeyword::Forecast)) {
                    self.plan_entry("activated keyword Forecast", "abilities", &def?, parts);
                } else {
                    let mut def = def?;
                    // As Transmogrant's Crown's two equips: the same ability at
                    // the other price, each worded with its own.
                    if let [other] = others {
                        let printed = before.clone().said(lane_k::Said::Activated, ability_text(body));
                        let first = before.clone().cost(cost, label)?;
                        let unlike = gap("engine: no choice between the costs of an ability");
                        let Some((one, two)) = worded_apart(&printed) else {
                            return Err(unlike);
                        };
                        if !def.contains(&first) || !def.contains(&quoted(&printed)) {
                            return Err(unlike);
                        }
                        let code = self.cost(other, &format!("{label}_or_1"))?;
                        let second = def
                            .replacen(&first, &code, 1)
                            .replacen(&quoted(&printed), &quoted(&two), 1);
                        def = def.replacen(&quoted(&printed), &quoted(&one), 1);
                        parts.abilities.push(def);
                        parts.abilities.push(second);
                    } else if others.is_empty() {
                        parts.abilities.push(def);
                    } else {
                        return Err(gap("engine: no choice between the costs of an ability"));
                    }
                }
            }
            Ability::Triggered {
                event,
                intervening_if,
                limits,
                announcement,
                body,
            } => {
                self.event_object = lane_j::event_object(event);
                let def = self.triggered(
                    label,
                    event,
                    intervening_if.as_ref(),
                    limits,
                    announcement,
                    body,
                );
                self.event_object = None;
                let def = match scope {
                    Some(scope) if scope.condition.is_some() => {
                        Err(gap("conditional scope on a triggered ability"))
                    }
                    Some(scope) => {
                        let zones = zone_list(scope.zones);
                        def.and_then(|def| Ok(format!("{def}.functions_in({})", zones?)))
                    }
                    None => def,
                };
                parts.abilities.push(def?);
            }
            Ability::Static {
                kind,
                affected,
                changes,
            } if !self.flashback_way(label, affected, changes, parts)? => {
                self.static_ability(label, kind, affected, changes, parts)?;
            }
            Ability::Static { .. } => {}
            Ability::Enchant { hosts } => {
                if parts.enchant != "None" {
                    return Err(gap("two enchant abilities"));
                }
                let reader = std::mem::replace(&mut self.reader, Reader::Query);
                let test = match hosts {
                    EnchantHosts::Permanents(filter) => self
                        .filter(filter, &Candidate::of("id"))
                        .map(|f| {
                            format!(
                                "matches!(t, Target::Object(id) if cantrip_mechanics::targets::permanent(q, id) && ({f}))"
                            )
                        }),
                    EnchantHosts::Players(players) => self
                        .player_in(players, "p")
                        .map(|p| format!("matches!(t, Target::Player(p) if {p})")),
                    EnchantHosts::GraveyardCards { .. } => Err(gap("enchant graveyard card")),
                };
                self.reader = reader;
                let function = format!("{label}_enchant");
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, t: Target) -> bool {{ {} }}", test?).unwrap();
                parts.enchant = format!("Some(target({function}))");
            }
            Ability::Conditional {
                condition,
                then,
                otherwise,
            } => {
                if self.static_guard.is_some() {
                    return Err(gap("nested conditional ability"));
                }
                self.conditional(condition, then, label, parts, scope)?;
                let unless = Condition::Not(Box::new(condition.clone()));
                self.conditional(
                    &unless,
                    otherwise,
                    &format!("{label}_otherwise"),
                    parts,
                    scope,
                )?;
            }
            Ability::Saga { chapters } => {
                for (i, chapter) in chapters.iter().enumerate() {
                    let Ability::Triggered {
                        event: EventSpec::SagaChapter(n),
                        intervening_if: None,
                        announcement,
                        body,
                        ..
                    } = chapter
                    else {
                        return Err(gap("saga chapter shape"));
                    };
                    let label = format!("{label}_{i}");
                    self.on_chapter(i64::from(*n));
                    let modal = self.reflexive_modes(announcement, &format!("{label}_modes"))?;
                    let targets =
                        self.announcement(&lane_p::without_modes(announcement), &label)?;
                    let divide = self.dividing.take();
                    let effect = self.effect(body, &label)?;
                    let def = format!(
                        "chapter({n}, {effect}, {}).targets({targets})",
                        quoted(&self.said_chapter(i64::from(*n), ability_text(body)))
                    );
                    let def = match divide {
                        Some(divide) => format!("{def}.divide({divide})"),
                        None => def,
                    };
                    parts.abilities.push(match modal {
                        Some(modal) if body.instructions.is_empty() => {
                            format!("{def}.modal(&{modal})")
                        }
                        Some(modal) => planned::planned_value(
                            "modal announcement",
                            &def,
                            &format!("{def}.modal(&{modal})"),
                        ),
                        None => def,
                    });
                }
            }
            Ability::Replacement {
                active,
                event,
                body,
            } => self.replacement(label, active, event, body, parts)?,
            Ability::Mechanic {
                mechanic,
                cost,
                amount,
                filter,
            } => {
                let compiled = self.mechanic(
                    *mechanic,
                    cost.as_ref(),
                    amount.as_ref(),
                    filter.as_ref(),
                    label,
                    parts,
                );
                compiled.or_else(|failed| {
                    let (cost, amount, filter) = (cost.as_ref(), amount.as_ref(), filter.as_ref());
                    self.stated_mechanic(*mechanic, cost, amount, filter, label, failed, parts)
                })?
            }
            Ability::Backup { amount, abilities } => {
                self.backup(*amount, abilities, label, parts)?
            }
            Ability::CounterStages {
                counter,
                level_up,
                stages,
            } => self.counter_stages(counter, level_up.as_ref(), stages, label, parts)?,
            Ability::Primitive { name, .. } => {
                return Err(gap(format!("primitive ability {name}")))
            }
            // A card that says it is out of the game has no game to generate.
            Ability::DeckRule(DeckRule::Banned | DeckRule::RemoveUnlessPlayingForAnte) => {
                self.plan_entry(
                    "card outside ordinary play",
                    "deck_rules",
                    "DeckRule::OnlyWhenPlayingForAnte",
                    parts,
                );
            }
            // How to build a deck, in no game: Cantrip's Sphinx's Approach says
            // the same of its line, "no line of code here".
            Ability::DeckRule(_) => {}
            other => return self.ability_rest(other, label, parts),
        }
        Ok(())
    }
}
