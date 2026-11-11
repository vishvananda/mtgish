//! Explicit schema188->189 adjustments for unversioned historical test captures.
//! Production persisted cards use the strict typed migration and binding maps.
use mtgish_ast::ir::{Destination, MovementDestination};
use serde_json::{json, Value};
fn test(comparison: Value, value: Value) -> Value {
    if value["kind"] == "AnyOf" {
        return json!({"kind":"AnyOf","args":value["args"].as_array().unwrap().iter().map(|v| test(comparison.clone(),v.clone())).collect::<Vec<_>>()});
    }
    if value["kind"] == "Aggregate" && value["args"]["aggregation"] == "AnyOf" {
        return json!({"kind":"AnyProperty","args":{"comparison":comparison,"objects":value["args"]["objects"],"property":value["args"]["property"]}});
    }
    json!({"kind":"Compare","args":{"comparison":comparison,"value":value}})
}
fn event(value: &mut Value) {
    if let Some(args) = value.as_object_mut().and_then(|v| v.remove("Either")) {
        value["AnyOf"] = args;
    }
    match value {
        Value::Object(fields) => fields.values_mut().for_each(event),
        Value::Array(items) => items.iter_mut().for_each(event),
        _ => {}
    }
}
pub fn json(value: &mut Value) {
    let kind = value["kind"].as_str().unwrap_or("").to_owned();
    if value["args"].is_array() && matches!(kind.as_str(), "All" | "Either") {
        value["kind"] = json!(if kind == "All" { "AllOf" } else { "AnyOf" });
    }
    if matches!(kind.as_str(), "Compare" | "Property" | "LimitX")
        && value["args"].get("comparison").is_some()
        && (kind != "Compare" || value["args"].get("left").is_some())
    {
        let args = value["args"].as_object_mut().unwrap();
        let comparison = args.remove("comparison").unwrap();
        let rhs = args
            .remove(if kind == "Compare" { "right" } else { "value" })
            .unwrap();
        args.insert("test".into(), test(comparison, rhs));
    }
    if let Some(args) = value.get_mut("NthEachTurn").and_then(Value::as_object_mut) {
        if let Some(comparison) = args.remove("comparison") {
            let rhs = args.remove("value").unwrap();
            args.insert("test".into(), test(comparison, rhs));
        }
    }
    if kind == "CounterWith" && value["args"].get("entry").is_none() {
        let args = value["args"].as_object_mut().unwrap();
        let object = args.remove("object").unwrap();
        let when = args.remove("when").unwrap();
        args.insert("objects".into(), json!({"kind":"One","args":object}));
        args.insert("destination_when".into(), when);
        value["kind"] = json!("Counter");
    }
    if kind == "Times" {
        value["kind"] = json!("RepeatableWithLimit");
    }
    if kind == "Counter" && value["args"].get("destination_when").is_none() {
        let old: Destination =
            serde_json::from_value(value["args"]["destination"].clone()).unwrap();
        let destination: MovementDestination = old.into();
        value["args"]["destination"] = serde_json::to_value(destination).unwrap();
        value["args"]["destination_when"] = Value::Null;
    }
    if matches!(
        kind.as_str(),
        "Triggered" | "CreateTrigger" | "CreateDelayedTrigger" | "EventHistory" | "Event"
    ) {
        if let Some(event_value) = value.get_mut("args").and_then(|v| v.get_mut("event")) {
            event(event_value);
        }
        if kind == "Event" {
            event(&mut value["args"]);
        }
    }
    if let Some(event_value) = value.get_mut("UntilEvent") {
        event(event_value);
    }
}

pub fn tree(value: &mut Value) {
    match value {
        Value::Object(fields) => fields.values_mut().for_each(tree),
        Value::Array(items) => items.iter_mut().for_each(tree),
        _ => {}
    }
    json(value);
}

