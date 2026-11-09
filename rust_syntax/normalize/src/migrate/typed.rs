//! Bottom-up upgrades against immutable input epochs, with explicit ID-site origins.
use super::*;
use crate::{
    binding_map::{self, SiteMap},
    ir_schema::{Fields, Schema, Ty, TypeDef},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MappedMigration {
    pub source_version: u32,
    pub card: UnifiedCard,
    pub map: SiteMap,
}

/// Origins address the exact supplied epoch. The older 179 entry-vector
/// migration remains available through `read_json`, but never masquerades as
/// an identity mapping from those vectors to the later authoring structures.
pub fn mapped_read_json(input: &str) -> Result<MappedMigration, String> {
    let value = crate::json_input::value(input)?;
    match value["schema_version"].as_u64() {
        Some(180 | 181) => upgrade(value, true),
        Some(v) if v == u64::from(SCHEMA_VERSION) => {
            let card = super::checked(value.clone())?;
            let sites = binding_map::sites(
                super::schema::input(SCHEMA_VERSION)?,
                &Ty::Named("UnifiedCard".into()),
                &value,
            );
            Ok(MappedMigration {
                source_version: SCHEMA_VERSION,
                card,
                map: SiteMap {
                    sites: sites.keys().map(|p| (p.clone(), vec![p.clone()])).collect(),
                    introduced: vec![],
                },
            })
        }
        _ => Err("mapped migration requires schema 180, 181 or the current epoch".into()),
    }
}
pub(super) fn upgrade(mut value: Value, track: bool) -> Result<MappedMigration, String> {
    let version = value["schema_version"]
        .as_u64()
        .ok_or("legacy IR requires a schema version")? as u32;
    if !matches!(version, 179 | 180 | 181)
        || value["faces"]
            .as_array()
            .ok_or("legacy IR requires faces")?
            .iter()
            .any(|face| face["schema_version"] != version)
    {
        return Err(format!("legacy IR faces must use schema {version}"));
    }
    let schema = super::schema::input(version)?;
    let ty = Ty::Named("UnifiedCard".into());
    let before = binding_map::sites(schema, &ty, &value);
    let map = track.then(|| SiteMap {
        sites: before
            .keys()
            .map(|p| (p.clone(), vec![p.clone()]))
            .collect(),
        introduced: vec![],
    });
    let mut walker = Walker {
        schema,
        map,
        version,
        work: 0,
    };
    walker.node(&ty, &mut value, "")?;
    let card = super::checked(value.clone())?;
    let map = walker.map.unwrap_or_default();
    if track {
        map.validate(
            &before,
            &binding_map::sites(super::schema::input(SCHEMA_VERSION)?, &ty, &value),
        )?;
    }
    Ok(MappedMigration {
        source_version: version,
        card,
        map,
    })
}

fn args_path(path: &str, adjacent: bool, variant: &str) -> String {
    binding_map::pointer(path, if adjacent { "args" } else { variant })
}
fn suffix<'a>(path: &'a str, prefix: &str) -> Option<&'a str> {
    path.strip_prefix(prefix)
        .filter(|rest| rest.is_empty() || rest.starts_with('/'))
}
struct Walker<'a> {
    schema: &'a Schema,
    map: Option<SiteMap>,
    version: u32,
    work: usize,
}
impl Walker<'_> {
    fn node(&mut self, ty: &Ty, value: &mut Value, path: &str) -> Result<(), String> {
        self.work += 1;
        if self.work > 2_000_000 || path.bytes().filter(|b| *b == b'/').count() > 255 {
            return Err("typed migration walk limit exceeded".into());
        }
        match ty {
            Ty::Option(inner) => {
                if !value.is_null() {
                    self.node(inner, value, path)?;
                }
            }
            Ty::Vec(inner) => {
                for (i, v) in value
                    .as_array_mut()
                    .ok_or_else(|| format!("{path}: expected a list"))?
                    .iter_mut()
                    .enumerate()
                {
                    self.node(inner, v, &binding_map::pointer(path, &i.to_string()))?;
                }
            }
            Ty::Tuple(items) => {
                let values = value.as_array_mut().ok_or("expected a typed tuple")?;
                if items.len() != values.len() {
                    return Err(format!("{path}: tuple length mismatch"));
                }
                for (i, (ty, v)) in items.iter().zip(values).enumerate() {
                    self.node(ty, v, &binding_map::pointer(path, &i.to_string()))?;
                }
            }
            Ty::Opaque => {}
            Ty::Named(name) => {
                let Some(descriptor) = self.schema.types.get(name) else {
                    return Ok(());
                };
                match descriptor {
                    TypeDef::Struct(fields) => {
                        self.fields(fields, value, path)?;
                        if matches!(name.as_str(), "Card" | "UnifiedCard") {
                            value["schema_version"] = json!(SCHEMA_VERSION);
                        }
                        if self.version == 179 && name == "EntrySpec" {
                            value["controller"] = Value::Null;
                        }
                        let regions = super::entry_payloads::structure(name, value, path)?;
                        self.regions(regions);
                    }
                    TypeDef::Enum { adjacent, variants } => {
                        let variant = if *adjacent {
                            value["kind"].as_str()
                        } else {
                            value.as_str().or_else(|| {
                                value
                                    .as_object()
                                    .filter(|o| o.len() == 1)
                                    .and_then(|o| o.keys().next().map(String::as_str))
                            })
                        }
                        .ok_or_else(|| format!("{path}: expected {name} constructor"))?
                        .to_string();
                        let fields = &variants
                            .iter()
                            .find(|(n, _)| n == &variant)
                            .ok_or_else(|| format!("{path}: unknown {name}::{variant}"))?
                            .1;
                        if matches!(fields, Fields::Unit) {
                            if *adjacent && value.as_object().is_none_or(|o| o.len() != 1)
                                || !*adjacent && !value.is_string()
                            {
                                return Err(format!("{path}: unexpected unit-variant fields"));
                            }
                            return Ok(());
                        }
                        if *adjacent && value.as_object().is_none_or(|o| o.len() != 2) {
                            return Err(format!("{path}: unexpected enum fields"));
                        }
                        let args = value
                            .get_mut(if *adjacent { "args" } else { &variant })
                            .ok_or_else(|| format!("{path}: missing variant arguments"))?;
                        let owner = format!("{name}::{variant}");
                        let at = args_path(path, *adjacent, &variant);
                        self.fields(fields, args, &at)?;
                        if self.version <= 180 {
                            if let Some((field, single)) = copy_field(&owner) {
                                self.copy(args, &at, field, single)?;
                            }
                        }
                        if self.version == 179
                            && name == "Operation"
                            && super::ENTRY_OPERATIONS.contains(&variant.as_str())
                        {
                            let flags: Vec<Operand> = serde_json::from_value(args["entry"].clone())
                                .map_err(|e| e.to_string())?;
                            args["entry"] =
                                serde_json::to_value(crate::movement::entry_from_source(&flags))
                                    .map_err(|e| e.to_string())?;
                        }
                        let regions = super::entry_payloads::variant(&owner, value, path)?;
                        self.regions(regions);
                    }
                }
            }
        }
        Ok(())
    }
    fn fields(&mut self, fields: &Fields, value: &mut Value, path: &str) -> Result<(), String> {
        match fields {
            Fields::Unit => {}
            Fields::Tuple(items) if items.len() == 1 => self.node(&items[0], value, path)?,
            Fields::Tuple(items) => self.node(&Ty::Tuple(items.clone()), value, path)?,
            Fields::Named(fields) => {
                let object = value
                    .as_object_mut()
                    .ok_or_else(|| format!("{path}: expected named fields"))?;
                let expected: BTreeSet<_> = fields.iter().map(|(n, _)| n.as_str()).collect();
                let actual: BTreeSet<_> = object.keys().map(String::as_str).collect();
                if actual != expected {
                    return Err(format!("{path}: unknown or missing legacy IR fields"));
                }
                for (name, ty) in fields {
                    self.node(
                        ty,
                        object.get_mut(name).unwrap(),
                        &binding_map::pointer(path, name),
                    )?;
                }
            }
        }
        Ok(())
    }
    fn regions(&mut self, regions: Vec<super::entry_payloads::Region>) {
        if let Some(map) = &mut self.map {
            for region in regions {
                remap(map, &region.source, &region.moves);
            }
        }
    }
    fn copy(
        &mut self,
        args: &mut Value,
        path: &str,
        field: &str,
        single: bool,
    ) -> Result<(), String> {
        let raw = args
            .get(field)
            .ok_or("missing legacy copy payload")?
            .clone();
        let input = if single { json!([raw]) } else { raw };
        let operands: Vec<Operand> =
            serde_json::from_value(input.clone()).map_err(|e| format!("{path}/{field}: {e}"))?;
        if serde_json::to_value(&operands).map_err(|e| e.to_string())? != input {
            return Err("unknown or missing fields in legacy copy exceptions".into());
        }
        let converted =
            crate::copying::convert(&operands).map_err(|e| format!("{path}/{field}: {e}"))?;
        let source = binding_map::pointer(path, field);
        let target = binding_map::pointer(path, "exceptions");
        if let Some(map) = &mut self.map {
            let moves = converted
                .moves
                .iter()
                .map(|(a, b)| {
                    let a = if single {
                        a.strip_prefix("/0")
                            .ok_or("invalid single-payload origin")?
                    } else {
                        a.as_str()
                    };
                    Ok((format!("{source}{a}"), format!("{target}{b}")))
                })
                .collect::<Result<Vec<_>, String>>()?;
            remap(map, &source, &moves);
        }
        let fields = args
            .as_object_mut()
            .ok_or("copy arguments require a record")?;
        fields.remove(field);
        fields.insert(
            "exceptions".into(),
            serde_json::to_value(converted.exceptions).map_err(|e| e.to_string())?,
        );
        Ok(())
    }
}
fn remap(map: &mut SiteMap, source: &str, moves: &[(String, String)]) {
    for destinations in map.sites.values_mut() {
        let mut next = vec![];
        for destination in destinations.iter() {
            if suffix(destination, source).is_none() {
                next.push(destination.clone());
                continue;
            }
            for (from, to) in moves {
                if let Some(rest) = suffix(destination, from) {
                    next.push(format!("{to}{rest}"));
                }
            }
        }
        next.sort();
        next.dedup();
        *destinations = next;
    }
}
fn copy_field(owner: &str) -> Option<(&'static str, bool)> {
    match owner {
        "TokenRecipe::Copy"
        | "TokenRecipe::CopyEach"
        | "TokenRecipe::CopyOne"
        | "TokenRecipe::CopyNamed" => Some(("adjustments", false)),
        "Change::CopyOf" | "EntryState::CopyOf" => Some(("effects", true)),
        "Operation::CopyOnStack" => Some(("effects", false)),
        _ => None,
    }
}
