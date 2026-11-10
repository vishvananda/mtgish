//! Input-only compatibility for old card code. Resolve names in the ordinary
//! reader, then use the same destination rewrite as saved JSON. Binding tags
//! follow explicit structural origins; no numeric ID identifies a scope.
use super::{document, syntax::Node};
use crate::ir_schema::{Fields, TypeDef};
use serde_json::Value;
pub(super) fn old_address(n: &Node) -> bool {
    match n {
        Node::Name(name) | Node::Scalar(Value::String(name)) => matches!(
            name.as_str(),
            "OwnersHand"
                | "OwnersGraveyard"
                | "OwnersLibraryTop"
                | "OwnersLibraryBottom"
                | "Exile"
                | "OwnersBattlefield"
                | "OwnersLibraryShuffled"
                | "OwnersLibraryBottomRandom"
                | "OwnersLibraryTopOrBottom"
                | "CommandZone"
                | "Ante"
        ),
        Node::Call(name, args) => {
            args.len() == 1
                && matches!(
                    name.as_str(),
                    "Battlefield" | "Hand" | "OwnersLibraryFromTop" | "OwnersPlanarDeck"
                )
        }
        _ => false,
    }
}
pub(super) fn legacy_fields(ty: &str, variant: &str, n: &Node) -> Option<&'static Fields> {
    let Node::Record(_, fields) = n else {
        return None;
    };
    let entry = fields
        .iter()
        .any(|(key, _)| matches!(key.as_str(), "entry" | "face_down"));
    let compatible = match (ty, variant) {
        ("Operation", "Move" | "MoveTopCard" | "CounterWith") => {
            entry || fields.iter().any(|(key, _)| key == "terms")
        }
        ("Operation", "Conjure") => fields.iter().any(|(key, _)| key == "zone"),
        ("CostPart", "Move") | ("SelectionConstraint", "CanMoveTo") => entry,
        _ => false,
    };
    if !compatible {
        return None;
    }
    let TypeDef::Enum { variants, .. } = crate::migrate::legacy_authoring_type(ty)? else {
        return None;
    };
    variants
        .iter()
        .find(|(name, _)| name == variant)
        .map(|(_, fields)| fields)
}
pub(super) fn upgrade(owner: &str, tagged: Value) -> Result<Value, String> {
    let mut value = tagged.clone();
    let mut uses = vec![];
    document::extract(&mut value, "", &mut uses)?;
    let moves = crate::migrate::upgrade_authoring_movement(owner, &mut value)?;
    for site in uses {
        let tag = tagged
            .pointer(&site.path)
            .ok_or("missing authoring binding origin")?;
        let destinations: Vec<_> = if moves.is_empty() {
            vec![site.path.clone()]
        } else {
            moves
                .iter()
                .filter_map(|(from, to)| {
                    site.path
                        .strip_prefix(from)
                        .filter(|suffix| suffix.is_empty() || suffix.starts_with('/'))
                        .map(|suffix| format!("{to}{suffix}"))
                })
                .collect()
        };
        if destinations.is_empty() {
            return Err(format!("movement rewrite lost binding at {}", site.path));
        }
        for path in destinations {
            let target = value
                .pointer_mut(&path)
                .ok_or_else(|| format!("missing movement binding destination {path}"))?;
            if target != &tag["$mtgish_id"] {
                return Err("movement rewrite changed a binding ID".into());
            }
            *target = tag.clone();
        }
    }
    Ok(value)
}
/// Lookup is constrained to the two compatibility-only payload types. All
/// expressions, operations and private programs inside use the current schema.
pub(super) fn legacy_type(name: &str) -> Option<&'static TypeDef> {
    if matches!(name, "EntrySpec" | "EntryState") {
        crate::migrate::legacy_authoring_type(name)
    } else {
        None
    }
}

pub(super) fn address(tagged: Value) -> Result<Value, String> {
    let old = serde_json::json!({"kind":"Move","args":{"objects":{"kind":"One","args":{"kind":"Source"}},"destination":tagged,"entry":crate::movement::legacy::EntrySpec::default(),"face_down":false,"terms":crate::ir::MovementTerms::default()}});
    Ok(upgrade("Operation::Move", old)?["args"]["destination"].take())
}
