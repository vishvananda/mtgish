//! Replacement effects (CR 614, 615) of static abilities. Two pure callbacks
//! read and reshape the proposed event, and what the program does besides goes
//! in `then`, which the engine runs after every effect has had its say. An
//! ability about the source's own arrival is a part of `CardDef::enters`.
use super::*;

/// One alternative of the pending event, as `Arm` is for a trigger: `locals`
/// bind `subject` and `data` from `binding`, a proposal's payload.
pub(super) struct Watched {
    pub kind: &'static str,
    pub variant: &'static str,
    pub binding: &'static str,
    pub locals: String,
    pub tests: Vec<String>,
}

/// Damage's values, as `damage_match` reads them, from a `DamageEvent` `e`.
const DAMAGE_LOCALS: &str =
    "let (source, target, amount, combat) = (e.source, &e.target, e.amount, e.combat);";

/// Replace the objects `event` names by what the callbacks will call them, and
/// collect those objects: the resolving ability's references mean nothing later.
/// Its source is the effect's, which the callbacks are told anyway.
fn capture(event: &mut ReplacementEvent, found: &mut Vec<ObjectRef>) {
    let mut name = |filter: &mut ObjectFilter| {
        if let ObjectFilter::SameObject(object) = filter {
            if !matches!(object, ObjectRef::Source) {
                let key = format!("{object:?}");
                if !found.iter().any(|f| format!("{f:?}") == key) {
                    found.push(object.clone());
                }
                *object = ObjectRef::Affected;
            }
        }
    };
    match event {
        ReplacementEvent::Damage(damage) => {
            name(&mut damage.sources);
            if let DamageRecipients::Objects(objects) = &mut damage.recipients {
                name(objects);
            }
        }
        ReplacementEvent::Enter { objects } | ReplacementEvent::ZoneChange { objects, .. } => {
            name(objects)
        }
        ReplacementEvent::Either(events) => {
            for event in events {
                capture(event, found);
            }
        }
        ReplacementEvent::Measured { event, .. } => capture(event, found),
        _ => {}
    }
}

/// A replacement runs after the ability that made it, without its targets.
fn names_target(recipients: &Recipients) -> bool {
    match recipients {
        Recipients::Targets(_) | Recipients::Target { .. } => true,
        Recipients::Union(parts) => parts.iter().any(names_target),
        _ => false,
    }
}

