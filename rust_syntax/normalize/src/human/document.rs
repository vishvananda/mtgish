//! Names belong to the authoring document, never to semantic equality or runtime
//! lookup. Sites address typed ID leaves, so the same numeric ID in two abilities
//! can have different names. The temporary tagged projection is codec-only.
use super::{checked, codec, schema, syntax};
use crate::{ir::*, ir_schema::Ty};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashSet};
mod migration;
mod transport;

pub use crate::binding_map::BindingKind;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingName {
    pub kind: BindingKind,
    /// A document-local lexical scope, independent of the numeric engine ID.
    pub scope: u32,
    /// Readable qualification when alternatives reuse a receipt's local name.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub scope_name: String,
    pub name: String,
    /// Explicit names on unused results must survive formatting too.
    pub explicit: bool,
    /// A numeric join operand can denote several mutually exclusive receipts.
    /// Such an operand is an expression over these definitions, not a new local.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingUse {
    /// JSON pointer into the exact semantic card, ending at an ID leaf.
    pub path: String,
    pub binding: usize,
    /// An explicit lexical qualification on a reference, never a declaration.
    #[serde(default, skip_serializing_if = "is_false")]
    pub qualified: bool,
}
fn is_false(value: &bool) -> bool {
    !*value
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub version: u32,
    card: UnifiedCard,
    bindings: Vec<BindingName>,
    uses: Vec<BindingUse>,
}

