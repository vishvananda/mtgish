//! Portable records for IR consumers, including explicit multi-face roles.
//! This adapter owns Source interpretation; consumers never need Source types.
use crate::{corpus, inspect, ir::Layout, mtg_types::OracleCard, unified, validate};
use serde_json::{json, Value};
use std::{io::Write, path::Path};

pub fn record(source: &OracleCard) -> corpus::Result<Value> {
    let lowered = unified::lower(source);
    validate::unified(&lowered.card)?;
    let identity = corpus::Identity::of(source)?;
    let raw = inspect::value(source)?;
    let args = &raw["args"];
    let primary = match &lowered.card.layout {
        Layout::Transforming | Layout::ModalDFC => args["FrontFace"]["Name"].as_str(),
        Layout::Flip => args["Unflipped"]["Name"].as_str(),
        Layout::Adventurer | Layout::Ominous | Layout::Preparer => args["Name"].as_str(),
        Layout::Room => args["LeftDoor"]["Name"].as_str(),
        Layout::Split => args["Cards"][0]["Name"].as_str(),
        _ => lowered.card.faces.first().map(|face| face.name.as_str()),
    };
    Ok(json!({"format_version":1,"card":lowered.card,
        "context":{"primary_face":primary,
            "hand_modifier":args["HandModifier"].as_i64().unwrap_or(0),
            "life_modifier":args["LifeModifier"].as_i64().unwrap_or(0)},
        "identity":identity,"diagnostics":lowered.diagnostics}))
}

pub fn export(source: &Path, destination: &Path) -> corpus::Result<usize> {
    let mut output = corpus::Artifact::create(destination)?;
    let mut count = 0;
    corpus::map_cards(source, |_, card| record(&card), |_, value| {
        serde_json::to_writer(&mut output, &value)?;
        writeln!(output)?;
        count += 1;
        Ok(())
    })?;
    if count == 0 { return Err("card corpus is empty".into()); }
    output.commit()?;
    Ok(count)
}
