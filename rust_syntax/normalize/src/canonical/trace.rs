//! Origin annotations exist only inside the mapped canonicalizer. They travel
//! with ID leaves through moves/clones; semantic comparisons ignore them.
use super::*;
use crate::{binding_map::SiteMap, ir::UnifiedCard};
use serde::{Deserialize, Serialize};

const ID: &str = "$mtgish_id";
const ORIGINS: &str = "$site_origins";

pub(super) fn number(v: &Value) -> Option<u64> {
    v.as_u64().or_else(|| v.get(ID).and_then(Value::as_u64))
}
pub(super) fn set_number(v: &mut Value, n: u64) {
    if v.get(ORIGINS).is_some() {
        v[ID] = json!(n);
    } else {
        *v = json!(n);
    }
}
pub(super) fn with_number(v: &Value, n: u64) -> Value {
    let mut v = v.clone();
    set_number(&mut v, n);
    v
}
pub(super) fn plain(v: &Value) -> Value {
    if v.get(ORIGINS).is_some() {
        if let Some(id) = number(v) {
            return json!(id);
        }
    }
    match v {
        Value::Array(items) => items.iter().map(plain).collect(),
        Value::Object(fields) => fields.iter().map(|(k, v)| (k.clone(), plain(v))).collect(),
        _ => v.clone(),
    }
}
pub(super) fn same(a: &Value, b: &Value, tracing: bool) -> bool {
    if tracing {
        plain(a) == plain(b)
    } else {
        a == b
    }
}
pub(super) fn key(v: &Value, tracing: bool) -> String {
    // Object insertion order is spelling metadata. Keep arrays positional:
    // only the owning commutative site chooses to sort its members.
    fn sorted(value: &Value) -> Value {
        match value {
            Value::Object(fields) => {
                let mut keys: Vec<_> = fields.keys().collect();
                keys.sort_unstable();
                Value::Object(
                    keys.into_iter()
                        .map(|key| (key.clone(), sorted(&fields[key])))
                        .collect(),
                )
            }
            Value::Array(items) => Value::Array(items.iter().map(sorted).collect()),
            scalar => scalar.clone(),
        }
    }
    if tracing {
        sorted(&plain(v)).to_string()
    } else {
        sorted(v).to_string()
    }
}
/// Equal semantic subtrees that coalesce retain origins from both inputs.
pub(super) fn merge(keep: &mut Value, other: &Value) {
    if number(keep).is_some() && (keep.get(ORIGINS).is_some() || other.get(ORIGINS).is_some()) {
        let mut origins: Vec<Value> = keep[ORIGINS].as_array().cloned().unwrap_or_default();
        origins.extend(other[ORIGINS].as_array().into_iter().flatten().cloned());
        origins.sort_by_key(|v| v.as_u64().unwrap_or_default());
        origins.dedup();
        let id = number(keep).unwrap();
        *keep = json!({(ID):id,(ORIGINS):origins});
        return;
    }
    match (keep, other) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, v) in a {
                if let Some(other) = b.get(k) {
                    merge(v, other);
                }
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            for (a, b) in a.iter_mut().zip(b) {
                merge(a, b);
            }
        }
        _ => (),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MappedCard {
    pub card: UnifiedCard,
    pub map: SiteMap,
}

impl MappedCard {
    pub fn from_json(input: &str) -> Result<Self, String> {
        let value = crate::json_input::wrapped_value(input)?;
        let mapped: Self = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
        if serde_json::to_value(&mapped).map_err(|e| e.to_string())? != value {
            return Err("unknown or missing mapped-card fields".into());
        }
        crate::validate::unified(&mapped.card)?;
        for face in &mapped.card.faces {
            crate::validate::card(face)?;
        }
        Ok(mapped)
    }
}

