//! Explicit upgrades for persisted normalized JSON. Source grammar flags are
//! decoded only at this input boundary; current validated IR is fully typed.
use crate::movement::legacy::{EntrySpec, EntryState};
use crate::{ir::*, validate};
use serde_json::{json, Value};
mod api_names;
mod coin_payloads;
mod entry_payloads;
mod keyword_payloads;
mod mode_payloads;
mod movement_payloads;
mod schema;
mod typed;
pub use typed::{mapped_read_json, MappedMigration};

pub(crate) fn input_sites(
    version: u32,
    value: &Value,
) -> Result<std::collections::BTreeMap<String, crate::binding_map::BindingKind>, String> {
    Ok(crate::binding_map::sites(
        schema::input(version)?,
        &crate::ir_schema::Ty::Named("UnifiedCard".into()),
        value,
    ))
}

const ENTRY_OPERATIONS: &[&str] = &[
    "Conjure",
    "MoveTopCard",
    "Manifest",
    "Meld",
    "CounterWith",
    "Populate",
];

/// Load exact current JSON or explicitly upgrade the preceding wire contract.
/// Unknown fields and unsupported epochs are errors, including during upgrade.
pub fn read_json(input: &str) -> Result<UnifiedCard, String> {
    let value = crate::json_input::value(input)?;
    match value.get("schema_version").and_then(Value::as_u64) {
        Some(version) if version == u64::from(SCHEMA_VERSION) => (),
        Some(179 | 180 | 181 | 182 | 183 | 184 | 185) => {
            return Ok(typed::upgrade(value, false)?.card)
        }
        _ => return Err("unsupported AST schema version for migration".into()),
    }
    checked(value)
}

fn checked(value: Value) -> Result<UnifiedCard, String> {
    let card: UnifiedCard = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
    if serde_json::to_value(&card).map_err(|e| e.to_string())? != value {
        return Err("unknown or missing IR fields during migration".into());
    }
    validate::unified(&card)?;
    for face in &card.faces {
        validate::card(face)?;
    }
    Ok(card)
}

/// Only the unversioned authoring compatibility reader uses these descriptors.
/// Portable current IR never includes the historical entry union.
pub(crate) fn legacy_authoring_type(name: &str) -> Option<&'static crate::ir_schema::TypeDef> {
    schema::input(185).ok()?.types.get(name)
}
pub(crate) fn upgrade_authoring_movement(
    owner: &str,
    value: &mut Value,
) -> Result<Vec<(String, String)>, String> {
    Ok(movement_payloads::variant(owner, value, "")?
        .into_iter()
        .flat_map(|r| r.moves)
        .collect())
}
