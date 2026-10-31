//! Delayed triggered abilities (CR 603.7). The body runs on a later stack item,
//! so what it names of the instruction that made it has to travel with it: the
//! engine carries one list of objects, or one subject.
use super::*;

/// What a delayed ability's body takes along from the instruction making it.
pub(super) struct Outer {
    /// The making instruction's view, to read references as they stand then.
    compiler: Compiler,
    /// `Vec<ObjectId>` code for each list carried.
    objects: RefCell<Vec<String>>,
    /// `Option<PlayerId>` code for the player carried.
    player: RefCell<Option<String>>,
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
            ObjectRef::Target { .. }
            | ObjectRef::RecipientObject { .. }
            | ObjectRef::EventSubject
            | ObjectRef::EventDestination
            | ObjectRef::EventOther
            | ObjectRef::Damage(_) => false,
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
        let code = format!("{}.into_iter().collect::<Vec<ObjectId>>()", outer.compiler.object(object)?);
        Ok(Some(format!("{}.first().copied()", self.carried_list(outer, code))))
    }
    /// A `Vec<ObjectId>` this body cannot read for itself, if `objects` is.
    pub(super) fn outside_objects(&self, objects: &ObjectSet) -> Result<Option<String>> {
        let Some(outer) = self.carrying() else {
            return Ok(None);
        };
        let local = match objects {
            ObjectSet::Selected(result) => self.selections.contains_key(&result.0),
            ObjectSet::Created(result) => self.creations.contains_key(&result.0),
            ObjectSet::Transition { result, .. } => self.moved.contains_key(&result.0),
            ObjectSet::Targets(_) => false,
            ObjectSet::One(object) => self.reads_itself(object),
            _ => true,
        };
        if local {
            return Ok(None);
        }
        let code = outer.compiler.objects(objects)?;
        Ok(Some(self.carried_list(outer, code)))
    }
    /// An `Option<PlayerId>` this body cannot read for itself, if `player` is.
    pub(super) fn outside_player(&self, player: &PlayerRef) -> Result<Option<String>> {
        let Some(outer) = self.carrying() else {
            return Ok(None);
        };
        if !matches!(
            player,
            PlayerRef::Target { .. } | PlayerRef::EventSubject | PlayerRef::EventOtherPlayer
        ) {
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
        Ok(format!("DelayedWhen::Step {{ step: Step::{step}, on_turn_of: {turn} }}"))
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
        let outer = Rc::new(Outer {
            compiler: self.clone(),
            objects: RefCell::new(Vec::new()),
            player: RefCell::new(None),
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
        let compiled = match intervening_if {
            Some(condition) => self.guarded_effect(condition, body, &label),
            None => self.effect(body, &label),
        };
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
        compiled?;
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
