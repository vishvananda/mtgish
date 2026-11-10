//! Bottom-up destination migration with explicit origins for every old ID leaf.
use super::{entry_payloads::Region, *};
pub(super) fn variant(owner: &str, value: &mut Value, path: &str) -> Result<Vec<Region>, String> {
    if owner == "EventChange::Entry" {
        let state: EntryState =
            serde_json::from_value(value["Entry"].clone()).map_err(|e| e.to_string())?;
        value["Entry"] =
            serde_json::to_value(crate::movement::battlefield_state_from_parts(&state)?)
                .map_err(|e| e.to_string())?;
        return Ok(vec![]);
    }
    if matches!(
        owner,
        "Operation::CreateTokens"
            | "Operation::Manifest"
            | "Operation::Meld"
            | "Operation::Populate"
    ) {
        let entry: EntrySpec =
            serde_json::from_value(value["args"]["entry"].clone()).map_err(|e| e.to_string())?;
        value["args"]["entry"] =
            serde_json::to_value(crate::movement::battlefield_from_parts(&entry)?)
                .map_err(|e| e.to_string())?;
        return Ok(vec![]);
    }
    let fields: &[&str] = match owner {
        "Operation::Move" => &["objects"],
        "Operation::MoveTopCard" => &["actor", "cards", "reveal"],
        "Operation::CounterWith" => &["object", "when"],
        "Operation::Conjure" => &["actor", "card", "count", "owner"],
        "CostPart::Move" => &["selections"],
        "SelectionConstraint::CanMoveTo" => &[],
        _ => return Ok(vec![]),
    };
    let key = if owner.starts_with("Operation::") {
        "args"
    } else {
        owner.split("::").nth(1).unwrap()
    };
    let args = &value[key];
    let entry: EntrySpec =
        serde_json::from_value(args["entry"].clone()).map_err(|e| e.to_string())?;
    let at = format!("{path}/{key}");
    let dest_path = format!("{at}/destination");
    let entry_path = format!("{at}/entry");
    let terms_path = format!("{at}/terms");
    let qualifiers_path = format!("{at}/qualifiers");
    let mut output = serde_json::Map::new();
    let mut moves = vec![];
    for field in fields {
        output.insert((*field).into(), args[*field].clone());
        moves.push((format!("{at}/{field}"), format!("{at}/{field}")));
    }
    if owner == "Operation::Conjure" {
        let zone: EventZone =
            serde_json::from_value(args["zone"].clone()).map_err(|e| e.to_string())?;
        let spot: Option<ConjureSpot> =
            serde_json::from_value(args["spot"].clone()).map_err(|e| e.to_string())?;
        let transfer = crate::movement::conjure_from_parts(
            &zone,
            &spot,
            &entry,
            Some((&entry_path, &format!("{at}/spot"), &dest_path)),
        )?;
        output.insert(
            "destination".into(),
            serde_json::to_value(transfer.destination).map_err(|e| e.to_string())?,
        );
        moves.extend(transfer.moves);
    } else {
        let destination: Destination =
            serde_json::from_value(args["destination"].clone()).map_err(|e| e.to_string())?;
        let ordinary = owner == "Operation::Move";
        let terms: MovementTerms = if ordinary {
            serde_json::from_value(args["terms"].clone()).map_err(|e| e.to_string())?
        } else {
            MovementTerms::default()
        };
        let hidden =
            if ordinary || matches!(owner, "CostPart::Move" | "SelectionConstraint::CanMoveTo") {
                args["face_down"]
                    .as_bool()
                    .ok_or("movement visibility requires Boolean")?
            } else {
                false
            };
        let transfer = crate::movement::destination_from_parts(
            &destination,
            &entry,
            hidden,
            &terms,
            Some((
                &dest_path,
                &entry_path,
                &terms_path,
                &dest_path,
                &qualifiers_path,
            )),
        )?;
        output.insert(
            "destination".into(),
            serde_json::to_value(transfer.destination).map_err(|e| e.to_string())?,
        );
        if ordinary {
            output.insert(
                "qualifiers".into(),
                serde_json::to_value(transfer.qualifiers).map_err(|e| e.to_string())?,
            );
        }
        moves.extend(transfer.moves);
    }
    value[key] = Value::Object(output);
    Ok(vec![Region { source: at, moves }])
}
