//! The normal form equality needs: result, slot and binding ids renumbered in
//! the order they are defined, and commutative lists sorted. Two cards that
//! differ only in lowering order are equal here. The walk reads the IR's own
//! type table, so a new field of an id type is covered without a change here.
//! Spellings the lowerer reaches by different routes that mean the same thing
//! (`All` holding `Any`, a zone scope narrowed by its filter, a one-object
//! `InSet`, an open count bounded by the total divided) reduce to one form.
use crate::ir::Card;
use crate::ir_schema::{Fields, Schema, Ty, TypeDef};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

mod choice_results;
#[cfg(test)]
mod choice_results_tests;
mod entry;
mod events;
#[cfg(test)]
mod mode_results;
mod movement;
#[cfg(test)]
mod result_scopes;
mod visibility;
use movement::groups as movement_groups;
#[cfg(test)]
mod tests;

pub(crate) fn schema() -> Result<&'static Schema, String> {
    static SCHEMA: OnceLock<Result<Schema, String>> = OnceLock::new();
    SCHEMA
        .get_or_init(Schema::of_ir)
        .as_ref()
        .map_err(Clone::clone)
}

/// Lists whose order carries no meaning.
fn commutative(ty: &str, variant: &str) -> bool {
    matches!(
        (ty, variant),
        ("ObjectFilter" | "Condition", "All" | "Either")
    )
}

#[derive(Default)]
struct Ids {
    /// Old id to new id, per id type, in this ability's scope.
    maps: HashMap<&'static str, HashMap<u64, u64>>,
    /// Which ability this is, in visiting order.
    visit: usize,
    /// Unread definitions still need distinct ids for executable, valid IR.
    unread: u64,
    selections: u64,
    /// Definition identities, distinct from the numeric labels a future local
    /// invocation may reuse. Allocation counters survive a closure's scope.
    result_definitions: HashMap<u64, u64>,
    /// Branch outcomes are readable only by JoinResults, not ordinary operands.
    branch_receipts: BranchReceipts,
    results: u64,
    live_results: u64,
}

type BranchReceipts = HashMap<u64, Vec<(u64, u64)>>;
type ResultScope = (Option<HashMap<u64, u64>>, HashMap<u64, u64>, BranchReceipts);

/// Used results count up from zero; unread definitions count down in lexical
/// order. Their original lowering ids have no meaning, but aliasing them all
/// would violate the IR's unique-definition contract.
const UNUSED_START: u64 = u32::MAX as u64;

