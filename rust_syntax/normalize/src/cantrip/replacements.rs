//! Replacement effects (CR 614, 615) of static abilities. Two pure callbacks
//! read and reshape the proposed event, and what the program does besides goes
//! in `then`, which the engine runs after every effect has had its say. An
//! ability about the source's own arrival is a part of `CardDef::enters`.
use super::*;

/// The X a number read with no stack item holds as a hole.
const ANNOUNCED_X_HELD: &str = "planned_value!(\"numeric expression AnnouncedX\",";
/// The same X in an effect a resolving ability made: the effect has no number of its own.
const MADE_EFFECT_X: &str =
    "planned_value!(\"the X of the ability that made a replacement effect\",";

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
pub(super) fn capture(event: &mut ReplacementEvent, found: &mut Vec<ObjectRef>) {
    fn named(filter: &mut ObjectFilter, found: &mut Vec<ObjectRef>) {
        match filter {
            ObjectFilter::SameObject(object) if !matches!(object, ObjectRef::Source) => {
                let key = format!("{object:?}");
                if !found.iter().any(|f| format!("{f:?}") == key) {
                    found.push(object.clone());
                }
                *object = ObjectRef::Affected;
            }
            ObjectFilter::All(filters) => {
                for filter in filters {
                    named(filter, found);
                }
            }
            _ => {}
        }
    }
    let mut name = |filter: &mut ObjectFilter| named(filter, found);
    match event {
        ReplacementEvent::Damage(damage) => {
            name(&mut damage.sources);
            if let DamageRecipients::Objects(objects) = &mut damage.recipients {
                name(objects);
            }
        }
        ReplacementEvent::Enter { objects }
        | ReplacementEvent::ZoneChange { objects, .. }
        | ReplacementEvent::Object { objects, .. } => name(objects),
        ReplacementEvent::Either(events) => {
            for event in events {
                capture(event, found);
            }
        }
        ReplacementEvent::Measured { event, .. } => capture(event, found),
        _ => {}
    }
}

