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
pub(crate) fn export_sites(value: &Value, choices: bool) -> Vec<(u64, String)> {
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
    let mut out = Vec::new();
    if let Ok(body) = serde_json::from_value::<Program>(value.clone()) {
        collect(&body, choices, "", &mut out);
    }
    out
}
