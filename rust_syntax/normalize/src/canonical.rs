//! The normal form equality needs: result, slot and binding ids renumbered in
//! the order they are defined, and commutative lists sorted. Two cards that
//! differ only in lowering order are equal here. The walk reads the IR's own
//! type table, so a new field of an id type is covered without a change here.
//! Spellings the lowerer reaches by different routes that mean the same thing
//! (`All` holding `Any`, a zone scope narrowed by its filter, a one-object
//! `InSet`, an open count bounded by the total divided) reduce to one form.
use crate::ir::Card;
use crate::ir_schema::{Fields, Schema, Ty, TypeDef};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

fn schema() -> Result<&'static Schema, String> {
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
}

impl Ids {
    fn define(&mut self, kind: &'static str, value: &mut Value) {
        let Some(old) = value.as_u64() else { return };
        let map = self.maps.entry(kind).or_default();
        let next = map.len() as u64;
        let new = *map.entry(old).or_insert(next);
        *value = Value::from(new);
    }
}

struct Walker {
    schema: &'static Schema,
    scopes: Vec<Ids>,
    /// The role each open ability's damage event calls its subject.
    damage_subjects: Vec<Option<Value>>,
    /// Keep ids and list order, for callers that read the text back out.
    keep_order: bool,
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
        _ => None,
    }
}

impl Walker {
    fn use_id(&self, kind: &'static str, value: &mut Value) {
        let Some(old) = value.as_u64() else { return };
        if self.keep_order {
            return;
        }
        for scope in self.scopes.iter().rev() {
            if let Some(new) = scope.maps.get(kind).and_then(|m| m.get(&old)) {
                *value = Value::from(*new);
                return;
            }
        }
    }

    fn walk(&mut self, value: &mut Value, ty: &Ty, defining: bool) {
        match ty {
            Ty::Opaque => {}
            Ty::Vec(inner) => {
                if let Value::Array(items) = value {
                    for item in items {
                        self.walk(item, inner, false);
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
        if let Some(kind) = id_kind(name) {
            if defining {
                if let (Some(scope), false) = (self.scopes.last_mut(), self.keep_order) {
                    scope.define(kind, value);
                }
            } else {
                self.use_id(kind, value);
            }
            return;
        }
        let scoped = name == "Ability";
        if scoped {
            self.scopes.push(Ids::default());
            let outer = self.damage_subjects.last().cloned().flatten();
            self.damage_subjects
                .push(damage_subject(&value["args"]["event"]).or(outer));
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
        }
    }

    fn variant(&mut self, value: &mut Value, name: &str, adjacent: bool) {
        let schema = self.schema;
        let variant = if adjacent {
            value.get("kind").and_then(Value::as_str).map(str::to_string)
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
                ("ObjectSet", "Zone") => narrow_zone_scope(content_of(value)),
                ("ObjectSet", "Battlefield") => drop_permanent_types(content_of(value)),
                ("ObjectRef", "Damage") => {
                    if self.damage_subjects.last().cloned().flatten().as_ref()
                        == value.get("args")
                    {
                        *value = serde_json::json!({"kind": "EventSubject"});
                    }
                }
                ("Recipients", "Union") => self.merge_permanent_groups(value),
                _ => {}
            }
        }
    }

    /// Damage to two groups of permanents is damage to the permanents of either.
    fn merge_permanent_groups(&self, value: &mut Value) {
        let Some(items) = value["args"].as_array() else {
            return;
        };
        let mut filters = Vec::with_capacity(items.len());
        for item in items {
            if item["kind"] != "Objects" || item["args"]["kind"] != "Battlefield" {
                return;
            }
            filters.push(item["args"]["args"].clone());
        }
        flatten(&mut filters, "Either");
        if !self.keep_order {
            filters.sort_by_cached_key(Value::to_string);
        }
        *value = serde_json::json!({"kind": "Objects", "args": {
            "kind": "Battlefield",
            "args": {"kind": "Either", "args": filters},
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
                for (field, ty) in types {
                    if let Some(item) = value.get_mut(field) {
                        self.walk(item, ty, defines(owner, field));
                    }
                }
            }
        }
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
            && ["Artifact", "Battle", "Creature", "Enchantment", "Land", "Planeswalker"]
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
    let mut walker = Walker {
        schema: schema()?,
        scopes: Vec::new(),
        damage_subjects: Vec::new(),
        keep_order,
    };
    walker.walk(value, &Ty::Vec(Box::new(Ty::Named("Ability".into()))), false);
    Ok(())
}

/// The canonical form of serialized abilities, in place.
pub fn abilities(value: &mut Value) -> Result<(), String> {
    normalize(value, false)
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
