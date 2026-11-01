//! Abilities and costs: what a card's `CardDef` is made of. An ability that
//! cannot be generated reports its gap and contributes nothing, and `module`
//! then publishes nothing, so no card loses an ability silently.
use super::*;

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
fn own_graveyard(objects: &ObjectSet) -> Option<ObjectSet> {
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
fn yours(filter: &ObjectFilter) -> bool {
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

impl Compiler {
    /// A generated `ObjectFilter`: what a cost accepts of an object.
    pub(super) fn cost_filter(&mut self, filter: &ObjectFilter, label: &str) -> Result<String> {
        // A cost's filter is a plain function, with no event or stack item to read.
        let saved = (self.reader, self.event_at);
        (self.reader, self.event_at) = (Reader::Query, EventAt::Nowhere);
        let test = self.filter(filter, &Candidate::of("id"));
        (self.reader, self.event_at) = saved;
        let function = format!("{label}_{}", self.fresh());
        writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, id: ObjectId) -> bool {{ {} }}", test?).unwrap();
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
                let [CostObjectSelection { objects, choose }] = selections.as_slice() else {
                    return Err(gap("move cost shape"));
                };
                if self.links_exile && matches!(destination, Destination::Exile) {
                    return Err(gap("exile cost an ability reads as exiled with its source"));
                }
                let n = exactly_u8(choose.as_ref())
                    .ok_or_else(|| gap("move cost without a fixed count"))?;
                let yours = own_graveyard(objects);
                let objects = yours.as_ref().unwrap_or(objects);
                match (destination, objects) {
                    (Destination::OwnersHand, ObjectSet::Battlefield(filter)) => {
                        format!("return_to_hand({n}, {})", self.cost_filter(filter, label)?)
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
                    (destination, objects) => {
                        return Err(gap(format!(
                            "move cost {} from {}",
                            form(destination),
                            form(objects)
                        )))
                    }
                }
            }
            other => return Err(gap(format!("cost part {}", form(other)))),
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
                other => others.push(self.cost_part(other, label)?),
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
            implied,
            unchosen,
            surcharge,
            distinct_targets,
            chooser,
            random,
            budget,
            may_decline,
        } = selection;
        let bound = |n: &Expr| match n {
            Expr::Constant(n) => u8::try_from(*n).ok(),
            _ => None,
        };
        // "Choose one. If you control a commander, you may choose both instead."
        let (most, more) = match maximum {
            Expr::Conditional {
                condition,
                then,
                otherwise,
            } if bound(then).is_some() => (&**otherwise, Some((condition, bound(then)))),
            most => (most, None),
        };
        let (Some(min), Some(max)) = (bound(minimum), bound(most)) else {
            return Err(gap("mode count that is not a constant"));
        };
        let refused = [
            (*implied, "modes implied by the target chosen"),
            (unchosen.is_some(), "modes limited by earlier choices"),
            (*distinct_targets, "modes with distinct targets"),
            (chooser.is_some(), "modes chosen by another player"),
            (*random, "modes chosen at random"),
            (budget.is_some(), "modes with a budget"),
            (*may_decline, "modes that may all be declined"),
        ];
        if let Some((_, feature)) = refused.iter().find(|(applies, _)| *applies) {
            return Err(gap(*feature));
        }
        let mut defs = Vec::new();
        for (i, mode) in modes.iter().enumerate() {
            if mode.requires.is_some() {
                return Err(gap("mode with a requirement"));
            }
            let label = format!("{label}_mode_{i}");
            let targets = match frame {
                Some(frame) => self.targets_for(&mode.announcement, &label, frame)?,
                None => self.announcement(&mode.announcement, &label)?,
            };
            let effect = self.effect(&mode.body, &label)?;
            defs.push(format!(
                "mode({effect}, {}).targets({targets})",
                quoted(&ability_text(&mode.body))
            ));
        }
        let mut modal = format!("modes(&[{}]).choose({min}, {max})", defs.join(","));
        if let Some((condition, Some(most))) = more {
            let test = self.activation_condition(condition, &format!("{label}_more_modes"))?;
            modal.push_str(&format!(".up_to_if({test}, {most})"));
        }
        if *repeat {
            modal.push_str(".repeat()");
        }
        if let Some(surcharge) = surcharge {
            let cost = self.cost(surcharge, label)?;
            modal.push_str(&format!(".escalate({cost})"));
        }
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
        writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker) -> bool {{ let o = q.state().obj(asker.source); let this = AbilityRef {{ card: o.copied_card().unwrap_or(o.card), index: {index} }}; (q.history().this_turn().iter().filter(|r| matches!(r.event, Event::AbilityActivated {{ source, ability, .. }} if source == asker.source && ability == this)).count() as i64) < {n} }}").unwrap();
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
        if limits.activators.is_some() {
            return Err(gap("activation by other players"));
        }
        let waterbent = waterbend_as_mana(price);
        let price = waterbent.as_ref().unwrap_or(price);
        let text = ability_text(body);
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
        let mut def = if announcement.modes.is_some() {
            let modal = self.modal(announcement, label, None)?;
            format!(
                "modal_ability({}, &{modal}, {})",
                self.cost(price, label)?,
                quoted(&text)
            )
        } else if crate::semantics::activation_kind(price, announcement, body)
            == crate::semantics::ActivationKind::Mana
        {
            let adds = self.mana_ability(body, &format!("{label}_mana"))?;
            self.paying = Paying::Mana;
            let cost = self.cost(price, label);
            self.paying = Paying::Ability;
            format!(
                "mana_ability({}, {adds}, {}){}",
                cost?,
                quoted(&text),
                self.colors_from(body, label)?
            )
        } else {
            let targets = self.announcement(announcement, label)?;
            let effect = self.effect(body, label)?;
            format!(
                "ability({}, {effect}, {}).targets({targets})",
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
                other => return Err(gap(format!("activation rule {}", form(other)))),
            }
        }
        match limits.keyword {
            Some(ActivatedKeyword::Exhaust) => def.push_str(".exhaust()"),
            Some(ActivatedKeyword::PowerUp) => def.push_str(".power_up()"),
            Some(ActivatedKeyword::Boast) => {
                if limits.condition.is_some() {
                    return Err(gap("boast with a condition of its own"));
                }
                let function = format!("{label}_attacked");
                writeln!(self.functions, "fn {function}(q: &dyn Query, asker: &Asker) -> bool {{ q.attacked_this_turn(asker.source) }}").unwrap();
                def.push_str(&format!(".condition({function})"));
            }
            Some(ActivatedKeyword::Forecast) => {
                def.push_str(".forecast()");
            }
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
        Ok(def)
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
                other => return Err(gap(format!("backup grants {}", form(other)))),
            }
        }
        let ability = granted.map_or("None".to_string(), |i| format!("Some({i})"));
        let function = format!("{label}_backup");
        writeln!(self.functions, "fn {function}(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {{ cantrip_mechanics::backup::resolve(g, item, {n}, &[{}], {ability}) }}", keywords.join(",")).unwrap();
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
            if stage.animates {
                return Err(gap("counter stage that animates"));
            }
            let least = compare(Comparison::GreaterOrEqual, stage.minimum);
            let condition = match stage.maximum {
                Some(most) => Condition::All(vec![least, compare(Comparison::LessOrEqual, most)]),
                None => least,
            };
            let mut then: Vec<Ability> = stage
                .power_toughness
                .map(|(power, toughness)| Ability::Static {
                    kind: StaticKind::Ordinary,
                    affected: EffectSubjects::Objects(ObjectSet::One(ObjectRef::Source)),
                    changes: vec![Change::SetBasePowerToughness {
                        power: Expr::Constant(power.into()),
                        toughness: Expr::Constant(toughness.into()),
                    }],
                })
                .into_iter()
                .collect();
            then.extend(stage.abilities.iter().cloned());
            let staged = Ability::Conditional {
                condition,
                then,
                otherwise: Vec::new(),
            };
            self.ability(&staged, &format!("{label}_stage_{i}"), parts, None)?;
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
            Ability::Keyword(k) => {
                match k {
                    Keyword::Prowess => parts
                        .abilities
                        .push("cantrip_mechanics::triggers::PROWESS".into()),
                    Keyword::Flanking => parts
                        .abilities
                        .push("cantrip_mechanics::flanking::TRIGGER".into()),
                    Keyword::Ward(cost) => {
                        let ward = match cost.parts.as_slice() {
                        [CostPart::Mana(symbols)] => match symbols.as_slice() {
                            [ManaSymbol::Generic(n)] if u8::try_from(*n).is_ok() => {
                                format!("cantrip_mechanics::triggers::ward::<{n}>(\"Ward {{{n}}}\")")
                            }
                            _ => return Err(gap("ward with colored mana")),
                        },
                        [CostPart::Life(Expr::Constant(n))] if u8::try_from(*n).is_ok() => {
                            format!("cantrip_mechanics::triggers::ward_life::<{n}>(\"Ward--Pay {n} life\")")
                        }
                        [CostPart::Discard {
                            count: Expr::Constant(1),
                            filter: ObjectFilter::Any,
                            random: false,
                        }] => "cantrip_mechanics::triggers::ward_discard(\"Ward--Discard a card\")".into(),
                        other => return Err(gap(format!("ward cost {}", form(&other[0])))),
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
                        Protection::Objects { filter, .. } => {
                            return Err(gap(format!("protection from {}", form(filter))))
                        }
                        other => return Err(gap(format!("protection {}", form(other)))),
                    },
                    Keyword::HexproofFrom(quality) => parts.field(
                        "hexproof_from",
                        lane_j::hexproof_quality(quality)?,
                    ),
                    k => parts.keywords.push(keyword(k)?),
                }
            }
            Ability::Casting(rule) => self.casting(rule, label, parts, None)?,
            Ability::CostBranch {
                keyword,
                cost,
                paid,
                unpaid,
            } => self.cost_branch(keyword, cost, paid, unpaid, label, parts)?,
            Ability::Scoped {
                zones,
                condition,
                ability,
            } => {
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
                    other => return Err(gap(format!("source-zone scope on {}", form(other)))),
                }
            }
            Ability::Spell { announcement, body } => {
                if parts.spell != "None" {
                    return Err(gap("multiple spell bodies"));
                }
                if announcement.modes.is_some() {
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
                    if let Some(divide) = self.dividing.take() {
                        parts.fields.insert("divide", format!("Some({divide})"));
                    }
                }
            }
            Ability::Activated {
                origin,
                cost,
                limits,
                announcement,
                body,
            } => {
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
                    parts.abilities.push(def?);
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
                if let Some(def) = self.trigger_entry(def, parts)? {
                    parts.abilities.push(def);
                }
            }
            Ability::Static {
                kind,
                affected,
                changes,
            } => {
                self.static_ability(label, kind, affected, changes, parts)?;
            }
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
                if !otherwise.is_empty() {
                    return Err(gap("conditional ability with an otherwise"));
                }
                if self.static_guard.is_some() {
                    return Err(gap("nested conditional ability"));
                }
                self.static_guard = Some(condition.clone());
                let mut compiled = Ok(());
                for (i, inner) in then.iter().enumerate() {
                    let step = match inner {
                        Ability::Static { .. } => {
                            self.ability(inner, &format!("{label}_{i}"), parts, scope)
                        }
                        Ability::Keyword(k) => self.static_ability(
                            &format!("{label}_{i}"),
                            &StaticKind::Ordinary,
                            &EffectSubjects::Objects(ObjectSet::One(ObjectRef::Source)),
                            &[Change::GrantKeyword(k.clone())],
                            parts,
                        ),
                        other => Err(gap(format!("conditional {}", form(other)))),
                    };
                    if step.is_err() {
                        compiled = step;
                        break;
                    }
                }
                self.static_guard = None;
                compiled?;
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
                    let targets = self.announcement(announcement, &label)?;
                    let effect = self.effect(body, &label)?;
                    parts.abilities.push(format!(
                        "chapter({n}, {effect}, {}).targets({targets})",
                        quoted(&ability_text(body))
                    ));
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
            } => self.mechanic(
                *mechanic,
                cost.as_ref(),
                amount.as_ref(),
                filter.as_ref(),
                label,
                parts,
            )?,
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
                return Err(gap("card outside ordinary play"))
            }
            // How to build a deck, in no game: Cantrip's Sphinx's Approach says
            // the same of its line, "no line of code here".
            Ability::DeckRule(_) => {}
            other => return self.ability_rest(other, label, parts),
        }
        Ok(())
    }
}
