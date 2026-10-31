//! Trigger events as generated matchers. A matcher binds what the event says
//! into `Fired`, whose subject and data reach the resolving ability; `Frame` is
//! the map of where each value went. The ability's text matters to the engine
//! only for `read_zone_words`, so `phrase` states exactly what those words read.
use super::*;

/// One alternative of an event: an `if let` over the event, then `locals`, which
/// must bind `subject` and `data`, then every test the event has to pass.
struct Arm {
    pattern: String,
    locals: String,
    tests: Vec<String>,
}

pub(super) struct Matched {
    masks: Vec<&'static str>,
    arms: Vec<Arm>,
    pub frame: Frame,
    /// The trigger's zones, when it cannot trigger from the battlefield.
    functions: Option<String>,
    /// "One or more": a batch triggers it once, split by `data[1]` when keyed.
    batch: Option<bool>,
    pub phrase: String,
}

fn object_fired(id: &str, destination: &str) -> String {
    format!("let id = {id}; let subject = Target::Object(id); let data = [{destination}, u32::MAX];")
}
fn single(mask: &'static str, arm: Arm, frame: Frame, phrase: &str) -> Matched {
    Matched {
        masks: vec![mask],
        arms: vec![arm],
        frame,
        functions: None,
        batch: None,
        phrase: phrase.into(),
    }
}