// Small unquoted-token edits for captures that predate portable JSON epochs.
// RON keeps the original enum payload types; arbitrary quoted card text is skipped.
pub fn ron_text(input: &str) -> String {
    fn mask(text: &str) -> Vec<u8> {
        let mut result = text.as_bytes().to_vec();
        let (mut quoted, mut escaped) = (false, false);
        for byte in &mut result {
            let c = *byte;
            if quoted {
                if escaped {
                    escaped = false;
                } else if c == b'\\' {
                    escaped = true;
                } else if c == b'"' {
                    quoted = false;
                }
                *byte = b' ';
            } else if c == b'"' {
                quoted = true;
                *byte = b' ';
            }
        }
        result
    }
    fn end(mask: &[u8], start: usize) -> usize {
        let mut depth = 1;
        for (offset, byte) in mask[start..].iter().enumerate() {
            if *byte == b'(' {
                depth += 1;
            } else if *byte == b')' {
                depth -= 1;
            }
            if depth == 0 {
                return start + offset;
            }
        }
        panic!("unterminated captured RON record")
    }
    fn fields(text: &str) -> Vec<(&str, &str)> {
        let m = mask(text);
        let (mut depth, mut from) = (0i32, 0);
        let mut out = vec![];
        for (i, c) in m.iter().enumerate() {
            match c {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                b',' if depth == 0 => {
                    if let Some((k, v)) = text[from..i].split_once(':') {
                        out.push((k.trim(), v.trim()));
                    }
                    from = i + 1;
                }
                _ => {}
            }
        }
        if let Some((k, v)) = text[from..].split_once(':') {
            out.push((k.trim(), v.trim()));
        }
        out
    }
    let mut out = input.to_owned();
    let replacements = [
        ("event:Either([", "event:AnyOf(["),
        ("kind:All,args:", "kind:AllOf,args:"),
        ("kind:Either,args:", "kind:AnyOf,args:"),
        ("kind:Times,args:", "kind:RepeatableWithLimit,args:"),
    ];
    for (old, new) in replacements {
        let m = String::from_utf8(mask(&out)).unwrap();
        for (at, _) in m.match_indices(old).collect::<Vec<_>>().into_iter().rev() {
            out.replace_range(at..at + old.len(), new);
        }
    }
    let m = String::from_utf8(mask(&out)).unwrap();
    let mut sites = vec![];
    for kind in ["Compare", "Property", "LimitX", "Counter"] {
        let prefix = format!("kind:{kind},args:(");
        for (at, _) in m.match_indices(&prefix) {
            sites.push((at + prefix.len(), kind));
        }
    }
    sites.sort_by_key(|(at, _)| *at);
    for (start, kind) in sites.into_iter().rev() {
        let finish = end(&mask(&out), start);
        let fs = fields(&out[start..finish]);
        let get = |key: &str| fs.iter().find(|(k, _)| *k == key).map(|(_, v)| *v);
        let body = if kind == "Counter" && get("destination_when").is_none() {
            let d: Destination = ron::from_str(get("destination").unwrap()).unwrap();
            let d: MovementDestination = d.into();
            format!(
                "objects:{},destination:{},destination_when:None",
                get("objects").unwrap(),
                ron::to_string(&d).unwrap()
            )
        } else if let (Some(c), Some(v)) = (
            get("comparison"),
            get(if kind == "Compare" { "right" } else { "value" }),
        ) {
            let kept: Vec<_> = fs
                .iter()
                .filter(|(k, _)| !matches!(*k, "comparison" | "value" | "right"))
                .map(|(k, v)| format!("{k}:{v}"))
                .collect();
            format!(
                "{}{}test:(kind:Compare,args:(comparison:{c},value:{v}))",
                kept.join(","),
                if kept.is_empty() { "" } else { "," }
            )
        } else {
            continue;
        };
        out.replace_range(start..finish, &body);
    }
    out
}

pub fn assert_origins(old: &Value, mapped: &mtgish_ast::migrate::MappedMigration) {
    let current = serde_json::to_value(&mapped.card).unwrap();
    for (from, destinations) in &mapped.map.sites {
        assert_eq!(destinations.len(), 1, "{from}");
        let before = old.pointer(from).expect("old binding origin exists");
        let after = current
            .pointer(&destinations[0])
            .expect("new binding site exists");
        assert_eq!(before, after, "{from} -> {}", destinations[0]);
    }
}
