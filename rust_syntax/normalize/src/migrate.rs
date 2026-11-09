//! Explicit upgrades for persisted normalized JSON. Source grammar flags are
//! decoded only at this input boundary; current validated IR is fully typed.
use crate::{ir::*, validate};
use serde::Deserialize;
use serde_json::{json, Value};

const ENTRY_OPERATIONS: &[&str] = &[
    "Conjure",
    "MoveTopCard",
    "Manifest",
    "Meld",
    "CounterWith",
    "Populate",
];

fn upgrade_179(value: &mut Value, depth: usize) -> Result<(), String> {
    if depth > 255 {
        return Err("IR migration nesting exceeds 255 levels".into());
    }
    match value {
        Value::Array(items) => {
            for item in items {
                upgrade_179(item, depth + 1)?;
            }
        }
        Value::Object(fields) => {
            if fields
                .get("kind")
                .and_then(Value::as_str)
                .is_some_and(|kind| ENTRY_OPERATIONS.contains(&kind))
            {
                if let Some(args) = fields.get_mut("args").and_then(Value::as_object_mut) {
                    if let Some(entry) = args.get_mut("entry") {
                        if entry.is_array() {
                            let flags: Vec<Operand> =
                                serde_json::from_value(entry.clone()).map_err(|e| e.to_string())?;
                            *entry =
                                serde_json::to_value(crate::movement::entry_from_source(&flags))
                                    .map_err(|e| e.to_string())?;
                        }
                    }
                }
            }
            // All pre-existing EntrySpec fields are retained, including their
            // absence/zero distinctions; only the new optional override is added.
            if fields.contains_key("tapped")
                && fields.contains_key("counters")
                && fields.contains_key("states")
            {
                fields.entry("controller").or_insert(Value::Null);
            }
            if fields.contains_key("schema_version")
                && (fields.contains_key("faces")
                    || fields.contains_key("characteristics") && fields.contains_key("abilities"))
            {
                fields.insert("schema_version".into(), json!(180));
            }
            for item in fields.values_mut() {
                upgrade_179(item, depth + 1)?;
            }
        }
        _ => (),
    }
    Ok(())
}

/// Load exact current JSON or explicitly upgrade the preceding wire contract.
/// Unknown fields and unsupported epochs are errors, including during upgrade.
pub fn read_json(input: &str) -> Result<UnifiedCard, String> {
    if input.len() > 2 * 1024 * 1024 {
        return Err("IR migration document exceeds 2 MiB".into());
    }
    // Bound nesting before entering Serde, matching the browser's ordinary
    // validator. Its default 128-level guard would reject valid saved drafts.
    let (mut depth, mut quoted, mut escaped) = (0usize, false, false);
    for b in input.bytes() {
        if quoted {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                quoted = false;
            }
        } else {
            match b {
                b'"' => quoted = true,
                b'{' | b'[' => {
                    depth += 1;
                    if depth >= 256 {
                        return Err("IR migration nesting exceeds 255 levels".into());
                    }
                }
                b'}' | b']' => depth = depth.saturating_sub(1),
                _ => (),
            }
        }
    }
    let mut reader = serde_json::Deserializer::from_str(input);
    reader.disable_recursion_limit();
    let mut value = Value::deserialize(&mut reader).map_err(|e| e.to_string())?;
    reader.end().map_err(|e| e.to_string())?;
    match value.get("schema_version").and_then(Value::as_u64) {
        Some(version) if version == u64::from(SCHEMA_VERSION) => (),
        Some(179) => {
            let faces = value
                .get("faces")
                .and_then(Value::as_array)
                .ok_or("legacy IR requires faces")?;
            if faces
                .iter()
                .any(|face| face.get("schema_version").and_then(Value::as_u64) != Some(179))
            {
                return Err("legacy IR faces must use schema 179".into());
            }
            upgrade_179(&mut value, 0)?;
        }
        _ => return Err("unsupported AST schema version for migration".into()),
    }
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
