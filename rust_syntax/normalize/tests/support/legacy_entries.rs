//! Declared schema-179 entry upgrade for immutable historical test captures.
#![allow(dead_code)]
use mtgish_ast::{
    ir::{EntrySpec, Operand},
    movement,
};
use serde::Deserialize;
use serde_json::{json, Value};
const OPERATIONS: &[&str] = &[
    "Conjure",
    "MoveTopCard",
    "Manifest",
    "Meld",
    "CounterWith",
    "Populate",
];

pub fn json(value: &mut Value) {
    if value
        .get("kind")
        .and_then(Value::as_str)
        .is_some_and(|k| OPERATIONS.contains(&k))
    {
        if let Some(entry) = value.get_mut("args").and_then(|args| args.get_mut("entry")) {
            if entry.is_array() {
                let flags: Vec<Operand> = serde_json::from_value(entry.clone()).unwrap();
                *entry = serde_json::to_value(movement::entry_from_source(&flags)).unwrap();
            }
        }
    }
    if let Some(fields) = value.as_object_mut() {
        if fields.contains_key("tapped")
            && fields.contains_key("counters")
            && fields.contains_key("states")
        {
            fields.entry("controller").or_insert(json!(null));
        }
    }
    match value {
        Value::Object(fields) => fields.values_mut().for_each(json),
        Value::Array(items) => items.iter_mut().for_each(json),
        _ => (),
    }
}

/// Upgrade only the six changed entry fields. RON's own deserializer consumes
/// each flag list; draft ChangeZone's intentionally generic list stays intact.
pub fn ron<T: serde::de::DeserializeOwned>(input: &str) -> T {
    let bytes = input.as_bytes();
    let (mut i, mut start, mut quoted, mut escaped) = (0, 0, false, false);
    let mut stack = Vec::new();
    let mut output = String::new();
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
        if input[i..].starts_with("entry:") {
            let value_start = i + "entry:".len();
            let tail = &input[value_start..];
            let operation = stack.iter().rev().find_map(|&open: &usize| {
                input[open..]
                    .strip_prefix("(kind:")?
                    .split_once(',')
                    .map(|(kind, _)| kind)
            });
            if tail.trim_start().starts_with('[')
                && operation.is_some_and(|kind| OPERATIONS.contains(&kind))
            {
                let mut de = ron::de::Deserializer::from_str(tail).unwrap();
                let flags = Vec::<Operand>::deserialize(&mut de).unwrap();
                let consumed = tail.len() - de.remainder().len();
                let entry: EntrySpec = movement::entry_from_source(&flags);
                output.push_str(&input[start..value_start]);
                output.push_str(&ron::to_string(&entry).unwrap());
                i = value_start + consumed;
                start = i;
                continue;
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
    output.push_str(&input[start..]);
    ron::from_str(&output).unwrap()
}