impl Ids {
    fn define(&mut self, kind: &'static str, value: &mut Value, read: Option<&HashSet<u64>>) {
        let Some(old) = value.as_u64() else { return };
        if kind == "selection" {
            let new = self.selections;
            self.selections += 1;
            self.maps.entry(kind).or_default().insert(old, new);
            *value = Value::from(new);
            return;
        }
        if kind == "result" {
            let definition = self.results;
            self.results += 1;
            // Preserve invalid duplicate definitions for validation to reject.
            if let Some(new) = self.maps.get(kind).and_then(|map| map.get(&old)) {
                *value = Value::from(*new);
                return;
            }
            self.result_definitions.insert(old, definition);
            let new = if read.is_some_and(|read| !read.contains(&definition)) {
                let new = UNUSED_START - self.unread;
                self.unread += 1;
                new
            } else {
                let new = self.live_results;
                self.live_results += 1;
                new
            };
            self.maps.entry(kind).or_default().insert(old, new);
            *value = Value::from(new);
            return;
        }
        let map = self.maps.entry(kind).or_default();
        let next = map.len() as u64;
        let new = *map.entry(old).or_insert(next);
        *value = Value::from(new);
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Subject {
    Other,
    /// A trigger on the source's own event: its subject is that same object.
    Source,
    /// The source's own entry: subject and controller are the source's as well.
    Entering,
}

/// What an ability's event is about, through a `Filtered` wrapper.
fn event_objects(event: &Value) -> Option<&Value> {
    if let Some(filtered) = event.get("Filtered") {
        return event_objects(&filtered["event"]);
    }
    ["Object", "ZoneChange", "Pair"]
        .iter()
        .find_map(|kind| event.get(*kind))
        .map(|body| &body["objects"])
}

fn subject_of(ability: &Value) -> Subject {
    let args = &ability["args"];
    if args["active"] == "SourceEntering" {
        return Subject::Entering;
    }
    events::subject(&args["event"])
}

struct Walker {
    schema: &'static Schema,
    scopes: Vec<Ids>,
    /// The role each open ability's damage event calls its subject.
    damage_subjects: Vec<Option<Value>>,
    /// Whether each open ability's event is about the source itself.
    subjects: Vec<Subject>,
    event_header: bool,
    reference_reductions: bool,
    /// Keep ids and list order, for callers that read the text back out.
    keep_order: bool,
    /// The definitions each ability reads, by visiting order. The first of the two
    /// passes fills this; the second consults it.
    read: Vec<HashSet<u64>>,
    /// Whether this is the pass that fills `read`.
    reading: bool,
    /// Only resolution-time PayCost exports receipts from its player actions.
    paying_cost: bool,
    /// A surrounding immediate loop gathers optional outcomes as receipts too.
    gathering_results: bool,
    visits: usize,
    join_work: usize,
    failure: Option<String>,
}

/// The role a damage event's own subject plays, through a `Filtered` wrapper.
fn damage_subject(event: &Value) -> Option<Value> {
    match event.get("Filtered") {
        Some(filtered) => damage_subject(&filtered["event"]),
        None => event.get("Damage").map(|damage| damage["subject"].clone()),
    }
}

fn id_kind(name: &str) -> Option<&'static str> {
    match name {
        "ResultId" => Some("result"),
        "Slot" => Some("slot"),
        "Binding" => Some("binding"),
        "SelectionId" => Some("selection"),
        _ => None,
    }
}

impl Walker {
    fn result_scope(&self) -> Option<ResultScope> {
        self.scopes.last().map(|s| {
            (
                s.maps.get("result").cloned(),
                s.result_definitions.clone(),
                s.branch_receipts.clone(),
            )
        })
    }

    fn restore_result_scope(&mut self, saved: Option<ResultScope>) {
        if let (Some((map, definitions, branches)), Some(scope)) = (saved, self.scopes.last_mut()) {
            match map {
                Some(map) => {
                    scope.maps.insert("result", map);
                }
                None => {
                    scope.maps.remove("result");
                }
            }
            scope.result_definitions = definitions;
            scope.branch_receipts = branches;
        }
    }

    fn restore_with_exports(&mut self, saved: Option<ResultScope>, exports: &HashSet<u64>) {
        let kept: Vec<_> = self
            .scopes
            .last()
            .into_iter()
            .flat_map(|s| {
                exports.iter().filter_map(|old| {
                    Some((
                        *old,
                        *s.maps.get("result")?.get(old)?,
                        *s.result_definitions.get(old)?,
                    ))
                })
            })
            .collect();
        self.restore_result_scope(saved);
        if let Some(scope) = self.scopes.last_mut() {
            for (old, new, definition) in kept {
                scope.maps.entry("result").or_default().insert(old, new);
                scope.result_definitions.insert(old, definition);
            }
        }
    }

    fn use_id(&mut self, kind: &'static str, value: &mut Value) {
        let Some(old) = value.as_u64() else { return };
        if self.keep_order {
            return;
        }
        for scope in self.scopes.iter().rev() {
            if let Some(new) = scope.maps.get(kind).and_then(|m| m.get(&old)) {
                if self.reading && kind == "result" {
                    if let Some(definition) = scope.result_definitions.get(&old) {
                        self.read[scope.visit].insert(*definition);
                    }
                }
                *value = Value::from(*new);
                return;
            }
        }
    }

