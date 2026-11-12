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
    for line in text.lines() {
        let card: OracleCard = ron::from_str(line).unwrap();
        let name = corpus::Identity::of(&card).unwrap().names.remove(0);
        source.insert(name, serde_json::to_value(card).unwrap());
    }
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/mtgish.lines.json");
    let mut checked = 0;
    for line in std::io::BufReader::new(std::fs::File::open(path).unwrap()).lines() {
        let line = line.unwrap();
        if !source
            .keys()
            .any(|name| line.contains(&format!("\"Name\":{name:?}")))
        {
            continue;
        }
        let tagged: Value = serde_json::from_str(&line).unwrap();
        let value = untag(tagged);
        let card: OracleCard = serde_json::from_value(value.clone()).unwrap();
        let name = corpus::Identity::of(&card).unwrap().names.remove(0);
        assert_eq!(value, source[&name], "{name}");
        checked += 1;
    }
    assert_eq!(checked, source.len());
}
