use super::{
    defaults, names, schema,
    syntax::{self, Node},
};
use crate::{
    ir::*,
    ir_schema::{Fields, Ty, TypeDef},
    targets::TargetSelector,
};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
fn named(n: &str) -> Ty {
    Ty::Named(n.into())
}
fn symbol(prefix: &str, n: u64) -> Node {
    Node::Name(format!("{prefix}{n}"))
}
fn ident(s: &str) -> bool {
    !s.is_empty()
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        && !s.as_bytes()[0].is_ascii_digit()
        && !matches!(s, "true" | "false" | "None")
}
fn literal(s: &str) -> Node {
    if ident(s) {
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
            if let Some(id) = v.as_u64() {
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
struct Writer {
    reads: HashSet<u32>,
}
impl Writer {
    fn node(&self, t: &Ty, v: &Value) -> Result<Node, String> {
        if t == &named("Ability") {
            let mut reads = HashSet::new();
            references(t, v, &mut reads, true);
            return Writer { reads }.inner(t, v);
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
                if let Some(prefix) = match n.as_str() {
                    "ResultId" => Some("r"),
                    "Slot" => Some("t"),
                    "Binding" => Some("b"),
                    "SelectionId" => Some("s"),
                    _ => None,
                } {
                    return Ok(symbol(prefix, v.as_u64().ok_or("expected ID")?));
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
                    let id = v["args"]["result"].as_u64().ok_or("missing result ID")?;
                    return Ok(if self.reads.contains(&(id as u32)) {
                        Node::Bind(format!("r{id}"), Box::new(op))
                    } else {
                        op
                    });
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
                            vec![self.node(&named("Slot"), slot)?],
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
                    let node = if key == "preventable" {
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
        let spec: TargetSpec = serde_json::from_value(v.clone()).map_err(|e| e.to_string())?;
        let selector = serde_json::to_value(spec.selector()?).map_err(|e| e.to_string())?;
        let select = selector_preset(&selector)
            .map(|s| Node::Name(s.into()))
            .map(Ok)
            .unwrap_or_else(|| self.node(&named("TargetSelector"), &selector))?;
        let mut fields = vec![
            ("name".into(), self.node(&named("Slot"), &v["slot"])?),
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
    let mut reads = HashSet::new();
    references(&named("UnifiedCard"), v, &mut reads, true);
    let w = Writer { reads };
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
}
impl Reader {
    fn reserve(&mut self, n: &Node) {
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
            Node::Record(_, fs) => {
                for (_, n) in fs {
                    self.reserve(n)
                }
            }
            _ => (),
        }
    }
    fn id(&mut self, family: &str, n: &Node) -> Result<Value, String> {
        let prefix = match family {
            "ResultId" => "r",
            "Slot" => "t",
            "Binding" => "b",
            _ => "s",
        };
        let name = match n {
            Node::Name(s) => s.clone(),
            Node::Scalar(Value::String(s)) => s.clone(),
            Node::Scalar(Value::Number(n)) => {
                let id = n.as_u64().ok_or("expected an unsigned binding ID")?;
                self.used.entry(prefix.into()).or_default().insert(id);
                return Ok(json!(id));
            }
            _ => return Err("expected a binding name".into()),
        };
        if let Some(n) = name
            .strip_prefix(prefix)
            .and_then(|s| s.parse::<u64>().ok())
        {
            self.used.entry(prefix.into()).or_default().insert(n);
            return Ok(json!(n));
        }
        let key = (prefix.into(), name);
        if let Some(id) = self.ids.get(&key) {
            return Ok(json!(id));
        }
        let used = self.used.entry(prefix.into()).or_default();
        let id = (0..=u32::MAX as u64)
            .find(|n| !used.contains(n))
            .ok_or("binding IDs exhausted")?;
        used.insert(id);
        self.ids.insert(key, id);
        Ok(json!(id))
    }
    fn anonymous(&mut self) -> Result<Value, String> {
        let used = self.used.entry("r".into()).or_default();
        while used.contains(&(self.anon as u64)) {
            self.anon = self.anon.checked_sub(1).ok_or("result IDs exhausted")?
        }
        let id = self.anon;
        used.insert(id as u64);
        Ok(json!(id))
    }
    fn node(&mut self, t: &Ty, n: &Node) -> Result<Value, String> {
        let independent = t == &named("Ability");
        let scope = independent
            || t == &named("Mode")
            || (t == &named("Operation")
                && matches!(constructor(n), Some("CreateTrigger" | "ReflexiveTrigger")));
        if scope {
            let prior_targets = self.targets.clone();
            let prior_ids = self.ids.clone();
            if independent {
                self.targets.clear();
                self.ids.clear();
            }
            let result = self.inner(t, n);
            self.targets = prior_targets;
            self.ids = prior_ids;
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
                if ["ResultId", "Slot", "Binding", "SelectionId"].contains(&name.as_str()) {
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
                            let slot = self.id("Slot", &args[0])?;
                            let id: u16 =
                                serde_json::from_value(slot.clone()).map_err(|e| e.to_string())?;
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
                        let result=if let Some(name)=binding{self.id("ResultId",&Node::Name(name.clone()))?}else{self.anonymous()?};
                        return Ok(json!({"kind":"Effect","args":{"result":result,"operation":self.node(&named("Operation"),op)?}}));
                    }
                    if binding.is_some() {
                        return Err("only an action can bind a result".into());
                    }
                }
                let Some(d) = schema().types.get(name) else {
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
                        let args =
                            self.fields(&format!("{name}::{variant}"), Some(raw), fields, n)?;
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
        match f {
            Fields::Unit => match n {
                Node::Name(_) | Node::Scalar(Value::String(_)) => Ok(Value::Null),
                _ => Err("unit constructor takes no operands".into()),
            },
            Fields::Tuple(ts) => {
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
                for (key, t) in fs {
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
                        .filter(|(k, _)| k == key || k == &alias)
                        .collect();
                    if candidates.len() > 1 {
                        return Err(format!("duplicate aliases for {owner}.{key}"));
                    }
                    let v = if let Some((k, n)) = candidates.first() {
                        consumed.insert(k.clone());
                        let v = self
                            .node(t, n)
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
        let slot = self.id("Slot", get("name").ok_or("target needs a name")?)?;
        let select = get("select").ok_or("target needs a selector")?;
        let preset = constructor(select).and_then(|name| {
            presets()
                .into_iter()
                .find(|(n, _)| *n == name)
                .map(|(_, s)| s)
        });
        let selector = if let Some(p) = preset {
            if !matches!(select, Node::Name(_)) {
                return Err("target preset takes no operands".into());
            }
            p
        } else {
            serde_json::from_value(self.node(&named("TargetSelector"), select)?)
                .map_err(|e| e.to_string())?
        };
        let spec = selector.into_spec(Slot(
            serde_json::from_value(slot).map_err(|e| e.to_string())?,
        ));
        self.targets.insert(spec.slot.0, spec.kind.clone());
        let mut value = serde_json::to_value(spec).map_err(|e| e.to_string())?;
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
    let mut reader = Reader {
        targets: HashMap::new(),
        ids: HashMap::new(),
        used: HashMap::new(),
        anon: u32::MAX,
        nodes: 0,
    };
    reader.reserve(n);
    if matches!(n,Node::Record(Some(name),_) if name=="Card") {
        let face = reader.node(&named("Card"), n)?;
        Ok(json!({"schema_version":SCHEMA_VERSION,"layout":"Card","data":[],"faces":[face]}))
    } else {
        reader.node(&named("UnifiedCard"), n)
    }
}
