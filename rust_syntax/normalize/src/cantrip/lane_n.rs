//! Events as they would happen and what their effects do about them: the
//! replacement events `watched` has no word for, the changes `reshape` has none
//! for, programs whose changes do not stand first, preventions that do something
//! besides, and what an effect made at resolution reads of the ability that made
//! it. Where Cantrip has no word for an event, the callbacks are written all the
//! same and held back until it has.
use super::*;
use replacements::Watched;

/// What compiling one replacement program has found out so far.
#[derive(Clone, Default)]
pub(super) struct Replacing {
    /// The event is replaced outright, so a `Cancel` in `then` has been said.
    canceled: bool,
    /// Changes that hang on a condition the callback can read.
    guarded: Vec<(Condition, Vec<EventChange>, Vec<EventChange>)>,
    /// The proposal `then` was compiled for: its variant, binding and frame.
    pub event: Option<(&'static str, &'static str, Frame)>,
    /// The name of the question for "you may".
    pub may: Option<String>,
    /// The result that says how much a prevention prevented.
    prevented: Option<u32>,
    /// The feature an event's callbacks wait on, when Cantrip has no such event.
    pub planned: Option<String>,
    /// Those callbacks, kept from the file's compiled functions.
    held: String,
    /// Builder calls the definition will take: a feature each.
    suffix: Vec<(String, String)>,
    /// The field of `PlayerCounters` the event is about.
    player_counter: Option<&'static str>,
    /// The results of the changes made: `then` runs because they were.
    pub changed: Vec<u32>,
    /// The kind of event the program replaces.
    pub kind: &'static str,
    /// Tokens are replaced one at a time.
    pub each_token: bool,
    /// Changes no `EventChange` says, as statements of `apply`.
    pub edits: String,
}

/// The events of one ability that are not one proposal: counters on objects and
/// counters on players are different proposals.
pub(super) fn separate_events(event: &ReplacementEvent) -> Option<Vec<ReplacementEvent>> {
    let ReplacementEvent::Counters {
        kind,
        objects,
        players: Some(players),
        by,
        effect_only,
    } = event
    else {
        return None;
    };
    if no_objects(objects) {
        return None;
    }
    Some(vec![
        ReplacementEvent::Counters {
            kind: kind.clone(),
            objects: objects.clone(),
            players: None,
            by: by.clone(),
            effect_only: *effect_only,
        },
        ReplacementEvent::Counters {
            kind: kind.clone(),
            objects: ObjectFilter::Not(Box::new(ObjectFilter::Any)),
            players: Some(players.clone()),
            by: by.clone(),
            effect_only: *effect_only,
        },
    ])
}

fn no_objects(filter: &ObjectFilter) -> bool {
    matches!(filter, ObjectFilter::Not(any) if matches!(**any, ObjectFilter::Any))
}

/// Which arm of `reshape` a proposal takes.
pub(super) fn shape_of(kind: &'static str, variant: &'static str) -> &'static str {
    if variant == "PlayerCounters" {
        variant
    } else {
        kind
    }
}

/// Whether these are the changes of a prevention (CR 615).
pub(super) fn prevents(changes: &[EventChange]) -> bool {
    matches!(
        changes,
        [EventChange::PreventDamage
            | EventChange::PreventSomeDamage(_)
            | EventChange::PreventAllButDamage(_)]
    )
}

/// The definition of a prevention an effect made, with what Cantrip's has no
/// field for yet.
pub(super) fn granted_prevention(
    hint: &str,
    applies: &str,
    then: Option<&str>,
    of: Option<&str>,
    may: Option<&str>,
) -> String {
    let base = format!(
        "GrantedPreventionDef {{ hint: {hint}, damage: \"damage\", of_it: None, applies: {applies} }}"
    );
    let mut more = String::new();
    if let Some(then) = then {
        write!(more, " then: Some({then}),").unwrap();
    }
    if let Some(of) = of {
        write!(more, " amount: Some({of}),").unwrap();
    }
    if let Some(may) = may {
        write!(more, " optional: Some(&{may}),").unwrap();
    }
    if more.is_empty() {
        return base;
    }
    let feature = if then.is_some() {
        "prevention with an effect besides"
    } else if of.is_some() {
        "prevention of part of the damage made at resolution"
    } else {
        "prevention a player may decline"
    };
    planned::planned_value(
        feature,
        &base,
        &format!(
            "GrantedPreventionDef {{ hint: {hint}, damage: \"damage\", of_it: None, applies: {applies},{more} }}"
        ),
    )
}

/// A redirection used up by how much it has moved: Cantrip's shields are for
/// prevention alone.
pub(super) fn shielded_redirect(index: usize) -> String {
    format!(
        "let _ = to; g.add_effect(item.object, about, EffectKind::Redirect {{ card: item.ability.card, index: {index}, shield: Shield::Next(shield) }}, DURATION);"
    )
}

/// Whether a program contains a change to its event where `then` would run it.
fn changes_event(program: &Program) -> bool {
    let mut steps = Vec::new();
    lane_m::flat_groups(&program.instructions, &mut steps);
    steps.iter().any(|step| match step {
        Instruction::Effect {
            operation: Operation::ModifyEvent { .. },
            ..
        } => true,
        Instruction::Effect {
            operation: Operation::ChooseAction { options, .. },
            ..
        } => options
            .iter()
            .any(|o| lane_l::action_program(o).is_some_and(|p| changes_event(&p))),
        Instruction::If {
            then, otherwise, ..
        } => changes_event(then) || changes_event(otherwise),
        Instruction::Optional { body, .. } => changes_event(body),
        Instruction::ChooseOne { options, .. } => options.iter().any(changes_event),
        _ => false,
    })
}

/// What "you may" gives the choice of, when it is the whole of a program that
/// changes its event, and what follows "if you do".
fn chosen_instead(steps: &[Instruction]) -> Option<(Program, Vec<Instruction>)> {
    match steps {
        [Instruction::Optional {
            decider: PlayerRef::AbilityController,
            body,
        }] if changes_event(body) => Some((body.clone(), Vec::new())),
        [Instruction::Effect {
            result,
            operation:
                Operation::PayCost {
                    player: PlayerRef::AbilityController,
                    cost,
                    optional: true,
                },
        }, then @ ..] => {
            let [CostPart::PlayerAction { body, .. }] = cost.parts.as_slice() else {
                return None;
            };
            if !changes_event(body) {
                return None;
            }
            let mut rest = Vec::new();
            for step in then {
                match step {
                    Instruction::If {
                        condition: Condition::PaymentChosen(chosen),
                        then,
                        otherwise,
                    } if chosen.0 == result.0 && otherwise.instructions.is_empty() => {
                        lane_m::flat_groups(&then.instructions, &mut rest)
                    }
                    _ => return None,
                }
            }
            Some((body.clone(), rest))
        }
        _ => None,
    }
}

/// Whether every way through these steps replaces the event outright.
fn always_cancels(steps: &[Instruction]) -> bool {
    steps.iter().any(|step| match step {
        Instruction::Effect {
            operation: Operation::ModifyEvent { change },
            ..
        } => matches!(change, EventChange::Cancel),
        Instruction::Group(group) => always_cancels(&group.instructions),
        Instruction::If {
            then, otherwise, ..
        } => always_cancels(&then.instructions) && always_cancels(&otherwise.instructions),
        Instruction::ChooseOne { options, .. } => {
            !options.is_empty() && options.iter().all(|o| always_cancels(&o.instructions))
        }
        Instruction::Effect {
            operation: Operation::ChooseAction { options, .. },
            ..
        } => {
            !options.is_empty()
                && options.iter().all(|o| {
                    lane_l::action_program(o).is_some_and(|p| always_cancels(&p.instructions))
                })
        }
        _ => false,
    })
}

/// Whether a program prevents its event, whatever stands around that.
fn prevents_at_all(program: &Program) -> bool {
    let mut steps = Vec::new();
    lane_m::flat_groups(&program.instructions, &mut steps);
    steps.iter().any(|step| match step {
        Instruction::Effect {
            operation: Operation::ModifyEvent { change },
            ..
        } => prevents(std::slice::from_ref(change)),
        Instruction::If { then, .. } => prevents_at_all(then),
        Instruction::Optional { body, .. } => prevents_at_all(body),
        _ => false,
    })
}

/// Whether a delayed ability waits for an event rather than a time: every event,
/// and a step that only counts this turn, which is that event once.
pub(super) fn waits_for_an_event(event: &EventSpec, expires: &Option<crate::ir::Duration>) -> bool {
    match event {
        EventSpec::Turn { .. } => matches!(expires, Some(crate::ir::Duration::EndOfTurn)),
        _ => true,
    }
}

/// Just the changes of a step that holds nothing else.
fn only_changes(program: &Program) -> Option<Vec<EventChange>> {
    let mut steps = Vec::new();
    lane_m::flat_groups(&program.instructions, &mut steps);
    steps
        .into_iter()
        .map(|step| match step {
            Instruction::Effect {
                operation: Operation::ModifyEvent { change },
                ..
            } => Some(change),
            _ => None,
        })
        .collect()
}

