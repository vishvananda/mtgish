//! Alternative results are visible only to an explicit receipt join. A numeric
//! label reused by alternatives denotes a group, never one arbitrary definition.
use super::*;

#[derive(Clone, Debug)]
pub(super) struct Receipt {
    id: u64,
    binding: usize,
}
pub(super) type Group = HashMap<String, Vec<Receipt>>;
pub(super) fn extend(into: &mut Group, group: Group) {
    for (name, receipts) in group {
        let values = into.entry(name).or_default();
        for receipt in receipts {
            if !values.iter().any(|r| r.binding == receipt.binding) {
                values.push(receipt);
            }
        }
    }
}
fn spelling(n: &Node) -> Option<String> {
    match n {
        Node::Name(name) | Node::Scalar(Value::String(name)) => Some(name.clone()),
        Node::Scalar(Value::Number(n)) => n.as_u64().map(|n| format!("r{n}")),
        _ => None,
    }
}
impl Reader {
    fn receipt_charge(&self, amount: usize) -> Result<(), String> {
        let total = self
            .receipt_work
            .get()
            .checked_add(amount)
            .filter(|n| *n <= 65_536)
            .ok_or("branch receipt join work limit exceeded")?;
        self.receipt_work.set(total);
        Ok(())
    }
    pub(super) fn ordinary_receipt(&self, node: &Node) -> Result<Value, String> {
        let receipts = self
            .receipt(node, &Group::new())?
            .ok_or("qualified result is not available in the ordinary scope")?;
        if receipts.len() != 1 {
            return Err(
                "result qualification matches several definitions; use a longer lexical path"
                    .into(),
            );
        }
        let mut value = document::tagged(receipts[0].id, receipts[0].binding);
        value["qualified"] = json!(true);
        Ok(value)
    }
    pub(super) fn branch(
        &mut self,
        ty: &Ty,
        node: &Node,
        label: &str,
    ) -> Result<(Value, Group), String> {
        let (ids, symbols, targets, scope, groups) = (
            self.ids.clone(),
            self.symbols.clone(),
            self.targets.clone(),
            self.scope,
            self.receipt_groups.clone(),
        );
        let available = self.available_results.clone();
        let numeric = self.numeric_bindings.clone();
        self.enter_scope(label);
        let value = self.node(ty, node);
        let mut receipts = Group::new();
        if let Ok(value) = &value {
            for (id, path) in crate::canonical::branch_receipt_sites(&document::untag(value)) {
                self.receipt_charge(2)?;
                let binding = value
                    .pointer(&path)
                    .and_then(|v| v.get("binding"))
                    .and_then(Value::as_u64)
                    .ok_or("branch receipt has no definition identity")?
                    as usize;
                let receipt = Receipt { id, binding };
                extend(
                    &mut receipts,
                    Group::from([
                        (self.bindings[binding].name.clone(), vec![receipt.clone()]),
                        (format!("r{id}"), vec![receipt]),
                    ]),
                );
            }
        }
        self.ids = ids;
        self.symbols = symbols;
        self.targets = targets;
        self.scope = scope;
        self.receipt_groups = groups;
        self.available_results = available;
        self.numeric_bindings = numeric;
        value.map(|value| (value, receipts))
    }
    pub(super) fn branches(&mut self, ty: &Ty, node: &Node) -> Result<(Value, Group), String> {
        let (Ty::Vec(inner), Node::List(items)) = (ty, node) else {
            return Err("expected alternative programs".into());
        };
        let (mut values, mut group) = (Vec::new(), Group::new());
        for item in items {
            let (value, receipts) = self.branch(inner, item, "option")?;
            values.push(value);
            extend(&mut group, receipts);
        }
        Ok((Value::Array(values), group))
    }
    fn receipt(&self, node: &Node, group: &Group) -> Result<Option<Vec<Receipt>>, String> {
        self.receipt_charge(1)?;
        if let Node::Call(name, args) = node {
            if name != "ReceiptFrom" || args.len() != 2 {
                return Err("expected a receipt name or ReceiptFrom(scope, name)".into());
            }
            let Node::Scalar(Value::String(scope)) = &args[0] else {
                return Err("receipt scope must be its quoted lexical path".into());
            };
            let name = spelling(&args[1]).ok_or("expected a receipt name")?;
            self.receipt_charge(
                self.available_results.len() + group.get(&name).map_or(0, Vec::len),
            )?;
            let ordinary = self
                .available_results
                .values()
                .filter(|binding| self.bindings[**binding].name == name)
                .filter_map(|binding| {
                    self.definitions.get(binding).map(|id| Receipt {
                        id: *id,
                        binding: *binding,
                    })
                });
            let candidates = ordinary.chain(group.get(&name).into_iter().flatten().cloned());
            let mut receipts = Vec::new();
            for receipt in candidates {
                let path = &self.bindings[receipt.binding].scope_name;
                if (path == scope || path.ends_with(&format!("/{scope}")))
                    && !receipts
                        .iter()
                        .any(|r: &Receipt| r.binding == receipt.binding)
                {
                    receipts.push(receipt);
                }
            }
            return Ok((!receipts.is_empty()).then_some(receipts));
        }
        let name = spelling(node).ok_or("expected a receipt name")?;
        if !self.symbolic_results.contains(&name) {
            if let Some(id) = name.strip_prefix('r').and_then(|s| s.parse::<u64>().ok()) {
                if let Some(binding) = self.available_results.get(&id) {
                    return Ok(Some(vec![Receipt {
                        id,
                        binding: *binding,
                    }]));
                }
            }
        }
        if let Some(binding) = self.symbols.get(&("r".into(), name.clone())) {
            if let Some(id) = self
                .definitions
                .get(binding)
                .filter(|id| self.available_results.get(id) == Some(binding))
            {
                return Ok(Some(vec![Receipt {
                    id: *id,
                    binding: *binding,
                }]));
            }
        }
        let values = group.get(&name);
        self.receipt_charge(values.map_or(0, Vec::len))?;
        Ok(values.cloned())
    }
    pub(super) fn join_receipts(&mut self, node: &Node) -> Result<Value, String> {
        let Node::List(nodes) = node else {
            return Err("expected receipt list".into());
        };
        let mut selected = None;
        for group in self.receipt_groups.iter().rev() {
            self.receipt_charge(nodes.len())?;
            let values = nodes
                .iter()
                .map(|n| self.receipt(n, group))
                .collect::<Result<Option<Vec<_>>, _>>()?;
            if let Some(values) = values {
                selected = Some(values);
                break;
            }
        }
        let Some(values) = selected else {
            // Native validation supplies the precise dangling/group error.
            return nodes
                .iter()
                .map(|n| self.id("ResultId", n))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array);
        };
        let mut output = Vec::new();
        for (node, receipts) in nodes.iter().zip(values) {
            if matches!(node,Node::Call(name,_) if name=="ReceiptFrom") && receipts.len() != 1 {
                return Err(
                    "receipt qualification matches several alternatives; use a longer lexical path"
                        .into(),
                );
            }
            self.receipt_charge(receipts.len())?;
            if receipts.len() > 1 && receipts.iter().all(|r| r.id == receipts[0].id) {
                let members: Vec<_> = receipts.iter().map(|r| r.binding).collect();
                let index = self
                    .bindings
                    .iter()
                    .position(|b| b.members == members)
                    .unwrap_or_else(|| {
                        let index = self.bindings.len();
                        self.bindings.push(BindingName {
                            kind: BindingKind::Result,
                            scope: self.scope,
                            scope_name: self
                                .scope_names
                                .get(&self.scope)
                                .cloned()
                                .unwrap_or_default(),
                            name: format!("r{}_receipts", receipts[0].id),
                            explicit: false,
                            members,
                        });
                        index
                    });
                output.push(document::tagged(receipts[0].id, index));
            } else {
                output.extend(receipts.iter().map(|r| document::tagged(r.id, r.binding)));
            }
        }
        Ok(Value::Array(output))
    }
}
impl Writer<'_> {
    pub(super) fn named_receipt(&self, member: &BindingName) -> Node {
        let ambiguous = self
            .bindings
            .iter()
            .filter(|b| {
                b.kind == BindingKind::Result && b.members.is_empty() && b.name == member.name
            })
            .count()
            > 1;
        if ambiguous {
            Node::Call(
                "ReceiptFrom".into(),
                vec![
                    Node::Scalar(json!(self.receipt_scope(member))),
                    Node::Name(member.name.clone()),
                ],
            )
        } else {
            Node::Name(member.name.clone())
        }
    }
    fn receipt_scope(&self, member: &BindingName) -> String {
        let parts: Vec<_> = member.scope_name.split('/').collect();
        for width in 1..=parts.len() {
            let suffix = parts[parts.len() - width..].join("/");
            let matches = self
                .bindings
                .iter()
                .filter(|b| {
                    b.kind == BindingKind::Result
                        && b.members.is_empty()
                        && b.name == member.name
                        && (b.scope_name == suffix || b.scope_name.ends_with(&format!("/{suffix}")))
                })
                .count();
            if matches == 1 {
                return suffix;
            }
        }
        member.scope_name.clone()
    }
    pub(super) fn joined_receipts(&self, value: &Value) -> Result<Node, String> {
        let mut nodes = Vec::new();
        for value in value.as_array().ok_or("expected receipt list")? {
            let Some(binding) = self.binding(value)? else {
                nodes.push(self.node(&named("ResultId"), value)?);
                continue;
            };
            let members: Vec<_> = if binding.members.is_empty() {
                vec![binding]
            } else {
                binding
                    .members
                    .iter()
                    .map(|i| self.bindings.get(*i).ok_or("unknown alternative receipt"))
                    .collect::<Result<_, _>>()?
            };
            for member in members {
                nodes.push(self.named_receipt(member));
            }
        }
        Ok(Node::List(nodes))
    }
}
