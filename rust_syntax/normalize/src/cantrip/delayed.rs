//! Delayed and reflexive triggered abilities (CR 603.7, 603.12). The body runs on
//! a later stack item, so what it names of the instruction that made it has to
//! travel with it: step delays can retain an independent subject plus one real-object list.
//! Event delays and reflexive abilities keep their own, separately checked capture contracts.
use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Cardinality {
    OneAtMost,
    Many,
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum CaptureRole {
    EventSubject,
    Value,
    AuraCard,
}

/// A creator-scope expression and its proved bound, not a packed group of runtime IDs.
struct ObjectCapture {
    code: String,
    cardinality: Cardinality,
    // Prefer the lexical event subject; an Aura's graveyard card can use the independent list.
    priority: CaptureRole,
}
struct StepCaptures {
    label: String,
    objects: RefCell<Vec<ObjectCapture>>,
}
impl StepCaptures {
    fn read(&self, code: String, cardinality: Cardinality, priority: CaptureRole) -> String {
        let mut captures = self.objects.borrow_mut();
        let index = match captures.iter().position(|c| c.code == code) {
            Some(index) => {
                if cardinality == Cardinality::OneAtMost {
                    captures[index].cardinality = cardinality;
                }
                captures[index].priority = captures[index].priority.min(priority);
                index
            }
            None => {
                let index = captures.len();
                captures.push(ObjectCapture {
                    code,
                    cardinality,
                    priority,
                });
                index
            }
        };
        format!("{}_capture_{index}(q, item)", self.label)
    }
}

/// Does this IR refer to the creating object, rather than a damage-role named Source?
pub(super) fn reads_creator<T: crate::visit::Walk>(value: &T) -> bool {
    use crate::visit::Visitor;
    struct Find(bool);
    impl Visitor for Find {
        fn object_ref(&mut self, object: &ObjectRef) -> bool {
            self.0 |= matches!(object, ObjectRef::Source);
            true
        }
    }
    let mut find = Find(false);
    value.walk(&mut find);
    find.0
}

/// A future stack object is the one exception to CreateDelayedTrigger's
/// lexical creator frame. Walk typed operands so nested reads are included.
pub(super) fn reads_event_stack_object<T: crate::visit::Walk>(value: &T) -> bool {
    use crate::visit::Visitor;
    struct Find(bool);
    impl Visitor for Find {
        fn object_ref(&mut self, object: &ObjectRef) -> bool {
            self.0 |= matches!(object, ObjectRef::EventStackObject);
            true
        }
    }
    let mut find = Find(false);
    value.walk(&mut find);
    find.0
}

/// Whether the first watched reference in this filter is the event's object itself, rather than an object read by
/// a history predicate about that event's object. This follows the same search order as `lane_i::watched`.
fn direct_watched(filter: &ObjectFilter) -> Option<bool> {
    match filter {
        ObjectFilter::SameObject(object) if !matches!(object, ObjectRef::Source) => Some(true),
        ObjectFilter::All(filters) => filters.iter().find_map(direct_watched),
        ObjectFilter::Condition(condition) => match &**condition {
            Condition::Compare {
                left: Expr::EventHistory { event, .. },
                ..
            } => match &**event {
                EventSpec::Damage(damage) => direct_watched(&damage.sources).map(|_| false),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

/// Whether the event subject can carry the watched identity (including its last-known identity).
pub(super) fn watched_is_event_subject(event: &EventSpec) -> bool {
    match event {
        EventSpec::ZoneChange {
            objects,
            from: Some(_),
            to,
        } if *to != EventZone::Battlefield => direct_watched(objects) == Some(true),
        EventSpec::Object { objects, .. } => direct_watched(objects) == Some(true),
        EventSpec::Attack { attackers, .. } => direct_watched(attackers) == Some(true),
        EventSpec::Damage(damage) => match direct_watched(&damage.sources) {
            Some(true) => damage.subject == DamageSubject::Source,
            Some(false) => false,
            None => match &damage.recipients {
                DamageRecipients::Objects(objects) => {
                    direct_watched(objects) == Some(true)
                        && damage.subject == DamageSubject::Recipient
                }
                _ => false,
            },
        },
        _ => false,
    }
}

/// A one-shot watching its own source can carry that same object as the
/// subject. Restrict this to direct singular event filters: a history query or
/// a different event participant does not prove the identity.
fn source_is_event_subject(event: &EventSpec) -> bool {
    fn direct(filter: &ObjectFilter) -> bool {
        match filter {
            ObjectFilter::SameObject(ObjectRef::Source) => true,
            ObjectFilter::All(filters) => filters.iter().any(direct),
            _ => false,
        }
    }
    match event {
        EventSpec::ZoneChange {
            objects,
            from: Some(_),
            to,
        } if *to != EventZone::Battlefield => direct(objects),
        EventSpec::Object { objects, .. } => direct(objects),
        EventSpec::Attack { attackers, .. } => direct(attackers),
        EventSpec::Damage(d) if d.subject == DamageSubject::Source => direct(&d.sources),
        EventSpec::Damage(d) if d.subject == DamageSubject::Recipient => {
            matches!(&d.recipients, DamageRecipients::Objects(objects) if direct(objects))
        }
        _ => false,
    }
}

/// What a delayed ability's body takes along from the instruction making it.
pub(super) struct Outer {
    /// Only step-delayed closures use the subject plus real-object-list layout.
    step: Option<StepCaptures>,
    /// The making instruction's view, to read references as they stand then.
    pub(super) compiler: Compiler,
    /// `Vec<ObjectId>` code for each list carried.
    pub(super) objects: RefCell<Vec<String>>,
    /// `Option<PlayerId>` code for the player carried.
    pub(super) player: RefCell<Option<String>>,
    /// `Option<ObjectId>` code of the object carried as a subject, for an
    /// ability that cannot carry lists.
    pub(super) subject: Option<RefCell<Option<String>>>,
    /// For an effect made at resolution, the requirement its reads wait on.
    made: Option<String>,
    /// A body with a matcher of its own that reads the same values.
    matcher: bool,
    /// `Vec<Target>` code of each part such an effect keeps.
    pub(super) parts: RefCell<Vec<String>>,
    /// For legacy event captures the maker is carried because the watched object was used as source.
    source_moved: bool,
    /// The object the matcher watches; the new contract retains the creating source separately.
    watches: Option<ObjectRef>,
    pub(super) separate_watcher: bool,
    pub(super) watched_is_event_subject: std::cell::Cell<bool>,
    /// A reflexive ability, which is told one number as well as its subject.
    pub(super) reflexive: bool,
    /// `i64` code of the number it is told, worked out as it triggers.
    count: RefCell<Option<String>>,
}

impl Outer {
    /// Whether a spell made this, not an activation or an effect at resolution.
    pub(super) fn made_by_a_spell(&self) -> bool {
        self.made.is_none() && self.compiler.activation_cost.is_none()
    }
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

/// `delayed_trigger_about` keeps its list to the moment of a step: what an event
/// triggers is put on the stack with none, so `about` would read nothing.
const KEEPS_NO_LIST: &str = "engine: a delayed ability on an event keeps no list";

/// A reflexive ability about a list of objects: Cantrip's carries one target.
const REFLEXIVE_LIST: &str = "reflexive ability about a list of objects";
pub(super) const ABOUT: &str = "fn about(q: &dyn Query, item: &StackItem, part: usize) -> Vec<ObjectId> { assert_eq!(part, 0, \"one real-object list\"); q.event_objects(item).to_vec() }\n";

impl Compiler {
    /// The engine carries actual objects, not sentinel separators. One list needs no encoding; distinct groups need
    /// an API that preserves their boundaries without placing non-objects in `event_objects`.
    fn carried_objects(&self, lists: &[String]) -> Result<String> {
        let [list] = lists else {
            return Err(gap(
                "a triggered ability carrying separate groups of objects",
            ));
        };
        Ok(format!("let about: Vec<ObjectId> = {}; ", self.fetch(list)))
    }
    /// Whether `object` of the maker is the object this ability is asked of.
    fn watched_by(outer: &Outer, object: &ObjectRef) -> bool {
        outer
            .watches
            .as_ref()
            .is_some_and(|w| format!("{w:?}") == format!("{object:?}"))
    }
    /// Whether the list `objects` of the maker is only that object.
    fn watched_list(outer: &Outer, objects: &ObjectSet) -> bool {
        match objects {
            ObjectSet::One(object) => Self::watched_by(outer, object),
            ObjectSet::Targets(slot) => {
                let single = outer.compiler.slots.get(&slot.0).is_some_and(|(_, spec)| {
                    matches!(spec.count, Count::Exactly(Expr::Constant(1)))
                });
                single
                    && Self::watched_by(
                        outer,
                        &ObjectRef::Target {
                            slot: slot.clone(),
                            index: 0,
                        },
                    )
            }
            _ => false,
        }
    }
    /// Whether this body's stack item can read carried values.
    pub(super) fn carrying(&self) -> Option<&Outer> {
        let readable = self.reader == Reader::Resolution && self.stack_item;
        self.outer.as_deref().filter(|outer| {
            readable
                || outer.made.is_some()
                || outer.matcher
                || (self.event_at == EventAt::TargetFilter && outer.subject.is_some())
                || (self.event_at == EventAt::TargetFilter && outer.step.is_some())
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
                || outer.compiler.slots.get(&slot.0).is_some_and(|(_, spec)| {
                    matches!(spec.count, Count::Exactly(Expr::Constant(1)))
                });
            if of_an_object && single {
                let object = ObjectRef::Target {
                    slot: slot.clone(),
                    index,
                };
                return Ok(self.outside_object(&object)?.map(|code| {
                    format!("({code}).map(Target::Object).into_iter().collect::<Vec<Target>>()")
                }));
            }
            if outer.made.is_none() {
                return Err(gap(
                    "a reflexive ability about a target that may be a player",
                ));
            }
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
        // Choosing the targets of the ability, no item is there yet: the list
        // is the triggering's, as `Query::event_subject` is its subject.
        if self.event_at == EventAt::TargetFilter {
            return format!("q.event_about().split(|&id| id == ObjectId::NONE).nth({part}).map_or_else(Vec::new, <[ObjectId]>::to_vec)");
        }
        format!("about(q, item, {part})")
    }
    /// Whether `result` is what the ability making this body did: known to it, or
    /// its instruction's hole there, whose result it carries in as it would a known one.
    fn outside_result(&self, result: &ResultId) -> bool {
        self.bound_outside(result)
            || (!self.failed.contains(&result.0)
                && self
                    .outer
                    .as_ref()
                    .is_some_and(|outer| outer.compiler.failed.contains(&result.0)))
    }
    fn reads_itself(&self, object: &ObjectRef) -> bool {
        match object {
            ObjectRef::Result { result, .. } | ObjectRef::Recorded { result, .. } => {
                self.selections.contains_key(&result.0)
                    || self.creations.contains_key(&result.0)
                    || self.moved.contains_key(&result.0)
                    || !self.outside_result(result)
            }
            ObjectRef::Target { slot, .. } => self.slots.contains_key(&slot.0),
            // A body watching for an event of its own reads that event.
            ObjectRef::EventSubject => self.frame.object.is_some(),
            // A step-delayed body must capture the permitted graveyard identity
            // while its creator's enchanted-permanent departure is in scope.
            ObjectRef::AuraAfterEnchantedLeft { .. } => self.frame.subject_is_attachment,
            ObjectRef::EventDestination => self.frame.destination.is_some(),
            ObjectRef::EventOther => self.frame.other.is_some(),
            ObjectRef::EventStackObject => {
                self.frame.stack_object.is_some()
                    || self.frame.activated.is_some()
                    || self.frame.spell_or_ability
            }
            ObjectRef::BlockPair(BlockRole::Attacker) => self.frame.attacker.is_some(),
            ObjectRef::BlockPair(BlockRole::Blocker) => self.frame.blocker.is_some(),
            ObjectRef::BlockPair(BlockRole::Other) => self.frame.other.is_some(),
            ObjectRef::Damage(DamageSubject::Source) => self.frame.damage_source.is_some(),
            ObjectRef::Damage(DamageSubject::Recipient) => self.frame.damage_recipient.is_some(),
            ObjectRef::RecipientObject { .. } => false,
            ObjectRef::Bound(binding) => self.bound_objects.contains(&binding.0),
            ObjectRef::Source => {
                let resolving = self.reader == Reader::Resolution && self.stack_item;
                !(resolving && self.outer.as_ref().is_some_and(|o| o.source_moved))
            }
            // "That card" of what the maker's instruction did: what a subject holds.
            ObjectRef::Query { objects, .. } => {
                self.list_is_local(objects)
                    || self
                        .outer
                        .as_ref()
                        .map_or(true, |outer| outer.subject.is_none())
            }
            _ => true,
        }
    }
    /// Whether the list `objects` narrows is this body's own.
    fn list_is_local(&self, objects: &ObjectSet) -> bool {
        match objects {
            ObjectSet::Matching { objects, .. } => self.list_is_local(objects),
            ObjectSet::Selected(result) | ObjectSet::Observed(result) => {
                self.selections.contains_key(&result.0)
            }
            ObjectSet::Created(result) => self.creations.contains_key(&result.0),
            ObjectSet::Affected(result) => self.selections.contains_key(&result.0),
            ObjectSet::Transition { result, .. } => self.moved.contains_key(&result.0),
            ObjectSet::Recorded(result) => {
                self.selections.contains_key(&result.0)
                    || self.creations.contains_key(&result.0)
                    || self.moved.contains_key(&result.0)
                    || !self.outside_result(result)
            }
            ObjectSet::Targets(slot) => self.slots.contains_key(&slot.0),
            ObjectSet::CostObjects {
                payment: PaymentRef::Resolution(result),
                ..
            } => self.moved.contains_key(&result.0),
            ObjectSet::CostObjects { .. } => false,
            _ => true,
        }
    }
    /// "That many", of a list a reflexive ability's maker made: the one number
    /// the ability is told (CR 603.12), worked out as it triggers. In its target
    /// filter there is no stack item yet to read it from.
    pub(super) fn outside_count(&self, n: &Expr, objects: &ObjectSet) -> Result<Option<String>> {
        let Some(outer) = self.carrying() else {
            return Ok(None);
        };
        if !outer.reflexive || self.event_at == EventAt::TargetFilter || self.list_is_local(objects)
        {
            return Ok(None);
        }
        let inner = &outer.compiler;
        let code = format!("({} as i64)", inner.fetch(inner.number(n)?.code));
        let mut carried = outer.count.borrow_mut();
        if carried.as_ref().is_some_and(|c| *c != code) {
            return Err(gap("reflexive ability told two numbers"));
        }
        *carried = Some(code);
        Ok(Some("i64::from(item.x)".into()))
    }
    /// The number of the event around a reflexive ability, told to it.
    pub(super) fn told_event_amount(&self, n: &Expr) -> Result<String> {
        let outer = self
            .carrying()
            .ok_or_else(|| gap("amount of an event its trigger does not carry"))?;
        let inner = &outer.compiler;
        let code = format!("({} as i64)", inner.fetch(inner.number(n)?.code));
        let mut carried = outer.count.borrow_mut();
        if carried.as_ref().is_some_and(|c| *c != code) {
            return Err(gap("reflexive ability told two numbers"));
        }
        *carried = Some(code);
        Ok("i64::from(item.x)".into())
    }
    /// An `Option<ObjectId>` this body cannot read for itself, if `object` is.
    pub(super) fn outside_object(&self, object: &ObjectRef) -> Result<Option<String>> {
        let Some(outer) = self.carrying() else {
            return Ok(None);
        };
        if self.reads_itself(object) {
            return Ok(None);
        }
        if Self::watched_by(outer, object) {
            if !outer.separate_watcher || self.event_at == EventAt::Matcher {
                return Ok(Some("Some(asker.source)".into()));
            }
            if !outer.watched_is_event_subject.get() {
                return Err(gap(
                    "delayed ability retaining both watched object and a different event subject",
                ));
            }
            return match self.event_at {
                EventAt::Item => Ok(Some("item.subject.object()".into())),
                EventAt::TargetFilter => Ok(Some("q.event_subject().object()".into())),
                _ => Err(gap("delayed condition reading its watched object")),
            };
        }
        if let Some(step) = &outer.step {
            if self.event_at == EventAt::TargetFilter {
                return Err(gap("step-delayed target filter reading captured objects"));
            }
            let code = format!(
                "{}.into_iter().collect::<Vec<ObjectId>>()",
                outer.compiler.object(object)?
            );
            let priority = match object {
                ObjectRef::EventSubject => CaptureRole::EventSubject,
                ObjectRef::AuraAfterEnchantedLeft { .. } => CaptureRole::AuraCard,
                _ => CaptureRole::Value,
            };
            // ObjectRef is singular by construction; the helper may read its subject or its sole list member.
            return Ok(Some(format!(
                "{}.first().copied()",
                step.read(code, Cardinality::OneAtMost, priority)
            )));
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
    /// `Vec<ObjectId>`: the targets of the maker's division, each as often as its
    /// share, for a removal of that many from each.
    pub(super) fn outside_shares(&self, removal: &CounterRemoval) -> Result<Option<String>> {
        let (
            Recipients::Objects(ObjectSet::Targets(slot)),
            CounterAmount::Count(Count::Exactly(Expr::DistributedShare(share))),
        ) = (&removal.recipients, &removal.amount)
        else {
            return Ok(None);
        };
        let Some(outer) = self.carrying() else {
            return Ok(None);
        };
        if slot.0 != share.0
            || self.slots.contains_key(&slot.0)
            || outer.subject.is_some()
            || outer.made.is_some()
        {
            return Ok(None);
        }
        let at = outer.compiler.slot(slot)?;
        let code = format!(
            "q.state().targets(item).iter().enumerate().filter(|(_, t)| t_{at}.contains(t)).flat_map(|(i, t)| {{ let share = q.state().share(item, i) as usize; t.object().into_iter().flat_map(move |o| std::iter::repeat(o).take(share)) }}).collect::<Vec<ObjectId>>()"
        );
        if let Some(step) = &outer.step {
            return Ok(Some(step.read(code, Cardinality::Many, CaptureRole::Value)));
        }
        Ok(Some(self.carried_list(outer, code)))
    }
    /// A `Vec<ObjectId>` this body cannot read for itself, if `objects` is.
    pub(super) fn outside_objects(&self, objects: &ObjectSet) -> Result<Option<String>> {
        let Some(outer) = self.carrying() else {
            return Ok(None);
        };
        let local = match objects {
            ObjectSet::Prepared(binding) => self.prepared_selection(binding).is_ok(),
            ObjectSet::Earlier {
                what: EarlierObjects::Selected,
                by: None,
            } => {
                let own = self
                    .notes
                    .earlier
                    .iter()
                    .any(|(word, _)| *word == "Selected");
                let implicit_outer = outer
                    .compiler
                    .notes
                    .earlier
                    .iter()
                    .rev()
                    .find(|(word, _)| *word == "Selected")
                    .is_some_and(|(_, result)| {
                        outer.compiler.implicit_selections.contains_key(&result.0)
                    });
                own || !implicit_outer
            }
            ObjectSet::Selected(result) | ObjectSet::Observed(result) => {
                self.selections.contains_key(&result.0)
            }
            ObjectSet::Created(result) => self.creations.contains_key(&result.0),
            ObjectSet::Affected(result) => self.selections.contains_key(&result.0),
            ObjectSet::Transition { result, .. } => self.moved.contains_key(&result.0),
            ObjectSet::Recorded(result) => {
                self.selections.contains_key(&result.0)
                    || self.creations.contains_key(&result.0)
                    || self.moved.contains_key(&result.0)
                    || !self.outside_result(result)
            }
            ObjectSet::Targets(slot) => self.slots.contains_key(&slot.0),
            ObjectSet::CostObjects {
                payment: PaymentRef::Resolution(result),
                ..
            } => self.moved.contains_key(&result.0),
            ObjectSet::CostObjects { .. } => false,
            ObjectSet::One(object) => self.reads_itself(object),
            _ => true,
        };
        if local {
            return Ok(None);
        }
        if Self::watched_list(outer, objects) {
            let watched = if !outer.separate_watcher || self.event_at == EventAt::Matcher {
                "Some(asker.source)"
            } else if !outer.watched_is_event_subject.get() {
                return Err(gap(
                    "delayed ability retaining both watched object and a different event subject",
                ));
            } else {
                match self.event_at {
                    EventAt::Item => "item.subject.object()",
                    EventAt::TargetFilter => "q.event_subject().object()",
                    _ => return Err(gap("delayed condition reading its watched object")),
                }
            };
            return Ok(Some(format!(
                "{watched}.into_iter().collect::<Vec<ObjectId>>()"
            )));
        }
        if let Some(step) = &outer.step {
            if let ObjectSet::One(object) = objects {
                return Ok(Some(format!(
                    "{}.into_iter().collect::<Vec<ObjectId>>()",
                    self.outside_object(object)?.expect("nonlocal object")
                )));
            }
            if self.event_at == EventAt::TargetFilter {
                return Err(gap("step-delayed target filter reading captured objects"));
            }
            let one = outer.compiler.one_object(objects)
                || outer.compiler.single_carried_selection(objects);
            let cardinality = if one {
                Cardinality::OneAtMost
            } else {
                Cardinality::Many
            };
            return Ok(Some(step.read(
                outer.compiler.objects(objects)?,
                cardinality,
                CaptureRole::Value,
            )));
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
        if let Some(subject) = &outer.subject {
            if outer.made.is_some() && !matches!(objects, ObjectSet::One(_)) {
                return Err(gap("a made effect about a list of objects"));
            }
            return match objects {
                ObjectSet::One(object) => Ok(Some(format!(
                    "{}.into_iter().collect::<Vec<ObjectId>>()",
                    self.outside_object(object)?.expect("carried")
                ))),
                // What can only be one thing is what a subject holds.
                objects if outer.compiler.single_carried_selection(objects) => {
                    let code = format!("({}).first().copied()", outer.compiler.objects(objects)?);
                    let mut carried = subject.borrow_mut();
                    if carried.as_ref().is_some_and(|c| *c != code) {
                        return Err(gap("reflexive ability about two objects"));
                    }
                    *carried = Some(code);
                    Ok(Some(
                        match self.event_at == EventAt::TargetFilter {
                            true => {
                                "q.event_subject().object().into_iter().collect::<Vec<ObjectId>>()"
                            }
                            false => "item.subject.object().into_iter().collect::<Vec<ObjectId>>()",
                        }
                        .to_string(),
                    ))
                }
                // A list the reflexive ability is about goes with it as a part
                // of its own (`reflexive_trigger_about`), as a delayed one's.
                _ => {
                    let code = outer.compiler.objects(objects)?;
                    // A delayed ability carries it so too (`make_watching`).
                    let read = self.carried_list(outer, code);
                    Ok(Some(planned::planned_value(
                        REFLEXIVE_LIST,
                        "Vec::<ObjectId>::new()",
                        &read,
                    )))
                }
            };
        }
        // Reading a single object both as a condition and as the object to
        // move must use one capture. The general collection reader adds
        // different wrappers for the same Option<ObjectId>.
        let code = match objects {
            ObjectSet::One(object) => format!(
                "{}.into_iter().collect::<Vec<ObjectId>>()",
                outer.compiler.object(object)?
            ),
            _ => outer.compiler.objects(objects)?,
        };
        Ok(Some(self.carried_list(outer, code)))
    }
    /// An `Option<PlayerId>` this body cannot read for itself, if `player` is.
    pub(super) fn outside_player(&self, player: &PlayerRef) -> Result<Option<String>> {
        let Some(outer) = self.carrying() else {
            return Ok(None);
        };
        let local = match player {
            PlayerRef::Active
                if outer.made.is_none() && outer.compiler.active_player_named_later() =>
            {
                false
            }
            PlayerRef::Target { slot, .. } => self.slots.contains_key(&slot.0),
            PlayerRef::PlayerOrControllerOf(recipients) => match &**recipients {
                Recipients::Target { slot, .. } | Recipients::Targets(slot) => {
                    self.slots.contains_key(&slot.0)
                }
                _ => true,
            },
            PlayerRef::EventSubject => self.frame.player.is_some() || self.frame.player_is_controller,
            PlayerRef::EventOtherPlayer => self.frame.other_player.is_some(),
            // A player chosen in the body around this ability goes with it.
            PlayerRef::Chosen => {
                self.chosen_local(ValueKind::Player).is_ok()
                    || outer.compiler.chosen_local(ValueKind::Player).is_err()
            }
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
        // "At the beginning of your next main phase" (Mana Drain): from that
        // player's own precombat main phase or combat it is that turn's
        // postcombat main phase, from anywhere else their next precombat main
        // phase (CR 505.1), as `delayed::at_your_next_main_phase` reads it.
        if let (
            TurnPoint::MainPhaseBegins,
            PlayerSet::One(player),
            DelayedStart::Immediately,
            None,
        ) = (point, players, start, expires)
        {
            let me = self.fetch(self.certain_player(player)?);
            return Ok(format!("DelayedWhen::Step {{ step: {{ let q: &dyn Query = g; let turn = q.state().turn; if turn.active == {me} && (Step::PrecombatMain as u8..Step::PostcombatMain as u8).contains(&(turn.step() as u8)) {{ Step::PostcombatMain }} else {{ Step::PrecombatMain }} }}, on_turn_of: Some({me}) }}"));
        }
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
        // A specified next turn is distinct from the next such step. Even
        // upkeep can repeat in an additional beginning phase (CR 500.8, 603.7).
        match start {
            DelayedStart::Immediately => {}
            DelayedStart::ExtraTurn(_) => {
                return Ok(format!(
                    "DelayedWhen::ExtraTurnStep {{ step: Step::{step}, turn }}"
                ))
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
                    &format!(
                        "DelayedWhen::Step {{ step: Step::{step}, on_turn_of: Some({player}) }}"
                    ),
                    &format!(
                        "DelayedWhen::StepOfNextTurn {{ step: Step::{step}, of: Some({player}) }}"
                    ),
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
        step: bool,
        announcement: Option<&Announcement>,
        intervening_if: &Option<Condition>,
        body: &Program,
    ) -> Result<(Rc<Outer>, Option<String>)> {
        self.closing_with_layout(None, subject_only, step.then_some(label), |c| {
            // A target filter is a function of its own: no local reaches it.
            c.reset_results();
            let counts = std::mem::replace(&mut c.slot_counts, target_counts::SlotCounts::Delayed);
            let targets = announcement
                .map(|a| c.targets_for(a, label, Frame::default()))
                .transpose();
            c.slot_counts = counts;
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
        self.closing_with_layout(made, subject_only, None, compile)
    }
    fn closing_with_layout<T>(
        &mut self,
        made: Option<String>,
        subject_only: bool,
        step: Option<&str>,
        compile: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<(Rc<Outer>, T)> {
        let outer = Rc::new(Outer {
            step: step.map(|label| StepCaptures {
                label: label.into(),
                objects: RefCell::new(Vec::new()),
            }),
            compiler: self.clone(),
            objects: RefCell::new(Vec::new()),
            player: RefCell::new(None),
            subject: subject_only.then(|| RefCell::new(None)),
            made,
            matcher: self.watching,
            parts: RefCell::new(Vec::new()),
            source_moved: std::mem::take(&mut self.source_moved),
            watches: self.watching_object.take(),
            separate_watcher: std::mem::take(&mut self.watching_separately),
            watched_is_event_subject: std::cell::Cell::new(std::mem::take(
                &mut self.watched_is_event_subject,
            )),
            reflexive: std::mem::take(&mut self.reflexing),
            count: RefCell::new(None),
        });
        let results = self.results();
        let repeating = self.repeating.take();
        let around_item = (self.stack_item, self.pending);
        if outer.made.is_none() {
            (self.stack_item, self.pending) = (true, Pending::Nothing);
        }
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
        (self.stack_item, self.pending) = around_item;
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
        // "For as long as any of those cards remain exiled, at the beginning of each
        // player's upkeep" (Dimensional Breach), "at the beginning of each of that
        // player's upkeeps, if that card is exiled" (Ertai's Meddling): Cantrip's
        // ability at each step (`DelayedWhen::EachStep`) is never used up and is put
        // on the stack with no check, so it would go on triggering once the cards
        // have left exile.
        if let (EventSpec::Turn { .. }, D::WhileExiled(_) | D::WhileAnyExiled(_), false) =
            (event, duration, once)
        {
            return Err(gap(
                "a delayed ability at each step for as long as cards remain exiled",
            ));
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
        let original_event_reads_source = reads_creator(event);
        let watched_is_subject = watched_is_event_subject(event) || source_is_event_subject(event);
        let (event, source, each) = self.watching_source(event, body)?;
        let (maker, _, _) = self.maker();
        let separate = source != format!("Some({maker})");
        if separate && original_event_reads_source {
            return Err(gap(
                "delayed event reading both its creator and watched object",
            ));
        }
        let indefinite_once = matches!((duration, once), (D::Indefinite, true));
        if !separate && indefinite_once && !watched_is_subject {
            return Err(gap(
                "one-shot delayed trigger without a proven event subject",
            ));
        }
        if separate
            && !(matches!((duration, once), (D::EndOfTurn, false))
                || (indefinite_once && watched_is_subject && intervening_if.is_none()))
        {
            return Err(gap(
                "delayed trigger with a separate watched object and unsupported duration",
            ));
        }
        let when = if separate && indefinite_once {
            "DelayedWhen::WhenEvent".to_string()
        } else {
            when
        };
        self.watching_separately = separate;
        self.watched_is_event_subject = watched_is_subject;
        let label = format!("watch_{}", self.fresh());
        let limits = TriggerLimits::default();
        if !separate {
            self.watching_for_another(&source, body)?;
        }
        let (outer, def) = self.closing_for_matcher(false, |c| {
            // Its matcher is a function of its own: no local reaches it.
            c.reset_results();
            let counts = std::mem::replace(&mut c.slot_counts, target_counts::SlotCounts::Delayed);
            c.waiting_trigger = true;
            let def = c.triggered(
                &label,
                &event,
                intervening_if.as_ref(),
                &limits,
                announcement,
                body,
            );
            c.waiting_trigger = false;
            c.slot_counts = counts;
            def
        })?;
        self.delayed.push(def);
        let index = self.delayed.len() - 1;
        let Some(each) = each else {
            return self.make_watching(&outer, &when, &source, index, true, out);
        };
        // One ability on each of them: "whenever either of those creatures ...".
        writeln!(out, "for each in {each} {{").unwrap();
        self.make_watching(&outer, &when, &source, index, true, out)?;
        writeln!(out, "}}").unwrap();
        Ok(())
    }
    /// A delayed ability whose source is what it watches still acts as its maker
    /// did: `body` reads the maker through the list the ability carries. Only an
    /// effect's own source reads it no differently: Okoye's "it gains" is the
    /// creature's, as the card is written by hand.
    pub(super) fn watching_for_another(&mut self, source: &str, body: &Program) -> Result<()> {
        let (maker, _, _) = self.maker();
        let says = format!("{body:?}").replace("CreateContinuousEffect { source: Source,", "");
        if source != format!("Some({maker})") && says.contains("Source") {
            // What is linked to the maker can't be read through a list (CR 607).
            if says.contains("ExiledWith") {
                return Err(gap("event trigger carrying values"));
            }
            self.source_moved = true;
        }
        Ok(())
    }
    /// The watched target must still be legal as the ability resolves.
    fn watched_object(&self, object: &ObjectRef) -> Result<String> {
        if let ObjectRef::Target { slot, index } = object {
            if self.slots.contains_key(&slot.0)
                && !self.stack_slot(slot)
                && self.choosing.is_none()
                && !self.costing_targets
            {
                let i = self.slot(slot)?;
                return Ok(format!("t_{i}_raw.get({index}).copied().filter(|t| t_{i}.contains(t)).and_then(|t: Target| t.object())"));
            }
        }
        Ok(self.fetch(self.object(object)?))
    }
    /// What a delayed ability on an event is asked of, and the event as that
    /// object watches it.
    pub(super) fn watching_source(
        &mut self,
        event: &EventSpec,
        body: &Program,
    ) -> Result<(EventSpec, String, Option<String>)> {
        let mut event = event.clone();
        let (maker, _, _) = self.maker();
        let mut source = format!("Some({maker})");
        if let EventSpec::ZoneChange { objects, .. } | EventSpec::Object { objects, .. } =
            &mut event
        {
            if let ObjectFilter::SameObject(object) = objects {
                if !matches!(object, ObjectRef::Source) {
                    source = self.watched_object(object)?;
                    self.watching_object = Some(object.clone());
                    *objects = ObjectFilter::SameObject(ObjectRef::Source);
                }
            }
        }
        if let Some(object) = lane_i::watched(&mut event) {
            source = self.watched_object(&object)?;
            self.watching_object = Some(object);
        } else if let Some(list) = lane_i::watched_each(&mut event) {
            source = "Some(each)".to_string();
            return Ok((event, source, Some(self.fetch(self.objects(&list)?))));
        }
        Ok((event, source, None))
    }
    /// Make a delayed ability on an event. Cantrip keeps one subject for what
    /// waits for one event, and no list: what carries a list waits on that.
    pub(super) fn make_watching(
        &mut self,
        outer: &Outer,
        when: &str,
        source: &str,
        index: usize,
        trigger_semantics: bool,
        out: &mut String,
    ) -> Result<()> {
        let (_, controller, card) = self.maker();
        let ability = format!("AbilityRef::delayed({card}, {index})");
        let objects = outer.objects.borrow();
        let player = outer.player.borrow();
        let subject = outer.subject.as_ref().and_then(|s| s.borrow().clone());
        if trigger_semantics
            && !outer.separate_watcher
            && when == "DelayedWhen::UntilEvent"
            && (!objects.is_empty() || player.is_some() || subject.is_some())
        {
            return Err(gap(
                "one-shot delayed trigger with independent captured values",
            ));
        }
        if outer.separate_watcher && (!objects.is_empty() || player.is_some() || subject.is_some())
        {
            return Err(gap(
                "delayed ability with separate watched object and additional captured values",
            ));
        }
        if objects.is_empty() {
            let target = match (subject, player.as_ref()) {
                (None, None) => "Target::None".to_string(),
                (Some(object), None) => {
                    format!(
                        "{}.map_or(Target::None, Target::Object)",
                        self.fetch(object)
                    )
                }
                (None, Some(player)) => {
                    format!(
                        "{}.map_or(Target::None, Target::Player)",
                        self.fetch(player)
                    )
                }
                _ => return Err(gap("reflexive ability about an object and a player")),
            };
            if outer.subject.is_some()
                || target == "Target::None"
                || (!trigger_semantics && when == "DelayedWhen::WhenEvent" && player.is_some())
            {
                if outer.separate_watcher {
                    let (maker, _, _) = self.maker();
                    if target != "Target::None" {
                        return Err(gap("delayed ability retaining a separate subject"));
                    }
                    let (when, target) = if when == "DelayedWhen::WhenEvent" {
                        if !outer.watched_is_event_subject.get() {
                            return Err(gap(
                                "one-shot delayed trigger retaining a different event subject",
                            ));
                        }
                        ("DelayedWhen::WhenEvent", "Target::Object(watched)")
                    } else {
                        ("DelayedWhen::ThisTurnWatching(watched)", "Target::None")
                    };
                    let code = format!("if let Some(watched) = {source} {{ g.delayed_trigger({when}, {maker}, {controller}, {ability}, {target}); }}");
                    self.plan_switched(
                        "delayed-watched-source",
                        "delayed ability with separate source and watched object",
                        &code,
                        out,
                    );
                } else {
                    let (when, target) = if trigger_semantics && when == "DelayedWhen::UntilEvent" {
                        ("DelayedWhen::WhenEvent", "Target::Object(source)")
                    } else {
                        (when, target.as_str())
                    };
                    writeln!(out, "if let Some(source) = {source} {{ g.delayed_trigger({when}, source, {controller}, {ability}, {target}); }}").unwrap();
                }
                return Ok(());
            }
        }
        self.helpers.insert("about");
        let about = self.carried_objects(&objects)?;
        if !trigger_semantics && !outer.separate_watcher && when == "DelayedWhen::WhenEvent" {
            if player.is_some() {
                return Err(gap(
                    "one-shot delayed trigger with object and player captures",
                ));
            }
            writeln!(out, "if let Some(source) = {source} {{ {about} g.delayed_trigger_about({when}, source, {controller}, {ability}, &about); }}").unwrap();
            return Ok(());
        }
        match player.as_ref() {
            // Its body reads the list; what its matcher reads is held where it is read.
            None => self.plan_code(
                KEEPS_NO_LIST,
                &format!("if let Some(source) = {source} {{ {about} g.delayed_trigger_about({when}, source, {controller}, {ability}, &about); }}"),
                out,
            ),
            Some(player) => {
                let code = format!("if let Some(source) = {source} {{ {about} let player = {}.map_or(Target::None, Target::Player); g.delayed_trigger_with({when}, source, {controller}, {ability}, player, &about); }}", self.fetch(player));
                self.plan_code("event trigger carrying values", &code, out);
            }
        }
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
        // Asked again as the reflexive resolves only when it reads the game, not what
        // the instruction did, which is gone by then.
        let intervening = condition
            .as_ref()
            .filter(|c| !format!("{c:?}").contains("ResultId("));
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
            .or_else(|| self.told_amount(performed, body, announcement))
            .or_else(|| self.told_roll(performed, announcement, body));
        let slot = self.counted_slot(announcement);
        let amount = amount.or_else(|| slot.as_ref().map(|(_, code)| code.clone()));
        let counted = std::mem::replace(&mut self.counted, amount.as_ref().map(|_| performed.0));
        let sized = std::mem::replace(&mut self.sized_slot, slot.map(|(count, _)| count));
        let announced = lane_p::without_modes(announcement);
        let dividing = self.dividing.take();
        self.reflexing = true;
        let closed = self.closing(true, |c| {
            c.reset_results();
            let counts = std::mem::replace(&mut c.slot_counts, target_counts::SlotCounts::Delayed);
            // "Another target permanent or opponent": one slot, and the mode is
            // what the target turns out to be.
            let implied = match body.instructions.is_empty() {
                true => c.implied_slots(announcement, &label, Some(Frame::default())),
                false => Ok(None),
            };
            if let Ok(Some(targets)) = implied {
                c.slot_counts = counts;
                let effect = c.implied_effect(announcement, &label)?;
                writeln!(c.functions, "fn {label}(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {{ {effect}(g, item) }}").unwrap();
                return Ok((Some(targets), Ok(None)));
            }
            implied?;
            let targets = c.targets_for(&announced, &label, Frame::default());
            c.slot_counts = counts;
            let targets = targets?;
            let modal = c.reflexive_modes(announcement, &label);
            // An intervening "if" is asked again as it resolves (CR 603.4; Ajani, Nacatl Avenger).
            match intervening {
                Some(condition) => {
                    let test = c.fetch(c.condition(condition)?);
                    c.tested_effect(test, body, &label)?
                }
                None => c.effect(body, &label)?,
            };
            Ok((Some(targets), modal))
        });
        self.counted = counted;
        self.sized_slot = sized;
        let divided = std::mem::replace(&mut self.dividing, dividing);
        let (outer, (targets, modal)) = closed.map_err(lane_l::reads_as_a_target)?;
        let told = outer.count.borrow().clone();
        if amount.is_some() && told.is_some() && amount != told {
            return Err(gap("reflexive ability told two numbers"));
        }
        let amount = amount.or(told);
        let text = self.delayed_words(true, ability_text(body));
        let mut def = format!("delayed_trigger({label}, {})", quoted(&text));
        if let Some(targets) = targets {
            write!(def, ".targets({targets})").unwrap();
        }
        if let Some(divide) = divided {
            write!(def, ".divide({divide})").unwrap();
        }
        // The modes are the ability: a modal item runs nothing else (700.2).
        if let Some(modal) = modal? {
            def = match body.instructions.is_empty() {
                true => format!("{def}.modal(&{modal})"),
                false => planned::planned_value(
                    "modal announcement",
                    &def,
                    &format!("{def}.modal(&{modal})"),
                ),
            };
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
        let lists = outer.objects.borrow();
        match (amount, lists.as_slice()) {
            (Some(amount), _) => writeln!(out, "if {happened} && {condition} {{ g.reflexive_trigger_counted({source}, {controller}, AbilityRef::delayed({card}, {index}), {subject}, ({amount}).clamp(0, i64::from({kind}::MAX)) as {kind}); }}", kind = quantity::count_kind()).unwrap(),
            (None, []) => writeln!(out, "if {happened} && {condition} {{ g.reflexive_trigger({source}, {controller}, AbilityRef::delayed({card}, {index}), {subject}); }}").unwrap(),
            (None, lists) => {
                self.helpers.insert("about");
                let about = self.carried_objects(lists)?;
                let make = format!("if {happened} && {condition} {{ {about}g.reflexive_trigger_about({source}, {controller}, AbilityRef::delayed({card}, {index}), {subject}, &about); }}");
                self.plan_code(REFLEXIVE_LIST, &make, out);
            }
        }
        Ok(())
    }
    /// `body` as one of the card's `delayed` abilities, for the engine to put on
    /// the stack when something it keeps says (the start of an added phase): its
    /// `AbilityRef`, data the game state can hold where a closure can't be.
    pub(super) fn delayed_program(&mut self, body: &Program) -> Result<String> {
        let label = format!("delayed_{}", self.fresh());
        self.closure(&label, false, false, None, &None, body)?;
        let text = self.delayed_words(false, ability_text(body));
        self.delayed
            .push(format!("delayed_trigger({label}, {})", quoted(&text)));
        let index = self.delayed.len() - 1;
        let (_, _, card) = self.maker();
        Ok(format!("AbilityRef::delayed({card}, {index})"))
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
        let (outer, _) = self.closure(&label, false, true, None, intervening_if, body)?;
        let text = self.delayed_words(false, ability_text(body));
        self.delayed
            .push(format!("delayed_trigger({label}, {})", quoted(&text)));
        let index = self.delayed.len() - 1;
        let (source, controller, card) = self.maker();
        let ability = format!("AbilityRef::delayed({card}, {index})");
        let player = outer.player.borrow();
        // "That turn": the turn the instruction before took, if it could.
        let (open, close) = match start {
            DelayedStart::ExtraTurn(result) => match self.performed.get(&result.0) {
                Some(local) => (format!("if let Some(turn) = {local} {{ "), " }"),
                None => return Err(gap(format!("delayed trigger starting {}", form(start)))),
            },
            _ => (String::new(), ""),
        };
        let step = outer.step.as_ref().expect("step-delayed closure");
        let captures = step.objects.borrow();
        // Solve after all reads, including nested closures. Discovery order cannot change the selected role.
        let subject = if player.is_some() || captures.len() <= 1 {
            None
        } else {
            captures
                .iter()
                .enumerate()
                .filter(|(_, c)| c.cardinality == Cardinality::OneAtMost)
                .min_by(|(_, a), (_, b)| (a.priority, &a.code).cmp(&(b.priority, &b.code)))
                .map(|(i, _)| i)
        };
        let lists: Vec<_> = captures
            .iter()
            .enumerate()
            .filter(|(i, _)| Some(*i) != subject)
            .collect();
        if lists.len() > 1 {
            return Err(gap("step-delayed ability carrying more than one plural object group or excess independent references"));
        }
        for (i, _) in captures.iter().enumerate() {
            let read = if Some(i) == subject {
                "item.subject.object().into_iter().collect::<Vec<ObjectId>>()"
            } else if captures.len() == 1 && player.is_none() {
                self.helpers.insert("about");
                "about(q, item, 0)"
            } else {
                "q.event_objects(item).to_vec()"
            };
            writeln!(self.functions, "#[allow(unused_variables)] fn {}_capture_{i}(q: &dyn Query, item: &StackItem) -> Vec<ObjectId> {{ {read} }}", step.label).unwrap();
        }
        let subject_code = match (player.as_ref(), subject) {
            (Some(player), _) => format!(
                "{}.map_or(Target::None, Target::Player)",
                self.fetch(player)
            ),
            (None, Some(i)) => {
                debug_assert!(captures[i].cardinality == Cardinality::OneAtMost);
                format!(
                    "({}).into_iter().next().map_or(Target::None, Target::Object)",
                    self.fetch(&captures[i].code)
                )
            }
            _ => "Target::None".into(),
        };
        match lists.first() {
            None => writeln!(out, "{open}g.delayed_trigger({when}, {source}, {controller}, {ability}, {subject_code});{close}").unwrap(),
            Some((_, capture)) => {
                let about = self.fetch(&capture.code);
                if subject.is_some() || player.is_some() {
                    let code = format!("{open}{{ let about: Vec<ObjectId> = {about}; let subject = {subject_code}; g.delayed_trigger_with({when}, {source}, {controller}, {ability}, subject, &about); }}{close}");
                    self.plan_switched("delayed-subject-members", "step-delayed ability retaining an independent subject and object list", &code, out);
                } else {
                    let guard = if matches!(body.instructions.as_slice(), [Instruction::Effect { operation: Operation::Move { destination: MovementDestination::Exile {..} | MovementDestination::OwnersHand(_), .. }, .. }]) {
                        "if !about.is_empty()"
                    } else { "" };
                    writeln!(out, "{open}{{ let about: Vec<ObjectId> = {about}; {guard} {{ g.delayed_trigger_about({when}, {source}, {controller}, {ability}, &about); }} }}{close}").unwrap();
                }
            }
        }
        Ok(())
    }
}