impl Compiler {
    /// Where the callbacks of the replacement being compiled are written.
    pub(super) fn callbacks(&mut self) -> &mut String {
        if self.replacing.planned.is_some() {
            &mut self.replacing.held
        } else {
            &mut self.functions
        }
    }
    /// Held callbacks wait with the entry that names them.
    fn release_callbacks(&mut self, feature: &str) {
        let held = std::mem::take(&mut self.replacing.held);
        if held.is_empty() {
            return;
        }
        if planned::landed(feature) {
            self.functions.push_str(&held);
        } else {
            let id = planned::requirement(feature);
            writeln!(self.holes, "planned_item!({id:?} => callbacks: {held});").unwrap();
        }
    }
    /// A finished replacement goes on the card, or waits on what it needs.
    pub(super) fn add_replacement(&mut self, mut def: String, parts: &mut Parts) {
        let mut feature = self.replacing.planned.take();
        for (needs, call) in std::mem::take(&mut self.replacing.suffix) {
            def.push_str(&call);
            feature.get_or_insert(needs);
        }
        match feature {
            None => parts.replacements.push(def),
            Some(feature) => {
                self.release_callbacks(&feature);
                self.plan_entry(&feature, "replacements", &def, parts);
            }
        }
    }
    /// A replacement a spell or ability made, as `add_replacement` for the
    /// definitions those keep.
    /// What a permanent entering attacking attacks (CR 508.4): `Target::None`
    /// when there is nothing it could.
    pub(super) fn entering_defender(
        &self,
        player: Option<&PlayerRef>,
        or_planeswalker: bool,
    ) -> Result<String> {
        const ANY: &str = "cantrip_mechanics::combat::choose_attack_target(g, controller, asker.source)?.unwrap_or(Target::None)";
        Ok(match (player, or_planeswalker) {
            (None, _) => ANY.to_string(),
            (Some(player), false) => {
                format!("Target::Player({})", self.fetch(self.certain_player(player)?))
            }
            (Some(player), true) => format!(
                "{{ let player = {}; let candidates: Vec<Target> = g.enter_attacking_targets(controller).into_iter().filter(|t| match *t {{ Target::Player(p) => p == player, Target::Object(o) => g.controller(o) == player && g.chars(o).is(CardType::Planeswalker), _ => false }}).collect(); cantrip_mechanics::combat::choose_among(g, controller, asker.source, &candidates)?.unwrap_or(Target::None) }}",
                self.fetch(self.certain_player(player)?)
            ),
        })
    }
    /// "If a Hero enters this way, it enters with ... counters" (CR 614.12): a
    /// replacement effect about the card, so it is judged as it would exist on the
    /// battlefield, as a look at it where it is now would not. The index of the
    /// made replacement.
    pub(super) fn arrival_counters(
        &mut self,
        filter: &ObjectFilter,
        counters: &[(crate::counter::CounterKind, Expr)],
    ) -> Result<usize> {
        let id = self.fresh();
        let test = self.filter(filter, &Candidate::of("affected"))?;
        let mut adds = String::new();
        for (kind, n) in counters {
            let n = self.number(n)?.amount(i16::MAX.into(), "i16")?;
            write!(adds, "z.counters[{} as usize] += {n}; ", counter_kind(kind)?).unwrap();
        }
        writeln!(self.functions, "#[allow(unused_variables)] fn arrival_{id}_applies(q: &dyn Query, asker: &Asker, affected: ObjectId, proposal: &Proposal) -> bool {{ matches!(proposal, Proposal::ZoneChange(z) if z.object == affected && z.to.is(ZoneKind::Battlefield)) && {test} }}").unwrap();
        writeln!(self.functions, "#[allow(unused_variables)] fn arrival_{id}_apply(q: &dyn Query, asker: &Asker, affected: ObjectId, proposal: &mut Proposal) {{ if let Proposal::ZoneChange(z) = proposal {{ {adds}}} }}").unwrap();
        self.add_made_replacement(format!("granted_replacement(ReplacementKind::ZoneChange, arrival_{id}_applies, arrival_{id}_apply, \"enters with counters\").once()"));
        Ok(self.made_replacements.len() - 1)
    }
    pub(super) fn add_made_replacement(&mut self, mut def: String) {
        let mut feature = self.replacing.planned.take();
        for (needs, call) in std::mem::take(&mut self.replacing.suffix) {
            def.push_str(&call);
            feature.get_or_insert(needs);
        }
        let def = match feature {
            None => def,
            Some(feature) => {
                self.release_callbacks(&feature);
                planned::planned_value(
                    &feature,
                    "granted_replacement(ReplacementKind::Damage, |_, _, _, _| false, |_, _, _, _| (), \"\")",
                    &def,
                )
            }
        };
        self.made_replacements.push(def);
    }
    /// The changes a program makes to its event, whatever stands around them,
    /// and the rest of what it does. "You may" around the whole of it is the
    /// effect's own choice (CR 614.1), and a program that always replaces the
    /// event replaces it whichever way it goes.
    pub(super) fn split_program(
        &mut self,
        program: &Program,
        frame: Frame,
    ) -> (Vec<EventChange>, Vec<Instruction>) {
        self.replacing.canceled = false;
        self.replacing.guarded.clear();
        self.replacing.prevented = None;
        self.replacing.may = None;
        self.replacing.changed.clear();
        self.replacing.edits.clear();
        let mut steps = Vec::new();
        lane_m::flat_groups(&program.instructions, &mut steps);
        if let Some((body, rest)) = chosen_instead(&steps) {
            let id = self.fresh();
            let hint = quoted(&ability_text(&body));
            writeln!(self.functions, "static INSTEAD_{id}: Question = Question::new(\"Instead\", {hint}, \"If you don't, the event goes on as it was.\", \"Do this instead\", \"Don't\");").unwrap();
            self.replacing.may = Some(format!("INSTEAD_{id}"));
            steps.clear();
            lane_m::flat_groups(&body.instructions, &mut steps);
            steps.extend(rest);
        }
        let cancels = always_cancels(&steps);
        let kind = self.replacing.kind;
        let mut changes = Vec::new();
        let mut rest = Vec::new();
        for step in steps {
            match step {
                Instruction::Effect {
                    operation: Operation::ModifyEvent { change },
                    result,
                } => {
                    if lane_n::prevents(std::slice::from_ref(&change)) {
                        self.replacing.prevented = Some(result.0);
                    } else {
                        self.replacing.changed.push(result.0);
                    }
                    changes.push(change);
                }
                Instruction::If {
                    condition,
                    then,
                    otherwise,
                } => {
                    let parts = only_changes(&then).zip(only_changes(&otherwise));
                    let readable = self.matching(frame, |c| c.condition(&condition)).is_ok();
                    match parts {
                        Some((yes, no)) if readable => {
                            self.replacing.guarded.push((condition, yes, no))
                        }
                        _ => rest.push(Instruction::If {
                            condition,
                            then,
                            otherwise,
                        }),
                    }
                }
                // "Exile it instead": the thing that would move goes there.
                Instruction::Effect {
                    operation:
                        Operation::Move {
                            objects: ObjectSet::One(ObjectRef::EventSubject),
                            destination,
                            entry,
                            face_down: false,
                        },
                    result,
                } if kind == "ZoneChange"
                    && !entry.tapped
                    && entry.counters.is_empty()
                    && entry.states.is_empty() =>
                {
                    self.replacing.changed.push(result.0);
                    changes.push(EventChange::SetDestination(destination));
                }
                // "Regenerate it", of what would be destroyed: no shield, the
                // replacement a shield makes (CR 701.19a).
                Instruction::Effect {
                    operation:
                        Operation::CreateRegenerationShields {
                            objects: ObjectSet::One(ObjectRef::EventSubject),
                        },
                    ..
                } if kind == "ZoneChange" => {
                    self.replacing.edits.push_str("z.regenerated = z.destroy;");
                }
                // "Instead that damage is dealt, but all other damage is healed."
                Instruction::Effect {
                    operation:
                        Operation::Keyword {
                            action: KeywordAction::RemoveAllDamage,
                            object: Some(ObjectRef::EventSubject),
                            ..
                        },
                    ..
                } if kind == "Damage" => self.replacing.edits.push_str("e.heal_marked = true;"),
                other => rest.push(other),
            }
        }
        if cancels && !changes.iter().any(|c| matches!(c, EventChange::Cancel)) {
            changes.push(EventChange::Cancel);
        }
        self.replacing.canceled = changes.iter().any(|c| matches!(c, EventChange::Cancel));
        (changes, rest)
    }
    /// The conditional changes of `split_program`, as statements of `apply`.
    pub(super) fn guarded_edits(&mut self, shape: &str, frame: Frame) -> Result<String> {
        let guarded = std::mem::take(&mut self.replacing.guarded);
        let mut edits = String::new();
        for (condition, yes, no) in &guarded {
            let test = self.matching(frame, |c| c.condition(condition))?;
            let mut then = String::new();
            for change in yes {
                then.push_str(&self.reshape(shape, change, frame)?);
            }
            let mut otherwise = String::new();
            for change in no {
                otherwise.push_str(&self.reshape(shape, change, frame)?);
            }
            write!(edits, "if {test} {{ {then} }} else {{ {otherwise} }}").unwrap();
        }
        Ok(edits)
    }
    /// A change to the event from the middle of what its effect does, which has
    /// to wait on an answer or a result `apply` is too early and too pure to have.
    pub(super) fn change_in_then(&mut self, change: &EventChange, out: &mut String) -> Result<()> {
        if self.pending == Pending::Reshaped {
            if matches!(change, EventChange::Cancel) && self.replacing.canceled {
                return Ok(());
            }
            if let Some((variant, binding, frame)) = self.replacing.event {
                let edit = match change {
                    EventChange::PreventDamage => format!("{binding}.prevent(None);"),
                    EventChange::PreventSomeDamage(n) => {
                        let n = self
                            .matching(frame, |c| c.number(n))?
                            .amount(u16::MAX.into(), "u16")?;
                        format!("{binding}.prevent(Some({n}));")
                    }
                    other => self.reshape(lane_n::shape_of(variant, variant), other, frame)?,
                };
                self.plan_code(
                    "event change in the program of another event",
                    &format!(
                        "g.change_event(|q, proposal| {{ if let Proposal::{variant}({binding}) = proposal {{ {edit} }} }});"
                    ),
                    out,
                );
                return Ok(());
            }
        }
        Err(gap("event change in the program of another event"))
    }
    /// The follow-up of a prevention: `PreventionDef::then`, told how much it
    /// prevented, which is what "if damage is prevented this way" reads.
    pub(super) fn prevention_then(
        &mut self,
        label: &str,
        arms: &[Watched],
        frame: Frame,
        rest: &[Instruction],
        about: &str,
    ) -> Result<Option<String>> {
        if rest.is_empty() {
            return Ok(None);
        }
        let then = format!("{label}_then");
        let locals = arms[0].locals.clone();
        self.replacing.event = Some(("Damage", "e", frame));
        let prevented = self.replacing.prevented;
        let out = self.then_steps(frame, rest, prevented.map(|r| (r, "done_prevented")))?;
        let pass = if about.is_empty() { "" } else { "affected, " };
        writeln!(self.functions, "#[allow(unused_variables, unused_mut)] fn {then}_steps(g: &mut dyn Game, asker: &Asker, {about}subject: Target, data: [u32; 2], done_prevented: u16) -> Result<(), Suspend> {{ {out} Ok(()) }}").unwrap();
        writeln!(self.functions, "#[allow(unused_variables, dead_code)] fn {then}(g: &mut dyn Game, asker: &Asker, {about}e: &cantrip_api::DamageEvent, prevented: u16) {{ let q: &dyn Query = g; {locals} let _ = {then}_steps(g, asker, {pass}subject, data, prevented); }}").unwrap();
        Ok(Some(then))
    }
    /// How much of the damage a made prevention prevents, where it is not all.
    pub(super) fn made_prevention_amount(
        &mut self,
        label: &str,
        arms: &[Watched],
        frame: Frame,
        changes: &[EventChange],
    ) -> Result<Option<String>> {
        let code = match changes {
            [EventChange::PreventSomeDamage(n)] => self
                .matching(frame, |c| c.number(n))?
                .amount(u16::MAX.into(), "u16")?,
            [EventChange::PreventAllButDamage(n)] => {
                let kept = self
                    .matching(frame, |c| c.number(n))?
                    .amount(u16::MAX.into(), "u16")?;
                format!("amount.saturating_sub({kept})")
            }
            _ => return Ok(None),
        };
        let function = format!("{label}_amount");
        writeln!(self.functions, "#[allow(unused_variables, dead_code)] fn {function}(q: &dyn Query, asker: &Asker, affected: ObjectId, e: &cantrip_api::DamageEvent) -> u16 {{ {} {code} }}", arms[0].locals).unwrap();
        Ok(Some(function))
    }
    /// The size of a shield a made effect is used up by, as code of the resolution.
    pub(super) fn made_shield(
        &self,
        limit: &ReplacementLimit,
        body: &Program,
    ) -> Result<Option<String>> {
        let amount = match limit {
            ReplacementLimit::TotalDamage(n) | ReplacementLimit::DamageToEach(n) => Some(n.clone()),
            _ => {
                let mut steps = Vec::new();
                lane_m::flat_groups(&body.instructions, &mut steps);
                steps.iter().find_map(|step| match step {
                    Instruction::Effect {
                        operation:
                            Operation::ModifyEvent {
                                change: EventChange::RedirectSomeDamage(n, _),
                            },
                        ..
                    } => Some(n.clone()),
                    _ => None,
                })
            }
        };
        amount
            .map(|n| self.number(&n)?.amount(u16::MAX.into(), "u16"))
            .transpose()
    }
    /// What compiling the program of `arms` needs to know of its event.
    pub(super) fn note_event(&mut self, event: &ReplacementEvent, arms: &[Watched]) {
        self.replacing.kind = arms[0].kind;
        self.replacing.each_token = matches!(event, ReplacementEvent::Tokens { each: true, .. });
    }
    /// The tokens of a replacement. Those it adds are part of the same event,
    /// so nothing that replaced it meets them again (CR 614.5, 111.1).
    fn token_change(&mut self, copies: u32, tokens: &[TokenSpec], frame: Frame) -> Result<String> {
        const FEATURE: &str = "replacement of token creation with other tokens";
        let each = self.replacing.each_token;
        if let Some(edits) = self.tokens_become_copies(copies, tokens, frame)? {
            return Ok(edits);
        }
        let mut edits = format!("let each = t.count; t.count = t.count.saturating_mul({copies}); ");
        for token in tokens {
            let TokenRecipe::Defined(definition) = &token.recipe else {
                return Err(gap(format!("token recipe {}", form(&token.recipe))));
            };
            let amount = self
                .matching(frame, |c| c.number(&token.amount))?
                .amount(u32::MAX.into(), "u32")?;
            let amount = if each {
                format!("({amount}).saturating_mul(each)")
            } else {
                amount
            };
            let definition = self.token_definition(definition)?;
            let words = match token.words {
                TokenWords::Rules => "Words::NONE",
                TokenWords::CreatingEffect => "q.words(asker.source)",
            };
            write!(
                edits,
                "{};",
                planned::planned_value(
                    FEATURE,
                    "()",
                    &format!("t.also(q.card_id(&{definition}), {amount}, {words})"),
                )
            )
            .unwrap();
        }
        Ok(edits)
    }
    /// The changes the events of this lane take.
    pub(super) fn reshape_rest(
        &mut self,
        kind: &str,
        change: &EventChange,
        frame: Frame,
    ) -> Result<String> {
        use EventChange as C;
        let owner = "q.state().obj(z.object).owner";
        Ok(match (kind, change) {
            ("ZoneChange", C::Cancel) => "z.replaced = true;".into(),
            ("ZoneChange", C::SetDestination(Destination::Exile)) if self.links_exile => {
                format!("z.to = Zone::exile({owner}); z.exiled_with = Some(q.linked_to(asker));")
            }
            (
                "ZoneChange",
                C::SetDestination(
                    end @ (Destination::OwnersLibraryTop | Destination::OwnersLibraryBottom),
                ),
            ) => {
                let end = match end {
                    Destination::OwnersLibraryTop => "Top",
                    _ => "Bottom",
                };
                format!(
                    "z.to = Zone::library({owner}); {};",
                    planned::planned_value(
                        &format!("replacement destination OwnersLibrary{end}"),
                        "()",
                        &format!("z.library_end = Some(LibraryEnd::{end})"),
                    )
                )
            }
            ("ZoneChange", C::SetDestination(Destination::OwnersBattlefield)) => {
                format!("z.to = Zone::BATTLEFIELD; z.controller = {owner};")
            }
            ("ZoneChange", C::SetDestination(Destination::Battlefield(player))) => {
                let player = self.matching(frame, |c| c.certain_player(player))?;
                format!("z.to = Zone::BATTLEFIELD; z.controller = {player};")
            }
            ("ZoneChange", C::SetController(player)) => {
                let player = self.matching(frame, |c| c.certain_player(player))?;
                format!("z.controller = {player};")
            }
            ("Damage", C::RedirectSomeDamage(_, recipients)) => {
                return self.reshape(kind, &C::RedirectDamage(recipients.clone()), frame)
            }
            ("TokenCreation", C::ReplaceTokens { copies, tokens }) => {
                self.token_change(*copies, tokens, frame)?
            }
            ("PlayerCounters", C::SetAmount(n)) => {
                let n = self
                    .matching(frame, |c| c.number(n))?
                    .amount(u16::MAX.into(), "u16")?;
                match self.replacing.player_counter {
                    Some(field) => format!("c.{field} = {n};"),
                    // "Of each of those kinds": the amount of each, its own.
                    None => format!("for kind in [&mut c.poison, &mut c.energy, &mut c.experience] {{ if *kind > 0 {{ let data = [u32::from(*kind), u32::MAX]; *kind = {n}; }} }}"),
                }
            }
            ("PlayerCounters", C::Cancel) => match self.replacing.player_counter {
                Some(field) => format!("c.{field} = 0;"),
                None => "c.poison = 0; c.energy = 0; c.experience = 0;".into(),
            },
            ("PlayerCounters", C::AddCounters { kind, amount }) => {
                let field = player_counter(kind)
                    .ok_or_else(|| gap("replacement of counters of more than one kind"))?;
                let n = self
                    .matching(frame, |c| c.number(amount))?
                    .amount(u16::MAX.into(), "u16")?;
                format!("c.{field} = c.{field}.saturating_add({n});")
            }
            ("ZoneChange", C::Entry(state)) => self.entry_change(state, frame)?,
            ("ManaProduction", C::SetAmount(n)) => {
                let n = self
                    .matching(frame, |c| c.number(n))?
                    .amount(u32::MAX.into(), "u32")?;
                // "Twice as much of that mana": of each type its own amount.
                format!("for kind in m.mana.0.iter_mut() {{ if *kind > 0 {{ let data = [*kind, u32::MAX]; *kind = {n}; }} }}")
            }
            ("ManaProduction", C::ReplaceColoredMana(ManaProduction::Amount { color, .. })) => {
                let colored =
                    "let colored: u32 = m.mana.0[..5].iter().sum(); m.mana.0[..5].fill(0);";
                format!("{colored} {}", mana_gained(color, "colored"))
            }
            ("ManaProduction", C::ReplaceMana(ManaProduction::Amount { color, .. })) => {
                let all = "let all = m.total(); m.mana.0.fill(0);";
                format!("{all} {}", mana_gained(color, "all"))
            }
            ("ManaProduction", C::ReplaceMana(production) | C::AddMana(production)) => {
                let choices = mana_choices(production)?;
                let feature = format!("event change {}", form(change));
                format!(
                    "{};",
                    planned::planned_value(
                        &feature,
                        "()",
                        &format!("m.instead_of_choice(&[{}])", choices.join(", ")),
                    )
                )
            }
            // `ReplacementDef::then` runs as the effect applies: this runs after it.
            ("ZoneChange", C::Then(program)) => {
                let id = self.fresh();
                let steps = self.then_steps(frame, &program.instructions, None)?;
                writeln!(self.functions, "#[allow(unused_variables, unused_mut, dead_code)] fn after_{id}(g: &mut dyn Game, asker: &Asker, subject: Target, data: [u32; 2]) -> Result<(), Suspend> {{ {steps} Ok(()) }}").unwrap();
                format!(
                    "{};",
                    planned::planned_value(
                        "event change then",
                        "()",
                        &format!("z.after(after_{id}, subject, data)"),
                    )
                )
            }
            ("TokenCreation", C::SetController(player)) => {
                let player = self.matching(frame, |c| c.certain_player(player))?;
                format!("t.controller = {player};")
            }
            ("Damage", C::DamageAsInfect) => format!(
                "{};",
                planned::planned_value("event change damage as infect", "()", "e.as_infect = true")
            ),
            (_, change) if self.replacing.planned.is_some() => {
                self.planned_change(change, frame)?
            }
            (_, other) => return Err(gap(format!("event change {}", form(other)))),
        })
    }
    /// How another permanent enters: as a copy, as more than it is, or with the
    /// counters something else has (CR 614.1c, 707.5).
    fn entry_change(&mut self, state: &EntryState, frame: Frame) -> Result<String> {
        const FEATURE: &str = "event change entry";
        let planned = |code: String| format!("{};", planned::planned_value(FEATURE, "()", &code));
        Ok(match state {
            EntryState::CopyOf {
                objects,
                until: None,
                effects,
            } if format!("{effects:?}").contains("NoCopyEffects") => {
                let original = self.matching(frame, |c| c.objects(objects))?;
                planned(format!(
                    "z.enters_as_copy_of = ({original}).first().copied()"
                ))
            }
            EntryState::Changes(changes) => {
                let mut change = "EntryChange::new()".to_string();
                for each in changes {
                    match each {
                        Change::SetBasePowerToughness {
                            power: Expr::Constant(power),
                            toughness: Expr::Constant(toughness),
                        } => write!(change, ".sized({power}, {toughness})").unwrap(),
                        Change::AddCardType(kind) => {
                            write!(change, ".with_type(CardType::{})", name(kind)).unwrap()
                        }
                        Change::CreatureTypes {
                            mode: ChangeMode::Add,
                            types: CreatureTypeSet::Listed(types),
                        } => {
                            for kind in types {
                                write!(change, ".with_subtype(Subtype::{})", name(kind)).unwrap();
                            }
                        }
                        other => return Err(gap(format!("event change entry {}", form(other)))),
                    }
                }
                planned(format!("z.enters_changed = Some({change})"))
            }
            EntryState::CountersOf {
                object,
                one_of_each: true,
            } => {
                let original = self.matching(frame, |c| c.object(object))?;
                format!("if let Some(original) = {original} {{ for kind in CounterKind::ALL {{ if q.state().obj(original).counters(kind) > 0 {{ z.counters[kind as usize] = z.counters[kind as usize].saturating_add(1); }} }} }}")
            }
            other => return Err(gap(format!("event change entry {}", form(other)))),
        })
    }
    /// A change to a proposal Cantrip does not have, said of the proposal it
    /// will have: `p`.
    fn planned_change(&mut self, change: &EventChange, frame: Frame) -> Result<String> {
        use EventChange as C;
        let number = |c: &mut Self, n: &Expr| {
            c.matching(frame, |c| c.number(n))?
                .amount(u32::MAX.into(), "u32")
        };
        Ok(match change {
            C::Cancel => "p.replaced = true;".into(),
            C::SetAmount(n) => format!("p.count = {};", number(self, n)?),
            C::Repeats(n) => format!("p.times = {};", number(self, n)?),
            C::SearchTop(n) => format!("p.top = Some({});", number(self, n)?),
            C::FlipExtraCoin => "p.extra_flips += 1;".into(),
            // It still happens, reading no such counters.
            C::IgnoreCounters(kind) => format!(
                "p.replaced = false; p.ignore_counters = Some({});",
                counter_kind(kind)?
            ),
            C::AddCopy { new_targets } => {
                format!("p.count += 1; p.new_targets_for_added = {new_targets};")
            }
            C::ExchangeDieResult {
                decider,
                object,
                properties,
            } => {
                let decider = self.matching(frame, |c| c.certain_player(decider))?;
                let object = self.matching(frame, |c| c.object(object))?;
                let properties = properties
                    .iter()
                    .map(|property| match property {
                        Property::BasePower => Ok("DieExchange::BasePower"),
                        Property::BaseToughness => Ok("DieExchange::BaseToughness"),
                        other => Err(gap(format!("a die exchanged with {}", form(other)))),
                    })
                    .collect::<Result<Vec<_>>>()?
                    .join(", ");
                format!("p.exchange = Some(DieExchangeOffer {{ decider: {decider}, object: {object}, properties: &[{properties}] }});")
            }
            C::RollExtraDie(DiceIgnore::Lowest) => {
                "p.extra_dice += 1; p.ignore = Some(DiceIgnore::Lowest);".into()
            }
            C::RollExtraDie(DiceIgnore::ChosenBy(player)) => format!(
                "p.extra_dice += 1; p.ignore = Some(DiceIgnore::ChosenBy({}));",
                self.matching(frame, |c| c.certain_player(player))?
            ),
            other => return Err(gap(format!("event change {}", form(other)))),
        })
    }
    /// The events no other part of the generator reads.
    pub(super) fn watched_rest(
        &mut self,
        event: &ReplacementEvent,
    ) -> Result<Option<(Vec<Watched>, Frame)>> {
        const ZONE: &str =
            "let id = z.object; let subject = Target::Object(id); let data = [u32::MAX; 2];";
        let object_frame = Frame {
            object: Some(Carried::Subject),
            ..Frame::default()
        };
        let player_frame = Frame {
            player: Some(Carried::Subject),
            ..Frame::default()
        };
        let amount_frame = Frame {
            amount: Some(Carried::Data(0)),
            ..player_frame
        };
        let zone = |tests: Vec<String>| Watched {
            kind: "ZoneChange",
            variant: "ZoneChange",
            binding: "z",
            locals: ZONE.into(),
            tests,
        };
        let one = |watched: Watched, frame: Frame| Ok(Some((vec![watched], frame)));
        match event {
            ReplacementEvent::Object {
                event: ReplacedObjectEvent::LeavesBattlefield,
                objects,
            } => {
                let test =
                    self.matching(object_frame, |c| c.filter(objects, &Candidate::of("id")))?;
                one(
                    zone(vec![
                        "z.from.is(ZoneKind::Battlefield)".into(),
                        "!z.to.is(ZoneKind::Battlefield)".into(),
                        test,
                    ]),
                    object_frame,
                )
            }
            ReplacementEvent::Object {
                event: ReplacedObjectEvent::Destroyed,
                objects,
            } => {
                let test =
                    self.matching(object_frame, |c| c.filter(objects, &Candidate::of("id")))?;
                one(zone(vec!["z.destruction()".into(), test]), object_frame)
            }
            ReplacementEvent::Object {
                event: what,
                objects,
            } => {
                let (kind, feature) = (object_proposal(*what), format!("replacement of {what:?}"));
                let test =
                    self.matching(object_frame, |c| c.filter(objects, &Candidate::of("id")))?;
                self.replacing.planned = Some(feature);
                one(
                    Watched {
                        kind,
                        variant: kind,
                        binding: "p",
                        locals: "let id = p.object; let subject = Target::Object(id); let data = [u32::MAX; 2];".into(),
                        tests: vec![test],
                    },
                    object_frame,
                )
            }
            ReplacementEvent::Player {
                event: ReplacedPlayerEvent::GetEnergy,
                players,
            } => self.player_counters(Some("energy"), players),
            ReplacementEvent::Counters {
                kind,
                players: Some(players),
                by: _,
                ..
            } => {
                let field = match kind {
                    Some(kind) => Some(
                        player_counter(kind)
                            .ok_or_else(|| gap("replacement of counters a player cannot have"))?,
                    ),
                    None => None,
                };
                self.player_counters(field, players)
            }
            ReplacementEvent::Player {
                event: ReplacedPlayerEvent::DrawInDrawStep,
                players,
            } => {
                let test = self.matching(player_frame, |c| c.player_in(players, "d.player"))?;
                one(
                    Watched {
                        kind: "Draw",
                        variant: "Draw",
                        binding: "d",
                        locals:
                            "let subject = Target::Player(d.player); let data = [1u32, u32::MAX];"
                                .into(),
                        tests: vec![
                            "q.state().turn.step() == Step::Draw".into(),
                            "q.state().turn.active == d.player".into(),
                            test,
                        ],
                    },
                    Frame {
                        amount: Some(Carried::Data(0)),
                        ..player_frame
                    },
                )
            }
            ReplacementEvent::Player {
                event: ReplacedPlayerEvent::ReduceLifeTotal,
                players,
            } => {
                let test = self.matching(player_frame, |c| c.player_in(players, "l.player"))?;
                one(
                    Watched {
                        kind: "LifeChange",
                        variant: "LifeChange",
                        binding: "l",
                        locals: "let amount = q.state().player(l.player).life.saturating_add(l.amount); let subject = Target::Player(l.player); let data = [amount.max(0) as u32, u32::MAX];".into(),
                        tests: vec!["l.amount < 0".into(), test],
                    },
                    amount_frame,
                )
            }
            ReplacementEvent::Player {
                event: what,
                players,
            } => {
                let (kind, measured) = player_proposal(*what);
                let feature = format!("replacement of {what:?}");
                let test = self.matching(player_frame, |c| c.player_in(players, "p.player"))?;
                self.replacing.planned = Some(feature);
                let locals = if measured {
                    "let amount = p.count; let subject = Target::Player(p.player); let data = [p.count, u32::MAX];"
                } else {
                    "let subject = Target::Player(p.player); let data = [u32::MAX; 2];"
                };
                one(
                    Watched {
                        kind,
                        variant: kind,
                        binding: "p",
                        locals: locals.into(),
                        tests: vec![test],
                    },
                    if measured { amount_frame } else { player_frame },
                )
            }
            ReplacementEvent::Discard {
                players,
                causes,
                card,
                effect_only,
            } => {
                let frame = Frame {
                    object: Some(Carried::Subject),
                    player: Some(Carried::Data(0)),
                    ..Frame::default()
                };
                let feature = format!("replacement event {}", form(event));
                let mut tests = vec![
                    "z.reason == ZoneReason::Discard".to_string(),
                    self.matching(frame, |c| c.player_in(players, "owner"))?,
                ];
                if let Some(card) = card {
                    tests.push(self.matching(frame, |c| c.filter(card, &Candidate::of("id")))?);
                }
                if let Some(cause) = causes {
                    let test =
                        self.matching(frame, |c| c.filter(cause, &Candidate::of("cause")))?;
                    tests.push(planned::planned_value(
                        &feature,
                        "true",
                        &format!("q.proposal_cause().is_some_and(|cause| {test})"),
                    ));
                }
                if *effect_only {
                    tests.push(planned::planned_value(
                        &feature,
                        "true",
                        "q.proposal_cause().is_some()",
                    ));
                }
                one(
                    Watched {
                        kind: "ZoneChange",
                        variant: "ZoneChange",
                        binding: "z",
                        locals: "let id = z.object; let owner = q.state().obj(id).owner; let subject = Target::Object(id); let data = [u32::from(owner.0), u32::MAX];".into(),
                        tests,
                    },
                    frame,
                )
            }
            ReplacementEvent::Countered { spells, by } => {
                let feature = format!("replacement event {}", form(event));
                let spell =
                    self.matching(object_frame, |c| c.filter(spells, &Candidate::of("id")))?;
                let cause =
                    self.matching(object_frame, |c| c.filter(by, &Candidate::of("cause")))?;
                one(
                    zone(vec![
                        "z.reason == ZoneReason::Counter".into(),
                        spell,
                        planned::planned_value(
                            &feature,
                            "true",
                            &format!("q.proposal_cause().is_some_and(|cause| {cause})"),
                        ),
                    ]),
                    object_frame,
                )
            }
            ReplacementEvent::PutOntoBattlefield { players, objects } => {
                let frame = Frame {
                    object: Some(Carried::Subject),
                    player: Some(Carried::Data(0)),
                    ..Frame::default()
                };
                let entering = Candidate {
                    controller: "z.controller".into(),
                    ..Candidate::entering()
                };
                let mut tests = vec!["z.to.is(ZoneKind::Battlefield)".to_string()];
                tests.push(self.matching(frame, |c| c.filter(objects, &entering))?);
                tests.push(self.matching(frame, |c| c.player_in(players, "z.controller"))?);
                one(
                    Watched {
                        kind: "ZoneChange",
                        variant: "ZoneChange",
                        binding: "z",
                        locals: "let id = z.object; let subject = Target::Object(id); let data = [u32::from(z.controller.0), u32::MAX];".into(),
                        tests,
                    },
                    frame,
                )
            }
            ReplacementEvent::DrawCausedBy { players, abilities } => {
                let feature = format!("replacement event {}", form(event));
                let test = self.matching(player_frame, |c| c.player_in(players, "d.player"))?;
                let cause = self.matching(player_frame, |c| {
                    c.filter(abilities, &Candidate::of("cause"))
                })?;
                one(
                    Watched {
                        kind: "DrawInstruction",
                        variant: "DrawInstruction",
                        binding: "d",
                        locals: "let subject = Target::Player(d.player); let data = [d.count, u32::MAX];".into(),
                        tests: vec![
                            test,
                            planned::planned_value(
                                &feature,
                                "true",
                                &format!("q.proposal_cause().is_some_and(|cause| {cause})"),
                            ),
                        ],
                    },
                    amount_frame,
                )
            }
            ReplacementEvent::CopySpell { players, spells } => {
                let feature = format!("replacement event {}", form(event));
                let frame = Frame {
                    object: Some(Carried::Subject),
                    player: Some(Carried::Data(0)),
                    ..Frame::default()
                };
                let mut tests = vec![self.matching(frame, |c| c.player_in(players, "p.player"))?];
                tests.push(self.matching(frame, |c| c.filter(spells, &Candidate::of("id")))?);
                self.replacing.planned = Some(feature);
                one(
                    Watched {
                        kind: "SpellCopy",
                        variant: "SpellCopy",
                        binding: "p",
                        locals: "let id = p.spell; let subject = Target::Object(id); let data = [u32::from(p.player.0), u32::MAX];".into(),
                        tests,
                    },
                    frame,
                )
            }
            ReplacementEvent::FirstEachTurn(inner) => {
                let feature = format!("replacement event {}", form(event));
                let found = self.watched(inner)?;
                self.replacing
                    .suffix
                    .push((feature, ".first_each_turn()".into()));
                Ok(Some(found))
            }
            ReplacementEvent::EachPoint(inner) => {
                let feature = format!("replacement event {}", form(event));
                let found = self.watched(inner)?;
                self.replacing
                    .suffix
                    .push((feature, ".each_point()".into()));
                Ok(Some(found))
            }
            ReplacementEvent::Caused {
                cause,
                event: inner,
            } => {
                let feature = format!("replacement event {}", form(event));
                let (mut arms, frame) = self.watched(inner)?;
                let cause = self.matching(frame, |c| c.filter(cause, &Candidate::of("cause")))?;
                for arm in &mut arms {
                    arm.tests.push(planned::planned_value(
                        &feature,
                        "true",
                        &format!("q.proposal_cause().is_some_and(|cause| {cause})"),
                    ));
                }
                Ok(Some((arms, frame)))
            }
            _ => Ok(None),
        }
    }
    fn player_counters(
        &mut self,
        field: Option<&'static str>,
        players: &PlayerSet,
    ) -> Result<Option<(Vec<Watched>, Frame)>> {
        let frame = Frame {
            player: Some(Carried::Subject),
            amount: Some(Carried::Data(0)),
            ..Frame::default()
        };
        let test = self.matching(frame, |c| c.player_in(players, "c.player"))?;
        self.replacing.player_counter = field;
        let (some, amount) = match field {
            Some(field) => (format!("c.{field} > 0"), format!("u32::from(c.{field})")),
            None => (
                "c.poison > 0 || c.energy > 0 || c.experience > 0".to_string(),
                "u32::from(c.poison) + u32::from(c.energy) + u32::from(c.experience)".to_string(),
            ),
        };
        Ok(Some((
            vec![Watched {
                kind: "Counters",
                variant: "PlayerCounters",
                binding: "c",
                locals: format!(
                    "let subject = Target::Player(c.player); let data = [{amount}, u32::MAX];"
                ),
                tests: vec![some, test],
            }],
            frame,
        )))
    }
    /// Who makes a delayed ability: what is resolving, or what a callback is
    /// told, as code for its source, its controller and the card it is on.
    pub(super) fn maker(&self) -> (&'static str, &'static str, String) {
        if self.stack_item {
            let card = match self.spliced() {
                true => "g.card_id(&CARD)",
                false => "item.ability.card",
            };
            ("item.object", "item.controller", card.into())
        } else {
            (
                "asker.source",
                "asker.controller",
                self.fetch(self.source_card()),
            )
        }
    }
    /// `Operation::CreateDelayedTrigger` on an event: once, the next time it
    /// happens. Its body has none of that event: what it names is the maker's.
    pub(super) fn delayed_on_event(
        &mut self,
        event: &EventSpec,
        start: &DelayedStart,
        expires: &Option<crate::ir::Duration>,
        intervening_if: &Option<Condition>,
        body: &Program,
        out: &mut String,
    ) -> Result<()> {
        use crate::ir::Duration as D;
        if !matches!(start, DelayedStart::Immediately) {
            return Err(gap(format!("delayed trigger starting {}", form(start))));
        }
        let when = match expires {
            None => "DelayedWhen::UntilEvent".to_string(),
            Some(D::EndOfTurn) => "DelayedWhen::NextThisTurn".to_string(),
            Some(other @ D::UntilStateBasedActionsChecked) => planned::planned_value(
                &format!("delayed trigger until {}", form(other)),
                "DelayedWhen::NextThisTurn",
                "DelayedWhen::UntilStateBasedActionsChecked",
            ),
            Some(other) => return Err(gap(format!("delayed trigger until {}", form(other)))),
        };
        let when = when.as_str();
        let (watched, source) = self.watching_source(event, body)?;
        let saved = self.clone();
        let mark = out.len();
        // Only what waits for one event keeps a subject (settle's `is_event`).
        for subject_only in [when == "DelayedWhen::UntilEvent", false] {
            let label = format!("delayed_{}", self.fresh());
            let closed = self.closing_for_matcher(subject_only, |c| {
                c.reset_results();
                let matched = c.event(&watched)?;
                let targets = c.targets_for(
                    &Announcement {
                        targets: Vec::new(),
                        modes: None,
                    },
                    &label,
                    Frame::default(),
                )?;
                let trigger = c.trigger(&matched, &format!("{label}_event"), &[]);
                c.reads_stack_object_of(&watched, matched.frame);
                match intervening_if {
                    Some(condition) => c.guarded_effect(condition, body, &label),
                    None => c.effect(body, &label),
                }?;
                let text = format!("{}, {}", matched.phrase, ability_text(body));
                Ok(format!(
                    "triggered({trigger}, {label}, {}).targets({targets})",
                    quoted(&text)
                ))
            });
            let made = closed.and_then(|(outer, def)| {
                self.delayed.push(def);
                let index = self.delayed.len() - 1;
                self.make_watching(&outer, when, &source, index, out)
            });
            match made {
                Ok(()) => return Ok(()),
                Err(failed) if !subject_only => return Err(failed),
                Err(_) => {
                    *self = saved.clone();
                    out.truncate(mark);
                }
            }
        }
        Err(gap("delayed trigger on an event"))
    }
    /// A made effect that reads the ability making it, closed over those
    /// values: `one` keeps them to the object the effect is about, which Cantrip
    /// can do, and otherwise it keeps a list.
    pub(super) fn carried_replacement(
        &mut self,
        event: &ReplacementEvent,
        limit: &ReplacementLimit,
        duration: &crate::ir::Duration,
        body: &Program,
        one: bool,
        out: &mut String,
    ) -> Result<()> {
        const KEPT: &str = "target reference outside a resolving ability";
        let duration = self.duration(duration)?;
        let shield = self.made_shield(limit, body)?;
        let mut plain = event.clone();
        // A prevention keeps whom it protects. Anything else leaves its recipient
        // in the event, and a target that is an object is the effect's object.
        let to = if prevents_at_all(body) {
            self.named_recipient_of(&mut plain)?
        } else {
            None
        };
        if let ReplacementEvent::Damage(damage) = &mut plain {
            if let DamageRecipients::Target { slot, index } = &damage.recipients {
                if self
                    .slots
                    .get(&slot.0)
                    .is_some_and(|(_, spec)| matches!(spec.kind, TargetKind::Object(_)))
                {
                    damage.recipients =
                        DamageRecipients::Objects(ObjectFilter::SameObject(ObjectRef::Target {
                            slot: slot.clone(),
                            index: *index,
                        }));
                }
            }
        }
        let mut about = Vec::new();
        if one {
            replacements::capture(&mut plain, &mut about);
            if about.len() > 1 {
                return Err(gap("replacement made about several objects"));
            }
        }
        let about = about.first().map(|o| self.object(o)).transpose()?;
        let hint = quoted(&ability_text(body));
        let label = format!("made_{}", self.fresh());
        let scope = self.affected_in_scope;
        let (known_to, known_about) = (to.is_some(), about.is_some());
        let made = self.closing_for(Some(KEPT.to_string()), one, |c| {
            c.reset_results();
            c.affected_in_scope = true;
            c.made_effect(
                &label,
                &plain,
                limit,
                shield.clone(),
                body,
                hint,
                known_to,
                known_about,
            )
        });
        self.affected_in_scope = scope;
        let (outer, (shield, kind)) = made?;
        let to = to.map_or_else(
            || "Some(Target::None)".to_string(),
            |to| self.fetch(format!("({to}).first().copied()")),
        );
        let subject = outer
            .subject
            .as_ref()
            .and_then(|subject| subject.borrow().clone());
        let mut effect = kind;
        if subject.is_some() {
            effect = effect.replace(
                "let _ = (to, about); g.add_player_effect(item.controller, item.object, EffectKind::ReplacementRule {",
                "let _ = to; g.add_effect(item.object, about, EffectKind::Replacement {",
            );
        }
        let about = match about.or(subject) {
            Some(object) => self.fetch(object),
            None => "to.and_then(|to: Target| to.object()).or(Some(item.object))".into(),
        };
        let shield = shield.map_or(Ok(String::new()), |n| {
            Ok::<_, Diagnostic>(format!("let shield = {};", self.fetch(n)))
        })?;
        let effect = effect.replace("DURATION", &duration);
        let parts = outer.parts.borrow();
        let bound = format!("{shield} let to = {to}; let about = {about};");
        if !parts.is_empty() {
            let mut carried = String::from("let mut carried: Vec<Target> = Vec::new(); ");
            for part in parts.iter() {
                write!(
                    carried,
                    "carried.extend({}); carried.push(Target::None); ",
                    self.fetch(part)
                )
                .unwrap();
            }
            let effect = effect
                .replace("g.add_effect(", "g.add_effect_about(&carried, ")
                .replace(
                    "g.add_player_effect(",
                    "g.add_player_effect_about(&carried, ",
                );
            self.plan_code(
                KEPT,
                &format!("{{ {bound} {carried} if let (Some(to), Some(about)) = (to, about) {{ {effect} }} }}"),
                out,
            );
        } else if effect.contains("EffectKind::Redirect") {
            self.plan_code(
                "replacement of damage to a named recipient",
                &format!(
                    "{{ {bound} if let (Some(to), Some(about)) = (to, about) {{ {effect} }} }}"
                ),
                out,
            );
        } else {
            writeln!(
                out,
                "{{ {bound} if let (Some(to), Some(about)) = (to, about) {{ {effect} }} }}"
            )
            .unwrap();
        }
        Ok(())
    }
    /// A callback can't read a list the ability made. What is about each thing
    /// a damage instruction was dealt to is made once for each, about that object.
    /// `moving`: it is about that object dying, which only a creature or a
    /// planeswalker does (CR 700.4), whatever else was dealt damage.
    pub(super) fn about_each_damaged(
        &mut self,
        objects: &mut ObjectFilter,
        moving: bool,
        out: &mut String,
    ) -> Option<u32> {
        let binding = 1000 + self.fresh() as u32;
        let each = ObjectRef::Bound(Binding(binding));
        let results: Vec<u32> = self.moved.keys().copied().collect();
        let result = results
            .into_iter()
            .find(|r| damaged_to_bound(objects, *r, &each))?;
        if moving && !format!("{objects:?}").contains("CardType(Creature)") {
            let dying = [crate::mtg_types::CardType::Creature, crate::mtg_types::CardType::Planeswalker];
            let dying = ObjectFilter::Either(dying.map(ObjectFilter::CardType).to_vec());
            *objects = ObjectFilter::All(vec![dying, objects.clone()]);
        }
        writeln!(out, "for bound_{binding} in {}.clone() {{", self.moved[&result]).unwrap();
        self.bound_objects.insert(binding);
        Some(binding)
    }
    pub(super) fn end_each_damaged(&mut self, each: Option<u32>, out: &mut String) {
        if let Some(binding) = each {
            self.bound_objects.remove(&binding);
            writeln!(out, "}}").unwrap();
        }
    }
    /// "Choose a player or a planeswalker that player controls": no target, and
    /// no one kind of thing to ask about.
    pub(super) fn choose_recipient(
        &mut self,
        id: usize,
        player: &str,
        source: &str,
        options: &[Operand],
        local: &str,
        out: &mut String,
    ) -> Result<()> {
        let [Operand::Recipients(candidates)] = options else {
            return Err(gap("a choice of Recipient"));
        };
        let candidates = self.fetch(self.recipients(candidates)?);
        let chosen = planned::planned_value(
            "a choice of Recipient",
            "Target::None",
            &format!(
                "cantrip_mechanics::choose::recipient(g, player_{id}, {source}, candidates_{id})?"
            ),
        );
        writeln!(
            out,
            "let player_{id} = {player}; let candidates_{id}: Vec<Target> = {candidates}; let {local}: Target = {chosen};"
        )
        .unwrap();
        Ok(())
    }
    /// The spells a cast of cards made, "those spells".
    pub(super) fn report_played(&mut self, result: &ResultId) {
        if let Some(played) = self.played.take().filter(|_| self.read_later(result)) {
            self.selections.insert(result.0, played);
        }
    }
    /// What one action did is what paid for it, "the creature attached this way".
    pub(super) fn action_paid(&mut self, body: &Program, result: &ResultId) {
        let mut steps = Vec::new();
        lane_m::flat_groups(&body.instructions, &mut steps);
        let [Instruction::Effect { result: inner, .. }] = steps.as_slice() else {
            return;
        };
        let list = self
            .selections
            .get(&inner.0)
            .or_else(|| self.moved.get(&inner.0))
            .cloned();
        if let Some(list) = list {
            self.before
                .entry(result.0)
                .or_insert(format!("{list}.clone()"));
            self.moved.entry(result.0).or_insert(list);
        }
    }
    /// What an instruction that says nothing did it to, for "those" later.
    pub(super) fn report_objects(
        &mut self,
        op: &Operation,
        result: &ResultId,
        out: &mut String,
    ) -> bool {
        if !read_as_objects(&self.after, result) {
            return false;
        }
        let id = self.fresh();
        let list = match op {
            Operation::Tap { objects, .. } => match self.acted_on(objects) {
                Ok(list) => list,
                Err(_) => return false,
            },
            Operation::Keyword {
                action: KeywordAction::Proliferate,
                ..
            } => planned::planned_value(
                "what a proliferate chose",
                "Vec::<ObjectId>::new()",
                "g.last_proliferated()",
            ),
            _ => return false,
        };
        writeln!(out, "let acted_{id}: Vec<ObjectId> = {list};").unwrap();
        self.selections.insert(result.0, format!("acted_{id}"));
        true
    }
    /// Whether the ability making this body knows what `result` did.
    pub(super) fn bound_outside(&self, result: &ResultId) -> bool {
        self.outer.as_ref().is_some_and(|outer| outer.knows(result))
    }
    /// Whether the instruction of `result`, here or in what makes this body, is
    /// one with a hole of its own.
    pub(super) fn failed_anywhere(&self, result: &ResultId) -> bool {
        self.failed.contains(&result.0)
            || self
                .outer
                .as_ref()
                .is_some_and(|outer| outer.failed(result))
    }
    /// Whether the event at hand is a move a replacement changed, its steps
    /// coming after it (CR 614.6): the object it moved is a new one there.
    pub(super) fn reads_replaced_move(&self) -> bool {
        self.pending == Pending::Reshaped && self.replacing.kind == "ZoneChange"
    }
    /// "Whenever this or that", of events the engine groups by different rules: an
    /// ability for each. No one event is both, so each happening triggers one.
    pub(super) fn triggered_each(
        &mut self,
        label: &str,
        events: &[EventSpec],
        intervening_if: Option<&Condition>,
        limits: &TriggerLimits,
        announcement: &Announcement,
        body: &Program,
    ) -> Result<String> {
        let mut defs = Vec::new();
        for (i, event) in events.iter().enumerate() {
            let each = format!("{label}_or_{i}");
            let def = self.triggered(&each, event, intervening_if, limits, announcement, body)?;
            // "This ability triggers only once each turn" is of the one ability.
            defs.push(match limits.per_turn {
                Some(_) => planned::planned_value(
                    "a limit shared by the abilities of one sentence",
                    &def,
                    &format!("{def}.limit_shared_with({})", quoted(label)),
                ),
                None => def,
            });
        }
        Ok(defs.join(", "))
    }
    /// What a replacement made by an effect will be where no event of Cantrip's
    /// could carry it.
    pub(super) fn stated_replacement(
        &mut self,
        event: &ReplacementEvent,
        limit: &ReplacementLimit,
        duration: &crate::ir::Duration,
        body: &Program,
        out: &mut String,
    ) -> Result<bool> {
        match (event, limit, body.instructions.as_slice()) {
            // "If target Plains is tapped for mana, it produces colorless mana
            // instead of white mana": an effect on that land.
            (
                ReplacementEvent::ProduceColor {
                    sources: ObjectFilter::SameObject(object),
                    color,
                },
                ReplacementLimit::UntilExpired,
                [Instruction::Effect {
                    operation:
                        Operation::ModifyEvent {
                            change:
                                EventChange::ReplaceMana(ManaProduction::Amount {
                                    color: instead,
                                    amount: Expr::Constant(1),
                                }),
                        },
                    ..
                }],
            ) => {
                let object = self.fetch(self.object(object)?);
                let until = self.stated_until(duration)?;
                let (color, instead) = (lane_j::mana_letter(color), lane_j::mana_letter(instead));
                self.plan_code(
                    "replacement event ProduceColor",
                    &format!("if let Some(object) = {object} {{ g.add_effect(asker.source, object, EffectKind::ProducesInstead {{ color: Mana::{color}, instead: Mana::{instead} }}, {until}); }}"),
                    out,
                );
                Ok(true)
            }
            // "During your next untap step, as you untap your permanents": a mana
            // ability has no item to hold such an effect.
            (
                ReplacementEvent::Player {
                    event: ReplacedPlayerEvent::UntapPermanents,
                    players: PlayerSet::One(player),
                },
                ReplacementLimit::NextOccurrence,
                _,
            ) if !self.stack_item => {
                let player = self.fetch(self.certain_player(player)?);
                let action = self.stated(&Operand::Program(body.clone()))?;
                self.plan_code(
                    "replacement made outside a resolving ability",
                    &format!("cantrip_mechanics::untap::as_permanents_untap(g, asker.source, {player}, {action})"),
                    out,
                );
                Ok(true)
            }
            _ => Ok(false),
        }
    }
    /// `event` during an activation, as a cost being paid is.
    pub(super) fn while_activating(
        &mut self,
        event: &EventSpec,
        players: &PlayerSet,
        abilities: &ObjectFilter,
    ) -> Result<triggers::Matched> {
        let mut matched = self.event(event)?;
        let frame = matched.frame;
        let who = self.matching(frame, |c| c.player_in(players, "player"))?;
        let what = self.activation_test(abilities, frame)?;
        let test = planned::planned_value(
            "trigger event WhileActivating",
            "false",
            &format!("q.activation_under_way().is_some_and(|(player, def): (PlayerId, Option<&ActivatedDef>)| {who} && {what})"),
        );
        for arm in &mut matched.arms {
            arm.tests.push(test.clone());
        }
        Ok(matched)
    }
    /// What playing a card this way does: "it" is the spell cast.
    pub(super) fn play_rider(&mut self, effect: &Operand) -> Result<String> {
        let around = (self.event_at, self.frame);
        (self.event_at, self.frame) = (
            EventAt::Item,
            Frame {
                object: Some(Carried::Subject),
                ..Frame::default()
            },
        );
        let rider = self.stated(effect);
        (self.event_at, self.frame) = around;
        let rider = rider?;
        Ok(match rider.contains("item.subject.object()") {
            true => rider
                .replace("item.subject.object()", "Some(spell)")
                .replace(
                    "|g: &mut dyn Game, asker: &Asker|",
                    "|g: &mut dyn Game, asker: &Asker, spell: ObjectId|",
                ),
            false => rider,
        })
    }
    /// The ability on the stack an activation event is about, as the object it is
    /// on the stack. Cantrip knows it by a serial, and only `Target` reads that.
    pub(super) fn activated_object(&self) -> Result<String> {
        Ok(planned::planned_value(
            "the ability an event is about",
            "None::<ObjectId>",
            &format!("q.ability_on_stack({})", self.carried_ability()?),
        ))
    }
    /// "Whenever a player gets this" from a boon: an ability of that player's,
    /// used up after `times` uses. It is compiled as a delayed ability is, so its
    /// event and its body read what its own trigger says.
    pub(super) fn create_boon(
        &mut self,
        feature: &str,
        player: String,
        times: String,
        event: &EventSpec,
        condition: Option<&Condition>,
        body: &Operand,
        out: &mut String,
    ) -> Result<()> {
        let Operand::Program(program) = body else {
            return Err(gap(feature));
        };
        let label = format!("boon_{}", self.fresh());
        let (limits, announcement) = (
            TriggerLimits::default(),
            Announcement {
                targets: Vec::new(),
                modes: None,
            },
        );
        let (outer, def) = self.closing_for_matcher(false, |c| {
            c.reset_results();
            c.triggered(&label, event, condition, &limits, &announcement, program)
        })?;
        if !outer.objects.borrow().is_empty() || outer.player.borrow().is_some() {
            return Err(gap("a boon that carries what its maker read"));
        }
        self.delayed.push(def);
        let index = self.delayed.len() - 1;
        let (_, _, card) = self.maker();
        self.plan_code(
            feature,
            &format!("cantrip_mechanics::boon::create(g, asker.source, {player}, {times}, AbilityRef::delayed({card}, {index}))"),
            out,
        );
        Ok(())
    }
    /// The rest of what a rule on triggering asks of an ability.
    pub(super) fn triggering_more(&self, filter: &ObjectFilter) -> Result<String> {
        use ObjectFilter as F;
        let source = Candidate {
            controller: "triggering.controller".into(),
            ..Candidate::of("triggering.source")
        };
        const FEATURE: &str = "what a rule on triggering asks of an ability";
        let asked = |code: &str| planned::planned_value(FEATURE, "false", code);
        Ok(match filter {
            F::All(filters) | F::Either(filters) => {
                let tests = filters
                    .iter()
                    .map(|f| self.triggering_ability(Some(f)))
                    .collect::<Result<Vec<_>>>()?;
                let joint = if matches!(filter, F::All(_)) {
                    " && "
                } else {
                    " || "
                };
                format!("({})", tests.join(joint))
            }
            F::OnStack(StackTrait::Triggered) => "true".into(),
            // A triggered ability of a spell: its source is on the stack.
            F::OnStack(StackTrait::Spell) => {
                "q.state().obj(triggering.source).zone.is(ZoneKind::Stack)".into()
            }
            F::OnStack(StackTrait::DungeonRoom) => asked("triggering.def.dungeon_room"),
            F::OnStack(StackTrait::OfEmblem) => asked("q.state().obj(triggering.source).is_emblem"),
            F::HasKeywordFamily(family) => asked(&format!(
                "triggering.def.keyword == Some(KeywordFamily::{family:?})"
            )),
            other => self.filter(other, &source)?,
        })
    }
    /// Countering what is on the stack, saying what became of it: "if a permanent's
    /// ability is countered this way" speaks of the ability's source, which is
    /// still that permanent only while it is on the battlefield (CR 113.7, 400.7).
    pub(super) fn counter_reporting(
        &mut self,
        id: usize,
        objects: &ObjectSet,
        result: &ResultId,
        out: &mut String,
    ) {
        let targets = self.stack_targets(objects).expect("a slot of the stack");
        let test = lane_i::STACK_ITEM;
        writeln!(out, "let items_{id}: Vec<(ObjectId, usize)> = {targets}.into_iter().filter_map(|t| {{ let stack = &g.state().stack; stack.iter().position(|e| {test}).map(|at| (stack[at].object, at)) }}).collect(); let abilities_{id}: Vec<ObjectId> = items_{id}.iter().filter(|&&(_, at)| g.state().stack[at].kind != StackKind::Spell).map(|&(o, _)| o).filter(|&o| g.state().obj(o).zone.is(ZoneKind::Battlefield)).collect(); let spells_{id}: Vec<ObjectId> = items_{id}.iter().filter(|&&(_, at)| g.state().stack[at].kind == StackKind::Spell).map(|&(o, _)| o).collect(); let countered_{id} = g.counter_items(&items_{id}); let mut moved_{id}: Vec<ObjectId> = if countered_{id} > 0 {{ abilities_{id} }} else {{ Vec::new() }}; moved_{id}.extend(spells_{id}.into_iter().filter_map(|o| g.state().obj(o).became));").unwrap();
        self.moved.insert(result.0, format!("moved_{id}"));
    }
    /// A damage event's recipient a made effect keeps, as a test of `target`.
    pub(super) fn named_recipient(&self, recipients: &Recipients) -> Result<String> {
        Ok(format!(
            "({}).contains(target)",
            self.recipients(recipients)?
        ))
    }
}

