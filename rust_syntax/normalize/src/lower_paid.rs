// Resolve shorthand to an exact cost namespace and structural component path.
// Ambiguity is retained rather than choosing an arbitrary component or object.
#[derive(Clone, serde::Serialize)]
struct PaidBinding {
    payment: PaymentRef,
    component: Vec<u32>,
    singular: bool,
}
impl Context {
    fn proposal(&self) -> Self {
        let mut context = self.clone();
        context.announcing = true;
        context
    }

    fn install_paid_history(&mut self, cost: &Cost, payment: PaymentRef) {
        self.sacrificed_source |= matches!(payment, PaymentRef::Activation)
            && self.paid_source_sacrificed(cost);
        let mut found = std::collections::BTreeMap::new();
        let mut parts: std::collections::BTreeMap<_, Vec<PaidBinding>> = Default::default();
        for (action, path) in crate::cost_history::components(cost) {
            let part = crate::cost_history::component(cost, &path).unwrap();
            let singular = crate::cost_history::singular(part);
            let binding = PaidBinding {
                payment: payment.clone(),
                component: path,
                singular,
            };
            parts.entry(action).or_default().push(binding.clone());
            found
                .entry(action)
                .and_modify(|v| *v = None)
                .or_insert(Some(binding));
        }
        for action in found.keys() {
            self.acted.remove(action);
            self.paid_parts.remove(action);
        }
        parts.retain(|_, parts| parts.len() > 1);
        self.paid_parts.extend(parts);
        self.paid.extend(found);
    }
    /// Whatever the components of `action` in the cost being read did together.
    fn paid_set(
        &self,
        action: crate::number_spec::PaymentAction,
        view: CostObjectView,
        phase: ObjectPhase,
    ) -> Option<ObjectSet> {
        let set = |binding: &PaidBinding| ObjectSet::CostObjects {
            payment: binding.payment.clone(),
            component: binding.component.clone(),
            view: view.clone(),
            phase: phase.clone(),
        };
        if let Ok(binding) = self.paid_binding(action) {
            return Some(set(binding));
        }
        let parts = self.paid_parts.get(&action)?;
        Some(ObjectSet::Union(parts.iter().map(set).collect()))
    }
    /// This ability's own cost sacrificed the object it is on.
    fn paid_source_sacrificed(&self, cost: &Cost) -> bool {
        cost.parts.iter().any(|part| {
            matches!(
                part,
                CostPart::Sacrifice { objects: ObjectSet::One(ObjectRef::Source), .. }
            )
        })
    }
    fn paid_binding(&self, action: crate::number_spec::PaymentAction) -> Result<&PaidBinding> {
        self.paid
            .get(&action)
            .and_then(Option::as_ref)
            .ok_or_else(|| {
                invalid("payment object reference has no unique component in this lexical scope")
            })
    }
    fn paid_objects(
        &self,
        action: crate::number_spec::PaymentAction,
        view: CostObjectView,
    ) -> Result<ObjectSet> {
        if let Some(result) = self.acted_result(action) {
            return Ok(ObjectSet::Transition {
                result: result.clone(),
                phase: ObjectPhase::Before,
            });
        }
        if !self.announcing && self.paid_binding(action).is_err() {
            if let Some(objects) = self.paid_set(action, view.clone(), ObjectPhase::Before) {
                return Ok(objects);
            }
            let done = match action {
                crate::number_spec::PaymentAction::Sacrifice => Some(Done::Sacrificed),
                crate::number_spec::PaymentAction::Discard => Some(Done::Discarded),
                crate::number_spec::PaymentAction::Exile => Some(Done::Exiled),
                _ => None,
            };
            if let Some(result) = done.and_then(|done| self.did.get(&done)) {
                return Ok(ObjectSet::Transition {
                    result: result.clone(),
                    phase: ObjectPhase::Before,
                });
            }
        }
        let binding = self.paid_binding(action)?;
        if self.announcing
            && !(matches!(view, CostObjectView::Inputs)
                && !matches!(binding.payment, PaymentRef::Resolution(_)))
        {
            return Err(invalid(
                "payment history is unavailable during announcement",
            ));
        }
        Ok(ObjectSet::CostObjects {
            payment: binding.payment.clone(),
            component: binding.component.clone(),
            view,
            phase: ObjectPhase::Before,
        })
    }
    /// What a cost put onto the battlefield, as it is there now.
    fn paid_entered(&self) -> Option<(ObjectSet, bool)> {
        let binding = self.paid.get(&crate::number_spec::PaymentAction::Enter)?;
        let binding = binding.as_ref().filter(|_| !self.announcing)?;
        Some((
            ObjectSet::CostObjects {
                payment: binding.payment.clone(),
                component: binding.component.clone(),
                view: CostObjectView::Performed,
                phase: ObjectPhase::After,
            },
            binding.singular,
        ))
    }
    /// The latest reveal, by an action here or else by a cost.
    fn revealed_objects(&self) -> Result<ObjectSet> {
        if let Some(objects) = &self.revealed {
            return Ok(objects.clone());
        }
        let binding = self
            .paid
            .get(&crate::number_spec::PaymentAction::Reveal)
            .and_then(Option::as_ref)
            .filter(|_| !self.announcing)
            .ok_or_else(|| invalid("no card was revealed in this scope"))?;
        Ok(ObjectSet::CostObjects {
            payment: binding.payment.clone(),
            component: binding.component.clone(),
            view: CostObjectView::Inputs,
            phase: ObjectPhase::Before,
        })
    }
    /// What a cost exiled, as it is now in exile. `None` when no cost here did.
    fn paid_exile(&self) -> Option<(ObjectSet, bool)> {
        let binding = self.paid.get(&crate::number_spec::PaymentAction::Exile)?;
        let binding = binding.as_ref().filter(|_| !self.announcing)?;
        Some((
            ObjectSet::CostObjects {
                payment: binding.payment.clone(),
                component: binding.component.clone(),
                view: CostObjectView::Performed,
                phase: ObjectPhase::After,
            },
            binding.singular,
        ))
    }
    fn acted_result(&self, action: crate::number_spec::PaymentAction) -> Option<&ResultId> {
        let paid = self.paid.get(&action).is_some_and(Option::is_some);
        self.acted.get(&action).filter(|_| !paid)
    }
    fn paid_object(&self, action: crate::number_spec::PaymentAction) -> Result<ObjectRef> {
        if let Some(result) = self.acted_result(action) {
            if !self.singular.contains(&result.0) {
                return Err(invalid(
                    "singular action reference has a plural or unknown cardinality",
                ));
            }
        } else if !self.paid_binding(action)?.singular {
            return Err(invalid(
                "singular payment reference has a plural or unknown cardinality",
            ));
        }
        Ok(ObjectRef::Query {
            objects: Box::new(self.paid_objects(action, CostObjectView::Inputs)?),
            index: 0,
        })
    }
}
