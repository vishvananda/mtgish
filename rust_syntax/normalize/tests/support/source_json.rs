//! The checked-in legacy tagged Source JSON must describe the same complete
//! records as parser RON, even though default Rust serde uses external tags.
use mtgish_ast::{corpus, mtg_types::OracleCard};
use serde_json::Value;
use std::{collections::BTreeMap, io::BufRead};
fn untag(value: Value) -> Value {
    untag_value(value, true)
}
fn untag_value(value: Value, root: bool) -> Value {
    match value {
        Value::Array(values) => {
            Value::Array(values.into_iter().map(|v| untag_value(v, false)).collect())
        }
        Value::Object(mut fields) => {
            if let Some(tag) = fields.keys().find(|key| key.starts_with('_')).cloned() {
                let variant = fields.remove(&tag).unwrap().as_str().unwrap().to_owned();
                // Nested card faces are Source structs, despite legacy JSON
                // adding the same tag used for the root OracleCard enum.
                if tag == "_OracleCard" && !root {
                    return untag_value(Value::Object(fields), false);
                }
                if fields.is_empty() {
                    return Value::String(variant);
                }
                let payload = if fields.len() == 1 && fields.contains_key("args") {
                    untag_value(fields.remove("args").unwrap(), false)
                } else {
                    untag_value(Value::Object(fields), false)
                };
                Value::Object([(variant, payload)].into_iter().collect())
            } else {
                Value::Object(
                    fields
                        .into_iter()
                        .map(|(key, value)| (key, untag_value(value, false)))
                        .collect(),
                )
            }
        }
        other => other,
    }
}
pub fn assert_fixture_matches(text: &str) {
    let mut source = BTreeMap::new();
    let mut unnamed = BTreeMap::<String, Vec<Value>>::new();
    for line in text.lines() {
        let card: OracleCard = ron::from_str(line).unwrap();
        let identity = corpus::Identity::of(&card).unwrap();
        let value = serde_json::to_value(card).unwrap();
        if let Some(name) = identity.names.into_iter().next() {
            source.insert(name, value);
        } else {
            // Sticker sheets have an identity but no card name. Compare their
            // whole enum value instead of assuming every Source is a card face.
            let variant = value.as_object().unwrap().keys().next().unwrap().clone();
            unnamed.entry(variant).or_default().push(value);
        }
    }
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/mtgish.lines.json");
    let mut checked = 0;
    for line in std::io::BufReader::new(std::fs::File::open(path).unwrap()).lines() {
        let line = line.unwrap();
        let named = source
            .keys()
            .any(|name| line.contains(&format!("\"Name\":{name:?}")));
        if !named
            && !unnamed
                .keys()
                .any(|kind| line.contains(&format!("\"_OracleCard\":{kind:?}")))
        {
            continue;
        }
        let tagged: Value = serde_json::from_str(&line).unwrap();
        let value = untag(tagged);
        let card: OracleCard = serde_json::from_value(value.clone()).unwrap();
        let Some(name) = corpus::Identity::of(&card)
            .unwrap()
            .names
            .into_iter()
            .next()
        else {
            let kind = value.as_object().unwrap().keys().next().unwrap();
            if let Some(expected) = unnamed.get_mut(kind) {
                if let Some(index) = expected.iter().position(|v| v == &value) {
                    expected.remove(index);
                }
            }
            continue;
        };
        assert_eq!(value, source[&name], "{name}");
        checked += 1;
    }
    assert_eq!(checked, source.len());
    assert!(
        unnamed.values().all(Vec::is_empty),
        "unmatched unnamed Source records: {unnamed:?}"
    );
}
