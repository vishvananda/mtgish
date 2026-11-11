//! Schema188 ->189: predicates, counter destinations and consistent boolean names.
//! Rewrites run after children and older epoch upgrades. Binding origins follow
//! each moved subtree; expressions in scalar positions are never reinterpreted.
use super::{entry_payloads::Region, *};
fn moved(source: String, target: String) -> Region {
    Region {
        source: source.clone(),
        moves: vec![(source, target)],
    }
}
fn predicate(
    comparison: Value,
    value: Value,
    from: &str,
    to: &str,
    regions: &mut Vec<Region>,
) -> Result<Value, String> {
    if value["kind"] == "AnyOf" {
        let values = value["args"]
            .as_array()
            .ok_or("numeric alternatives require an array")?;
        let mut tests = Vec::with_capacity(values.len());
        for (i, value) in values.iter().enumerate() {
            tests.push(predicate(
                comparison.clone(),
                value.clone(),
                &format!("{from}/args/{i}"),
                &format!("{to}/args/{i}"),
                regions,
            )?);
        }
        return Ok(json!({"kind":"AnyOf","args":tests}));
    }
    if value["kind"] == "Aggregate" && value["args"]["aggregation"] == "AnyOf" {
        regions.push(moved(
            format!("{from}/args/objects"),
            format!("{to}/args/objects"),
        ));
        return Ok(
            json!({"kind":"AnyProperty","args":{"comparison":comparison,"objects":value["args"]["objects"],"property":value["args"]["property"]}}),
        );
    }
    regions.push(moved(from.into(), format!("{to}/args/value")));
    Ok(json!({"kind":"Compare","args":{"comparison":comparison,"value":value}}))
}
pub(super) fn variant(owner: &str, value: &mut Value, path: &str) -> Result<Vec<Region>, String> {
    let mut regions = Vec::new();
    let field = match owner {
        "Condition::Compare" => Some("right"),
        "ObjectFilter::Property" | "EventSpec::NthEachTurn" | "Operation::LimitX" => Some("value"),
        _ => None,
    };
    if let Some(field) = field {
        let adjacent = owner != "EventSpec::NthEachTurn";
        let at = if adjacent {
            format!("{path}/args")
        } else {
            format!("{path}/NthEachTurn")
        };
        let args = if adjacent {
            &mut value["args"]
        } else {
            &mut value["NthEachTurn"]
        };
        let object = args
            .as_object_mut()
            .ok_or("numeric test requires named arguments")?;
        let comparison = object
            .remove("comparison")
            .ok_or("numeric test missing comparison")?;
        let scalar = object.remove(field).ok_or("numeric test missing value")?;
        object.insert(
            "test".into(),
            predicate(
                comparison,
                scalar,
                &format!("{at}/{field}"),
                &format!("{at}/test"),
                &mut regions,
            )?,
        );
    }
    match owner {
        "Operand::Comparison"
            if value["args"][1]["kind"] == "AnyOf"
                || value["args"][1]["args"]["aggregation"] == "AnyOf" =>
        {
            let old = value["args"][1].take();
            let test = predicate(
                value["args"][0].clone(),
                old,
                &format!("{path}/args/1"),
                &format!("{path}/args"),
                &mut regions,
            )?;
            value["kind"] = json!("NumberTest");
            value["args"] = test;
        }
        "ActivationRule::XIs" => {
            let old = value["XIs"].take();
            value["XIs"] = predicate(
                json!("Equal"),
                old,
                &format!("{path}/XIs"),
                &format!("{path}/XIs"),
                &mut regions,
            )?;
        }
        "Operation::Counter" => {
            let destination: Destination =
                serde_json::from_value(value["args"]["destination"].clone())
                    .map_err(|e| e.to_string())?;
            let target: MovementDestination = destination.into();
            // The legacy address has an Expr or PlayerRef only in these variants.
            let from = format!("{path}/args/destination");
            let (old, new) = match &target {
                MovementDestination::Battlefield { .. } => {
                    ("/Battlefield", "/Battlefield/controller/Player")
                }
                MovementDestination::Hand { .. } => ("/Hand", "/Hand/player"),
                MovementDestination::Library {
                    position: LibraryPosition::FromTop(_),
                    ..
                } => ("/OwnersLibraryFromTop", "/Library/position/FromTop"),
                _ => ("", ""),
            };
            regions.push(moved(format!("{from}{old}"), format!("{from}{new}")));
            value["args"]["destination"] =
                serde_json::to_value(target).map_err(|e| e.to_string())?;
            value["args"]["destination_when"] = Value::Null;
        }
        "Operation::CounterWith" => {
            let args = value["args"]
                .as_object_mut()
                .ok_or("counter requires named arguments")?;
            let object = args.remove("object").ok_or("counter missing object")?;
            let filter = args
                .remove("when")
                .ok_or("counter missing destination condition")?;
            args.insert("objects".into(), json!({"kind":"One","args":object}));
            args.insert("destination_when".into(), filter);
            regions.push(moved(
                format!("{path}/args/object"),
                format!("{path}/args/objects/args"),
            ));
            regions.push(moved(
                format!("{path}/args/when"),
                format!("{path}/args/destination_when"),
            ));
            value["kind"] = json!("Counter");
        }
        "Instruction::Times" => value["kind"] = json!("RepeatableWithLimit"),
        "Condition::All" | "ObjectFilter::All" => value["kind"] = json!("AllOf"),
        "Condition::Either" | "ObjectFilter::Either" => value["kind"] = json!("AnyOf"),
        "EventSpec::Either" => {
            let args = value.as_object_mut().unwrap().remove("Either").unwrap();
            value["AnyOf"] = args;
            regions.push(moved(format!("{path}/Either"), format!("{path}/AnyOf")));
        }
        _ => {}
    }
    Ok(regions)
}
