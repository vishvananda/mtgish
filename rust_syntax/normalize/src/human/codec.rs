use super::{
    defaults,
    document::{self, BindingKind, BindingName},
    names, schema,
    syntax::{self, Node},
};
use crate::{
    ir::*,
    ir_schema::{Fields, Ty, TypeDef},
    targets::TargetSelector,
};
use serde_json::{json, Value};
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
mod receipts;
mod results;
fn named(n: &str) -> Ty {
    Ty::Named(n.into())
}
fn symbol(prefix: &str, n: u64) -> Node {
    Node::Name(format!("{prefix}{n}"))
}
pub(super) fn valid_binding_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        && !s.as_bytes()[0].is_ascii_digit()
        && !matches!(s, "true" | "false" | "None")
}
fn literal(s: &str) -> Node {
    if valid_binding_name(s) {
        Node::Name(s.into())
    } else {
        Node::Scalar(json!(s))
    }
}
fn enum_value<'a>(v: &'a Value, adjacent: bool) -> Result<(&'a str, &'a Value), String> {
    if adjacent {
        Ok((
            v["kind"].as_str().ok_or("missing enum kind")?,
            v.get("args").unwrap_or(&Value::Null),
        ))
    } else if let Some(s) = v.as_str() {
        Ok((s, &Value::Null))
    } else {
        let obj = v.as_object().ok_or("expected enum object")?;
        if obj.len() != 1 {
            return Err("expected one enum variant".into());
        }
        let (n, v) = obj.iter().next().unwrap();
        Ok((n, v))
    }
}
fn pack(adjacent: bool, name: &str, fields: &Fields, args: Value) -> Value {
    if adjacent {
        if matches!(fields, Fields::Unit) {
            json!({"kind":name})
        } else {
            json!({"kind":name,"args":args})
        }
    } else if matches!(fields, Fields::Unit) {
        json!(name)
    } else {
        json!({name:args})
    }
}
fn fields_for<'a>(t: &Ty, v: &'a Value) -> Option<(&'static Fields, &'a Value, String)> {
    let Ty::Named(n) = t else { return None };
    match schema().types.get(n)? {
        TypeDef::Struct(f) => Some((f, v, n.clone())),
        TypeDef::Enum { adjacent, variants } => {
            let (name, args) = enum_value(v, *adjacent).ok()?;
            Some((
                &variants.iter().find(|(s, _)| s == name)?.1,
                args,
                format!("{n}::{name}"),
            ))
        }
    }
}
fn references(t: &Ty, v: &Value, out: &mut HashSet<u32>, root: bool) {
    if !root && t == &named("Ability") {
        return;
    }
    if let Ty::Named(n) = t {
        if n == "ResultId" {
            if let Some(id) = document::id_number(v) {
                out.insert(id as u32);
            }
            return;
        }
    }
    match t {
        Ty::Vec(t) => {
            if let Some(a) = v.as_array() {
                for v in a {
                    references(t, v, out, false)
                }
            }
        }
        Ty::Option(t) => {
            if !v.is_null() {
                references(t, v, out, false)
            }
        }
        Ty::Tuple(ts) => {
            if let Some(vs) = v.as_array() {
                for (t, v) in ts.iter().zip(vs) {
                    references(t, v, out, false)
                }
            }
        }
        _ => {
            if let Some((fields, args, owner)) = fields_for(t, v) {
                match fields {
                    Fields::Named(fs) => {
                        for (n, t) in fs {
                            if !(owner == "Instruction::Effect" && n == "result") {
                                references(t, &args[n], out, false)
                            }
                        }
                    }
                    Fields::Tuple(ts) => {
                        for (i, t) in ts.iter().enumerate() {
                            references(t, if ts.len() == 1 { args } else { &args[i] }, out, false)
                        }
                    }
                    Fields::Unit => (),
                }
            }
        }
    }
}
struct Writer<'a> {
    reads: HashSet<u32>,
    bindings: &'a [BindingName],
}
impl Writer<'_> {
    fn binding(&self, value: &Value) -> Result<Option<&BindingName>, String> {
        value
            .get("binding")
            .and_then(Value::as_u64)
            .map(|i| {
                self.bindings
                    .get(i as usize)
                    .ok_or("unknown codec binding".into())
            })
            .transpose()
    }
    fn node(&self, t: &Ty, v: &Value) -> Result<Node, String> {
        if t == &named("Ability") {
            let mut reads = HashSet::new();
            references(t, v, &mut reads, true);
            return Writer {
                reads,
                bindings: self.bindings,
            }
            .inner(t, v);
        }
        self.inner(t, v)
    }
    fn inner(&self, t: &Ty, v: &Value) -> Result<Node, String> {
        match t {
            Ty::Option(inner) => {
                if v.is_null() {
                    return Ok(Node::Scalar(Value::Null));
                }
                let n = self.node(inner, v)?;
                return Ok(
                    if matches!(inner.as_ref(), Ty::Option(_))
                        || matches!(&n, Node::Scalar(Value::Null))
                    {
                        Node::Call("Some".into(), vec![n])
                    } else {
                        n
                    },
                );
            }
            Ty::Vec(inner) => {
                return Ok(Node::List(
                    v.as_array()
                        .ok_or("expected list")?
                        .iter()
                        .map(|v| self.node(inner, v))
                        .collect::<Result<_, _>>()?,
                ))
            }
            Ty::Tuple(ts) => {
                return Ok(Node::List(
                    ts.iter()
                        .zip(v.as_array().ok_or("expected tuple")?)
                        .map(|(t, v)| self.node(t, v))
                        .collect::<Result<_, _>>()?,
                ))
            }
            Ty::Opaque => return Ok(raw_node(v)),
            Ty::Named(n) => {
                if let Some(kind) = BindingKind::of_type(n) {
                    let prefix = kind.prefix();
                    return Ok(if let Some(binding) = self.binding(v)? {
                        if kind == BindingKind::Result && v.get("qualified") == Some(&json!(true)) {
                            self.named_receipt(binding)
                        } else {
                            Node::Name(binding.name.clone())
                        }
                    } else {
                        symbol(prefix, document::id_number(v).ok_or("expected ID")?)
                    });
                }
                if n == "Expr" && v["kind"] == "Constant" {
                    return Ok(Node::Scalar(v["args"].clone()));
                }
                if n == "Program" {
                    return self.node(&Ty::Vec(Box::new(named("Instruction"))), &v["instructions"]);
                }
                if n == "TargetSpec" {
                    return self.target(v);
                }
                if n == "Instruction" && v["kind"] == "Effect" {
                    let op = self.node(&named("Operation"), &v["args"]["operation"])?;
                    let result = &v["args"]["result"];
                    let id = document::id_number(result).ok_or("missing result ID")?;
                    let binding = self.binding(result)?;
                    return Ok(
                        if self.reads.contains(&(id as u32)) || binding.is_some_and(|b| b.explicit)
                        {
                            Node::Bind(
                                binding.map_or_else(|| format!("r{id}"), |b| b.name.clone()),
                                Box::new(op),
                            )
                        } else {
                            op
                        },
                    );
                }
                if n == "PlayerRef" && v == &json!({"kind":"AbilityController"}) {
                    return Ok(Node::Name("You".into()));
                }
                if n == "PlayerSet"
                    && v == &json!({"kind":"One","args":{"kind":"AbilityController"}})
                {
                    return Ok(Node::Name("You".into()));
                }
                if n == "ObjectRef" && v == &json!({"kind":"Source"}) {
                    return Ok(Node::Name("This".into()));
                }
                if n == "Recipients" {
                    let slot = if v["kind"] == "Targets" {
                        Some(&v["args"])
                    } else if matches!(v["kind"].as_str(), Some("Objects" | "Players"))
                        && v["args"]["kind"] == "Targets"
                    {
                        Some(&v["args"]["args"])
                    } else {
                        None
                    };
                    if let Some(slot) = slot {
                        return Ok(Node::Call(
                            "Targets".into(),
                            vec![self.node(&named("TargetSlot"), slot)?],
                        ));
                    }
                }
                let Some(d) = schema().types.get(n) else {
                    return Ok(raw_node(v));
                };
                return match d {
                    TypeDef::Struct(fields) => self.fields(None, n, fields, v),
                    TypeDef::Enum { adjacent, variants } => {
                        let (variant, args) = enum_value(v, *adjacent)?;
                        let Some((_, fields)) = variants.iter().find(|(s, _)| s == variant) else {
                            return Err(format!("unknown {n}::{variant}"));
                        };
                        let display = if n == "Operation" {
                            names::action_name(variant)
                        } else {
                            names::reference_name(n, variant).unwrap_or(variant)
                        };
                        // Empty arrival payloads add no authoring information. The
                        // reader accepts the bare destination with its typed default.
                        if let Fields::Tuple(ts) = fields {
                            if let [Ty::Named(arrival)] = ts.as_slice() {
                                if matches!(
                                    arrival.as_str(),
                                    "CardArrival" | "BattlefieldArrival" | "ExileArrival"
                                ) && defaults::type_value(arrival).as_ref() == Some(args)
                                {
                                    return Ok(Node::Name(display.into()));
                                }
                            }
                        }
                        self.fields(Some(display), &format!("{n}::{variant}"), fields, args)
                    }
                };
            }
        }
    }
    fn fields(
        &self,
        display: Option<&str>,
        owner: &str,
        f: &Fields,
        v: &Value,
    ) -> Result<Node, String> {
        Ok(match f {
            Fields::Unit => literal(display.ok_or("unit needs a name")?),
            Fields::Tuple(ts) => {
                let items = ts
                    .iter()
                    .enumerate()
                    .map(|(i, t)| self.node(t, if ts.len() == 1 { v } else { &v[i] }))
                    .collect::<Result<_, _>>()?;
                if let Some(n) = display {
                    Node::Call(n.into(), items)
                } else {
                    Node::List(items)
                }
            }
            Fields::Named(fs) => {
                let mut fields = vec![];
                for (key, ty) in fs {
                    let value = v.get(key).ok_or_else(|| format!("missing {owner}.{key}"))?;
                    if defaults::field(owner, key, ty).as_ref() == Some(value) {
                        continue;
                    }
                    if key == "announcement" && ty == &named("Announcement") {
                        for (name, t) in [
                            ("targets", Ty::Vec(Box::new(named("TargetSpec")))),
                            ("modes", Ty::Option(Box::new(named("ModeSelection")))),
                        ] {
                            if value[name] != json!([]) && !value[name].is_null() {
                                fields.push((name.into(), self.node(&t, &value[name])?));
                            }
                        }
                        continue;
                    }
                    let node = if owner == "Operation::JoinResults" && key == "results" {
                        self.joined_receipts(value)?
                    } else if key == "preventable" {
                        Node::Scalar(json!(!value.as_bool().ok_or("expected prevention flag")?))
                    } else {
                        self.node(ty, value)?
                    };
                    fields.push((names::field(owner, key), node));
                }
                Node::Record(display.map(str::to_owned), fields)
            }
        })
    }
    fn target(&self, v: &Value) -> Result<Node, String> {
        let spec: TargetSpec =
            serde_json::from_value(document::untag(v)).map_err(|e| e.to_string())?;
        let mut selector = serde_json::to_value(spec.selector()?).map_err(|e| e.to_string())?;
        match selector["kind"].as_str() {
            Some("Objects") => selector["args"]["filter"] = v["filter"].clone(),
            Some("Players") => selector["args"]["filter"] = v["players"].clone(),
            Some("Damageable") => {
                selector["args"]["objects"] = v["filter"].clone();
                selector["args"]["players"] = v["players"].clone();
            }
            _ => (),
        }
        let select = selector_preset(&selector)
            .map(|s| Node::Name(s.into()))
            .map(Ok)
            .unwrap_or_else(|| self.node(&named("TargetSelector"), &selector))?;
        let mut fields = vec![
            ("name".into(), self.node(&named("TargetSlot"), &v["slot"])?),
            ("select".into(), select),
        ];
        let TypeDef::Struct(Fields::Named(fs)) = &schema().types["TargetSpec"] else {
            unreachable!()
        };
        for (key, t) in fs {
            if ["slot", "kind", "filter", "players"].contains(&key.as_str())
                || defaults::field("TargetSpec", key, t).as_ref() == Some(&v[key])
            {
                continue;
            }
            fields.push((key.clone(), self.node(t, &v[key])?));
        }
        Ok(Node::Record(Some("Target".into()), fields))
    }
}
fn selector_preset(v: &Value) -> Option<&'static str> {
    for (n, s) in presets() {
        if serde_json::to_value(s).unwrap() == *v {
            return Some(n);
        }
    }
    None
}
fn presets() -> [(&'static str, TargetSelector); 4] {
    [
        ("AnyTarget", TargetSelector::any_target()),
        ("Creature", TargetSelector::creature()),
        ("Planeswalker", TargetSelector::planeswalker()),
        ("Player", TargetSelector::player()),
    ]
}
fn raw_node(v: &Value) -> Node {
    match v {
        Value::Array(a) => Node::List(a.iter().map(raw_node).collect()),
        Value::Object(o) => Node::Record(
            None,
            o.iter().map(|(k, v)| (k.clone(), raw_node(v))).collect(),
        ),
        _ => Node::Scalar(v.clone()),
    }
}
pub(super) fn write(v: &Value) -> Result<String, String> {
    write_with_bindings(v, &[])
}
pub(super) fn write_with_bindings(v: &Value, bindings: &[BindingName]) -> Result<String, String> {
    let mut reads = HashSet::new();
    references(&named("UnifiedCard"), v, &mut reads, true);
    let w = Writer { reads, bindings };
    let mut node = if v["layout"] == "Card"
        && v["data"] == json!([])
        && v["faces"].as_array().is_some_and(|a| a.len() == 1)
    {
        w.node(&named("Card"), &v["faces"][0])?
    } else {
        w.node(&named("UnifiedCard"), v)?
    };
    if let Node::Record(name, _) = &mut node {
        *name = Some(
            if v["layout"] == "Card"
                && v["data"] == json!([])
                && v["faces"].as_array().is_some_and(|a| a.len() == 1)
            {
                "Card"
            } else {
                "UnifiedCard"
            }
            .into(),
        );
    }
    Ok(syntax::render(&node, 0))
}
struct Reader {
    targets: HashMap<u16, TargetKind>,
    ids: HashMap<(String, String), u64>,
    used: HashMap<String, HashSet<u64>>,
    anon: u32,
    nodes: usize,
    symbols: HashMap<(String, String), usize>,
    bindings: Vec<BindingName>,
    scope: u32,
    next_scope: u32,
    binding_namespace: &'static str,
    branch_scopes: bool,
    definitions: HashMap<usize, u64>,
    receipt_groups: Vec<receipts::Group>,
    symbolic_results: HashSet<String>,
    receipt_work: Cell<usize>,
    scope_names: HashMap<u32, String>,
    scope_counts: HashMap<(u32, String), usize>,
    available_results: HashMap<u64, usize>,
    operation_depth: usize,
    payment_depth: Option<usize>,
    numeric_bindings: HashMap<(String, u64), usize>,
    declaring: bool,
}
impl Reader {
    fn enter_scope(&mut self, label: &str) {
        let label = label.to_ascii_lowercase();
        let count = self
            .scope_counts
            .entry((self.scope, label.clone()))
            .or_default();
        *count += 1;
        let name = if *count == 1 && matches!(label.as_str(), "then" | "otherwise") {
            label
        } else {
            format!("{label}_{count}")
        };
        let parent = self
            .scope_names
            .get(&self.scope)
            .cloned()
            .unwrap_or_default();
        self.next_scope += 1;
        self.scope = self.next_scope;
        self.scope_names.insert(
            self.scope,
            if parent.is_empty() {
                name
            } else {
                format!("{parent}/{name}")
            },
        );
    }
    fn reserve(&mut self, n: &Node) {
        if let Node::Call(name, args) = n {
            if name == "ReceiptFrom" && args.len() == 2 {
                if let Node::Name(name) | Node::Scalar(Value::String(name)) = &args[1] {
                    self.symbolic_results.insert(name.clone());
                }
            }
        }
        if let Node::Bind(_, inner) = n {
            self.reserve(inner);
        }
        match n {
            Node::Name(n) | Node::Bind(n, _) => {
                for prefix in ["r", "t", "b", "s"] {
                    if let Some(id) = n.strip_prefix(prefix).and_then(|s| s.parse::<u64>().ok()) {
                        self.used.entry(prefix.into()).or_default().insert(id);
                    }
                }
            }
            Node::List(ns) | Node::Call(_, ns) => {
                for n in ns {
                    self.reserve(n)
                }
            }
            Node::Record(name, fs) => {
                if self.branch_scopes {
                    if name.as_deref() == Some("Effect") {
                        if let Some((_, value)) = fs.iter().find(|(key, _)| key == "result") {
                            self.reserve_declaration("r", value);
                        }
                    }
                    if name.as_deref() == Some("Target")
                        || name.is_none() && fs.iter().any(|(key, _)| key == "select")
                    {
                        if let Some((_, value)) = fs.iter().find(|(key, _)| key == "name") {
                            self.reserve_declaration("t", value);
                        }
                    }
                    if let Some((_, value)) = fs.iter().find(|(key, _)| key == "binding") {
                        self.reserve_declaration("b", value);
                        self.reserve_declaration("s", value);
                    }
                }
                for (_, n) in fs {
                    self.reserve(n)
                }
            }
            _ => (),
        }
    }
    fn reserve_declaration(&mut self, prefix: &str, node: &Node) {
        let id = match node {
            Node::Scalar(Value::Number(n)) => n.as_u64(),
            Node::Name(name) | Node::Scalar(Value::String(name)) => name
                .strip_prefix(prefix)
                .and_then(|s| s.parse::<u64>().ok()),
            _ => None,
        };
        if let Some(id) = id {
            self.used.entry(prefix.into()).or_default().insert(id);
        }
    }
    fn id(&mut self, family: &str, n: &Node) -> Result<Value, String> {
        let kind = BindingKind::of_type(family).ok_or("unknown binding kind")?;
        if kind == BindingKind::Result && matches!(n,Node::Call(name,_) if name=="ReceiptFrom") {
            return self.ordinary_receipt(n);
        }
        let prefix = kind.prefix();
        let name = match n {
            Node::Name(s) | Node::Scalar(Value::String(s)) => s.clone(),
            Node::Scalar(Value::Number(n)) => format!(
                "{prefix}{}",
                n.as_u64().ok_or("expected an unsigned binding ID")?
            ),
            _ => return Err("expected a binding name".into()),
        };
        if !valid_binding_name(&name) {
            return Err("a binding name must be an identifier".into());
        }
        let raw = (!matches!(kind, BindingKind::Result) || !self.symbolic_results.contains(&name))
            .then(|| {
                name.strip_prefix(prefix)
                    .and_then(|s| s.parse::<u64>().ok())
            })
            .flatten();
        let namespace = if kind == BindingKind::Loop {
            self.binding_namespace
        } else {
            prefix
        };
        let key = (namespace.into(), name.clone());
        let id = if let Some(id) = raw {
            self.used.entry(prefix.into()).or_default().insert(id);
            id
        } else if let Some(id) = self.ids.get(&key) {
            *id
        } else {
            let used = self.used.entry(prefix.into()).or_default();
            let max = kind.maximum();
            let id = (0..=max)
                .find(|n| !used.contains(n))
                .ok_or("binding IDs exhausted")?;
            used.insert(id);
            self.ids.insert(key.clone(), id);
            id
        };
        if self.branch_scopes && !self.declaring && raw.is_some() {
            let existing = if kind == BindingKind::Result {
                self.available_results.get(&id)
            } else {
                self.numeric_bindings.get(&(namespace.into(), id))
            };
            if let Some(index) = existing {
                return Ok(document::tagged(id, *index));
            }
        }
        let index = if let Some(index) = self.symbols.get(&key) {
            *index
        } else {
            let index = self.bindings.len();
            self.bindings.push(BindingName {
                kind: BindingKind::of_type(family).ok_or("unknown binding kind")?,
                scope: self.scope,
                scope_name: self
                    .scope_names
                    .get(&self.scope)
                    .cloned()
                    .unwrap_or_default(),
                name,
                explicit: raw.is_none(),
                members: vec![],
            });
            self.symbols.insert(key, index);
            index
        };
        Ok(document::tagged(id, index))
    }
    fn declare(&mut self, family: &str, node: &Node) -> Result<Value, String> {
        let kind = BindingKind::of_type(family).ok_or("unknown binding kind")?;
        let prefix = kind.prefix();
        let name = match node {
            Node::Name(name) | Node::Scalar(Value::String(name)) => Some(name.clone()),
            Node::Scalar(Value::Number(n)) => n.as_u64().map(|id| format!("{prefix}{id}")),
            _ => None,
        };
        if let Some(name) = name {
            let namespace = if kind == BindingKind::Loop {
                self.binding_namespace
            } else {
                prefix
            };
            let key = (namespace.into(), name);
            if self
                .symbols
                .get(&key)
                .is_some_and(|i| self.bindings[*i].scope != self.scope)
            {
                self.ids.remove(&key);
                self.symbols.remove(&key);
            }
        }
        let prior = self.declaring;
        self.declaring = true;
        let value = self.id(family, node);
        self.declaring = prior;
        let value = value?;
        let index = value["binding"].as_u64().ok_or("missing codec binding")? as usize;
        if !matches!(node, Node::Scalar(Value::Number(_))) {
            self.bindings[index].explicit = true;
        }
        if kind == BindingKind::Result {
            self.definitions.insert(
                index,
                document::id_number(&value).ok_or("missing result ID")?,
            );
            self.available_results.insert(
                document::id_number(&value).ok_or("missing result ID")?,
                index,
            );
        } else {
            let namespace = if kind == BindingKind::Loop {
                self.binding_namespace
            } else {
                prefix
            };
            self.numeric_bindings.insert(
                (
                    namespace.into(),
                    document::id_number(&value).ok_or("missing binding ID")?,
                ),
                index,
            );
        }
        Ok(value)
    }
    fn anonymous(&mut self) -> Result<Value, String> {
        let used = self.used.entry("r".into()).or_default();
        while used.contains(&(self.anon as u64)) {
            self.anon = self.anon.checked_sub(1).ok_or("result IDs exhausted")?
        }
        let id = self.anon;
        used.insert(id as u64);
        let index = self.bindings.len();
        self.bindings.push(BindingName {
            kind: BindingKind::Result,
            scope: self.scope,
            scope_name: self
                .scope_names
                .get(&self.scope)
                .cloned()
                .unwrap_or_default(),
            name: format!("r{id}"),
            explicit: false,
            members: vec![],
        });
        self.definitions.insert(index, id.into());
        self.available_results.insert(id.into(), index);
        Ok(document::tagged(id.into(), index))
    }
    fn node(&mut self, t: &Ty, n: &Node) -> Result<Value, String> {
        let operation = matches!(t, Ty::Named(name) if name == "Operation");
        if operation {
            self.operation_depth += 1;
        }
        let result = self.scoped_node(t, n, false);
        if operation {
            self.operation_depth -= 1;
        }
        result
    }
    fn scoped_node(&mut self, t: &Ty, n: &Node, private_program: bool) -> Result<Value, String> {
        let independent = t == &named("Ability");
        let cost_scope = t == &named("CostPart") && constructor(n) == Some("PlayerAction");
        let scope = independent
            || private_program
            || cost_scope
            || t == &named("Operand") && constructor(n) == Some("Program")
            || t == &named("EventChange") && constructor(n) == Some("Then")
            || t == &named("Mode")
            || (t == &named("Operation")
                && matches!(
                    constructor(n),
                    Some(
                        "CreateTrigger"
                            | "ReflexiveTrigger"
                            | "CreateDelayedTrigger"
                            | "CreateReplacement"
                    )
                ));
        let local_scope = (t == &named("Instruction")
            && matches!(
                constructor(n),
                Some("ForEachPlayer" | "ForEachObject" | "Optional" | "RepeatableWithLimit" | "Repeatable")
            ))
            || self.branch_scopes
                && t == &named("Instruction")
                && matches!(constructor(n), Some("If" | "ChooseOne"))
            || t == &named("Condition") && constructor(n) == Some("ExistsPlayer")
            || t == &named("PlayerSet") && constructor(n) == Some("Matching");
        let selection_scope = t == &named("Operation") && constructor(n) == Some("WithSelections");
        if scope || local_scope || selection_scope {
            let prior_targets = self.targets.clone();
            let prior_ids = self.ids.clone();
            let prior_symbols = self.symbols.clone();
            let prior_scope = self.scope;
            let prior_receipts = self.receipt_groups.clone();
            let prior_available = self.available_results.clone();
            let prior_numeric = self.numeric_bindings.clone();
            // Keep the historical program scope identity when a typed body
            // replaces Operand::Program in a persisted authoring document.
            self.enter_scope(if private_program {
                "Program"
            } else {
                constructor(n).unwrap_or(match t {
                    Ty::Named(name) => name,
                    _ => "scope",
                })
            });
            if independent {
                self.targets.clear();
                self.ids.clear();
                self.symbols.clear();
                self.available_results.clear();
                self.numeric_bindings.clear();
            }
            if independent
                || private_program
                || t == &named("Mode")
                || t == &named("Operand") && constructor(n) == Some("Program")
                || t == &named("Operation") && scope
            {
                self.receipt_groups.clear();
            }
            // Preparation inputs inherit earlier selections. A local binder
            // shadows only after its inputs, matching native validation.
            let result = self.inner(t, n);
            self.targets = prior_targets;
            if scope {
                self.ids = prior_ids;
                self.symbols = prior_symbols;
                self.receipt_groups = prior_receipts;
                self.available_results = prior_available;
                self.numeric_bindings = prior_numeric;
            } else {
                let local = if local_scope { "b" } else { "s" };
                self.numeric_bindings.retain(|(kind, _), _| {
                    kind != "t"
                        && if local == "b" {
                            !kind.ends_with("_binding") && kind != "b"
                        } else {
                            kind != "s"
                        }
                });
                self.numeric_bindings.extend(prior_numeric);
                self.ids.retain(|(kind, _), _| {
                    if local == "b" {
                        !kind.ends_with("_binding") && kind != "b"
                    } else {
                        kind != local
                    }
                });
                self.symbols.retain(|(kind, _), _| {
                    if local == "b" {
                        !kind.ends_with("_binding") && kind != "b"
                    } else {
                        kind != local
                    }
                });
                for (key, value) in &prior_ids {
                    self.ids.insert(key.clone(), *value);
                }
                for (key, value) in &prior_symbols {
                    self.symbols.insert(key.clone(), *value);
                }
                if self.branch_scopes && t == &named("Instruction") {
                    self.finish_control(
                        constructor(n).unwrap_or(""),
                        result.as_ref().ok(),
                        &prior_ids,
                        &prior_symbols,
                        prior_available,
                        prior_receipts,
                    )?;
                }
            }
            self.scope = prior_scope;
            return result;
        }
        self.inner(t, n)
    }
    fn inner(&mut self, t: &Ty, n: &Node) -> Result<Value, String> {
        self.nodes += 1;
        if self.nodes > 500_000 {
            return Err("card code exceeds lowering work limit".into());
        }
        match t {
            Ty::Option(inner) => {
                if matches!(n, Node::Scalar(Value::Null)) {
                    return Ok(Value::Null);
                }
                if let Node::Call(name, ns) = n {
                    if name == "Some" && ns.len() == 1 {
                        return self.node(inner, &ns[0]);
                    }
                }
                self.node(inner, n)
            }
            Ty::Vec(inner) => {
                let Node::List(ns) = n else {
                    return Err("expected a list".into());
                };
                Ok(Value::Array(
                    ns.iter()
                        .map(|n| self.node(inner, n))
                        .collect::<Result<_, _>>()?,
                ))
            }
            Ty::Tuple(ts) => {
                let Node::List(ns) = n else {
                    return Err("expected a tuple list".into());
                };
                if ns.len() != ts.len() {
                    return Err("tuple length mismatch".into());
                }
                Ok(Value::Array(
                    ts.iter()
                        .zip(ns)
                        .map(|(t, n)| self.node(t, n))
                        .collect::<Result<_, _>>()?,
                ))
            }
            Ty::Opaque => raw_value(n),
            Ty::Named(name) => {
                if BindingKind::of_type(name).is_some() {
                    return self.id(name, n);
                }
                if name == "Program" {
                    return Ok(
                        json!({"instructions":self.node(&Ty::Vec(Box::new(named("Instruction"))),n)?}),
                    );
                }
                if name == "TargetSpec" {
                    return self.target(n);
                }
                if name == "Expr" && matches!(n, Node::Scalar(Value::Number(_))) {
                    return Ok(json!({"kind":"Constant","args":raw_value(n)?}));
                }
                if matches!(n,Node::Name(s) if s=="You") {
                    if name == "PlayerRef" {
                        return Ok(json!({"kind":"AbilityController"}));
                    }
                    if name == "PlayerSet" {
                        return Ok(json!({"kind":"One","args":{"kind":"AbilityController"}}));
                    }
                }
                if name == "ObjectRef" && matches!(n,Node::Name(s) if s=="This") {
                    return Ok(json!({"kind":"Source"}));
                }
                if name == "Recipients" {
                    if let Node::Call(name, args) = n {
                        if name == "Targets" && args.len() == 1 {
                            let slot = self.id("TargetSlot", &args[0])?;
                            let id: u16 = document::id_number(&slot)
                                .ok_or("expected target ID")?
                                .try_into()
                                .map_err(|_| "target ID exceeds u16")?;
                            return Ok(
                                match self
                                    .targets
                                    .get(&id)
                                    .ok_or_else(|| format!("unbound target {id}"))?
                                {
                                    TargetKind::DamageRecipient => {
                                        json!({"kind":"Targets","args":slot})
                                    }
                                    TargetKind::Player => {
                                        json!({"kind":"Players","args":{"kind":"Targets","args":slot}})
                                    }
                                    TargetKind::Object(_) => {
                                        json!({"kind":"Objects","args":{"kind":"Targets","args":slot}})
                                    }
                                },
                            );
                        }
                    }
                }
                if name == "Instruction" {
                    let (binding, op) = if let Node::Bind(b, n) = n {
                        (Some(b), n.as_ref())
                    } else {
                        (None, n)
                    };
                    let constructor = constructor(op).map(|n| names::action_wire(n));
                    if constructor.is_some_and(|n|matches!(&schema().types["Operation"],TypeDef::Enum{variants,..} if variants.iter().any(|(v,_)|v==n))){
                        let operation=self.operation_input(op)?;
                        let result=if let Some(name)=binding{self.declare("ResultId",&Node::Name(name.clone()))?}else{self.anonymous()?};
                        return Ok(json!({"kind":"Effect","args":{"result":result,"operation":operation}}));
                    }
                    if binding.is_some() {
                        return Err("only an action can bind a result".into());
                    }
                }
                if name == "MovementDestination" && super::movement::old_address(n) {
                    let old = self.node(&named("Destination"), n)?;
                    return super::movement::address(old);
                }
                let Some(d) = schema()
                    .types
                    .get(name)
                    .or_else(|| super::movement::legacy_type(name))
                else {
                    return raw_value(n);
                };
                match d {
                    TypeDef::Struct(f) => self.fields(name, Some(name), f, n),
                    TypeDef::Enum { adjacent, variants } => {
                        let raw =
                            constructor(n).ok_or_else(|| format!("expected {name} constructor"))?;
                        let variant = if name == "Operation" {
                            names::action_wire(raw)
                        } else {
                            names::reference_wire(name, raw)
                        };
                        let (_, fields) = variants
                            .iter()
                            .find(|(v, _)| v == variant)
                            .ok_or_else(|| format!("unknown {name} constructor {raw}"))?;
                        let owner = format!("{name}::{variant}");
                        if let Some(old) = super::movement::legacy_fields(name, variant, n) {
                            let args = self.fields(&owner, Some(raw), old, n)?;
                            return super::movement::upgrade(
                                &owner,
                                pack(*adjacent, variant, old, args),
                            );
                        }
                        let args = self.fields(&owner, Some(raw), fields, n)?;
                        Ok(pack(*adjacent, variant, fields, args))
                    }
                }
            }
        }
    }
    fn fields(
        &mut self,
        owner: &str,
        name: Option<&str>,
        f: &Fields,
        n: &Node,
    ) -> Result<Value, String> {
        let saved = self.binding_namespace;
        if let Some(namespace) = crate::binding_roles::namespace(owner) {
            self.binding_namespace = namespace;
        }
        let result = self.fields_inner(owner, name, f, n);
        self.binding_namespace = saved;
        result
    }
    fn fields_inner(
        &mut self,
        owner: &str,
        name: Option<&str>,
        f: &Fields,
        n: &Node,
    ) -> Result<Value, String> {
        match f {
            Fields::Unit => match n {
                Node::Name(_) | Node::Scalar(Value::String(_)) => Ok(Value::Null),
                _ => Err("unit constructor takes no operands".into()),
            },
            Fields::Tuple(ts) => {
                if ts.len() == 1 && matches!(n, Node::Name(_) | Node::Scalar(Value::String(_))) {
                    if let Ty::Named(ty) = &ts[0] {
                        if matches!(
                            ty.as_str(),
                            "CardArrival" | "BattlefieldArrival" | "ExileArrival"
                        ) {
                            if let Some(value) = defaults::type_value(ty) {
                                return Ok(value);
                            }
                        }
                    }
                }
                let ns = match n {
                    Node::Call(_, ns) | Node::List(ns) => ns,
                    _ => return Err(format!("{owner} needs tuple operands")),
                };
                if ns.len() != ts.len() {
                    return Err(format!("{owner} expects {} operands", ts.len()));
                }
                let mut vs = ts
                    .iter()
                    .zip(ns)
                    .map(|(t, n)| self.node(t, n))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(if vs.len() == 1 {
                    vs.remove(0)
                } else {
                    Value::Array(vs)
                })
            }
            Fields::Named(fs) => {
                let Node::Record(given, values) = n else {
                    return Err(format!("{owner} needs named fields"));
                };
                if given.as_deref().is_some_and(|g| Some(g) != name) {
                    return Err(format!("expected {} record", name.unwrap_or(owner)));
                }
                let mut out = serde_json::Map::new();
                let mut consumed = HashSet::new();
                let mut branch_receipts = receipts::Group::new();
                // Inputs are evaluated before result/selection definitions. A
                // loop binder is visible only in its body or predicate.
                let has_binding = fs.iter().any(|(key, _)| key == "binding");
                let mut ordered: Vec<_> = fs.iter().collect();
                ordered.sort_by_key(|(key, _)| {
                    if owner == "Ability::Activated" {
                        return match key.as_str() {
                            "announcement" => 1,
                            "limits" => 2,
                            "body" => 3,
                            _ => 0,
                        };
                    }
                    if owner == "Instruction::Effect" && key == "result"
                        || owner == "SelectionStep" && key == "binding"
                        || owner == "Mode" && key == "body"
                    {
                        3
                    } else if has_binding
                        && owner != "SelectionStep"
                        && matches!(key.as_str(), "body" | "condition")
                    {
                        2
                    } else if has_binding && owner != "SelectionStep" && key == "binding" {
                        1
                    } else {
                        0
                    }
                });
                for (key, t) in ordered {
                    if key == "announcement"
                        && t == &named("Announcement")
                        && values.iter().any(|(k, _)| k == "targets" || k == "modes")
                    {
                        let announcement: Vec<_> = values
                            .iter()
                            .filter(|(k, _)| k == "targets" || k == "modes")
                            .cloned()
                            .collect();
                        for (k, _) in &announcement {
                            consumed.insert(k.clone());
                        }
                        out.insert(
                            key.clone(),
                            self.node(t, &Node::Record(None, announcement))?,
                        );
                        continue;
                    }
                    let alias = names::field(owner, key);
                    let candidates: Vec<_> = values
                        .iter()
                        .filter(|(k, _)| {
                            k == key
                                || k == &alias
                                || names::legacy_field(owner, key) == Some(k.as_str())
                        })
                        .collect();
                    if candidates.len() > 1 {
                        return Err(format!("duplicate aliases for {owner}.{key}"));
                    }
                    let v = if let Some((k, n)) = candidates.first() {
                        consumed.insert(k.clone());
                        let v = if (key == "binding"
                            || owner == "Instruction::Effect" && key == "result")
                            && matches!(t,Ty::Named(name) if BindingKind::of_type(name).is_some())
                        {
                            let Ty::Named(family) = t else { unreachable!() };
                            self.declare(family, n)
                        } else if self.branch_scopes
                            && owner == "Instruction::If"
                            && matches!(key.as_str(), "then" | "otherwise")
                        {
                            self.branch(t, n, key).map(|(value, receipts)| {
                                receipts::extend(&mut branch_receipts, receipts);
                                value
                            })
                        } else if self.branch_scopes
                            && owner == "Instruction::ChooseOne"
                            && key == "options"
                        {
                            self.branches(t, n).map(|(value, receipts)| {
                                receipts::extend(&mut branch_receipts, receipts);
                                value
                            })
                        } else if self.branch_scopes
                            && owner == "Operation::JoinResults"
                            && key == "results"
                        {
                            self.join_receipts(n)
                        } else if matches!(
                            owner,
                            "EntryState::ExiledForWith" | "ExileState::ExiledForWith"
                        ) && key == "body"
                        {
                            self.scoped_node(t, n, true)
                        } else if owner == "Instruction::Effect" && key == "operation" {
                            self.operation_input(n)
                        } else if owner == "Operation::PayCost"
                            && key == "cost"
                            && (!self.branch_scopes
                                || self.payment_depth == Some(self.operation_depth))
                        {
                            let prior: HashSet<_> = self.symbols.keys().cloned().collect();
                            let parsed = self.node(t, n);
                            if let Ok(cost) = &parsed {
                                for part in cost["parts"].as_array().into_iter().flatten() {
                                    let body = &part["PlayerAction"]["body"];
                                    for (id, path) in crate::canonical::exported_receipt_sites(
                                        &document::untag(body),
                                        false,
                                    ) {
                                        let Some(binding) = body
                                            .pointer(&path)
                                            .and_then(|v| v.get("binding"))
                                            .and_then(Value::as_u64)
                                            .map(|i| i as usize)
                                        else {
                                            continue;
                                        };
                                        let key = ("r".into(), self.bindings[binding].name.clone());
                                        self.available_results.insert(id, binding);
                                        if !prior.contains(&key) {
                                            self.ids.insert(key.clone(), id);
                                            self.symbols.insert(key, binding);
                                        }
                                    }
                                }
                            }
                            parsed
                        } else {
                            self.node(t, n)
                        }
                        .map_err(|e| format!("{owner}.{alias}: {e}"))?;
                        if key == "preventable" && k.as_str() == "unpreventable" {
                            json!(!v.as_bool().ok_or("unpreventable must be Boolean")?)
                        } else {
                            v
                        }
                    } else {
                        defaults::field(owner, key, t)
                            .ok_or_else(|| format!("missing {owner}.{alias}"))?
                    };
                    out.insert(key.clone(), v);
                }
                if let Some((key, _)) = values.iter().find(|(k, _)| !consumed.contains(k)) {
                    return Err(format!("unknown {owner} field {key}"));
                }
                if self.branch_scopes
                    && matches!(owner, "Instruction::If" | "Instruction::ChooseOne")
                {
                    self.receipt_groups.push(branch_receipts);
                }
                Ok(Value::Object(out))
            }
        }
    }
    fn target(&mut self, n: &Node) -> Result<Value, String> {
        let Node::Record(name, fields) = n else {
            return Err("expected Target { name, select }".into());
        };
        if name.as_deref().is_some_and(|s| s != "Target") {
            return Err("expected Target record".into());
        }
        let get = |key: &str| fields.iter().find(|(k, _)| k == key).map(|(_, n)| n);
        let slot = self.declare("TargetSlot", get("name").ok_or("target needs a name")?)?;
        let select = get("select").ok_or("target needs a selector")?;
        let preset = constructor(select).and_then(|name| {
            presets()
                .into_iter()
                .find(|(n, _)| *n == name)
                .map(|(_, s)| s)
        });
        let selector_value = if let Some(p) = preset {
            if !matches!(select, Node::Name(_)) {
                return Err("target preset takes no operands".into());
            }
            serde_json::to_value(p).map_err(|e| e.to_string())?
        } else {
            self.node(&named("TargetSelector"), select)?
        };
        let selector: TargetSelector =
            serde_json::from_value(document::untag(&selector_value)).map_err(|e| e.to_string())?;
        let spec = selector.into_spec(TargetSlot(
            document::id_number(&slot)
                .ok_or("expected target ID")?
                .try_into()
                .map_err(|_| "target ID exceeds u16")?,
        ));
        self.targets.insert(spec.slot.0, spec.kind.clone());
        let mut value = serde_json::to_value(spec).map_err(|e| e.to_string())?;
        value["slot"] = slot;
        match selector_value["kind"].as_str() {
            Some("Objects") => value["filter"] = selector_value["args"]["filter"].clone(),
            Some("Players") => value["players"] = selector_value["args"]["filter"].clone(),
            Some("Damageable") => {
                value["filter"] = selector_value["args"]["objects"].clone();
                value["players"] = selector_value["args"]["players"].clone();
            }
            _ => (),
        }
        let TypeDef::Struct(Fields::Named(fs)) = &schema().types["TargetSpec"] else {
            unreachable!()
        };
        for (key, n) in fields {
            if key == "name" || key == "select" {
                continue;
            }
            if ["slot", "kind", "filter", "players"].contains(&key.as_str()) {
                return Err(format!("use name/select for target {key}"));
            }
            let (_, t) = fs
                .iter()
                .find(|(k, _)| k == key)
                .ok_or_else(|| format!("unknown Target field {key}"))?;
            value[key] = self.node(t, n)?;
        }
        Ok(value)
    }
}
fn constructor(n: &Node) -> Option<&str> {
    match n {
        Node::Name(n) | Node::Call(n, _) => Some(n),
        Node::Record(Some(n), _) => Some(n),
        Node::Scalar(Value::String(n)) => Some(n),
        _ => None,
    }
}
fn raw_value(n: &Node) -> Result<Value, String> {
    Ok(match n {
        Node::Scalar(v) => v.clone(),
        Node::Name(n) => json!(n),
        Node::List(ns) => Value::Array(ns.iter().map(raw_value).collect::<Result<_, _>>()?),
        Node::Record(None, fs) => Value::Object(
            fs.iter()
                .map(|(k, n)| Ok((k.clone(), raw_value(n)?)))
                .collect::<Result<_, String>>()?,
        ),
        _ => return Err("expected scalar data".into()),
    })
}
pub(super) fn read(n: &Node) -> Result<Value, String> {
    let (value, _) = read_with_bindings(n)?;
    Ok(document::untag(&value))
}
pub(super) fn read_with_bindings(n: &Node) -> Result<(Value, Vec<BindingName>), String> {
    read_bindings(n, true)
}
pub(super) fn read_legacy_bindings(n: &Node) -> Result<(Value, Vec<BindingName>), String> {
    read_bindings(n, false)
}
fn read_bindings(n: &Node, branch_scopes: bool) -> Result<(Value, Vec<BindingName>), String> {
    let mut reader = Reader {
        targets: HashMap::new(),
        ids: HashMap::new(),
        used: HashMap::new(),
        anon: u32::MAX,
        nodes: 0,
        symbols: HashMap::new(),
        bindings: Vec::new(),
        scope: 0,
        next_scope: 0,
        binding_namespace: "binding",
        branch_scopes,
        definitions: HashMap::new(),
        receipt_groups: Vec::new(),
        symbolic_results: HashSet::new(),
        receipt_work: Cell::new(0),
        scope_names: HashMap::new(),
        scope_counts: HashMap::new(),
        available_results: HashMap::new(),
        operation_depth: 0,
        payment_depth: None,
        numeric_bindings: HashMap::new(),
        declaring: false,
    };
    reader.reserve(n);
    let value = if matches!(n,Node::Record(Some(name),_) if name=="Card") {
        let face = reader.node(&named("Card"), n)?;
        json!({"schema_version":SCHEMA_VERSION,"layout":"Card","data":[],"faces":[face]})
    } else {
        reader.node(&named("UnifiedCard"), n)?
    };
    Ok((value, reader.bindings))
}
