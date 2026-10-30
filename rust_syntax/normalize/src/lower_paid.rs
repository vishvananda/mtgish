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
        let mut found = std::collections::BTreeMap::new();
        for (action, path) in crate::cost_history::components(cost) {
            let part = crate::cost_history::component(cost, &path).unwrap();
            let singular = crate::cost_history::singular(part);
            let binding = PaidBinding {
                payment: payment.clone(),
                component: path,
                singular,
            };
            found
                .entry(action)
                .and_modify(|v| *v = None)
                .or_insert(Some(binding));
        }
        for action in found.keys() {
            self.acted.remove(action);
        }
        self.paid.extend(found);
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
