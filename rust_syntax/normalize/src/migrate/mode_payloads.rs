//! Origins follow parameter subtrees rather than numeric labels.
use super::{entry_payloads::Region, *};
pub(super) fn structure(name: &str, value: &mut Value, path: &str) -> Result<Vec<Region>, String> {
    if name != "ModeSelection" {
        return Ok(vec![]);
    }
    let old: crate::mode_policies::LegacyModeSelection =
        serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
    let mut moves = vec![];
    for (from, to) in [
        ("minimum", "count/minimum"),
        ("maximum", "count/maximum"),
        ("modes", "options"),
        ("unchosen", "rules/availability/Unchosen"),
        ("surcharge", "rules/surcharge"),
        ("chooser", "mechanism/Announced/chooser/OneOf"),
        ("budget", "budget"),
    ] {
        moves.push((format!("{path}/{from}"), format!("{path}/{to}")));
    }
    *value = serde_json::to_value(old.current()?).map_err(|e| e.to_string())?;
    Ok(vec![Region {
        source: path.into(),
        moves,
    }])
}
