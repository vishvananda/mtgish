//! Replacement effects (CR 614, 615) of static abilities. Two pure callbacks
//! read and reshape the proposed event, and what the program does besides goes
//! in `then`, which the engine runs after every effect has had its say. An
//! ability about the source's own arrival is a part of `CardDef::enters`.
use super::*;

/// One alternative of the pending event, as `Arm` is for a trigger: `locals`
/// bind `subject` and `data` from `binding`, a proposal's payload.
struct Watched {
    kind: &'static str,
    variant: &'static str,
    binding: &'static str,
    locals: String,
    tests: Vec<String>,
}

/// Damage's values, as `damage_match` reads them, from a `DamageEvent` `e`.
const DAMAGE_LOCALS: &str =
    "let (source, target, amount, combat) = (e.source, &e.target, e.amount, e.combat);";

impl Compiler {
    fn watched(&mut self, event: &ReplacementEvent) -> Result<(Vec<Watched>, Frame)> {
        Ok(match event {
            ReplacementEvent::Primitive { name, .. } => {
                return Err(gap(format!("primitive replacement event {name}")))
            }
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
                if arms.iter().any(|a| a.kind != "Damage") {
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
                format!("e.amount = {};", number(self, n)?.amount(u16::MAX.into(), "u16")?)
            }
            ("Damage", EventChange::Cancel) => "e.amount = 0;".into(),
            ("Damage", EventChange::RedirectDamage(recipients)) => {
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
                    Destination::Exile => format!("z.to = Zone::exile({owner});"),
                    Destination::OwnersHand => format!("z.to = Zone::hand({owner});"),
                    Destination::OwnersGraveyard => format!("z.to = Zone::graveyard({owner});"),
                    Destination::OwnersLibraryShuffled => {
                        format!("z.to = Zone::library({owner}); z.shuffle = true;")
                    }
                    other => {
                        return Err(gap(format!("replacement destination {}", form(other))))
                    }
                }
            }
            (_, other) => return Err(gap(format!("event change {}", form(other)))),
        })
    }
    /// An `Enters` callback for the source's own arrival.
    fn entering(&mut self, body: &Program, label: &str) -> Result<()> {
        self.reset_results();
        let saved = (self.reader, self.event_at, self.stack_item, self.pending);
        (self.reader, self.event_at, self.stack_item, self.pending) = (
            Reader::Resolution,
            EventAt::Nowhere,
            false,
            Pending::Entering,
        );
        let mut out = String::new();
        let compiled = self.program(body, &mut out, false);
        (self.reader, self.event_at, self.stack_item, self.pending) = saved;
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
            EventChange::SetTapped(tapped) => {
                writeln!(out, "entering.tapped = {tapped};").unwrap()
            }
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
                let calls: String = many.iter().map(|f| format!("{f}(g, entering)?; ")).collect();
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
                writeln!(body, "{{ {} if {} {{ return true; }} }}", arm.locals, tests.join(" && ")).unwrap();
            } else {
                writeln!(body, "if let Proposal::{}({}) = proposal {{ {} if {} {{ return true; }} }}", arm.variant, arm.binding, arm.locals, tests.join(" && ")).unwrap();
            }
        }
        writeln!(self.functions, "#[allow(unused_variables)] fn {function}{signature} -> bool {{ {body} false }}").unwrap();
    }
    pub(super) fn replacement(
        &mut self,
        label: &str,
        active: &ReplacementActive,
        event: &ReplacementEvent,
        body: &Program,
        parts: &mut Parts,
    ) -> Result<()> {
        if let ReplacementActive::SourceEntering = active {
            let ReplacementEvent::Enter {
                objects: ObjectFilter::SameObject(ObjectRef::Source),
            } = event
            else {
                return Err(gap("as-enters replacement of another event"));
            };
            let function = format!("{label}_enters");
            self.entering(body, &function)?;
            parts.enters.push(function);
            return Ok(());
        }
        let (arms, frame) = self.watched(event)?;
        // "If it does X, instead": an `if` around the whole program is one more
        // thing that decides whether the effect applies at all (CR 614.1).
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
        let changes: Vec<&EventChange> = changes
            .iter()
            .filter_map(|i| match i {
                Instruction::Effect {
                    operation: Operation::ModifyEvent { change },
                    ..
                } => Some(change),
                _ => None,
            })
            .collect();
        let hint = quoted(&ability_text(body));
        if arms[0].kind == "Damage" {
            if let [change] = changes.as_slice() {
                let amount = match change {
                    EventChange::PreventDamage => Some(None),
                    EventChange::PreventSomeDamage(n) => Some(Some(
                        self.matching(frame, |c| Ok(c.number(n)?.amount(u16::MAX.into(), "u16")?))?,
                    )),
                    EventChange::PreventAllButDamage(n) => Some(Some(self.matching(frame, |c| {
                        let kept = c.number(n)?.amount(u16::MAX.into(), "u16")?;
                        Ok(format!("amount.saturating_sub({kept})"))
                    })?)),
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
        let applies = format!("{label}_applies");
        self.watch_function(
            &applies,
            "(q: &dyn Query, asker: &Asker, proposal: &Proposal)",
            &arms,
            &extra,
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
        writeln!(self.functions, "#[allow(unused_variables, unused_mut)] fn {apply}(q: &dyn Query, asker: &Asker, proposal: &mut Proposal) {{ let seen = *proposal; if let Proposal::{variant}(seen) = &seen {{ let {binding} = seen; {locals} if let Proposal::{variant}({binding}) = proposal {{ {edits} }} }} }}").unwrap();
        let mut def = format!("replacement(ReplacementKind::{kind}, {applies}, {apply}, {hint})");
        if matches!(active, ReplacementActive::Everywhere) {
            def.push_str(".own_everywhere()");
        }
        if !rest.is_empty() {
            let then = format!("{label}_then");
            self.reset_results();
            let saved = (self.reader, self.event_at, self.stack_item, self.pending, self.frame);
            (self.reader, self.event_at, self.stack_item, self.pending, self.frame) = (
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
            (self.reader, self.event_at, self.stack_item, self.pending, self.frame) = saved;
            compiled?;
            writeln!(self.functions, "#[allow(unused_variables, unused_mut)] fn {then}_steps(g: &mut dyn Game, asker: &Asker, subject: Target, data: [u32; 2]) -> Result<(), Suspend> {{ {out} Ok(()) }}").unwrap();
            writeln!(self.functions, "fn {then}(g: &mut dyn Game, asker: &Asker, before: &Proposal, _: &Proposal) {{ if let Proposal::{variant}({binding}) = before {{ {locals} let _ = {then}_steps(g, asker, subject, data); }} }}").unwrap();
            def.push_str(&format!(".then({then})"));
        }
        parts.replacements.push(def);
        Ok(())
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
