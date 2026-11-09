//! Explicit entry/copy/characteristic upgrades for immutable historical captures.
#![allow(dead_code)]
use mtgish_ast::{copying, ir::Operand, movement};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
struct OldShape {
    power_toughness: Operand,
    creature_type: Option<Operand>,
    artifact: bool,
}
impl OldShape {
    fn current(self) -> mtgish_ast::ir::FaceDownAs {
        movement::face_down_from_source(
            &self.power_toughness,
            self.creature_type.as_ref(),
            self.artifact,
        )
        .unwrap()
    }
}

const OPERATIONS: &[&str] = &[
    "Conjure",
    "MoveTopCard",
    "Manifest",
    "Meld",
    "CounterWith",
    "Populate",
];

fn copy_json(fields: &mut serde_json::Map<String, Value>, old: &str, single: bool) {
    if fields.contains_key("exceptions") {
        return;
    }
    let Some(value) = fields.remove(old) else {
        return;
    };
    let values = if single { json!([value]) } else { value };
    let operands: Vec<Operand> = serde_json::from_value(values).unwrap();
    fields.insert(
        "exceptions".into(),
        serde_json::to_value(copying::from_source(&operands).unwrap()).unwrap(),
    );
}
pub fn json(value: &mut Value) {
    // Quoted abilities can themselves contain old arrivals/copies. Upgrade
    // children before a containing Operand is deserialized into today's types.
    match value {
        Value::Object(fields) => fields.values_mut().for_each(json),
        Value::Array(items) => items.iter_mut().for_each(json),
        _ => (),
    }
    if value
        .get("kind")
        .and_then(Value::as_str)
        .is_some_and(|k| OPERATIONS.contains(&k))
    {
        if let Some(entry) = value.get_mut("args").and_then(|a| a.get_mut("entry")) {
            if entry.is_array() {
                let flags: Vec<Operand> = serde_json::from_value(entry.clone()).unwrap();
                *entry = serde_json::to_value(movement::entry_from_source(&flags)).unwrap();
            }
        }
    }
    let kind = value.get("kind").and_then(Value::as_str).map(str::to_owned);
    if matches!(kind.as_deref(), Some("CopyOnStack" | "CopyOf")) {
        if let Some(fields) = value.get_mut("args").and_then(Value::as_object_mut) {
            copy_json(fields, "effects", kind.as_deref() == Some("CopyOf"));
        }
    }
    for variant in ["Copy", "CopyEach", "CopyOne", "CopyNamed", "CopyOf"] {
        if let Some(fields) = value.get_mut(variant).and_then(Value::as_object_mut) {
            copy_json(
                fields,
                if variant == "CopyOf" {
                    "effects"
                } else {
                    "adjustments"
                },
                variant == "CopyOf",
            );
        }
    }
    if value.get("power_toughness").is_some()
        && value.get("creature_type").is_some()
        && value.get("artifact").is_some()
    {
        let old: OldShape = serde_json::from_value(value.clone()).unwrap();
        *value = serde_json::to_value(old.current()).unwrap();
    }
    if let Some(fields) = value.as_object_mut() {
        if fields.contains_key("tapped")
            && fields.contains_key("counters")
            && fields.contains_key("states")
        {
            fields.entry("controller").or_insert(json!(null));
        }
    }
}

#[derive(Clone, Copy)]
enum Upgrade {
    Entry,
    Copy(bool),
    Shape,
}