/// The steps of a program that prevent damage.
pub(super) fn preventions(program: &Program) -> Vec<u32> {
    let mut steps = Vec::new();
    lane_m::flat_groups(&program.instructions, &mut steps);
    steps
        .iter()
        .filter_map(|step| match step {
            Instruction::Effect {
                operation: Operation::ModifyEvent { change },
                result,
            } if prevents(std::slice::from_ref(change)) => Some(result.0),
            _ => None,
        })
        .collect()
}
/// "If this would enter and ...": the same replacement, asked as it enters.
pub(super) fn entering_if(event: &ReplacementEvent, body: &Program) -> Option<Program> {
    let ReplacementEvent::Enter {
        objects: ObjectFilter::All(filters),
    } = event
    else {
        return None;
    };
    let [ObjectFilter::SameObject(ObjectRef::Source), rest @ ..] = filters.as_slice() else {
        return None;
    };
    let filter = match rest {
        [one] => one.clone(),
        many => ObjectFilter::All(many.to_vec()),
    };
    Some(Program {
        instructions: vec![Instruction::If {
            condition: Condition::ObjectMatches {
                object: ObjectRef::Source,
                filter,
            },
            then: body.clone(),
            otherwise: Program {
                instructions: Vec::new(),
            },
        }],
    })
}
/// "It has changeling": an ability that makes its object every creature type.
pub(super) fn every_creature_type(ability: &Ability) -> bool {
    let Ability::Static {
        affected: EffectSubjects::Objects(ObjectSet::One(ObjectRef::Source)),
        changes,
        ..
    } = ability
    else {
        return false;
    };
    matches!(
        changes.as_slice(),
        [Change::CreatureTypes {
            mode: ChangeMode::Set,
            types: CreatureTypeSet::All,
        }]
    )
}
/// "A card exiled with this creature", of a card played: the object it was with.
pub(super) fn played_from_exile_with(objects: &ObjectFilter) -> Option<&ObjectRef> {
    let ObjectFilter::InSet(set) = objects else {
        return None;
    };
    match &**set {
        ObjectSet::Zone {
            scope: ZoneScope::Shared(EventZone::Exile),
            filter: ObjectFilter::ExiledWith(object),
            ..
        } => Some(object),
        _ => None,
    }
}
/// What is said of an activation by the words of an event object.
pub(super) fn of_the_activation(condition: &Condition, frame: Frame) -> bool {
    let Condition::ObjectMatches { object, filter } = condition else {
        return false;
    };
    let mana = |filter: &ObjectFilter| matches!(filter, ObjectFilter::OnStack(StackTrait::Mana));
    frame.activated.is_some()
        && matches!(
            object,
            ObjectRef::EventStackObject | ObjectRef::EventSubject
        )
        && match filter {
            ObjectFilter::OnStack(StackTrait::XInCost | StackTrait::TapSymbolInCost) => true,
            ObjectFilter::ManaSpentFrom(_) => true,
            ObjectFilter::Not(inner) => mana(inner),
            _ => false,
        }
}
/// "If it isn't a mana ability" reaches here without its negation: the AST says
/// the opposite of the card, so this stays a hole rather than the opposite rule.
pub(super) fn lost_its_negation(condition: &Condition, frame: Frame) -> bool {
    frame.activated.is_some()
        && matches!(
            condition,
            Condition::ObjectMatches {
                object: ObjectRef::EventStackObject,
                filter: ObjectFilter::OnStack(StackTrait::Mana),
            }
        )
}
/// A spell or an ability, as two events, where what reads the event must know
/// which: no one object stands for both, and no one happening is both.
pub(super) fn spell_or_ability(
    event: &EventSpec,
    intervening_if: Option<&Condition>,
    body: &Program,
) -> Option<Vec<EventSpec>> {
    if let EventSpec::Either(events) = event {
        // "That permanent or player": the two are read from different places.
        if format!("{body:?}").contains("DamagedByEvent") {
            return Some(events.clone());
        }
    }
    let events = match event {
        EventSpec::SpellOrActivation {
            players,
            spells,
            abilities,
        } => vec![
            EventSpec::Spell {
                event: SpellEvent::Cast,
                players: players.clone(),
                spells: spells.clone(),
            },
            EventSpec::Activated {
                players: players.clone(),
                abilities: abilities.clone(),
            },
        ],
        EventSpec::Either(events)
            if matches!(
                events.as_slice(),
                [EventSpec::Spell { .. }, EventSpec::Activated { .. }]
            ) =>
        {
            events.clone()
        }
        _ => return None,
    };
    let text = format!("{intervening_if:?} {body:?}");
    let reads = text.contains("EventStackObject")
        || text.contains("ObjectMatches { object: EventSubject, filter: ManaSpentFrom");
    reads.then_some(events)
}

