//! Mutually exclusive branch outcomes have a dedicated join-only namespace.
use super::{BranchReceipts, Walker};
use crate::{
    ir::{Instruction, Operation, Program},
    ir_schema::Ty,
};
use serde_json::Value;

// Each pass bounds both namespace probes and emitted join operands. A refusal
// propagates through normalize; it never silently truncates a receipt list.
const MAX_JOIN_WORK: usize = 65_536;

fn choice_exports(value: &Value) -> std::collections::HashSet<u64> {
    fn joins(body: &Program, out: &mut std::collections::HashSet<u64>) {
        for instruction in &body.instructions {
            match instruction {
                Instruction::Effect { result, operation }
                    if matches!(operation.action(), Operation::JoinResults { .. }) =>
                {
                    out.insert(u64::from(result.0));
                }
                Instruction::Group(body) | Instruction::Optional { body, .. } => joins(body, out),
                _ => {}
            }
        }
    }
    let mut out = super::visibility::exports(value, false);
    if let Ok(body) = serde_json::from_value::<Program>(value.clone()) {
        joins(&body, &mut out);
    }
    out
}

impl Walker {
    fn charge_join(&mut self, work: usize) -> bool {
        if let Some(total) = self
            .join_work
            .checked_add(work)
            .filter(|n| *n <= MAX_JOIN_WORK)
        {
            self.join_work = total;
            true
        } else {
            self.failure = Some("branch receipt join work limit exceeded".into());
            false
        }
    }

    pub(super) fn choice_options(&mut self, value: &mut Value, ty: &Ty) {
        let (Value::Array(options), Ty::Vec(inner)) = (value, ty) else {
            return;
        };
        let mut outcomes = BranchReceipts::new();
        for option in options {
            // Every sibling inherits exactly the enclosing ordinary namespace.
            // A sibling must never see a preceding option's local definitions.
            let saved = self.result_scope();
            let exported = choice_exports(option);
            self.walk(option, inner, false);
            if let Some(scope) = self.scopes.last() {
                for old in exported {
                    if let (Some(new), Some(definition)) = (
                        scope.maps.get("result").and_then(|map| map.get(&old)),
                        scope.result_definitions.get(&old),
                    ) {
                        outcomes.entry(old).or_default().push((*new, *definition));
                    }
                }
            }
            self.restore_result_scope(saved);
        }
        if let Some(scope) = self.scopes.last_mut() {
            // A later choice owns its labels. Within one choice, repeated old
            // labels denote alternative definitions and retain each identity.
            scope.branch_receipts.extend(outcomes);
        }
    }

    pub(super) fn join_results(&mut self, value: &mut Value) {
        if self.keep_order {
            return;
        }
        let Value::Array(results) = value else { return };
        let mut joined = Vec::new();
        for result in results.iter() {
            let Some(old) = result.as_u64() else {
                if !self.charge_join(1) {
                    return;
                }
                joined.push(result.clone());
                continue;
            };
            let mut resolved = None;
            for index in (0..self.scopes.len()).rev() {
                if !self.charge_join(1) {
                    return;
                }
                let scope = &self.scopes[index];
                if let Some(new) = scope.maps.get("result").and_then(|m| m.get(&old)) {
                    resolved = Some((
                        scope.visit,
                        vec![(*new, scope.result_definitions.get(&old).copied())],
                    ));
                    break;
                }
                if let Some(receipts) = scope.branch_receipts.get(&old) {
                    if !self.charge_join(receipts.len()) {
                        return;
                    }
                    let scope = &self.scopes[index];
                    resolved = Some((
                        scope.visit,
                        scope.branch_receipts[&old]
                            .iter()
                            .map(|(new, definition)| (*new, Some(*definition)))
                            .collect(),
                    ));
                    break;
                }
            }
            if let Some((visit, receipts)) = resolved {
                for (new, definition) in receipts {
                    if self.reading {
                        if let Some(definition) = definition {
                            self.read[visit].insert(definition);
                        }
                    }
                    joined.push(Value::from(new));
                }
            } else {
                joined.push(result.clone());
            }
        }
        *results = joined;
    }
}