fn normalize_tree(value: &mut Value, tracing: bool) -> Result<(), String> {
    let mut failure = None;
    crate::binding_map::transform(
        super::schema()?,
        &Ty::Named("UnifiedCard".into()),
        value,
        &mut |ty, v| {
            if failure.is_some() {
                return false;
            }
            let single = *ty == Ty::Named("Ability".into());
            let list = matches!(ty,Ty::Vec(inner) if **inner==Ty::Named("Ability".into()));
            if !single && !list {
                return true;
            }
            if single {
                let mut wrapped = Value::Array(vec![v.take()]);
                match super::normalize_with_trace(&mut wrapped, false, tracing) {
                    Ok(()) => {
                        let items = wrapped.as_array_mut().unwrap();
                        if items.len() != 1 {
                            failure = Some("single ability rewrite changed multiplicity".into());
                        } else {
                            *v = items.pop().unwrap();
                        }
                    }
                    Err(e) => failure = Some(e),
                }
            } else if let Err(e) = super::normalize_with_trace(v, false, tracing) {
                failure = Some(e);
            }
            false
        },
        &mut |_, _| {},
    );
    failure.map_or(Ok(()), Err)
}
pub(super) fn unified(card: &UnifiedCard) -> Result<UnifiedCard, String> {
    let mut value = serde_json::to_value(card).map_err(|e| e.to_string())?;
    normalize_tree(&mut value, false)?;
    serde_json::from_value(value).map_err(|e| e.to_string())
}
pub(super) fn mapped(card: &UnifiedCard) -> Result<MappedCard, String> {
    crate::validate::unified(card)?;
    for face in &card.faces {
        crate::validate::card(face)?;
    }
    let ty = Ty::Named("UnifiedCard".into());
    let mut value = serde_json::to_value(card).map_err(|e| e.to_string())?;
    let before = crate::binding_map::sites(super::schema()?, &ty, &value);
    let source: Vec<_> = before.keys().cloned().collect();
    for (i, path) in source.iter().enumerate() {
        let leaf = value.pointer_mut(path).ok_or("missing source ID site")?;
        let id = leaf.as_u64().ok_or("source ID is not numeric")?;
        *leaf = json!({(ID):id,(ORIGINS):[i]});
    }
    normalize_tree(&mut value, true)?;
    let after = crate::binding_map::sites(super::schema()?, &ty, &value);
    let mut map = SiteMap {
        sites: source.iter().map(|p| (p.clone(), vec![])).collect(),
        introduced: vec![],
    };
    for path in after.keys() {
        let v = value.pointer(path).ok_or("missing canonical ID site")?;
        if let Some(origins) = v.get(ORIGINS).and_then(Value::as_array) {
            for origin in origins {
                let i = origin.as_u64().ok_or("invalid canonical origin")? as usize;
                let source = source.get(i).ok_or("unknown canonical origin")?;
                map.sites.get_mut(source).unwrap().push(path.clone());
            }
        } else {
            map.introduced.push(path.clone());
        }
    }
    if !map.introduced.is_empty() {
        return Err("canonical rewrite produced ID sites without origins".into());
    }
    let value = plain(&value);
    let expected = serde_json::to_value(unified(card)?).map_err(|e| e.to_string())?;
    if value != expected {
        return Err("origin tracing changed canonical output".into());
    }
    map.validate(&before, &after)?;
    let card: UnifiedCard = serde_json::from_value(value).map_err(|e| e.to_string())?;
    crate::validate::unified(&card)?;
    for face in &card.faces {
        crate::validate::card(face)?;
    }
    Ok(MappedCard { card, map })
}

// The typed movement normalizer supplies its own path moves. Retagging the
// resulting typed operation preserves both changed IDs and original identities.
pub(super) fn movement_groups(value: &mut Value) -> Result<(), String> {
    if !super::movement::needed(value) {
        return Ok(());
    }
    let mut expected = plain(value);
    super::movement_groups(&mut expected)?;
    let schema = super::schema()?;
    let ty = Ty::Vec(Box::new(Ty::Named("Ability".into())));
    let operation = Ty::Named("Operation".into());
    let mut failure = None;
    crate::binding_map::transform(schema, &ty, value, &mut |_, _| true, &mut |name, v| {
        if failure.is_some() || name != "Operation" || v["kind"] != "Together" {
            return;
        }
        let result = (|| -> Result<(), String> {
            let mut typed: crate::ir::Operation =
                serde_json::from_value(plain(v)).map_err(|e| e.to_string())?;
            let Some(map) = crate::movement::prepare_group(&mut typed, true) else {
                return Ok(());
            };
            let before = crate::binding_map::sites(schema, &operation, v);
            let mut after = serde_json::to_value(typed).map_err(|e| e.to_string())?;
            let mut seen = HashSet::new();
            for path in before.keys() {
                let (from, to) = map
                    .prefixes
                    .iter()
                    .filter(|(prefix, _)| {
                        path.as_str() == prefix || path.starts_with(&format!("{prefix}/"))
                    })
                    .max_by_key(|(prefix, _)| prefix.len())
                    .ok_or_else(|| format!("preparation rewrite omitted ID site {path}"))?;
                let destination = format!("{to}{}", &path[from.len()..]);
                if !seen.insert(destination.clone()) {
                    return Err("preparation rewrite merges ID sites unexpectedly".into());
                }
                let target = after
                    .pointer_mut(&destination)
                    .ok_or("missing prepared ID destination")?;
                let id = target.as_u64().ok_or("prepared ID is not numeric")?;
                *target = with_number(v.pointer(path).ok_or("missing preparation origin")?, id);
            }
            *v = after;
            Ok(())
        })();
        if let Err(error) = result {
            failure = Some(error);
        }
    });
    if let Some(error) = failure {
        return Err(error);
    }
    if plain(value) != expected {
        return Err("mapped movement prepass differs from native normalization".into());
    }
    Ok(())
}