fn owner(input: &str, open: usize) -> Option<&str> {
    let after = input[open + 1..].trim_start();
    if let Some(after) = after.strip_prefix("kind:") {
        return after
            .split_once(',')
            .map(|(name, _)| name.trim().trim_matches('"'));
    }
    let before = &input[..open];
    let start = before
        .rfind(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .map_or(0, |i| i + 1);
    (start < before.len()).then_some(&before[start..])
}

/// RON's deserializer consumes each old payload with its actual type. Process
/// the deepest eligible field first, including old fields in quoted abilities;
/// draft ChangeZone's intentionally generic entry vector remains untouched.
pub fn ron<T: serde::de::DeserializeOwned>(input: &str) -> T {
    let mut input = input.to_string();
    for _ in 0..65_536 {
        let bytes = input.as_bytes();
        let (mut i, mut quoted, mut escaped) = (0, false, false);
        let mut stack = Vec::new();
        let mut deepest = None;
        while i < bytes.len() {
            let c = bytes[i];
            if quoted {
                if escaped {
                    escaped = false;
                } else if c == b'\\' {
                    escaped = true;
                } else if c == b'"' {
                    quoted = false;
                }
                i += 1;
                continue;
            }
            if c == b'"' {
                quoted = true;
                i += 1;
                continue;
            }
            if c >= 128 {
                i += input[i..].chars().next().unwrap().len_utf8();
                continue;
            }
            let field = ["entry:", "effects:", "adjustments:", "power_toughness:"]
                .into_iter()
                .find(|f| input[i..].starts_with(f));
            if let Some(field) = field {
                let value_start = i + field.len();
                let context = stack.iter().rev().find_map(|&open| owner(&input, open));
                let upgrade = match (field, context) {
                    ("entry:", Some(name))
                        if OPERATIONS.contains(&name)
                            && input[value_start..].trim_start().starts_with('[') =>
                    {
                        Some(Upgrade::Entry)
                    }
                    ("effects:", Some("CopyOnStack")) => Some(Upgrade::Copy(false)),
                    ("effects:", Some("CopyOf")) => Some(Upgrade::Copy(true)),
                    ("adjustments:", Some("Copy" | "CopyEach" | "CopyOne" | "CopyNamed")) => {
                        Some(Upgrade::Copy(false))
                    }
                    ("power_toughness:", _)
                        if stack.last().is_some_and(|&open| {
                            input[open + 1..]
                                .trim_start()
                                .starts_with("power_toughness:")
                                && input[value_start..].trim_start().starts_with("(kind:")
                        }) =>
                    {
                        Some(Upgrade::Shape)
                    }
                    _ => None,
                };
                if let Some(upgrade) = upgrade {
                    let (start, value_start) = if matches!(upgrade, Upgrade::Shape) {
                        let open = *stack.last().unwrap();
                        (open, open)
                    } else {
                        (i, value_start)
                    };
                    if deepest
                        .as_ref()
                        .is_none_or(|(depth, _, _, _)| stack.len() >= *depth)
                    {
                        deepest = Some((stack.len(), start, value_start, upgrade));
                    }
                }
            }
            match c {
                b'(' | b'[' | b'{' => stack.push(i),
                b')' | b']' | b'}' => {
                    stack.pop().unwrap();
                }
                _ => (),
            }
            i += 1;
        }
        let Some((_, start, value_start, upgrade)) = deepest else {
            return ron::from_str(&input).unwrap();
        };
        let tail = &input[value_start..];
        let mut de = ron::de::Deserializer::from_str(tail).unwrap();
        let (name, replacement) = match upgrade {
            Upgrade::Shape => (
                "",
                ron::to_string(&OldShape::deserialize(&mut de).unwrap().current()).unwrap(),
            ),
            Upgrade::Entry => {
                let flags = Vec::<Operand>::deserialize(&mut de).unwrap();
                (
                    "entry:",
                    ron::to_string(&movement::entry_from_source(&flags)).unwrap(),
                )
            }
            Upgrade::Copy(single) => {
                let operands = if single {
                    vec![Operand::deserialize(&mut de).unwrap()]
                } else {
                    Vec::<Operand>::deserialize(&mut de).unwrap()
                };
                (
                    "exceptions:",
                    ron::to_string(&copying::from_source(&operands).unwrap()).unwrap(),
                )
            }
        };
        let end = value_start + tail.len() - de.remainder().len();
        input = format!("{}{name}{replacement}{}", &input[..start], &input[end..]);
    }
    panic!("historical payload upgrade limit exceeded");
}
