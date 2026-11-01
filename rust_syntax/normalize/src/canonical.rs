//! The normal form equality needs: result, slot and binding ids renumbered in
//! the order they are defined, and commutative lists sorted. Two cards that
//! differ only in lowering order are equal here. The walk reads the IR's own
//! type table, so a new field of an id type is covered without a change here.
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
                if let Some(scope) = self.scopes.last_mut() {
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
        }
        let schema = self.schema;
        match schema.types.get(name) {
            Some(TypeDef::Struct(fields)) => self.fields(value, fields, name),
            Some(TypeDef::Enum { adjacent, .. }) => self.variant(value, name, *adjacent),
            None => {}
        }
        if scoped {
            self.scopes.pop();
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
                    items.sort_by_cached_key(Value::to_string);
                }
            }
        }
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

/// Every other position of an id type reads an id defined at one of these.
fn defines(owner: &str, field: &str) -> bool {
    field == "binding"
        || (owner == "TargetSpec" && field == "slot")
        || (owner == "Instruction::Effect" && field == "result")
}

/// The canonical form of serialized abilities, in place.
pub fn abilities(value: &mut Value) -> Result<(), String> {
    let mut walker = Walker {
        schema: schema()?,
        scopes: Vec::new(),
    };
    walker.walk(value, &Ty::Vec(Box::new(Ty::Named("Ability".into()))), false);
    Ok(())
}

pub fn card(card: &Card) -> Result<Card, String> {
    let mut value = serde_json::to_value(card).map_err(|e| e.to_string())?;
    let abilities = value.get_mut("abilities").ok_or("card has no abilities")?;
    self::abilities(abilities)?;
    serde_json::from_value(value).map_err(|e| e.to_string())
}
