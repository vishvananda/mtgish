//! Result visibility at the same instruction boundaries used by validation.
//! Loop and optional exports are explicit; their private choice groups do not
//! leak. Paying a cost exports moves only at a root PayCost instruction.
use super::*;

impl Reader {
    pub(super) fn operation_input(&mut self, node: &Node) -> Result<Value, String> {
        let previous = self.payment_depth;
        self.payment_depth =
            (constructor(node) == Some("PayCost")).then_some(self.operation_depth + 1);
        let result = self.node(&named("Operation"), node);
        self.payment_depth = previous;
        result
    }
    pub(super) fn finish_control(
        &mut self,
        constructor: &str,
        value: Option<&Value>,
        prior_ids: &HashMap<(String, String), u64>,
        prior_symbols: &HashMap<(String, String), usize>,
        prior_available: HashMap<u64, usize>,
        prior_receipts: Vec<receipts::Group>,
    ) -> Result<(), String> {
        let choices = match constructor {
            "ForEachPlayer" | "ForEachObject" | "Repeatable" => Some(true),
            "Optional" => Some(false),
            "RepeatableWithLimit" | "If" | "ChooseOne" => None,
            _ => return Ok(()),
        };
        self.ids.retain(|(kind, _), _| kind != "r");
        self.symbols.retain(|(kind, _), _| kind != "r");
        for (key, id) in prior_ids.iter().filter(|((kind, _), _)| kind == "r") {
            self.ids.insert(key.clone(), *id);
        }
        for (key, binding) in prior_symbols.iter().filter(|((kind, _), _)| kind == "r") {
            self.symbols.insert(key.clone(), *binding);
        }
        self.available_results = prior_available;
        if !matches!(constructor, "If" | "ChooseOne") {
            self.receipt_groups = prior_receipts;
        }
        if let (Some(value), Some(choices)) = (value, choices) {
            let body = if constructor == "Repeatable" {
                &value["args"]
            } else {
                &value["args"]["body"]
            };
            for (id, path) in
                crate::canonical::exported_receipt_sites(&document::untag(body), choices)
            {
                let binding = body
                    .pointer(&path)
                    .and_then(|v| v.get("binding"))
                    .and_then(Value::as_u64)
                    .ok_or("exported result has no definition identity")?
                    as usize;
                self.available_results.insert(id, binding);
                let key = ("r".into(), self.bindings[binding].name.clone());
                if !prior_symbols.contains_key(&key) {
                    self.ids.insert(key.clone(), id);
                    self.symbols.insert(key, binding);
                }
            }
        }
        Ok(())
    }
}
