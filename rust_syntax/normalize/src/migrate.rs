//! Explicit upgrades for persisted normalized JSON. Source grammar flags are
//! decoded only at this input boundary; current validated IR is fully typed.
use crate::{ir::*, validate};
use serde_json::{json, Value};
mod entry_payloads;
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
        Some(179 | 180 | 181) => return Ok(typed::upgrade(value, false)?.card),
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
