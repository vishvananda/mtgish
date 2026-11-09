//! Explicit transport of typed ID sites across a structural rewrite. Numeric
//! labels alone cannot identify a binding across independent or shadowed scopes.
use crate::ir_schema::{Fields, Schema, Ty, TypeDef};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BindingKind {
    Target,
    Result,
    Loop,
    Selection,
}
impl BindingKind {
    pub(crate) fn prefix(self) -> &'static str {
        match self {
            Self::Target => "t",
            Self::Result => "r",
            Self::Loop => "b",
            Self::Selection => "s",
        }
    }
    pub(crate) fn maximum(self) -> u64 {
        if self == Self::Target {
            u16::MAX as u64
        } else {
            u32::MAX as u64
        }
    }
    pub(crate) fn of_type(name: &str) -> Option<Self> {
        Some(match name {
            "Slot" | "TargetSlot" => Self::Target,
            "ResultId" => Self::Result,
            "Binding" | "LoopBinding" => Self::Loop,
            "SelectionId" => Self::Selection,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SiteMap {
    /// Every input ID leaf, including leaves removed by the rewrite. A leaf can
    /// move, be cloned, or be combined with another leaf at one output site.
    pub sites: BTreeMap<String, Vec<String>>,
    /// New output leaves that have no input origin. Omission is never inferred.
    pub introduced: Vec<String>,
}

pub(crate) fn family(name: &str) -> bool {
    BindingKind::of_type(name).is_some()
}
pub(crate) fn pointer(path: &str, key: &str) -> String {
    format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"))
}
pub(crate) fn sites(schema: &Schema, ty: &Ty, value: &Value) -> BTreeMap<String, BindingKind> {
    fn walk(
        schema: &Schema,
        ty: &Ty,
        v: &Value,
        path: &str,
        out: &mut BTreeMap<String, BindingKind>,
    ) {
        match ty {
            Ty::Named(name) if family(name) => {
                out.insert(path.into(), BindingKind::of_type(name).unwrap());
            }
            Ty::Vec(inner) => {
                if let Some(items) = v.as_array() {
                    for (i, v) in items.iter().enumerate() {
                        walk(schema, inner, v, &pointer(path, &i.to_string()), out);
                    }
                }
            }
            Ty::Option(inner) => {
                if !v.is_null() {
                    walk(schema, inner, v, path, out);
                }
            }
            Ty::Tuple(types) => {
                for (i, t) in types.iter().enumerate() {
                    walk(schema, t, &v[i], &pointer(path, &i.to_string()), out);
                }
            }
            Ty::Named(name) => match schema.types.get(name) {
                Some(TypeDef::Struct(f)) => fields(schema, f, v, path, out),
                Some(TypeDef::Enum { adjacent, variants }) => {
                    let name = if *adjacent {
                        v["kind"].as_str()
                    } else {
                        v.as_str().or_else(|| {
                            v.as_object()
                                .and_then(|m| m.keys().next().map(String::as_str))
                        })
                    };
                    if let Some((variant, f)) =
                        variants.iter().find(|(n, _)| Some(n.as_str()) == name)
                    {
                        let key = if *adjacent { "args" } else { variant };
                        fields(schema, f, &v[key], &pointer(path, key), out);
                    }
                }
                _ => (),
            },
            Ty::Opaque => (),
        }
    }
    fn fields(
        schema: &Schema,
        f: &Fields,
        v: &Value,
        path: &str,
        out: &mut BTreeMap<String, BindingKind>,
    ) {
        match f {
            Fields::Named(fields) => {
                for (name, t) in fields {
                    walk(schema, t, &v[name], &pointer(path, name), out);
                }
            }
            Fields::Tuple(types) => {
                for (i, t) in types.iter().enumerate() {
                    if types.len() == 1 {
                        walk(schema, t, v, path, out);
                    } else {
                        walk(schema, t, &v[i], &pointer(path, &i.to_string()), out);
                    }
                }
            }
            Fields::Unit => (),
        }
    }
    let mut out = BTreeMap::new();
    walk(schema, ty, value, "", &mut out);
    out
}
impl SiteMap {
    /// A complete no-change map, useful as the starting point of a migration.
    pub fn identity(card: &crate::ir::UnifiedCard) -> Result<Self, String> {
        let value = serde_json::to_value(card).map_err(|e| e.to_string())?;
        Ok(Self {
            sites: sites(
                crate::canonical::schema()?,
                &Ty::Named("UnifiedCard".into()),
                &value,
            )
            .into_keys()
            .map(|p| (p.clone(), vec![p]))
            .collect(),
            introduced: vec![],
        })
    }
    /// Compose input -> intermediate and intermediate -> output maps. Clones
    /// that later coalesce retain one physical destination; a site with any
    /// original origin is no longer classified as wholly introduced.
    pub fn then(&self, next: &Self) -> Result<Self, String> {
        fn outputs(map: &SiteMap) -> Result<BTreeSet<String>, String> {
            let mut covered = BTreeSet::new();
            for paths in map.sites.values() {
                let mut seen = BTreeSet::new();
                for p in paths {
                    if !seen.insert(p) {
                        return Err("duplicate destination for one input site".into());
                    }
                    covered.insert(p.clone());
                }
            }
            for p in &map.introduced {
                if !covered.insert(p.clone()) {
                    return Err("introduced site is duplicated or already mapped".into());
                }
            }
            Ok(covered)
        }
        let middle = outputs(self)?;
        outputs(next)?;
        if middle.iter().ne(next.sites.keys()) {
            return Err("maps describe different intermediate ID sites".into());
        }
        let mut result = Self::default();
        for (source, middle) in &self.sites {
            let paths: BTreeSet<_> = middle
                .iter()
                .flat_map(|p| next.sites[p].iter().cloned())
                .collect();
            result
                .sites
                .insert(source.clone(), paths.into_iter().collect());
        }
        let mapped: BTreeSet<_> = result.sites.values().flatten().cloned().collect();
        result.introduced = next
            .introduced
            .iter()
            .cloned()
            .chain(
                self.introduced
                    .iter()
                    .flat_map(|p| next.sites[p].iter().cloned()),
            )
            .filter(|p| !mapped.contains(p))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Ok(result)
    }
    pub(crate) fn validate(
        &self,
        before: &BTreeMap<String, BindingKind>,
        after: &BTreeMap<String, BindingKind>,
    ) -> Result<(), String> {
        if self.sites.keys().ne(before.keys()) {
            return Err("site map must account for every input ID leaf exactly once".into());
        }
        let mut covered = BTreeSet::new();
        for (source, destinations) in &self.sites {
            let mut seen = BTreeSet::new();
            for destination in destinations {
                if !seen.insert(destination) {
                    return Err("duplicate destination for one input site".into());
                }
                if after.get(destination) != before.get(source) {
                    return Err(format!(
                        "site map has the wrong ID type or output path: {destination}"
                    ));
                }
                covered.insert(destination.clone());
            }
        }
        for path in &self.introduced {
            if !after.contains_key(path) || !covered.insert(path.clone()) {
                return Err("introduced ID site is missing, duplicated or already mapped".into());
            }
        }
        if covered.iter().ne(after.keys()) {
            return Err("site map must account for every output ID leaf".into());
        }
        Ok(())
    }
}

/// Postorder structural access through the actual Rust wire declarations.
pub(crate) fn transform(
    schema: &Schema,
    ty: &Ty,
    value: &mut Value,
    before: &mut impl FnMut(&Ty, &mut Value) -> bool,
    after: &mut impl FnMut(&str, &mut Value),
) {
    fn fields(
        schema: &Schema,
        f: &Fields,
        v: &mut Value,
        before: &mut impl FnMut(&Ty, &mut Value) -> bool,
        after: &mut impl FnMut(&str, &mut Value),
    ) {
        match f {
            Fields::Named(fields) => {
                for (name, t) in fields {
                    if let Some(v) = v.get_mut(name) {
                        transform(schema, t, v, before, after);
                    }
                }
            }
            Fields::Tuple(types) if types.len() == 1 => {
                transform(schema, &types[0], v, before, after)
            }
            Fields::Tuple(types) => {
                if let Some(items) = v.as_array_mut() {
                    for (t, v) in types.iter().zip(items) {
                        transform(schema, t, v, before, after);
                    }
                }
            }
            Fields::Unit => (),
        }
    }
    if !before(ty, value) {
        return;
    }
    match ty {
        Ty::Vec(t) => {
            if let Some(items) = value.as_array_mut() {
                for v in items {
                    transform(schema, t, v, before, after);
                }
            }
        }
        Ty::Option(t) => {
            if !value.is_null() {
                transform(schema, t, value, before, after);
            }
        }
        Ty::Tuple(types) => {
            if let Some(items) = value.as_array_mut() {
                for (t, v) in types.iter().zip(items) {
                    transform(schema, t, v, before, after);
                }
            }
        }
        Ty::Named(name) => {
            if family(name) {
                return;
            }
            match schema.types.get(name) {
                Some(TypeDef::Struct(f)) => fields(schema, f, value, before, after),
                Some(TypeDef::Enum { adjacent, variants }) => {
                    let variant = if *adjacent {
                        value["kind"].as_str()
                    } else {
                        value.as_str().or_else(|| {
                            value
                                .as_object()
                                .and_then(|m| m.keys().next().map(String::as_str))
                        })
                    }
                    .map(str::to_owned);
                    if let Some((variant, f)) =
                        variants.iter().find(|(v, _)| Some(v) == variant.as_ref())
                    {
                        let key = if *adjacent { "args" } else { variant };
                        if let Some(v) = value.get_mut(key) {
                            fields(schema, f, v, before, after);
                        }
                    }
                }
                _ => (),
            }
            after(name, value);
        }
        Ty::Opaque => (),
    }
}