    fn walk(&mut self, value: &mut Value, ty: &Ty, defining: bool) {
        if self.failure.is_some() {
            return;
        }
        match ty {
            Ty::Opaque => {}
            Ty::Vec(inner) => {
                if let Value::Array(items) = value {
                    for item in items.iter_mut() {
                        let local =
                            (**inner == Ty::Named("Program".into())).then(|| self.result_scope());
                        self.walk(item, inner, false);
                        if let Some(local) = local {
                            self.restore_result_scope(local);
                        }
                    }
                    if **inner == Ty::Named("Instruction".into()) {
                        splice_groups(items);
                    } else if **inner == Ty::Named("Ability".into()) {
                        merge_statics(items);
                        if !self.keep_order {
                            merge_triggers(items);
                        }
                    }
                }
            }
            Ty::Option(inner) => {
                if !value.is_null() {
                    self.walk(value, inner, defining);
                }
            }
            Ty::Tuple(parts) => {
                if let Value::Array(items) = value {
                    for (item, part) in items.iter_mut().zip(parts) {
                        self.walk(item, part, false);
                    }
                }
            }
            Ty::Named(name) => self.named(value, name, defining),
        }
    }

    fn named(&mut self, value: &mut Value, name: &str, defining: bool) {
        // Presentation wraps the same ability/keyword, without adding an id,
        // event-subject or ability-holder scope of its own.
        let empty = crate::presentation::empty(value, name);
        if let Some(inner) = crate::presentation::content(value, name) {
            self.named(inner, name, defining);
            if empty {
                *value = inner.take();
            }
            return;
        }
        if let Some(kind) = id_kind(name) {
            if defining {
                if let (Some(scope), false) = (self.scopes.last_mut(), self.keep_order) {
                    let read = (!self.reading).then(|| &self.read[scope.visit]);
                    scope.define(kind, value, read);
                }
            } else {
                self.use_id(kind, value);
            }
            return;
        }
        // Modes inherit enclosing receipts, but their definitions cannot escape
        // into sibling modes or the common ability body (see validator).
        let local_results = (name == "Mode"
            || name == "Operand" && value["kind"] == "Program"
            || name == "Instruction" && value["kind"] == "Repeatable")
            .then(|| self.result_scope());
        let exported = if name == "Instruction" && value["kind"] == "Repeatable" {
            visibility::exports(&value["args"], true)
        } else {
            HashSet::new()
        };
        let previous_gathering = self.gathering_results;
        if name == "Instruction" && value["kind"] == "Repeatable" {
            self.gathering_results = true;
        }
        let scoped = name == "Ability";
        if scoped {
            self.gathering_results = false;
        }
        if scoped {
            if !self.keep_order && value["kind"] == "Triggered" {
                if let Some((_, fields)) = self.schema.variant("Ability", "Triggered") {
                    entry::header(&mut value["args"], fields, self.schema);
                }
            }
            self.scopes.push(Ids {
                visit: self.visits,
                ..Ids::default()
            });
            self.visits += 1;
            if self.reading {
                self.read.push(HashSet::new());
            }
            self.damage_subjects
                .push(damage_subject(&value["args"]["event"]));
            self.subjects.push(subject_of(value));
        }
        let schema = self.schema;
        match schema.types.get(name) {
            Some(TypeDef::Struct(fields)) => {
                self.fields(value, fields, name);
                if name == "TargetSpec" {
                    bound_divided_count(value);
                }
            }
            Some(TypeDef::Enum { adjacent, .. }) => self.variant(value, name, *adjacent),
            None => {}
        }
        if scoped {
            self.scopes.pop();
            self.damage_subjects.pop();
            self.subjects.pop();
        }
        if let Some(saved) = local_results {
            self.restore_with_exports(saved, &exported);
        }
        self.gathering_results = previous_gathering;
    }

