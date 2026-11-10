//! Coin procedures, scoped receipt evidence and repetition views. Protocol
//! inference belongs only to Source/legacy boundaries, never to generation or
//! loading an already explicit current card.
use crate::{
    human::document,
    ir::*,
    ir_schema::{Fields, Schema, Ty, TypeDef},
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

pub(crate) fn positive(protocol: CoinProtocol, outcome: &CoinOutcome) -> Option<bool> {
    match (protocol, outcome) {
        (CoinProtocol::Called, CoinOutcome::Won) | (CoinProtocol::Uncalled, CoinOutcome::Heads) => {
            Some(true)
        }
        (CoinProtocol::Called, CoinOutcome::Lost)
        | (CoinProtocol::Uncalled, CoinOutcome::Tails) => Some(false),
        _ => None,
    }
}
pub(crate) fn call(protocol: CoinProtocol) -> &'static str {
    match protocol {
        CoinProtocol::Called => "flip_coin",
        CoinProtocol::Uncalled => "flip_coin_for_heads",
    }
}
#[derive(Clone)]
pub(crate) struct Series<'a> {
    pub times: Option<&'a Expr>,
    pub each: Option<&'a ObjectSet>,
    pub stop: Option<CoinStop>,
}
pub(crate) fn series(repetition: &CoinRepetition) -> Series<'_> {
    let (times, each, stop) = match repetition {
        CoinRepetition::Count(n) => (Some(n), None, None),
        CoinRepetition::EachObject(objects) => (None, Some(objects), None),
        CoinRepetition::UpTo {
            extent: CoinExtent::Count(n),
            stop,
        } => (Some(n), None, Some(*stop)),
        CoinRepetition::UpTo {
            extent: CoinExtent::Objects(objects),
            stop,
        } => (None, Some(objects), Some(*stop)),
        CoinRepetition::Until(stop) => (None, None, Some(*stop)),
    };
    Series { times, each, stop }
}
fn schema() -> &'static Schema {
    static S: OnceLock<Schema> = OnceLock::new();
    S.get_or_init(|| Schema::of_ir().expect("coin schema"))
}
fn ptr(path: &str, key: &str) -> String {
    format!("{path}/{}", key.replace('~', "~0").replace('/', "~1"))
}
fn variant<'a>(v: &'a Value, adjacent: bool) -> Option<&'a str> {
    if adjacent {
        v["kind"].as_str()
    } else {
        v.as_str()
            .or_else(|| v.as_object()?.keys().next().map(String::as_str))
    }
}
/// Typed child enumeration: text, draft-form labels and opaque data are never
/// mistaken for semantic operations or observations by matching their spelling.
fn children(ty: &Ty, value: &Value, path: &str) -> Vec<(Ty, Value, String)> {
    fn fields(f: &Fields, v: &Value, p: &str, out: &mut Vec<(Ty, Value, String)>) {
        match f {
            Fields::Unit => {}
            Fields::Named(fs) => {
                for (k, t) in fs {
                    out.push((t.clone(), v[k].clone(), ptr(p, k)));
                }
            }
            Fields::Tuple(ts) => {
                for (i, t) in ts.iter().enumerate() {
                    out.push((
                        t.clone(),
                        if ts.len() == 1 {
                            v.clone()
                        } else {
                            v[i].clone()
                        },
                        if ts.len() == 1 {
                            p.into()
                        } else {
                            ptr(p, &i.to_string())
                        },
                    ));
                }
            }
        }
    }
    let mut out = vec![];
    match ty {
        Ty::Option(t) if !value.is_null() => out.push(((**t).clone(), value.clone(), path.into())),
        Ty::Vec(t) => {
            for (i, v) in value.as_array().into_iter().flatten().enumerate() {
                out.push(((**t).clone(), v.clone(), ptr(path, &i.to_string())));
            }
        }
        Ty::Tuple(ts) => {
            for (i, t) in ts.iter().enumerate() {
                out.push((t.clone(), value[i].clone(), ptr(path, &i.to_string())));
            }
        }
        Ty::Named(name) => match schema().types.get(name) {
            Some(TypeDef::Struct(f)) => fields(f, value, path, &mut out),
            Some(TypeDef::Enum { adjacent, variants }) => {
                if let Some(n) = variant(value, *adjacent) {
                    if let Some((_, f)) = variants.iter().find(|(n2, _)| n2 == n) {
                        fields(
                            f,
                            &value[if *adjacent { "args" } else { n }],
                            &ptr(path, if *adjacent { "args" } else { n }),
                            &mut out,
                        );
                    }
                }
            }
            _ => {}
        },
        _ => {}
    };
    out
}
#[derive(Clone)]
struct Producer {
    protocol: CoinProtocol,
    locked: bool,
    faces: bool,
    winner: bool,
}
#[derive(Clone, Default)]
struct History {
    last: BTreeSet<String>,
    players: BTreeSet<String>,
    objects: BTreeSet<String>,
}
impl History {
    fn merge(&mut self, other: Self) {
        self.last.extend(other.last);
        self.players.extend(other.players);
        self.objects.extend(other.objects);
    }
}
struct Analysis {
    definitions: BTreeMap<usize, Vec<String>>,
    uses: BTreeMap<String, usize>,
    producers: BTreeMap<String, Producer>,
    source: bool,
    forced: BTreeSet<String>,
}
impl Analysis {
    fn collect(&mut self, ty: &Ty, v: &Value, p: &str, binding: Option<usize>) {
        let effect = matches!(ty,Ty::Named(n) if n=="Instruction") && v["kind"] == "Effect";
        let binding = if effect {
            self.uses.get(&format!("{p}/args/result")).copied()
        } else {
            binding
        };
        if matches!(ty,Ty::Named(n) if n=="Operation")
            && matches!(
                v["kind"].as_str(),
                Some("FlipCoin" | "FlipCoins" | "FlipCoinSeries")
            )
        {
            let protocol: CoinProtocol =
                serde_json::from_value(v["args"]["protocol"].clone()).expect("typed protocol");
            let series = v["kind"] == "FlipCoinSeries";
            let forced_source =
                self.source && series && v["args"]["repetition"].get("EachObject").is_none();
            self.producers.insert(
                p.into(),
                Producer {
                    protocol,
                    locked: self.forced.contains(p)
                        || forced_source
                        || self.source && protocol == CoinProtocol::Uncalled,
                    faces: false,
                    winner: false,
                },
            );
            if let Some(b) = binding {
                self.definitions.entry(b).or_default().push(p.into());
            }
        }
        for (t, v, p) in children(ty, v, p) {
            self.collect(&t, &v, &p, binding);
        }
    }
    fn constrain(&mut self, paths: &BTreeSet<String>, outcome: &CoinOutcome) {
        for p in paths {
            if let Some(d) = self.producers.get_mut(p) {
                match outcome {
                    CoinOutcome::Won | CoinOutcome::Lost => d.winner = true,
                    _ => d.faces = true,
                }
            }
        }
    }
    fn receipt(&mut self, path: &str, outcome: &CoinOutcome) {
        if let Some(b) = self.uses.get(path) {
            let paths = self
                .definitions
                .get(b)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect();
            self.constrain(&paths, outcome);
        }
    }
    fn node(&mut self, ty: &Ty, v: &Value, p: &str, h: &mut History) {
        let Ty::Named(name) = ty else {
            for (t, v, p) in children(ty, v, p) {
                self.node(&t, &v, &p, &mut h.clone());
            }
            return;
        };
        if name == "Ability" || name == "Mode" {
            for (t, v, p) in children(ty, v, p) {
                self.node(&t, &v, &p, &mut History::default());
            }
            return;
        }
        if name == "Program" {
            for (i, v) in v["instructions"]
                .as_array()
                .into_iter()
                .flatten()
                .enumerate()
            {
                self.node(
                    &Ty::Named("Instruction".into()),
                    v,
                    &format!("{p}/instructions/{i}"),
                    h,
                );
            }
            return;
        }
        if name == "Instruction" {
            let a = &v["args"];
            let q = format!("{p}/args");
            match v["kind"].as_str() {
                Some("Effect") => {
                    self.node(
                        &Ty::Named("Operation".into()),
                        &a["operation"],
                        &format!("{q}/operation"),
                        h,
                    );
                    return;
                }
                Some("Group") => {
                    self.node(&Ty::Named("Program".into()), a, &q, h);
                    return;
                }
                Some("If") => {
                    self.node(
                        &Ty::Named("Condition".into()),
                        &a["condition"],
                        &format!("{q}/condition"),
                        &mut h.clone(),
                    );
                    let mut left = h.clone();
                    let mut right = h.clone();
                    self.node(
                        &Ty::Named("Program".into()),
                        &a["then"],
                        &format!("{q}/then"),
                        &mut left,
                    );
                    self.node(
                        &Ty::Named("Program".into()),
                        &a["otherwise"],
                        &format!("{q}/otherwise"),
                        &mut right,
                    );
                    left.merge(right);
                    *h = left;
                    return;
                }
                Some("ChooseOne") => {
                    let mut all = History::default();
                    for (i, body) in a["options"].as_array().into_iter().flatten().enumerate() {
                        let mut child = h.clone();
                        self.node(
                            &Ty::Named("Program".into()),
                            body,
                            &format!("{q}/options/{i}"),
                            &mut child,
                        );
                        all.merge(child);
                    }
                    *h = all;
                    return;
                }
                Some("ForEachPlayer" | "ForEachObject" | "Optional" | "Times") => {
                    // Non-body inputs read the incoming history, never a future flip.
                    for (t, v, p2) in children(ty, v, p) {
                        if p2 != format!("{q}/body") {
                            self.node(&t, &v, &p2, &mut h.clone());
                        }
                    }
                    let mut child = h.clone();
                    self.node(
                        &Ty::Named("Program".into()),
                        &a["body"],
                        &format!("{q}/body"),
                        &mut child,
                    );
                    if v["kind"] == "ForEachPlayer" {
                        let mut body = &a["body"];
                        let mut at = format!("{q}/body");
                        loop {
                            let Some(ins) = body["instructions"]
                                .as_array()
                                .filter(|x| x.len() == 1)
                                .map(|x| &x[0])
                            else {
                                break;
                            };
                            if ins["kind"] == "Group" {
                                body = &ins["args"];
                                at.push_str("/instructions/0/args");
                                continue;
                            }
                            if ins["kind"] == "Effect"
                                && ins["args"]["operation"]["kind"] == "FlipCoin"
                            {
                                let batch =
                                    BTreeSet::from([format!("{at}/instructions/0/args/operation")]);
                                // This loop defines a new batch even when its
                                // player set is empty. An earlier batch is not
                                // an alternative outcome of this instruction.
                                child.players = batch.clone();
                                child.last = batch;
                                *h = child;
                                return;
                            }
                            break;
                        }
                    }
                    h.merge(child);
                    return;
                }
                Some("Repeatable") => {
                    self.node(&Ty::Named("Program".into()), a, &q, h);
                    return;
                }
                _ => {}
            }
        }
        if name == "Operation" {
            if self.producers.contains_key(p) {
                for (t, v, p2) in children(ty, v, p) {
                    self.node(&t, &v, &p2, &mut h.clone());
                }
                h.last = BTreeSet::from([p.into()]);
                if v["kind"] == "FlipCoinSeries"
                    && v["args"]["repetition"].get("EachObject").is_some()
                {
                    h.objects = h.last.clone();
                }
                if v["kind"] == "FlipCoinSeries" {
                    let series: CoinRepetition =
                        serde_json::from_value(v["args"]["repetition"].clone()).unwrap();
                    if matches!(
                        crate::coin_policies::series(&series).stop,
                        Some(CoinStop::Lose | CoinStop::LoseOrPlayerStops)
                    ) {
                        self.constrain(&h.last, &CoinOutcome::Won);
                    }
                }
                return;
            }
            if v["kind"] == "WithSelections" {
                for (t, v, p2) in children(ty, v, p) {
                    if p2 == format!("{p}/args/action") {
                        self.node(&t, &v, &p2, h);
                    } else {
                        self.node(&t, &v, &p2, &mut h.clone());
                    }
                }
                return;
            }
            if v["kind"] == "InOrder" {
                for (t, v, p2) in children(ty, v, p) {
                    if let Ty::Vec(inner) = t {
                        for (i, v) in v.as_array().into_iter().flatten().enumerate() {
                            self.node(&inner, v, &ptr(&p2, &i.to_string()), h);
                        }
                    }
                }
                return;
            }
            if v["kind"] == "Together" {
                let mut all = h.clone();
                for (t, v, p2) in children(ty, v, p) {
                    if let Ty::Vec(inner) = t {
                        for (i, v) in v.as_array().into_iter().flatten().enumerate() {
                            let mut child = h.clone();
                            self.node(&inner, v, &ptr(&p2, &i.to_string()), &mut child);
                            all.merge(child);
                        }
                    }
                }
                *h = all;
                return;
            }
        }
        if name == "Expr" && v["kind"] == "CountCoinOutcomes" {
            let outcome: CoinOutcome =
                serde_json::from_value(v["args"]["outcome"].clone()).unwrap();
            if v["args"]["source"] == "Latest" {
                self.constrain(&h.last, &outcome);
            } else {
                self.receipt(&format!("{p}/args/source/Receipt"), &outcome);
            }
        }
        if name == "Condition" && v["kind"] == "CoinFlip" {
            let o = serde_json::from_value(v["args"]["outcome"].clone()).unwrap();
            self.receipt(&format!("{p}/args/result"), &o);
        }
        if name == "Condition" && v["kind"] == "CoinCameUp" {
            let o = serde_json::from_value(v["args"]["outcome"].clone()).unwrap();
            self.constrain(
                if v["args"]["player"].is_null() {
                    &h.objects
                } else {
                    &h.players
                },
                &o,
            );
        }
        for (t, v, p) in children(ty, v, p) {
            let mut private = if name == "Operand"
                && v.is_object()
                && matches!(&t, Ty::Named(n) if n == "Program")
            {
                History::default()
            } else {
                h.clone()
            };
            self.node(&t, &v, &p, &mut private);
        }
    }
}
/// Infer only at an implicit input boundary. Current authoring fields are
/// authoritative and never passed here. Explicit historical calls stay called.
pub(crate) fn infer(
    card: &mut UnifiedCard,
    source: bool,
    forced: BTreeSet<String>,
) -> Result<(), String> {
    if !has_coins(card) {
        return Ok(());
    }
    let doc = document::from_card(card)?;
    let mut value = serde_json::to_value(&*card).map_err(|e| e.to_string())?;
    let mut a = Analysis {
        definitions: BTreeMap::new(),
        uses: doc
            .uses()
            .iter()
            .map(|u| (u.path.clone(), u.binding))
            .collect(),
        producers: BTreeMap::new(),
        source,
        forced,
    };
    let ty = Ty::Named("UnifiedCard".into());
    a.collect(&ty, &value, "", None);
    a.node(&ty, &value, "", &mut History::default());
    for (path, d) in a.producers {
        let protocol = if d.locked {
            d.protocol
        } else if d.faces && !d.winner {
            CoinProtocol::Uncalled
        } else {
            CoinProtocol::Called
        };
        value
            .pointer_mut(&format!("{path}/args/protocol"))
            .ok_or("coin protocol path disappeared")?
            .clone_from(&serde_json::to_value(protocol).unwrap());
    }
    *card = serde_json::from_value(value).map_err(|e| e.to_string())?;
    Ok(())
}
fn has_coins<T: crate::visit::Walk>(value: &T) -> bool {
    use crate::visit::Visitor;
    struct Find(bool);
    impl Visitor for Find {
        fn operation(&mut self, op: &Operation) -> bool {
            if matches!(
                op,
                Operation::FlipCoin { .. }
                    | Operation::FlipCoins { .. }
                    | Operation::FlipCoinSeries { .. }
            ) {
                self.0 = true;
            }
            !self.0
        }
    }
    let mut find = Find(false);
    value.walk(&mut find);
    find.0
}
#[cfg(feature = "legacy")]
pub(crate) fn source_card(card: &mut Card) -> Result<(), String> {
    if !has_coins(card) {
        return Ok(());
    }
    let mut wrapped = UnifiedCard {
        schema_version: SCHEMA_VERSION,
        layout: Layout::Card,
        faces: vec![card.clone()],
        data: vec![],
    };
    infer(&mut wrapped, true, BTreeSet::new())?;
    *card = wrapped.faces.remove(0);
    Ok(())
}
#[cfg(feature = "legacy")]
pub(crate) fn source_unified(card: &mut UnifiedCard) -> Result<(), String> {
    infer(card, true, BTreeSet::new())
}

#[cfg(feature = "legacy")]
pub(crate) fn source_operand(operand: &mut Operand) -> Result<(), String> {
    if !has_coins(operand) {
        return Ok(());
    }
    // Metadata-only operands still have private program/ability scopes. Use
    // the same typed document resolver instead of a separate ID heuristic.
    let mut wrapped = UnifiedCard {
        schema_version: SCHEMA_VERSION,
        layout: Layout::Extension {
            namespace: "org.mtgish.source".into(),
            name: "Operand".into(),
        },
        faces: vec![],
        data: vec![Field {
            name: "Value".into(),
            value: operand.clone(),
        }],
    };
    infer(&mut wrapped, true, BTreeSet::new())?;
    *operand = wrapped.data.remove(0).value;
    Ok(())
}
