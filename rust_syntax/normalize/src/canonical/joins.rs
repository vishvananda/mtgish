//! Explicit joins may read branch receipts without exposing ordinary branch locals.
use super::{JoinGroup, Ty, Value, Walker};

// Bound namespace probes and receipt visits on each pass, including visits
// whose duplicate output is discarded. A refusal never truncates a join.
const MAX_JOIN_WORK: usize = 65_536;
fn charge(work: &mut usize, failure: &mut Option<String>, amount: usize) -> bool {
    if let Some(total) = work.checked_add(amount).filter(|n| *n <= MAX_JOIN_WORK) {
        *work = total;
        true
    } else {
        *failure = Some("branch receipt join work limit exceeded".into());
        false
    }
}
impl Walker {
    pub(super) fn collect_join_branch(&mut self, floor: u64, into: &mut JoinGroup) {
        let Some(scope) = self.scopes.last() else {
            return;
        };
        let probes = scope.result_definitions.len()
            + scope
                .join_groups
                .iter()
                .flat_map(|group| group.values())
                .map(Vec::len)
                .sum::<usize>();
        if !charge(&mut self.join_work, &mut self.failure, probes) {
            return;
        }
        let mut add = |old, new, definition| {
            if definition >= floor {
                let values = into.entry(old).or_default();
                if !charge(&mut self.join_work, &mut self.failure, values.len() + 1) {
                    return false;
                }
                if !values.iter().any(|(_, d)| *d == definition) {
                    values.push((new, definition));
                }
            }
            true
        };
        for (&old, &definition) in &scope.result_definitions {
            if let Some(&new) = scope.maps.get("result").and_then(|m| m.get(&old)) {
                if !add(old, new, definition) {
                    return;
                }
            }
        }
        for group in &scope.join_groups {
            for (&old, values) in group {
                for &(new, definition) in values {
                    if !add(old, new, definition) {
                        return;
                    }
                }
            }
        }
    }
    pub(super) fn choice_branches(&mut self, items: &mut [Value], ty: &Ty) {
        let mut joined = JoinGroup::new();
        for branch in items {
            if self.failure.is_some() {
                return;
            }
            let saved = self.result_scope();
            self.walk(branch, ty, false);
            if let Some(saved) = &saved {
                self.collect_join_branch(saved.floor, &mut joined);
            }
            self.restore_result_scope(saved);
        }
        if let Some(scope) = self.scopes.last_mut() {
            scope.join_groups.push(joined);
        }
    }
    pub(super) fn join_ids(&mut self, value: &mut Value) {
        if self.keep_order {
            return;
        }
        let Some(input) = value.as_array() else {
            return;
        };
        let Some(ids) = input.iter().map(Value::as_u64).collect::<Option<Vec<_>>>() else {
            return;
        };
        let Some(scope) = self.scopes.last() else {
            return;
        };
        let ordinary = scope.maps.get("result");
        let mut group = None;
        for candidate in scope.join_groups.iter().rev() {
            if !charge(&mut self.join_work, &mut self.failure, ids.len()) {
                return;
            }
            if ids.iter().all(|id| {
                ordinary.is_some_and(|m| m.contains_key(id)) || candidate.contains_key(id)
            }) {
                group = Some(candidate);
                break;
            }
        }
        let Some(group) = group else { return };
        let visit = scope.visit;
        let mut results = Vec::new();
        for id in ids {
            // An enclosing definition made after the choice owns its numeric
            // label. Branch-only receipts are a fallback within this scope.
            let local = ordinary
                .and_then(|m| m.get(&id))
                .zip(scope.result_definitions.get(&id))
                .map(|(&new, &definition)| [(new, definition)]);
            let receipts = local
                .as_ref()
                .map(|a| a.as_slice())
                .unwrap_or_else(|| group[&id].as_slice());
            if !charge(&mut self.join_work, &mut self.failure, receipts.len()) {
                return;
            }
            for &(new, definition) in receipts {
                // Preserve each operand occurrence. The existing consumer
                // appends receipts; silently deduplicating changes that list.
                if self.reading {
                    self.read[visit].insert(definition);
                }
                results.push(Value::from(new));
            }
        }
        *value = Value::Array(results);
    }
}