    fn variant(&mut self, value: &mut Value, name: &str, adjacent: bool) {
        let schema = self.schema;
        if name == "EventSpec" && !self.keep_order {
            entry::player(value, schema);
        }
        let variant = if adjacent {
            value
                .get("kind")
                .and_then(Value::as_str)
                .map(str::to_string)
        } else {
            match &*value {
                Value::Object(object) if object.len() == 1 => object.keys().next().cloned(),
                _ => None,
            }
        };
        let Some(variant) = variant else { return };
        let Some((_, fields)) = schema.variant(name, &variant) else {
            return;
        };
        let subject = self.subjects.last().copied().unwrap_or(Subject::Other);
        let references = self.reference_reductions && !self.event_header;
        if references
            && name == "ObjectRef"
            && variant == "EventSubject"
            && self.names_source(subject)
        {
            *value = serde_json::json!({"kind": "Source"});
            return;
        }
        let content = if adjacent {
            value.get_mut("args")
        } else {
            value.get_mut(&variant)
        };
        if let Some(content) = content {
            self.fields(content, fields, &format!("{name}::{variant}"));
            if commutative(name, &variant) {
                if let Value::Array(items) = content {
                    flatten(items, &variant);
                    if variant == "All" {
                        drop_identity(items);
                    }
                    if !self.keep_order {
                        items.sort_by_cached_key(Value::to_string);
                    }
                }
                if let Some(only) = single(content) {
                    *value = only;
                }
                return;
            }
            match (name, variant.as_str()) {
                ("ObjectFilter", "InSet") => one_object_set(value),
                ("Operation", "Move") => {
                    let args = content_of(value);
                    if args.get("terms").is_none() {
                        args["terms"] = json!({"order":"DestinationDefault","placement":"DestinationDefault","qualifiers":[]});
                    }
                }
                ("ObjectSet", "Zone") => {
                    narrow_zone_scope(content_of(value));
                    if let Some(filter) = battlefield_filter_mut(value) {
                        drop_permanent_types(filter);
                    }
                }
                ("ObjectSet", "InZone") => repeated_current_zone(value),
                ("EventSpec", "ZoneChange") => {
                    if value["ZoneChange"]["to"] == "Battlefield" {
                        drop_permanent_types(&mut value["ZoneChange"]["objects"]);
                    }
                }
                ("EventSpec", "Player") => {
                    if !self.keep_order && value["Player"]["event"] == "EntersUnderControl" {
                        drop_permanent_types(&mut value["Player"]["objects"]);
                    }
                }
                ("EventSpec", "Either") if !self.keep_order => {
                    if let Some(Value::Array(events)) = value.get_mut("Either") {
                        events.sort_by_cached_key(Value::to_string);
                    }
                }
                ("ObjectRef", "Damage") => {
                    if references
                        && self.damage_subjects.last().cloned().flatten().as_ref()
                            == value.get("args")
                    {
                        *value = serde_json::json!({"kind": "EventSubject"});
                    }
                }
                ("PlayerRef", "ControllerOf") => {
                    if references
                        && subject == Subject::Entering
                        && value["args"]["kind"] == "Source"
                    {
                        *value = serde_json::json!({"kind": "AbilityController"});
                    }
                }
                ("Recipients", "Union") => self.merge_permanent_groups(value),
                _ => {}
            }
        }
    }

    /// A printer reads `it` back from the event's subject, so only comparison merges them.
    fn names_source(&self, subject: Subject) -> bool {
        subject == Subject::Entering || (subject == Subject::Source && !self.keep_order)
    }

    /// Damage to two groups of permanents is damage to the permanents of either.
    fn merge_permanent_groups(&self, value: &mut Value) {
        let Some(items) = value["args"].as_array() else {
            return;
        };
        let mut filters = Vec::with_capacity(items.len());
        for item in items {
            if item["kind"] != "Objects" {
                return;
            }
            let Some(filter) = battlefield_filter(&item["args"]) else {
                return;
            };
            filters.push(filter.clone());
        }
        flatten(&mut filters, "Either");
        if !self.keep_order {
            filters.sort_by_cached_key(Value::to_string);
        }
        *value = serde_json::json!({"kind": "Objects", "args": {
            "kind": "Zone",
            "args": {"scope":{"Shared":"Battlefield"}, "kind":"Permanent",
                "filter":{"kind": "Either", "args": filters}},
        }});
    }