/// Whether what comes later reads which cards a surveil put into the graveyard,
/// which is what `InResult` of it says. "Cards you put on top" has no word of its
/// own in the AST: `Named` of a surveil does not say which part it means.
pub(super) fn reads_graveyard_part(after: &[Instruction], result: &ResultId) -> bool {
    let r = result.0;
    let text = format!("{after:?}");
    text.contains(&format!("InResult(ResultId({r}))"))
        || text.contains(&format!(
            "what: PutInGraveyard, measure: Count, result: Some(ResultId({r}))"
        ))
}

/// A cost of moving objects it names, as "exile it".
pub(super) fn moves_named_objects(cost: &Cost) -> bool {
    matches!(
        cost.parts.as_slice(),
        [CostPart::Move { selections, face_down: false, .. }]
            if matches!(selections.as_slice(), [CostObjectSelection { choose: None, .. }])
    )
}

/// Whether what comes later reads a damage instruction's result as what it was
/// dealt to, which is only known from the events: a prevention names none and a
/// redirection names another (CR 614.9).
/// The one keyword a permanent gains for the turn as it arrives.
pub(super) fn keyword_until_end_of_turn(states: &[EntryState]) -> Option<&Keyword> {
    match states {
        [EntryState::ChangesUntil {
            changes,
            duration: crate::ir::Duration::EndOfTurn,
        }] => match changes.as_slice() {
            [Change::GrantKeyword(k)] => Some(k),
            _ => None,
        },
        _ => None,
    }
}

