//! The checked-in legacy tagged Source JSON must describe the same complete
//! records as parser RON, even though default Rust serde uses external tags.
use mtgish_ast::{corpus, mtg_types::OracleCard};
use serde_json::Value;
use std::{collections::BTreeMap, io::BufRead};
fn untag(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.into_iter().map(untag).collect()),
        Value::Object(mut fields) => {
            if let Some(tag) = fields.keys().find(|key| key.starts_with('_')).cloned() {
                let variant = fields.remove(&tag).unwrap().as_str().unwrap().to_owned();
                if fields.is_empty() {
                    return Value::String(variant);
                }
                let payload = if fields.len() == 1 && fields.contains_key("args") {
                    untag(fields.remove("args").unwrap())
                } else {
                    untag(Value::Object(fields))
                };
                Value::Object([(variant, payload)].into_iter().collect())
            } else {
                Value::Object(
                    fields
                        .into_iter()
                        .map(|(key, value)| (key, untag(value)))
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
        let name = tagged["Name"].as_str().unwrap().to_owned();
        let value = untag(tagged);
        assert_eq!(value, source[&name], "{name}");
        let _: OracleCard = serde_json::from_value(value).unwrap();
        checked += 1;
    }
    assert_eq!(checked, source.len());
}