    fn fields(&mut self, value: &mut Value, fields: &Fields, owner: &str) {
        match fields {
            Fields::Unit => {}
            Fields::Tuple(types) => {
                if let [only] = types.as_slice() {
                    self.walk(value, only, false);
                } else if let Value::Array(items) = value {
                    for (item, ty) in items.iter_mut().zip(types) {
                        self.walk(item, ty, false);
                    }
                }
            }
            Fields::Named(types) => {
                // A loop or predicate binds only its own body. Keeping those
                // ids in the ability-wide map makes a later sibling depend on
                // traversal order, including when commutative events move.
                let binding = types.iter().any(|(field, _)| field == "binding");
                let previous_binding = binding
                    .then(|| self.scopes.last().map(|s| s.maps.get("binding").cloned()))
                    .flatten();
                let previous_selection = (owner == "Operation::WithSelections").then(|| {
                    self.scopes
                        .last()
                        .and_then(|s| s.maps.get("selection").cloned())
                });
                if owner == "Operation::CreateTrigger" && !self.keep_order {
                    entry::header(value, fields, self.schema);
                }
                let created = events::created(owner, value);
                let old_reductions = self.reference_reductions;
                if matches!(
                    owner,
                    "Operation::CreateDelayedTrigger"
                        | "Operation::CreateReplacement"
                        | "Operation::ReflexiveTrigger"
                ) {
                    // Closures may retain the creator's event or install a new
                    // event. Preserve their explicit references rather than
                    // equating roles without proof of that binding context.
                    self.reference_reductions = false;
                }
                // A preparation step's inputs see the earlier bindings. Its
                // own binding starts only after those inputs have been read.
                let selection = owner == "SelectionStep";
                let late = |field: &str| {
                    selection && field == "binding"
                        || owner == "Instruction::Effect" && field == "result"
                };
                let ordered = types
                    .iter()
                    .filter(|(field, _)| !late(field))
                    .chain(types.iter().filter(|(field, _)| late(field)));
                for (field, ty) in ordered {
                    if let Some(item) = value.get_mut(field) {
                        if owner == "Instruction::ChooseOne" && field == "options" {
                            self.choice_options(item, ty);
                            continue;
                        }
                        if owner == "Operation::JoinResults" && field == "results" {
                            self.join_results(item);
                            continue;
                        }
                        if owner == "Ability::Saga" && field == "chapters" {
                            if let (Value::Array(items), Ty::Vec(inner)) = (item, ty) {
                                for chapter in items {
                                    self.walk(chapter, inner, false);
                                }
                            }
                            continue;
                        }
                        let old_header = self.event_header;
                        if field == "event" {
                            self.event_header = true;
                        }
                        // Delayed/replacement/reflexive bodies inherit earlier
                        // definitions, but do not define results in the creator.
                        let local_results = ((field == "body"
                            && matches!(
                                owner,
                                "Operation::CreateDelayedTrigger"
                                    | "Operation::CreateReplacement"
                                    | "Operation::ReflexiveTrigger"
                                    | "Operation::CreateTrigger"
                                    | "Instruction::Optional"
                                    | "CostPart::PlayerAction"
                                    | "Instruction::ForEachPlayer"
                                    | "Instruction::ForEachObject"
                                    | "Instruction::Times"
                            ))
                            || owner == "Instruction::If"
                                && matches!(field.as_str(), "then" | "otherwise"))
                        .then(|| self.result_scope());
                        let exported = if field == "body" {
                            match owner {
                                "Instruction::ForEachPlayer" | "Instruction::ForEachObject" => {
                                    visibility::exports(item, true)
                                }
                                "Instruction::Optional" => {
                                    visibility::exports(item, self.gathering_results)
                                }
                                "CostPart::PlayerAction" if self.paying_cost => {
                                    visibility::exports(item, false)
                                }
                                _ => HashSet::new(),
                            }
                        } else {
                            HashSet::new()
                        };
                        let old_gathering = self.gathering_results;
                        if field == "body" {
                            match owner {
                                "Instruction::ForEachPlayer" | "Instruction::ForEachObject" => {
                                    self.gathering_results = true
                                }
                                "Operation::CreateDelayedTrigger"
                                | "Operation::CreateReplacement"
                                | "Operation::ReflexiveTrigger"
                                | "Operation::CreateTrigger"
                                | "Instruction::Times"
                                | "CostPart::PlayerAction" => self.gathering_results = false,
                                _ => {}
                            }
                        } else if owner == "Instruction::If"
                            && matches!(field.as_str(), "then" | "otherwise")
                        {
                            self.gathering_results = false;
                        }
                        let old_cost = self.paying_cost;
                        self.paying_cost = owner == "Operation::PayCost" && field == "cost"
                            || self.paying_cost && owner == "Cost" && field == "parts";
                        if let Some(frame) = created.as_ref().filter(|_| {
                            matches!(field.as_str(), "body" | "announcement" | "intervening_if")
                        }) {
                            self.walk_created(item, ty, frame);
                        } else {
                            self.walk(item, ty, defines(owner, field));
                        }
                        self.paying_cost = old_cost;
                        self.gathering_results = old_gathering;
                        if let Some(saved) = local_results {
                            self.restore_with_exports(saved, &exported);
                        }
                        self.event_header = old_header;
                    }
                }
                self.reference_reductions = old_reductions;
                if let (Some(previous), Some(scope)) = (previous_selection, self.scopes.last_mut())
                {
                    if let Some(map) = previous {
                        scope.maps.insert("selection", map);
                    } else {
                        scope.maps.remove("selection");
                    }
                }
                if let (Some(previous), Some(scope)) = (previous_binding, self.scopes.last_mut()) {
                    match previous {
                        Some(map) => {
                            scope.maps.insert("binding", map);
                        }
                        None => {
                            scope.maps.remove("binding");
                        }
                    }
                }
            }
        }
    }

