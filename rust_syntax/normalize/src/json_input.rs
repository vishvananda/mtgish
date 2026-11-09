//! Shared bounded JSON input for native upgrades and the browser bridge.
use serde::Deserialize;
use serde_json::Value;

pub fn value(input: &str) -> Result<Value, String> {
    parse(input, 255)
}
/// An authoring document adds one object outside the exact semantic card.
pub(crate) fn wrapped_value(input: &str) -> Result<Value, String> {
    parse(input, 256)
}
fn parse(input: &str, maximum: usize) -> Result<Value, String> {
    if input.len() > 2 * 1024 * 1024 {
        return Err("JSON document exceeds 2 MiB".into());
    }
    let (mut depth, mut quoted, mut escaped) = (0usize, false, false);
    for b in input.bytes() {
        if quoted {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                quoted = false;
            }
        } else {
            match b {
                b'"' => quoted = true,
                b'{' | b'[' => {
                    depth += 1;
                    if depth > maximum {
                        return Err(format!("JSON nesting exceeds {maximum} levels"));
                    }
                }
                b'}' | b']' => depth = depth.saturating_sub(1),
                _ => (),
            }
        }
    }
    let mut reader = serde_json::Deserializer::from_str(input);
    reader.disable_recursion_limit();
    let value = Value::deserialize(&mut reader).map_err(|e| e.to_string())?;
    reader.end().map_err(|e| e.to_string())?;
    Ok(value)
}
