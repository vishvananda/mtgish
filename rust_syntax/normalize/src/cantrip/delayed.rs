//! Delayed and reflexive triggered abilities (CR 603.7, 603.12). The body runs on
//! a later stack item, so what it names of the instruction that made it has to
//! travel with it: the engine carries one list of objects, or one subject, and
//! for a reflexive ability only the subject.
use super::*;

/// What a delayed ability's body takes along from the instruction making it.
pub(super) struct Outer {
    /// The making instruction's view, to read references as they stand then.
    compiler: Compiler,
    /// `Vec<ObjectId>` code for each list carried.
    objects: RefCell<Vec<String>>,
    /// `Option<PlayerId>` code for the player carried.
    player: RefCell<Option<String>>,
    /// `Option<ObjectId>` code of the object carried as a subject, for an
    /// ability that cannot carry lists.
    pub(super) subject: Option<RefCell<Option<String>>>,
    /// For an effect made at resolution, the requirement its reads wait on.
    made: Option<String>,
    /// A body with a matcher of its own that reads the same values.
    matcher: bool,
    /// `Vec<Target>` code of each part such an effect keeps.
    pub(super) parts: RefCell<Vec<String>>,
}

impl Outer {
    pub(super) fn knows(&self, result: &ResultId) -> bool {
        let c = &self.compiler;
        c.selections.contains_key(&result.0)
            || c.creations.contains_key(&result.0)
            || c.moved.contains_key(&result.0)
            || c.bound_outside(result)
    }
    pub(super) fn failed(&self, result: &ResultId) -> bool {
        self.compiler.failed_anywhere(result)
    }
}

/// The lists a delayed ability carries are one list to the engine, so each ends
/// with an object nothing can be.
pub(super) const ABOUT: &str = "fn about(q: &dyn Query, item: &StackItem, part: usize) -> Vec<ObjectId> { q.event_objects(item).split(|&id| id == ObjectId::NONE).nth(part).map_or_else(Vec::new, <[ObjectId]>::to_vec) }\n";