impl Compiler {
    fn watched(&mut self, event: &ReplacementEvent) -> Result<(Vec<Watched>, Frame)> {
        if let Some(found) = self.more_watched(event)? {
            return Ok(found);
        }
        Ok(match event {
            ReplacementEvent::Primitive { name, .. } => {
                return Err(gap(format!("primitive replacement event {name}")))
            }
            ReplacementEvent::Player { event, .. } => {
                return Err(gap(format!("replacement of {event:?}")))
            }
            ReplacementEvent::Object { event, .. } => {
                return Err(gap(format!("replacement of {event:?}")))
            }
            ReplacementEvent::Tokens { .. } => return Err(gap("replacement of token creation")),
            ReplacementEvent::Counters { .. } => return Err(gap("replacement of counters")),
            ReplacementEvent::ProduceMana { .. } => return Err(gap("replacement of mana")),
            ReplacementEvent::Damage(damage) => {
                let (frame, locals, tests) = self.damage_match(damage)?;
                (
                    vec![Watched {
                        kind: "Damage",
                        variant: "Damage",
                        binding: "e",
                        locals: format!("{DAMAGE_LOCALS} {locals}"),
                        tests,
                    }],
                    frame,
                )
            }
            ReplacementEvent::Enter { objects } => {
                let frame = Frame {
                    object: Some(Carried::Subject),
                    ..Frame::default()
                };
                let test = self.matching(frame, |c| c.filter(objects, &Candidate::entering()))?;
                (
                    vec![Watched {
                        kind: "ZoneChange",
                        variant: "ZoneChange",
                        binding: "z",
                        locals: "let id = z.object; let subject = Target::Object(id); let data = [u32::MAX; 2];".into(),
                        tests: vec!["z.to.is(ZoneKind::Battlefield)".into(), test],
                    }],
                    frame,
                )
            }
            ReplacementEvent::ZoneChange { objects, from, to } => {
                let frame = Frame {
                    object: Some(Carried::Subject),
                    ..Frame::default()
                };
                let mut tests = vec![format!("z.to.is({})", zone_kind(to)?)];
                if let Some(from) = from {
                    tests.push(format!("z.from.is({})", zone_kind(from)?));
                }
                tests.push(self.matching(frame, |c| c.filter(objects, &Candidate::of("id")))?);
                (
                    vec![Watched {
                        kind: "ZoneChange",
                        variant: "ZoneChange",
                        binding: "z",
                        locals: "let id = z.object; let subject = Target::Object(id); let data = [u32::MAX; 2];".into(),
                        tests,
                    }],
                    frame,
                )
            }
            ReplacementEvent::Either(events) => {
                let mut all: Vec<Watched> = Vec::new();
                let mut frame: Option<Frame> = None;
                for event in events {
                    let (arms, next) = self.watched(event)?;
                    if let Some(first) = all.first() {
                        if arms.iter().any(|a| a.kind != first.kind) {
                            return Err(gap("alternative replacement events of different kinds"));
                        }
                        // `apply` and `then` bind one way, so every arm has to.
                        if arms.iter().any(|a| a.locals != first.locals) {
                            return Err(gap("alternative replacement events bound differently"));
                        }
                    }
                    frame = Some(frame.map_or(next, |f| f.meet(next)));
                    all.extend(arms);
                }
                match frame {
                    Some(frame) => (all, frame),
                    None => return Err(gap("empty alternative event")),
                }
            }
            ReplacementEvent::Measured {
                event,
                comparison,
                amount,
            } => {
                let (mut arms, frame) = self.watched(event)?;
                if arms
                    .iter()
                    .any(|a| !matches!(a.kind, "Damage" | "ManaProduction" | "LifeChange"))
                {
                    return Err(gap("measured replacement of an event without an amount"));
                }
                let amount = self.matching(frame, |c| Ok(c.number(amount)?.code))?;
                for arm in &mut arms {
                    arm.tests
                        .push(read::compare("i64::from(amount)", comparison, &amount));
                }
                (arms, frame)
            }
            #[allow(unreachable_patterns)]
            other => return Err(gap(format!("replacement event {}", form(other)))),
        })
    }
    /// The change a static effect makes to a proposal, as statements with
    /// `binding` in scope; `reads` are computed before the proposal is borrowed.
    fn reshape(&mut self, kind: &str, change: &EventChange, frame: Frame) -> Result<String> {
        let number = |c: &mut Self, n: &Expr| c.matching(frame, |c| c.number(n));
        let counter = |c: &mut Self, kind: &crate::counter::CounterKind, amount: &Expr| {
            let n = number(c, amount)?.amount(i16::MAX.into(), "i16")?;
            let kind = counter_kind(kind)?;
            Ok::<_, Diagnostic>(format!(
                "z.counters[{kind} as usize] = z.counters[{kind} as usize].saturating_add({n});"
            ))
        };
        Ok(match (kind, change) {
            ("Damage", EventChange::SetAmount(n)) => {
                format!(
                    "e.amount = {};",
                    number(self, n)?.amount(u16::MAX.into(), "u16")?
                )
            }
            ("Damage", EventChange::Cancel) => "e.amount = 0;".into(),
            ("Damage", EventChange::RedirectDamage(recipients)) => {
                if names_target(recipients) {
                    return Err(gap("redirect to a target the callback cannot read"));
                }
                let recipients = self.matching(frame, |c| c.recipients(recipients))?;
                format!("if let Some(recipient) = ({recipients}).first().copied() {{ e.target = recipient; }}")
            }
            ("ZoneChange", EventChange::SetTapped(tapped)) => format!("z.tapped = {tapped};"),
            ("ZoneChange", EventChange::AddCounters { kind, amount }) => {
                counter(self, kind, amount)?
            }
            ("ZoneChange", EventChange::SetDestination(destination)) => {
                let owner = "q.state().obj(z.object).owner";
                match destination {
                    Destination::Exile if self.links_exile => {
                        return Err(gap(
                            "exile instead that an ability reads as exiled with its source",
                        ))
                    }
                    Destination::Exile => format!("z.to = Zone::exile({owner});"),
                    Destination::OwnersHand => format!("z.to = Zone::hand({owner});"),
                    Destination::OwnersGraveyard => format!("z.to = Zone::graveyard({owner});"),
                    Destination::OwnersLibraryShuffled => {
                        format!("z.to = Zone::library({owner}); z.shuffle = true;")
                    }
                    other => return Err(gap(format!("replacement destination {}", form(other)))),
                }
            }
            (kind, other) => return self.more_reshape(kind, other, frame),
        })
    }
    /// An `Enters` callback for the source's own arrival.
    fn entering(&mut self, body: &Program, label: &str) -> Result<()> {
        self.reset_results();
        let saved = (
            self.reader,
            self.event_at,
            self.stack_item,
            self.pending,
            self.frame,
        );
        (
            self.reader,
            self.event_at,
            self.stack_item,
            self.pending,
            self.frame,
        ) = (
            Reader::Resolution,
            EventAt::Entering,
            false,
            Pending::Entering,
            Frame {
                object: Some(Carried::Subject),
                ..Frame::default()
            },
        );
        let mut out = String::new();
        let compiled = self.program(body, &mut out, false);
        self.chosen.clear();
        (
            self.reader,
            self.event_at,
            self.stack_item,
            self.pending,
            self.frame,
        ) = saved;
        compiled?;
        writeln!(self.functions, "#[allow(unused_variables, unused_mut)] fn {label}(g: &mut dyn Game, entering: &mut Entering) -> Result<(), Suspend> {{ let asker = &Asker::new(entering.object, entering.controller); {out} Ok(()) }}").unwrap();
        Ok(())
    }
    /// `Operation::ModifyEvent` inside an `Enters` callback.
    pub(super) fn modify_entering(&mut self, change: &EventChange, out: &mut String) -> Result<()> {
        if self.pending != Pending::Entering {
            return Err(gap("event change in the program of another event"));
        }
        match change {
            EventChange::SetTapped(tapped) => writeln!(out, "entering.tapped = {tapped};").unwrap(),
            EventChange::AddCounters { kind, amount } => {
                let n = self.number(amount)?.amount(i16::MAX.into(), "i16")?;
                let kind = counter_kind(kind)?;
                writeln!(
                    out,
                    "let more = {}; entering.counters[{kind} as usize] = entering.counters[{kind} as usize].saturating_add(more);",
                    self.fetch(n)
                )
                .unwrap();
            }
            EventChange::Entry(state) => self.entry_state(state, out)?,
            EventChange::SetDestination(Destination::OwnersGraveyard) => writeln!(
                out,
                "entering.instead = Some(Zone::graveyard(g.obj(entering.object).owner));"
            )
            .unwrap(),
            EventChange::SetController(PlayerRef::OneOf(players)) => {
                let players = self.fetch(self.players(players)?);
                self.plan_code(
                    "event change as it enters SetController",
                    &format!("let players = {players}; entering.controller = cantrip_mechanics::choose::player(g, entering.controller, entering.object, &players)?;"),
                    out,
                );
            }
            other => return Err(gap(format!("event change as it enters {}", form(other)))),
        }
        Ok(())
    }
    /// `CardDef::enters`: each ability in printed order, since the field holds one.
    pub(super) fn enters(&mut self, enters: &[String]) -> String {
        match enters {
            [] => "None".into(),
            [one] => format!("Some({one})"),
            many => {
                let calls: String = many
                    .iter()
                    .map(|f| format!("{f}(g, entering)?; "))
                    .collect();
                writeln!(self.functions, "fn entering_all(g: &mut dyn Game, entering: &mut Entering) -> Result<(), Suspend> {{ {calls} Ok(()) }}").unwrap();
                "Some(entering_all)".into()
            }
        }
    }
    fn watch_function(
        &mut self,
        function: &str,
        signature: &str,
        arms: &[Watched],
        extra: &[String],
        prevention: bool,
    ) {
        let mut body = String::new();
        for arm in arms {
            let tests: Vec<&str> = arm.tests.iter().chain(extra).map(String::as_str).collect();
            if prevention {
                writeln!(
                    body,
                    "{{ {} if {} {{ return true; }} }}",
                    arm.locals,
                    tests.join(" && ")
                )
                .unwrap();
            } else {
                writeln!(
                    body,
                    "if let Proposal::{}({}) = proposal {{ {} if {} {{ return true; }} }}",
                    arm.variant,
                    arm.binding,
                    arm.locals,
                    tests.join(" && ")
                )
                .unwrap();
            }
        }
        writeln!(
            self.functions,
            "#[allow(unused_variables)] fn {function}{signature} -> bool {{ {body} false }}"
        )
        .unwrap();
    }
    pub(super) fn replacement(
        &mut self,
        label: &str,
        active: &ReplacementActive,
        event: &ReplacementEvent,
        body: &Program,
        parts: &mut Parts,
    ) -> Result<()> {
        let guard = self.ability_guard.take();
        if guard.is_none() && lane_h::reveal_and_shuffle_instead(active, event, body) {
            parts.replacements.push(
                "cantrip_mechanics::replacements::REVEAL_AND_SHUFFLE_INTO_LIBRARY_INSTEAD_OF_GRAVEYARD"
                    .into(),
            );
            return Ok(());
        }
        if let ReplacementActive::SourceEntering = active {
            if guard.is_some() {
                return Err(gap("conditional Replacement"));
            }
            let ReplacementEvent::Enter {
                objects: ObjectFilter::SameObject(ObjectRef::Source),
            } = event
            else {
                return Err(gap("as-enters replacement of another event"));
            };
            let function = format!("{label}_enters");
            match self.static_guard.clone() {
                Some(condition) => self.entering(
                    &Program {
                        instructions: vec![Instruction::If {
                            condition,
                            then: body.clone(),
                            otherwise: Program {
                                instructions: Vec::new(),
                            },
                        }],
                    },
                    &function,
                )?,
                None => self.entering(body, &function)?,
            }
            if lane_h::enters_as_copy(body) {
                parts.field("enters_as_copy", format!("Some({function})"));
            } else {
                parts.enters.push(function);
            }
            return Ok(());
        }
        let (arms, frame) = self.watched(event)?;
        let (mut extra, changes, rest) = self.program_parts(body, frame)?;
        let (changes, rest) = lane_h::life_amount(arms[0].kind, changes, rest);
        if let Some(guard) = &guard {
            extra.push(self.matching(frame, |c| c.condition(guard))?);
        }
        let hint = quoted(&self.said(lane_k::Said::Sentence, ability_text(body)));
        if arms[0].kind == "Damage" {
            if let [change] = changes.as_slice() {
                let amount = match change {
                    EventChange::PreventDamage => Some(None),
                    EventChange::PreventSomeDamage(n) => {
                        Some(Some(self.matching(frame, |c| {
                            Ok(c.number(n)?.amount(u16::MAX.into(), "u16")?)
                        })?))
                    }
                    EventChange::PreventAllButDamage(n) => {
                        Some(Some(self.matching(frame, |c| {
                            let kept = c.number(n)?.amount(u16::MAX.into(), "u16")?;
                            Ok(format!("amount.saturating_sub({kept})"))
                        })?))
                    }
                    _ => None,
                };
                if let Some(amount) = amount {
                    if !rest.is_empty() {
                        return Err(gap("prevention with an effect besides"));
                    }
                    return self.prevention(label, &arms, &extra, amount, hint, parts);
                }
            }
        }
        let (kind, applies, apply, then) =
            self.reshaping(label, &arms, frame, &extra, &changes, &rest, false)?;
        let mut def = format!("replacement(ReplacementKind::{kind}, {applies}, {apply}, {hint})");
        if matches!(active, ReplacementActive::Everywhere) {
            def.push_str(".own_everywhere()");
        }
        if let Some(then) = then {
            def.push_str(&format!(".then({then})"));
        }
        parts.replacements.push(def);
        Ok(())
    }
    /// What decides a replacement program and what it does: "if it does X,
    /// instead", an `if` around the whole of it, is one more thing that decides
    /// whether the effect applies at all (CR 614.1); then the changes to the
    /// event, then whatever else it does.
    fn program_parts(
        &mut self,
        body: &Program,
        frame: Frame,
    ) -> Result<(Vec<String>, Vec<EventChange>, Vec<Instruction>)> {
        let mut extra = Vec::new();
        let program = match body.instructions.as_slice() {
            [Instruction::If {
                condition,
                then,
                otherwise,
            }] if otherwise.instructions.is_empty() => {
                extra.push(self.matching(frame, |c| c.condition(condition))?);
                then
            }
            _ => body,
        };
        let split = program
            .instructions
            .iter()
            .take_while(|i| {
                matches!(
                    i,
                    Instruction::Effect {
                        operation: Operation::ModifyEvent { .. },
                        ..
                    }
                )
            })
            .count();
        let (changes, rest) = program.instructions.split_at(split);
        let changes = changes
            .iter()
            .filter_map(|i| match i {
                Instruction::Effect {
                    operation: Operation::ModifyEvent { change },
                    ..
                } => Some(change.clone()),
                _ => None,
            })
            .collect();
        Ok((extra, changes, rest.to_vec()))
    }
    /// The callbacks of an effect that reshapes its event: `made` for one a
    /// spell or ability made, whose callbacks are told the object it is about.
    fn reshaping(
        &mut self,
        label: &str,
        arms: &[Watched],
        frame: Frame,
        extra: &[String],
        changes: &[EventChange],
        rest: &[Instruction],
        made: bool,
    ) -> Result<(&'static str, String, String, Option<String>)> {
        let about = if made { "affected: ObjectId, " } else { "" };
        let applies = format!("{label}_applies");
        self.watch_function(
            &applies,
            &format!("(q: &dyn Query, asker: &Asker, {about}proposal: &Proposal)"),
            arms,
            extra,
            false,
        );
        let arm = &arms[0];
        let (kind, variant, binding, locals) =
            (arm.kind, arm.variant, arm.binding, arm.locals.clone());
        let mut edits = String::new();
        for change in changes {
            edits.push_str(&self.reshape(kind, change, frame)?);
        }
        let apply = format!("{label}_apply");
        // Reads see a copy, so `apply` may write as it reads.
        writeln!(self.functions, "#[allow(unused_variables, unused_mut)] fn {apply}(q: &dyn Query, asker: &Asker, {about}proposal: &mut Proposal) {{ let seen = *proposal; if let Proposal::{variant}(seen) = &seen {{ let {binding} = seen; {locals} if let Proposal::{variant}({binding}) = proposal {{ {edits} }} }} }}").unwrap();
        if rest.is_empty() {
            return Ok((kind, applies, apply, None));
        }
        let then = format!("{label}_then");
        self.reset_results();
        let saved = (
            self.reader,
            self.event_at,
            self.stack_item,
            self.pending,
            self.frame,
        );
        (
            self.reader,
            self.event_at,
            self.stack_item,
            self.pending,
            self.frame,
        ) = (
            Reader::Resolution,
            EventAt::Matcher,
            false,
            Pending::Reshaped,
            frame,
        );
        let mut out = String::new();
        let compiled = self.program(
            &Program {
                instructions: rest.to_vec(),
            },
            &mut out,
            false,
        );
        (
            self.reader,
            self.event_at,
            self.stack_item,
            self.pending,
            self.frame,
        ) = saved;
        compiled?;
        let pass = if made { "affected, " } else { "" };
        writeln!(self.functions, "#[allow(unused_variables, unused_mut)] fn {then}_steps(g: &mut dyn Game, asker: &Asker, {about}subject: Target, data: [u32; 2]) -> Result<(), Suspend> {{ {out} Ok(()) }}").unwrap();
        writeln!(self.functions, "#[allow(unused_variables)] fn {then}(g: &mut dyn Game, asker: &Asker, {about}before: &Proposal, _: &Proposal) {{ if let Proposal::{variant}({binding}) = before {{ {locals} let _ = {then}_steps(g, asker, {pass}subject, data); }} }}").unwrap();
        Ok((kind, applies, apply, Some(then)))
    }
    /// `Operation::CreateReplacement`: a continuous effect (CR 611.2) whose
    /// callbacks outlive the resolution, so all they can know of it is the
    /// object it is about and, for prevention, whom the damage is dealt to.
    pub(super) fn create_replacement(
        &mut self,
        event: &ReplacementEvent,
        limit: &ReplacementLimit,
        duration: &crate::ir::Duration,
        body: &Program,
        out: &mut String,
    ) -> Result<()> {
        if !self.stack_item || self.pending != Pending::Nothing {
            return Err(gap("replacement made outside a resolving ability"));
        }
        let duration = self.duration(duration)?;
        let mut plain = event.clone();
        let mut to = None;
        if let ReplacementEvent::Damage(damage) = &mut plain {
            to = match &damage.recipients {
                DamageRecipients::Target { slot, index } => {
                    Some(self.recipients(&Recipients::Target {
                        slot: slot.clone(),
                        index: *index,
                    })?)
                }
                DamageRecipients::Players(PlayerSet::One(player)) => {
                    Some(self.recipients(&Recipients::Players(PlayerSet::One(player.clone())))?)
                }
                DamageRecipients::Objects(ObjectFilter::SameObject(object)) => Some(format!(
                    "({}).map(Target::Object).into_iter().collect::<Vec<Target>>()",
                    self.object(object)?
                )),
                _ => None,
            };
            if to.is_some() {
                damage.recipients = DamageRecipients::Any;
            }
        }
        let mut about = Vec::new();
        capture(&mut plain, &mut about);
        if about.len() > 1 {
            return Err(gap("replacement made about several objects"));
        }
        let about = about.first().map(|o| self.object(o)).transpose()?;
        let hint = quoted(&self.said(lane_k::Said::Sentence, ability_text(body)));
        let label = format!("made_{}", self.fresh());
        let results = self.results();
        let scope = self.affected_in_scope;
        self.reset_results();
        self.affected_in_scope = true;
        let compiled = self.made_effect(
            &label,
            &plain,
            limit,
            body,
            hint,
            to.is_some(),
            about.is_some(),
        );
        self.affected_in_scope = scope;
        self.restore_results(results);
        let kind = compiled?;
        let to = to.map_or_else(
            || "Some(Target::None)".to_string(),
            |to| self.fetch(format!("({to}).first().copied()")),
        );
        // Without one of its own the effect is about whom it protects, which
        // keeps it once the spell that made it has gone (CR 400.7).
        let about = match about {
            Some(object) => self.fetch(object),
            None => "to.and_then(|to: Target| to.object()).or(Some(item.object))".into(),
        };
        let (shield, kind) = kind;
        let shield = shield.map_or(Ok(String::new()), |n| {
            Ok::<_, Diagnostic>(format!("let shield = {};", self.fetch(n)))
        })?;
        let effect = kind.replace("DURATION", &duration);
        writeln!(out, "{{ {shield} let to = {to}; let about = {about}; if let (Some(to), Some(about)) = (to, about) {{ {effect} }} }}").unwrap();
        Ok(())
    }
    /// The definition a created effect names, and how to make the effect: a
    /// shield's size to compute first, then a statement to run with `to`,
    /// `about` and `shield` bound.
    fn made_effect(
        &mut self,
        label: &str,
        event: &ReplacementEvent,
        limit: &ReplacementLimit,
        body: &Program,
        hint: String,
        to: bool,
        about: bool,
    ) -> Result<(Option<String>, String)> {
        let (arms, frame) = self.watched(event)?;
        let (extra, changes, rest) = self.program_parts(body, frame)?;
        if arms[0].kind == "Damage" && matches!(changes.as_slice(), [EventChange::PreventDamage]) {
            if !rest.is_empty() {
                return Err(gap("prevention with an effect besides"));
            }
            let shield = match limit {
                ReplacementLimit::UntilExpired => None,
                ReplacementLimit::TotalDamage(n) => {
                    let saved = (self.reader, self.affected_in_scope);
                    (self.reader, self.affected_in_scope) = (Reader::Resolution, false);
                    let amount = self.number(n);
                    (self.reader, self.affected_in_scope) = saved;
                    Some(amount?.amount(u16::MAX.into(), "u16")?)
                }
                other => return Err(gap(format!("prevention limited by {}", form(other)))),
            };
            let applies = format!("{label}_applies");
            self.watch_function(
                &applies,
                "(q: &dyn Query, asker: &Asker, affected: ObjectId, e: &cantrip_api::DamageEvent)",
                &arms,
                &extra,
                true,
            );
            self.made_preventions.push(format!(
                "GrantedPreventionDef {{ hint: {hint}, damage: \"damage\", of_it: None, applies: {applies} }}"
            ));
            let index = self.made_preventions.len() - 1;
            let shield_value = if shield.is_some() {
                "Shield::Next(shield)"
            } else {
                "Shield::All"
            };
            return Ok((shield, format!(
                "g.add_effect(item.object, about, EffectKind::Prevention {{ card: item.ability.card, index: {index}, to, shield: {shield_value} }}, DURATION);"
            )));
        }
        // The engine finds these through the source's own card, which a granted
        // ability is not printed on, and hands them no recipient.
        if self.granting {
            return Err(gap("replacement made by a granted ability"));
        }
        if to {
            return Err(gap("replacement of damage to a named recipient"));
        }
        let once = match limit {
            ReplacementLimit::UntilExpired => false,
            ReplacementLimit::NextOccurrence => true,
            other => return Err(gap(format!("replacement limited by {}", form(other)))),
        };
        let (kind, applies, apply, then) =
            self.reshaping(label, &arms, frame, &extra, &changes, &rest, true)?;
        let mut def =
            format!("granted_replacement(ReplacementKind::{kind}, {applies}, {apply}, {hint})");
        if once {
            def.push_str(".once()");
        }
        if let Some(then) = then {
            def.push_str(&format!(".then({then})"));
        }
        self.made_replacements.push(def);
        let index = self.made_replacements.len() - 1;
        Ok((
            None,
            if about {
                format!("let _ = to; g.add_effect(item.object, about, EffectKind::Replacement {{ card: item.ability.card, index: {index} }}, DURATION);")
            } else {
                format!("let _ = (to, about); g.add_player_effect(item.controller, item.object, EffectKind::ReplacementRule {{ card: item.ability.card, index: {index} }}, DURATION);")
            },
        ))
    }
    fn prevention(
        &mut self,
        label: &str,
        arms: &[Watched],
        extra: &[String],
        amount: Option<String>,
        hint: String,
        parts: &mut Parts,
    ) -> Result<()> {
        let applies = format!("{label}_applies");
        self.watch_function(
            &applies,
            "(q: &dyn Query, asker: &Asker, e: &cantrip_api::DamageEvent)",
            arms,
            extra,
            true,
        );
        let amount = match amount {
            None => "None".to_string(),
            Some(code) => {
                let function = format!("{label}_amount");
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, e: &cantrip_api::DamageEvent) -> u16 {{ {} {code} }}", arms[0].locals).unwrap();
                format!("Some({function})")
            }
        };
        parts.preventions.push(format!(
            "PreventionDef {{ hint: {hint}, applies: {applies}, amount: {amount} }}"
        ));
        Ok(())
    }
}
