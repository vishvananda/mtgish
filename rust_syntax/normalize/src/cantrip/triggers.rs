//! Trigger events as generated matchers. A matcher binds what the event says
//! into `Fired`, whose subject and data reach the resolving ability; `Frame` is
//! the map of where each value went. The ability's text matters to the engine
//! only for `read_zone_words`, so `phrase` states exactly what those words read.
use super::*;

/// One alternative of an event: an `if let` over the event, then `locals`, which
/// must bind `subject` and `data`, then every test the event has to pass.
pub(super) struct Arm {
    pub pattern: String,
    pub locals: String,
    pub tests: Vec<String>,
}

pub(super) struct Matched {
    masks: Vec<&'static str>,
    pub arms: Vec<Arm>,
    pub frame: Frame,
    /// The trigger's zones, when it cannot trigger from the battlefield.
    functions: Option<String>,
    /// "One or more": a batch triggers it once, split by `data[1]` when keyed.
    pub batch: Option<bool>,
    /// A batch's events about one subject are one event of their total.
    pub sums: bool,
    /// Tapping for mana, which the engine shows only to mana abilities.
    pub mana: bool,
    pub phrase: String,
    /// What Cantrip must add before this event triggers anything: see `lane_g`.
    pub planned: Vec<String>,
}

/// Whether simultaneous damage is one event of its total to this subject
/// (CR 120.2): all a source dealt, or all that was dealt to a recipient.
fn sums_damage(damage: &DamageEvent) -> bool {
    // Excess is repeated on each source's record: it is not to be added up.
    if matches!(damage.kind, DamageKind::Excess | DamageKind::ExcessNoncombat) {
        return false;
    }
    match (&damage.grouping, damage.subject == DamageSubject::Source) {
        (DamageGrouping::Source, true) | (DamageGrouping::Recipient, false) => true,
        _ => false,
    }
}

/// The event a damage trigger hears.
fn damage_pattern(damage: &DamageEvent) -> &'static str {
    match damage.kind {
        DamageKind::Prevented => "Event::DamagePrevented { source, target, amount, combat, .. }",
        DamageKind::Excess | DamageKind::ExcessNoncombat => {
            "Event::Damage { source, target, recipient_excess, kind, .. }"
        }
        _ => "Event::Damage { source, target, amount, kind, .. }",
    }
}
/// `source`, `amount` and `combat` of that event. An excess amount reads as the
/// excess; so does a prevented one what was stopped.
fn damage_values(damage: &DamageEvent) -> &'static str {
    match damage.kind {
        DamageKind::Prevented => "let (source, amount, combat) = (*source, *amount, *combat);",
        DamageKind::Excess | DamageKind::ExcessNoncombat => {
            "let (source, amount, combat) = (*source, (*recipient_excess).min(u32::from(u16::MAX)) as u16, kind.is_combat());"
        }
        _ => "let (source, amount, combat) = (*source, *amount, kind.is_combat());",
    }
}

pub(super) fn object_fired(id: &str, destination: &str) -> String {
    format!(
        "let id = {id}; let subject = Target::Object(id); let data = [{destination}, u32::MAX];"
    )
}
pub(super) fn single(mask: &'static str, arm: Arm, frame: Frame, phrase: &str) -> Matched {
    Matched {
        masks: vec![mask],
        arms: vec![arm],
        frame,
        functions: None,
        batch: None,
        sums: false,
        mana: false,
        phrase: phrase.into(),
        planned: Vec::new(),
    }
}

