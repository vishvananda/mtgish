//! Presentation is retained for printing and explicitly erased for meaning checks.
use crate::ir::{Ability, Keyword};
use crate::ir_schema::{Fields, Schema, Ty, TypeDef};
use serde_json::Value;

/// The semantic keyword under its per-instance reminder flag.
pub fn keyword(mut keyword: &Keyword) -> &Keyword {
    while let Keyword::WithReminder { keyword: inner, .. } = keyword {
        keyword = inner;
    }
    keyword
}

/// The semantic ability under its optional presentation wrapper.
pub fn ability(mut ability: &Ability) -> &Ability {
    while let Ability::WithPresentation { ability: inner, .. } = ability {
        ability = inner;
    }
    ability
}

pub(crate) fn validate_label(label: Option<&str>) -> Result<(), String> {
    if let Some(label) = label {
        if label.is_empty()
            || label.len() > 1024
            || label.trim() != label
            || label.chars().any(|c| c.is_control() || c == '—')
        {
            return Err("ability label must be nonempty, bounded and contain no separator".into());
        }
    }
    Ok(())
}

/// Only known typed positions are wrappers; an opaque operand can use the same
/// spelling without being an ability or keyword.
pub(crate) fn content<'a>(value: &'a mut Value, ty: &str) -> Option<&'a mut Value> {
    match ty {
        "Ability" if value.get("kind")?.as_str()? == "WithPresentation" => {
            value.get_mut("args")?.get_mut("ability")
        }
        "Keyword" if value.get("kind")?.as_str()? == "WithReminder" => {
            value.get_mut("args")?.get_mut("keyword")
        }
        _ => None,
    }
}

pub(crate) fn empty(value: &Value, ty: &str) -> bool {
    match ty {
        "Ability" => {
            value.get("kind").and_then(Value::as_str) == Some("WithPresentation")
                && value["args"]["label"].is_null()
                && value["args"]["reminder"] == false
        }
        "Keyword" => {
            value.get("kind").and_then(Value::as_str) == Some("WithReminder")
                && value["args"]["reminder"] == false
        }
        _ => false,
    }
}

fn bounds(value: &Value) -> Result<(), String> {
    let mut pending = vec![(value, 0)];
    let mut nodes = 0usize;
    let mut bytes = 0usize;
    while let Some((value, depth)) = pending.pop() {
        nodes += 1;
        if depth > 256 || nodes + pending.len() > 1_000_000 || bytes > 16 * 1024 * 1024 {
            return Err("presentation walk limit".into());
        }
        let mut push = |value| {
            if nodes + pending.len() >= 1_000_000 {
                return Err("presentation walk limit".to_string());
            }
            pending.push((value, depth + 1));
            Ok(())
        };
        match value {
            Value::Array(items) => {
                for item in items {
                    push(item)?;
                }
            }
            Value::Object(fields) => {
                for (key, value) in fields {
                    bytes = bytes.saturating_add(key.len());
                    push(value)?;
                }
            }
            Value::String(text) => bytes = bytes.saturating_add(text.len()),
            _ => {}
        }
        if bytes > 16 * 1024 * 1024 {
            return Err("presentation walk limit".into());
        }
    }
    Ok(())
}

struct Eraser {
    schema: &'static Schema,
    nodes: usize,
}

impl Eraser {
    fn walk(&mut self, value: &mut Value, ty: &Ty, depth: usize) -> Result<(), String> {
        self.nodes += 1;
        if depth > 256 || self.nodes > 1_000_000 {
            return Err("presentation walk limit".into());
        }
        match ty {
            Ty::Opaque => {}
            Ty::Vec(inner) => {
                if let Value::Array(items) = value {
                    for item in items {
                        self.walk(item, inner, depth + 1)?;
                    }
                }
            }
            Ty::Option(inner) if !value.is_null() => self.walk(value, inner, depth + 1)?,
            Ty::Option(_) => {}
            Ty::Tuple(parts) => {
                if let Value::Array(items) = value {
                    for (item, ty) in items.iter_mut().zip(parts) {
                        self.walk(item, ty, depth + 1)?;
                    }
                }
            }
            Ty::Named(name) => {
                if let Some(inner) = content(value, name) {
                    let mut inner = inner.take();
                    self.walk(&mut inner, ty, depth + 1)?;
                    *value = inner;
                    return Ok(());
                }
                match self.schema.types.get(name) {
                    Some(TypeDef::Struct(fields)) => self.fields(value, fields, depth + 1)?,
                    Some(TypeDef::Enum { adjacent, .. }) => {
                        let variant = if *adjacent {
                            value.get("kind").and_then(Value::as_str)
                        } else if let Some(name) = value.as_str() {
                            Some(name)
                        } else {
                            value
                                .as_object()
                                .and_then(|o| o.keys().next())
                                .map(String::as_str)
                        };
                        if let Some((variant, (_, fields))) = variant
                            .and_then(|v| self.schema.variant(name, v).map(|fields| (v, fields)))
                        {
                            let key = if *adjacent { "args" } else { variant };
                            let key = key.to_owned();
                            if let Some(inner) = value.get_mut(&key) {
                                self.fields(inner, fields, depth + 1)?;
                            }
                        }
                    }
                    None => {}
                }
            }
        }
        Ok(())
    }

    fn fields(&mut self, value: &mut Value, fields: &Fields, depth: usize) -> Result<(), String> {
        match fields {
            Fields::Unit => {}
            Fields::Tuple(parts) if parts.len() == 1 => self.walk(value, &parts[0], depth)?,
            Fields::Tuple(parts) => {
                if let Value::Array(items) = value {
                    for (item, ty) in items.iter_mut().zip(parts) {
                        self.walk(item, ty, depth)?;
                    }
                }
            }
            Fields::Named(parts) => {
                for (name, ty) in parts {
                    if let Some(inner) = value.get_mut(name) {
                        self.walk(inner, ty, depth)?;
                    }
                }
            }
        }
        Ok(())
    }
}

/// Remove presentation from serialized abilities, including nested grants and
/// keyword changes. This is a separate gate from presentation-preserving IR
/// equality; callers must not use it to verify an exact round trip.
pub fn erase_abilities(value: &mut Value) -> Result<(), String> {
    // Bound the iterative walk before a recursive serde_json::Value clone.
    bounds(value)?;
    let mut copy = value.clone();
    Eraser {
        schema: crate::canonical::schema()?,
        nodes: 0,
    }
    .walk(
        &mut copy,
        &Ty::Vec(Box::new(Ty::Named("Ability".into()))),
        0,
    )?;
    *value = copy;
    Ok(())
}

#[cfg(test)]
mod tests;