/// A power and a toughness set by one effect are the one thing the engine sets
/// (CR 613.4b). `None` where the list does not say both.
pub(super) fn power_with_toughness(changes: &[Change]) -> Option<Vec<Change>> {
    let power = changes.iter().position(|c| matches!(c, Change::SetPower(_)))?;
    let toughness = changes.iter().position(|c| matches!(c, Change::SetToughness(_)))?;
    let (Change::SetPower(p), Change::SetToughness(t)) = (&changes[power], &changes[toughness])
    else {
        return None;
    };
    let mut joined = changes.to_vec();
    joined[power] = Change::SetBasePowerToughness {
        power: p.clone(),
        toughness: t.clone(),
    };
    joined.remove(toughness);
    Some(joined)
}

/// Damage to every other permanent, where the words were "each other creature":
/// no card says that, and read as written it would burn planeswalkers.
pub(super) fn names_no_card_type(recipients: &Recipients) -> bool {
    format!("{recipients:?}").contains("Battlefield(Not(")
}

/// The N of toxic N, as a grant says it.
pub(super) fn toxic_of(ability: &Ability) -> Option<i64> {
    match ability {
        Ability::Mechanic {
            mechanic: Mechanic::Toxic,
            amount: Some(Expr::Constant(n)),
            ..
        } if *n > 0 => Some(*n),
        _ => None,
    }
}