impl Matched {
    /// A closure over one history record, as the matcher would answer it.
    /// Whether `event` is this one, for a rule asked of an event.
    pub(super) fn fired_test(&self) -> String {
        let mut body = String::new();
        for arm in &self.arms {
            write!(
                body,
                "if let {} = event {{ {} if {} {{ return true; }} }} ",
                arm.pattern,
                arm.locals,
                arm.tests.join(" && ")
            )
            .unwrap();
        }
        format!("(|| -> bool {{ {body} false }})()").replace("cast_of(q, ", "cast_at(q, event, ")
    }
    pub(super) fn history_test(&self) -> String {
        let mut body = String::new();
        for arm in &self.arms {
            write!(
                body,
                "if let {} = event {{ {} if {} {{ return Some(Fired {{ subject, data }}); }} }} ",
                arm.pattern,
                arm.locals,
                arm.tests.join(" && ")
            )
            .unwrap();
        }
        // A record says whose turn it was and what caused it, whatever is so now.
        let body = body
            .replace("q.state().turn.active", "r.active")
            .replace("q.event_cause()", "r.cause");
        format!("|r: &Record| -> Option<Fired> {{ let event = &r.event; {body} None }}")
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
        // A question a layer asks of the history keeps to what a layer can read.
        let reader = match (self.reader, &self.history) {
            (Reader::Layer, Some(_)) => Reader::Layer,
            // Asked inside a resolving effect, where the targets are in reach.
            (Reader::Resolution, Some(_)) if self.stack_item => Reader::Resolution,
            _ => Reader::Query,
        };
        let (event_at, frame) = match &self.history {
            Some(history) => (history.event_at, history.frame),
            None => (EventAt::Matcher, frame),
        };
        (self.reader, self.event_at, self.frame) = (reader, event_at, frame);
        let compiled = part(self);
        (self.reader, self.event_at, self.frame) = saved;
        compiled
    }
    pub(super) fn zone_change(
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
        let id = if entering || from.is_none() {
            "*new"
        } else {
            "*old"
        };
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
        // Once however many block it, so the blockers are no one subject.
        if let ObjectEvent::BecomesBlocked = event {
            let test = self.matching(plain, |c| c.filter(objects, &Candidate::of("id")))?;
            let mut matched = single(
                "BLOCKS",
                Arm {
                    pattern: "Event::Blocks { attacker, .. }".into(),
                    locals: "let id = *attacker; let subject = Target::Object(id); let data = [u32::MAX, id.0];".into(),
                    tests: vec![test],
                },
                plain,
                "Whenever this becomes blocked",
            );
            matched.batch = Some(true);
            return Ok(matched);
        }
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
            ObjectEvent::Transforms => (
                "TRANSFORMED",
                "Event::Transformed { object }",
                "*object",
                "u32::MAX",
                plain,
                None,
                "Whenever this transforms",
            ),
            ObjectEvent::BecomesMonstrous => (
                "MONSTROUS",
                "Event::Monstrous { permanent }",
                "*permanent",
                "u32::MAX",
                plain,
                None,
                "Whenever this becomes monstrous",
            ),
            ObjectEvent::TappedForMana => (
                "TAPPED",
                "Event::TappedForMana { permanent, player, produced }",
                "*permanent",
                "u32::from(player.0)",
                plain,
                None,
                "Whenever this is tapped for mana",
            ),
            ObjectEvent::BecomesUntapped => (
                "UNTAPPED",
                "Event::Untapped { permanent }",
                "*permanent",
                "u32::MAX",
                plain,
                None,
                "Whenever this becomes untapped",
            ),
            ObjectEvent::Mutates => (
                "MUTATED",
                "Event::Mutated { permanent }",
                "*permanent",
                "u32::MAX",
                plain,
                None,
                "Whenever this mutates",
            ),
            ObjectEvent::Specializes => (
                "SPECIALIZED",
                "Event::Specialized { permanent }",
                "*permanent",
                "u32::MAX",
                plain,
                None,
                "Whenever this specializes",
            ),
            other => return self.object_event_rest(other, objects),
        };
        let mut tests: Vec<String> = extra.map(Into::into).into_iter().collect();
        tests.push(self.matching(frame, |c| c.filter(objects, &Candidate::of("id")))?);
        let mut matched = single(
            mask,
            Arm {
                pattern: pattern.into(),
                locals: if matches!(event, ObjectEvent::TappedForMana) {
                    lane_g::TAPPED_FOR_MANA.to_string()
                } else {
                    object_fired(id, data)
                },
                tests,
            },
            frame,
            phrase,
        );
        matched.mana = matches!(event, ObjectEvent::TappedForMana);
        let planned = match event {
            ObjectEvent::BecomesUntapped => Some("engine: no untap event"),
            ObjectEvent::Mutates => Some("engine: no mutate event"),
            ObjectEvent::Specializes => Some("engine: no specialize event"),
            _ => None,
        };
        if let Some(feature) = planned {
            self.plan_event(&mut matched, feature, false)?;
        }
        Ok(matched)
    }
    /// `EventSpec::Player`. The arms name the player `player`, by reference or
    /// not as `who` says, and whatever else was done to `id`.
    fn player_event(
        &mut self,
        event: &PlayerEvent,
        players: &PlayerSet,
        objects: Option<&ObjectFilter>,
    ) -> Result<Matched> {
        const ONE: &str = "let subject = Target::Player(*player); let data = [0u32; 2];";
        let about = Frame {
            player: Some(Carried::Subject),
            ..Frame::default()
        };
        let life = Frame {
            amount: Some(Carried::Data(0)),
            ..about
        };
        let acted = Frame {
            object: Some(Carried::Subject),
            player: Some(Carried::Data(0)),
            ..Frame::default()
        };
        let (mask, pattern, locals, who, frame, test, phrase) = match event {
            PlayerEvent::GainsLife | PlayerEvent::LosesLife => (
                "LIFE_CHANGE",
                "Event::LifeChange { player, delta }",
                "let subject = Target::Player(*player); let data = [delta.unsigned_abs(), 0];",
                "*player",
                life,
                Some(match event {
                    PlayerEvent::GainsLife => "*delta > 0",
                    _ => "*delta < 0",
                }),
                "Whenever a player's life total changes",
            ),
            PlayerEvent::Draws => (
                "DREW",
                "Event::Drew { player, .. }",
                ONE,
                "*player",
                about,
                None,
                "Whenever a player draws a card",
            ),
            PlayerEvent::Attacks => (
                "ATTACKERS_DECLARED",
                "Event::AttackersDeclared { player, count }",
                ONE,
                "*player",
                about,
                Some("*count > 0"),
                "Whenever a player attacks",
            ),
            PlayerEvent::Scries => (
                "SCRIED",
                "Event::Scried { player, looked, .. }",
                "let subject = Target::Player(*player); let data = [*looked as u32, 0];",
                "*player",
                life,
                None,
                "Whenever a player scries",
            ),
            PlayerEvent::Surveils => (
                "SURVEILLED",
                "Event::Surveilled { player, .. }",
                ONE,
                "*player",
                about,
                None,
                "Whenever a player surveils",
            ),
            PlayerEvent::CommitsCrime => (
                "CRIME",
                "Event::CrimeCommitted { player, .. }",
                ONE,
                "*player",
                about,
                None,
                "Whenever a player commits a crime",
            ),
            PlayerEvent::SearchesLibrary => (
                "SEARCHED",
                "Event::Searched { player, library }",
                ONE,
                "*player",
                about,
                Some("player == library"),
                "Whenever a player searches their library",
            ),
            PlayerEvent::WinsCoinFlip => (
                "COIN_FLIPPED",
                "Event::CoinFlipped { player, result: CoinFlip::Won }",
                ONE,
                "*player",
                about,
                None,
                "Whenever a player wins a coin flip",
            ),
            PlayerEvent::Sacrifices => (
                "ZONE_CHANGE",
                "Event::ZoneChange { old, new, from, reason: ZoneReason::Sacrifice, .. }",
                "let id = *old; let player = q.controller(id); let subject = Target::Object(id); let data = [new.0, u32::from(player.0)];",
                "player",
                Frame {
                    object: Some(Carried::Subject),
                    destination: Some(Carried::Data(0)),
                    player: Some(Carried::Data(1)),
                    ..Frame::default()
                },
                Some("from.is(ZoneKind::Battlefield)"),
                "Whenever a player sacrifices a permanent",
            ),
            // "When you unlock this door": which door is its place on the Room.
            PlayerEvent::UnlocksDoor if self.door.is_some() => (
                "DOOR_UNLOCKED",
                "Event::DoorUnlocked { permanent, door, player }",
                "let id = *permanent; let subject = Target::Object(id); let data = [u32::from(player.0), 0];",
                "*player",
                acted,
                self.door.map(lane_f::Door::unlocked),
                "When you unlock this door",
            ),
            PlayerEvent::PlaysLand => (
                "ZONE_CHANGE",
                "Event::ZoneChange { new, to, reason: ZoneReason::PlayLand, .. }",
                "let id = *new; let player = q.controller(id); let subject = Target::Object(id); let data = [u32::from(player.0), 0];",
                "player",
                acted,
                Some("to.is(ZoneKind::Battlefield)"),
                "Whenever a player plays a land",
            ),
            PlayerEvent::TapsForMana => (
                "TAPPED",
                "Event::TappedForMana { permanent, player, produced }",
                lane_g::TAPPED_FOR_MANA,
                "*player",
                acted,
                None,
                "Whenever a player taps a permanent for mana",
            ),
            // `Event::GameLost` is filed in the history only.
            PlayerEvent::LosesGame => (
                "GAME_LOST",
                "Event::GameLost { player, .. }",
                ONE,
                "*player",
                about,
                None,
                "Whenever a player loses the game",
            ),
            // Each die is an event of its own, not "one or more dice" at once.
            PlayerEvent::RollsDice => (
                "DICE_ROLLED",
                "Event::DiceRolled { player, .. }",
                ONE,
                "*player",
                about,
                None,
                "Whenever a player rolls one or more dice",
            ),
            other => return self.player_event_rest(other, players, objects),
        };
        let mut tests: Vec<String> = test.map(Into::into).into_iter().collect();
        tests.push(self.matching(frame, |c| c.player_in(players, who))?);
        match objects {
            Some(objects) if frame.object.is_some() => {
                tests.push(self.matching(frame, |c| c.filter(objects, &Candidate::of("id")))?)
            }
            None if frame.object.is_none() => {}
            _ => return Err(gap(format!("trigger event Player {} objects", form(event)))),
        }
        let mut matched = single(
            mask,
            Arm {
                pattern: pattern.into(),
                locals: locals.into(),
                tests,
            },
            frame,
            phrase,
        );
        matched.mana = matches!(event, PlayerEvent::TapsForMana);
        match event {
            PlayerEvent::LosesGame => {
                let feature = "engine: no ability triggers on losing the game";
                self.plan_event(&mut matched, feature, true)?
            }
            PlayerEvent::RollsDice => {
                let feature = "engine: no event for dice rolled together";
                self.plan_event(&mut matched, feature, false)?
            }
            _ => {}
        }
        Ok(matched)
    }
    /// `EventSpec::Pair`: `id` is the object the event is about, `other` the
    /// second one it binds.
    fn pair_event(
        &mut self,
        event: &PairEvent,
        objects: &ObjectFilter,
        others: &ObjectFilter,
    ) -> Result<Matched> {
        let about = Frame {
            object: Some(Carried::Subject),
            other: Some(Carried::Data(0)),
            ..Frame::default()
        };
        let (mask, pattern, id, other, frame, phrase) = match event {
            PairEvent::Blocks => (
                "BLOCKS",
                "Event::Blocks { blocker, attacker, .. }",
                "*blocker",
                "*attacker",
                Frame {
                    blocker: Some(Carried::Subject),
                    attacker: Some(Carried::Data(0)),
                    ..about
                },
                "Whenever a creature blocks a creature",
            ),
            PairEvent::BecomesBlockedBy => (
                "BLOCKS",
                "Event::Blocks { blocker, attacker, .. }",
                "*attacker",
                "*blocker",
                Frame {
                    attacker: Some(Carried::Subject),
                    blocker: Some(Carried::Data(0)),
                    ..about
                },
                "Whenever a creature becomes blocked by a creature",
            ),
            PairEvent::BecomesAttachedTo => (
                "ATTACHED",
                "Event::Attached { attachment, to: Target::Object(host) }",
                "*attachment",
                "*host",
                about,
                "Whenever this becomes attached",
            ),
            PairEvent::Crews => (
                "CREWED",
                "Event::Crewed { creature, vehicle }",
                "*creature",
                "*vehicle",
                about,
                "Whenever this crews a Vehicle",
            ),
            PairEvent::Saddles => (
                "SADDLED",
                "Event::Saddled { creature, mount }",
                "*creature",
                "*mount",
                about,
                "Whenever this saddles a Mount",
            ),
            PairEvent::Exploits => (
                "EXPLOITED",
                "Event::Exploited { creature, sacrificed }",
                "*creature",
                "*sacrificed",
                about,
                "Whenever this exploits a creature",
            ),
            other => return self.pair_event_rest(other, objects, others),
        };
        let tests = vec![
            self.matching(frame, |c| c.filter(objects, &Candidate::of("id")))?,
            self.matching(frame, |c| c.filter(others, &Candidate::of("other")))?,
        ];
        let mut matched = single(
            mask,
            Arm {
                pattern: pattern.into(),
                locals: format!("let id = {id}; let other = {other}; let subject = Target::Object(id); let data = [other.0, u32::MAX];"),
                tests,
            },
            frame,
            phrase,
        );
        match event {
            // The history has it, for what asks who crewed; no ability sees it.
            PairEvent::Crews => self.plan_event(&mut matched, "engine: no crew event", true)?,
            PairEvent::Saddles => {
                self.plan_event(&mut matched, "engine: no saddle event", false)?
            }
            PairEvent::Exploits => {
                self.plan_event(&mut matched, "engine: no exploit event", false)?
            }
            _ => {}
        }
        Ok(matched)
    }
    /// `EventSpec::BecomesTarget`. The engine names the object on the stack by
    /// `by`, which for an ability is its source, so only a spell is that object.
    fn becomes_target(
        &mut self,
        objects: &ObjectFilter,
        kind: &TargetingKind,
        by: &Operand,
    ) -> Result<Matched> {
        const IS_SPELL: &str =
            "q.state().stack.iter().any(|i| i.kind == StackKind::Spell && i.object == by)";
        let frame = Frame {
            object: Some(Carried::Subject),
            stack_object: matches!(kind, TargetingKind::Spell).then_some(Carried::Data(0)),
            stack_controller: Some(Carried::Data(1)),
            ..Frame::default()
        };
        let mut tests = vec![self.matching(frame, |c| c.filter(objects, &Candidate::of("id")))?];
        match kind {
            TargetingKind::SpellOrAbility => {}
            _ if self.history.is_some() => {
                return Err(gap("what targeted an object, in a history question"))
            }
            TargetingKind::Spell => tests.push(IS_SPELL.into()),
            TargetingKind::Ability => tests.push(format!("!{IS_SPELL}")),
        }
        match by {
            Operand::Filter(filter) => {
                tests.push(self.matching(frame, |c| c.filter(filter, &Candidate::of("by")))?)
            }
            Operand::Term { form, operands, .. } => match (form.as_str(), operands.as_slice()) {
                ("AnySpellOrAbility", []) => {}
                ("ControlledByAPlayer", [Operand::Players(players)]) => {
                    tests.push(self.matching(frame, |c| c.player_in(players, "controller"))?)
                }
                (other, _) => return Err(gap(format!("targeted by {other}"))),
            },
            other => return Err(gap(format!("targeted by {}", form(other)))),
        }
        Ok(single(
            "BECAME_TARGET",
            Arm {
                pattern: "Event::BecameTarget { target: Target::Object(target), by, controller, .. }".into(),
                locals: "let id = *target; let by = *by; let controller = *controller; let subject = Target::Object(id); let data = [by.0, u32::from(controller.0) | (q.state().stack.len().saturating_sub(1) as u32) << 8];".into(),
                tests,
            },
            frame,
            "Whenever this becomes a target",
        ))
    }
    /// Whether the `Target` `defender` is one of `defenders`.
    pub(super) fn attacked(&self, defenders: &AttackDefenders) -> Result<String> {
        Ok(match defenders {
            AttackDefenders::Any => "true".into(),
            AttackDefenders::Objects(filter) => format!(
                "match *defender {{ Target::Object(o) => {}, _ => false }}",
                self.filter(filter, &Candidate::of("o"))?
            ),
            AttackDefenders::Players(players) => format!(
                "match *defender {{ Target::Player(p) => {}, _ => false }}",
                self.player_in(players, "p")?
            ),
            AttackDefenders::PlayersAndPlaneswalkers(players) => format!(
                "match *defender {{ Target::Player(p) => {}, Target::Object(o) => q.chars(o).is(CardType::Planeswalker) && {}, _ => false }}",
                self.player_in(players, "p")?,
                self.player_in(players, "q.controller(o)")?
            ),
        })
    }
    /// `EventSpec::Activated`: a mana ability's activation is one too (605.3).
    /// The player is "that player" and the ability's object "that permanent".
    fn activated_event(
        &mut self,
        players: &PlayerSet,
        abilities: &ObjectFilter,
    ) -> Result<Matched> {
        let frame = Frame {
            player: Some(Carried::Subject),
            object: Some(Carried::Data(0)),
            activated: Some(Carried::Data(1)),
            ..Frame::default()
        };
        let tests = vec![
            "def.is_some()".to_string(),
            self.matching(frame, |c| c.player_in(players, "*player"))?,
            self.activation_test(abilities, frame)?,
        ];
        Ok(single(
            "ABILITY_ACTIVATED",
            Arm {
                pattern: "Event::AbilityActivated { source, ability, player, .. }".into(),
                locals: "let id = *source; let def = q.activated_ability(*ability); let subject = Target::Player(*player); let serial = match cantrip_mechanics::triggers::activated_ability_on_stack(q, event) { Some(Target::Ability(serial)) => serial, _ => 0 }; let data = [id.0, serial];".into(),
                tests,
            },
            frame,
            "Whenever a player activates an ability",
        ))
    }
    /// What an activation trigger asks of the ability. "Of a creature" means a
    /// permanent (109.2), so cycling a creature card is no such ability.
    pub(super) fn activation_test(
        &mut self,
        filter: &ObjectFilter,
        frame: Frame,
    ) -> Result<String> {
        let def = |test: &str| format!("def.is_some_and(|d| {test})");
        Ok(match filter {
            ObjectFilter::Any => "true".into(),
            ObjectFilter::All(filters) | ObjectFilter::Either(filters) => {
                let tests = filters
                    .iter()
                    .map(|f| self.activation_test(f, frame))
                    .collect::<Result<Vec<_>>>()?;
                let joint = if matches!(filter, ObjectFilter::All(_)) {
                    " && "
                } else {
                    " || "
                };
                format!("({})", tests.join(joint))
            }
            ObjectFilter::Not(inner) => format!("!({})", self.activation_test(inner, frame)?),
            ObjectFilter::OnStack(StackTrait::Activated) => "true".into(),
            ObjectFilter::OnStack(StackTrait::Mana) => def("d.is_mana_ability()"),
            ObjectFilter::OnStack(StackTrait::Loyalty) => def("d.is_loyalty_ability()"),
            ObjectFilter::OnStack(StackTrait::TapSymbolInCost) => def("d.cost.tap"),
            ObjectFilter::OnStack(StackTrait::XInCost) => def("d.cost.mana.x > 0"),
            ObjectFilter::OnStack(StackTrait::Keyword(AbilityKeyword::Exhaust)) => def("d.exhaust"),
            ObjectFilter::AbilityOf(objects) => {
                let of = self.matching(frame, |c| c.filter(objects, &Candidate::of("id")))?;
                format!("(def.is_some_and(|d| d.from == ZoneKind::Battlefield) && {of})")
            }
            other => return Err(gap(format!("activated ability {}", form(other)))),
        })
    }
    /// `EventSpec::Attack`. `data` is the attacker's controller, then its
    /// defending player, which is what a batch of them is split by.
    fn attack_event(
        &mut self,
        attackers: &ObjectFilter,
        by: Option<&PlayerSet>,
        defenders: &AttackDefenders,
    ) -> Result<(Matched, Vec<String>)> {
        let named = !matches!(defenders, AttackDefenders::Any);
        let frame = Frame {
            object: Some(Carried::Subject),
            player: match (by, named) {
                (Some(_), _) => Some(Carried::Data(0)),
                (None, true) => Some(Carried::Data(1)),
                (None, false) => None,
            },
            other_player: (by.is_some() && named).then_some(Carried::Data(1)),
            ..Frame::default()
        };
        let tests = self.matching(frame, |c| {
            let mut tests = vec![c.filter(attackers, &Candidate::of("id"))?];
            if let Some(by) = by {
                tests.push(c.player_in(by, "q.controller(id)")?);
            }
            tests.push(c.attacked(defenders)?);
            Ok(tests)
        })?;
        let arm = Arm {
            pattern: "Event::Attacks { attacker, defender, defending, .. }".into(),
            locals: "let id = *attacker; let subject = Target::Object(id); let data = [u32::from(q.controller(id).0), u32::from(defending.0)];".into(),
            tests: tests.clone(),
        };
        Ok((
            single("ATTACKS", arm, frame, "Whenever a creature attacks"),
            tests,
        ))
    }
    /// Creatures attacking at once. The declaration is whole by the time its
    /// events are matched, so the attackers state how many there are.
    fn attack_batch(
        &mut self,
        event: &EventSpec,
        per: &BatchPer,
        count: Option<&(Comparison, Expr)>,
        including: Option<&ObjectRef>,
    ) -> Result<Matched> {
        let EventSpec::Attack {
            attackers,
            by,
            defenders,
        } = event
        else {
            return Err(gap("attack batch of another event"));
        };
        let (mut matched, tests) = self.attack_event(attackers, by.as_ref(), defenders)?;
        let keyed = match per {
            BatchPer::Whole => false,
            BatchPer::Defender => true,
            BatchPer::Subject => return Err(gap("attack batch by attacker")),
        };
        matched.batch = Some(keyed);
        let frame = matched.frame;
        let mut each = tests;
        if keyed {
            each.push("a.defending == *defending".into());
        }
        let mut extra = Vec::new();
        if let Some(one) = including {
            let one = self.matching(frame, |c| c.object(one))?;
            extra.push(format!(
                "q.state().attackers.iter().any(|a| Some(a.id) == {one})"
            ));
            // Its count is of the others, as "and at least two other creatures".
            each.push(format!("Some(a.id) != {one}"));
        }
        if let Some((comparison, value)) = count {
            let value = self.matching(frame, |c| Ok(c.number(value)?.code))?;
            let many = format!(
                "q.state().attackers.iter().filter(|a| {{ let (id, defender) = (a.id, &a.target); {} }}).count() as i64",
                each.join(" && ")
            );
            extra.push(read::compare(&many, comparison, &value));
        }
        for arm in &mut matched.arms {
            arm.tests.extend(extra.iter().cloned());
        }
        Ok(matched)
    }
    /// `EventSpec::Counters` put at once. `data[1]` is the object, which keeps
    /// one object's counters apart from another's when split by subject.
    fn counters_batch(
        &mut self,
        change: &CounterChange,
        kind: Option<&crate::counter::CounterKind>,
        objects: &ObjectFilter,
        by: Option<&PlayerSet>,
        per: &BatchPer,
    ) -> Result<Matched> {
        let removed = *change != CounterChange::Put;
        let frame = Frame {
            object: Some(Carried::Subject),
            amount: Some(Carried::Data(0)),
            ..Frame::default()
        };
        let mut tests = vec![self.matching(frame, |c| c.filter(objects, &Candidate::of("id")))?];
        if let Some(kind) = kind {
            tests.push(format!("*kind == {}", read::counter_kind(kind)?));
        }
        if let Some(by) = by {
            tests.push(self.matching(frame, |c| c.player_in(by, "*by"))?);
        }
        if *change == CounterChange::LastRemoved {
            tests.push("q.state().obj(id).counters(*kind) == 0".into());
        }
        let mut matched = single(
            if removed { "COUNTERS_REMOVED" } else { "COUNTERS" },
            Arm {
                pattern: match removed {
                    true => "Event::CountersRemoved { object, kind, amount, by, .. }".into(),
                    false => "Event::CountersPut { object, kind, amount, by, .. }".into(),
                },
                locals: "let id = *object; let subject = Target::Object(id); let data = [*amount as u32, id.0];".into(),
                tests,
            },
            frame,
            match removed {
                true => "Whenever counters are removed from a permanent",
                false => "Whenever counters are put on a permanent",
            },
        );
        if removed {
            let feature = "engine: no event for a counter being removed";
            self.plan_event(&mut matched, feature, false)?;
        }
        matched.batch = Some(match per {
            BatchPer::Whole => false,
            BatchPer::Subject => true,
            BatchPer::Defender => return Err(gap("counters batch by defender")),
        });
        Ok(matched)
    }
    /// `EventSpec::Batch`: however many of an event happen at once.
    fn batch_event(
        &mut self,
        event: &EventSpec,
        per: &BatchPer,
        count: Option<&(Comparison, Expr)>,
        including: Option<&ObjectRef>,
    ) -> Result<Matched> {
        let mut matched = match event {
            EventSpec::Attack { .. } => self.attack_batch(event, per, count, including)?,
            EventSpec::Counters {
                change,
                kind,
                objects,
                by,
            } if count.is_none() && including.is_none() => {
                self.counters_batch(change, kind.as_ref(), objects, by.as_ref(), per)?
            }
            // "Blocks one or more creatures": one object's events, so its batch is the
            // whole of them.
            EventSpec::Pair {
                objects: ObjectFilter::SameObject(_),
                ..
            } if count.is_none() && including.is_none() => {
                let mut matched = self.event(event)?;
                matched.batch = Some(false);
                matched
            }
            other => {
                if count.is_some() || including.is_some() || *per != BatchPer::Whole {
                    return Err(gap(format!("batch of {}", form(other))));
                }
                let mut matched = self.event(other)?;
                if matched.sums {
                    return Err(gap("a batch of grouped events"));
                }
                matched.batch = Some(false);
                matched
            }
        };
        // No one of a whole batch is the event's, however the form binds it.
        if *per == BatchPer::Whole {
            matched.frame.members = matched.frame.object == Some(Carried::Subject)
                && !matches!(event, EventSpec::Counters { .. });
            matched.frame.object = None;
        }
        matched.frame.amount = matched.frame.amount.filter(|_| *per == BatchPer::Subject);
        Ok(matched)
    }
    /// An event by how many came before it this turn, of its own player's when
    /// `by_player`. The history a matcher sees stops short of its event.
    fn ordinal_event(
        &mut self,
        event: &EventSpec,
        comparison: &Comparison,
        value: &Expr,
        by_player: bool,
    ) -> Result<Matched> {
        if let EventSpec::Player {
            event: PlayerEvent::Expends,
            players,
            ..
        } = event
        {
            return self.expends(players, comparison, value);
        }
        if self.history.is_some() {
            return Err(gap("an ordinal event in a history question"));
        }
        let mut matched = self.event(event)?;
        // The first of a batch is the batch's: the rest see it in the history.
        let first = matches!(comparison, Comparison::Equal) && matches!(value, Expr::Constant(1));
        if (matched.batch.is_some() && !first) || matched.sums {
            return Err(gap("an ordinal of grouped events"));
        }
        let same = match (by_player, matched.frame.player) {
            (false, _) => String::new(),
            (true, Some(Carried::Subject)) => ".filter(|f: &Fired| f.subject == subject)".into(),
            (true, Some(Carried::Data(i))) => {
                format!(".filter(|f: &Fired| f.data[{i}] == data[{i}])")
            }
            (true, None) => return Err(gap("an ordinal of an event with no player")),
        };
        let mut asked = self.clone();
        asked.history = Some(History {
            outer: None,
            event_at: EventAt::Matcher,
            frame: Frame::default(),
        });
        let past = asked.event(event)?;
        let value = self.matching(matched.frame, |c| Ok(c.number(value)?.code))?;
        let count = format!(
            "q.history().this_turn().iter().filter_map({}){same}.count() as i64 + 1",
            past.history_test()
        );
        let test = read::compare(&count, comparison, &value);
        for arm in &mut matched.arms {
            arm.tests.push(test.clone());
        }
        Ok(matched)
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
        let to_players = matches!(recipients, DamageRecipients::Players(_));
        // "Target artifact that player controls": all a target filter can read
        // of the event is its subject, so the player is it, as in Cantrip's own
        // DEALS_COMBAT_DAMAGE_TO_PLAYER_SUBJECT.
        let player_subject = self.recipient_subject && to_players;
        let by_source = *subject == DamageSubject::Source && !player_subject;
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
        let excess = matches!(kind, DamageKind::Excess | DamageKind::ExcessNoncombat);
        let mut frame = Frame {
            amount: (singular || excess || sums_damage(damage)).then_some(Carried::Data(0)),
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
            // A player is no object: "that creature" is a gap here, not a player.
            frame.object = (!player_subject).then_some(Carried::Subject);
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
            DamageKind::Prevented => {}
            DamageKind::Excess => {
                tests.push("amount > 0".to_string())
            }
            DamageKind::ExcessNoncombat => {
                tests.push("amount > 0 && !combat".to_string())
            }
            other => return Err(gap(format!("damage kind {}", form(other)))),
        }
        tests.push(match source_kind {
            ObjectKind::Any => "true".to_string(),
            ObjectKind::Permanent => "!q.state().obj(source).zone.is(ZoneKind::Stack)".to_string(),
            // A spell deals its damage as it resolves, still on the stack.
            ObjectKind::Spell if self.history.is_none() => {
                "q.state().obj(source).zone.is(ZoneKind::Stack)".to_string()
            }
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
            DamageRecipients::Target { slot, index } => {
                c.named_recipient(&Recipients::Target {
                    slot: slot.clone(),
                    index: *index,
                })
            }
            DamageRecipients::Targets(slot) => {
                c.named_recipient(&Recipients::Targets(slot.clone()))
            }
        })?);
        let subject = if by_source {
            "Target::Object(source)"
        } else {
            "*target"
        };
        let locals = format!("let subject = {subject}; let data = [u32::from(amount), {second}];");
        Ok((frame, locals, tests))
    }
    /// `EventSpec::Discard`. The card is named where the discard put it, which
    /// is where an ability of the card itself has to be found (CR 702.29c).
    fn discard_event(
        &mut self,
        players: &PlayerSet,
        cards: &ObjectFilter,
        source_only: bool,
        kind: &DiscardKind,
        grouping: &DiscardGrouping,
    ) -> Result<Matched> {
        let frame = Frame {
            object: Some(Carried::Subject),
            player: Some(Carried::Data(0)),
            ..Frame::default()
        };
        let mut tests = vec![
            self.matching(frame, |c| c.player_in(players, "*player"))?,
            self.matching(frame, |c| c.filter(cards, &Candidate::of("id")))?,
        ];
        let (mask, pattern, phrase) = match kind {
            DiscardKind::Cycling => (
                "CYCLED",
                "Event::Cycled { card, player }",
                "Whenever a card is cycled",
            ),
            DiscardKind::Any => {
                tests.push("!*undefined".into());
                (
                    "DISCARDED",
                    "Event::Discarded { card, player, undefined }",
                    "Whenever a card is discarded",
                )
            }
            DiscardKind::CausedBy(filter) => {
                tests.push("!*undefined".into());
                let cause = self.matching(frame, |c| c.filter(filter, &Candidate::of("cause")))?;
                tests.push(planned::planned_value(
                    "engine: no discard cause in events",
                    "false",
                    &format!("cause.is_some_and(|cause| {cause})"),
                ));
                (
                    "DISCARDED",
                    "Event::Discarded { card, player, undefined }",
                    "Whenever a card is discarded",
                )
            }
        };
        if source_only {
            tests.push("id == asker.source".to_string());
        }
        let mut matched = single(
            mask,
            Arm {
                pattern: pattern.into(),
                locals: "let id = *card; let subject = Target::Object(id); let data = [u32::from(player.0), u32::MAX];".into(),
                tests,
            },
            frame,
            phrase,
        );
        if source_only {
            matched.functions = Some("WHERE_A_DISCARD_ENDS".into());
        }
        if let DiscardGrouping::Player = grouping {
            matched.batch = Some(false);
            matched.frame.members = true;
        }
        Ok(matched)
    }
    fn damage_event(&mut self, damage: &DamageEvent) -> Result<Matched> {
        let (frame, locals, tests) = self.damage_match(damage)?;
        let mut matched = single(
            "DAMAGE",
            Arm {
                pattern: damage_pattern(damage).into(),
                locals: format!("{} {locals}", damage_values(damage)),
                tests,
            },
            frame,
            "Whenever damage is dealt",
        );
        if matches!(damage.kind, DamageKind::Prevented) {
            matched.masks = vec!["DAMAGE_PREVENTED"];
        }
        matched.sums = sums_damage(damage);
        matched.batch = match damage.grouping {
            _ if matched.sums => None,
            DamageGrouping::SourceRecipient => None,
            DamageGrouping::Source | DamageGrouping::Recipient => Some(true),
            DamageGrouping::Batch => Some(false),
            DamageGrouping::SourceController => {
                return Err(gap("damage grouped per source controller"))
            }
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
                    SpellEvent::Copy => ("COPIED", "Event::Copied { copy, player, .. }", "*copy"),
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
                    // A turn's only second main phase (CR 500.8, 505.1a).
                    TurnPoint::PostcombatMainPhaseBegins | TurnPoint::SecondMainPhaseBegins => {
                        &["PostcombatMain"]
                    }
                    TurnPoint::CombatBegins => &["BeginCombat"],
                    TurnPoint::CombatEnds => &["EndOfCombat"],
                    TurnPoint::EndStepBegins => &["End"],
                    TurnPoint::CleanupStepBegins => &["Cleanup"],
                    TurnPoint::DeclareAttackersStepBegins => &["DeclareAttackers"],
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
                source_only,
                kind,
                grouping,
            } => self.discard_event(players, cards, *source_only, kind, grouping),
            EventSpec::Either(events) => {
                let mut merged: Option<Matched> = None;
                for event in events {
                    let next = self.event(event)?;
                    merged = Some(match merged {
                        None => next,
                        Some(mut all) => {
                            if all.functions != next.functions
                                || all.batch != next.batch
                                || all.sums != next.sums
                                || all.mana != next.mana
                            {
                                return Err(gap("alternative events with different rules"));
                            }
                            all.frame = all.frame.meet(next.frame);
                            all.planned.extend(next.planned);
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
            EventSpec::Player {
                event,
                players,
                objects,
            } => self.player_event(event, players, objects.as_ref()),
            EventSpec::Pair {
                event,
                objects,
                others,
            } => self.pair_event(event, objects, others),
            EventSpec::BecomesTarget { objects, kind, by } => {
                self.becomes_target(objects, kind, by)
            }
            EventSpec::NthEachTurn {
                event,
                comparison,
                value,
            } => self.ordinal_event(event, comparison, value, true),
            EventSpec::FirstEachTurn(event) => {
                self.ordinal_event(event, &Comparison::Equal, &Expr::Constant(1), false)
            }
            EventSpec::Attack {
                attackers,
                by,
                defenders,
            } => Ok(self.attack_event(attackers, by.as_ref(), defenders)?.0),
            EventSpec::Batch {
                event,
                per,
                count,
                including,
            } => self.batch_event(event, per, count.as_ref(), including.as_ref()),
            // The engine's event is one put, and each counter is one of these.
            EventSpec::Counters {
                change,
                kind,
                objects,
                by,
            } => {
                let per = BatchPer::Subject;
                let mut matched =
                    self.counters_batch(change, kind.as_ref(), objects, by.as_ref(), &per)?;
                matched.batch = None;
                self.plan_event(&mut matched, "engine: no event for each counter put", false)?;
                Ok(matched)
            }
            EventSpec::Activated { players, abilities } => self.activated_event(players, abilities),
            EventSpec::Primitive { name, .. } => {
                Err(gap(format!("primitive trigger event {name}")))
            }
            other => self.event_rest(other),
        }
    }
    /// The `Trigger`, with the matcher named `label`.
    pub(super) fn trigger(&mut self, matched: &Matched, label: &str, extra: &[String]) -> String {
        let mut body = String::new();
        for arm in &matched.arms {
            let tests: Vec<&str> = arm.tests.iter().chain(extra).map(String::as_str).collect();
            let tests = tests
                .join(" && ")
                .replace("cast_of(q, ", "cast_at(q, event, ");
            writeln!(
                body,
                "if let {} = event {{ {} if {} {{ return Some(Fired {{ subject, data }}); }} }}",
                arm.pattern, arm.locals, tests
            )
            .unwrap();
        }
        let matcher = format!("#[allow(unused_variables)] fn {label}(q: &dyn Query, asker: &Asker, event: &Event) -> Option<Fired> {{ {body} None }}");
        let mask = matched
            .masks
            .iter()
            .map(|m| format!("EventMask::{m}.0"))
            .collect::<Vec<_>>()
            .join(" | ");
        let trigger = if matched.planned.iter().all(|f| planned::landed(f)) {
            writeln!(self.functions, "{matcher}").unwrap();
            format!("when_asked(EventMask({mask}), {label})")
        } else {
            self.planned_trigger(&matched.planned, label, &matcher, &mask)
        };
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
        if let Some(total) = limits.total {
            let free = TriggerLimits {
                per_turn: limits.per_turn,
                total: None,
            };
            let def = self.triggered(label, event, intervening_if, &free, announcement, body)?;
            return Ok(planned::planned_value(
                "trigger total limit",
                &def,
                &format!("{def}.triggers_total({total})"),
            ));
        }
        let gate = self.trigger_gate.take();
        let conditional = self.ability_guard.take();
        if conditional.is_some()
            && (matches!(event, EventSpec::State { .. }) || announcement.modes.is_some())
        {
            return Err(gap("conditional Triggered"));
        }
        if gate.is_some()
            && (announcement.modes.is_some() || matches!(event, EventSpec::State { .. }))
        {
            return Err(gap("modal or state trigger that works from a level"));
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
        self.recipient_subject =
            format!("{:?}", announcement.targets).contains("ControlledBy(One(EventSubject))");
        let matched = self.event(event);
        self.recipient_subject = false;
        if let (EventSpec::Either(events), Err(failed)) = (event, &matched) {
            let whole = gate.is_none() && conditional.is_none() && limits.per_turn.is_none();
            if failed.feature == "alternative events with different rules" && whole {
                return self.triggered_each(label, events, intervening_if, limits, announcement, body);
            }
        }
        let matched = matched?;
        let adds_mana = |i: &Instruction| {
            matches!(
                i,
                Instruction::Effect {
                    operation: Operation::AddMana { .. },
                    ..
                }
            )
        };
        let mut matched = matched;
        if matched.mana
            && (announcement.modes.is_some()
                || !announcement.targets.is_empty()
                || !body.instructions.iter().all(adds_mana))
        {
            matched.mana = false;
            self.plan_event(
                &mut matched,
                "engine: only a mana ability triggers on tapping for mana",
                false,
            )?;
        }
        if announcement.modes.is_some() {
            return self.modal_triggered(
                label,
                &matched,
                intervening_if,
                limits,
                announcement,
                body,
            );
        }
        let targets = self.targets_for(announcement, label, matched.frame)?;
        self.frame = matched.frame;
        // CR 603.4 asks an intervening "if" twice. The engine's `condition` has
        // no event to read, so one that reads the event is asked by the matcher
        // and by the resolving effect instead.
        let mut extra = Vec::new();
        if let Some(gate) = &gate {
            extra.push(self.matching(matched.frame, |c| c.condition(gate))?);
        }
        let mut guard = None;
        let mut condition = None;
        // The ability is there or isn't when the event happens, and only then.
        if let Some(conditional) = &conditional {
            extra.push(self.matching(matched.frame, |c| c.condition(conditional))?);
        }
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
        if matched.sums {
            def.push_str(".sums_per_subject()");
        }
        if matched.mana {
            def.push_str(if lane_g::adds_what_was_produced(body) {
                ".mana_ability(TriggeredMana::OneOfProduced)"
            } else {
                ".mana_ability(TriggeredMana::Unstated)"
            });
        }
        if let Some(per_turn) = limits.per_turn {
            def.push_str(&format!(".triggers_each_turn({per_turn})"));
        }
        if let Some(divide) = self.dividing.take() {
            def.push_str(&format!(".divide({divide})"));
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
        let mut extra = Vec::new();
        let mut guard = None;
        let mut condition = None;
        if let Some(intervening) = intervening_if {
            match self.condition_function(intervening, &format!("{label}_if")) {
                Ok(function) => condition = Some(function),
                // As `triggered` does: asked by the matcher, and by each mode.
                Err(gap) if gap.feature.contains("event") => {
                    extra.push(self.matching(matched.frame, |c| c.condition(intervening))?);
                    guard = Some(intervening.clone());
                }
                Err(gap) => return Err(gap),
            }
        }
        let trigger = self.trigger(matched, &format!("{label}_event"), &extra);
        self.frame = matched.frame;
        self.grouped = matched.batch.is_some();
        self.mode_guard = guard;
        let modal = self.modal(announcement, label, Some(matched.frame));
        self.mode_guard = None;
        self.grouped = false;
        self.frame = Frame::default();
        let modal = modal?;
        let mut def = format!(
            "modal_triggered({trigger}, &{modal}, {})",
            quoted(&matched.phrase)
        );
        if let Some(condition) = condition {
            def.push_str(&format!(".condition({condition})"));
        }
        match matched.batch {
            Some(true) => def.push_str(".once_per_batch().batch_by_data()"),
            Some(false) => def.push_str(".once_per_batch()"),
            None => {}
        }
        if matched.sums {
            def.push_str(".sums_per_subject()");
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
