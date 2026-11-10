//! Historical flags are decoded only here. Origins follow operands rather than
//! guessed numeric labels; explicit historical calls remain authoritative.
use super::{entry_payloads::Region, *};
pub(super) fn variant(
    owner: &str,
    value: &mut Value,
    path: &str,
) -> Result<(Vec<Region>, bool), String> {
    let mut regions = vec![];
    let mut forced = false;
    match owner {
        "Operation::FlipCoin" => {
            value["args"]["protocol"] = json!("Called");
        }
        "Operation::FlipCoins" => {
            let fields = value["args"].as_object_mut().ok_or("coin fields")?;
            let actor = fields.remove("actor").ok_or("coin flippers")?;
            fields.insert("players".into(), actor);
            fields.insert("protocol".into(), json!("Called"));
            let from = format!("{path}/args/actor");
            regions.push(Region {
                source: from.clone(),
                moves: vec![(from, format!("{path}/args/players"))],
            });
        }
        "Operation::CoinFlips" => {
            let a = &value["args"];
            let until = a["until_lost"]
                .as_bool()
                .ok_or("coin loss stop requires Boolean")?;
            let may = a["may_stop"]
                .as_bool()
                .ok_or("coin choice stop requires Boolean")?;
            forced = a["called"].as_bool().ok_or("called requires Boolean")?;
            let stop = match (until, may) {
                (false, false) => None,
                (true, false) => Some("Lose"),
                (false, true) => Some("PlayerStops"),
                (true, true) => Some("LoseOrPlayerStops"),
            };
            let (extent, operand, field) = match (a["times"].is_null(), a["each"].is_null()) {
                (false, true) => (
                    Some(json!({"Count":a["times"]})),
                    Some(a["times"].clone()),
                    "times",
                ),
                (true, false) => (
                    Some(json!({"Objects":a["each"]})),
                    Some(a["each"].clone()),
                    "each",
                ),
                (true, true) => (None, None, ""),
                _ => {
                    return Err("coin series cannot have both a count and per-object extent".into())
                }
            };
            let (repetition, to) = match (extent, stop) {
                (Some(extent), Some(stop)) => (
                    json!({"UpTo":{"extent":extent,"stop":stop}}),
                    format!(
                        "repetition/UpTo/extent/{}",
                        if field == "times" { "Count" } else { "Objects" }
                    ),
                ),
                (Some(_), None) => {
                    let name = if field == "times" {
                        "Count"
                    } else {
                        "EachObject"
                    };
                    let mut o = serde_json::Map::new();
                    o.insert(name.into(), operand.unwrap());
                    (Value::Object(o), format!("repetition/{name}"))
                }
                (None, Some(stop)) => (json!({"Until":stop}), String::new()),
                (None, None) => {
                    return Err("coin series requires a finite extent or an explicit stop".into())
                }
            };
            let mut moves = vec![(format!("{path}/args/actor"), format!("{path}/args/players"))];
            if !field.is_empty() {
                moves.push((format!("{path}/args/{field}"), format!("{path}/args/{to}")));
            }
            *value = json!({"kind":"FlipCoinSeries","args":{"players":a["actor"],"protocol":"Called","repetition":repetition}});
            regions.push(Region {
                source: format!("{path}/args"),
                moves,
            });
        }
        "Expr::HeadsFlipped" => {
            let id = value["args"].clone();
            *value = json!({"kind":"CountCoinOutcomes","args":{"source":{"Receipt":id},"outcome":"Heads"}});
            regions.push(Region {
                source: path.into(),
                moves: vec![(
                    format!("{path}/args"),
                    format!("{path}/args/source/Receipt"),
                )],
            });
        }
        "Expr::CoinFlipsWon" => {
            let won = value["args"]
                .as_bool()
                .ok_or("legacy wins/losses requires Boolean")?;
            *value = json!({"kind":"CountCoinOutcomes","args":{"source":"Latest","outcome":if won{"Won"}else{"Lost"}}});
        }
        _ => {}
    };
    Ok((regions, forced))
}