/// Whether what comes later reads the excess of a damage instruction.
pub(super) fn reads_excess(after: &[Instruction], result: &ResultId) -> bool {
    format!("{after:?}").contains(&format!("ExcessDamage {{ result: ResultId({})", result.0))
}

pub(super) fn read_as_damaged(after: &[Instruction], result: &ResultId) -> bool {
    format!("{after:?}").contains(&format!("DamagedBy(ResultId({}))", result.0))
}

/// Say of `filter` that it is `each`, where it says it was dealt damage by
/// `result`, which a callback has no way to read.
pub(super) fn damaged_to_bound(filter: &mut ObjectFilter, result: u32, each: &ObjectRef) -> bool {
    match filter {
        ObjectFilter::DamagedBy(r) if r.0 == result => {
            *filter = ObjectFilter::SameObject(each.clone());
            true
        }
        ObjectFilter::All(filters) => filters
            .iter_mut()
            .fold(false, |found, f| damaged_to_bound(f, result, each) || found),
        _ => false,
    }
}

/// Whether what comes later reads an instruction's result as objects.
pub(super) fn read_as_objects(after: &[Instruction], result: &ResultId) -> bool {
    let r = result.0;
    let text = format!("{after:?}");
    let joined = after.iter().any(|i| matches!(i, Instruction::Effect { operation: Operation::JoinResults { results }, .. } if results.iter().any(|joined| joined.0 == r)));
    joined
        || [
            format!("Named(ResultId({r}))"),
            format!("Named {{ result: ResultId({r})"),
            format!("InResult(ResultId({r}))"),
            format!("Affected(ResultId({r}))"),
            format!("Transition {{ result: ResultId({r})"),
        ]
        .iter()
        .any(|read| text.contains(read))
}