/// `capture` of a slot of several: "X target creatures would deal", which is one
/// effect on each of them.
pub(super) fn capture_slot(event: &mut ReplacementEvent, found: &mut Vec<TargetSlot>) {
    let mut name = |filter: &mut ObjectFilter| {
        if let ObjectFilter::InSet(set) = filter {
            if let ObjectSet::Targets(slot) = &**set {
                if !found.iter().any(|f| f.0 == slot.0) {
                    found.push(slot.clone());
                }
                *filter = ObjectFilter::SameObject(ObjectRef::Affected);
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
        ReplacementEvent::Either(events) => {
            for event in events {
                capture_slot(event, found);
            }
        }
        _ => {}
    }
}

/// A replacement runs after the ability that made it, without its targets.
/// "All damage is dealt to you as though its source had infect" (CR 702.90b,
/// 120.3b): a rule asked of the player as damage is dealt to them, as Phyrexian
/// Unlife prints it, not a replacement of the event. Only when nothing but the
/// player decides it.
fn infect_rule(
    label: &str,
    active: &ReplacementActive,
    arms: &[Watched],
    extra: &[String],
    changes: &[EventChange],
    rest: &[Instruction],
) -> Option<String> {
    if !matches!(changes, [EventChange::DamageAsInfect])
        || !rest.is_empty()
        || !matches!(active, ReplacementActive::Battlefield)
    {
        return None;
    }
    let [arm] = arms else { return None };
    if arm.kind != "Damage" {
        return None;
    }
    let tests: Vec<&str> = arm.tests.iter().chain(extra).map(String::as_str).collect();
    let reads = |test: &str, local: &str| {
        test.match_indices(local).any(|(at, _)| {
            let before = test[..at].chars().next_back();
            let after = test[at + local.len()..].chars().next();
            let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
            !word(before) && before != Some('.') && !word(after)
        })
    };
    if tests.iter().any(|test| {
        ["source", "amount", "combat", "data", "e"]
            .iter()
            .any(|local| reads(test, local))
    }) {
        return None;
    }
    Some(format!(
        "fn {label}_infect(q: &dyn Query, asker: &Asker, player: PlayerId) -> bool {{ let target = &Target::Player(player); let subject = *target; {} }}",
        tests.join(" && ")
    ))
}

fn names_target(recipients: &Recipients) -> bool {
    match recipients {
        Recipients::Targets(_) | Recipients::Target { .. } => true,
        Recipients::Union(parts) => parts.iter().any(names_target),
        _ => false,
    }
}

impl Compiler {
    pub(super) fn watched(&mut self, event: &ReplacementEvent) -> Result<(Vec<Watched>, Frame)> {
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
                if arms.iter().any(|a| {
                    !matches!(
                        a.kind,
                        "Damage" | "ManaProduction" | "LifeChange" | "DrawInstruction"
                    ) && self.replacing.planned.is_none()
                }) {
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
    pub(super) fn reshape(
        &mut self,
        kind: &str,
        change: &EventChange,
        frame: Frame,
    ) -> Result<String> {
        let number = |c: &mut Self, n: &Expr| c.matching(frame, |c| c.number(n));
        let counter = |c: &mut Self, kind: &crate::counter::CounterKind, amount: &Expr| {
            let n = number(c, amount)?.quantity(Quantity::CounterPlacement)?;
            let kind = counter_kind(kind)?;
            Ok::<_, Diagnostic>(format!(
                "z.counters[{kind} as usize] = z.counters[{kind} as usize].saturating_add({n});"
            ))
        };
        Ok(match (kind, change) {
            ("Damage", EventChange::SetAmount(Expr::Add(amount, plus)))
                if matches!(**plus, Expr::ObjectProperty { .. }) =>
            {
                let amount = number(self, amount)?.code;
                let plus = number(self, plus)?.code;
                if planned::switched_on(quantity::FEATURE) {
                    format!("e.amount = ({amount}).saturating_add(({plus}).max(0)).max(0);")
                } else if planned::switched_on(quantity::BOUNDED_32) {
                    format!("e.amount = (({amount}) as i64).saturating_add(i64::from(({plus}).max(0))).clamp(0, i64::from(u32::MAX)) as u32;")
                } else {
                    format!("e.amount = (({amount}) + ({plus}).max(0)).clamp(0, 65535) as u16;")
                }
            }
            ("Damage", EventChange::SetAmount(n)) => {
                format!(
                    "e.amount = {};",
                    number(self, n)?.quantity(Quantity::Prevention)?
                )
            }
            ("Damage", EventChange::Cancel) => "e.amount = 0;".into(),
            ("Damage", EventChange::RedirectDamage(recipients)) => {
                if names_target(recipients) && self.outer.is_none() {
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
                        return self.reshape_rest(kind, change, frame)
                    }
                    Destination::Exile => format!("z.to = Zone::exile({owner});"),
                    Destination::OwnersHand => format!("z.to = Zone::hand({owner});"),
                    Destination::OwnersGraveyard => format!("z.to = Zone::graveyard({owner});"),
                    Destination::OwnersLibraryShuffled => {
                        format!("z.to = Zone::library({owner}); z.shuffle = true;")
                    }
                    Destination::OwnersLibraryTop
                    | Destination::OwnersLibraryBottom
                    | Destination::OwnersBattlefield
                    | Destination::Battlefield(_) => return self.reshape_rest(kind, change, frame),
                    other => return Err(gap(format!("replacement destination {}", form(other)))),
                }
            }
            (kind, other) => return self.more_reshape(kind, other, frame),
        })
    }
    /// An `Enters` callback for the source's own arrival.
    pub(super) fn entering(&mut self, body: &Program, label: &str) -> Result<()> {
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
            return self.change_in_then(change, out);
        }
        match change {
            EventChange::SetTapped(tapped) => writeln!(out, "entering.tapped = {tapped};").unwrap(),
            EventChange::AddCounters { kind, amount } => {
                let n = self.number(amount)?.quantity(Quantity::CounterPlacement)?;
                let kind = counter_kind(kind)?;
                writeln!(
                    out,
                    "let more = {}; entering.counters[{kind} as usize] = entering.counters[{kind} as usize].saturating_add(more);",
                    self.fetch(n)
                )
                .unwrap();
            }
            EventChange::Entry(state) => {
                self.entry_state(&crate::movement::battlefield_state_view(state), out)?
            }
            EventChange::SetDestination(Destination::OwnersGraveyard) => writeln!(
                out,
                "entering.instead = Some(Zone::graveyard(g.obj(entering.object).owner));"
            )
            .unwrap(),
            EventChange::SetDestination(Destination::Exile) => writeln!(
                out,
                "entering.instead = Some(Zone::exile(g.obj(entering.object).owner));"
            )
            .unwrap(),
            EventChange::SetController(PlayerRef::OneOf(players)) => {
                let players = self.fetch(self.players(players)?);
                self.plan_code(
                    "event change as it enters SetController",
                    &format!("let players = {players}; if let Some(chosen) = cantrip_mechanics::choose::player(g, entering.controller, entering.object, players, \"GainControl\", 0)? {{ entering.controller = chosen; }}"),
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
            let tests: Vec<String> = arm
                .tests
                .iter()
                .chain(extra)
                .map(|t| lane_j::without_the_event(t))
                .collect();
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
            self.callbacks(),
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
        if let Some(events) = lane_n::separate_events(event) {
            for (i, event) in events.iter().enumerate() {
                self.replacement(&format!("{label}_{i}"), active, event, body, parts)?;
            }
            return Ok(());
        }
        self.replacing = Default::default();
        let guard = self.ability_guard.take();
        if guard.is_none() && lane_h::reveal_and_shuffle_instead(active, event, body) {
            parts.replacements.push(
                "cantrip_mechanics::replacements::REVEAL_AND_SHUFFLE_INTO_LIBRARY_INSTEAD_OF_GRAVEYARD"
                    .into(),
            );
            return Ok(());
        }
        if let ReplacementActive::SourceEntering = active {
            if lane_j::escapes(event) {
                if guard.is_some() {
                    return Err(gap("conditional Replacement"));
                }
                self.escapes_with(body, label, parts)?;
                return Ok(());
            }
            // "If [condition], this enters with ...": the condition is asked as
            // it would enter (CR 614.1c, 614.12), as a static one's is.
            if guard.is_some() && self.static_guard.is_some() {
                return Err(gap("conditional Replacement"));
            }
            let narrowed = lane_n::entering_if(event, body);
            let body = narrowed.as_ref().unwrap_or(body);
            // "If this creature would enter and it wasn't cast or no mana was spent
            // to cast it" (Primeval Spawn): as it would enter it is still the spell
            // on the stack (`Entering::object`, CR 614.12), whose cast record the
            // read finds; a card that wasn't cast has none.
            let about_its_cast = |event: &ReplacementEvent| {
                fn cast_only(f: &ObjectFilter) -> bool {
                    match f {
                        ObjectFilter::Cast { .. } | ObjectFilter::ManaSpent { .. } => true,
                        ObjectFilter::Not(f) => cast_only(f),
                        ObjectFilter::All(fs) | ObjectFilter::Either(fs) => {
                            fs.iter().all(cast_only)
                        }
                        _ => false,
                    }
                }
                matches!(event, ReplacementEvent::Enter { objects: ObjectFilter::All(fs) } if fs[1..].iter().all(cast_only))
            };
            if narrowed.is_some() && about_its_cast(event) {
            } else if narrowed.is_some() {
                // What it was cast as is the spell's, and it is leaving the stack.
                self.gaps
                    .push(gap("as-enters replacement of another event"));
            } else if !matches!(
                event,
                ReplacementEvent::Enter {
                    objects: ObjectFilter::SameObject(ObjectRef::Source),
                }
            ) {
                return Err(gap("as-enters replacement of another event"));
            }
            let function = format!("{label}_enters");
            match guard.or_else(|| self.static_guard.clone()) {
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
        self.note_event(event, &arms);
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
                            Ok(c.number(n)?.quantity(Quantity::Prevention)?)
                        })?))
                    }
                    EventChange::PreventAllButDamage(n) => {
                        Some(Some(self.matching(frame, |c| {
                            let kept = c.number(n)?.quantity(Quantity::Prevention)?;
                            Ok(format!("amount.saturating_sub({kept})"))
                        })?))
                    }
                    _ => None,
                };
                if let Some(amount) = amount {
                    let then =
                        self.prevention_then(label, &arms, frame, &rest, "", (true, true))?;
                    let relative = match change {
                        EventChange::PreventSomeDamage(n) => quantity::relative_to_event(n),
                        EventChange::PreventAllButDamage(_) => true,
                        _ => false,
                    };
                    return self
                        .prevention(label, &arms, &extra, amount, relative, hint, then, parts);
                }
            }
        }
        if let Some(rule) = infect_rule(label, active, &arms, &extra, &changes, &rest) {
            writeln!(self.functions, "#[allow(unused_variables)] {rule}").unwrap();
            parts.table("infect_damage_to", format!("{label}_infect"));
            return Ok(());
        }
        if let [arm] = arms.as_slice() {
            if arm.kind == "BeginTurn" && matches!(active, ReplacementActive::Battlefield) {
                return self
                    .turn_begin_rule(label, arm, frame, &extra, &changes, &rest, hint, parts);
            }
        }
        let damage = self.replacement_damage(label, &arms, frame, &rest, false)?;
        let remaining = if damage.is_some() { &[][..] } else { &rest };
        let (kind, applies, apply, then) =
            self.reshaping(label, &arms, frame, &extra, &changes, remaining, false)?;
        let mut def = format!("replacement(ReplacementKind::{kind}, {applies}, {apply}, {hint})");
        if let Some(damage) = damage {
            def.push_str(&format!(".with_damage({damage})"));
        }
        if matches!(active, ReplacementActive::Everywhere) {
            def.push_str(".own_everywhere()");
        }
        if let Some(then) = then {
            def.push_str(&format!(".then({then})"));
        }
        if let Some(ask) = self.replacing.may.take() {
            def.push_str(&format!(".optional(&{ask})"));
        }
        self.add_replacement(def, parts);
        Ok(())
    }
    /// "If you would begin your turn while ..., you may skip that turn instead"
    /// (CR 614.10): no proposal carries a turn's beginning, so the effect is a
    /// `TurnBeginRule` (Time Vault), which its controller may decline, and what it
    /// does besides skipping the turn is its `on_skip`. Only a turn of the
    /// effect's controller: `on_skip` is told no player.
    #[allow(clippy::too_many_arguments)]
    fn turn_begin_rule(
        &mut self,
        label: &str,
        arm: &Watched,
        frame: Frame,
        extra: &[String],
        changes: &[EventChange],
        rest: &[Instruction],
        hint: String,
        parts: &mut Parts,
    ) -> Result<()> {
        const YOURS: &str = "Some(asker.controller) == Some(p.player)";
        let skips = matches!(changes, [EventChange::Cancel]);
        let Some(may) = self
            .replacing
            .may
            .take()
            .filter(|_| skips && arm.tests.iter().any(|t| t == YOURS))
        else {
            return Err(gap(
                "replacement of BeginTurn other than a turn of yours you may skip",
            ));
        };
        // The rule asks its own question: the generic "instead" one goes.
        let asked = format!("static {may}: Question");
        if let Some(at) = self.functions.find(&asked) {
            let end = self.functions[at..]
                .find('\n')
                .map_or(self.functions.len(), |n| at + n + 1);
            self.functions.replace_range(at..end, "");
        }
        self.replacing.planned = None;
        let tests: Vec<&str> = arm.tests.iter().chain(extra).map(String::as_str).collect();
        writeln!(self.functions, "#[allow(unused_variables)] fn {label}_turn_begins(q: &dyn Query, asker: &Asker, player: PlayerId) -> bool {{ struct Seen {{ player: PlayerId }} let p = Seen {{ player }}; {} {} }}", arm.locals, tests.join(" && ")).unwrap();
        let out = self.then_steps(frame, rest, None)?;
        writeln!(self.functions, "#[allow(unused_variables, unused_mut)] fn {label}_on_skip(g: &mut dyn Game, asker: &Asker) -> Result<(), Suspend> {{ let subject = Target::Player(asker.controller); let data = [u32::MAX; 2]; {out} Ok(()) }}").unwrap();
        let question = format!("{}_SKIP", label.to_uppercase());
        writeln!(self.functions, "static {question}: Question = Question::new(\"SkipTurn\", \"skip this turn?\", {hint}, \"Skip the turn\", \"Take the turn\").no_default();").unwrap();
        parts.table(
            "turn_begin",
            format!("TurnBeginRule {{ applies: {label}_turn_begins, on_skip: {label}_on_skip, question: &{question}, hint: {hint} }}"),
        );
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
        let (changes, rest) = self.split_program(program, frame);
        Ok((extra, changes, rest))
    }
    /// Damage an effect adds while replacing damage belongs to that same event
    /// (CR 120.4b-d, 614.6). A mutating `then` would deal a separate event, with
    /// different life totals, infect rules and replacement eligibility.
    fn replacement_damage(
        &mut self,
        label: &str,
        arms: &[Watched],
        frame: Frame,
        rest: &[Instruction],
        made: bool,
    ) -> Result<Option<String>> {
        if arms[0].kind != "Damage" || rest.is_empty() {
            return Ok(None);
        }
        use mtgish_ir::visit::{Visitor, Walk};
        struct AddsDamage(bool);
        impl Visitor for AddsDamage {
            fn operation(&mut self, operation: &Operation) -> bool {
                self.0 |= matches!(operation, Operation::DealDamage { .. });
                // A separately created replacement or delayed ability does not
                // add damage to this event merely because its future body does.
                false
            }
        }
        let mut adds_damage = AddsDamage(false);
        rest.walk(&mut adds_damage);
        if !adds_damage.0 {
            return Ok(None);
        }
        if !planned::switched_on("simultaneous-replacement-damage") {
            return Err(gap("simultaneous additional replacement damage"));
        }
        let mut out = String::from("let mut components = Vec::new(); ");
        let mut flat = Vec::new();
        lane_m::flat_groups(rest, &mut flat);
        for step in &flat {
            let Instruction::Effect {
                operation:
                    Operation::DealDamage {
                        source,
                        recipients,
                        amount,
                        preventable: true,
                    },
                ..
            } = step
            else {
                return Err(gap(
                    "additional replacement damage mixed with other instructions",
                ));
            };
            let (source, recipients, amount) = self.matching(frame, |c| {
                Ok((
                    c.object(source)?,
                    c.recipients(recipients)?,
                    c.number(amount)?.quantity(Quantity::Damage)?,
                ))
            })?;
            write!(out, "if let Some(source) = {source} {{ let amount = {amount}; for target in {recipients} {{ components.push(DamageComponent::Additional(DamageEvent {{ source, target, amount, combat: false, heal_marked: false, shielded: None }})); }} }} ").unwrap();
        }
        out.push_str("components");
        let function = format!("{label}_damage");
        let about = if made { "affected: ObjectId, " } else { "" };
        writeln!(self.callbacks(), "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, {about}before: &DamageEvent, _: &DamageEvent) -> Vec<DamageComponent> {{ let {} = before; {} {out} }}", arms[0].binding, arms[0].locals).unwrap();
        Ok(Some(function))
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
        self.replacing.event = Some((variant, binding, frame));
        let shape = lane_n::shape_of(kind, variant);
        let mut edits = String::new();
        for change in changes {
            edits.push_str(&self.reshape(shape, change, frame)?);
        }
        edits.push_str(&self.guarded_edits(shape, frame)?);
        edits.push_str(&self.replacing.edits);
        let apply = format!("{label}_apply");
        // Reads see a copy, so `apply` may write as it reads.
        writeln!(self.callbacks(), "#[allow(unused_variables, unused_mut)] fn {apply}(q: &dyn Query, asker: &Asker, {about}proposal: &mut Proposal) {{ let seen = *proposal; if let Proposal::{variant}(seen) = &seen {{ let {binding} = seen; {locals} if let Proposal::{variant}({binding}) = proposal {{ {edits} }} }} }}").unwrap();
        if rest.is_empty() {
            return Ok((kind, applies, apply, None));
        }
        let then = format!("{label}_then");
        let out = self.then_steps(frame, rest, None)?;
        let pass = if made { "affected, " } else { "" };
        let (quantity_param, quantity_arg) = if frame.quantity.is_some() {
            (", event_quantity: i64", ", event_quantity")
        } else {
            ("", "")
        };
        writeln!(self.functions, "#[allow(unused_variables, unused_mut)] fn {then}_steps(g: &mut dyn Game, asker: &Asker, {about}subject: Target, data: [u32; 2]{quantity_param}) -> Result<(), Suspend> {{ {out} Ok(()) }}").unwrap();
        writeln!(self.callbacks(), "#[allow(unused_variables)] fn {then}(g: &mut dyn Game, asker: &Asker, {about}before: &Proposal, _: &Proposal) {{ if let Proposal::{variant}({binding}) = before {{ let q: &dyn Query = g; {locals} let _ = {then}_steps(g, asker, {pass}subject, data{quantity_arg}); }} }}").unwrap();
        Ok((kind, applies, apply, Some(then)))
    }
    /// What an effect does besides changing its event, run once it has: the
    /// statements, with `performed` as the local that says how it went.
    pub(super) fn then_steps(
        &mut self,
        frame: Frame,
        rest: &[Instruction],
        performed: Option<(u32, &str, Quantity)>,
    ) -> Result<String> {
        let hoisted = self.hoisted.take();
        let out = self.then_program(frame, rest, performed);
        self.hoisted = hoisted;
        out
    }
    fn then_program(
        &mut self,
        frame: Frame,
        rest: &[Instruction],
        performed: Option<(u32, &str, Quantity)>,
    ) -> Result<String> {
        self.reset_results();
        if let Some((result, local, domain)) = performed {
            self.report_quantity(result, local, domain);
        }
        for result in self.replacing.changed.clone() {
            self.performed.insert(result, "true".to_string());
            if frame.object == Some(Carried::Subject) {
                self.moved.insert(
                    result,
                    "subject.object().into_iter().collect::<Vec<ObjectId>>()".into(),
                );
            }
        }
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
        let steps = Program {
            instructions: rest.to_vec(),
        };
        let compiled = match self.dig(&steps, &mut out) {
            Ok(false) => self.program(&steps, &mut out, false),
            done => done.map(drop),
        };
        (
            self.reader,
            self.event_at,
            self.stack_item,
            self.pending,
            self.frame,
        ) = saved;
        compiled?;
        Ok(out)
    }
    /// Alternatives one replacement can't watch together: each is a replacement
    /// of its own, doing the same thing and ending the same way.
    fn alternatives_apart<'e>(
        &self,
        event: &'e ReplacementEvent,
    ) -> Option<&'e [ReplacementEvent]> {
        let ReplacementEvent::Either(events) = event else {
            return None;
        };
        let mut probe = self.clone();
        match probe.watched(event) {
            Err(failed)
                if failed.feature == "alternative replacement events of different kinds" =>
            {
                Some(events)
            }
            _ => None,
        }
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
        if let Some(events) = self.alternatives_apart(event) {
            for one in events {
                self.create_replacement(one, limit, duration, body, out)?;
            }
            return Ok(());
        }
        if self.stated_replacement(event, limit, duration, body, out)? {
            return Ok(());
        }
        if !self.stack_item || self.pending != Pending::Nothing {
            return Err(gap("replacement made outside a resolving ability"));
        }
        if self.divided_shields(event, limit, duration, body, out)? {
            return Ok(());
        }
        let saved = self.clone();
        let mark = out.len();
        let holes = self.gaps.len();
        let first = match self.free_replacement(event, limit, duration, body, out) {
            Ok(()) if !lane_n::lost_its_maker(&self.gaps[holes..]) => return Ok(()),
            Ok(()) => gap("target reference outside a resolving ability"),
            Err(first) => first,
        };
        *self = saved.clone();
        out.truncate(mark);
        if self.outer.is_none() {
            for one in [true, false] {
                match self.carried_replacement(event, limit, duration, body, one, out) {
                    Ok(()) => return Ok(()),
                    Err(_) => {
                        *self = saved.clone();
                        out.truncate(mark);
                    }
                }
            }
        }
        Err(first)
    }
    /// A made effect whose callbacks need nothing of the ability making it.
    fn free_replacement(
        &mut self,
        event: &ReplacementEvent,
        limit: &ReplacementLimit,
        duration: &crate::ir::Duration,
        body: &Program,
        out: &mut String,
    ) -> Result<()> {
        let duration = self.duration(duration)?;
        let shield = self.made_shield(limit, body)?;
        let mut plain = event.clone();
        let to = self.named_recipient_of(&mut plain)?;
        let mut about = Vec::new();
        capture(&mut plain, &mut about);
        if about.len() > 1 {
            return Err(gap("replacement made about several objects"));
        }
        self.choosing_source = about.first().and_then(|o| self.source_chosen(o));
        let about = about.first().map(|o| self.object(o)).transpose()?;
        let hint = quoted(&self.said(lane_k::Said::Sentence, ability_text(body)));
        let label = format!("made_{}", self.fresh());
        let functions_from = self.functions.len();
        let results = self.results();
        let scope = self.affected_in_scope;
        self.reset_results();
        self.affected_in_scope = true;
        // A loop's object is out of reach as well: it has to be the effect's.
        let bound = std::mem::take(&mut self.bound_objects);
        // An effect is made with no value of its own: "sources of the color of your
        // choice" is a definition for each color, and the choice says which is made.
        let by_color = results.chosen.iter().find(|(k, _)| *k == ValueKind::Color);
        // "Lands tapped for mana produce mana of the chosen color instead" (Hall
        // of Gemstone) is one for each color too.
        let of_mana = format!("{body:?}").contains("ReplaceColoredMana(Amount { color: Chosen");
        let by_color = by_color.map(|(_, local)| local.clone()).filter(|local| {
            !local.starts_with("colors_")
                && (of_mana
                    || (matches!(plain, ReplacementEvent::Damage(_))
                        && format!("{plain:?}").contains("Chosen(Color)")
                        && format!("{body:?}").contains("PreventDamage")))
        });
        let compiled = match by_color {
            Some(local) => {
                let first = match of_mana {
                    true => self.made_replacements.len(),
                    false => self.made_preventions.len(),
                };
                let mut made = Ok((None, String::new()));
                for color in lane_p::COLORS {
                    self.each_color = Some(format!("Color::{color}"));
                    made = self.made_effect(
                        &format!("{label}_{}", color.to_lowercase()),
                        &plain,
                        limit,
                        shield.clone(),
                        body,
                        hint.clone(),
                        to.is_some(),
                        about.is_some(),
                    );
                    self.each_color = None;
                    if made.is_err() {
                        break;
                    }
                }
                let arms: Vec<String> = lane_p::COLORS
                    .iter()
                    .enumerate()
                    .map(|(i, color)| format!("Color::{color} => {}", first + i))
                    .collect();
                let last = first + lane_p::COLORS.len() - 1;
                made.map(|(shield, kind)| {
                    let index = format!("index: match {local} {{ {} }}", arms.join(", "));
                    let kind = kind.replace(&format!("index: {last},"), &format!("{index},"));
                    (
                        shield,
                        kind.replace(&format!("index: {last} }}"), &format!("{index} }}")),
                    )
                })
            }
            None => self.made_effect(
                &label,
                &plain,
                limit,
                shield,
                body,
                hint,
                to.is_some(),
                about.is_some(),
            ),
        };
        self.bound_objects = bound;
        self.affected_in_scope = scope;
        self.restore_results(results);
        let kind = compiled?;
        // "{X}, {T}: ... this turn, it deals that much damage plus X instead" (Taii
        // Wakeen), "the next time you would draw a card this turn, instead look at
        // the top X cards" (Aladdin's Lamp): the effect is asked with no stack item,
        // and `EffectKind::ReplacementRule` keeps no number of the ability that made it.
        if self
            .functions
            .get(functions_from..)
            .is_some_and(|made| made.contains(ANNOUNCED_X_HELD))
        {
            let made = self.functions.split_off(functions_from);
            self.functions
                .push_str(&made.replace(ANNOUNCED_X_HELD, MADE_EFFECT_X));
        }
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
    /// Whom a made prevention protects, taken out of its event: the recipient
    /// is the effect's to keep, so the callbacks need not read it.
    pub(super) fn named_recipient_of(
        &self,
        event: &mut ReplacementEvent,
    ) -> Result<Option<String>> {
        let ReplacementEvent::Damage(damage) = event else {
            return Ok(None);
        };
        let to = match &damage.recipients {
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
        Ok(to)
    }
    /// The definition a created effect names, and how to make the effect: a
    /// shield's size to compute first, then a statement to run with `to`,
    /// `about` and `shield` bound.
    pub(super) fn made_effect(
        &mut self,
        label: &str,
        event: &ReplacementEvent,
        limit: &ReplacementLimit,
        shield: Option<String>,
        body: &Program,
        hint: String,
        to: bool,
        about: bool,
    ) -> Result<(Option<String>, String)> {
        self.replacing = Default::default();
        let (mut arms, frame) = self.watched(event)?;
        self.note_event(event, &arms);
        self.chosen_source(&mut arms, frame)?;
        let (extra, changes, rest) = self.program_parts(body, frame)?;
        if arms[0].kind == "Damage" && lane_n::prevents(&changes) {
            let alone = lane_n::parts_alone(event, to);
            let then =
                self.prevention_then(label, &arms, frame, &rest, "affected: ObjectId, ", alone)?;
            let of = self.made_prevention_amount(label, &arms, frame, &changes)?;
            let applies = format!("{label}_applies");
            self.watch_function(
                &applies,
                "(q: &dyn Query, asker: &Asker, affected: ObjectId, e: &cantrip_api::DamageEvent)",
                &arms,
                &extra,
                true,
            );
            let may = self.replacing.may.take();
            let text = quoted(&self.waiting_words());
            let hint = lane_n::hint_or_words(hint, &text);
            self.made_preventions.push(lane_n::granted_prevention(
                &hint,
                &applies,
                &text,
                then.as_deref(),
                of.as_deref(),
                may.as_deref(),
            ));
            let index = self.made_preventions.len() - 1;
            let next_time = matches!(limit, ReplacementLimit::NextOccurrence);
            let shield_value = if matches!(limit, ReplacementLimit::DamageToEach(_)) {
                planned::planned_value(
                    "prevention limited by DamageToEach",
                    "Shield::All",
                    "Shield::NextToEach(shield)",
                )
            } else if shield.is_some() {
                "Shield::Next(shield)".to_string()
            } else if next_time {
                "Shield::NextTime".to_string()
            } else {
                "Shield::All".to_string()
            };
            return Ok((shield, format!(
                "g.add_effect(item.object, about, EffectKind::Prevention {{ card: item.ability.card, index: {index}, to, shield: {shield_value} }}, DURATION);"
            )));
        }
        if to && !self.granting {
            return Err(gap("replacement of damage to a named recipient"));
        }
        let once = match limit {
            ReplacementLimit::UntilExpired => false,
            ReplacementLimit::NextOccurrence => true,
            _ if shield.is_some() => false,
            other => return Err(gap(format!("replacement limited by {}", form(other)))),
        };
        // `once` ends with the first assignment, and the next time is all of the
        // event (CR 120.2): only damage to one recipient by one source is that.
        if once && arms[0].kind == "Damage" && lane_n::parts_alone(event, to) != (true, true) {
            return Err(gap(
                "engine: no once-only replacement of a whole damage event",
            ));
        }
        let damage = self.replacement_damage(label, &arms, frame, &rest, true)?;
        let remaining = if damage.is_some() { &[][..] } else { &rest };
        let (kind, applies, apply, then) =
            self.reshaping(label, &arms, frame, &extra, &changes, remaining, true)?;
        let mut def =
            format!("granted_replacement(ReplacementKind::{kind}, {applies}, {apply}, {hint})");
        if let Some(damage) = damage {
            def.push_str(&format!(".with_damage({damage})"));
        }
        if once {
            def.push_str(".once()");
        }
        if let Some(then) = then {
            def.push_str(&format!(".then({then})"));
        }
        self.add_made_replacement(def);
        let index = self.made_replacements.len() - 1;
        let made = if shield.is_some() {
            lane_n::shielded_redirect(index)
        } else if about {
            format!("let _ = to; g.add_effect(item.object, about, EffectKind::Replacement {{ card: item.ability.card, index: {index} }}, DURATION);")
        } else {
            format!("let _ = (to, about); g.add_player_effect(item.controller, item.object, EffectKind::ReplacementRule {{ card: item.ability.card, index: {index} }}, DURATION);")
        };
        // The engine finds these through the source's own card, which a granted
        // ability is not printed on, and hands them no recipient.
        if self.granting {
            let mut planned = String::new();
            self.plan_code("replacement made by a granted ability", &made, &mut planned);
            return Ok((shield, planned));
        }
        Ok((shield, made))
    }
    fn prevention(
        &mut self,
        label: &str,
        arms: &[Watched],
        extra: &[String],
        amount: Option<String>,
        relative: bool,
        hint: String,
        then: Option<String>,
        parts: &mut Parts,
    ) -> Result<()> {
        let hint = faces::prevention_hint(hint);
        let applies = format!("{label}_applies");
        self.watch_function(
            &applies,
            "(q: &dyn Query, asker: &Asker, e: &cantrip_api::DamageEvent)",
            arms,
            extra,
            true,
        );
        let amount = match amount {
            None if planned::switched_on(quantity::FEATURE)
                || planned::switched_on("typed-prevention-amount") =>
            {
                "PreventionAmount::All".into()
            }
            None => "None".to_string(),
            Some(code) => {
                let function = format!("{label}_amount");
                writeln!(self.functions, "#[allow(unused_variables)] fn {function}(q: &dyn Query, asker: &Asker, e: &cantrip_api::DamageEvent) -> {prevention_ty} {{ {} {code} }}", arms[0].locals, prevention_ty = Quantity::Prevention.kind()).unwrap();
                if planned::switched_on(quantity::FEATURE)
                    || planned::switched_on("typed-prevention-amount")
                {
                    let kind = if relative { "Relative" } else { "Specific" };
                    format!("PreventionAmount::{kind}({function})")
                } else {
                    format!("Some({function})")
                }
            }
        };
        let may = self.replacing.may.take();
        let mut more = String::new();
        if let Some(then) = &then {
            write!(more, ", then: Some({then})").unwrap();
        }
        if let Some(ask) = &may {
            write!(more, ", optional: Some(&{ask})").unwrap();
        }
        let def =
            format!("PreventionDef {{ hint: {hint}, applies: {applies}, amount: {amount}{more} }}");
        match (then, may) {
            (None, None) => parts.preventions.push(def),
            (Some(_), _) => self.plan_entry(
                "engine: no static prevention with an effect besides",
                "preventions",
                &def,
                parts,
            ),
            (None, Some(_)) => self.plan_entry(
                "prevention a player may decline",
                "preventions",
                &def,
                parts,
            ),
        }
        Ok(())
    }
}
