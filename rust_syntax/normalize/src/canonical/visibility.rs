//! Result receipts which the validator exports from immediate control flow.
use crate::ir::{Instruction, Operation, Program};
use serde_json::Value;
use std::collections::HashSet;

pub(super) fn exports(value: &Value, choices: bool) -> HashSet<u64> {
    fn collect(body: &Program, choices: bool, out: &mut HashSet<u64>) {
        for instruction in &body.instructions {
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
                        out.insert(u64::from(result.0));
                    }
                }
                Instruction::Group(body) | Instruction::Optional { body, .. } => {
                    collect(body, choices, out)
                }
                Instruction::ForEachPlayer { body, .. } if choices => collect(body, choices, out),
                _ => {}
            }
        }
    }
    let mut out = HashSet::new();
    if let Ok(body) = serde_json::from_value::<Program>(value.clone()) {
        collect(&body, choices, &mut out);
    }
    out
}