/// Whether an amount a walk over the players made is read player by player.
pub(super) fn read_player_by_player(rest: &[Instruction], result: &u32) -> bool {
    let read = format!("Performed(ResultId({result}))");
    rest.iter().any(|instruction| {
        let text = format!("{instruction:?}");
        text.contains(&read)
            && (text.contains("players: All")
                || matches!(instruction, Instruction::ForEachPlayer { .. }))
    })
}

/// Whether a made effect's steps left a hole for a read of the ability making it.
pub(super) fn lost_its_maker(holes: &[Diagnostic]) -> bool {
    holes
        .iter()
        .any(|hole| hole.feature == "target reference outside a resolving ability")
}

/// A statement: `amount` more mana of `color` in the production `m`.
fn mana_gained(color: &ManaColor, amount: &str) -> String {
    match color_index(color) {
        Ok(at) => format!("m.mana.0[{at}] += {amount};"),
        Err(_) => format!(
            "{};",
            planned::planned_value(
                "event change replace colored mana",
                "()",
                &format!("m.add_chosen(q.chosen_color(asker), {amount})"),
            )
        ),
    }
}

/// The mana one may choose among, as `Mana` values.
fn mana_choices(production: &ManaProduction) -> Result<Vec<String>> {
    let ManaProduction::ChooseOne(options) = production else {
        return Err(gap(format!("event change mana {}", form(production))));
    };
    options
        .iter()
        .map(|option| match option {
            ManaProduction::Amount { color, .. } => Ok(format!(
                "Mana::{}",
                ["W", "U", "B", "R", "G", "C"][color_index(color)?]
            )),
            other => Err(gap(format!("event change mana {}", form(other)))),
        })
        .collect()
}