impl Compiler {
    /// Compile with the matcher's view of the event.
    pub(super) fn matching<T>(
        &mut self,
        frame: Frame,
        part: impl FnOnce(&Self) -> Result<T>,
    ) -> Result<T> {
        let saved = (self.reader, self.event_at, self.frame);
        (self.reader, self.event_at, self.frame) = (Reader::Query, EventAt::Matcher, frame);
        let compiled = part(self);
        (self.reader, self.event_at, self.frame) = saved;
        compiled
    }
    fn zone_change(
        &mut self,
        objects: &ObjectFilter,
        from: Option<&EventZone>,
        to: &EventZone,
    ) -> Result<Matched> {
        let frame = Frame {
            object: Some(Carried::Subject),
            destination: Some(Carried::Data(0)),
            ..Frame::default()
        };
        let entering = *to == EventZone::Battlefield;
        // CR 603.10: a departure is judged as the object last was, an arrival
        // as it now is, and a card moved between hidden or public zones as it
        // was where it left from.
        let id = if entering || from.is_none() { "*new" } else { "*old" };
        let mut tests = vec![format!("to.is({})", zone_kind(to)?)];
        if let Some(from) = from {
            tests.push(format!("from.is({})", zone_kind(from)?));
        }
        tests.push(self.matching(frame, |c| c.filter(objects, &Candidate::of("id")))?);
        let own = matches!(objects, ObjectFilter::SameObject(ObjectRef::Source));
        let functions = if own && !entering && from != Some(&EventZone::Battlefield) {
            Some(format!("ZoneKinds::of(&[{}])", zone_kind(to)?))
        } else {
            None
        };
        let phrase = match from {
            None if !entering => "Whenever this is put there from anywhere",
            None => "Whenever this enters",
            Some(EventZone::Battlefield) => "Whenever this goes there from the battlefield",
            Some(_) => "Whenever this moves there",
        };
        let mut matched = single(
            "ZONE_CHANGE",
            Arm {
                pattern: "Event::ZoneChange { old, new, from, to, .. }".into(),
                locals: object_fired(id, "new.0"),
                tests,
            },
            frame,
            phrase,
        );
        matched.functions = functions;
        Ok(matched)
    }
    fn object_event(&mut self, event: &ObjectEvent, objects: &ObjectFilter) -> Result<Matched> {
        let plain = Frame {
            object: Some(Carried::Subject),
            ..Frame::default()
        };
        let (mask, pattern, id, data, frame, extra, phrase): (
            &str,
            &str,
            &str,
            &str,
            Frame,
            Option<&str>,
            &str,
        ) = match event {
            ObjectEvent::LeavesBattlefield => (
                "ZONE_CHANGE",
                "Event::ZoneChange { old, new, from, .. }",
                "*old",
                "new.0",
                Frame {
                    destination: Some(Carried::Data(0)),
                    ..plain
                },
                Some("from.is(ZoneKind::Battlefield)"),
                "Whenever this leaves the battlefield",
            ),
            ObjectEvent::Attacks => (
                "ATTACKS",
                "Event::Attacks { attacker, defending, .. }",
                "*attacker",
                "u32::from(defending.0)",
                Frame {
                    other_player: Some(Carried::Data(0)),
                    ..plain
                },
                None,
                "Whenever this attacks",
            ),
            ObjectEvent::AttacksAlone => (
                "ATTACKS",
                "Event::Attacks { attacker, defending, .. }",
                "*attacker",
                "u32::from(defending.0)",
                Frame {
                    other_player: Some(Carried::Data(0)),
                    ..plain
                },
                Some("q.state().attackers.len() == 1"),
                "Whenever this attacks alone",
            ),
            ObjectEvent::Blocks => (
                "BLOCKS",
                "Event::Blocks { blocker, attacker, declared: true }",
                "*blocker",
                "attacker.0",
                Frame {
                    other: Some(Carried::Data(0)),
                    ..plain
                },
                None,
                "Whenever this blocks",
            ),
            ObjectEvent::AttacksAndIsNotBlocked => (
                "UNBLOCKED",
                "Event::Unblocked { attacker, .. }",
                "*attacker",
                "u32::MAX",
                plain,
                None,
                "Whenever this attacks and isn't blocked",
            ),
            ObjectEvent::BecomesTapped => (
                "TAPPED",
                "Event::Tapped { permanent }",
                "*permanent",
                "u32::MAX",
                plain,
                None,
                "Whenever this becomes tapped",
            ),
            ObjectEvent::TurnsFaceUp => (
                "TURNED_FACE_UP",
                "Event::TurnedFaceUp { permanent, .. }",
                "*permanent",
                "u32::MAX",
                plain,
                None,
                "Whenever this is turned face up",
            ),
            // The engine records no event for a permanent untapping.
            ObjectEvent::BecomesUntapped => return Err(gap("engine: no untap event")),
            other => return Err(gap(format!("trigger event Object {}", form(other)))),
        };
        let mut tests: Vec<String> = extra.map(Into::into).into_iter().collect();
        tests.push(self.matching(frame, |c| c.filter(objects, &Candidate::of("id")))?);
        Ok(single(
            mask,
            Arm {
                pattern: pattern.into(),
                locals: object_fired(id, data),
                tests,
            },
            frame,
            phrase,
        ))
    }
    /// What damage binds and what it must pass, read from the locals `source`,
    /// `target` (a reference), `amount` and `combat`; replacement effects and
    /// triggers see the same damage differently, but match it the same way.
    pub(super) fn damage_match(
        &mut self,
        damage: &DamageEvent,
    ) -> Result<(Frame, String, Vec<String>)> {
        let DamageEvent {
            source_kind,
            sources,
            recipients,
            kind,
            grouping,
            subject,
        } = damage;
        let by_source = *subject == DamageSubject::Source;
        let to_players = matches!(recipients, DamageRecipients::Players(_));
        let to_objects = matches!(recipients, DamageRecipients::Objects(_));
        // What `data[1]` holds: a batch key where grouped, and whichever role
        // the subject is not, when the recipients say what kind of thing it is.
        let recipient_code = if to_players {
            "match target { Target::Player(p) => u32::from(p.0), _ => u32::MAX }"
        } else if to_objects {
            "match target { Target::Object(o) => o.0, _ => u32::MAX }"
        } else {
            "match target { Target::Object(o) => o.0, Target::Player(p) => u32::MAX - 1 - u32::from(p.0), _ => u32::MAX }"
        };
        let keyed_by_recipient = matches!(grouping, DamageGrouping::Recipient);
        let second = match (by_source, grouping) {
            (false, DamageGrouping::Recipient) => recipient_code,
            (false, _) => "source.0",
            (true, DamageGrouping::Source) => "source.0",
            (true, _) => recipient_code,
        };
        let singular = matches!(grouping, DamageGrouping::SourceRecipient);
        let carried_second = Some(Carried::Data(1));
        let mut frame = Frame {
            amount: singular.then_some(Carried::Data(0)),
            ..Frame::default()
        };
        if by_source {
            frame.object = Some(Carried::Subject);
            frame.damage_source = Some(Carried::Subject);
            if !matches!(grouping, DamageGrouping::Source) {
                if to_players {
                    frame.player = carried_second;
                } else if to_objects {
                    frame.damage_recipient = carried_second;
                }
            }
        } else {
            frame.object = Some(Carried::Subject);
            frame.player = Some(Carried::Subject);
            frame.damage_recipient = Some(Carried::Subject);
            if !keyed_by_recipient {
                frame.damage_source = carried_second;
            }
        }
        let mut tests = Vec::new();
        match kind {
            DamageKind::Any => {}
            DamageKind::Combat => tests.push("combat".to_string()),
            DamageKind::Noncombat => tests.push("!combat".to_string()),
            other => return Err(gap(format!("damage kind {}", form(other)))),
        }
        tests.push(match source_kind {
            ObjectKind::Any => "true".to_string(),
            ObjectKind::Permanent => "!q.state().obj(source).zone.is(ZoneKind::Stack)".to_string(),
            other => return Err(gap(format!("damage source kind {}", form(other)))),
        });
        tests.push(self.matching(frame, |c| c.filter(sources, &Candidate::of("source")))?);
        tests.push(self.matching(frame, |c| match recipients {
            DamageRecipients::Any => Ok("true".to_string()),
            DamageRecipients::Players(players) => Ok(format!(
                "matches!(*target, Target::Player(p) if {})",
                c.player_in(players, "p")?
            )),
            DamageRecipients::Objects(filter) => Ok(format!(
                "matches!(*target, Target::Object(o) if {})",
                c.filter(filter, &Candidate::of("o"))?
            )),
            DamageRecipients::ObjectsAndPlayers { objects, players } => Ok(format!(
                "match *target {{ Target::Object(o) => {}, Target::Player(p) => {}, _ => false }}",
                c.filter(objects, &Candidate::of("o"))?,
                c.player_in(players, "p")?
            )),
            other => Err(gap(format!("damage recipients {}", form(other)))),
        })?);
        let subject = if by_source {
            "Target::Object(source)"
        } else {
            "*target"
        };
        let locals = format!("let subject = {subject}; let data = [u32::from(amount), {second}];");
        Ok((frame, locals, tests))
    }
    fn damage_event(&mut self, damage: &DamageEvent) -> Result<Matched> {
        let (frame, locals, tests) = self.damage_match(damage)?;
        let mut matched = single(
            "DAMAGE",
            Arm {
                pattern: "Event::Damage { source, target, amount, kind, .. }".into(),
                locals: format!(
                    "let (source, amount, combat) = (*source, *amount, kind.is_combat()); {locals}"
                ),
                tests,
            },
            frame,
            "Whenever damage is dealt",
        );
        matched.batch = match damage.grouping {
            DamageGrouping::SourceRecipient => None,
            DamageGrouping::Source | DamageGrouping::Recipient => Some(true),
            DamageGrouping::Batch => Some(false),
        };
        Ok(matched)
    }
    pub(super) fn event(&mut self, event: &EventSpec) -> Result<Matched> {
        match event {
            EventSpec::ZoneChange { objects, from, to } => {
                self.zone_change(objects, from.as_ref(), to)
            }
            EventSpec::Object { event, objects } => self.object_event(event, objects),
            EventSpec::Spell {
                event,
                players,
                spells,
            } => {
                let frame = Frame {
                    object: Some(Carried::Subject),
                    player: Some(Carried::Data(0)),
                    ..Frame::default()
                };
                let test = self.matching(frame, |c| {
                    Ok(vec![
                        c.player_in(players, "*player")?,
                        c.filter(spells, &Candidate::of("id"))?,
                    ])
                })?;
                let (mask, pattern, id) = match event {
                    SpellEvent::Cast => (
                        "SPELL_CAST",
                        "Event::SpellCast { spell, player, .. }",
                        "*spell",
                    ),
                    SpellEvent::Copy => (
                        "COPIED",
                        "Event::Copied { copy, player, .. }",
                        "*copy",
                    ),
                };
                Ok(single(
                    mask,
                    Arm {
                        pattern: pattern.into(),
                        locals: object_fired(id, "u32::from(player.0)"),
                        tests: test,
                    },
                    frame,
                    "Whenever a spell is cast",
                ))
            }
            EventSpec::Turn { point, players } => {
                let steps: &[&str] = match point {
                    TurnPoint::UpkeepBegins => &["Upkeep"],
                    TurnPoint::DrawStepBegins => &["Draw"],
                    TurnPoint::MainPhaseBegins => &["PrecombatMain", "PostcombatMain"],
                    TurnPoint::FirstMainPhaseBegins => &["PrecombatMain"],
                    TurnPoint::PostcombatMainPhaseBegins => &["PostcombatMain"],
                    TurnPoint::CombatBegins => &["BeginCombat"],
                    TurnPoint::CombatEnds => &["EndOfCombat"],
                    TurnPoint::EndStepBegins => &["End"],
                    TurnPoint::CleanupStepBegins => &["Cleanup"],
                    TurnPoint::DeclareAttackersStepBegins => &["DeclareAttackers"],
                    other => return Err(gap(format!("turn point {}", form(other)))),
                };
                let frame = Frame {
                    player: Some(Carried::Subject),
                    ..Frame::default()
                };
                let players =
                    self.matching(frame, |c| c.player_in(players, "q.state().turn.active"))?;
                let steps = steps
                    .iter()
                    .map(|s| format!("Step::{s}"))
                    .collect::<Vec<_>>()
                    .join(" | ");
                Ok(single(
                    "STEP_BEGAN",
                    Arm {
                        pattern: "Event::StepBegan { step }".into(),
                        locals: "let subject = Target::Player(q.state().turn.active); let data = [0u32; 2];".into(),
                        tests: vec![format!("matches!(*step, {steps})"), players],
                    },
                    frame,
                    "At the beginning of this step",
                ))
            }
            EventSpec::Damage(damage) => self.damage_event(damage),
            EventSpec::Discard {
                players,
                cards,
                source_only: false,
                kind,
                grouping: DiscardGrouping::Card,
            } => {
                let frame = Frame {
                    object: Some(Carried::Subject),
                    player: Some(Carried::Data(0)),
                    ..Frame::default()
                };
                let mut tests = self.matching(frame, |c| {
                    Ok(vec![
                        c.player_in(players, "*player")?,
                        c.filter(cards, &Candidate::of("id"))?,
                    ])
                })?;
                let (mask, pattern) = match kind {
                    DiscardKind::Any => {
                        tests.push("!*undefined".into());
                        ("DISCARDED", "Event::Discarded { player, card, undefined }")
                    }
                    DiscardKind::Cycling => ("CYCLED", "Event::Cycled { player, card }"),
                };
                Ok(single(
                    mask,
                    Arm {
                        pattern: pattern.into(),
                        locals: object_fired("*card", "u32::from(player.0)"),
                        tests,
                    },
                    frame,
                    "Whenever a card is discarded",
                ))
            }
            EventSpec::Either(events) => {
                let mut merged: Option<Matched> = None;
                for event in events {
                    let next = self.event(event)?;
                    merged = Some(match merged {
                        None => next,
                        Some(mut all) => {
                            if all.functions != next.functions || all.batch != next.batch {
                                return Err(gap("alternative events with different rules"));
                            }
                            all.frame = all.frame.meet(next.frame);
                            for mask in next.masks {
                                if !all.masks.contains(&mask) {
                                    all.masks.push(mask);
                                }
                            }
                            all.arms.extend(next.arms);
                            all
                        }
                    });
                }
                merged.ok_or_else(|| gap("empty alternative event"))
            }
            EventSpec::Filtered { event, condition } => {
                let mut matched = self.event(event)?;
                let test = self.matching(matched.frame, |c| c.condition(condition))?;
                for arm in &mut matched.arms {
                    arm.tests.push(test.clone());
                }
                Ok(matched)
            }
            EventSpec::Primitive { name, .. } => {
                Err(gap(format!("primitive trigger event {name}")))
            }
            other => Err(gap(format!("trigger event {}", form(other)))),
        }
    }
    /// The `Trigger`, with the matcher named `label`.
    pub(super) fn trigger(&mut self, matched: &Matched, label: &str, extra: &[String]) -> String {
        let mut body = String::new();
        for arm in &matched.arms {
            let tests: Vec<&str> = arm
                .tests
                .iter()
                .chain(extra)
                .map(String::as_str)
                .collect();
            writeln!(
                body,
                "if let {} = event {{ {} if {} {{ return Some(Fired {{ subject, data }}); }} }}",
                arm.pattern,
                arm.locals,
                tests.join(" && ")
            )
            .unwrap();
        }
        writeln!(self.functions,"#[allow(unused_variables)] fn {label}(q: &dyn Query, asker: &Asker, event: &Event) -> Option<Fired> {{ {body} None }}").unwrap();
        let mask = matched
            .masks
            .iter()
            .map(|m| format!("EventMask::{m}.0"))
            .collect::<Vec<_>>()
            .join(" | ");
        let trigger = format!("when_asked(EventMask({mask}), {label})");
        match &matched.functions {
            Some(zones) => format!("{trigger}.functions_in({zones})"),
            None => trigger,
        }
    }
    /// A callback the engine asks with no event in reach.
    fn condition_function(&mut self, condition: &Condition, label: &str) -> Result<String> {
        let saved = (self.reader, self.event_at);
        (self.reader, self.event_at) = (Reader::Query, EventAt::Nowhere);
        let compiled = self.condition(condition);
        (self.reader, self.event_at) = saved;
        writeln!(
            self.functions,
            "fn {label}(q: &dyn Query, asker: &Asker) -> bool {{ {} }}",
            compiled?
        )
        .unwrap();
        Ok(label.into())
    }
    pub(super) fn activation_condition(
        &mut self,
        condition: &Condition,
        label: &str,
    ) -> Result<String> {
        self.condition_function(condition, label)
    }
    pub(super) fn triggered(
        &mut self,
        label: &str,
        event: &EventSpec,
        intervening_if: Option<&Condition>,
        limits: &TriggerLimits,
        announcement: &Announcement,
        body: &Program,
    ) -> Result<String> {
        if limits.total.is_some() {
            return Err(gap("trigger total limit"));
        }
        if let EventSpec::State { condition } = event {
            let state = self.condition_function(condition, &format!("{label}_state"))?;
            let mut def = {
                self.targets_for(announcement, label, Frame::default())?;
                let effect = self.effect(body, label)?;
                format!("state_triggered({state}, {effect}, \"When this state holds\")")
            };
            if let Some(per_turn) = limits.per_turn {
                def.push_str(&format!(".triggers_each_turn({per_turn})"));
            }
            return Ok(def);
        }
        let matched = self.event(event)?;
        if announcement.modes.is_some() {
            return self.modal_triggered(label, &matched, intervening_if, limits, announcement, body);
        }
        let targets = self.targets_for(announcement, label, matched.frame)?;
        self.frame = matched.frame;
        // CR 603.4 asks an intervening "if" twice. The engine's `condition` has
        // no event to read, so one that reads the event is asked by the matcher
        // and by the resolving effect instead.
        let mut extra = Vec::new();
        let mut guard = None;
        let mut condition = None;
        if let Some(intervening) = intervening_if {
            match self.condition_function(intervening, &format!("{label}_if")) {
                Ok(function) => condition = Some(function),
                Err(gap) if gap.feature.contains("event") => {
                    extra.push(self.matching(matched.frame, |c| c.condition(intervening))?);
                    guard = Some(intervening);
                }
                Err(gap) => {
                    self.frame = Frame::default();
                    return Err(gap);
                }
            }
        }
        let trigger = self.trigger(&matched, &format!("{label}_event"), &extra);
        self.grouped = matched.batch.is_some();
        let effect = match guard {
            Some(condition) => self.guarded_effect(condition, body, label),
            None => self.effect(body, label),
        };
        self.grouped = false;
        self.frame = Frame::default();
        let effect = effect?;
        let text = format!("{}, {}", matched.phrase, ability_text(body));
        let mut def = format!(
            "triggered({trigger}, {effect}, {}).targets({targets})",
            quoted(&text)
        );
        if let Some(condition) = condition {
            def.push_str(&format!(".condition({condition})"));
        }
        match matched.batch {
            Some(true) => def.push_str(".once_per_batch().batch_by_data()"),
            Some(false) => def.push_str(".once_per_batch()"),
            None => {}
        }
        if let Some(per_turn) = limits.per_turn {
            def.push_str(&format!(".triggers_each_turn({per_turn})"));
        }
        Ok(def)
    }
    fn modal_triggered(
        &mut self,
        label: &str,
        matched: &Matched,
        intervening_if: Option<&Condition>,
        limits: &TriggerLimits,
        announcement: &Announcement,
        body: &Program,
    ) -> Result<String> {
        if !body.instructions.is_empty() {
            return Err(gap("modal ability with instructions besides its modes"));
        }
        let condition = intervening_if
            .map(|c| self.condition_function(c, &format!("{label}_if")))
            .transpose()?;
        let trigger = self.trigger(matched, &format!("{label}_event"), &[]);
        self.frame = matched.frame;
        self.grouped = matched.batch.is_some();
        let modal = self.modal(announcement, label, Some(matched.frame));
        self.grouped = false;
        self.frame = Frame::default();
        let modal = modal?;
        let mut def = format!("modal_triggered({trigger}, &{modal}, {})", quoted(&matched.phrase));
        if let Some(condition) = condition {
            def.push_str(&format!(".condition({condition})"));
        }
        match matched.batch {
            Some(true) => def.push_str(".once_per_batch().batch_by_data()"),
            Some(false) => def.push_str(".once_per_batch()"),
            None => {}
        }
        if let Some(per_turn) = limits.per_turn {
            def.push_str(&format!(".triggers_each_turn({per_turn})"));
        }
        Ok(def)
    }
    pub(super) fn targets_for(
        &mut self,
        announcement: &Announcement,
        label: &str,
        frame: Frame,
    ) -> Result<String> {
        let saved = (self.frame, self.event_at);
        (self.frame, self.event_at) = (frame, EventAt::TargetFilter);
        let targets = self.announcement(announcement, label);
        (self.frame, self.event_at) = saved;
        targets
    }
}