    /// Created trigger bodies have their own matched event. Their matcher and
    /// duration still capture the creator's context. Their invocation's local
    /// results cannot escape into the creator; allocation remains deterministic.
    fn walk_created(&mut self, value: &mut Value, ty: &Ty, frame: &events::Frame) {
        let old_header = self.event_header;
        let old_reductions = self.reference_reductions;
        self.subjects.push(frame.subject);
        self.damage_subjects.push(frame.damage.clone());
        self.event_header = false;
        self.reference_reductions = true;
        self.walk(value, ty, false);
        self.subjects.pop();
        self.damage_subjects.pop();
        self.event_header = old_header;
        self.reference_reductions = old_reductions;
    }
}

/// A group only nests a sequence; its instructions run in the sequence around it.
fn splice_groups(items: &mut Vec<Value>) {
    if !items.iter().any(|item| item["kind"] == "Group") {
        return;
    }
    let mut flat = Vec::with_capacity(items.len());
    for item in items.drain(..) {
        match item["args"]["instructions"].as_array() {
            Some(inner) if item["kind"] == "Group" => flat.extend(inner.iter().cloned()),
            _ => flat.push(item),
        }
    }
    *items = flat;
}

/// Two statics in a row for the same objects are one static with both changes.
fn merge_statics(items: &mut Vec<Value>) {
    let mut merged: Vec<Value> = Vec::with_capacity(items.len());
    for item in items.drain(..) {
        if let Some(last) = merged.last_mut() {
            let same = item["kind"] == "Static"
                && last["kind"] == "Static"
                && item["args"]["kind"] == last["args"]["kind"]
                && item["args"]["affected"] == last["args"]["affected"];
            if same {
                let more = item["args"]["changes"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                if let Some(changes) = last["args"]["changes"].as_array_mut() {
                    changes.extend(more);
                }
                continue;
            }
        }
        merged.push(item);
    }
    *items = merged;
}

/// Whether two triggered abilities differ in nothing but what triggers them.
fn same_but_event(a: &Value, b: &Value) -> bool {
    a["kind"] == "Triggered"
        && b["kind"] == "Triggered"
        && ["intervening_if", "limits", "announcement", "body"]
            .iter()
            .all(|field| a["args"][*field] == b["args"][*field])
}

/// The events of a trigger, whether it has one or lists several.
fn trigger_events(ability: &Value) -> Vec<Value> {
    match ability["args"]["event"].get("Either") {
        Some(Value::Array(cases)) => cases.clone(),
        _ => vec![ability["args"]["event"].clone()],
    }
}

/// `Whenever a or b, do c`, held as one ability with `Either` or as two sharing a
/// body, means the same either way, so comparison sees one ability in event order.
fn merge_triggers(items: &mut Vec<Value>) {
    let mut merged: Vec<Value> = Vec::with_capacity(items.len());
    for item in items.drain(..) {
        match merged.last_mut() {
            Some(last) if same_but_event(last, &item) => {
                let mut events = trigger_events(last);
                events.extend(trigger_events(&item));
                last["args"]["event"] = json!({ "Either": events });
            }
            _ => merged.push(item),
        }
    }
    for item in merged.iter_mut().filter(|item| item["kind"] == "Triggered") {
        if let Some(Value::Array(events)) = item["args"]["event"].get_mut("Either") {
            events.sort_by_key(|event| event.to_string());
        }
    }
    *items = merged;
}

/// Indexing one currently filtered object already enforces this exact zone.
/// Keep the inner query: unlike EventSubject/Source, it selects nothing after
/// the object leaves. No history view or mismatched zone is simplified here.
fn repeated_current_zone(value: &mut Value) {
    let outer = &value["args"];
    let one = &outer["objects"];
    let reference = &one["args"];
    let query = &reference["args"];
    let objects = &query["objects"];
    if one["kind"] == "One"
        && reference["kind"] == "Query"
        && query["index"] == 0
        && objects["kind"] == "InZone"
        && objects["args"]["objects"]["kind"] == "One"
        && objects["args"]["zone"] == outer["zone"]
    {
        *value = one.clone();
    }
}

/// `All([a, All([b])])` means `All([a, b])`, for `All` and for `Either`.
fn flatten(items: &mut Vec<Value>, variant: &str) {
    let mut flat = Vec::with_capacity(items.len());
    for item in items.drain(..) {
        match (item.get("kind").and_then(Value::as_str), item.get("args")) {
            (Some(kind), Some(Value::Array(inner))) if kind == variant => {
                flat.extend(inner.iter().cloned());
            }
            _ => flat.push(item),
        }
    }
    *items = flat;
}

/// `All` of nothing but `Any` keeps one, so the list still means `Any`.
fn drop_identity(items: &mut Vec<Value>) {
    let any = |item: &Value| item.get("kind").and_then(Value::as_str) == Some("Any");
    if items.iter().any(|item| !any(item)) {
        items.retain(|item| !any(item));
    } else {
        items.truncate(1);
    }
}

fn content_of(value: &mut Value) -> &mut Value {
    match value.get("args") {
        Some(_) => &mut value["args"],
        None => value,
    }
}

/// `InSet(One(x))` is `SameObject(x)`.
fn one_object_set(value: &mut Value) {
    if value["args"]["kind"] == "One" {
        let object = value["args"]["args"].take();
        *value = serde_json::json!({"kind": "SameObject", "args": object});
    }
}

/// Every object on the battlefield is a permanent, so naming all six types
/// narrows nothing there. Other zones keep it: it is what `permanent card` means.
fn drop_permanent_types(filter: &mut Value) {
    let names_all = |item: &Value| {
        let Some(cases) = item["args"].as_array() else {
            return false;
        };
        item["kind"] == "Either"
            && cases.len() == 6
            && [
                "Artifact",
                "Battle",
                "Creature",
                "Enchantment",
                "Land",
                "Planeswalker",
            ]
            .iter()
            .all(|name| {
                cases
                    .iter()
                    .any(|case| case["kind"] == "CardType" && case["args"] == *name)
            })
    };
    if names_all(filter) {
        *filter = serde_json::json!({"kind": "Any"});
        return;
    }
    if filter["kind"] != "All" {
        return;
    }
    if let Some(items) = filter["args"].as_array_mut() {
        items.retain(|item| !names_all(item));
    }
    if let Some(only) = single(&filter["args"]) {
        *filter = only;
    }
}

/// A scope over all players narrowed by an `InZone` of that zone is that
/// player's zone, however the filter is nested.
fn narrow_zone_scope(zone: &mut Value) {
    let wide = |scope: &Value| scope["Players"]["players"]["kind"] == "All";
    if !wide(&zone["scope"]) {
        return;
    }
    let same_zone = |filter: &Value, scope: &Value| {
        filter["kind"] == "InZone" && filter["args"]["Players"]["zone"] == scope["Players"]["zone"]
    };
    let scope = zone["scope"].clone();
    if same_zone(&zone["filter"], &scope) {
        zone["scope"] = zone["filter"]["args"].clone();
        zone["filter"] = serde_json::json!({"kind": "Any"});
        return;
    }
    if zone["filter"]["kind"] != "All" {
        return;
    }
    let Some(items) = zone["filter"]["args"].as_array_mut() else {
        return;
    };
    let Some(index) = items.iter().position(|item| same_zone(item, &scope)) else {
        return;
    };
    let narrowed = items.remove(index);
    if let Some(only) = single(&zone["filter"]["args"]) {
        zone["filter"] = only;
    }
    zone["scope"] = narrowed["args"].clone();
}

/// Divided among any number of targets: at most one target per point.
fn bound_divided_count(spec: &mut Value) {
    if spec["count"] == "AnyNumber" && !spec["distribute"].is_null() {
        spec["count"] = serde_json::json!({"UpTo": spec["distribute"].clone()});
    }
}

/// A commutative list of one item means that item.
fn single(content: &Value) -> Option<Value> {
    match content {
        Value::Array(items) if items.len() == 1 => items.first().cloned(),
        _ => None,
    }
}

/// Every other position of an id type reads an id defined at one of these.
fn defines(owner: &str, field: &str) -> bool {
    field == "binding"
        || (owner == "TargetSpec" && field == "slot")
        || (owner == "Instruction::Effect" && field == "result")
}

fn normalize(value: &mut Value, keep_order: bool) -> Result<(), String> {
    movement_groups(value)?;
    let mut walker = Walker {
        schema: schema()?,
        scopes: Vec::new(),
        damage_subjects: Vec::new(),
        subjects: Vec::new(),
        event_header: false,
        reference_reductions: true,
        keep_order,
        read: Vec::new(),
        reading: !keep_order,
        paying_cost: false,
        gathering_results: false,
        visits: 0,
        join_work: 0,
        failure: None,
    };
    let abilities = Ty::Vec(Box::new(Ty::Named("Ability".into())));
    if !keep_order {
        walker.walk(&mut value.clone(), &abilities, false);
        if let Some(why) = walker.failure.take() {
            return Err(why);
        }
        walker.reading = false;
        walker.visits = 0;
        walker.join_work = 0;
    }
    walker.walk(value, &abilities, false);
    match walker.failure {
        Some(why) => Err(why),
        None => Ok(()),
    }
}

/// The canonical form of serialized abilities, in place.
pub fn abilities(value: &mut Value) -> Result<(), String> {
    normalize(value, false)
}

/// Meaning comparison explicitly removes presentation before canonicalizing.
/// Ordinary `abilities` and `reduced` preserve it for exact round trips.
pub fn semantic_abilities(value: &mut Value) -> Result<(), String> {
    crate::presentation::erase_abilities(value)?;
    abilities(value)
}

/// The same reductions with ids and list order left as lowered: the order is
/// what a printer reads the card's wording back from.
pub fn reduced(value: &mut Value) -> Result<(), String> {
    normalize(value, true)
}

pub fn card(card: &Card) -> Result<Card, String> {
    let mut value = serde_json::to_value(card).map_err(|e| e.to_string())?;
    let abilities = value.get_mut("abilities").ok_or("card has no abilities")?;
    self::abilities(abilities)?;
    serde_json::from_value(value).map_err(|e| e.to_string())
}

/// Only shared, permanent battlefield queries imply the permanent type domain.
fn battlefield_filter(value: &Value) -> Option<&Value> {
    (value["kind"] == "Zone"
        && value["args"]["scope"] == json!({"Shared":"Battlefield"})
        && value["args"]["kind"] == "Permanent")
        .then(|| &value["args"]["filter"])
}
fn battlefield_filter_mut(value: &mut Value) -> Option<&mut Value> {
    battlefield_filter(value)?;
    value.get_mut("args")?.get_mut("filter")
}