/// The field of `PlayerCounters` a kind of counter is kept in.
fn player_counter(kind: &crate::counter::CounterKind) -> Option<&'static str> {
    match format!("{kind:?}").as_str() {
        "Poison" | "Named(PoisonCounter)" => Some("poison"),
        "Energy" | "Named(EnergyCounter)" => Some("energy"),
        "Experience" | "Named(ExperienceCounter)" => Some("experience"),
        _ => None,
    }
}

/// The proposal an object's event will be.
fn object_proposal(event: ReplacedObjectEvent) -> &'static str {
    use ReplacedObjectEvent as E;
    match event {
        E::UntapsInUntapStep => "StepUntap",
        E::Connives => "Connive",
        E::Adapts => "Adapt",
        E::Explores => "Explore",
        E::BecomesTapped => "Tap",
        E::AssignsCombatDamage => "CombatDamageAssignment",
        E::Destroyed | E::LeavesBattlefield => "ZoneChange",
    }
}

/// The proposal a player's event will be, and whether it has a count.
fn player_proposal(event: ReplacedPlayerEvent) -> (&'static str, bool) {
    use ReplacedPlayerEvent as E;
    match event {
        E::Mill => ("Mill", true),
        E::Scry => ("Scry", true),
        E::RollDice | E::RollD6 => ("DieRoll", true),
        E::FlipCoin => ("CoinFlip", true),
        E::PayLife => ("PayLife", true),
        E::GainLifeByEffect => ("EffectLifeGain", true),
        E::LoseGame => ("LoseGame", false),
        E::Learn => ("Learn", false),
        E::Proliferate => ("Proliferate", false),
        E::BeginExtraTurn => ("ExtraTurn", false),
        E::BeginTurn => ("BeginTurn", false),
        E::BeginDrawStep => ("BeginDrawStep", false),
        E::Planeswalk => ("Planeswalk", false),
        E::RollPlanarDie => ("PlanarDieRoll", false),
        E::PlaneswalkByDie => ("PlaneswalkByDie", false),
        E::SetSchemeInMotion => ("SetSchemeInMotion", false),
        E::SearchLibrary => ("SearchLibrary", false),
        E::UntapPermanents => ("StepUntapPermanents", false),
        _ => ("Unsupported", false),
    }
}
