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
    subject: Option<RefCell<Option<String>>>,
}

/// The lists a delayed ability carries are one list to the engine, so each ends
/// with an object nothing can be.
pub(super) const ABOUT: &str = "fn about(q: &dyn Query, item: &StackItem, part: usize) -> Vec<ObjectId> { q.event_objects(item).split(|&id| id == ObjectId::NONE).nth(part).map_or_else(Vec::new, <[ObjectId]>::to_vec) }\n";

impl Compiler {
    /// Whether this body's stack item can read carried values.
    fn carrying(&self) -> Option<&Outer> {
        let readable = self.reader == Reader::Resolution && self.stack_item;
        self.outer.as_deref().filter(|_| readable)
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
            ObjectRef::Result { result, .. } => {
                self.selections.contains_key(&result.0)
                    || self.creations.contains_key(&result.0)
                    || self.moved.contains_key(&result.0)
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
            return Ok(Some("item.subject.object()".into()));
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
            ObjectSet::Targets(slot) => self.slots.contains_key(&slot.0),
            ObjectSet::CostObjects { .. } => false,
            ObjectSet::One(object) => self.reads_itself(object),
            _ => true,
        };
        if local {
            return Ok(None);
        }
        if outer.subject.is_some() {
            return match objects {
                ObjectSet::One(object) => Ok(Some(format!(
                    "{}.into_iter().collect::<Vec<ObjectId>>()",
                    self.outside_object(object)?.expect("carried")
                ))),
                _ => Err(gap("reflexive ability about a list of objects")),
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
            PlayerRef::EventSubject => self.frame.player.is_some(),
            PlayerRef::EventOtherPlayer => self.frame.other_player.is_some(),
            _ => true,
        };
        if local {
            return Ok(None);
        }
        let code = outer.compiler.player(player)?;
        let mut carried = outer.player.borrow_mut();
        if carried.as_ref().is_some_and(|c| *c != code) {
            return Err(gap("delayed ability about two players"));
        }
        *carried = Some(code);
        Ok(Some("item.subject.player()".into()))
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
            other => return Err(gap(format!("delayed trigger starting {}", form(other)))),
        }
        match expires {
            None => {}
            Some(crate::ir::Duration::EndOfCombat) if matches!(point, TurnPoint::CombatEnds) => {}
            Some(other) => return Err(gap(format!("delayed trigger until {}", form(other)))),
        }
        let turn = match players {
            PlayerSet::All => "None".to_string(),
            PlayerSet::One(player) => format!("Some({})", self.fetch(self.certain_player(player)?)),
            other => return Err(gap(format!("delayed trigger on turns of {}", form(other)))),
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
        let outer = Rc::new(Outer {
            compiler: self.clone(),
            objects: RefCell::new(Vec::new()),
            player: RefCell::new(None),
            subject: subject_only.then(|| RefCell::new(None)),
        });
        let results = self.results();
        let scope = (
            self.event_at,
            self.frame,
            self.grouped,
            std::mem::take(&mut self.slots),
            std::mem::take(&mut self.bound_objects),
            std::mem::take(&mut self.bound_players),
        );
        (self.event_at, self.frame, self.grouped) = (EventAt::Nowhere, Frame::default(), false);
        self.outer = Some(outer.clone());
        let compiled = compile(self);
        self.outer = None;
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
        out: &mut String,
    ) -> Result<()> {
        use crate::ir::Duration as D;
        if !self.stack_item || self.pending != Pending::Nothing || self.outer.is_some() {
            return Err(gap("event trigger outside a resolving ability"));
        }
        let when = match (duration, once) {
            (D::EndOfTurn, false) => "DelayedWhen::ThisTurn".to_string(),
            (D::EndOfTurn, true) => "DelayedWhen::NextThisTurn".to_string(),
            (D::UntilNextTurn(player), false) => format!(
                "DelayedWhen::UntilNextTurnOf({})",
                self.fetch(self.certain_player(player)?)
            ),
            (other, _) => return Err(gap(format!("event trigger for {}", form(other)))),
        };
        let mut event = event.clone();
        let mut source = "Some(item.object)".to_string();
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
        let label = format!("watch_{}", self.fresh());
        let limits = TriggerLimits::default();
        let (outer, def) = self.closing(false, |c| {
            // Its matcher is a function of its own: no local reaches it.
            c.reset_results();
            c.triggered(&label, &event, None, &limits, announcement, body)
        })?;
        if !outer.objects.borrow().is_empty() || outer.player.borrow().is_some() {
            return Err(gap("event trigger carrying values"));
        }
        self.delayed.push(def);
        let index = self.delayed.len() - 1;
        writeln!(out, "if let Some(source) = {source} {{ g.delayed_trigger({when}, source, item.controller, AbilityRef::delayed(item.ability.card, {index}), Target::None); }}").unwrap();
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
        let Some(performed) = performed else {
            return Err(gap("reflexive trigger with no base action"));
        };
        if !self.stack_item || self.pending != Pending::Nothing || self.outer.is_some() {
            return Err(gap("reflexive trigger outside a resolving ability"));
        }
        let happened = if let Some(local) = self.performed.get(&performed.0) {
            match local.starts_with("removed_") {
                true => format!("{local} > 0"),
                false => local.clone(),
            }
        } else if self.moved.contains_key(&performed.0) {
            format!("!{}.is_empty()", self.moved(performed)?)
        } else if self.selections.contains_key(&performed.0) {
            format!("!{}.is_empty()", self.selected(performed)?)
        } else if self.creations.contains_key(&performed.0) {
            format!("!{}.is_empty()", self.created(performed)?)
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
            .cloned();
        let counted = std::mem::replace(&mut self.counted, amount.as_ref().map(|_| performed.0));
        let closed = self.closure(&label, true, Some(announcement), &None, body);
        self.counted = counted;
        let (outer, targets) = closed?;
        let mut def = format!("delayed_trigger({label}, {})", quoted(&ability_text(body)));
        if let Some(targets) = targets {
            write!(def, ".targets({targets})").unwrap();
        }
        self.delayed.push(def);
        let index = self.delayed.len() - 1;
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
            Some(amount) => writeln!(out, "if {happened} && {condition} {{ g.reflexive_trigger_counted(item.object, item.controller, AbilityRef::delayed(item.ability.card, {index}), {subject}, ({amount}).clamp(0, i64::from(u16::MAX)) as u16); }}").unwrap(),
            None => writeln!(out, "if {happened} && {condition} {{ g.reflexive_trigger(item.object, item.controller, AbilityRef::delayed(item.ability.card, {index}), {subject}); }}").unwrap(),
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
        if !self.stack_item || self.pending != Pending::Nothing || self.outer.is_some() {
            return Err(gap("delayed trigger outside a resolving ability"));
        }
        let when = self.delayed_when(event, start, expires)?;
        let label = format!("delayed_{}", self.fresh());
        let (outer, _) = self.closure(&label, false, None, intervening_if, body)?;
        self.delayed.push(format!(
            "delayed_trigger({label}, {})",
            quoted(&ability_text(body))
        ));
        let index = self.delayed.len() - 1;
        let ability = format!("AbilityRef::delayed(item.ability.card, {index})");
        let objects = outer.objects.borrow();
        let player = outer.player.borrow();
        match (objects.as_slice(), player.as_ref()) {
            ([], None) => writeln!(out, "g.delayed_trigger({when}, item.object, item.controller, {ability}, Target::None);").unwrap(),
            ([], Some(player)) => writeln!(
                out,
                "if let Some(player) = {} {{ g.delayed_trigger({when}, item.object, item.controller, {ability}, Target::Player(player)); }}",
                self.fetch(player)
            )
            .unwrap(),
            (lists, None) => {
                self.helpers.insert("about");
                let mut about = String::from("let mut about: Vec<ObjectId> = Vec::new(); ");
                for list in lists {
                    write!(about, "about.extend({}); about.push(ObjectId::NONE); ", self.fetch(list)).unwrap();
                }
                writeln!(out, "{{ {about} g.delayed_trigger_about({when}, item.object, item.controller, {ability}, &about); }}").unwrap();
            }
            _ => return Err(gap("delayed ability about objects and a player")),
        }
        Ok(())
    }
}
