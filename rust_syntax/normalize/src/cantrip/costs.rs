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
    fn cost_filter(&mut self, filter: &ObjectFilter, label: &str) -> Result<String> {
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
                let n = exactly_u8(choose.as_ref())
                    .ok_or_else(|| gap("sacrifice cost without a fixed count"))?;
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
                let n =
                    constant_u8(count).ok_or_else(|| gap("discard cost without a fixed count"))?;
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
                let n = exactly_u8(choose.as_ref())
                    .ok_or_else(|| gap("tap cost without a fixed count"))?;
                format!("tap_untapped({n}, {})", self.cost_filter(filter, label)?)
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
                let n = constant_u8(n).ok_or_else(|| gap("counter cost without a fixed count"))?;
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
        let (Some(min), Some(max)) = (bound(minimum), bound(maximum)) else {
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
    fn activated(
        &mut self,
        label: &str,
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
        if !limits.modifiers.is_empty() {
            return Err(gap("activation modifier"));
        }
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
            format!(
                "mana_ability({}, {adds}, {})",
                self.cost(price, label)?,
                quoted(&text)
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
        match limits.keyword {
            Some(ActivatedKeyword::Exhaust) => def.push_str(".exhaust()"),
            Some(ActivatedKeyword::PowerUp) => def.push_str(".power_up()"),
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
        match conditions.as_slice() {
            [] => {}
            [condition] => {
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
                let def = self.activated(label, origin, cost, limits, announcement, body, scope);
                self.activation_cost = None;
                parts.abilities.push(def?);
            }
            Ability::Triggered {
                event,
                intervening_if,
                limits,
                announcement,
                body,
            } => {
                let mut def = self.triggered(
                    label,
                    event,
                    intervening_if.as_ref(),
                    limits,
                    announcement,
                    body,
                )?;
                if let Some(scope) = scope {
                    if scope.condition.is_some() {
                        return Err(gap("conditional scope on a triggered ability"));
                    }
                    def.push_str(&format!(".functions_in({})", zone_list(scope.zones)?));
                }
                parts.abilities.push(def);
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
            Ability::Primitive { name, .. } => {
                return Err(gap(format!("primitive ability {name}")))
            }
            other => return Err(gap(format!("ability {}", form(other)))),
        }
        Ok(())
    }
}
