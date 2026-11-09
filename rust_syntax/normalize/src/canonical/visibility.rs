//! Result receipts which the validator exports from immediate control flow.
use crate::ir::{Instruction, Operation, Program};
use serde_json::Value;
use std::collections::HashSet;

pub(super) fn exports(value: &Value, choices: bool) -> HashSet<u64> {
    export_sites(value, choices)
        .into_iter()
        .map(|(id, _)| id)
        .collect()
}
/// The exact declaration sites, shared with authoring-name resolution. Quotes
/// and future invocations are not traversed, even if they reuse the same ID.
fn collect(body: &Program, choices: bool, path: &str, out: &mut Vec<(u64, String)>) {
    for (i, instruction) in body.instructions.iter().enumerate() {
        let at = format!("{path}/instructions/{i}/args");
        match instruction {
            Instruction::Effect { result, operation } => {
                if choices
                    || matches!(
                        operation.action(),
                        Operation::Move { .. }
                            | Operation::Destroy { .. }
                            | Operation::Sacrifice { .. }
                            | Operation::Discard { .. }
                            | Operation::DiscardObjects { .. }
                            | Operation::DiscardAllBut { .. }
                            | Operation::Mill { .. }
                            | Operation::CreateTokens { .. }
                            | Operation::ChooseObjects { .. }
                    )
                {
                    out.push((u64::from(result.0), format!("{at}/result")));
                }
            }
            Instruction::Group(body) => collect(body, choices, &at, out),
            Instruction::Optional { body, .. } => {
                collect(body, choices, &format!("{at}/body"), out)
            }
            Instruction::ForEachPlayer { body, .. } if choices => {
                collect(body, choices, &format!("{at}/body"), out)
            }
            _ => (),
        }
    }
}

pub(crate) fn export_sites(value: &Value, choices: bool) -> Vec<(u64, String)> {
    let mut out = Vec::new();
    if let Ok(body) = serde_json::from_value::<Program>(super::trace::plain(value)) {
        collect(&body, choices, "", &mut out);
    }
    out
}

/// Results a branch publishes to its enclosing choice's receipt group. Ordinary
/// instructions publish their result, nested choices publish their alternatives,
/// and immediate control flow uses the same export policy as the validator.
pub(crate) fn branch_sites(value: &Value) -> Vec<(u64, String)> {
    fn branch(body: &Program, path: &str, out: &mut Vec<(u64, String)>) {
        for (i, instruction) in body.instructions.iter().enumerate() {
            let at = format!("{path}/instructions/{i}/args");
            match instruction {
                Instruction::Effect { result, operation } => {
                    if let Operation::PayCost { cost, .. } = operation {
                        for (part_index, part) in cost.parts.iter().enumerate() {
                            if let crate::ir::CostPart::PlayerAction { body, .. } = part {
                                collect(body,false,&format!("{at}/operation/args/cost/parts/{part_index}/PlayerAction/body"),out);
                            }
                        }
                    }
                    out.push((u64::from(result.0), format!("{at}/result")));
                }
                Instruction::Group(body) => branch(body, &at, out),
                Instruction::If {
                    then, otherwise, ..
                } => {
                    branch(then, &format!("{at}/then"), out);
                    branch(otherwise, &format!("{at}/otherwise"), out);
                }
                Instruction::ChooseOne { options, .. } => {
                    for (j, option) in options.iter().enumerate() {
                        branch(option, &format!("{at}/options/{j}"), out);
                    }
                }
                Instruction::Optional { body, .. } => {
                    collect(body, false, &format!("{at}/body"), out)
                }
                Instruction::ForEachObject { body, .. }
                | Instruction::ForEachPlayer { body, .. } => {
                    collect(body, true, &format!("{at}/body"), out)
                }
                Instruction::Repeatable(body) => collect(body, true, &at, out),
                Instruction::Times { .. } => {}
            }
        }
    }
    let mut out = Vec::new();
    if let Ok(body) = serde_json::from_value::<Program>(super::trace::plain(value)) {
        branch(&body, "", &mut out);
    }
    out
}