pub(super) fn tagged(id: u64, binding: usize) -> Value {
    json!({"$mtgish_id":id,"binding":binding})
}
pub(super) fn id_number(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.get("$mtgish_id").and_then(Value::as_u64))
}
pub(super) fn untag(value: &Value) -> Value {
    if let Some(id) = value.get("$mtgish_id").and_then(Value::as_u64) {
        return json!(id);
    }
    match value {
        Value::Object(fields) => fields.iter().map(|(k, v)| (k.clone(), untag(v))).collect(),
        Value::Array(items) => items.iter().map(untag).collect(),
        _ => value.clone(),
    }
}
fn pointer(path: &str, key: &str) -> String {
    format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"))
}
pub(super) fn extract(
    value: &mut Value,
    path: &str,
    uses: &mut Vec<BindingUse>,
) -> Result<(), String> {
    if let Some(id) = value.get("$mtgish_id").and_then(Value::as_u64) {
        let binding = value["binding"]
            .as_u64()
            .ok_or("invalid codec binding marker")? as usize;
        uses.push(BindingUse {
            path: path.into(),
            binding,
            qualified: value
                .get("qualified")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        });
        *value = json!(id);
        return Ok(());
    }
    match value {
        Value::Object(fields) => {
            for (key, v) in fields {
                extract(v, &pointer(path, key), uses)?;
            }
        }
        Value::Array(items) => {
            for (i, v) in items.iter_mut().enumerate() {
                extract(v, &pointer(path, &i.to_string()), uses)?;
            }
        }
        _ => (),
    }
    Ok(())
}
fn id_sites(ty: &Ty, value: &Value, path: &str, sites: &mut BTreeMap<String, BindingKind>) {
    for (relative, family) in crate::binding_map::sites(schema(), ty, value) {
        sites.insert(format!("{path}{relative}"), family);
    }
}
impl Document {
    pub fn card(&self) -> &UnifiedCard {
        &self.card
    }
    pub fn bindings(&self) -> &[BindingName] {
        &self.bindings
    }
    pub fn uses(&self) -> &[BindingUse] {
        &self.uses
    }
    /// Rebind a compiler's pure ID renumbering. Structural rewrites require an
    /// explicit site mapping; accepting them by position would lose author intent.
    pub fn rebind_ids(&self, card: &UnifiedCard) -> Result<Self, String> {
        checked(card)?;
        fn shape(card: &UnifiedCard) -> Result<Value, String> {
            let mut value = serde_json::to_value(card).map_err(|e| e.to_string())?;
            let mut sites = BTreeMap::new();
            id_sites(&Ty::Named("UnifiedCard".into()), &value, "", &mut sites);
            for path in sites.keys() {
                *value.pointer_mut(path).ok_or("missing ID site")? = Value::Null;
            }
            Ok(value)
        }
        if shape(&self.card)? != shape(card)? {
            return Err("structural card edits require a binding-site mapping".into());
        }
        if meaning(&self.card)? != meaning(card)? {
            return Err("ID remapping changes the card's meaning".into());
        }
        let mut rebound = self.clone();
        rebound.card = card.clone();
        rebound.write()?;
        Ok(rebound)
    }
    pub fn from_json(input: &str) -> Result<Self, String> {
        let mut value = crate::json_input::wrapped_value(input)?;
        if matches!(value["card"]["schema_version"].as_u64(), Some(180 | 181 | 182 | 183)) {
            migration::upgrade_schema(&mut value)?;
        }
        let mut doc: Self = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
        if serde_json::to_value(&doc).map_err(|e| e.to_string())? != value {
            return Err("unknown or missing authoring document fields".into());
        }
        if doc.version == 1 {
            doc = doc.upgrade_v1()?;
        }
        doc.project()?;
        Ok(doc)
    }
    /// Format with names after checking the annotation sites against Rust types.
    pub fn write(&self) -> Result<String, String> {
        let projected = self.project()?;
        let code = codec::write_with_bindings(&projected, &self.bindings)? + "\n";
        let back = super::read(&code)?;
        let expected = meaning(&self.card)?;
        let actual = meaning(&back)?;
        if let Some(path) = first_difference(&expected, &actual, "") {
            return Err(format!(
                "binding names change the card's meaning or reference scope at {path}"
            ));
        }
        Ok(code)
    }
    pub fn rename(&mut self, binding: usize, name: &str) -> Result<(), String> {
        self.rename_many(&[(binding, name.to_owned())])
    }
    /// A rename transaction updates every use and rolls back on any collision.
    pub fn rename_many(&mut self, names: &[(usize, String)]) -> Result<(), String> {
        let mut updated = self.bindings.clone();
        let mut seen = HashSet::new();
        for (index, name) in names {
            if !seen.insert(*index) {
                return Err("duplicate binding in rename transaction".into());
            }
            if !codec::valid_binding_name(name) {
                return Err(
                    "a binding name must be an identifier of at most 128 characters".into(),
                );
            }
            let binding = updated.get_mut(*index).ok_or("unknown binding")?;
            if !binding.members.is_empty() {
                return Err(
                    "a receipt group is an expression; rename its branch declarations".into(),
                );
            }
            binding.name = name.clone();
            binding.explicit = true;
        }
        let old = std::mem::replace(&mut self.bindings, updated);
        if let Err(error) = self.write() {
            self.bindings = old;
            return Err(error);
        }
        Ok(())
    }
    /// A typed, temporary projection for the editor/codec; never engine IR.
    pub(super) fn project(&self) -> Result<Value, String> {
        if self.version != 2 {
            return Err("unsupported authoring document version".into());
        }
        checked(&self.card)?;
        let value = serde_json::to_value(&self.card).map_err(|e| e.to_string())?;
        let mut sites = BTreeMap::new();
        id_sites(&Ty::Named("UnifiedCard".into()), &value, "", &mut sites);
        let resolved = resolve_card(&self.card)?;
        self.project_against(value, sites, &resolved, true)
    }
    fn project_against(
        &self,
        mut value: Value,
        sites: BTreeMap<String, BindingKind>,
        resolved: &Document,
        check_scopes: bool,
    ) -> Result<Value, String> {
        let groups: BTreeMap<_, _> = resolved
            .uses
            .iter()
            .map(|site| (site.path.as_str(), site.binding))
            .collect();
        let (mut assigned, mut reverse) = (BTreeMap::new(), BTreeMap::new());
        let mut seen = HashSet::new();
        for site in &self.uses {
            if !seen.insert(&site.path) {
                return Err("duplicate binding annotation site".into());
            }
            let binding = self
                .bindings
                .get(site.binding)
                .ok_or("unknown binding annotation")?;
            if !codec::valid_binding_name(&binding.name) {
                return Err("a binding name must be an identifier".into());
            }
            if sites.get(&site.path) != Some(&binding.kind) {
                return Err(format!(
                    "binding annotation has the wrong type or path: {}",
                    site.path
                ));
            }
            if site.qualified && binding.kind != BindingKind::Result {
                return Err("only result references have receipt qualifications".into());
            }
            let group = *groups
                .get(site.path.as_str())
                .ok_or("unresolved binding annotation")?;
            if assigned
                .insert(site.binding, group)
                .is_some_and(|prior| prior != group)
                || reverse
                    .insert(group, site.binding)
                    .is_some_and(|prior| prior != site.binding)
            {
                return Err(
                    "binding annotation crosses a lexical scope or splits one binding".into(),
                );
            }
            let leaf = value
                .pointer_mut(&site.path)
                .ok_or("missing binding annotation site")?;
            let id = leaf
                .as_u64()
                .ok_or("binding annotation must address an ID")?;
            *leaf = tagged(id, site.binding);
            if site.qualified {
                leaf["qualified"] = json!(true);
            }
        }
        for (index, binding) in self.bindings.iter().enumerate() {
            let mut members = BTreeSet::new();
            for member in &binding.members {
                if *member == index
                    || !members.insert(*member)
                    || self
                        .bindings
                        .get(*member)
                        .is_none_or(|b| b.kind != BindingKind::Result || !b.members.is_empty())
                {
                    return Err("receipt group must contain distinct result definitions".into());
                }
            }
            if !members.is_empty()
                && (binding.kind != BindingKind::Result || members.len() < 2 || binding.explicit)
            {
                return Err(
                    "a receipt group is an unnamed expression over alternative results".into(),
                );
            }
            let Some(group) = assigned.get(&index) else {
                continue;
            };
            let expected = &resolved.bindings[*group];
            if check_scopes
                && (binding.scope != expected.scope || binding.scope_name != expected.scope_name)
            {
                return Err("binding annotation has inconsistent lexical scope metadata".into());
            }
            let actual = members
                .iter()
                .map(|member| {
                    assigned
                        .get(member)
                        .copied()
                        .ok_or("receipt member has no definition")
                })
                .collect::<Result<BTreeSet<_>, _>>()?;
            if actual != expected.members.iter().copied().collect() {
                return Err(
                    "receipt group members cross a branch scope or omit an alternative".into(),
                );
            }
        }
        Ok(value)
    }
}
/// Start an authoring document from validated IR. IDs in the caller's card are
/// retained even when the compact codec elides unused result declarations.
pub fn from_card(card: &UnifiedCard) -> Result<Document, String> {
    resolve_card(card)
}
fn resolve_card(card: &UnifiedCard) -> Result<Document, String> {
    let code = super::write(card)?;
    let mut doc = read(&code)?;
    doc.card = card.clone();
    let value = serde_json::to_value(card).map_err(|e| e.to_string())?;
    for site in &doc.uses {
        let binding = &mut doc.bindings[site.binding];
        binding.explicit = false;
        if let Some(id) = value.pointer(&site.path).and_then(Value::as_u64) {
            let prefix = binding.kind.prefix();
            binding.name = if binding.members.is_empty() {
                format!("{prefix}{id}")
            } else {
                format!("r{id}_receipts")
            };
        }
    }
    Ok(doc)
}
pub fn read(text: &str) -> Result<Document, String> {
    let node = syntax::parse(text)?;
    let (mut value, bindings) = codec::read_with_bindings(&node)?;
    let mut uses = Vec::new();
    extract(&mut value, "", &mut uses)?;
    let card: UnifiedCard = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
    if serde_json::to_value(&card).map_err(|e| e.to_string())? != value {
        return Err("unknown or missing card-code fields".into());
    }
    checked(&card)?;
    Ok(Document {
        version: 2,
        card,
        bindings,
        uses,
    })
}
fn meaning(card: &UnifiedCard) -> Result<Value, String> {
    serde_json::to_value(crate::canonical::unified(card)?).map_err(|e| e.to_string())
}

fn first_difference(a: &Value, b: &Value, path: &str) -> Option<String> {
    if a == b {
        return None;
    }
    match (a, b) {
        (Value::Object(a), Value::Object(b)) if a.keys().eq(b.keys()) => a
            .iter()
            .find_map(|(key, a)| first_difference(a, &b[key], &pointer(path, key))),
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => a
            .iter()
            .zip(b)
            .enumerate()
            .find_map(|(i, (a, b))| first_difference(a, b, &pointer(path, &i.to_string()))),
        _ => Some(format!("{path}: expected {a}, got {b}")),
    }
}