impl Compiler {
    /// Whether this body's stack item can read carried values.
    fn carrying(&self) -> Option<&Outer> {
        let readable = self.reader == Reader::Resolution && self.stack_item;
        self.outer.as_deref().filter(|outer| {
            readable
                || outer.made.is_some()
                || outer.matcher
                || (self.event_at == EventAt::TargetFilter && outer.subject.is_some())
        })
    }
    /// What a matcher reads of the list its ability carries, when this is one.
    fn read_by_matcher(
        &self,
        outer: &Outer,
        code: impl FnOnce() -> Result<String>,
    ) -> Result<Option<String>> {
        if !outer.matcher || (self.reader == Reader::Resolution && self.stack_item) {
            return Ok(None);
        }
        let code = code()?;
        let mut objects = outer.objects.borrow_mut();
        let part = objects.iter().position(|c| *c == code).unwrap_or_else(|| {
            objects.push(code);
            objects.len() - 1
        });
        Ok(Some(format!("q.trigger_about(asker, {part})")))
    }
    /// What an effect kept, read as `Vec<Target>` by its callbacks.
    fn kept_part(&self, outer: &Outer, code: String) -> String {
        let mut parts = outer.parts.borrow_mut();
        let part = parts.iter().position(|c| *c == code).unwrap_or_else(|| {
            parts.push(code);
            parts.len() - 1
        });
        format!("q.effect_about(asker, {part})")
    }
    /// A `Vec<Target>` this body cannot read for itself, if `recipients` is.
    pub(super) fn outside_recipients(&self, recipients: &Recipients) -> Result<Option<String>> {
        let Some(outer) = self.carrying() else {
            return Ok(None);
        };
        let (slot, index) = match recipients {
            Recipients::Targets(slot) => (slot, 0),
            Recipients::Target { slot, index } => (slot, *index),
            _ => return Ok(None),
        };
        if self.slots.contains_key(&slot.0) {
            return Ok(None);
        }
        if outer.subject.is_some() {
            let of_an_object = outer
                .compiler
                .slots
                .get(&slot.0)
                .is_some_and(|(_, spec)| matches!(spec.kind, TargetKind::Object(_)));
            let single = matches!(recipients, Recipients::Target { .. })
                || outer
                    .compiler
                    .slots
                    .get(&slot.0)
                    .is_some_and(|(_, spec)| matches!(spec.count, Count::Exactly(Expr::Constant(1))));
            if !(of_an_object && single) {
                return Err(gap("a made effect about a target that may be a player"));
            }
            let object = ObjectRef::Target {
                slot: slot.clone(),
                index,
            };
            return Ok(self.outside_object(&object)?.map(|code| {
                format!("({code}).map(Target::Object).into_iter().collect::<Vec<Target>>()")
            }));
        }
        let Some(feature) = &outer.made else {
            return Ok(None);
        };
        let code = outer.compiler.recipients(recipients)?;
        Ok(Some(planned::planned_value(
            feature,
            "Vec::<Target>::new()",
            &self.kept_part(outer, code),
        )))
    }
    /// Read a list made when the delayed ability was, as `Vec<ObjectId>`.
    fn carried_list(&self, outer: &Outer, code: String) -> String {
        let mut objects = outer.objects.borrow_mut();
        let part = objects.iter().position(|c| *c == code).unwrap_or_else(|| {
            objects.push(code);
            objects.len() - 1
        });
        format!("about(q, item, {part})")
    }
    fn reads_itself(&self, object: &ObjectRef) -> bool {
        match object {
            ObjectRef::Result { result, .. } | ObjectRef::Named { result, .. } => {
                self.selections.contains_key(&result.0)
                    || self.creations.contains_key(&result.0)
                    || self.moved.contains_key(&result.0)
                    || !self.bound_outside(result)
            }
            ObjectRef::Target { slot, .. } => self.slots.contains_key(&slot.0),
            // A body watching for an event of its own reads that event.
            ObjectRef::EventSubject => self.frame.object.is_some(),
            ObjectRef::EventDestination => self.frame.destination.is_some(),
            ObjectRef::EventOther => self.frame.other.is_some(),
            ObjectRef::EventStackObject => self.frame.stack_object.is_some(),
            ObjectRef::BlockPair(BlockRole::Attacker) => self.frame.attacker.is_some(),
            ObjectRef::BlockPair(BlockRole::Blocker) => self.frame.blocker.is_some(),
            ObjectRef::BlockPair(BlockRole::Other) => self.frame.other.is_some(),
            ObjectRef::Damage(DamageSubject::Source) => self.frame.damage_source.is_some(),
            ObjectRef::Damage(DamageSubject::Recipient) => self.frame.damage_recipient.is_some(),
            ObjectRef::RecipientObject { .. } => false,
            ObjectRef::Bound(binding) => self.bound_objects.contains(&binding.0),
            _ => true,
        }
    }
    /// An `Option<ObjectId>` this body cannot read for itself, if `object` is.
    pub(super) fn outside_object(&self, object: &ObjectRef) -> Result<Option<String>> {
        let Some(outer) = self.carrying() else {
            return Ok(None);
        };
        if self.reads_itself(object) {
            return Ok(None);
        }
        if let Some(subject) = &outer.subject {
            let code = outer.compiler.object(object)?;
            let mut carried = subject.borrow_mut();
            if carried.as_ref().is_some_and(|c| *c != code) {
                return Err(gap("reflexive ability about two objects"));
            }
            *carried = Some(code);
            return Ok(Some(if outer.made.is_some() {
                "Some(affected)".into()
            } else if self.event_at == EventAt::TargetFilter {
                "q.event_subject().object()".into()
            } else {
                "item.subject.object()".into()
            }));
        }
        if let Some(read) = self.read_by_matcher(outer, || {
            Ok(format!(
                "{}.into_iter().collect::<Vec<ObjectId>>()",
                outer.compiler.object(object)?
            ))
        })? {
            return Ok(Some(planned::planned_value(
                "event trigger carrying values",
                "None::<ObjectId>",
                &format!("{read}.first().copied()"),
            )));
        }
        if let Some(feature) = &outer.made {
            let code = format!(
                "{}.map(Target::Object).into_iter().collect::<Vec<Target>>()",
                outer.compiler.object(object)?
            );
            return Ok(Some(planned::planned_value(
                feature,
                "None::<ObjectId>",
                &format!(
                    "{}.first().and_then(|t: &Target| t.object())",
                    self.kept_part(outer, code)
                ),
            )));
        }
        let code = format!(
            "{}.into_iter().collect::<Vec<ObjectId>>()",
            outer.compiler.object(object)?
        );
        Ok(Some(format!(
            "{}.first().copied()",
            self.carried_list(outer, code)
        )))
    }
    /// A `Vec<ObjectId>` this body cannot read for itself, if `objects` is.
    pub(super) fn outside_objects(&self, objects: &ObjectSet) -> Result<Option<String>> {
        let Some(outer) = self.carrying() else {
            return Ok(None);
        };
        let local = match objects {
            ObjectSet::Selected(result) | ObjectSet::Observed(result) => {
                self.selections.contains_key(&result.0)
            }
            ObjectSet::Created(result) => self.creations.contains_key(&result.0),
            ObjectSet::Affected(result) => self.selections.contains_key(&result.0),
            ObjectSet::Transition { result, .. } => self.moved.contains_key(&result.0),
            ObjectSet::Named(result) => {
                self.selections.contains_key(&result.0)
                    || self.creations.contains_key(&result.0)
                    || self.moved.contains_key(&result.0)
                    || !self.bound_outside(result)
            }
            ObjectSet::Targets(slot) => self.slots.contains_key(&slot.0),
            ObjectSet::CostObjects { .. } => false,
            ObjectSet::One(object) => self.reads_itself(object),
            _ => true,
        };
        if local {
            return Ok(None);
        }
        if let Some(read) = self.read_by_matcher(outer, || outer.compiler.objects(objects))? {
            return Ok(Some(planned::planned_value(
                "event trigger carrying values",
                "Vec::<ObjectId>::new()",
                &read,
            )));
        }
        if let (None, Some(feature)) = (&outer.subject, &outer.made) {
            let code = format!(
                "({}).into_iter().map(Target::Object).collect::<Vec<Target>>()",
                outer.compiler.objects(objects)?
            );
            return Ok(Some(planned::planned_value(
                feature,
                "Vec::<ObjectId>::new()",
                &format!(
                    "{}.iter().filter_map(|t: &Target| t.object()).collect::<Vec<ObjectId>>()",
                    self.kept_part(outer, code)
                ),
            )));
        }
        if outer.subject.is_some() {
            if outer.made.is_some() && !matches!(objects, ObjectSet::One(_)) {
                return Err(gap("a made effect about a list of objects"));
            }
            return match objects {
                ObjectSet::One(object) => Ok(Some(format!(
                    "{}.into_iter().collect::<Vec<ObjectId>>()",
                    self.outside_object(object)?.expect("carried")
                ))),
                _ => {
                    let code = outer.compiler.objects(objects)?;
                    Ok(Some(planned::planned_value(
                        "reflexive ability about a list of objects",
                        "Vec::<ObjectId>::new()",
                        &format!("about(q, item, {code})"),
                    )))
                }
            };
        }
        let code = outer.compiler.objects(objects)?;
        Ok(Some(self.carried_list(outer, code)))
    }
    /// An `Option<PlayerId>` this body cannot read for itself, if `player` is.
    pub(super) fn outside_player(&self, player: &PlayerRef) -> Result<Option<String>> {
        let Some(outer) = self.carrying() else {
            return Ok(None);
        };
        let local = match player {
            PlayerRef::Target { slot, .. } => self.slots.contains_key(&slot.0),
            PlayerRef::PlayerOrControllerOf(recipients) => match &**recipients {
                Recipients::Target { slot, .. } | Recipients::Targets(slot) => {
                    self.slots.contains_key(&slot.0)
                }
                _ => true,
            },
            PlayerRef::EventSubject => self.frame.player.is_some(),
            PlayerRef::EventOtherPlayer => self.frame.other_player.is_some(),
            _ => true,
        };
        if local {
            return Ok(None);
        }
        if outer.matcher && !(self.reader == Reader::Resolution && self.stack_item) {
            let code = outer.compiler.player(player)?;
            let mut carried = outer.player.borrow_mut();
            if carried.as_ref().is_some_and(|c| *c != code) {
                return Err(gap("delayed ability about two players"));
            }
            *carried = Some(code);
            return Ok(Some(planned::planned_value(
                "event trigger carrying values",
                "None::<PlayerId>",
                "q.trigger_subject(asker).player()",
            )));
        }
        if let Some(feature) = &outer.made {
            if outer.subject.is_some() {
                return Err(gap("a made effect about a player"));
            }
            let code = format!(
                "{}.map(Target::Player).into_iter().collect::<Vec<Target>>()",
                outer.compiler.player(player)?
            );
            return Ok(Some(planned::planned_value(
                feature,
                "None::<PlayerId>",
                &format!(
                    "{}.first().and_then(|t: &Target| t.player())",
                    self.kept_part(outer, code)
                ),
            )));
        }
        let code = outer.compiler.player(player)?;
        let mut carried = outer.player.borrow_mut();
        if carried.as_ref().is_some_and(|c| *c != code) {
            return Err(gap("delayed ability about two players"));
        }
        *carried = Some(code);
        Ok(Some(if self.event_at == EventAt::TargetFilter {
            "q.event_subject().player()".into()
        } else {
            "item.subject.player()".into()
        }))
    }
    /// When a delayed ability triggers.
    fn delayed_when(
        &self,
        event: &EventSpec,
        start: &DelayedStart,
        expires: &Option<crate::ir::Duration>,
    ) -> Result<String> {
        let EventSpec::Turn { point, players } = event else {
            return Err(gap(format!("delayed trigger on event {}", form(event))));
        };
        let turn_of = |c: &Self| -> Result<String> {
            Ok(match players {
                PlayerSet::All => "None".to_string(),
                PlayerSet::One(player) => {
                    format!("Some({})", c.fetch(c.certain_player(player)?))
                }
                other => return Err(gap(format!("delayed trigger on turns of {}", form(other)))),
            })
        };
        if let TurnPoint::MainPhaseBegins = point {
            let turn = turn_of(self)?;
            return Ok(planned::planned_value(
                &format!("delayed trigger at {}", form(point)),
                &format!("DelayedWhen::Step {{ step: Step::PostcombatMain, on_turn_of: {turn} }}"),
                &format!("DelayedWhen::NextMainPhase {{ on_turn_of: {turn} }}"),
            ));
        }
        let step = match point {
            TurnPoint::UpkeepBegins => "Upkeep",
            TurnPoint::DrawStepBegins => "Draw",
            TurnPoint::FirstMainPhaseBegins => "PrecombatMain",
            TurnPoint::PostcombatMainPhaseBegins => "PostcombatMain",
            TurnPoint::CombatBegins => "BeginCombat",
            TurnPoint::CombatEnds => "EndOfCombat",
            TurnPoint::EndStepBegins => "End",
            TurnPoint::CleanupStepBegins => "Cleanup",
            TurnPoint::DeclareAttackersStepBegins => "DeclareAttackers",
            other => return Err(gap(format!("delayed trigger at {}", form(other)))),
        };
        // The next such step comes next turn only for a step no effect can
        // resolve ahead of this turn; a later one could still be this turn's.
        match start {
            DelayedStart::Immediately => {}
            DelayedStart::NextTurn if matches!(point, TurnPoint::UpkeepBegins) => {}
            DelayedStart::PlayersNextTurn(_) if matches!(point, TurnPoint::UpkeepBegins) => {}
            DelayedStart::ExtraTurn(_) => {
                return Ok(format!("DelayedWhen::ExtraTurnStep {{ step: Step::{step}, turn }}"))
            }
            DelayedStart::NextTurn => {
                let turn = turn_of(self)?;
                return Ok(planned::planned_value(
                    &format!("delayed trigger starting {}", form(start)),
                    &format!("DelayedWhen::Step {{ step: Step::{step}, on_turn_of: {turn} }}"),
                    &format!("DelayedWhen::StepOfNextTurn {{ step: Step::{step}, of: None }}"),
                ));
            }
            DelayedStart::PlayersNextTurn(player) => {
                let player = self.fetch(self.certain_player(player)?);
                return Ok(planned::planned_value(
                    &format!("delayed trigger starting {}", form(start)),
                    &format!("DelayedWhen::Step {{ step: Step::{step}, on_turn_of: Some({player}) }}"),
                    &format!("DelayedWhen::StepOfNextTurn {{ step: Step::{step}, of: Some({player}) }}"),
                ));
            }
        }
        match expires {
            None => {}
            Some(crate::ir::Duration::EndOfCombat) if matches!(point, TurnPoint::CombatEnds) => {}
            Some(other) => return Err(gap(format!("delayed trigger until {}", form(other)))),
        }
        let turn = match start {
            DelayedStart::PlayersNextTurn(player) => {
                format!("Some({})", self.fetch(self.certain_player(player)?))
            }
            _ => turn_of(self)?,
        };
        Ok(format!(
            "DelayedWhen::Step {{ step: Step::{step}, on_turn_of: {turn} }}"
        ))
    }
    /// Compile a body that closes over the current instruction into `label`,
    /// with its targets. What it carried is on the `Outer` this returns.
    fn closure(
        &mut self,
        label: &str,
        subject_only: bool,
        announcement: Option<&Announcement>,
        intervening_if: &Option<Condition>,
        body: &Program,
    ) -> Result<(Rc<Outer>, Option<String>)> {
        self.closing(subject_only, |c| {
            // A target filter is a function of its own: no local reaches it.
            c.reset_results();
            let targets = announcement
                .map(|a| c.targets_for(a, label, Frame::default()))
                .transpose();
            match intervening_if {
                Some(condition) => c.guarded_effect(condition, body, label),
                None => c.effect(body, label),
            }?;
            targets
        })
    }
    /// `compile`, closed over the current instruction's values.
    fn closing<T>(
        &mut self,
        subject_only: bool,
        compile: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<(Rc<Outer>, T)> {
        self.closing_for(None, subject_only, compile)
    }
    /// `closing`, for an ability whose matcher reads what its body does.
    pub(super) fn closing_for_matcher<T>(
        &mut self,
        subject_only: bool,
        compile: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<(Rc<Outer>, T)> {
        let around = self.watching;
        self.watching = true;
        let closed = self.closing_for(None, subject_only, compile);
        self.watching = around;
        closed
    }
    /// `closing`, for an effect made at resolution when `made` says what its
    /// reads wait on, or a delayed ability.
    pub(super) fn closing_for<T>(
        &mut self,
        made: Option<String>,
        subject_only: bool,
        compile: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<(Rc<Outer>, T)> {
        let outer = Rc::new(Outer {
            compiler: self.clone(),
            objects: RefCell::new(Vec::new()),
            player: RefCell::new(None),
            subject: subject_only.then(|| RefCell::new(None)),
            made,
            matcher: self.watching,
            parts: RefCell::new(Vec::new()),
        });
        let results = self.results();
        let repeating = self.repeating.take();
        let scope = (
            self.event_at,
            self.frame,
            self.grouped,
            std::mem::take(&mut self.slots),
            std::mem::take(&mut self.bound_objects),
            std::mem::take(&mut self.bound_players),
        );
        (self.event_at, self.frame, self.grouped) = (EventAt::Nowhere, Frame::default(), false);
        // `affected` is a parameter of the function around, not of this one.
        let affected = std::mem::take(&mut self.affected_in_scope);
        let around = self.outer.replace(outer.clone());
        let compiled = compile(self);
        self.outer = around;
        self.affected_in_scope = affected;
        self.repeating = repeating;
        (
            self.event_at,
            self.frame,
            self.grouped,
            self.slots,
            self.bound_objects,
            self.bound_players,
        ) = scope;
        self.restore_results(results);
        Ok((outer, compiled?))
    }
    /// `Operation::CreateTrigger`: a delayed ability that watches for an event
    /// for a while (603.7b). The engine asks its matcher of `source`, so one
    /// fixed on a single object has that object for its source.
    pub(super) fn create_trigger(
        &mut self,
        event: &EventSpec,
        duration: &crate::ir::Duration,
        announcement: &Announcement,
        body: &Program,
        once: bool,
        intervening_if: &Option<Condition>,
        out: &mut String,
    ) -> Result<()> {
        use crate::ir::Duration as D;
        if self.reader != Reader::Resolution {
            return Err(gap("event trigger outside a resolving ability"));
        }
        let when = match (duration, once) {
            (D::EndOfTurn, false) => "DelayedWhen::ThisTurn".to_string(),
            (D::EndOfTurn, true) => "DelayedWhen::NextThisTurn".to_string(),
            (D::UntilNextTurn(player), false) => format!(
                "DelayedWhen::UntilNextTurnOf({})",
                self.fetch(self.certain_player(player)?)
            ),
            // "When you lose control of it", a one-time boon: the next such event.
            (D::Indefinite, true) => "DelayedWhen::UntilEvent".to_string(),
            (other, once) => {
                let lasting = self.duration(other)?;
                let (shape, placeholder) = match once {
                    true => ("LastingOnce", "DelayedWhen::NextThisTurn"),
                    false => ("Lasting", "DelayedWhen::ThisTurn"),
                };
                planned::planned_value(
                    &format!("event trigger for {}", form(other)),
                    placeholder,
                    &format!("DelayedWhen::{shape}({lasting})"),
                )
            }
        };
        let (event, source) = self.watching_source(event, body)?;
        let label = format!("watch_{}", self.fresh());
        let limits = TriggerLimits::default();
        let (outer, def) = self.closing_for_matcher(false, |c| {
            // Its matcher is a function of its own: no local reaches it.
            c.reset_results();
            c.triggered(
                &label,
                &event,
                intervening_if.as_ref(),
                &limits,
                announcement,
                body,
            )
        })?;
        self.delayed.push(def);
        let index = self.delayed.len() - 1;
        self.make_watching(&outer, &when, &source, index, out)
    }
    /// What a delayed ability on an event is asked of, and the event as that
    /// object watches it.
    pub(super) fn watching_source(
        &self,
        event: &EventSpec,
        body: &Program,
    ) -> Result<(EventSpec, String)> {
        let mut event = event.clone();
        let (maker, _, _) = self.maker();
        let mut source = format!("Some({maker})");
        if let EventSpec::ZoneChange { objects, .. } | EventSpec::Object { objects, .. } =
            &mut event
        {
            if let ObjectFilter::SameObject(object) = objects {
                if !matches!(object, ObjectRef::Source) {
                    source = self.fetch(self.object(object)?);
                    *objects = ObjectFilter::SameObject(ObjectRef::Source);
                }
            }
        }
        if !format!("{body:?}").contains("Source") {
            if let Some(object) = lane_i::watched(&mut event) {
                source = self.fetch(self.object(&object)?);
            }
        }
        Ok((event, source))
    }
    /// Make a delayed ability on an event. Cantrip keeps one subject for what
    /// waits for one event, and no list: what carries a list waits on that.
    pub(super) fn make_watching(
        &mut self,
        outer: &Outer,
        when: &str,
        source: &str,
        index: usize,
        out: &mut String,
    ) -> Result<()> {
        let (_, controller, card) = self.maker();
        let ability = format!("AbilityRef::delayed({card}, {index})");
        let objects = outer.objects.borrow();
        let player = outer.player.borrow();
        let subject = outer.subject.as_ref().and_then(|s| s.borrow().clone());
        if objects.is_empty() {
            let target = match (subject, player.as_ref()) {
                (None, None) => "Target::None".to_string(),
                (Some(object), None) => {
                    format!("{}.map_or(Target::None, Target::Object)", self.fetch(object))
                }
                (None, Some(player)) => {
                    format!("{}.map_or(Target::None, Target::Player)", self.fetch(player))
                }
                _ => return Err(gap("reflexive ability about an object and a player")),
            };
            if outer.subject.is_some() || target == "Target::None" {
                writeln!(out, "if let Some(source) = {source} {{ g.delayed_trigger({when}, source, {controller}, {ability}, {target}); }}").unwrap();
                return Ok(());
            }
        }
        self.helpers.insert("about");
        let mut about = String::from("let mut about: Vec<ObjectId> = Vec::new(); ");
        for list in objects.iter() {
            write!(about, "about.extend({}); about.push(ObjectId::NONE); ", self.fetch(list)).unwrap();
        }
        let code = match player.as_ref() {
            None => format!("if let Some(source) = {source} {{ {about} g.delayed_trigger_about({when}, source, {controller}, {ability}, &about); }}"),
            Some(player) => format!("if let Some(source) = {source} {{ {about} let player = {}.map_or(Target::None, Target::Player); g.delayed_trigger_with({when}, source, {controller}, {ability}, player, &about); }}", self.fetch(player)),
        };
        self.plan_code("event trigger carrying values", &code, out);
        Ok(())
    }
    /// `Operation::ReflexiveTrigger`.
    pub(super) fn create_reflexive(
        &mut self,
        performed: &Option<ResultId>,
        condition: &Option<Condition>,
        announcement: &Announcement,
        body: &Program,
        out: &mut String,
    ) -> Result<()> {
        if self.reader != Reader::Resolution {
            return Err(gap("reflexive trigger outside a resolving ability"));
        }
        // With no action named, getting this far is what happened.
        let reached = ResultId(u32::MAX);
        let (performed, reached) = match performed {
            Some(performed) => (performed, false),
            None => (&reached, true),
        };
        let happened = if reached {
            "true".to_string()
        } else if let Some(local) = self.performed.get(&performed.0) {
            match local.starts_with("removed_") || local.starts_with("done_") {
                true => format!("{local} > 0"),
                false => local.clone(),
            }
        } else if let Some(amount) = self
            .moved
            .get(&performed.0)
            .and_then(|m| self.whole_mills.get(m))
        {
            format!("{}.len() as u32 >= {amount}", self.moved(performed)?)
        } else if self.moved.contains_key(&performed.0) {
            format!("!{}.is_empty()", self.moved(performed)?)
        } else if self.selections.contains_key(&performed.0) {
            format!("!{}.is_empty()", self.selected(performed)?)
        } else if self.creations.contains_key(&performed.0) {
            format!("!{}.is_empty()", self.created(performed)?)
        } else if self.values.contains_key(&performed.0) || self.flips.contains_key(&performed.0) {
            "true".to_string()
        } else if self.failed.contains(&performed.0) {
            planned::unreported("false", performed)
        } else {
            return Err(gap(
                "reflexive trigger on what an instruction does not report",
            ));
        };
        let condition = condition
            .as_ref()
            .map(|c| Ok::<_, Diagnostic>(self.fetch(self.condition(c)?)))
            .transpose()?
            .unwrap_or_else(|| "true".into());
        let label = format!("reflexive_{}", self.fresh());
        // "When you do" of a payment the payer sized: its ability is told how much.
        let amount = self
            .values
            .get(&performed.0)
            .filter(|value| value.starts_with("bound_"))
            .cloned()
            .or_else(|| self.told_amount(performed, body));
        let slot = self.counted_slot(announcement);
        let amount = amount.or_else(|| slot.as_ref().map(|(_, code)| code.clone()));
        let counted = std::mem::replace(&mut self.counted, amount.as_ref().map(|_| performed.0));
        let sized = std::mem::replace(&mut self.sized_slot, slot.map(|(count, _)| count));
        let modal = self.reflexive_modes(announcement, &label);
        let announced = lane_p::without_modes(announcement);
        let closed = self.closure(&label, true, Some(&announced), &None, body);
        self.counted = counted;
        self.sized_slot = sized;
        let (outer, targets) = closed.map_err(lane_l::reads_as_a_target)?;
        let text = self.said(lane_k::Said::Triggered, ability_text(body));
        let mut def = format!("delayed_trigger({label}, {})", quoted(&text));
        if let Some(targets) = targets {
            write!(def, ".targets({targets})").unwrap();
        }
        if let Some(modal) = modal? {
            def = planned::planned_value(
                "modal announcement",
                &def,
                &format!("{def}.modes(&{modal})"),
            );
        }
        self.delayed.push(def);
        let index = self.delayed.len() - 1;
        let (source, controller, card) = self.maker();
        let subject = outer.subject.as_ref().expect("reflexive").borrow();
        let player = outer.player.borrow();
        let subject = match (subject.as_ref(), player.as_ref()) {
            (None, None) => "Target::None".to_string(),
            (Some(object), None) => format!(
                "{}.map_or(Target::None, Target::Object)",
                self.fetch(object)
            ),
            (None, Some(player)) => format!(
                "{}.map_or(Target::None, Target::Player)",
                self.fetch(player)
            ),
            _ => return Err(gap("reflexive ability about an object and a player")),
        };
        match amount {
            Some(amount) => writeln!(out, "if {happened} && {condition} {{ g.reflexive_trigger_counted({source}, {controller}, AbilityRef::delayed({card}, {index}), {subject}, ({amount}).clamp(0, i64::from(u16::MAX)) as u16); }}").unwrap(),
            None => writeln!(out, "if {happened} && {condition} {{ g.reflexive_trigger({source}, {controller}, AbilityRef::delayed({card}, {index}), {subject}); }}").unwrap(),
        }
        Ok(())
    }
    /// `Operation::CreateDelayedTrigger`.
    pub(super) fn create_delayed(
        &mut self,
        event: &EventSpec,
        start: &DelayedStart,
        expires: &Option<crate::ir::Duration>,
        intervening_if: &Option<Condition>,
        body: &Program,
        out: &mut String,
    ) -> Result<()> {
        if self.reader != Reader::Resolution {
            return Err(gap("delayed trigger outside a resolving ability"));
        }
        if lane_n::waits_for_an_event(event, expires) {
            return self.delayed_on_event(event, start, expires, intervening_if, body, out);
        }
        let when = self.delayed_when(event, start, expires)?;
        let label = format!("delayed_{}", self.fresh());
        let (outer, _) = self.closure(&label, false, None, intervening_if, body)?;
        let text = self.said(lane_k::Said::Triggered, ability_text(body));
        self.delayed
            .push(format!("delayed_trigger({label}, {})", quoted(&text)));
        let index = self.delayed.len() - 1;
        let (source, controller, card) = self.maker();
        let ability = format!("AbilityRef::delayed({card}, {index})");
        let objects = outer.objects.borrow();
        let player = outer.player.borrow();
        // "That turn": the turn the instruction before took, if it could.
        let (open, close) = match start {
            DelayedStart::ExtraTurn(result) => match self.performed.get(&result.0) {
                Some(local) => (format!("if let Some(turn) = {local} {{ "), " }"),
                None => return Err(gap(format!("delayed trigger starting {}", form(start)))),
            },
            _ => (String::new(), ""),
        };
        match (objects.as_slice(), player.as_ref()) {
            ([], None) => writeln!(out, "{open}g.delayed_trigger({when}, {source}, {controller}, {ability}, Target::None);{close}").unwrap(),
            ([], Some(player)) => writeln!(
                out,
                "{open}if let Some(player) = {} {{ g.delayed_trigger({when}, {source}, {controller}, {ability}, Target::Player(player)); }}{close}",
                self.fetch(player)
            )
            .unwrap(),
            (lists, player) => {
                self.helpers.insert("about");
                let mut about = String::from("let mut about: Vec<ObjectId> = Vec::new(); ");
                for list in lists {
                    write!(about, "about.extend({}); about.push(ObjectId::NONE); ", self.fetch(list)).unwrap();
                }
                let only_about_them = lists.len() == 1
                    && matches!(
                        body.instructions.as_slice(),
                        [Instruction::Effect {
                            operation: Operation::Move {
                                destination: Destination::Exile | Destination::OwnersHand,
                                ..
                            },
                            ..
                        }]
                    );
                let any = match only_about_them {
                    true => "if about.iter().any(|&o| o != ObjectId::NONE) ",
                    false => "",
                };
                match player {
                    None => writeln!(out, "{open}{{ {about} {any}{{ g.delayed_trigger_about({when}, {source}, {controller}, {ability}, &about); }} }}{close}").unwrap(),
                    Some(player) => self.plan_code(
                        "delayed ability about objects and a player",
                        &format!("{open}{{ {about} let player = {}.map_or(Target::None, Target::Player); g.delayed_trigger_with({when}, {source}, {controller}, {ability}, player, &about); }}{close}", self.fetch(player)),
                        out,
                    ),
                }
            }
        }
        Ok(())
    }
}
