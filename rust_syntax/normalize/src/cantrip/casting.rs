//! How a spell is cast (CR 601.2): the card's rules for costs and permissions,
//! and spells with two announcements chosen by a cost.
use super::*;

const KICKED_SPELL: &str = "q.obj(asker.source).kicked > 0";

/// A target count the engine holds as numbers.
fn target_range(count: &Count) -> Result<(u32, u32)> {
    let constant = |n: &Expr| match n {
        Expr::Constant(n) => u32::try_from(*n).ok(),
        _ => None,
    };
    let range = match count {
        Count::Exactly(n) => constant(n).map(|n| (n, n)),
        Count::UpTo(n) => constant(n).map(|n| (0, n)),
        Count::Between { minimum, maximum } => constant(minimum).zip(constant(maximum)),
        _ => None,
    };
    range.ok_or_else(|| gap(format!("target count by branch {}", form(count))))
}

impl Compiler {
    /// `Ability::CostBranch`.
    pub(super) fn cost_branch(
        &mut self,
        keyword: &str,
        cost: &Cost,
        paid: &Ability,
        unpaid: &Ability,
        label: &str,
        parts: &mut Parts,
    ) -> Result<()> {
        let (
            Ability::Spell {
                announcement: paid_announced,
                body: paid_body,
            },
            Ability::Spell {
                announcement: unpaid_announced,
                body: unpaid_body,
            },
        ) = (paid, unpaid)
        else {
            return Err(gap("cost branch of abilities that are not spells"));
        };
        if parts.spell != "None" {
            return Err(gap("multiple spell bodies"));
        }
        let same_targets =
            format!("{:?}", paid_announced.targets) == format!("{:?}", unpaid_announced.targets);
        match keyword {
            "Kicker" => {
                parts.field("kicker", self.kicker(cost, label)?);
                if !same_targets {
                    parts.field("late_targets", "true".into());
                }
                if !matches!(cost.parts.as_slice(), [CostPart::Mana(_)]) {
                    self.cost_labels(cost, parts);
                }
                let targets = self.branch_targets(paid_announced, unpaid_announced, label)?;
                let effect = self.branch_effect(
                    "item.kicked > 0",
                    paid_body,
                    paid_announced,
                    unpaid_body,
                    unpaid_announced,
                    label,
                )?;
                parts.spell = format!("spell({targets}, {effect})");
            }
            "Overload" => {
                if !paid_announced.targets.is_empty() {
                    return Err(gap("overload with targets"));
                }
                let [CostPart::Mana(symbols)] = cost.parts.as_slice() else {
                    return Err(gap("overload cost other than mana"));
                };
                parts.field("overload", format!("Some({})", mana(symbols)?));
                let targets = self.announcement(unpaid_announced, &format!("{label}_unpaid"))?;
                let effect = self.branch_effect(
                    "item.way == WAY_OVERLOAD",
                    paid_body,
                    paid_announced,
                    unpaid_body,
                    unpaid_announced,
                    label,
                )?;
                parts.spell = format!("spell({targets}, {effect})");
            }
            "Cleave" => {
                if !same_targets {
                    return Err(gap("cleave that changes its targets"));
                }
                let [CostPart::Mana(symbols)] = cost.parts.as_slice() else {
                    return Err(gap("cleave cost other than mana"));
                };
                let text = quoted(&self.said_word("Cleave".into()));
                parts.table(
                    "ways",
                    format!(
                        "cantrip_mechanics::cleave::way({}, {text})",
                        mana(symbols)?
                    ),
                );
                let targets = self.announcement(unpaid_announced, &format!("{label}_unpaid"))?;
                let effect = self.branch_effect(
                    "item.way == 1",
                    paid_body,
                    paid_announced,
                    unpaid_body,
                    unpaid_announced,
                    label,
                )?;
                parts.spell = format!("spell({targets}, {effect})");
            }
            "Entwine" => {
                let (Some(paid_modes), Some(_)) = (&paid_announced.modes, &unpaid_announced.modes)
                else {
                    return Err(gap("entwine of a spell without modes"));
                };
                let (Expr::Constant(most), true) = (
                    &paid_modes.maximum,
                    format!("{:?}", paid_modes.modes)
                        == format!(
                            "{:?}",
                            unpaid_announced.modes.as_ref().map(|m| &m.modes).unwrap()
                        ),
                ) else {
                    return Err(gap("entwine that changes its modes"));
                };
                let most = u8::try_from(*most).map_err(|_| gap("mode count beyond u8"))?;
                parts.field("kicker", self.kicker(cost, label)?);
                let modal = self.modal(unpaid_announced, label, None)?;
                writeln!(
                    self.functions,
                    "static {modal}_ENTWINED: ModalDef = {modal}.when_paid({most}, {most});"
                )
                .unwrap();
                parts.spell = "Some(SpellDef::MODAL)".into();
                parts.modal = format!("Some(&{modal}_ENTWINED)");
            }
            other => self.cost_branch_beyond(other, cost, paid, unpaid, label, parts)?,
        }
        Ok(())
    }
    pub(super) fn kicker(&mut self, cost: &Cost, label: &str) -> Result<String> {
        if let [CostPart::Mana(symbols)] = cost.parts.as_slice() {
            return Ok(format!("kicker({})", mana(symbols)?));
        }
        let extra = self.cost(cost, &format!("{label}_kicker"))?;
        Ok(format!(
            "Some(Kicker {{ cost: ManaCost::ZERO, extra: Some({extra}), multi: false, waterbend: false }})"
        ))
    }
    /// One effect that runs the branch `test` picks as it resolves. The two
    /// bodies number their targets alike, so each reads the slots it announced.
    pub(super) fn branch_effect(
        &mut self,
        test: &str,
        paid: &Program,
        paid_announced: &Announcement,
        unpaid: &Program,
        unpaid_announced: &Announcement,
        label: &str,
    ) -> Result<String> {
        let (paid_label, unpaid_label) = (format!("{label}_paid"), format!("{label}_unpaid"));
        self.resolving_slots(paid_announced)?;
        self.effect(paid, &paid_label)?;
        self.resolving_slots(unpaid_announced)?;
        self.effect(unpaid, &unpaid_label)?;
        writeln!(self.functions, "fn {label}(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {{ if {test} {{ {paid_label}(g, item) }} else {{ {unpaid_label}(g, item) }} }}").unwrap();
        Ok(label.into())
    }
    /// The slots an effect body reads, for announcements compiled elsewhere.
    fn resolving_slots(&mut self, announced: &Announcement) -> Result<()> {
        self.slots.clear();
        for (i, spec) in announced.targets.iter().enumerate() {
            self.slots.insert(spec.slot.0, (i, spec.clone()));
        }
        Ok(())
    }
    /// A spell's targets where a kicker changes them: its slots are asked once
    /// the kicker is announced, and each slot reads which it is (CR 601.2c).
    fn branch_targets(
        &mut self,
        paid: &Announcement,
        unpaid: &Announcement,
        label: &str,
    ) -> Result<String> {
        if paid.modes.is_some() || unpaid.modes.is_some() {
            return Err(gap("kicked spell with modes"));
        }
        // Compiling each announcement generates its slots' filters.
        self.announcement(paid, &format!("{label}_paid"))?;
        let paid_filters = std::mem::take(&mut self.announced_filters);
        self.announcement(unpaid, &format!("{label}_unpaid"))?;
        let unpaid_filters = std::mem::take(&mut self.announced_filters);
        let mut specs = Vec::new();
        for i in 0..paid.targets.len().max(unpaid.targets.len()) {
            let range = |targets: &[TargetSpec]| {
                targets
                    .get(i)
                    .map_or(Ok((0, 0)), |spec| target_range(&spec.count))
            };
            let ((paid_min, paid_max), (min, max)) =
                (range(&paid.targets)?, range(&unpaid.targets)?);
            let side = |filters: &[String]| {
                filters
                    .get(i)
                    .map_or("false".to_string(), |f| format!("({f})(q, asker, t)"))
            };
            let function = format!("{label}_target_{i}");
            writeln!(
                self.functions,
                "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, t: Target) -> bool {{ if {KICKED_SPELL} {{ {} }} else {{ {} }} }}",
                side(&paid_filters),
                side(&unpaid_filters)
            )
            .unwrap();
            specs.push(format!(
                "TargetSpec {{ min: {min}, max: {max}, filter: {function}, label: \"\", if_kicked: Some(({paid_min}, {paid_max})), together: None }}"
            ));
        }
        if specs.is_empty() {
            return Ok("&[]".into());
        }
        let array = format!("{}_TARGETS", label.to_ascii_uppercase());
        writeln!(
            self.functions,
            "static {array}: &[cantrip_api::TargetSpec] = &[{}];",
            specs.join(",")
        )
        .unwrap();
        Ok(array)
    }
    /// `Ability::Casting`.
    pub(super) fn casting(
        &mut self,
        rule: &CastingRule,
        label: &str,
        parts: &mut Parts,
        condition: Option<&Condition>,
    ) -> Result<()> {
        let saved = self.paying;
        self.paying = lane_g::Paying::of_rule(rule);
        let done = self.casting_rule(rule, label, parts, condition);
        self.paying = saved;
        done
    }
    fn casting_rule(
        &mut self,
        rule: &CastingRule,
        label: &str,
        parts: &mut Parts,
        condition: Option<&Condition>,
    ) -> Result<()> {
        let one_field = |parts: &mut Parts, field: &'static str, value: String| {
            if parts.fields.contains_key(field) {
                return Err(gap(format!("two rules for {field}")));
            }
            parts.field(field, value);
            Ok(())
        };
        let way = |c: &mut Self, way: String, label: &str| -> Result<String> {
            Ok(match condition {
                Some(condition) => {
                    let test = c.activation_condition(condition, &format!("{label}_if"))?;
                    format!("{way}.condition({test})")
                }
                None => way,
            })
        };
        match rule {
            CastingRule::If {
                condition: inner,
                rule,
            } => {
                if condition.is_some() {
                    return Err(gap("casting rule under two conditions"));
                }
                self.casting(rule, label, parts, Some(inner))?;
            }
            CastingRule::Flash {
                additional_cost: None,
            } => match condition {
                Some(condition) => {
                    let test = self.activation_condition(condition, &format!("{label}_if"))?;
                    one_field(parts, "flash_if", format!("Some({test})"))?;
                }
                None => one_field(parts, "cast_as_though_flash", "true".into())?,
            },
            CastingRule::AlternativeCost(cost) => {
                let cost = self.cost(cost, label)?;
                let text = quoted(&self.said(lane_k::Said::Sentence, "Alternative cost".into()));
                let def = way(self, format!("way({cost}, ZoneKind::Hand, {text})"), label)?;
                parts.table("ways", def);
            }
            CastingRule::AlternativeCostWithFlash(cost) => {
                let cost = self.cost(cost, label)?;
                let text = "Alternative cost with flash".to_string();
                let text = quoted(&self.said(lane_k::Said::Sentence, text));
                let def = way(
                    self,
                    format!("way({cost}, ZoneKind::Hand, {text}).instant_speed()"),
                    label,
                )?;
                parts.table("ways", def);
            }
            CastingRule::Permission {
                origin,
                player: PlayerRef::OwnerOf(owner),
                from:
                    ZoneScope::Players {
                        zone: EventZone::Graveyard,
                        ..
                    },
                require,
                cost,
                leave_stack,
            } if matches!(**owner, ObjectRef::Source) => {
                let exile = match leave_stack {
                    Some(Destination::Exile) => true,
                    None => false,
                    Some(other) => {
                        return Err(gap(format!(
                            "cast from a graveyard and left to {}",
                            form(other)
                        )))
                    }
                };
                let cost = self.cost(cost, label)?;
                let text = if matches!(origin, CastingOrigin::Flashback) {
                    self.said_word("Flashback".into())
                } else {
                    self.said(lane_k::Said::Sentence, "Cast from your graveyard".into())
                };
                let text = quoted(&text);
                let mut def = format!("way({cost}, ZoneKind::Graveyard, {text}).only_from()");
                if exile {
                    def.push_str(".exile_after()");
                }
                if !matches!(require, ObjectFilter::Any) {
                    if condition.is_some() {
                        return Err(gap("permission with a requirement and a condition"));
                    }
                    let saved = std::mem::replace(&mut self.reader, Reader::Query);
                    let test = self.filter(require, &Candidate::of("asker.source"));
                    self.reader = saved;
                    let function = format!("{label}_require");
                    writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker) -> bool {{ {} }}", test?).unwrap();
                    def.push_str(&format!(".condition({function})"));
                }
                let def = way(self, def, label)?;
                parts.table("ways", def);
            }
            CastingRule::AdditionalCost {
                cost,
                optional: false,
                repetition: CostRepetition::Once,
            } if condition.is_none() => {
                if let [CostPart::Choice(costs)] = cost.parts.as_slice() {
                    let printed = self.said_costs(costs.len());
                    for (i, choice) in costs.iter().enumerate() {
                        let cost = self.cost(choice, &format!("{label}_choice_{i}"))?;
                        let text = printed.get(i).cloned();
                        parts.table(
                            "additional_choice",
                            format!(
                                "ExtraCost {{ text: {}, cost: {cost} }}",
                                quoted(&text.unwrap_or_else(|| format!("Cost {}", i + 1)))
                            ),
                        );
                    }
                } else {
                    let cost = self.cost(cost, label)?;
                    one_field(parts, "additional_cost", format!("Some({cost})"))?;
                }
            }
            CastingRule::ReduceCost(reductions) => {
                let [ManaReduction {
                    color: None,
                    amount,
                }] = reductions.as_slice()
                else {
                    return Err(gap("spell cost reduction of colored mana"));
                };
                let saved = (self.reader, self.event_at);
                (self.reader, self.event_at) = (Reader::Query, EventAt::Nowhere);
                self.costing_targets = true;
                let reads = self
                    .number(amount)
                    .and_then(|n| n.fit(0, 255, "u8"))
                    .and_then(|n| Ok((n, condition.map(|c| self.condition(c)).transpose()?)));
                self.costing_targets = false;
                (self.reader, self.event_at) = saved;
                let (amount, test) = reads?;
                let test = test.unwrap_or_else(|| "true".into());
                let function = format!("{label}_cost");
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, source: ObjectId, player: PlayerId, costed: Costed, targets: &[Target]) -> CostDelta {{ let asker = &Asker::new(source, player); if costed == Costed::Spell(source) && {test} {{ CostDelta::less({amount}) }} else {{ CostDelta::NONE }} }}").unwrap();
                parts.table("own_cost_changes", function);
            }
            CastingRule::Require(required) if condition.is_none() => {
                let test = self.activation_condition(required, &format!("{label}_if"))?;
                one_field(parts, "cast_only_if", format!("Some({test})"))?;
            }
            other => self.casting_beyond(other, label, parts, condition)?,
        }
        Ok(())
    }
}
