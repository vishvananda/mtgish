//! Explicit joins may read branch receipts without exposing ordinary branch locals.
use super::{JoinGroup, Ty, Value, Walker};
use std::collections::HashSet;
impl Walker {
    pub(super) fn collect_join_branch(&self, floor: u64, into: &mut JoinGroup) {
        let Some(scope) = self.scopes.last() else {
            return;
        };
        let mut add = |old, new, definition| {
            if definition >= floor {
                let values = into.entry(old).or_default();
                if !values.iter().any(|(_, d)| *d == definition) {
                    values.push((new, definition));
                }
            }
        };
        for (&old, &definition) in &scope.result_definitions {
            if let Some(&new) = scope.maps.get("result").and_then(|m| m.get(&old)) {
                add(old, new, definition);
            }
        }
        for group in &scope.join_groups {
            for (&old, values) in group {
                for &(new, definition) in values {
                    add(old, new, definition);
                }
            }
        }
    }
    pub(super) fn choice_branches(&mut self, items: &mut [Value], ty: &Ty) {
        let mut joined = JoinGroup::new();
        for branch in items {
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
        let Some(group) = scope
            .join_groups
            .iter()
            .rev()
            .find(|g| ids.iter().all(|id| g.contains_key(id)))
        else {
            return;
        };
        let visit = scope.visit;
        let mut seen = HashSet::new();
        let mut results = Vec::new();
        for id in ids {
            for &(new, definition) in &group[&id] {
                if seen.insert(definition) {
                    if self.reading {
                        self.read[visit].insert(definition);
                    }
                    results.push(Value::from(new));
                }
            }
        }
        *value = Value::Array(results);
    }
}
